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
    ManualHaulPinnedSource, ManualTransportRequest, ReceiverPolicyTier, TransportDemand,
    TransportRequest, TransportRequestState, WheelbarrowLease, WheelbarrowPendingSince,
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
        Option<&'static Designation>,
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
            Option<&'static crate::zone::StockpilePolicy>,
            Option<&'static StoredItems>,
        ),
    >,
    pub q_blueprints: Query<'w, 's, &'static hw_jobs::Blueprint>,
    pub q_transforms: Query<'w, 's, &'static Transform>,
    pub q_incoming: Query<'w, 's, &'static IncomingDeliveries>,
    pub q_resource_items: Query<'w, 's, &'static ResourceItem>,
    pub q_receiver_policy_tiers: Query<'w, 's, &'static ReceiverPolicyTier>,
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
    let removed_stockpile_policy = drain_removed(&mut dirty.removed_stockpile_policy);
    let removed_receiver_policy_tier = drain_removed(&mut dirty.removed_receiver_policy_tier);

    let request_dirty = !dirty.q_request_dirty.is_empty()
        || removed_requests
        || removed_leases
        || removed_receiver_policy_tier;
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
    let stockpile_dirty = !dirty.q_stockpile_dirty.is_empty()
        || removed_stored_items
        || removed_incoming
        || removed_stockpile_policy;
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
                q_resource_items: &p.q_resource_items,
                q_receiver_policy_tiers: &p.q_receiver_policy_tiers,
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
                q_resource_items: &p.q_resource_items,
                q_belongs: &p.q_belongs,
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
    use crate::SharedResourceCache;
    use crate::transport_request::{
        TransportPolicy, TransportPriority, TransportRequestKind, WheelbarrowLease,
    };
    use crate::types::{BelongsTo, ResourceType, Wheelbarrow};
    use crate::zone::{Stockpile, StockpileAcceptance, StockpilePolicy};
    use bevy::app::ScheduleRunnerPlugin;
    use hw_core::relationships::ParkedAt;
    use hw_jobs::{Designation, TaskSlots, WorkType};

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

    fn arbitration_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
        app.init_resource::<SharedResourceCache>()
            .init_resource::<TransportRequestMetrics>()
            .init_resource::<WheelbarrowArbitrationRuntime>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .add_systems(Update, wheelbarrow_arbitration_system);
        app
    }

    fn spawn_managed_stockpile(app: &mut App, owner: Entity) -> Entity {
        app.world_mut()
            .spawn((
                Transform::default(),
                Stockpile {
                    capacity: 3,
                    resource_type: None,
                },
                StockpilePolicy {
                    acceptance: StockpileAcceptance::Any,
                    inbound_priority: TransportPriority::Normal,
                    target_amount: 3,
                    allow_export: true,
                },
                BelongsTo(owner),
            ))
            .id()
    }

    fn set_stockpile_capacity(app: &mut App, stockpile: Entity, capacity: usize) {
        let mut stockpile_entity = app.world_mut().entity_mut(stockpile);
        stockpile_entity
            .get_mut::<Stockpile>()
            .expect("stockpile")
            .capacity = capacity;
        stockpile_entity
            .get_mut::<StockpilePolicy>()
            .expect("stockpile policy")
            .target_amount = capacity;
    }

    fn spawn_deposit_request(
        app: &mut App,
        anchor: Entity,
        resource_type: ResourceType,
        desired_slots: u32,
        enabled: bool,
    ) -> Entity {
        let mut entity = app.world_mut().spawn((
            Transform::default(),
            TransportRequest {
                kind: TransportRequestKind::DepositToStockpile,
                anchor,
                resource_type,
                issued_by: Entity::PLACEHOLDER,
                priority: TransportPriority::Normal,
                stockpile_group: vec![anchor],
            },
            ReceiverPolicyTier(TransportPriority::Normal),
            TransportDemand {
                desired_slots,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
        if enabled {
            entity.insert((
                Designation {
                    work_type: WorkType::Haul,
                },
                TaskSlots::new(desired_slots),
            ));
        }
        entity.id()
    }

    fn spawn_unowned_items(app: &mut App, resource_type: ResourceType) -> Vec<Entity> {
        (0..3)
            .map(|offset| {
                app.world_mut()
                    .spawn((
                        Transform::from_xyz(offset as f32, 0.0, 0.0),
                        Visibility::Visible,
                        ResourceItem(resource_type),
                    ))
                    .id()
            })
            .collect()
    }

    fn spawn_parked_wheelbarrow(app: &mut App) -> Entity {
        let parking = app.world_mut().spawn_empty().id();
        app.world_mut()
            .spawn((
                Transform::default(),
                Wheelbarrow { capacity: 10 },
                ParkedAt(parking),
            ))
            .id()
    }

    #[test]
    fn owned_stockpile_request_grants_a_lease_from_unowned_ground_items() {
        let mut app = arbitration_test_app();
        let owner = app.world_mut().spawn_empty().id();
        let stockpile = spawn_managed_stockpile(&mut app, owner);
        let request = spawn_deposit_request(&mut app, stockpile, ResourceType::Wood, 3, true);
        let items = spawn_unowned_items(&mut app, ResourceType::Wood);
        spawn_parked_wheelbarrow(&mut app);

        app.update();

        let lease = app
            .world()
            .get::<WheelbarrowLease>(request)
            .expect("unowned items must be eligible for an owned ordinary stockpile");
        assert_eq!(lease.items.len(), 3);
        assert!(lease.items.iter().all(|item| items.contains(item)));
        assert_eq!(
            app.world()
                .resource::<WheelbarrowArbitrationDiagnostics>()
                .outcome(request),
            Some(WheelbarrowArbitrationOutcome::LeaseGranted)
        );
    }

    #[test]
    fn grant_time_shadow_prevents_two_requests_from_overbooking_one_physical_cell() {
        let mut app = arbitration_test_app();
        let owner = app.world_mut().spawn_empty().id();
        let stockpile = spawn_managed_stockpile(&mut app, owner);
        let wood_request = spawn_deposit_request(&mut app, stockpile, ResourceType::Wood, 3, true);
        let rock_request = spawn_deposit_request(&mut app, stockpile, ResourceType::Rock, 3, true);
        spawn_unowned_items(&mut app, ResourceType::Wood);
        spawn_unowned_items(&mut app, ResourceType::Rock);
        spawn_parked_wheelbarrow(&mut app);
        spawn_parked_wheelbarrow(&mut app);

        app.update();

        let leases: Vec<_> = [wood_request, rock_request]
            .into_iter()
            .filter_map(|request| app.world().get::<WheelbarrowLease>(request))
            .collect();
        assert_eq!(leases.len(), 1);
        assert_eq!(leases[0].items.len(), 3);
        assert_eq!(
            leases[0].destination,
            crate::transport_request::WheelbarrowDestination::Stockpile(stockpile)
        );

        let diagnostics = app.world().resource::<WheelbarrowArbitrationDiagnostics>();
        let outcomes = [
            diagnostics.outcome(wood_request),
            diagnostics.outcome(rock_request),
        ];
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == Some(WheelbarrowArbitrationOutcome::LeaseGranted))
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| {
                    **outcome == Some(WheelbarrowArbitrationOutcome::CapacityReserved)
                })
                .count(),
            1
        );
    }

    #[test]
    fn grant_keeps_owned_items_with_a_compatible_cell_in_a_mixed_owner_group() {
        let mut app = arbitration_test_app();
        let owner_a = app.world_mut().spawn_empty().id();
        let owner_b = app.world_mut().spawn_empty().id();
        let stockpile_a = spawn_managed_stockpile(&mut app, owner_a);
        let stockpile_b = spawn_managed_stockpile(&mut app, owner_b);
        set_stockpile_capacity(&mut app, stockpile_a, 4);
        let request = spawn_deposit_request(&mut app, stockpile_a, ResourceType::Wood, 3, true);
        app.world_mut()
            .get_mut::<TransportRequest>(request)
            .expect("transport request")
            .stockpile_group = vec![stockpile_a, stockpile_b];
        for item in spawn_unowned_items(&mut app, ResourceType::Wood) {
            app.world_mut().entity_mut(item).insert(BelongsTo(owner_a));
        }
        spawn_parked_wheelbarrow(&mut app);

        app.update();

        let lease = app
            .world()
            .get::<WheelbarrowLease>(request)
            .expect("the compatible owner cell must receive the lease");
        assert_eq!(
            lease.destination,
            crate::transport_request::WheelbarrowDestination::Stockpile(stockpile_a),
            "the smaller best-fit cell owned by B must not receive A-owned items"
        );
    }

    #[test]
    fn pending_request_without_designation_releases_its_lease_and_reports_demand_gone() {
        let mut app = arbitration_test_app();
        let owner = app.world_mut().spawn_empty().id();
        let stockpile = spawn_managed_stockpile(&mut app, owner);
        let items = spawn_unowned_items(&mut app, ResourceType::Wood);
        let wheelbarrow = spawn_parked_wheelbarrow(&mut app);
        let request = spawn_deposit_request(&mut app, stockpile, ResourceType::Wood, 3, false);
        let live_request = spawn_deposit_request(&mut app, stockpile, ResourceType::Rock, 3, true);
        spawn_unowned_items(&mut app, ResourceType::Rock);
        app.world_mut().entity_mut(request).insert((
            WheelbarrowLease {
                wheelbarrow,
                items,
                source_pos: Vec2::ZERO,
                destination: crate::transport_request::WheelbarrowDestination::Stockpile(stockpile),
                lease_until: f64::MAX,
            },
            WheelbarrowPendingSince(0.0),
        ));

        app.update();

        let request_ref = app.world().entity(request);
        assert!(!request_ref.contains::<WheelbarrowLease>());
        assert!(!request_ref.contains::<WheelbarrowPendingSince>());
        assert_eq!(
            app.world()
                .get::<WheelbarrowLease>(live_request)
                .map(|lease| lease.wheelbarrow),
            Some(wheelbarrow),
            "the released wheelbarrow must be reusable by a live request in the same pass"
        );
        assert_eq!(
            app.world()
                .resource::<WheelbarrowArbitrationDiagnostics>()
                .outcome(request),
            Some(WheelbarrowArbitrationOutcome::DemandGone)
        );
    }

    #[test]
    fn zero_demand_pending_request_with_designation_cannot_gain_a_lease() {
        let mut app = arbitration_test_app();
        let owner = app.world_mut().spawn_empty().id();
        let stockpile = spawn_managed_stockpile(&mut app, owner);
        let request = spawn_deposit_request(&mut app, stockpile, ResourceType::Wood, 0, true);
        spawn_unowned_items(&mut app, ResourceType::Wood);
        spawn_parked_wheelbarrow(&mut app);

        app.update();

        let request_ref = app.world().entity(request);
        assert!(!request_ref.contains::<WheelbarrowLease>());
        assert!(!request_ref.contains::<WheelbarrowPendingSince>());
        assert_eq!(
            app.world()
                .resource::<WheelbarrowArbitrationDiagnostics>()
                .outcome(request),
            Some(WheelbarrowArbitrationOutcome::DemandGone)
        );
    }
}
