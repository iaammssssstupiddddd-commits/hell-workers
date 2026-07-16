use bevy::prelude::Resource;
use hw_core::constants::MAX_PATHFINDS_PER_FRAME;

/// Outcome of a budgeted pathfinding request.
///
/// `Deferred` is intentionally distinct from `Unreachable`: callers must keep
/// their current movement/task state and retry on a later frame when no core
/// A* slot remains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSearchResult<T> {
    Found(T),
    Unreachable,
    Deferred,
}

/// Runtime subsystem that initiated a core A* request.
///
/// The tag is intentionally attached at the budget boundary rather than at a
/// logical request boundary: a direct route and its adjacent fallback are two
/// separate core searches and are therefore counted separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathSearchCaller {
    ActorNew,
    ActorReuse,
    ActorRestFallback,
    Escape,
    TaskExecution,
    BucketTransport,
}

/// Capture-period core A* observations, available only in profiling builds.
#[cfg(feature = "profiling")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePathSearchMetrics {
    pub actor_new_core_searches: u64,
    pub actor_new_deferred: u64,
    pub actor_reuse_core_searches: u64,
    pub actor_reuse_deferred: u64,
    pub actor_rest_fallback_core_searches: u64,
    pub actor_rest_fallback_deferred: u64,
    pub escape_core_searches: u64,
    pub escape_deferred: u64,
    pub task_execution_core_searches: u64,
    pub task_execution_deferred: u64,
    pub bucket_transport_core_searches: u64,
    pub bucket_transport_deferred: u64,
    /// Sum of valid open-set pops across budgeted core A* invocations.
    pub expanded_nodes: u64,
    /// Largest single core A* expansion observed during the capture.
    pub max_expanded_nodes_per_search: u64,
}

#[cfg(feature = "profiling")]
impl RuntimePathSearchMetrics {
    fn record(&mut self, caller: PathSearchCaller, claimed: bool) {
        let counter = match (caller, claimed) {
            (PathSearchCaller::ActorNew, true) => &mut self.actor_new_core_searches,
            (PathSearchCaller::ActorNew, false) => &mut self.actor_new_deferred,
            (PathSearchCaller::ActorReuse, true) => &mut self.actor_reuse_core_searches,
            (PathSearchCaller::ActorReuse, false) => &mut self.actor_reuse_deferred,
            (PathSearchCaller::ActorRestFallback, true) => {
                &mut self.actor_rest_fallback_core_searches
            }
            (PathSearchCaller::ActorRestFallback, false) => &mut self.actor_rest_fallback_deferred,
            (PathSearchCaller::Escape, true) => &mut self.escape_core_searches,
            (PathSearchCaller::Escape, false) => &mut self.escape_deferred,
            (PathSearchCaller::TaskExecution, true) => &mut self.task_execution_core_searches,
            (PathSearchCaller::TaskExecution, false) => &mut self.task_execution_deferred,
            (PathSearchCaller::BucketTransport, true) => &mut self.bucket_transport_core_searches,
            (PathSearchCaller::BucketTransport, false) => &mut self.bucket_transport_deferred,
        };
        *counter = counter.saturating_add(1);
    }

    pub const fn total_core_searches(&self) -> u64 {
        self.actor_new_core_searches
            + self.actor_reuse_core_searches
            + self.actor_rest_fallback_core_searches
            + self.escape_core_searches
            + self.task_execution_core_searches
            + self.bucket_transport_core_searches
    }

    fn record_expanded_nodes(&mut self, expanded_nodes: u64) {
        self.expanded_nodes = self.expanded_nodes.saturating_add(expanded_nodes);
        self.max_expanded_nodes_per_search = self.max_expanded_nodes_per_search.max(expanded_nodes);
    }
}

/// Per-frame upper bound for core A* searches performed by runtime systems.
///
/// The phase ceiling can be tightened temporarily while retaining the same
/// frame-wide usage counter. This reserves capacity for a later phase without
/// treating a composite path request as one search.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePathSearchBudget {
    hard_limit: usize,
    phase_limit: usize,
    used: usize,
    #[cfg(feature = "profiling")]
    metrics: RuntimePathSearchMetrics,
}

impl Default for RuntimePathSearchBudget {
    fn default() -> Self {
        Self::new(MAX_PATHFINDS_PER_FRAME)
    }
}

impl RuntimePathSearchBudget {
    pub const fn new(hard_limit: usize) -> Self {
        Self {
            hard_limit,
            phase_limit: hard_limit,
            used: 0,
            #[cfg(feature = "profiling")]
            metrics: RuntimePathSearchMetrics {
                actor_new_core_searches: 0,
                actor_new_deferred: 0,
                actor_reuse_core_searches: 0,
                actor_reuse_deferred: 0,
                actor_rest_fallback_core_searches: 0,
                actor_rest_fallback_deferred: 0,
                escape_core_searches: 0,
                escape_deferred: 0,
                task_execution_core_searches: 0,
                task_execution_deferred: 0,
                bucket_transport_core_searches: 0,
                bucket_transport_deferred: 0,
                expanded_nodes: 0,
                max_expanded_nodes_per_search: 0,
            },
        }
    }

