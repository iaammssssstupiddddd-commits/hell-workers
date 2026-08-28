//! TransportRequest のメトリクス集計

#[cfg(feature = "profiling")]
use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
#[cfg(feature = "profiling")]
use hw_core::relationships::ManagedBy;
#[cfg(feature = "profiling")]
use hw_jobs::construction::{TargetFloorConstructionSite, TargetWallConstructionSite};
#[cfg(feature = "profiling")]
use hw_jobs::{
    Designation, Priority, TargetBlueprint, TargetMixer, TargetSoulSpaSite, TaskSlots, WorkType,
};

#[cfg(feature = "profiling")]
use crate::transport_request::{
    ManualTransportRequest, ReceiverPolicyTier, TransportDemand, TransportPolicy,
};
#[cfg(feature = "profiling")]
use crate::transport_request::{TransportRequest, TransportRequestKind};

/// Wheelbarrow arbitration の最新フレーム計測値。
#[derive(Resource, Default, Debug)]
pub struct WheelbarrowArbitrationMetrics {
    pub wheelbarrow_leases_active: u32,
    pub wheelbarrow_leases_granted_this_frame: u32,
    pub wheelbarrow_arb_eligible_requests: u32,
    pub wheelbarrow_arb_bucket_items_total: u32,
    pub wheelbarrow_arb_candidates_after_topk: u32,
    pub wheelbarrow_arb_items_deduped: u32,
    pub wheelbarrow_arb_candidates_dropped_by_dedup: u32,
    pub wheelbarrow_arb_avg_pending_secs: f32,
    pub wheelbarrow_arb_avg_lease_duration: f32,
    pub wheelbarrow_arb_elapsed_ms: f32,
}

/// Task-area producer の最新フレーム計測値。
#[derive(Resource, Default, Debug)]
pub struct TaskAreaMetrics {
    pub task_area_groups: u32,
    pub task_area_free_items_scanned: u32,
    pub task_area_items_matched: u32,
    pub task_area_elapsed_ms: f32,
}

/// Floor material sync の最新フレーム計測値。
#[derive(Resource, Default, Debug)]
pub struct FloorMaterialSyncMetrics {
    pub floor_material_sync_sites_processed: u32,
    pub floor_material_sync_resources_scanned: u32,
    pub floor_material_sync_tiles_scanned: u32,
    pub floor_material_sync_elapsed_ms: f32,
}

/// Wall material sync の最新フレーム計測値。
#[derive(Resource, Default, Debug)]
pub struct WallMaterialSyncMetrics {
    pub wall_material_sync_sites_processed: u32,
    pub wall_material_sync_resources_scanned: u32,
    pub wall_material_sync_tiles_scanned: u32,
    pub wall_material_sync_elapsed_ms: f32,
}

/// Wheelbarrow arbitration の期間累積 work counter。
///
/// `WheelbarrowArbitrationMetrics` は最新フレームの gauge であり、
/// fixed-step checkpoint や capture window の比較には使えない。この resource は
/// profiling build だけで登録し、計測境界でまとめて reset する。
#[cfg(feature = "profiling")]
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelbarrowArbitrationPerfMetrics {
    pub rebuilds: u32,
    pub request_bucket_builds: u32,
    pub bucket_items_scanned: u32,
    pub candidates_after_top_k: u32,
}

/// `TransportRequest` component の change tick を種別ごとに集計する。
///
/// producer が queue した command 数ではなく、`ApplyDeferred` 後に実際に観測された
/// Added / changed-existing を数える。M0 baseline と M1 semantic upsert candidate は
/// 同じ観測境界を使う。
#[cfg(feature = "profiling")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TransportRequestKindChangePerfMetrics {
    pub added: u64,
    pub changed_existing: u64,
}

