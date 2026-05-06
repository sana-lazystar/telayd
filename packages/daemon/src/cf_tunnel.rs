//! Cloudflared Quick Tunnel Manager.
//!
//! Responsibilities (ADR-W004):
//! 1. Auto-install `cloudflared` binary with SHA256 verification.
//! 2. Spawn `cloudflared tunnel --url http://localhost:7777`.
//! 3. Capture the tunnel URL from stderr (regex match).
//! 4. Update `config.toml.last_tunnel_url` on capture / rotation.
//! 5. Fire macOS notification with the URL + token.
//! 6. On URL rotation, reprint + re-fire notification.
//!
//! Security (§5.4 checklist):
//! - Download URL hardcoded (no user input).
//! - SHA256 verify before install.
//! - SHA256SUMS fetched via TLS from the same release channel.
//! - xattr call uses argv array (no shell concat).
//! - Tunnel URL validated by strict regex before use.
//! - Subprocess env minimised (HOME + PATH only).
//! - SIGTERM → 5s → SIGKILL.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{info, warn};

// ── Constants ─────────────────────────────────────────────────────────────────

const CF_DOWNLOAD_URL: &str = "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-arm64.tgz";
const CF_SHA256SUMS_URL: &str = "https://github.com/cloudflare/cloudflared/releases/latest/download/SHA256SUMS";
const CF_ARCHIVE_FILENAME: &str = "cloudflared-darwin-arm64.tgz";

/// Strict regex: `^https://[a-z0-9-]{1,63}\.trycloudflare\.com$`
fn is_valid_tunnel_url(url: &str) -> bool {
    let Some(host) = url.strip_prefix("https://") else {
        return false;
    };
    let Some(subdomain) = host.strip_suffix(".trycloudflare.com") else {
        return false;
    };
    !subdomain.is_empty()
        && subdomain.len() <= 63
        && subdomain
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !subdomain.starts_with('-')
        && !subdomain.ends_with('-')
}

// ── Install ───────────────────────────────────────────────────────────────────

/// Returns the path to the `cloudflared` binary.
pub fn cloudflared_bin() -> Result<PathBuf> {
    Ok(crate::paths::local_bin()?.join("cloudflared"))
}

/// Checks whether `cloudflared` is already installed and functional.
pub async fn is_installed() -> bool {
    let Ok(bin) = cloudflared_bin() else {
        return false;
    };
    if !bin.exists() {
        return false;
    }
    Command::new(&bin)
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Installs `cloudflared` if not already present.
pub async fn ensure_installed() -> Result<PathBuf> {
    let bin = cloudflared_bin()?;
    if is_installed().await {
        info!(target: "cf_tunnel", bin = ?bin, "cloudflared already installed");
        return Ok(bin);
    }
    install_cloudflared().await
}

async fn install_cloudflared() -> Result<PathBuf> {
    let bin = cloudflared_bin()?;
    info!(target: "cf_tunnel", "downloading cloudflared...");

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .context("build reqwest client")?;

    // Download archive to tempfile.
    let archive_bytes = fetch_bytes(&client, CF_DOWNLOAD_URL).await?;
    info!(
        target: "cf_tunnel",
        bytes = archive_bytes.len(),
        "cloudflared archive downloaded"
    );

    // Fetch SHA256SUMS.
    let sha256sums = fetch_text(&client, CF_SHA256SUMS_URL).await?;
    let expected_sha = parse_sha256sums(&sha256sums, CF_ARCHIVE_FILENAME)?;

    // Verify.
    let actual_sha = sha256_bytes(&archive_bytes);
    if actual_sha != expected_sha {
        anyhow::bail!(
            "SHA256 mismatch for cloudflared — possible supply chain attack. \
             expected={expected_sha}, got={actual_sha}"
        );
    }
    info!(target: "cf_tunnel", "SHA256 verified");

    // Extract the binary from the .tgz archive.
    extract_cloudflared_binary(&archive_bytes, &bin)?;

    // chmod +x
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("chmod 0755 {bin:?}"))?;

    // Remove macOS quarantine bit.
    Command::new("xattr")
        .args(["-d", "com.apple.quarantine"])
        .arg(&bin)
        .status()
        .await
        .context("xattr -d quarantine")?;

    info!(target: "cf_tunnel", bin = ?bin, "cloudflared installed");
    Ok(bin)
}

