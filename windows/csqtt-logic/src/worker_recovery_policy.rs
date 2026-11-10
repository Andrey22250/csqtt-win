//! Port of WorkerRecoveryPolicy.kt

use std::sync::Mutex;

#[derive(Debug, Default)]
struct WorkerRecoveryPolicyInner {
    peak: i32,
    armed_target: i32,
}

/// Tracks worker thread availability drops and arms recovery restarts when active
/// worker counts drop to zero following a healthy cohort.
#[derive(Debug)]
pub struct WorkerRecoveryPolicy {
    minimum_peak: i32,
    inner: Mutex<WorkerRecoveryPolicyInner>,
}

impl Default for WorkerRecoveryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerRecoveryPolicy {
    pub fn new() -> Self {
        Self::with_minimum_peak(crate::constants::tunnel::WORKERS_PER_GROUP)
    }

    pub fn with_minimum_peak(minimum_peak: i32) -> Self {
        Self {
            minimum_peak,
            inner: Mutex::new(WorkerRecoveryPolicyInner::default()),
        }
    }

    pub fn observe(&self, active: i32) -> bool {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let bounded = active.max(0);
        inner.peak = inner.peak.max(bounded);
        if inner.armed_target != 0 && bounded >= inner.armed_target {
            inner.armed_target = 0;
            return true;
        }
        false
    }

    pub fn arm_at_zero(&self) -> Option<i32> {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if inner.armed_target != 0 {
            return Some(inner.armed_target);
        }
        if inner.peak < self.minimum_peak {
            return None;
        }
        inner.armed_target = inner.peak;
        Some(inner.armed_target)
    }

    pub fn should_recover(&self, active: i32, target: i32) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.armed_target == target && active.max(0) < target
    }

    pub fn reset(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.peak = 0;
        inner.armed_target = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_zero_does_not_arm_recovery() {
        let policy = WorkerRecoveryPolicy::new();
        assert!(policy.arm_at_zero().is_none());
        assert!(!policy.should_recover(0, 9));
    }

    #[test]
    fn zero_after_healthy_cohort_arms_one_recovery() {
        let policy = WorkerRecoveryPolicy::new();
        policy.observe(27);
        assert_eq!(Some(27), policy.arm_at_zero());
        assert_eq!(Some(27), policy.arm_at_zero());
        assert!(policy.should_recover(0, 27));
    }

    #[test]
    fn partial_recovery_does_not_hide_full_outage() {
        let policy = WorkerRecoveryPolicy::new();
        policy.observe(27);
        let target = policy.arm_at_zero().unwrap();
        policy.observe(6);
        assert!(policy.should_recover(6, target));
    }

    #[test]
    fn complete_recovery_disarms_restart() {
        let policy = WorkerRecoveryPolicy::new();
        policy.observe(27);
        let target = policy.arm_at_zero().unwrap();
        assert!(policy.observe(27));
        assert!(!policy.should_recover(27, target));
    }

    #[test]
    fn reset_requires_a_healthy_cohort_again() {
        let policy = WorkerRecoveryPolicy::new();
        policy.observe(54);
        assert_eq!(Some(54), policy.arm_at_zero());
        policy.reset();
        assert!(policy.arm_at_zero().is_none());
    }
}