/// Producer phase後に観測されたrequestのsemantic transition分類。
///
/// `no_op_writes`は少なくとも1つのproducer-owned componentにchange tickが立ったものの、
/// 観測値が前frameと同じだった回数。`steady_observations`は対象componentにchange tickが
/// 立たなかった回数であり、command発行数には含めない。
#[cfg(feature = "profiling")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TransportRequestKindProducerPerfMetrics {
    pub spawns: u64,
    pub missing_repairs: u64,
    pub semantic_updates: u64,
    pub disable_updates: u64,
    pub no_op_writes: u64,
    pub steady_observations: u64,
}

#[cfg(feature = "profiling")]
impl TransportRequestKindProducerPerfMetrics {
    pub const fn observations(self) -> u64 {
        self.spawns
            .saturating_add(self.missing_repairs)
            .saturating_add(self.semantic_updates)
            .saturating_add(self.disable_updates)
            .saturating_add(self.no_op_writes)
            .saturating_add(self.steady_observations)
    }
}

#[cfg(feature = "profiling")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TransportRequestProducerSnapshot {
    kind: TransportRequestKind,
    anchor: Entity,
    resource_type: crate::types::ResourceType,
    issued_by: Entity,
    request_priority: crate::transport_request::TransportPriority,
    stockpile_group: Vec<Entity>,
    transform: Option<([u32; 3], [u32; 4], [u32; 3])>,
    visibility: Option<Visibility>,
    work_type: Option<WorkType>,
    managed_by: Option<Entity>,
    task_slots: Option<u32>,
    priority: Option<u32>,
    receiver_tier: Option<crate::transport_request::TransportPriority>,
    demand: Option<(u32, u32)>,
    policy: Option<(bool, bool, u32)>,
    target_blueprint: Option<Entity>,
    target_floor: Option<Entity>,
    target_wall: Option<Entity>,
    target_mixer: Option<Entity>,
    target_soul_spa: Option<Entity>,
}

#[cfg(feature = "profiling")]
impl TransportRequestProducerSnapshot {
    fn fully_disabled(&self) -> bool {
        self.work_type.is_none() && self.task_slots.is_none() && self.priority.is_none()
    }

    fn required_missing_fields(&self, active: bool) -> u32 {
        let mut missing = u32::from(self.transform.is_none())
            + u32::from(self.visibility.is_none())
            + u32::from(self.managed_by.is_none())
            + u32::from(self.demand.is_none())
            + u32::from(self.policy.is_none());
        if active {
            missing += u32::from(self.work_type.is_none())
                + u32::from(self.task_slots.is_none())
                + u32::from(self.priority.is_none());
        }
        missing
            + match self.kind {
                TransportRequestKind::DeliverToBlueprint => {
                    u32::from(self.target_blueprint.is_none())
                }
                TransportRequestKind::DeliverToFloorConstruction => {
                    u32::from(self.target_floor.is_none())
                }
                TransportRequestKind::DeliverToWallConstruction => {
                    u32::from(self.target_wall.is_none())
                }
                TransportRequestKind::DeliverToMixerSolid
                | TransportRequestKind::DeliverWaterToMixer => {
                    u32::from(self.target_mixer.is_none())
                }
                TransportRequestKind::DeliverToSoulSpa => u32::from(self.target_soul_spa.is_none()),
                _ => 0,
            }
    }
}

#[cfg(feature = "profiling")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransportRequestProducerOutcome {
    Spawn,
    MissingRepair,
    SemanticUpdate,
    DisableUpdate,
    NoOpWrite,
    Steady,
}

#[cfg(feature = "profiling")]
impl TransportRequestKindChangePerfMetrics {
    pub const fn changed(self) -> u64 {
        self.added.saturating_add(self.changed_existing)
    }
}

/// 計測区間における `TransportRequest` component change の累積値。
///
/// この resource への書き込みは `TransportRequestSet::Execute` の deferred commandを
/// flushした後の単一collectorだけが行うため、producer同士へ共有`ResMut`競合を追加しない。
#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct TransportRequestChangePerfMetrics {
    pub observer_runs: u64,
    by_kind: [TransportRequestKindChangePerfMetrics; TransportRequestKind::ALL.len()],
    producer_by_kind: [TransportRequestKindProducerPerfMetrics; TransportRequestKind::ALL.len()],
    producer_snapshots: HashMap<Entity, TransportRequestProducerSnapshot>,
}

