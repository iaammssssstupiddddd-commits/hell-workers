use std::collections::HashMap;
use std::time::Instant;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::ecs::{drain_removed, drain_removed_where};
use hw_core::relationships::{IncomingDeliveries, ParkedAt, PushedBy, StoredIn, StoredItems};
use hw_jobs::Designation;

use crate::transport_request::metrics::TransportRequestMetrics;
use crate::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportRequest,
    TransportRequestState, WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::types::{BelongsTo, ResourceItem, Wheelbarrow};

use super::collection::{CollectCandidatesContext, CollectCandidatesQueries, collect_candidates};
use super::grants::{GrantLeaseQueries, GrantStats, grant_leases};
use super::lease_state::update_lease_state;
use super::metrics_update::{MetricsUpdateSpec, update_metrics};
use super::{
    WheelbarrowArbitrationDiagnostics, WheelbarrowArbitrationDirtyParams,
    WheelbarrowArbitrationHeader, WheelbarrowArbitrationOutcome, WheelbarrowArbitrationRuntime,
};

fn removed_affects_resource_items<T: Component>(
    removed: &mut RemovedComponents<T>,
    q_resource_entities: &Query<(), With<ResourceItem>>,
) -> bool {
    drain_removed_where(removed, |entity| q_resource_entities.get(entity).is_ok())
}

fn removed_affects_wheelbarrows<T: Component>(
    removed: &mut RemovedComponents<T>,
    q_wheelbarrow_entities: &Query<(), With<Wheelbarrow>>,
) -> bool {
    drain_removed_where(removed, |entity| q_wheelbarrow_entities.get(entity).is_ok())
}

type ArbitrationRequestQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TransportRequest,
        &'static TransportRequestState,
        &'static TransportDemand,
        &'static Transform,
        Option<&'static WheelbarrowLease>,
        Option<&'static WheelbarrowPendingSince>,
        Option<&'static ManualTransportRequest>,
    ),
>;

type ArbitrationWheelbarrowQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform),
    (With<Wheelbarrow>, With<ParkedAt>, Without<PushedBy>),
>;

type ArbitrationFreeItemQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Visibility,
        &'static ResourceItem,
    ),
    (
        Without<Designation>,
        Without<hw_core::relationships::TaskWorkers>,
        Without<ManualHaulPinnedSource>,
    ),
>;

/// `wheelbarrow_arbitration_system` の ECS クエリ・リソースをまとめた SystemParam。
#[derive(SystemParam)]
pub struct WheelbarrowArbitrationParams<'w, 's> {
    pub q_requests: ArbitrationRequestQuery<'w, 's>,
    pub q_wheelbarrows: ArbitrationWheelbarrowQuery<'w, 's>,
    pub q_free_items: ArbitrationFreeItemQuery<'w, 's>,
    pub q_belongs: Query<'w, 's, &'static BelongsTo>,
    pub q_stored_in: Query<'w, 's, &'static StoredIn>,
    pub q_stockpiles: Query<
        'w,
        's,
        (
            &'static crate::zone::Stockpile,
            Option<&'static StoredItems>,
        ),
    >,
    pub q_blueprints: Query<'w, 's, &'static hw_jobs::Blueprint>,
    pub q_transforms: Query<'w, 's, &'static Transform>,
    pub q_incoming: Query<'w, 's, &'static IncomingDeliveries>,
}

/// Arbitration の時刻・キャッシュ・公開状態を一貫した単位で借用する。
#[derive(SystemParam)]
pub struct WheelbarrowArbitrationResources<'w> {
    time: Res<'w, Time>,
    runtime: ResMut<'w, WheelbarrowArbitrationRuntime>,
    metrics: ResMut<'w, TransportRequestMetrics>,
    cache: Res<'w, crate::resource_cache::SharedResourceCache>,
    diagnostics: ResMut<'w, WheelbarrowArbitrationDiagnostics>,
}