async fn fetch_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("HTTP error {url}"))?;
    let bytes = resp.bytes().await.context("read response bytes")?;
    Ok(bytes.to_vec())
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("HTTP error {url}"))?;
    resp.text().await.context("read response text")
}

fn parse_sha256sums(content: &str, filename: &str) -> Result<String> {
    for line in content.lines() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() == 2 {
            let hash = parts[0].trim();
            let name = parts[1].trim().trim_start_matches('*');
            if name == filename {
                return Ok(hash.to_lowercase());
            }
        }
    }
    anyhow::bail!("SHA256 entry for {filename:?} not found in SHA256SUMS")
}

fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn extract_cloudflared_binary(archive_bytes: &[u8], dest: &PathBuf) -> Result<()> {
    use std::io::Read;

    let gz = flate2::read::GzDecoder::new(std::io::Cursor::new(archive_bytes));
    let mut tar = tar::Archive::new(gz);

    for entry in tar.entries().context("iterate tar entries")? {
        let mut entry = entry.context("read tar entry")?;
        let path = entry.path().context("entry path")?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if name == "cloudflared" {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).context("read cloudflared from archive")?;
            crate::paths::write_secret_file(dest, &buf)
                .with_context(|| format!("write binary {dest:?}"))?;
            // Fix to executable mode after write_secret_file (which forces 0600).
            std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))
                .context("chmod 0755 cloudflared")?;
            return Ok(());
        }
    }
    anyhow::bail!("cloudflared binary not found in archive")
}

// ── Tunnel spawn ──────────────────────────────────────────────────────────────

/// Spawns the cloudflared quick tunnel and captures the URL.
///
/// Runs until the `cancellation_token` is cancelled.
/// Calls `on_url` whenever a new tunnel URL is detected (initial + rotation).
pub async fn run_tunnel<F>(
    cancellation_token: Arc<tokio_util::sync::CancellationToken>,
    pairing_token: String,
    on_url: F,
) -> Result<()>
where
    F: Fn(String) + Send + 'static,
{
    let bin = cloudflared_bin()?;

    // Minimise environment (HOME + PATH only).
    let home = std::env::var("HOME").unwrap_or_default();
    let path_env = std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".to_string());

    let mut child = Command::new(&bin)
        .args(["tunnel", "--url", "http://localhost:7777"])
        .env_clear()
        .env("HOME", &home)
        .env("PATH", &path_env)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawn cloudflared")?;

    info!(target: "cf_tunnel", "cloudflared spawned");

    let stderr = child.stderr.take().context("no stderr")?;
    let stdout = child.stdout.take().context("no stdout")?;

    let token = pairing_token.clone();
    let on_url = Arc::new(on_url);
    let on_url2 = on_url.clone();

    // Read stderr for URL.
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if let Some(url) = extract_tunnel_url(&line) {
                info!(
                    target: "cf_tunnel",
                    event = "url_rotation",
                    url_prefix = &url[..url.len().min(40)],
                    "tunnel URL captured"
                );
                fire_notification(&url, &token);
                on_url(url);
            }
        }
    });

    // Drain stdout.
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if let Some(url) = extract_tunnel_url(&line) {
                info!(target: "cf_tunnel", event = "url_rotation_stdout", "URL on stdout");
                on_url2(url);
            }
        }
    });

    // Wait for cancellation or child exit.
    tokio::select! {
        _ = cancellation_token.cancelled() => {
            info!(target: "cf_tunnel", "tunnel cancellation requested");
            graceful_kill(&mut child).await;
        }
        status = child.wait() => {
            match status {
                Ok(s) => info!(target: "cf_tunnel", exit_status = ?s, "cloudflared exited"),
                Err(e) => warn!(target: "cf_tunnel", err = %e, "cloudflared wait error"),
            }
        }
    }

    stderr_task.abort();
    stdout_task.abort();
    Ok(())
}

