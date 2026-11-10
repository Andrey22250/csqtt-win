//! Port of TunnelStartupProgress.kt.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// Stages of tunnel startup progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TunnelStartupStage {
    WaitingForCredentials,
    WaitingForTurnOrPeer,
    Ready,
}

/// Tracks tunnel startup progress concurrently.
#[derive(Debug, Default)]
pub struct TunnelStartupProgress {
    credentials: AtomicI32,
    ready: AtomicBool,
}

impl TunnelStartupProgress {
    /// Creates a new progress tracker.
    pub fn new() -> Self {
        Self {
            credentials: AtomicI32::new(0),
            ready: AtomicBool::new(false),
        }
    }

    /// Records that credentials were received, returning the incremented count.
    pub fn credential_received(&self) -> i32 {
        self.credentials.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Marks that the tunnel stream is ready.
    pub fn stream_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }

    /// Returns the current stage of tunnel startup.
    pub fn stage(&self) -> TunnelStartupStage {
        if self.ready.load(Ordering::SeqCst) {
            TunnelStartupStage::Ready
        } else if self.credentials.load(Ordering::SeqCst) > 0 {
            TunnelStartupStage::WaitingForTurnOrPeer
        } else {
            TunnelStartupStage::WaitingForCredentials
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn startup_stage_identifies_the_blocked_layer() {
        let progress = TunnelStartupProgress::new();
        assert_eq!(TunnelStartupStage::WaitingForCredentials, progress.stage());
        assert_eq!(1, progress.credential_received());
        assert_eq!(TunnelStartupStage::WaitingForTurnOrPeer, progress.stage());
        progress.stream_ready();
        assert_eq!(TunnelStartupStage::Ready, progress.stage());
    }

    #[test]
    fn concurrent_credential_events_are_never_lost() {
        let progress = Arc::new(TunnelStartupProgress::new());
        let count = 128;
        let mut handles = Vec::with_capacity(count);
        for _ in 0..count {
            let p = Arc::clone(&progress);
            handles.push(thread::spawn(move || {
                p.credential_received();
            }));
        }
        for handle in handles {
            handle.join().expect("thread join failed");
        }
        assert_eq!(129, progress.credential_received());
    }
}