pub fn wheelbarrow_arbitration_system(
    mut commands: Commands,
    p: WheelbarrowArbitrationParams,
    mut dirty: WheelbarrowArbitrationDirtyParams,
    mut resources: WheelbarrowArbitrationResources,
) {
    let arbitration_started_at = Instant::now();
    let now = resources.time.elapsed_secs_f64();

    let lease_state = update_lease_state(
        &mut commands,
        &p.q_requests,
        &p.q_free_items,
        &p.q_wheelbarrows,
        now,
    );

    // query 側が dirty でも、全 lifecycle reader をこのフレームで消費する。
    // `read()` を `||` に埋め込むと後続の removal を次フレームへ残してしまう。
    let removed_requests = drain_removed(&mut dirty.removed_requests);
    let removed_leases = drain_removed(&mut dirty.removed_leases);
    let removed_resource_items = drain_removed(&mut dirty.removed_resource_items);
    let removed_pinned_source = removed_affects_resource_items(
        &mut dirty.removed_pinned_source,
        &dirty.q_resource_entities,
    );
    let removed_belongs =
        removed_affects_resource_items(&mut dirty.removed_belongs, &dirty.q_resource_entities);
    let removed_stored_in =
        removed_affects_resource_items(&mut dirty.removed_stored_in, &dirty.q_resource_entities);
    let removed_designations =
        removed_affects_resource_items(&mut dirty.removed_designations, &dirty.q_resource_entities);
    let removed_wheelbarrows = drain_removed(&mut dirty.removed_wheelbarrows);
    let removed_parked_at =
        removed_affects_wheelbarrows(&mut dirty.removed_parked_at, &dirty.q_wheelbarrow_entities);
    let removed_pushed_by =
        removed_affects_wheelbarrows(&mut dirty.removed_pushed_by, &dirty.q_wheelbarrow_entities);
    let removed_stored_items = drain_removed(&mut dirty.removed_stored_items);
    let removed_incoming = drain_removed(&mut dirty.removed_incoming);

    let request_dirty = !dirty.q_request_dirty.is_empty() || removed_requests || removed_leases;
    let free_item_dirty = !dirty.q_free_item_dirty.is_empty()
        || removed_resource_items
        || removed_pinned_source
        || removed_belongs
        || removed_stored_in
        || removed_designations;
    let wheelbarrow_dirty = !dirty.q_wheelbarrow_dirty.is_empty()
        || removed_wheelbarrows
        || removed_parked_at
        || removed_pushed_by;
    let stockpile_dirty =
        !dirty.q_stockpile_dirty.is_empty() || removed_stored_items || removed_incoming;
    let interval_due = !resources.runtime.initialized
        || (now - resources.runtime.last_full_eval_secs)
            >= WHEELBARROW_ARBITRATION_FALLBACK_INTERVAL_SECS;
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
        resources.runtime.initialized = true;
        resources.runtime.last_full_eval_secs = now;

        let mut available_wheelbarrows: Vec<(Entity, Vec2)> = p
            .q_wheelbarrows
            .iter()
            .filter(|(e, _)| !lease_state.used_wheelbarrows.contains(e))
            .map(|(e, t)| (e, t.translation.truncate()))
            .collect();
        let header = WheelbarrowArbitrationHeader {
            generation: resources.diagnostics.next_generation(),
            availability_generation: resources.cache.semantic_generation(),
            any_vehicle_exists: !dirty.q_wheelbarrow_entities.is_empty(),
            available_vehicle_count: available_wheelbarrows.len() as u32,
            leased_vehicle_count: lease_state.used_wheelbarrows.len() as u32,
        };
        let mut outcomes = HashMap::<Entity, WheelbarrowArbitrationOutcome>::new();

        let (candidates, eligible, bucket_total, after_top_k, pending_total) = collect_candidates(
            &p.q_requests,
            &p.q_free_items,
            CollectCandidatesQueries {
                q_belongs: &p.q_belongs,
                q_stored_in: &p.q_stored_in,
                q_stockpiles: &p.q_stockpiles,
                q_blueprints: &p.q_blueprints,
                q_incoming: &p.q_incoming,
            },
            CollectCandidatesContext {
                available_wheelbarrows: &available_wheelbarrows,
                stale_cleared_requests: &lease_state.cleared_requests,
                cache: &resources.cache,
                now,
                outcomes: &mut outcomes,
            },
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
            GrantLeaseQueries {
                q_stockpiles: &p.q_stockpiles,
                q_incoming: &p.q_incoming,
                q_transforms: &p.q_transforms,
            },
            &mut outcomes,
        );
        resources.diagnostics.publish(header, outcomes);
    }

    update_metrics(
        &mut resources.metrics,
        MetricsUpdateSpec {
            active_leases: lease_state.used_wheelbarrows.len() as u32 + grant_stats.leases_granted,
            leases_granted: grant_stats.leases_granted,
            eligible_requests,
            bucket_items_total,
            candidates_after_top_k,
            items_deduped: grant_stats.items_deduped,
            candidates_dropped_by_dedup: grant_stats.candidates_dropped_by_dedup,
            pending_secs_total,
            lease_duration_total_secs: grant_stats.lease_duration_total_secs,
            arbitration_started_at,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ResourceType;
    use bevy::app::ScheduleRunnerPlugin;

    #[derive(Component)]
    struct DirtyMarker;

    #[derive(Resource, Default)]
    struct RemovalReport {
        affects_resource_items: bool,
        unread_after_check: usize,
    }

    fn consume_resource_item_removals(
        mut removed: RemovedComponents<DirtyMarker>,
        q_resource_entities: Query<(), With<ResourceItem>>,
        mut report: ResMut<RemovalReport>,
    ) {
        report.affects_resource_items |=
            removed_affects_resource_items(&mut removed, &q_resource_entities);
        report.unread_after_check += removed.read().count();
    }

    #[test]
    fn resource_item_removal_predicate_consumes_nonmatching_entries() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
        app.init_resource::<RemovalReport>();
        app.add_systems(Update, consume_resource_item_removals);
        app.update();

        let relevant = app
            .world_mut()
            .spawn((DirtyMarker, ResourceItem(ResourceType::Wood)))
            .id();
        let irrelevant = app.world_mut().spawn(DirtyMarker).id();
        app.world_mut().entity_mut(relevant).remove::<DirtyMarker>();
        app.world_mut()
            .entity_mut(irrelevant)
            .remove::<DirtyMarker>();
        app.update();

        let report = app.world().resource::<RemovalReport>();
        assert!(report.affects_resource_items);
        assert_eq!(report.unread_after_check, 0);
    }
}