#[cfg(feature = "profiling")]
impl TransportRequestChangePerfMetrics {
    pub fn clear(&mut self) {
        self.observer_runs = 0;
        self.by_kind = Default::default();
        self.producer_by_kind = Default::default();
    }

    pub fn for_kind(&self, kind: TransportRequestKind) -> TransportRequestKindChangePerfMetrics {
        self.by_kind[kind.metric_index()]
    }

    pub fn producer_for_kind(
        &self,
        kind: TransportRequestKind,
    ) -> TransportRequestKindProducerPerfMetrics {
        self.producer_by_kind[kind.metric_index()]
    }
}

#[cfg(feature = "profiling")]
type ProducerCommonRefs<'a> = (
    Option<Ref<'a, Transform>>,
    Option<Ref<'a, Visibility>>,
    Option<Ref<'a, Designation>>,
    Option<Ref<'a, ManagedBy>>,
    Option<Ref<'a, TaskSlots>>,
    Option<Ref<'a, Priority>>,
    Option<Ref<'a, ReceiverPolicyTier>>,
    Option<Ref<'a, TransportDemand>>,
    Option<Ref<'a, TransportPolicy>>,
);

#[cfg(feature = "profiling")]
type ProducerTargetRefs<'a> = (
    Option<Ref<'a, TargetBlueprint>>,
    Option<Ref<'a, TargetFloorConstructionSite>>,
    Option<Ref<'a, TargetWallConstructionSite>>,
    Option<Ref<'a, TargetMixer>>,
    Option<Ref<'a, TargetSoulSpaSite>>,
);

#[cfg(feature = "profiling")]
type ProducerChangeQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Ref<'static, TransportRequest>,
        ProducerCommonRefs<'static>,
        ProducerTargetRefs<'static>,
    ),
    Without<ManualTransportRequest>,
>;

#[cfg(feature = "profiling")]
fn transform_bits(transform: &Transform) -> ([u32; 3], [u32; 4], [u32; 3]) {
    (
        transform.translation.to_array().map(f32::to_bits),
        transform.rotation.to_array().map(f32::to_bits),
        transform.scale.to_array().map(f32::to_bits),
    )
}

#[cfg(feature = "profiling")]
fn producer_snapshot(
    request: &TransportRequest,
    common: &ProducerCommonRefs<'_>,
    targets: &ProducerTargetRefs<'_>,
) -> TransportRequestProducerSnapshot {
    let (transform, visibility, designation, managed_by, slots, priority, tier, demand, policy) =
        common;
    let (blueprint, floor, wall, mixer, soul_spa) = targets;
    TransportRequestProducerSnapshot {
        kind: request.kind,
        anchor: request.anchor,
        resource_type: request.resource_type,
        issued_by: request.issued_by,
        request_priority: request.priority,
        stockpile_group: request.stockpile_group.clone(),
        transform: transform.as_deref().map(transform_bits),
        visibility: visibility.as_deref().copied(),
        work_type: designation.as_deref().map(|value| value.work_type),
        managed_by: managed_by.as_deref().map(|value| value.0),
        task_slots: slots.as_deref().map(|value| value.max),
        priority: priority.as_deref().map(|value| value.0),
        receiver_tier: tier.as_deref().map(|value| value.0),
        demand: demand
            .as_deref()
            .map(|value| (value.desired_slots, value.inflight)),
        policy: policy.as_deref().map(|value| {
            (
                value.allow_cross_area_source,
                value.allow_cross_familiar_claim,
                value.source_search_radius_tiles.to_bits(),
            )
        }),
        target_blueprint: blueprint.as_deref().map(|value| value.0),
        target_floor: floor.as_deref().map(|value| value.0),
        target_wall: wall.as_deref().map(|value| value.0),
        target_mixer: mixer.as_deref().map(|value| value.0),
        target_soul_spa: soul_spa.as_deref().map(|value| value.0),
    }
}

