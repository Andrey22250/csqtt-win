//! Port of TunnelLifecycleState.kt

use std::sync::Mutex;

/// Advances or computes the next tunnel generation ID given the current time
/// and the currently stored generation counter.
pub fn next_tunnel_generation_id(now_seconds: i64, current: i64) -> i64 {
    let next_current = if current == i64::MAX {
        i64::MAX
    } else {
        current + 1
    };
    now_seconds.max(next_current)
}

/// Reserves a generation ID taking into account current timestamp, a proposed
/// generation ID, and the persisted generation counter.
pub fn reserve_tunnel_generation_id(
    now_seconds: i64,
    proposed: i64,
    persisted: i64,
) -> i64 {
    next_tunnel_generation_id(now_seconds.max(proposed), persisted)
}

/// Identifies an active or reserved tunnel process incarnation by epoch and sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TunnelLifecycleTicket {
    pub epoch: i64,
    pub sequence: i64,
}

#[derive(Debug, Default)]
struct TunnelLifecycleStateInner {
    epoch: i64,
    sequence: i64,
    desired_running: bool,
    active: Option<TunnelLifecycleTicket>,
}

impl TunnelLifecycleStateInner {
    fn can_start(&self, expected_epoch: i64) -> bool {
        self.desired_running && self.epoch == expected_epoch
    }
}

/// Thread-safe coordinator for tunnel process lifecycle state transitions.
#[derive(Debug, Default)]
pub struct TunnelLifecycleState {
    inner: Mutex<TunnelLifecycleStateInner>,
}