    /// Starts a new frame. Call this before runtime pathfinding consumers run.
    pub fn reset(&mut self) {
        self.used = 0;
        self.phase_limit = self.hard_limit;
    }

    /// Caps the current phase while preserving searches already used this frame.
    pub fn begin_phase(&mut self, phase_limit: usize) {
        self.phase_limit = phase_limit.min(self.hard_limit);
    }

    /// Reserves one core A* invocation.
    pub fn try_claim(&mut self) -> bool {
        if self.used >= self.phase_limit {
            return false;
        }

        self.used += 1;
        true
    }

    /// Reserves one core A* invocation and attributes it to a runtime caller.
    pub fn try_claim_for(&mut self, caller: PathSearchCaller) -> bool {
        let claimed = self.try_claim();
        #[cfg(feature = "profiling")]
        self.metrics.record(caller, claimed);
        #[cfg(not(feature = "profiling"))]
        let _ = caller;
        claimed
    }

    pub const fn used(&self) -> usize {
        self.used
    }

    pub const fn hard_limit(&self) -> usize {
        self.hard_limit
    }

    pub const fn phase_limit(&self) -> usize {
        self.phase_limit
    }

    #[cfg(feature = "profiling")]
    pub const fn metrics(&self) -> &RuntimePathSearchMetrics {
        &self.metrics
    }

    #[cfg(feature = "profiling")]
    pub fn clear_metrics(&mut self) {
        self.metrics = RuntimePathSearchMetrics {
            actor_new_core_searches: 0,
            actor_new_deferred: 0,
            actor_reuse_core_searches: 0,
            actor_reuse_deferred: 0,
            actor_rest_fallback_core_searches: 0,
            actor_rest_fallback_deferred: 0,
            escape_core_searches: 0,
            escape_deferred: 0,
            task_execution_core_searches: 0,
            task_execution_deferred: 0,
            bucket_transport_core_searches: 0,
            bucket_transport_deferred: 0,
            expanded_nodes: 0,
            max_expanded_nodes_per_search: 0,
        };
    }

    /// Records the diagnostic expansion count after a successfully claimed
    /// core A* call. `Deferred` requests intentionally never reach this API.
    #[cfg(feature = "profiling")]
    pub fn record_expanded_nodes(&mut self, expanded_nodes: u64) {
        self.metrics.record_expanded_nodes(expanded_nodes);
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimePathSearchBudget;

    #[test]
    fn escape_task_and_idle_phases_share_the_cumulative_limit() {
        let mut budget = RuntimePathSearchBudget::new(8);

        // Escape runs first and can use its reserved two slots.
        budget.begin_phase(2);
        assert!((0..2).all(|_| budget.try_claim()));
        assert!(!budget.try_claim());
        assert_eq!(budget.used(), 2);

        // Actor task search can then raise the cumulative ceiling to six.
        budget.begin_phase(6);
        assert!((0..4).all(|_| budget.try_claim()));
        assert!(!budget.try_claim());
        assert_eq!(budget.used(), 6);

        // Idle retains the final two slots.
        budget.begin_phase(8);
        assert!(budget.try_claim());
        assert!(budget.try_claim());
        assert!(!budget.try_claim());
        assert_eq!(budget.used(), 8);
    }

    #[test]
    fn reset_restores_the_full_frame_limit() {
        let mut budget = RuntimePathSearchBudget::new(3);
        budget.begin_phase(1);
        assert!(budget.try_claim());

        budget.reset();

        assert_eq!(budget.used(), 0);
        assert_eq!(budget.phase_limit(), 3);
        assert!((0..3).all(|_| budget.try_claim()));
        assert!(!budget.try_claim());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn caller_observations_match_claimed_core_searches() {
        use super::PathSearchCaller;

        let mut budget = RuntimePathSearchBudget::new(2);
        assert!(budget.try_claim_for(PathSearchCaller::ActorNew));
        assert!(budget.try_claim_for(PathSearchCaller::TaskExecution));
        assert!(!budget.try_claim_for(PathSearchCaller::Escape));

        assert_eq!(budget.used(), 2);
        let metrics = budget.metrics();
        assert_eq!(metrics.total_core_searches(), budget.used() as u64);
        assert_eq!(metrics.actor_new_core_searches, 1);
        assert_eq!(metrics.task_execution_core_searches, 1);
        assert_eq!(metrics.escape_deferred, 1);
    }
}