/// Extracts `https://*.trycloudflare.com` from a log line.
fn extract_tunnel_url(line: &str) -> Option<String> {
    // Find the URL in the line.
    let start = line.find("https://")?;
    let rest = &line[start..];
    // Take up to the first whitespace or end.
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '|')
        .unwrap_or(rest.len());
    let candidate = rest[..end].trim_end_matches('/');
    if is_valid_tunnel_url(candidate) {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// Sends a macOS native notification.
fn fire_notification(url: &str, pairing_token: &str) {
    let token_masked = if pairing_token.len() >= 4 {
        format!("{}***", &pairing_token[..4])
    } else {
        "****".to_string()
    };

    // Print for user visibility.
    println!(
        "\nTunnel: {}#token={}\nmacOS notification fired.",
        url, pairing_token
    );

    let script = format!(
        r#"display notification "Open: {}#token={}" with title "Telayd" sound name "Glass""#,
        url, token_masked
    );

    // Fire-and-forget — failure is non-fatal.
    let _ = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output();
}

async fn graceful_kill(child: &mut tokio::process::Child) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    if let Some(pid) = child.id().map(|id| Pid::from_raw(id as i32)) {
        let _ = kill(pid, Signal::SIGTERM);
        let timeout = tokio::time::sleep(Duration::from_secs(5));
        tokio::select! {
            _ = child.wait() => {}
            _ = timeout => {
                warn!(target: "cf_tunnel", "SIGKILL after 5s timeout");
                let _ = child.kill().await;
            }
        }
    } else {
        let _ = child.kill().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_verifier_matches() {
        let data = b"hello world";
        let hash = sha256_bytes(data);
        // Known SHA256 of "hello world"
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576e359c25cc10e2f98"
                .trim_end_matches('\n')
        );
        // More precisely:
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576e359c25cc10e2f98";
        // Note: actual SHA256("hello world") is:
        // b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576e359c25cc10e2f98 — 63 chars
        // Let's use a known-good value:
        let mut hasher = Sha256::new();
        hasher.update(b"hello world");
        let correct = hex::encode(hasher.finalize());
        assert_eq!(hash, correct);
        let _ = expected; // suppress warning
    }

    #[test]
    fn url_regex_valid_cases() {
        assert!(is_valid_tunnel_url("https://abc123.trycloudflare.com"));
        assert!(is_valid_tunnel_url("https://random-name.trycloudflare.com"));
        assert!(is_valid_tunnel_url("https://a.trycloudflare.com"));
    }

    #[test]
    fn url_regex_rejects_invalid() {
        assert!(!is_valid_tunnel_url("http://abc.trycloudflare.com")); // no https
        assert!(!is_valid_tunnel_url("https://abc.trycloudflare.com/path")); // path
        assert!(!is_valid_tunnel_url("https://evil.example.com")); // wrong domain
        assert!(!is_valid_tunnel_url("https://ABC.trycloudflare.com")); // uppercase
        assert!(!is_valid_tunnel_url("https://.trycloudflare.com")); // empty subdomain
        assert!(!is_valid_tunnel_url("https://-bad.trycloudflare.com")); // leading dash
    }

    #[test]
    fn extract_tunnel_url_from_log_line() {
        let line = "2026-05-06T07:38:36Z INF +--------------------------------------------------------------------------------------------+";
        assert!(extract_tunnel_url(line).is_none());

        let line2 =
            "2026-05-06T07:38:36Z INF  | https://random-x1y2.trycloudflare.com |";
        assert_eq!(
            extract_tunnel_url(line2),
            Some("https://random-x1y2.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn parse_sha256sums_finds_entry() {
        let sums = "abc123def456  cloudflared-darwin-arm64.tgz\n\
                    deadbeef0000  cloudflared-linux-amd64.tgz\n";
        let result = parse_sha256sums(sums, "cloudflared-darwin-arm64.tgz").unwrap();
        assert_eq!(result, "abc123def456");
    }

    #[test]
    fn parse_sha256sums_not_found() {
        let sums = "abc123  cloudflared-linux-amd64.tgz\n";
        assert!(parse_sha256sums(sums, "cloudflared-darwin-arm64.tgz").is_err());
    }
}
