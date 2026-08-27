//! Thread-local allocation scope used by profiling-memory system probes.

use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScopedAllocationMeasurement {
    pub calls: u64,
    pub bytes: u64,
}

#[derive(Clone, Copy, Default)]
struct ScopeState {
    active: bool,
    calls: u64,
    bytes: u64,
}

thread_local! {
    static SCOPE: Cell<ScopeState> = const { Cell::new(ScopeState {
        active: false,
        calls: 0,
        bytes: 0,
    }) };
}

pub fn begin() {
    SCOPE.with(|scope| {
        let state = scope.get();
        debug_assert!(!state.active, "profiling allocation scopes must not nest");
        scope.set(ScopeState {
            active: true,
            calls: 0,
            bytes: 0,
        });
    });
}

pub fn end() -> ScopedAllocationMeasurement {
    SCOPE.with(|scope| {
        let state = scope.replace(ScopeState::default());
        debug_assert!(state.active, "profiling allocation scope was not active");
        ScopedAllocationMeasurement {
            calls: state.calls,
            bytes: state.bytes,
        }
    })
}

pub fn record_allocation(size: usize) {
    SCOPE.with(|scope| {
        let mut state = scope.get();
        if !state.active {
            return;
        }
        state.calls = state.calls.saturating_add(1);
        state.bytes = state
            .bytes
            .saturating_add(u64::try_from(size).unwrap_or(u64::MAX));
        scope.set(state);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_counts_only_active_allocations_and_resets() {
        record_allocation(99);
        begin();
        record_allocation(8);
        record_allocation(13);
        assert_eq!(
            end(),
            ScopedAllocationMeasurement {
                calls: 2,
                bytes: 21,
            }
        );

        begin();
        assert_eq!(end(), ScopedAllocationMeasurement::default());
    }
}
