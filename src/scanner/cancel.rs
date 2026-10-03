use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Marker returned by [`CancelToken::check`] when a run has been cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("indexing run was cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// Cooperative cancellation handle shared by every stage of one indexing run.
///
/// A plain `Arc<AtomicBool>` is not enough here. `tokio::task::JoinHandle::abort`
/// cannot stop a `spawn_blocking` stage, so cancelling a run and immediately
/// starting a new one used to leave the previous run's walker/filter/parser/writer
/// threads alive and writing into the index. They then had their cancel flag
/// flipped back to `false` by the new run and carried on.
///
/// Instead, the controller hands out a *generation* number. A token is cancelled
/// the moment the controller's generation moves past the one the token was
/// created with, so a superseded run can detect it even if nobody ever awaits or
/// aborts its task handle.
#[derive(Debug, Clone)]
pub struct CancelToken {
    shared: Arc<AtomicU64>,
    generation: u64,
}

impl CancelToken {
    /// A token that is never cancelled.
    #[must_use]
    pub fn never() -> Self {
        Self {
            shared: Arc::new(AtomicU64::new(0)),
            generation: 0,
        }
    }

    /// True once this run has been cancelled or superseded by a newer run.
    ///
    /// Cheap enough (one atomic load) to call per file.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.generation != 0 && self.shared.load(Ordering::Acquire) != self.generation
    }

    /// Checks cancellation once and returns `Err(())` when cancelled.
    ///
    /// Lets call sites use `?` inside loops without repeating the comparison.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` when this run has been cancelled or superseded.
    pub fn check(&self) -> Result<(), Cancelled> {
        if self.is_cancelled() {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Issues [`CancelToken`]s and invalidates outstanding runs.
///
/// Cloning shares the same generation counter, so every clone controls the same
/// set of runs.
#[derive(Debug, Clone, Default)]
pub struct CancellationController {
    shared: Arc<AtomicU64>,
}

impl CancellationController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Invalidates any in-flight run and returns a token for a new one.
    #[must_use]
    pub fn begin(&self) -> CancelToken {
        // `fetch_add` returns the previous value, so this is always a fresh,
        // strictly increasing generation and never 0.
        let generation = self.shared.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        CancelToken {
            shared: Arc::clone(&self.shared),
            generation,
        }
    }

    /// Invalidates the current run, if any.
    pub fn cancel(&self) {
        self.shared.fetch_add(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_run_cancels_the_previous_one() {
        let controller = CancellationController::new();
        let first = controller.begin();
        let second = controller.begin();

        assert!(
            first.is_cancelled(),
            "superseded run must observe cancellation"
        );
        assert!(!second.is_cancelled());
    }

    #[test]
    fn explicit_cancel_stops_the_current_run() {
        let controller = CancellationController::new();
        let token = controller.begin();
        assert!(!token.is_cancelled());

        controller.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn clones_share_one_controller() {
        let controller = CancellationController::new();
        let handle = controller.clone();
        let token = controller.begin();

        assert!(!token.is_cancelled());
        handle.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn never_cancelled_token_ignores_the_controller() {
        let token = CancelToken::never();
        assert!(!token.is_cancelled());
        assert!(token.check().is_ok());
    }

    #[test]
    fn check_short_circuits() {
        let controller = CancellationController::new();
        let token = controller.begin();
        assert!(token.check().is_ok());
        controller.cancel();
        assert_eq!(token.check(), Err(Cancelled));
        assert_eq!(
            token.check().unwrap_err().to_string(),
            "indexing run was cancelled"
        );
    }
}
