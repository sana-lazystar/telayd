//! CLI entry point — `telayd <subcommand>`.
//!
//! Subcommands:
//! - `init`   — generate token + install hook + install cloudflared.
//! - `start`  — start daemon (WS bridge + IPC + CF tunnel).
//! - `stop`   — send SIGTERM to running daemon.
//! - `logs`   — tail `daemon.log`.
//! - `status` — print last_tunnel_url + daemon state.
//! - `status` (with `--reveal`): print full token after Y/n confirmation.

use clap::{Parser, Subcommand};

/// Telayd — Claude Code mobile companion daemon.
#[derive(Debug, Parser)]
#[command(name = "telayd", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initialise: generate pairing token, install hook, install cloudflared.
    Init,
    /// Start the daemon (WS bridge + IPC listener + CF tunnel).
    Start,
    /// Stop the running daemon (SIGTERM).
    Stop,
    /// Tail the daemon log.
    Logs {
        /// Number of lines to show (default 50).
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: u32,
    },
    /// Print current status (last tunnel URL, daemon pid, mode).
    ///
    /// Use --reveal to print the full pairing token once (requires Y/n confirmation).
    /// This is the only safe recovery path after `telayd init` — the notification
    /// no longer includes the full token (IG1 fix).
    Status {
        /// Reveal the full pairing token to stdout after Y/n confirmation.
        ///
        /// Use this when you need to re-enter the token on a new device.
        /// The token is printed once and not logged.
        #[arg(long, default_value_t = false)]
        reveal: bool,
    },
}
