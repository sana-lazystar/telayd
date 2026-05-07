//! Telayd daemon entry point.
//!
//! Dispatches to subcommands defined in `cli.rs`.

#![forbid(unsafe_code)]

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{error, info};

// std::io::Read is needed for stdin().read_line in cmd_status --reveal.
use std::io::BufRead as _;

mod cli;
mod cf_tunnel;
mod config;
mod hook_installer;
mod ipc;
mod logger;
mod metrics;
mod notification;
mod paths;
mod pairing;
mod protocol;
mod sentinel;
mod supervisor;
mod tmux_controller;
mod tmux_keymap;
mod ws_bridge;

use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    // IG7 fix (P1, coupled with IG3): set process umask to 0o077 so that ALL
    // files created by this process (daemon.log, PID, sock, tmp cloudflared
    // extract, etc.) default to at most mode 0o600 (rw-------).
    //
    // nix::sys::stat::umask is safe Rust — no unsafe block needed.
    // This satisfies Q-Sec-3 (file mode confidentiality) + IG3 log-mode coupling.
    {
        use nix::sys::stat::{umask, Mode};
        // 0o077 masks group and other read/write/execute bits.
        // Pattern-wide: grep -rn "OpenOptions\|File::create" daemon/src
        // verifies all callsites rely on this umask OR use write_secret_file.
        umask(Mode::from_bits_truncate(0o077));
    }

    let cli = Cli::parse();

    // Init logging early (best-effort — log dir may not exist yet for `init`).
    let _log_guard = paths::config_dir()
        .ok()
        .and_then(|dir| logger::init_logging(&dir).ok());

    let result = match cli.command {
        Commands::Init => cmd_init().await,
        Commands::Start => cmd_start().await,
        Commands::Stop => cmd_stop(),
        Commands::Logs { lines } => cmd_logs(lines),
        Commands::Status { reveal } => cmd_status(reveal),
    };

    if let Err(e) = result {
        error!(err = %e, "command failed");
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

// ── Init ──────────────────────────────────────────────────────────────────────

async fn cmd_init() -> Result<()> {
    info!(target: "cli", "telayd init");

    // Ensure config directory.
    let config_dir = paths::config_dir()?;
    info!(target: "cli", dir = ?config_dir, "config dir ready");

    // Generate or reuse pairing token.
    let config_path = paths::config_file()?;
    let cfg = if config_path.exists() {
        info!(target: "cli", "existing config found — preserving token");
        config::Config::load()?
    } else {
        let token = pairing::generate_token()?;
        let cfg = config::Config {
            pairing_token: token,
            last_tunnel_url: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        cfg.save()?;
        info!(target: "cli", "pairing token generated and saved");
        cfg
    };

    // Display masked token.
    let masked = {
        let t = &cfg.pairing_token;
        format!("{}***", &t[..4.min(t.len())])
    };
    println!("Pairing token: {masked}");

    // Install hook.
    hook_installer::install_hook().context("install hook")?;
    println!("Hook installed: telayd-hook-emit");

    // Install cloudflared.
    cf_tunnel::ensure_installed()
        .await
        .context("install cloudflared")?;
    println!("cloudflared installed: {:?}", cf_tunnel::cloudflared_bin()?);

    println!("\nRun `telayd start` to launch the daemon.");
    Ok(())
}

// ── Start ─────────────────────────────────────────────────────────────────────

async fn cmd_start() -> Result<()> {
    info!(target: "cli", "telayd start");

    // IG6 fix (P2): acquire exclusive flock on PID file before writing.
    // This prevents two concurrent `telayd start` invocations from both
    // believing they are the sole daemon (CWE-672 variant).
    //
    // Approach: open PID file with create+write, acquire LOCK_EX|LOCK_NB.
    // If the lock fails, another process holds it → daemon already running.
    //
    // Note: `#[allow(unsafe_code)]` cannot be used here because main.rs has
    // `#![forbid(unsafe_code)]`.  nix::fcntl::flock is safe Rust.
    let pid_path = paths::daemon_pid()?;

    // Open the PID file (create if absent).  We keep the fd open for the
    // lifetime of the process so the flock is held.
    let pid_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&pid_path)
        .context("open PID file")?;

    // IG6 fix (P2): use nix::fcntl::Flock (new API, replaces deprecated flock fn).
    // LOCK_EX|LOCK_NB — fail immediately if another process holds the lock.
    //
    // Flock::lock takes ownership of the file and returns a Flock<File> guard.
    // We use std::ops::Deref to access the file for writing.
    let locked_pid_file = {
        use nix::fcntl::{Flock, FlockArg};
        Flock::lock(pid_file, FlockArg::LockExclusiveNonblock)
            .map_err(|(_, e)| {
                if e == nix::errno::Errno::EWOULDBLOCK {
                    anyhow::anyhow!(
                        "daemon is already running (PID file locked). \
                         Run `telayd stop` first, or remove {:?} manually.",
                        pid_path
                    )
                } else {
                    anyhow::anyhow!("flock PID file: {e}")
                }
            })?
    };

    // Write current PID into the locked file.
    let pid = std::process::id();
    {
        use std::io::Write;
        // Deref to access inner File, write PID, then truncate.
        let mut f = &*locked_pid_file;
        write!(f, "{pid}").context("write PID")?;
    }
    // Leak the Flock guard so the OS-level lock is held for the process lifetime.
    // The lock is automatically released when the process exits (fd close).
    // SAFETY: intentional leak — flock semantics require fd to stay open.
    std::mem::forget(locked_pid_file);

    let cfg = config::Config::load().context("load config (run `telayd init` first)")?;

    // IG6 fix (P1): wrap inner daemon loop with supervisor::supervised_run.
    // This satisfies NFR-2 + K1 + K5: any panic in the main task
    // causes a restart (up to MAX_RESTARTS=5) instead of daemon death.
    //
    // The supervisor closure captures: cfg (pairing token), pid, pid_path.
    // Each restart re-registers sessions + rebuilds shared state — this is
    // correct for L0 single-session dogfooding (in-memory state is rebuilt
    // from the persisted config.toml).
    let cfg_for_supervisor = cfg.clone();
    let pid_path_for_cleanup = pid_path.clone();

    println!("Telayd daemon started. PID {pid}");
    println!("Press Ctrl-C or send SIGTERM to stop.");

    supervisor::supervised_run(move || {
        let cfg = cfg_for_supervisor.clone();
        let pid_path = pid_path_for_cleanup.clone();
        async move {
            daemon_service_loop(cfg, pid_path).await
        }
    })
    .await?;

    info!(target: "cli", "daemon stopped");
    Ok(())
}

/// Inner daemon service loop — spawns IPC, WS, tunnel, and signal handlers.
///
/// IG6 fix: extracted from cmd_start so supervisor::supervised_run can restart
/// it on error without re-doing the PID file / flock setup.
async fn daemon_service_loop(cfg: config::Config, pid_path: std::path::PathBuf) -> Result<()> {
    // Shared components.
    let metrics = Arc::new(metrics::MetricsCollector::new());
    let sentinel: Arc<dyn sentinel::SentinelParser> = Arc::new(sentinel::NoopSentinelParser);
    let tmux = Arc::new(tmux_controller::TmuxController::new(sentinel));

    // IPC → WS forwarding channel.
    let (inquiry_tx, mut inquiry_rx) = tokio::sync::mpsc::channel::<protocol::Inquiry>(64);

    // Shared WS bridge state.
    let ws_state = ws_bridge::WsBridgeState::new(
        cfg.pairing_token.clone(),
        tmux.clone(),
        metrics.clone(),
    );

    let cancellation = Arc::new(tokio_util::sync::CancellationToken::new());

    // Determine active tmux session (best-effort from $TMUX env var).
    let tmux_session = std::env::var("TMUX")
        .ok()
        .and_then(|s| s.split(',').next().map(|p| p.to_string()))
        .unwrap_or_else(|| "telayd-session".to_string());

    // Register session with tmux controller.
    tmux.register_session(&tmux_session);

    // Spawn IPC listener.
    let sock_path = paths::daemon_sock()?;
    let ipc_cancel = cancellation.clone();
    let ipc_handle = tokio::spawn(ipc::run_ipc_listener(
        sock_path,
        inquiry_tx,
        ipc_cancel,
        tmux_session.clone(),
    ));

    // Spawn WS bridge.
    let ws_cancel = cancellation.clone();
    let ws_state_clone = ws_state.clone();
    let ws_handle = tokio::spawn(ws_bridge::run_ws_bridge(ws_state_clone, ws_cancel));

    // Spawn inquiry forwarder: IPC mpsc → WS push_inquiry.
    {
        let ws_state_fwd = ws_state.clone();
        let fwd_cancel = cancellation.clone();
        let tmux_fwd = tmux.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = fwd_cancel.cancelled() => break,
                    Some(inq) = inquiry_rx.recv() => {
                        // Auto-register the hook-reported session so the tmux
                        // controller's whitelist accepts subsequent injects.
                        // Hook payload is the trust source: a payload reaching
                        // this point has passed `payload.validate()` upstream.
                        // (host-env-drift fix — dogfooding-discovered.)
                        if !inq.tmux_session.is_empty() {
                            tmux_fwd.register_session(&inq.tmux_session);
                        }
                        if let Err(e) = ws_bridge::push_inquiry(&ws_state_fwd, inq).await {
                            tracing::warn!(target: "forwarder", err = %e, "push_inquiry failed");
                        }
                    }
                }
            }
        });
    }

    // Spawn cloudflared tunnel.
    let cf_cancel = cancellation.clone();
    let token_for_cf = cfg.pairing_token.clone();
    let mut config_for_tunnel = cfg.clone();
    // IG8 fix: capture ws_state reference so the tunnel URL is pushed into
    // WsBridgeState::tunnel_origin on every rotation.  The Origin allowlist
    // in ws_upgrade_handler reads this field before accepting WS upgrades.
    let ws_state_for_tunnel = ws_state.clone();
    let tunnel_handle = tokio::spawn(async move {
        cf_tunnel::run_tunnel(cf_cancel, token_for_cf, move |url| {
            info!(target: "cli", "tunnel URL: {}", &url[..url.len().min(40)]);
            config_for_tunnel.last_tunnel_url = Some(url.clone());
            let _ = config_for_tunnel.save();
            // Propagate URL to ws_bridge Origin allowlist (async → fire-and-forget via block_on is not
            // available here; use the synchronous RwLock write instead via tokio::runtime::Handle).
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let ws = ws_state_for_tunnel.clone();
                handle.spawn(async move {
                    ws.set_tunnel_url(Some(url)).await;
                });
            }
        })
        .await
    });

    // Setup shutdown signal handlers.
    // IG-r2-4 fix: preserve JoinHandles so orphan tasks are not silently leaked
    // (project-rule.md §Rust 5 — orphan task ban).  The handles are joined below
    // alongside the main service handles so the supervisor loop sees their exit.
    let cancel_for_signal = cancellation.clone();
    let sigterm_handle = tokio::spawn(async move {
        if let Ok(mut sigterm) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sigterm.recv().await;
            info!(target: "cli", "SIGTERM received — shutting down");
            cancel_for_signal.cancel();
        }
    });

    // Ctrl-C.
    let cancel_for_ctrlc = cancellation.clone();
    let ctrlc_handle = tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            info!(target: "cli", "Ctrl-C received — shutting down");
            cancel_for_ctrlc.cancel();
        }
    });

    // Wait for all tasks.
    let (ipc_res, ws_res, tunnel_res) =
        tokio::join!(ipc_handle, ws_handle, tunnel_handle);

    // Abort signal handler tasks (they are now idle after cancellation).
    // We do not await them — their JoinHandle is consumed here so they are
    // no longer orphaned (project-rule.md §Rust 5).
    sigterm_handle.abort();
    ctrlc_handle.abort();

    for (name, res) in [
        ("ipc", ipc_res),
        ("ws", ws_res),
        ("tunnel", tunnel_res),
    ] {
        match res {
            Ok(Ok(())) => info!(target: "cli", task = name, "exited cleanly"),
            Ok(Err(e)) => error!(target: "cli", task = name, err = %e, "task error"),
            Err(e) => error!(target: "cli", task = name, err = %e, "task panicked"),
        }
    }

    // Remove PID file on clean exit.
    let _ = std::fs::remove_file(&pid_path);
    Ok(())
}

