//! Port of NetworkPauseGate.kt

use std::sync::atomic::{AtomicBool, Ordering};

/// Atomic gate to pause and resume network activity across threads.
#[derive(Debug)]
pub struct NetworkPauseGate {
    paused: AtomicBool,
}

impl Default for NetworkPauseGate {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkPauseGate {
    /// Creates a new unpaused `NetworkPauseGate`.
    pub fn new() -> Self {
        Self {
            paused: AtomicBool::new(false),
        }
    }

    /// Atomically transitions from unpaused to paused.
    ///
    /// Returns true if this call changed the state from unpaused to paused.
    pub fn pause(&self) -> bool {
        self.paused
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Atomically transitions from paused to unpaused.
    ///
    /// Returns true if this call changed the state from paused to unpaused.
    pub fn resume(&self) -> bool {
        self.paused
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Returns true if the gate is currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Resets the gate to an unpaused state.
    pub fn reset(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    /// Forces the gate into a paused state.
    pub fn restore(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Barrier};

    #[test]
    fn concurrent_callbacks_can_pause_and_resume_only_once() {
        let gate = Arc::new(NetworkPauseGate::new());

        let g1 = Arc::clone(&gate);
        assert_eq!(1, concurrent_successes(move || g1.pause()));
        assert!(gate.is_paused());

        let g2 = Arc::clone(&gate);
        assert_eq!(1, concurrent_successes(move || g2.resume()));
        assert!(!gate.is_paused());
    }

    #[test]
    fn reset_and_restore_keep_the_state_deterministic() {
        let gate = NetworkPauseGate::new();
        gate.restore();
        assert!(gate.is_paused());
        gate.reset();
        assert!(!gate.is_paused());
        assert!(gate.pause());
        assert!(!gate.pause());
    }

    fn concurrent_successes<F>(operation: F) -> usize
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        let threads = 16;
        let barrier = Arc::new(Barrier::new(threads + 1));
        let successes = Arc::new(AtomicUsize::new(0));
        let op = Arc::new(operation);
        let mut handles = Vec::with_capacity(threads);

        for _ in 0..threads {
            let b = Arc::clone(&barrier);
            let s = Arc::clone(&successes);
            let op_clone = Arc::clone(&op);
            handles.push(std::thread::spawn(move || {
                b.wait();
                if op_clone() {
                    s.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        barrier.wait();

        for handle in handles {
            handle.join().expect("thread join failed");
        }

        successes.load(Ordering::SeqCst)
    }
}
