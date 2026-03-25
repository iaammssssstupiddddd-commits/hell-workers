use crate::transport_request::metrics::TransportRequestMetrics;

pub(super) struct MetricsUpdateSpec {
    pub active_leases: u32,
    pub leases_granted: u32,
    pub eligible_requests: u32,
    pub bucket_items_total: u32,
    pub candidates_after_top_k: u32,
    pub items_deduped: u32,
    pub candidates_dropped_by_dedup: u32,
    pub pending_secs_total: f64,
    pub lease_duration_total_secs: f64,
    pub arbitration_started_at: std::time::Instant,
}

pub(super) fn update_metrics(metrics: &mut TransportRequestMetrics, spec: MetricsUpdateSpec) {
    metrics.wheelbarrow_leases_active = spec.active_leases;
    metrics.wheelbarrow_leases_granted_this_frame = spec.leases_granted;
    metrics.wheelbarrow_arb_eligible_requests = spec.eligible_requests;
    metrics.wheelbarrow_arb_bucket_items_total = spec.bucket_items_total;
    metrics.wheelbarrow_arb_candidates_after_topk = spec.candidates_after_top_k;
    metrics.wheelbarrow_arb_items_deduped = spec.items_deduped;
    metrics.wheelbarrow_arb_candidates_dropped_by_dedup = spec.candidates_dropped_by_dedup;
    metrics.wheelbarrow_arb_avg_pending_secs = if spec.eligible_requests > 0 {
        (spec.pending_secs_total / spec.eligible_requests as f64) as f32
    } else {
        0.0
    };
    metrics.wheelbarrow_arb_avg_lease_duration = if spec.leases_granted > 0 {
        (spec.lease_duration_total_secs / spec.leases_granted as f64) as f32
    } else {
        0.0
    };
    metrics.wheelbarrow_arb_elapsed_ms =
        spec.arbitration_started_at.elapsed().as_secs_f32() * 1000.0;
}