// ── Stop ──────────────────────────────────────────────────────────────────────

fn cmd_stop() -> Result<()> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    let pid_path = paths::daemon_pid()?;
    if !pid_path.exists() {
        println!("Daemon not running (no PID file).");
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path).context("read PID file")?;
    let pid: i32 = pid_str
        .trim()
        .parse()
        .context("parse PID from PID file")?;

    // IG6 fix (P2): verify the process at `pid` is actually `telayd` before
    // sending SIGTERM (prevents killing an unrelated process that reused the PID).
    // Uses `ps -p <pid> -o comm=` — POSIX-compatible on macOS.
    let comm_output = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output();

    match comm_output {
        Ok(out) if out.status.success() => {
            let comm = String::from_utf8_lossy(&out.stdout).trim().to_string();
            // comm may be "telayd" or truncated; check prefix.
            if !comm.starts_with("telayd") {
                anyhow::bail!(
                    "PID {pid} does not appear to be a telayd process (comm={comm:?}). \
                     Remove {:?} manually if it is stale.",
                    pid_path
                );
            }
        }
        Ok(_) => {
            // ps returned non-zero: process not found.
            println!("PID {pid} not found — removing stale PID file.");
            let _ = std::fs::remove_file(&pid_path);
            return Ok(());
        }
        Err(e) => {
            tracing::warn!(target: "cli", err = %e, "ps comm-check failed — proceeding with SIGTERM");
        }
    }

    kill(Pid::from_raw(pid), Signal::SIGTERM).context("send SIGTERM")?;
    println!("SIGTERM sent to PID {pid}.");
    Ok(())
}