#[cfg(feature = "profiling")]
fn ref_changed<T: Component>(value: &Option<Ref<'_, T>>) -> bool {
    value.as_ref().is_some_and(DetectChanges::is_changed)
}

#[cfg(feature = "profiling")]
fn producer_owned_component_changed(
    request: &Ref<'_, TransportRequest>,
    common: &ProducerCommonRefs<'_>,
    targets: &ProducerTargetRefs<'_>,
) -> bool {
    request.is_changed()
        || ref_changed(&common.0)
        || ref_changed(&common.1)
        || ref_changed(&common.2)
        || ref_changed(&common.3)
        || ref_changed(&common.4)
        || ref_changed(&common.5)
        || ref_changed(&common.6)
        || ref_changed(&common.7)
        || ref_changed(&common.8)
        || ref_changed(&targets.0)
        || ref_changed(&targets.1)
        || ref_changed(&targets.2)
        || ref_changed(&targets.3)
        || ref_changed(&targets.4)
}

#[cfg(feature = "profiling")]
fn classify_producer_outcome(
    previous: Option<&TransportRequestProducerSnapshot>,
    current: &TransportRequestProducerSnapshot,
    component_changed: bool,
) -> TransportRequestProducerOutcome {
    let Some(previous) = previous else {
        return TransportRequestProducerOutcome::Spawn;
    };
    if previous == current {
        return if component_changed {
            TransportRequestProducerOutcome::NoOpWrite
        } else {
            TransportRequestProducerOutcome::Steady
        };
    }
    if current.fully_disabled() {
        return TransportRequestProducerOutcome::DisableUpdate;
    }
    let current_active = !current.fully_disabled();
    if previous.required_missing_fields(current_active)
        > current.required_missing_fields(current_active)
    {
        return TransportRequestProducerOutcome::MissingRepair;
    }
    TransportRequestProducerOutcome::SemanticUpdate
}

#[cfg(feature = "profiling")]
fn record_producer_outcome(
    metrics: &mut TransportRequestKindProducerPerfMetrics,
    outcome: TransportRequestProducerOutcome,
) {
    let counter = match outcome {
        TransportRequestProducerOutcome::Spawn => &mut metrics.spawns,
        TransportRequestProducerOutcome::MissingRepair => &mut metrics.missing_repairs,
        TransportRequestProducerOutcome::SemanticUpdate => &mut metrics.semantic_updates,
        TransportRequestProducerOutcome::DisableUpdate => &mut metrics.disable_updates,
        TransportRequestProducerOutcome::NoOpWrite => &mut metrics.no_op_writes,
        TransportRequestProducerOutcome::Steady => &mut metrics.steady_observations,
    };
    *counter = counter.saturating_add(1);
}

/// Producerのdeferred command適用後に、component changeとsemantic transitionを一度だけ採取する。
#[cfg(feature = "profiling")]
pub fn collect_transport_request_change_metrics_system(
    q_requests: ProducerChangeQuery,
    metrics: Option<ResMut<TransportRequestChangePerfMetrics>>,
    mut observed_entities: Local<HashSet<Entity>>,
) {
    let Some(mut metrics) = metrics else {
        return;
    };
    metrics.observer_runs = metrics.observer_runs.saturating_add(1);
    observed_entities.clear();
    for (entity, request, common, targets) in q_requests.iter() {
        observed_entities.insert(entity);
        if request.kind == TransportRequestKind::DeliverToSoulSpa {
            continue;
        }
        collect_request_observation(entity, &request, &common, &targets, &mut metrics);
    }
    metrics
        .producer_snapshots
        .retain(|entity, _| observed_entities.contains(entity));
}

/// Root所有のSoul Spa producerを、そのenergy pipeline内のflush後に採取する。
///
/// 通常collectorと同じresourceへ書くがhot producerとは競合せず、Soul Spa kindは通常collector側で
/// 除外されるため二重計上しない。`observer_runs`は通常collectorの1回/frameを正本とする。
#[cfg(feature = "profiling")]
pub fn collect_soul_spa_transport_request_change_metrics_system(
    q_requests: ProducerChangeQuery,
    metrics: Option<ResMut<TransportRequestChangePerfMetrics>>,
) {
    let Some(mut metrics) = metrics else {
        return;
    };
    for (entity, request, common, targets) in q_requests.iter() {
        if request.kind != TransportRequestKind::DeliverToSoulSpa {
            continue;
        }
        collect_request_observation(entity, &request, &common, &targets, &mut metrics);
    }
}

