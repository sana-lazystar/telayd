//! Panic-catch + error-restart supervisor wrapper.
//!
//! Wraps the daemon's inner async loop by spawning it as a `tokio` task and
//! examining the `JoinHandle` result. This captures both:
//!   - `Err(e)` — task returned an error (restart with cooldown).
//!   - `JoinError::is_panic()` — task panicked (restart with cooldown).
//!
//! ADR (IG-r2-3): path-(b) "True fix" chosen over path-(a) "Honest doc".
//! Rationale: architecture.md §14 Failure 3 explicitly promises panic-catch
//! and restart as the L0 mitigation for NFR-2 / K1 / K5. Keeping the promise
//! is safer than downgrading the doc, especially because unwrap() panics in
//! daemon code are plausible during L0 dogfooding. `tokio::spawn` + JoinHandle
//! is the idiomatic async-safe mechanism (no `catch_unwind` needed).
//!
//! Security (§5.8): supervisor catches panics only at the spawned-task boundary.
//! Sub-tasks spawned inside the daemon loop use their own cancellation tokens
//! and are not directly supervised here.

use anyhow::Result;
use tracing::{error, info, warn};

/// Maximum number of consecutive restarts before giving up.
const MAX_RESTARTS: u32 = 5;

/// Runs `f` in a supervised loop, catching both errors and panics, restarting
/// on either.
///
/// Internally spawns the future returned by `f` as a `tokio` task so that
/// `JoinHandle::await` exposes panic information via `JoinError::is_panic()`.
///
/// Returns `Ok(())` after `f` returns `Ok(())` (clean exit).
/// Returns `Err` if the restart count exceeds `MAX_RESTARTS`.
pub async fn supervised_run<F, Fut>(f: F) -> Result<()>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    let mut restarts: u32 = 0;

    loop {
        // Spawn the future as a separate task so panics are caught via JoinHandle.
        let handle = tokio::task::spawn(f());

        match handle.await {
            // Clean exit.
            Ok(Ok(())) => {
                info!(target: "supervisor", "daemon exited cleanly");
                return Ok(());
            }
            // Task returned an error — restart.
            Ok(Err(e)) => {
                restarts += 1;
                error!(
                    target: "supervisor",
                    err = %e,
                    restarts,
                    "daemon error — restarting"
                );
                if restarts >= MAX_RESTARTS {
                    anyhow::bail!(
                        "daemon restarted {restarts} times, giving up: {e}"
                    );
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            // Task panicked — restart (IG-r2-3: JoinError::is_panic path).
            Err(join_err) if join_err.is_panic() => {
                restarts += 1;
                warn!(
                    target: "supervisor",
                    restarts,
                    "daemon panicked — restarting"
                );
                if restarts >= MAX_RESTARTS {
                    anyhow::bail!(
                        "daemon panicked {restarts} times, giving up"
                    );
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            // Task was cancelled (should not happen in normal operation).
            Err(join_err) => {
                anyhow::bail!("daemon task cancelled unexpectedly: {join_err}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn supervisor_restarts_on_error() {
        let count = Arc::new(AtomicU32::new(0));
        let count2 = count.clone();

        let result = supervised_run(move || {
            let c = count2.clone();
            async move {
                let v = c.fetch_add(1, Ordering::Relaxed);
                if v < 2 {
                    Err(anyhow::anyhow!("transient error #{v}"))
                } else {
                    Ok(())
                }
            }
        })
        .await;

        assert!(result.is_ok(), "should succeed after 2 restarts");
        assert_eq!(count.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn supervisor_gives_up_after_max_restarts() {
        let result = supervised_run(|| async { Err(anyhow::anyhow!("always fails")) }).await;
        assert!(result.is_err(), "should give up after MAX_RESTARTS");
    }

    /// IG-r2-3 regression: supervisor must catch a panic and restart.
    ///
    /// path-(b) implementation: tokio::spawn wraps the future so JoinHandle
    /// exposes the panic via JoinError::is_panic(). This test injects one panic,
    /// verifies the restart counter increments, and then lets the task succeed.
    ///
    /// Pattern-wide grep: `grep -rn "JoinError::is_panic\|join_err.is_panic"
    ///   packages/daemon/src/supervisor.rs` → ≥1 hit.
    #[tokio::test]
    async fn supervisor_restarts_on_panic() {
        let count = Arc::new(AtomicU32::new(0));
        let count2 = count.clone();

        let result = supervised_run(move || {
            let c = count2.clone();
            async move {
                let v = c.fetch_add(1, Ordering::Relaxed);
                // First call panics; second call succeeds.
                // Use `assert!(false, ...)` to avoid `clippy::if_then_panic`.
                assert!(v != 0, "injected test panic on first call");
                Ok(())
            }
        })
        .await;

        assert!(result.is_ok(), "should succeed after 1 panic restart");
        // Called twice: once panicked, once succeeded.
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }
}
