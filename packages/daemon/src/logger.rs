//! Thin `tracing` wrapper for the telayd daemon.
//!
//! Configures:
//! - `tracing_subscriber` with an `EnvFilter` (default INFO)
//! - A rolling file appender writing to `~/.config/telayd/daemon.log`
//!   with daily rotation (filename suffix YYYY-MM-DD).
//! - A stdout layer for interactive use.
//!
//! # Logging rules (security)
//! - **Never log the pairing token** or full hook payload content.
//! - Log `tool_use_id` and `session_id` for correlation.
//! - Free-text user responses: log length only.
//! - Tunnel URL: OK to log (public once rotated).

use anyhow::{Context, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

/// Initialises the global tracing subscriber.
///
/// Returns a [`WorkerGuard`] that must be kept alive for the duration of
/// the process — dropping it flushes and closes the log file.
pub fn init_logging(log_dir: &std::path::Path) -> Result<WorkerGuard> {
    // Ensure the log directory exists.
    std::fs::create_dir_all(log_dir)
        .with_context(|| format!("create log dir {log_dir:?}"))?;

    // Daily rolling file appender → telayd-YYYY-MM-DD.log
    // IG10 fix: deployment-plan.md specifies `telayd-YYYYMMDD.log` as the canonical
    // log filename; implementation was drifted to `daemon-<DATE>`.  Plan stays SSOT.
    let file_appender = tracing_appender::rolling::daily(log_dir, "telayd");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // ENV-filter: RUST_LOG or TELAYD_LOG_LEVEL, default INFO.
    let filter = EnvFilter::try_from_env("TELAYD_LOG_LEVEL")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // File layer (compact JSON-like via fmt default).
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false);

    // Stdout layer (human-readable).
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .with_target(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .try_init()
        .context("init tracing subscriber")?;

    Ok(guard)
}