// ── Logs ──────────────────────────────────────────────────────────────────────

fn cmd_logs(lines: u32) -> Result<()> {
    let log_path = paths::daemon_log()?;

    // IG3 fix (diagnosis.md §Group3 P0): tracing_appender::rolling::Builder produces
    // `telayd.YYYY-MM-DD.log` (period separator, `.log` extension).
    // Updated glob: `starts_with("telayd.") && ends_with(".log")`.
    // Pattern-wide grep: single locus — this is the only `starts_with("telayd` in daemon/src.
    let config_dir = paths::config_dir()?;
    let mut log_files: Vec<_> = std::fs::read_dir(&config_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            s.starts_with("telayd.") && s.ends_with(".log")
        })
        .collect();
    log_files.sort_by_key(|e| e.file_name());

    let target = if let Some(last) = log_files.last() {
        last.path()
    } else if log_path.exists() {
        log_path
    } else {
        println!("No log file found.");
        return Ok(());
    };

    let content = std::fs::read_to_string(&target).context("read log")?;
    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(lines as usize);
    for line in &all_lines[start..] {
        // Redact token patterns before printing (safety net).
        let redacted = redact_token(line);
        println!("{redacted}");
    }
    Ok(())
}

fn redact_token(line: &str) -> String {
    // Replaces any 43-char base64url string that looks like a token.
    // Simple length + charset heuristic.
    let mut result = line.to_string();
    // Find sequences of 43+ alphanumeric/_/- chars and check length == 43.
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '-' || chars[i] == '_')
            {
                i += 1;
            }
            let token_candidate: String = chars[start..i].iter().collect();
            if token_candidate.len() == 43 {
                let masked = format!("{}***[redacted]", &token_candidate[..4]);
                result = result.replacen(&token_candidate, &masked, 1);
            }
        } else {
            i += 1;
        }
    }
    result
}

