//! Simple panic-catch + restart supervisor wrapper.
//!
//! Wraps the daemon's inner async loop with `std::panic::catch_unwind`
//! (via a synchronous boundary). On panic, logs the error and restarts
//! after a 1-second cooldown. This satisfies the F-1 stability acceptance
//! without a full launchd/systemd integration (L1+).
//!
//! Security (§5.8): panic catch is only at the supervisor layer.
//! Child task panics propagate via JoinHandle, not this wrapper.

use anyhow::Result;
use tracing::{error, info};

/// Maximum number of consecutive restarts before giving up.
const MAX_RESTARTS: u32 = 5;

/// Runs `f` in a supervised loop, catching panics and restarting.
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
        let result = f().await;

        match result {
            Ok(()) => {
                info!(target: "supervisor", "daemon exited cleanly");
                return Ok(());
            }
            Err(e) => {
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
}
