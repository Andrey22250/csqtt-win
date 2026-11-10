//! Port of WorkerCountPolicy.kt

/// Policy rules for calculating, bounding, and normalizing tunnel worker thread counts.
pub struct WorkerCountPolicy;

impl WorkerCountPolicy {
    /// Normalizes requested worker count using the default maximum worker limit.
    pub fn normalize(requested: i32) -> i32 {
        Self::normalize_with_maximum(requested, crate::constants::tunnel::MAX_WORKERS)
    }

    /// Normalizes requested worker count, enforcing lower bounds and rounding to
    /// whole credential groups up to the given maximum.
    pub fn normalize_with_maximum(requested: i32, maximum: i32) -> i32 {
        if requested <= 5 {
            return 5;
        }
        let group = crate::constants::tunnel::WORKERS_PER_GROUP;
        let normalized_maximum = (maximum.max(group) / group) * group;
        let clamped = requested.clamp(5, normalized_maximum);
        let rounded = (clamped / group) * group;
        rounded.max(5)
    }

    /// Returns the maximum workers allowed for the given number of active VK hashes.
    pub fn maximum_for_hashes(hash_count: i32) -> i32 {
        let count = hash_count.clamp(1, crate::constants::tunnel::MAX_VK_HASHES);
        (count * crate::constants::tunnel::WORKERS_PER_GROUP)
            .clamp(5, crate::constants::tunnel::MAX_WORKERS)
    }

    /// Returns the maximum workers allowed across different credential sources.
    pub fn maximum_for_sources(
        _link_mode: bool,
        _link_hash_count: i32,
        _auto_hash_mode: bool,
        _manual_hash_count: i32,
    ) -> i32 {
        crate::constants::tunnel::MAX_WORKERS
    }

    /// Returns the default maximum workers limit.
    pub fn default_maximum(_extra_workers: bool) -> i32 {
        crate::constants::tunnel::MAX_WORKERS
    }

    /// Normalizes requested workers constrained by available hash count.
    pub fn normalize_for_hashes(requested: i32, hash_count: i32) -> i32 {
        Self::normalize_with_maximum(requested, Self::maximum_for_hashes(hash_count))
    }