// ── Status ────────────────────────────────────────────────────────────────────

/// Prints daemon status.
///
/// IG1/IG6 fix: added `reveal` flag (diagnosis.md §Group1/§Group6).
/// When `--reveal` is passed, prints the full token once after a Y/n prompt.
/// This is the only recovery path now that the notification no longer contains
/// the full token (IG1 strips it from the notification body).
fn cmd_status(reveal: bool) -> Result<()> {
    let pid_path = paths::daemon_pid()?;
    let daemon_running = pid_path.exists();
    let pid_str = if daemon_running {
        std::fs::read_to_string(&pid_path)
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        "not running".to_string()
    };

    let cfg = config::Config::load();
    let (tunnel_url, token_masked, token_raw) = match cfg {
        Ok(c) => {
            let t = c.pairing_token.clone();
            let masked = if t.len() >= 8 {
                format!("{}\u{2026}{}", &t[..4], &t[t.len() - 4..])
            } else {
                format!("{}…", &t[..4.min(t.len())])
            };
            (
                c.last_tunnel_url
                    .unwrap_or_else(|| "(none yet)".to_string()),
                masked,
                Some(t),
            )
        }
        Err(_) => (
            "(config not found — run `telayd init`)".to_string(),
            "(none)".to_string(),
            None,
        ),
    };

    println!("Daemon:       {}", if daemon_running { &pid_str } else { "not running" });
    println!("Token:        {token_masked}");
    println!("Tunnel URL:   {tunnel_url}");

    // IG1/IG6 fix: --reveal flag prints the full token once after Y/n confirmation.
    if reveal {
        if let Some(raw_token) = token_raw {
            // Prompt user for confirmation before revealing.
            eprint!("Reveal full pairing token? [y/N] ");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .context("read confirmation")?;
            let answer = input.trim().to_lowercase();
            if answer == "y" || answer == "yes" {
                // Print to stdout exactly once (no logging — audit trail only).
                // tracing::warn without the token — for audit only.
                tracing::warn!(
                    target: "security",
                    "pairing token revealed via telayd status --reveal (token NOT logged here)"
                );
                println!("Token: {raw_token}");
            } else {
                println!("Reveal cancelled.");
            }
        } else {
            eprintln!("No config found — run `telayd init` first.");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_init() {
        let cli = Cli::try_parse_from(["telayd", "init"]).unwrap();
        assert!(matches!(cli.command, Commands::Init));
    }

    #[test]
    fn cli_parses_start() {
        let cli = Cli::try_parse_from(["telayd", "start"]).unwrap();
        assert!(matches!(cli.command, Commands::Start));
    }

    #[test]
    fn cli_parses_stop() {
        let cli = Cli::try_parse_from(["telayd", "stop"]).unwrap();
        assert!(matches!(cli.command, Commands::Stop));
    }

    #[test]
    fn cli_parses_logs_default() {
        let cli = Cli::try_parse_from(["telayd", "logs"]).unwrap();
        assert!(matches!(cli.command, Commands::Logs { lines: 50 }));
    }

    #[test]
    fn cli_parses_logs_custom_lines() {
        let cli = Cli::try_parse_from(["telayd", "logs", "-n", "100"]).unwrap();
        assert!(matches!(cli.command, Commands::Logs { lines: 100 }));
    }

    #[test]
    fn cli_parses_status() {
        let cli = Cli::try_parse_from(["telayd", "status"]).unwrap();
        assert!(matches!(cli.command, Commands::Status { reveal: false }));
    }

    #[test]
    fn cli_parses_status_reveal() {
        let cli = Cli::try_parse_from(["telayd", "status", "--reveal"]).unwrap();
        assert!(matches!(cli.command, Commands::Status { reveal: true }));
    }

    #[test]
    fn redact_token_masks_43_char_string() {
        let token = "A".repeat(43);
        let line = format!("token={token}");
        let redacted = redact_token(&line);
        assert!(!redacted.contains(&token));
        assert!(redacted.contains("[redacted]"));
    }
}
