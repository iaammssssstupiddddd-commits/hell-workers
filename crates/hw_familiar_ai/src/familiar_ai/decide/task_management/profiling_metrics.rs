#[cfg(feature = "profiling")]
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CandidatePipelinePerfSnapshot {
    pub membership_checks: u32,
    pub policy_disabled_rejections: u32,
    pub snapshot_attempts: u32,
    pub score_attempts: u32,
    pub worker_score_attempts: u32,
}

#[cfg(feature = "profiling")]
static CANDIDATE_MEMBERSHIP_CHECKS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "profiling")]
static POLICY_DISABLED_REJECTIONS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "profiling")]
static CANDIDATE_SNAPSHOT_ATTEMPTS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "profiling")]
static CANDIDATE_SCORE_ATTEMPTS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "profiling")]
static WORKER_SCORE_ATTEMPTS: AtomicU32 = AtomicU32::new(0);

#[inline]
pub(super) fn mark_candidate_membership_check() {
    #[cfg(feature = "profiling")]
    CANDIDATE_MEMBERSHIP_CHECKS.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub(super) fn mark_policy_disabled_rejection() {
    #[cfg(feature = "profiling")]
    POLICY_DISABLED_REJECTIONS.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub(super) fn mark_candidate_snapshot_attempt() {
    #[cfg(feature = "profiling")]
    CANDIDATE_SNAPSHOT_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub(super) fn mark_candidate_score_attempt() {
    #[cfg(feature = "profiling")]
    CANDIDATE_SCORE_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
}

#[inline]
pub(super) fn mark_worker_score_attempt() {
    #[cfg(feature = "profiling")]
    WORKER_SCORE_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
}

#[cfg(feature = "profiling")]
pub fn take_candidate_pipeline_perf_snapshot() -> CandidatePipelinePerfSnapshot {
    CandidatePipelinePerfSnapshot {
        membership_checks: CANDIDATE_MEMBERSHIP_CHECKS.swap(0, Ordering::Relaxed),
        policy_disabled_rejections: POLICY_DISABLED_REJECTIONS.swap(0, Ordering::Relaxed),
        snapshot_attempts: CANDIDATE_SNAPSHOT_ATTEMPTS.swap(0, Ordering::Relaxed),
        score_attempts: CANDIDATE_SCORE_ATTEMPTS.swap(0, Ordering::Relaxed),
        worker_score_attempts: WORKER_SCORE_ATTEMPTS.swap(0, Ordering::Relaxed),
    }
}

#[cfg(not(feature = "profiling"))]
pub fn take_candidate_pipeline_perf_snapshot() -> CandidatePipelinePerfSnapshot {
    CandidatePipelinePerfSnapshot::default()
}
