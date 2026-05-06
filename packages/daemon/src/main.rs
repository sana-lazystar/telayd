//! Telayd daemon entry point.
//!
//! Dispatches to subcommands defined in `cli.rs`.

#![forbid(unsafe_code)]

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{error, info};

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
        Commands::Status => cmd_status(),
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

    // Write PID file.
    let pid_path = paths::daemon_pid()?;
    let pid = std::process::id();
    paths::write_secret_file(&pid_path, pid.to_string().as_bytes())
        .context("write PID file")?;

    // Lock PID file to prevent duplicate daemon.
    // (Simple check: if another process is already running with our PID file.)
    // For L0, a basic existence + stale PID check suffices.

    let cfg = config::Config::load().context("load config (run `telayd init` first)")?;

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
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = fwd_cancel.cancelled() => break,
                    Some(inq) = inquiry_rx.recv() => {
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
    let tunnel_handle = tokio::spawn(async move {
        cf_tunnel::run_tunnel(cf_cancel, token_for_cf, move |url| {
            info!(target: "cli", "tunnel URL: {}", &url[..url.len().min(40)]);
            config_for_tunnel.last_tunnel_url = Some(url);
            let _ = config_for_tunnel.save();
        })
        .await
    });

    // Setup shutdown signal handler.
    let cancel_for_signal = cancellation.clone();
    tokio::spawn(async move {
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
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            info!(target: "cli", "Ctrl-C received — shutting down");
            cancel_for_ctrlc.cancel();
        }
    });

    println!("Telayd daemon started. PID {pid}");
    println!("Press Ctrl-C or send SIGTERM to stop.");

    // Wait for all tasks.
    let (ipc_res, ws_res, tunnel_res) =
        tokio::join!(ipc_handle, ws_handle, tunnel_handle);

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

    // Remove PID file.
    let _ = std::fs::remove_file(&pid_path);
    info!(target: "cli", "daemon stopped");
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

    kill(Pid::from_raw(pid), Signal::SIGTERM).context("send SIGTERM")?;
    println!("SIGTERM sent to PID {pid}.");
    Ok(())
}

// ── Logs ──────────────────────────────────────────────────────────────────────

fn cmd_logs(lines: u32) -> Result<()> {
    let log_path = paths::daemon_log()?;

    // Scan from the config dir for the most-recent telayd-*.log file.
    // IG10 fix: deployment-plan uses `telayd-YYYYMMDD.log`; glob updated to match.
    let config_dir = paths::config_dir()?;
    let mut log_files: Vec<_> = std::fs::read_dir(&config_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("telayd-")
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

fn cmd_status() -> Result<()> {
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
    let (tunnel_url, token_masked) = match cfg {
        Ok(c) => {
            let t = &c.pairing_token;
            let masked = format!("{}***", &t[..4.min(t.len())]);
            (
                c.last_tunnel_url
                    .unwrap_or_else(|| "(none yet)".to_string()),
                masked,
            )
        }
        Err(_) => ("(config not found — run `telayd init`)".to_string(), "(none)".to_string()),
    };

    println!("Daemon:       {}", if daemon_running { &pid_str } else { "not running" });
    println!("Token:        {token_masked}");
    println!("Tunnel URL:   {tunnel_url}");
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
        assert!(matches!(cli.command, Commands::Status));
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
