use crate::systems::logistics::transport_request::metrics::TransportRequestMetrics;

pub(super) fn update_metrics(
    metrics: &mut TransportRequestMetrics,
    active_leases: u32,
    leases_granted: u32,
    eligible_requests: u32,
    bucket_items_total: u32,
    candidates_after_top_k: u32,
    items_deduped: u32,
    candidates_dropped_by_dedup: u32,
    pending_secs_total: f64,
    lease_duration_total_secs: f64,
    arbitration_started_at: std::time::Instant,
) {
    metrics.wheelbarrow_leases_active = active_leases;
    metrics.wheelbarrow_leases_granted_this_frame = leases_granted;
    metrics.wheelbarrow_arb_eligible_requests = eligible_requests;
    metrics.wheelbarrow_arb_bucket_items_total = bucket_items_total;
    metrics.wheelbarrow_arb_candidates_after_topk = candidates_after_top_k;
    metrics.wheelbarrow_arb_items_deduped = items_deduped;
    metrics.wheelbarrow_arb_candidates_dropped_by_dedup = candidates_dropped_by_dedup;
    metrics.wheelbarrow_arb_avg_pending_secs = if eligible_requests > 0 {
        (pending_secs_total / eligible_requests as f64) as f32
    } else {
        0.0
    };
    metrics.wheelbarrow_arb_avg_lease_duration = if leases_granted > 0 {
        (lease_duration_total_secs / leases_granted as f64) as f32
    } else {
        0.0
    };
    metrics.wheelbarrow_arb_elapsed_ms = arbitration_started_at.elapsed().as_secs_f32() * 1000.0;
}