impl TunnelLifecycleState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(TunnelLifecycleStateInner::default()),
        }
    }

    pub fn request_start(&self) -> i64 {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.epoch = Self::next(inner.epoch);
        inner.desired_running = true;
        inner.active = None;
        inner.epoch
    }

    pub fn request_restart(&self) -> Option<i64> {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if !inner.desired_running {
            return None;
        }
        inner.epoch = Self::next(inner.epoch);
        inner.active = None;
        Some(inner.epoch)
    }

    pub fn request_stop(&self) -> i64 {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.epoch = Self::next(inner.epoch);
        inner.desired_running = false;
        inner.active = None;
        inner.epoch
    }

    pub fn reserve_process(
        &self,
        expected_epoch: i64,
        process_is_null: bool,
    ) -> Option<TunnelLifecycleTicket> {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if !process_is_null || !inner.can_start(expected_epoch) || inner.active.is_some() {
            return None;
        }
        inner.sequence = Self::next(inner.sequence);
        let ticket = TunnelLifecycleTicket {
            epoch: expected_epoch,
            sequence: inner.sequence,
        };
        inner.active = Some(ticket);
        Some(ticket)
    }

    pub fn release_reservation(&self, ticket: TunnelLifecycleTicket) {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if inner.active == Some(ticket) {
            inner.active = None;
        }
    }

    pub fn process_ended(&self, ticket: TunnelLifecycleTicket) -> bool {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if inner.active != Some(ticket) {
            return false;
        }
        inner.active = None;
        inner.desired_running && inner.epoch == ticket.epoch
    }

    pub fn accepts(&self, ticket: TunnelLifecycleTicket) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.desired_running && inner.epoch == ticket.epoch && inner.active == Some(ticket)
    }

    pub fn can_start(&self, expected_epoch: i64) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.can_start(expected_epoch)
    }

    pub fn is_desired_running(&self) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        inner.desired_running
    }

    fn next(value: i64) -> i64 {
        if value == i64::MAX {
            1
        } else {
            value + 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_advances_when_clock_stalls_or_moves_backward() {
        assert_eq!(next_tunnel_generation_id(100, 100), 101);
        assert_eq!(next_tunnel_generation_id(50, 100), 101);
        assert_eq!(next_tunnel_generation_id(200, 100), 200);
        assert_eq!(next_tunnel_generation_id(0, i64::MAX), i64::MAX);
    }

    #[test]
    fn persistent_generation_reservation_dominates_clock_and_delayed_intent() {
        assert_eq!(101, reserve_tunnel_generation_id(50, 75, 100));
        assert_eq!(200, reserve_tunnel_generation_id(200, 75, 100));
        assert_eq!(250, reserve_tunnel_generation_id(200, 250, 100));
        assert_eq!(251, reserve_tunnel_generation_id(200, 250, 250));
    }

    #[test]
    fn only_newest_of_two_restart_delays_can_start() {
        let state = TunnelLifecycleState::new();
        let initial_epoch = state.request_start();
        let initial = state.reserve_process(initial_epoch, true).unwrap();
        let first_restart = state.request_restart().unwrap();
        let second_restart = state.request_restart().unwrap();

        assert!(!state.accepts(initial));
        assert!(state.reserve_process(first_restart, true).is_none());
        assert!(state.reserve_process(second_restart, true).is_some());
    }

    #[test]
    fn stop_during_restart_delay_cannot_resurrect_process() {
        let state = TunnelLifecycleState::new();
        let epoch = state.request_start();
        let process = state.reserve_process(epoch, true).unwrap();
        assert!(state.process_ended(process));

        state.request_stop();

        assert!(state.reserve_process(epoch, true).is_none());
        assert!(!state.is_desired_running());
    }

    #[test]
    fn stale_eof_cannot_detach_replacement() {
        let state = TunnelLifecycleState::new();
        let first_epoch = state.request_start();
        let first = state.reserve_process(first_epoch, true).unwrap();
        let second_epoch = state.request_restart().unwrap();
        let second = state.reserve_process(second_epoch, true).unwrap();

        assert!(!state.process_ended(first));
        assert!(state.accepts(second));
    }

    #[test]
    fn stale_process_logs_are_rejected() {
        let state = TunnelLifecycleState::new();
        let first_epoch = state.request_start();
        let first = state.reserve_process(first_epoch, true).unwrap();
        let second_epoch = state.request_restart().unwrap();
        let second = state.reserve_process(second_epoch, true).unwrap();

        assert!(!state.accepts(first));
        assert!(state.accepts(second));
    }

    #[test]
    fn restart_invalidates_the_old_process_and_issues_a_new_epoch() {
        let state = TunnelLifecycleState::new();
        let first_epoch = state.request_start();
        let first = state.reserve_process(first_epoch, true).unwrap();
        let restarted_epoch = state.request_restart().unwrap();

        assert!(!state.accepts(first));
        assert!(state.reserve_process(first_epoch, true).is_none());
        assert!(state.reserve_process(restarted_epoch, true).is_some());
    }

    #[test]
    fn process_presence_prevents_parallel_reservation() {
        let state = TunnelLifecycleState::new();
        let epoch = state.request_start();

        assert!(state.reserve_process(epoch, false).is_none());
        let first = state.reserve_process(epoch, true).unwrap();
        assert!(state.reserve_process(epoch, true).is_none());
        assert!(state.accepts(first));
    }

    #[test]
    fn ten_thousand_stale_delays_cannot_start_after_stop() {
        let state = TunnelLifecycleState::new();
        let mut epochs = Vec::with_capacity(10_000);
        state.request_start();
        for _ in 0..10_000 {
            epochs.push(state.request_restart().unwrap());
        }
        state.request_stop();

        for epoch in epochs {
            assert!(state.reserve_process(epoch, true).is_none());
        }
    }

    #[test]
    fn concurrent_reservations_create_exactly_one_process_ticket() {
        use std::sync::{Arc, Barrier, Mutex};

        let state = Arc::new(TunnelLifecycleState::new());
        let epoch = state.request_start();
        let barrier = Arc::new(Barrier::new(32));
        let results = Arc::new(Mutex::new(Vec::new()));

        let mut handles = Vec::with_capacity(32);
        for _ in 0..32 {
            let state_clone = Arc::clone(&state);
            let barrier_clone = Arc::clone(&barrier);
            let results_clone = Arc::clone(&results);
            handles.push(std::thread::spawn(move || {
                barrier_clone.wait();
                if let Some(ticket) = state_clone.reserve_process(epoch, true) {
                    results_clone.lock().unwrap().push(ticket);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let tickets = results.lock().unwrap().clone();
        assert_eq!(tickets.len(), 1);
        assert!(state.accepts(tickets[0]));
    }

    #[test]
    fn stale_eof_storm_cannot_detach_current_process() {
        let state = TunnelLifecycleState::new();
        let mut stale = Vec::with_capacity(10_000);

        for i in 0..10_000 {
            let epoch = if i == 0 {
                state.request_start()
            } else {
                state.request_restart().unwrap()
            };
            stale.push(state.reserve_process(epoch, true).unwrap());
        }
        let current_epoch = state.request_restart().unwrap();
        let current = state.reserve_process(current_epoch, true).unwrap();

        for ticket in stale {
            assert!(!state.process_ended(ticket));
        }
        assert!(state.accepts(current));
    }

    // --- Chaos tests (TunnelLifecycleStateChaosTest.kt) ---

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct ModelTicket {
        epoch: i64,
        sequence: i64,
    }

    #[derive(Default)]
    struct Model {
        epoch: i64,
        sequence: i64,
        desired_running: bool,
        active: Option<ModelTicket>,
    }

    impl Model {
        fn next(value: i64) -> i64 {
            if value == i64::MAX {
                1
            } else {
                value + 1
            }
        }

        fn start(&mut self) -> i64 {
            self.epoch = Self::next(self.epoch);
            self.desired_running = true;
            self.active = None;
            self.epoch
        }

        fn restart(&mut self) -> Option<i64> {
            if !self.desired_running {
                return None;
            }
            self.epoch = Self::next(self.epoch);
            self.active = None;
            Some(self.epoch)
        }

        fn stop(&mut self) -> i64 {
            self.epoch = Self::next(self.epoch);
            self.desired_running = false;
            self.active = None;
            self.epoch
        }

        fn reserve(
            &mut self,
            expected_epoch: i64,
            process_is_null: bool,
            allow_stale_epoch: bool,
        ) -> Option<ModelTicket> {
            if !process_is_null
                || !self.desired_running
                || (!allow_stale_epoch && self.epoch != expected_epoch)
                || self.active.is_some()
            {
                return None;
            }
            self.sequence = Self::next(self.sequence);
            let ticket = ModelTicket {
                epoch: expected_epoch,
                sequence: self.sequence,
            };
            self.active = Some(ticket);
            Some(ticket)
        }

        fn release(&mut self, ticket: ModelTicket) {
            if self.active == Some(ticket) {
                self.active = None;
            }
        }

        fn ended(&mut self, ticket: ModelTicket) -> bool {
            if self.active != Some(ticket) {
                return false;
            }
            self.active = None;
            self.desired_running && self.epoch == ticket.epoch
        }

        fn accepts(&self, ticket: ModelTicket) -> bool {
            self.desired_running && self.epoch == ticket.epoch && self.active == Some(ticket)
        }

        fn can_start(&self, expected_epoch: i64) -> bool {
            self.desired_running && self.epoch == expected_epoch
        }
    }

    fn next_random(value: i64) -> i64 {
        let mut next = value as u64;
        next ^= next << 13;
        next ^= next >> 7;
        next ^= next << 17;
        next as i64
    }

    fn remember<T>(values: &mut Vec<T>, value: T) {
        if values.len() == 512 {
            values.remove(0);
        }
        values.push(value);
    }

    fn remember_epoch(epochs: &mut Vec<i64>, actual: i64, expected: i64) -> bool {
        if actual != expected {
            return false;
        }
        remember(epochs, actual);
        true
    }

    fn remember_nullable_epoch(
        epochs: &mut Vec<i64>,
        actual: Option<i64>,
        expected: Option<i64>,
    ) -> bool {
        if actual != expected {
            return false;
        }
        if let Some(actual) = actual {
            remember(epochs, actual);
        }
        true
    }

    fn remember_ticket(
        values: &mut Vec<(TunnelLifecycleTicket, ModelTicket)>,
        value: (TunnelLifecycleTicket, ModelTicket),
    ) {
        if values.len() == 512 {
            values.remove(0);
        }
        values.push(value);
    }

    fn select_epoch(epochs: &[i64], random: i64) -> i64 {
        if epochs.is_empty() || (random & 0x80) != 0 {
            random
        } else {
            let idx = ((random as u64 >> 8) % epochs.len() as u64) as usize;
            epochs[idx]
        }
    }

    fn select_ticket(
        tickets: &[(TunnelLifecycleTicket, ModelTicket)],
        random: i64,
    ) -> Option<(TunnelLifecycleTicket, ModelTicket)> {
        if tickets.is_empty() {
            None
        } else {
            let idx = ((random as u64 >> 8) % tickets.len().max(1) as u64) as usize;
            tickets.get(idx).copied()
        }
    }

    fn run_trace(seed: i64, steps: usize) -> bool {
        let state = TunnelLifecycleState::new();
        let mut model = Model::default();
        let mut epochs = Vec::with_capacity(512);
        let mut tickets = Vec::with_capacity(512);
        let mut random = seed ^ 0x6a09_e667_f3bc_c909_i64;

        for _ in 0..steps {
            random = next_random(random);
            match ((random as u64 >> 1) % 8) as i32 {
                0 => {
                    if !remember_epoch(&mut epochs, state.request_start(), model.start()) {
                        return false;
                    }
                }
                1 => {
                    if !remember_nullable_epoch(
                        &mut epochs,
                        state.request_restart(),
                        model.restart(),
                    ) {
                        return false;
                    }
                }
                2 => {
                    if !remember_epoch(&mut epochs, state.request_stop(), model.stop()) {
                        return false;
                    }
                }
                3 => {
                    let expected = select_epoch(&epochs, random);
                    let process_is_null = (random & 0x400) == 0;
                    let actual = state.reserve_process(expected, process_is_null);
                    let expected_ticket = model.reserve(expected, process_is_null, false);
                    if actual.is_some() != expected_ticket.is_some() {
                        return false;
                    }
                    if let (Some(actual), Some(expected_ticket)) = (actual, expected_ticket) {
                        if actual.epoch != expected_ticket.epoch
                            || actual.sequence != expected_ticket.sequence
                        {
                            return false;
                        }
                        remember_ticket(&mut tickets, (actual, expected_ticket));
                    }
                }
                4 => {
                    if let Some(pair) = select_ticket(&tickets, random) {
                        state.release_reservation(pair.0);
                        model.release(pair.1);
                    }
                }
                5 => {
                    if let Some(pair) = select_ticket(&tickets, random) {
                        if state.process_ended(pair.0) != model.ended(pair.1) {
                            return false;
                        }
                    }
                }
                6 => {
                    if let Some(pair) = select_ticket(&tickets, random) {
                        if state.accepts(pair.0) != model.accepts(pair.1) {
                            return false;
                        }
                    }
                }
                _ => {
                    let expected = select_epoch(&epochs, random);
                    if state.can_start(expected) != model.can_start(expected) {
                        return false;
                    }
                }
            }
            if state.is_desired_running() != model.desired_running {
                return false;
            }
        }
        true
    }

    fn operation_trace(seed: i64, length: usize) -> Vec<i32> {
        let mut random = seed ^ 0x6a09_e667_f3bc_c909_i64;
        let mut trace = Vec::with_capacity(length);
        for _ in 0..length {
            random = next_random(random);
            trace.push(((random as u64 >> 1) % 8) as i32);
        }
        trace
    }

    #[test]
    fn deterministic_lifecycle_chaos_matches_independent_model() {
        let first_seed: i64 = std::env::var("CSQTT_SOAK_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let seeds: usize = std::env::var("CSQTT_ANDROID_LIFECYCLE_SEEDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .map(|s: usize| s.max(1))
            .unwrap_or(128);
        let steps: usize = std::env::var("CSQTT_ANDROID_LIFECYCLE_STEPS")
            .ok()
            .and_then(|s| s.parse().ok())
            .map(|s: usize| s.max(1))
            .unwrap_or(10_000);

        for offset in 0..seeds {
            let seed = first_seed + offset as i64;
            assert!(
                run_trace(seed, steps),
                "lifecycle diverged at reproducible seed {}",
                seed
            );
        }
    }

    #[test]
    fn lifecycle_fault_generator_is_reproducible_and_covers_every_transition() {
        let first = operation_trace(104_729, 10_000);
        let second = operation_trace(104_729, 10_000);
        let different = operation_trace(104_730, 10_000);
        assert_eq!(first, second);
        assert_ne!(first, different);
        let mut covered = [false; 8];
        for &op in &first {
            covered[op as usize] = true;
        }
        assert!(covered.iter().all(|&c| c));
    }

    #[test]
    fn lifecycle_oracle_rejects_stale_epoch_acceptance_mutation() {
        let state = TunnelLifecycleState::new();
        let mut model = Model::default();
        state.request_start();
        model.start();
        assert!(state.reserve_process(0, true).is_none());
        assert!(model.reserve(0, true, true).is_some());
    }

    #[test]
    fn concurrent_restart_pause_stop_storm_cannot_resurrect_after_final_stop() {
        use std::sync::{Arc, Barrier, Mutex};

        let state = Arc::new(TunnelLifecycleState::new());
        let epochs = Arc::new(Mutex::new(Vec::new()));
        let tickets = Arc::new(Mutex::new(Vec::new()));

        epochs.lock().unwrap().push(state.request_start());

        let threads = 16;
        let barrier = Arc::new(Barrier::new(threads));
        let mut handles = Vec::with_capacity(threads);

        for thread in 0..threads {
            let state_clone = Arc::clone(&state);
            let epochs_clone = Arc::clone(&epochs);
            let tickets_clone = Arc::clone(&tickets);
            let barrier_clone = Arc::clone(&barrier);

            handles.push(std::thread::spawn(move || {
                let mut random = thread as i64 + 1;
                barrier_clone.wait();

                for _ in 0..5_000 {
                    random = next_random(random);
                    match ((random as u64 >> 1) % 5) as i32 {
                        0 => {
                            if let Some(ep) = state_clone.request_restart() {
                                epochs_clone.lock().unwrap().push(ep);
                            }
                        }
                        1 => {
                            epochs_clone.lock().unwrap().push(state_clone.request_start());
                        }
                        2 => {
                            let epoch = {
                                let ep = epochs_clone.lock().unwrap();
                                if ep.is_empty() {
                                    None
                                } else {
                                    let idx = ((random as u64 >> 8) % ep.len() as u64) as usize;
                                    Some(ep[idx])
                                }
                            };
                            if let Some(epoch) = epoch {
                                if let Some(ticket) = state_clone.reserve_process(epoch, true) {
                                    tickets_clone.lock().unwrap().push(ticket);
                                }
                            }
                        }
                        3 => {
                            let ticket = {
                                let t = tickets_clone.lock().unwrap();
                                t.first().copied()
                            };
                            if let Some(ticket) = ticket {
                                state_clone.process_ended(ticket);
                            }
                        }
                        _ => {
                            let ticket = {
                                let t = tickets_clone.lock().unwrap();
                                t.first().copied()
                            };
                            if let Some(ticket) = ticket {
                                state_clone.release_reservation(ticket);
                            }
                        }
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        state.request_stop();
        assert!(!state.is_desired_running());
        for epoch in epochs.lock().unwrap().iter() {
            assert!(state.reserve_process(*epoch, true).is_none());
        }
        for ticket in tickets.lock().unwrap().iter() {
            assert!(!state.accepts(*ticket));
        }
    }
}
