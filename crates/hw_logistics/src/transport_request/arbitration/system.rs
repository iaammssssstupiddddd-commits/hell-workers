use std::time::Instant;

use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::relationships::{IncomingDeliveries, ParkedAt, PushedBy, StoredIn, StoredItems};
use hw_jobs::Designation;

use crate::transport_request::metrics::TransportRequestMetrics;
use crate::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportRequest,
    TransportRequestState, WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::types::{BelongsTo, ReservedForTask, ResourceItem, Wheelbarrow};
use crate::zone::Stockpile;

use super::collection::collect_candidates;
use super::grants::{GrantStats, grant_leases};
use super::lease_state::update_lease_state;
use super::metrics_update::update_metrics;
use super::{WheelbarrowArbitrationDirtyParams, WheelbarrowArbitrationRuntime};

fn removed_affects_resource_items<T: Component>(
    removed: &mut RemovedComponents<T>,
    q_resource_entities: &Query<(), With<ResourceItem>>,
) -> bool {
    removed
        .read()
        .any(|entity| q_resource_entities.get(entity).is_ok())
}

fn removed_affects_wheelbarrows<T: Component>(
    removed: &mut RemovedComponents<T>,
    q_wheelbarrow_entities: &Query<(), With<Wheelbarrow>>,
) -> bool {
    removed
        .read()
        .any(|entity| q_wheelbarrow_entities.get(entity).is_ok())
}

pub fn wheelbarrow_arbitration_system(
    mut commands: Commands,
    time: Res<Time>,
    mut runtime: Local<WheelbarrowArbitrationRuntime>,
    q_requests: Query<(
        Entity,
        &TransportRequest,
        &TransportRequestState,
        &TransportDemand,
        &Transform,
        Option<&WheelbarrowLease>,
        Option<&WheelbarrowPendingSince>,
        Option<&ManualTransportRequest>,
    )>,
    q_wheelbarrows: Query<
        (Entity, &Transform),
        (With<Wheelbarrow>, With<ParkedAt>, Without<PushedBy>),
    >,
    q_free_items: Query<
        (Entity, &Transform, &Visibility, &ResourceItem),
        (
            Without<Designation>,
            Without<hw_core::relationships::TaskWorkers>,
            Without<ReservedForTask>,
            Without<ManualHaulPinnedSource>,
        ),
    >,
    q_belongs: Query<&BelongsTo>,
    q_stored_in: Query<&StoredIn>,
    q_stockpiles: Query<(&Stockpile, Option<&StoredItems>)>,
    q_blueprints: Query<&hw_jobs::Blueprint>,
    q_transforms: Query<&Transform>,
    mut dirty: WheelbarrowArbitrationDirtyParams,
    mut metrics: ResMut<TransportRequestMetrics>,
    cache: Res<crate::resource_cache::SharedResourceCache>,
    q_incoming: Query<&IncomingDeliveries>,
) {
    let arbitration_started_at = Instant::now();
    let now = time.elapsed_secs_f64();

    let lease_state = update_lease_state(
        &mut commands,
        &q_requests,
        &q_free_items,
        &q_wheelbarrows,
        now,
    );

    let request_dirty = dirty.q_request_dirty.iter().next().is_some()
        || dirty.removed_requests.read().next().is_some()
        || dirty.removed_leases.read().next().is_some();
    let free_item_dirty = dirty.q_free_item_dirty.iter().next().is_some()
        || dirty.removed_resource_items.read().next().is_some()
        || removed_affects_resource_items(
            &mut dirty.removed_reserved_for_task,
            &dirty.q_resource_entities,
        )
        || removed_affects_resource_items(
            &mut dirty.removed_pinned_source,
            &dirty.q_resource_entities,
        )
        || removed_affects_resource_items(&mut dirty.removed_belongs, &dirty.q_resource_entities)
        || removed_affects_resource_items(&mut dirty.removed_stored_in, &dirty.q_resource_entities)
        || removed_affects_resource_items(
            &mut dirty.removed_designations,
            &dirty.q_resource_entities,
        );
    let wheelbarrow_dirty = dirty.q_wheelbarrow_dirty.iter().next().is_some()
        || dirty.removed_wheelbarrows.read().next().is_some()
        || removed_affects_wheelbarrows(
            &mut dirty.removed_parked_at,
            &dirty.q_wheelbarrow_entities,
        )
        || removed_affects_wheelbarrows(
            &mut dirty.removed_pushed_by,
            &dirty.q_wheelbarrow_entities,
        );
    let stockpile_dirty = dirty.q_stockpile_dirty.iter().next().is_some()
        || dirty.removed_stored_items.read().next().is_some()
        || dirty.removed_incoming.read().next().is_some();
    let interval_due = !runtime.initialized
        || (now - runtime.last_full_eval_secs) >= WHEELBARROW_ARBITRATION_FALLBACK_INTERVAL_SECS;
    let stale_lease_removed = !lease_state.cleared_requests.is_empty();
    let should_rebuild = request_dirty
        || free_item_dirty
        || wheelbarrow_dirty
        || stockpile_dirty
        || interval_due
        || stale_lease_removed;

    let mut grant_stats = GrantStats::default();
    let mut eligible_requests = 0u32;
    let mut bucket_items_total = 0u32;
    let mut candidates_after_top_k = 0u32;
    let mut pending_secs_total = 0.0f64;

    if should_rebuild {
        runtime.initialized = true;
        runtime.last_full_eval_secs = now;

        let mut available_wheelbarrows: Vec<(Entity, Vec2)> = q_wheelbarrows
            .iter()
            .filter(|(e, _)| !lease_state.used_wheelbarrows.contains(e))
            .map(|(e, t)| (e, t.translation.truncate()))
            .collect();

        let (candidates, eligible, bucket_total, after_top_k, pending_total) = collect_candidates(
            &q_requests,
            &q_free_items,
            &q_belongs,
            &q_stored_in,
            &q_stockpiles,
            &q_blueprints,
            &available_wheelbarrows,
            &lease_state.cleared_requests,
            &cache,
            &q_incoming,
            now,
        );

        eligible_requests = eligible;
        bucket_items_total = bucket_total;
        candidates_after_top_k = after_top_k;
        pending_secs_total = pending_total;
        grant_stats = grant_leases(
            &candidates,
            &mut available_wheelbarrows,
            now,
            &mut commands,
            &q_stockpiles,
            &q_incoming,
            &q_transforms,
        );
    }

    update_metrics(
        &mut metrics,
        lease_state.used_wheelbarrows.len() as u32 + grant_stats.leases_granted,
        grant_stats.leases_granted,
        eligible_requests,
        bucket_items_total,
        candidates_after_top_k,
        grant_stats.items_deduped,
        grant_stats.candidates_dropped_by_dedup,
        pending_secs_total,
        grant_stats.lease_duration_total_secs,
        arbitration_started_at,
    );
}
