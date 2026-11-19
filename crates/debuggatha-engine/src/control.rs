//! What the caller of a long operation may do while it runs: ask it to stop, and hear how far it
//! has got. Work checks the token between steps and stops cleanly; it never leaves a stored file
//! half written, because the writes are the last steps and are not interrupted.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A request to stop, shared between whoever asks and whoever works.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// The steps of a review, in order (`PROTOCOL.md`, section 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Orient,
    Detect,
    Analyzers,
    Semantic,
    Sync,
}

impl Stage {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Orient => "orient",
            Self::Detect => "detect",
            Self::Analyzers => "analyzers",
            Self::Semantic => "semantic",
            Self::Sync => "sync",
        }
    }
}

type Listener = dyn Fn(Stage, u32, u32, &str) + Send + Sync;

/// Cancellation and progress for one operation.
#[derive(Clone, Default)]
pub struct Control {
    cancel: Cancel,
    listener: Option<Arc<Listener>>,
}

impl Control {
    /// No one to stop it and no one listening.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn new(cancel: Cancel) -> Self {
        Self {
            cancel,
            listener: None,
        }
    }

    /// Calls `listener(stage, done, total, message)` as the work advances.
    #[must_use]
    pub fn listening(
        mut self,
        listener: impl Fn(Stage, u32, u32, &str) + Send + Sync + 'static,
    ) -> Self {
        self.listener = Some(Arc::new(listener));
        self
    }

    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    #[must_use]
    pub fn token(&self) -> &Cancel {
        &self.cancel
    }

    pub fn report(&self, stage: Stage, done: usize, total: usize, message: &str) {
        if let Some(listener) = &self.listener {
            let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
            listener(stage, count(done), count(total), message);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // a poisoned lock in a test is a failed test
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn a_token_is_shared_and_progress_reaches_the_listener() {
        let cancel = Cancel::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let control = Control::new(cancel.clone()).listening(move |stage, done, total, _| {
            log.lock().unwrap().push((stage.name(), done, total));
        });
        assert!(!control.cancelled());
        control.report(Stage::Detect, 2, 10, "");
        cancel.cancel();
        assert!(control.cancelled());
        assert_eq!(*seen.lock().unwrap(), [("detect", 2, 10)]);
        Control::none().report(Stage::Sync, 0, 0, "no listener, no harm");
    }
}