#[cfg(feature = "profiling")]
fn collect_request_observation(
    entity: Entity,
    request: &Ref<'_, TransportRequest>,
    common: &ProducerCommonRefs<'_>,
    targets: &ProducerTargetRefs<'_>,
    metrics: &mut TransportRequestChangePerfMetrics,
) {
    if request.is_changed() {
        let kind_metrics = &mut metrics.by_kind[request.kind.metric_index()];
        if request.is_added() {
            kind_metrics.added = kind_metrics.added.saturating_add(1);
        } else {
            kind_metrics.changed_existing = kind_metrics.changed_existing.saturating_add(1);
        }
    }
    let snapshot = producer_snapshot(request, common, targets);
    let outcome = classify_producer_outcome(
        metrics.producer_snapshots.get(&entity),
        &snapshot,
        producer_owned_component_changed(request, common, targets),
    );
    record_producer_outcome(
        &mut metrics.producer_by_kind[request.kind.metric_index()],
        outcome,
    );
    metrics.producer_snapshots.insert(entity, snapshot);
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use super::*;
    use crate::types::ResourceType;

    fn request(anchor: Entity) -> TransportRequest {
        TransportRequest {
            kind: TransportRequestKind::DeliverToBlueprint,
            anchor,
            resource_type: ResourceType::Wood,
            issued_by: anchor,
            priority: Default::default(),
            stockpile_group: Vec::new(),
        }
    }

    fn active_request_bundle(anchor: Entity) -> impl Bundle {
        (
            request(anchor),
            Transform::default(),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            ManagedBy(anchor),
            TaskSlots::new(1),
            Priority(0),
            TransportDemand {
                desired_slots: 1,
                inflight: 0,
            },
            TransportPolicy::default(),
            TargetBlueprint(anchor),
        )
    }

    fn active_soul_spa_request_bundle(anchor: Entity) -> impl Bundle {
        let mut soul_spa_request = request(anchor);
        soul_spa_request.kind = TransportRequestKind::DeliverToSoulSpa;
        (
            soul_spa_request,
            Transform::default(),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            ManagedBy(anchor),
            TaskSlots::new(1),
            Priority(0),
            TransportDemand {
                desired_slots: 1,
                inflight: 0,
            },
            TransportPolicy::default(),
            TargetSoulSpaSite(anchor),
        )
    }

    #[test]
    fn change_metrics_distinguish_added_changed_existing_and_steady_frames() {
        let mut app = App::new();
        app.init_resource::<TransportRequestChangePerfMetrics>()
            .add_systems(Update, collect_transport_request_change_metrics_system);

        let anchor = app.world_mut().spawn_empty().id();
        let request_entity = app.world_mut().spawn(request(anchor)).id();
        app.update();

        let metrics = app.world().resource::<TransportRequestChangePerfMetrics>();
        assert_eq!(metrics.observer_runs, 1);
        assert_eq!(
            metrics.for_kind(TransportRequestKind::DeliverToBlueprint),
            TransportRequestKindChangePerfMetrics {
                added: 1,
                changed_existing: 0,
            }
        );
        assert_eq!(
            metrics
                .producer_for_kind(TransportRequestKind::DeliverToBlueprint)
                .spawns,
            1
        );

        app.world_mut()
            .resource_mut::<TransportRequestChangePerfMetrics>()
            .clear();
        app.world_mut()
            .entity_mut(request_entity)
            .insert(request(anchor));
        app.update();

        let metrics = app.world().resource::<TransportRequestChangePerfMetrics>();
        assert_eq!(metrics.observer_runs, 1);
        assert_eq!(
            metrics.for_kind(TransportRequestKind::DeliverToBlueprint),
            TransportRequestKindChangePerfMetrics {
                added: 0,
                changed_existing: 1,
            }
        );
        assert_eq!(
            metrics
                .producer_for_kind(TransportRequestKind::DeliverToBlueprint)
                .no_op_writes,
            1
        );

        app.world_mut()
            .resource_mut::<TransportRequestChangePerfMetrics>()
            .clear();
        app.update();

        let metrics = app.world().resource::<TransportRequestChangePerfMetrics>();
        assert_eq!(metrics.observer_runs, 1);
        assert_eq!(
            metrics.for_kind(TransportRequestKind::DeliverToBlueprint),
            TransportRequestKindChangePerfMetrics::default()
        );
        assert_eq!(
            metrics
                .producer_for_kind(TransportRequestKind::DeliverToBlueprint)
                .steady_observations,
            1
        );
    }

    #[test]
    fn producer_metrics_classify_semantic_repair_and_disable_transitions() {
        let mut app = App::new();
        app.init_resource::<TransportRequestChangePerfMetrics>()
            .add_systems(Update, collect_transport_request_change_metrics_system);

        let anchor = app.world_mut().spawn_empty().id();
        let request_entity = app.world_mut().spawn(active_request_bundle(anchor)).id();
        app.update();

        app.world_mut()
            .resource_mut::<TransportRequestChangePerfMetrics>()
            .clear();
        let mut changed_request = request(anchor);
        changed_request.priority = crate::transport_request::TransportPriority::High;
        app.world_mut()
            .entity_mut(request_entity)
            .insert(changed_request);
        app.update();
        assert_eq!(
            app.world()
                .resource::<TransportRequestChangePerfMetrics>()
                .producer_for_kind(TransportRequestKind::DeliverToBlueprint)
                .semantic_updates,
            1
        );

        app.world_mut()
            .resource_mut::<TransportRequestChangePerfMetrics>()
            .clear();
        app.world_mut()
            .entity_mut(request_entity)
            .remove::<Designation>();
        app.update();
        app.world_mut()
            .resource_mut::<TransportRequestChangePerfMetrics>()
            .clear();
        app.world_mut()
            .entity_mut(request_entity)
            .insert(Designation {
                work_type: WorkType::Haul,
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<TransportRequestChangePerfMetrics>()
                .producer_for_kind(TransportRequestKind::DeliverToBlueprint)
                .missing_repairs,
            1
        );

        app.world_mut()
            .resource_mut::<TransportRequestChangePerfMetrics>()
            .clear();
        app.world_mut()
            .entity_mut(request_entity)
            .remove::<(Designation, TaskSlots, Priority)>();
        app.update();
        assert_eq!(
            app.world()
                .resource::<TransportRequestChangePerfMetrics>()
                .producer_for_kind(TransportRequestKind::DeliverToBlueprint)
                .disable_updates,
            1
        );
    }

    #[test]
    fn soul_spa_collector_owns_its_kind_without_double_counting() {
        let mut app = App::new();
        app.init_resource::<TransportRequestChangePerfMetrics>()
            .add_systems(
                Update,
                (
                    collect_transport_request_change_metrics_system,
                    collect_soul_spa_transport_request_change_metrics_system,
                )
                    .chain(),
            );

        let anchor = app.world_mut().spawn_empty().id();
        app.world_mut()
            .spawn(active_soul_spa_request_bundle(anchor));
        app.update();

        let metrics = app.world().resource::<TransportRequestChangePerfMetrics>();
        assert_eq!(metrics.observer_runs, 1);
        assert_eq!(
            metrics.for_kind(TransportRequestKind::DeliverToSoulSpa),
            TransportRequestKindChangePerfMetrics {
                added: 1,
                changed_existing: 0,
            }
        );
        assert_eq!(
            metrics.producer_for_kind(TransportRequestKind::DeliverToSoulSpa),
            TransportRequestKindProducerPerfMetrics {
                spawns: 1,
                ..Default::default()
            }
        );
    }
}