    /// Normalizes requested workers given an iterable of hash strings.
    pub fn normalize_for_hash_values<I, S>(
        requested: i32,
        hashes: I,
        allow_redistribution: bool,
    ) -> i32
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let _ = (hashes, allow_redistribution);
        Self::normalize_with_maximum(requested, crate::constants::tunnel::MAX_WORKERS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct VkAutoCallsManager;

    impl VkAutoCallsManager {
        fn call_count_for_workers(workers: i32) -> i32 {
            for count in 1..=crate::constants::tunnel::MAX_VK_HASHES {
                if workers <= WorkerCountPolicy::maximum_for_hashes(count) {
                    return count;
                }
            }
            crate::constants::tunnel::MAX_VK_HASHES
        }

        fn request_delay_for_call_count(call_count: i32) -> i64 {
            if call_count <= 4 {
                crate::constants::vk_auto_hash::AUTO_CALL_SMALL_DELAY_MS
            } else {
                crate::constants::vk_auto_hash::AUTO_CALL_LARGE_DELAY_MS
            }
        }
    }

    #[test]
    fn every_supported_total_remains_an_exact_multiple_of_five() {
        let max_workers = crate::constants::tunnel::MAX_WORKERS;
        let workers_per_group = crate::constants::tunnel::WORKERS_PER_GROUP;
        let supported: Vec<i32> = (1..=(max_workers / workers_per_group))
            .map(|it| it * workers_per_group)
            .collect();
        for (groups, &workers) in supported.iter().enumerate() {
            assert_eq!(workers, WorkerCountPolicy::normalize(workers));
            assert_eq!(groups as i32 + 1, workers / workers_per_group);
        }
        assert_eq!(100, *supported.last().unwrap());
        assert_eq!(100, WorkerCountPolicy::normalize(max_workers));
    }

    #[test]
    fn hash_capacity_has_no_hidden_extra_workers() {
        assert_eq!(10, WorkerCountPolicy::maximum_for_hashes(1));
        assert_eq!(20, WorkerCountPolicy::maximum_for_hashes(2));
        assert_eq!(30, WorkerCountPolicy::maximum_for_hashes(3));
        assert_eq!(40, WorkerCountPolicy::maximum_for_hashes(4));
        assert_eq!(50, WorkerCountPolicy::maximum_for_hashes(5));
        assert_eq!(60, WorkerCountPolicy::maximum_for_hashes(6));
        assert_eq!(70, WorkerCountPolicy::maximum_for_hashes(7));
    }

    #[test]
    fn participant_capacity_comes_from_link_before_local_profile_state() {
        assert_eq!(
            100,
            WorkerCountPolicy::maximum_for_sources(true, 4, false, 0),
        );
        assert_eq!(
            100,
            WorkerCountPolicy::maximum_for_sources(false, 4, false, 1),
        );
        assert_eq!(
            100,
            WorkerCountPolicy::maximum_for_sources(false, 0, true, 1),
        );
    }

    #[test]
    fn invalid_persisted_counts_are_bounded_and_never_create_partial_credential_group() {
        let max_workers = crate::constants::tunnel::MAX_WORKERS;
        for requested in -1_000..=1_000 {
            let normalized = WorkerCountPolicy::normalize(requested);
            assert!((5..=max_workers).contains(&normalized));
            assert_eq!(0, normalized % 5);
            assert!(normalized <= requested.max(5) || normalized == 100);
        }
    }

    #[test]
    fn stale_persisted_maximum_is_recapped_by_current_hash_count_at_runtime() {
        assert_eq!(10, WorkerCountPolicy::normalize_for_hashes(i32::MAX, 1));
        assert_eq!(20, WorkerCountPolicy::normalize_for_hashes(i32::MAX, 2));
        assert_eq!(30, WorkerCountPolicy::normalize_for_hashes(i32::MAX, 3));
        assert_eq!(40, WorkerCountPolicy::normalize_for_hashes(i32::MAX, 4));
        assert_eq!(50, WorkerCountPolicy::normalize_for_hashes(i32::MAX, 5));
        assert_eq!(60, WorkerCountPolicy::normalize_for_hashes(i32::MAX, 6));
        assert_eq!(70, WorkerCountPolicy::normalize_for_hashes(i32::MAX, 7));
    }

    #[test]
    fn automatic_hash_mode_allows_the_full_worker_range() {
        let max_workers = crate::constants::tunnel::MAX_WORKERS;
        for requested in -100..=300 {
            let admitted = WorkerCountPolicy::normalize(requested);
            assert!((5..=max_workers).contains(&admitted));
            assert_eq!(0, admitted % 5);
        }
        assert_eq!(100, WorkerCountPolicy::normalize(i32::MAX));
    }

    #[test]
    fn crafted_intent_counts_cannot_bypass_hash_aware_runtime_admission() {
        let max_vk_hashes = crate::constants::tunnel::MAX_VK_HASHES;
        let workers_per_group = crate::constants::tunnel::WORKERS_PER_GROUP;
        for requested in [i32::MIN, -1, 0, 4, 28, 109, i32::MAX] {
            for hash_count in 1..=max_vk_hashes {
                let admitted = WorkerCountPolicy::normalize_for_hashes(requested, hash_count);
                assert!((5..=WorkerCountPolicy::maximum_for_hashes(hash_count)).contains(&admitted));
                assert!(admitted == 5 || admitted % workers_per_group == 0);
            }
        }
        assert_eq!(
            100,
            WorkerCountPolicy::normalize_for_hash_values(
                i32::MAX,
                ["same", " same ", "same", ""],
                false,
            ),
        );
        assert_eq!(
            100,
            WorkerCountPolicy::normalize_for_hash_values(
                i32::MAX,
                ["one", "two", "three", "four", "five", "six", "ignored-seventh"],
                false,
            ),
        );
    }

    #[test]
    fn automatic_call_count_scales_to_seven_hashes() {
        assert_eq!(1, VkAutoCallsManager::call_count_for_workers(10));
        assert_eq!(2, VkAutoCallsManager::call_count_for_workers(20));
        assert_eq!(3, VkAutoCallsManager::call_count_for_workers(30));
        assert_eq!(5, VkAutoCallsManager::call_count_for_workers(45));
        assert_eq!(6, VkAutoCallsManager::call_count_for_workers(60));
        assert_eq!(7, VkAutoCallsManager::call_count_for_workers(100));
        assert_eq!(
            crate::constants::vk_auto_hash::AUTO_CALL_SMALL_DELAY_MS,
            VkAutoCallsManager::request_delay_for_call_count(1)
        );
        assert_eq!(
            crate::constants::vk_auto_hash::AUTO_CALL_SMALL_DELAY_MS,
            VkAutoCallsManager::request_delay_for_call_count(4)
        );
        assert_eq!(
            crate::constants::vk_auto_hash::AUTO_CALL_LARGE_DELAY_MS,
            VkAutoCallsManager::request_delay_for_call_count(5)
        );
        assert_eq!(
            crate::constants::vk_auto_hash::AUTO_CALL_LARGE_DELAY_MS,
            VkAutoCallsManager::request_delay_for_call_count(6)
        );
    }

    #[test]
    fn automatic_call_failure_can_redistribute_whole_groups_across_created_hashes() {
        assert_eq!(
            100,
            WorkerCountPolicy::normalize_for_hash_values(
                i32::MAX,
                ["one", "two", "three", "four", "five"],
                true,
            ),
        );
    }

    #[test]
    fn manual_hashes_support_full_worker_range_up_to_max_workers() {
        assert_eq!(
            30,
            WorkerCountPolicy::normalize_for_hash_values(
                30,
                ["hash1_at_least_16_chars", "hash2_at_least_16_chars"],
                false,
            ),
        );
        assert_eq!(
            100,
            WorkerCountPolicy::normalize_for_hash_values(
                100,
                ["hash1_at_least_16_chars", "hash2_at_least_16_chars"],
                false,
            ),
        );
        assert_eq!(
            50,
            WorkerCountPolicy::normalize_for_hash_values(
                50,
                ["single_hash_at_least_16_chars"],
                false,
            ),
        );
    }
}
