//! Producer upsert/cleanup 共通化ヘルパー

use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use std::collections::HashMap;
use std::hash::Hash;

use hw_core::relationships::ManagedBy;
use hw_jobs::{Designation, Priority, TaskSlots, WorkType};

use crate::transport_request::{
    ReceiverPolicyTier, TransportDemand, TransportPolicy, TransportPriority, TransportRequest,
    TransportRequestKind, TransportRequestState,
};
use crate::types::ResourceType;

/// Distinguishes an absolute worker limit from newly available capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestSlots {
    TotalSlots(u32),
    AdditionalSlots(usize),
}

impl RequestSlots {
    pub fn resolve(self, workers: usize) -> u32 {
        match self {
            Self::TotalSlots(total) => total,
            Self::AdditionalSlots(additional) => {
                super::to_u32_saturating(workers.saturating_add(additional))
            }
        }
    }
}

pub fn select_canonical_requests<K: Hash + Eq>(
    requests: impl IntoIterator<Item = (K, Entity, usize)>,
) -> HashMap<K, (Entity, usize)> {
    let mut canonical = HashMap::new();
    for (key, entity, workers) in requests {
        canonical
            .entry(key)
            .and_modify(|current| {
                *current = prefer_canonical_request((entity, workers), *current);
            })
            .or_insert((entity, workers));
    }
    canonical
}

#[derive(Clone, Copy)]
pub struct RequestSlotSnapshot<'a> {
    slots: Option<&'a TaskSlots>,
    demand: Option<&'a TransportDemand>,
    state: Option<&'a TransportRequestState>,
}

impl<'a> RequestSlotSnapshot<'a> {
    pub fn from_runtime(current: ExistingRequestRuntime<'a>) -> Self {
        RequestRuntimeView::from(current).slot_snapshot()
    }
    pub fn from_stockpile_runtime(current: ExistingStockpileRequestRuntime<'a>) -> Self {
        StockpileRequestRuntimeView::from(current)
            .common
            .slot_snapshot()
    }
}

/// A duplicate with workers keeps its committed work, but accepts no new workers.
pub fn reconcile_duplicate_request(
    commands: &mut Commands,
    entity: Entity,
    workers: usize,
    is_canonical: bool,
    current: RequestSlotSnapshot<'_>,
) -> bool {
    if is_canonical {
        return true;
    }
    if workers == 0 {
        commands.entity(entity).try_despawn();
    } else {
        cap_committed_request_if_needed(commands, entity, workers, current);
    }
    false
}

#[inline]
pub fn request_state_for_workers(workers: usize) -> TransportRequestState {
    if workers == 0 {
        TransportRequestState::Pending
    } else {
        TransportRequestState::Claimed
    }
}

/// Producer-owned components on an existing transport request.
///
/// Producers use this snapshot to avoid queueing equivalent deferred writes every frame. Runtime
/// worker relationships are intentionally not part of the snapshot; callers pass the derived
/// inflight count and state explicitly.
pub type ExistingRequestRuntime<'a> = (
    Option<&'a Transform>,
    Option<&'a Visibility>,
    Option<&'a Designation>,
    Option<&'a ManagedBy>,
    Option<&'a TaskSlots>,
    Option<&'a Priority>,
    Option<&'a TransportDemand>,
    Option<&'a TransportRequestState>,
    Option<&'a TransportPolicy>,
);

#[derive(Clone, Copy)]
struct RequestRuntimeView<'a> {
    transform: Option<&'a Transform>,
    visibility: Option<&'a Visibility>,
    designation: Option<&'a Designation>,
    managed_by: Option<&'a ManagedBy>,
    slots: Option<&'a TaskSlots>,
    priority: Option<&'a Priority>,
    demand: Option<&'a TransportDemand>,
    state: Option<&'a TransportRequestState>,
    policy: Option<&'a TransportPolicy>,
}

impl<'a> From<ExistingRequestRuntime<'a>> for RequestRuntimeView<'a> {
    fn from(value: ExistingRequestRuntime<'a>) -> Self {
        let (
            transform,
            visibility,
            designation,
            managed_by,
            slots,
            priority,
            demand,
            state,
            policy,
        ) = value;
        Self {
            transform,
            visibility,
            designation,
            managed_by,
            slots,
            priority,
            demand,
            state,
            policy,
        }
    }
}

impl<'a> RequestRuntimeView<'a> {
    fn slot_snapshot(self) -> RequestSlotSnapshot<'a> {
        RequestSlotSnapshot {
            slots: self.slots,
            demand: self.demand,
            state: self.state,
        }
    }
}

#[derive(Clone, Copy)]
struct StockpileRequestRuntimeView<'a> {
    common: RequestRuntimeView<'a>,
    receiver_tier: Option<&'a ReceiverPolicyTier>,
}

impl<'a> From<ExistingStockpileRequestRuntime<'a>> for StockpileRequestRuntimeView<'a> {
    fn from(value: ExistingStockpileRequestRuntime<'a>) -> Self {
        let (
            transform,
            visibility,
            designation,
            managed_by,
            slots,
            priority,
            receiver_tier,
            demand,
            state,
            policy,
        ) = value;
        Self {
            common: (
                transform,
                visibility,
                designation,
                managed_by,
                slots,
                priority,
                demand,
                state,
                policy,
            )
                .into(),
            receiver_tier,
        }
    }
}

struct CommonRequestSpec {
    site_pos: Vec2,
    issued_by: Entity,
    slots: u32,
    priority: u32,
    work_type: WorkType,
    inflight: u32,
    state: TransportRequestState,
}

fn update_common_header(
    entity: &mut EntityCommands<'_>,
    current: RequestRuntimeView<'_>,
    desired: &CommonRequestSpec,
) {
    let transform = Transform::from_xyz(desired.site_pos.x, desired.site_pos.y, 0.0);
    if current.transform.is_none_or(|value| {
        value.translation != transform.translation
            || value.rotation != transform.rotation
            || value.scale != transform.scale
    }) {
        entity.try_insert(transform);
    }
    if current
        .visibility
        .is_none_or(|value| *value != Visibility::Hidden)
    {
        entity.try_insert(Visibility::Hidden);
    }
    if current
        .designation
        .is_none_or(|value| value.work_type != desired.work_type)
    {
        entity.try_insert(Designation {
            work_type: desired.work_type,
        });
    }
    if current
        .managed_by
        .is_none_or(|value| value.0 != desired.issued_by)
    {
        entity.try_insert(ManagedBy(desired.issued_by));
    }
    if current.slots.is_none_or(|value| value.max != desired.slots) {
        entity.try_insert(TaskSlots::new(desired.slots));
    }
    if current
        .priority
        .is_none_or(|value| value.0 != desired.priority)
    {
        entity.try_insert(Priority(desired.priority));
    }
}

fn update_common_runtime(
    entity: &mut EntityCommands<'_>,
    current: RequestRuntimeView<'_>,
    desired: &CommonRequestSpec,
) {
    if current.demand.is_none_or(|value| {
        value.desired_slots != desired.slots || value.inflight != desired.inflight
    }) {
        entity.try_insert(TransportDemand {
            desired_slots: desired.slots,
            inflight: desired.inflight,
        });
    }
    if current.state.is_none_or(|value| *value != desired.state) {
        entity.try_insert(desired.state);
    }
    if current.policy.is_none_or(|value| !policy_is_default(value)) {
        entity.try_insert(TransportPolicy::default());
    }
}

/// Desired producer-owned values for a request that already exists.
pub struct SemanticRequestSpec {
    pub key: (Entity, ResourceType),
    pub site_pos: Vec2,
    pub issued_by: Entity,
    pub slots: RequestSlots,
    pub inflight: u32,
    pub priority: u32,
    pub transport_priority: TransportPriority,
    pub kind: TransportRequestKind,
    pub work_type: WorkType,
    pub state: TransportRequestState,
}

fn request_matches(current: &TransportRequest, desired: &SemanticRequestSpec) -> bool {
    current.kind == desired.kind
        && current.anchor == desired.key.0
        && current.resource_type == desired.key.1
        && current.issued_by == desired.issued_by
        && current.priority == desired.transport_priority
        && current.stockpile_group.is_empty()
}

fn policy_is_default(current: &TransportPolicy) -> bool {
    let desired = TransportPolicy::default();
    current.allow_cross_area_source == desired.allow_cross_area_source
        && current.allow_cross_familiar_claim == desired.allow_cross_familiar_claim
        && current.source_search_radius_tiles == desired.source_search_radius_tiles
}

/// Updates only producer-owned values that differ from the desired semantic state.
#[inline]
pub fn update_request_runtime_if_needed(
    commands: &mut Commands,
    request_entity: Entity,
    current_request: &TransportRequest,
    current: ExistingRequestRuntime<'_>,
    desired: SemanticRequestSpec,
) {
    let current = RequestRuntimeView::from(current);
    let desired_slots = desired.slots.resolve(desired.inflight as usize);
    let common = CommonRequestSpec {
        site_pos: desired.site_pos,
        issued_by: desired.issued_by,
        slots: desired_slots,
        priority: desired.priority,
        work_type: desired.work_type,
        inflight: desired.inflight,
        state: desired.state,
    };
    let mut entity_commands = commands.entity(request_entity);
    update_common_header(&mut entity_commands, current, &common);
    if !request_matches(current_request, &desired) {
        entity_commands.try_insert(TransportRequest {
            kind: desired.kind,
            anchor: desired.key.0,
            resource_type: desired.key.1,
            issued_by: desired.issued_by,
            priority: desired.transport_priority,
            stockpile_group: Vec::new(),
        });
    }
    update_common_runtime(&mut entity_commands, current, &common);
}

/// Removes activation components, and optionally reconciles demand, only when they differ.
#[inline]
pub fn disable_request_if_needed(
    commands: &mut Commands,
    request_entity: Entity,
    current: ExistingRequestRuntime<'_>,
    inflight: Option<u32>,
) {
    let current = RequestRuntimeView::from(current);
    let mut entity_commands = commands.entity(request_entity);
    if current.designation.is_some() {
        entity_commands.try_remove::<Designation>();
    }
    if current.slots.is_some() {
        entity_commands.try_remove::<TaskSlots>();
    }
    if current.priority.is_some() {
        entity_commands.try_remove::<Priority>();
    }
    if let Some(inflight) = inflight
        && current
            .demand
            .is_none_or(|current| current.desired_slots != 0 || current.inflight != inflight)
    {
        entity_commands.try_insert(TransportDemand {
            desired_slots: 0,
            inflight,
        });
    }
}

/// Stockpile producers own one additional receiver-policy component and a non-empty source group.
pub type ExistingStockpileRequestRuntime<'a> = (
    Option<&'a Transform>,
    Option<&'a Visibility>,
    Option<&'a Designation>,
    Option<&'a ManagedBy>,
    Option<&'a TaskSlots>,
    Option<&'a Priority>,
    Option<&'a ReceiverPolicyTier>,
    Option<&'a TransportDemand>,
    Option<&'a TransportRequestState>,
    Option<&'a TransportPolicy>,
);

/// Semantic state shared by deposit and consolidation stockpile requests.
pub struct StockpileRequestSpec<'a> {
    pub name: &'static str,
    pub anchor: Entity,
    pub resource_type: ResourceType,
    pub site_pos: Vec2,
    pub issued_by: Entity,
    pub slots: RequestSlots,
    pub job_priority: u32,
    pub transport_priority: TransportPriority,
    pub stockpile_group: &'a [Entity],
    pub receiver_policy_tier: TransportPriority,
    pub kind: TransportRequestKind,
    pub work_type: WorkType,
}

/// Prefer an in-flight request, then the stable lowest entity id.
#[inline]
pub fn prefer_canonical_request(
    candidate: (Entity, usize),
    current: (Entity, usize),
) -> (Entity, usize) {
    match (candidate.1 > 0, current.1 > 0) {
        (true, false) => candidate,
        (false, true) => current,
        _ if entity_sort_key(candidate.0) < entity_sort_key(current.0) => candidate,
        _ => current,
    }
}

fn entity_sort_key(entity: Entity) -> (u32, u32) {
    (entity.index_u32(), entity.generation().to_bits())
}

fn stockpile_request_matches(
    current: &TransportRequest,
    desired: &StockpileRequestSpec<'_>,
) -> bool {
    current.kind == desired.kind
        && current.anchor == desired.anchor
        && current.resource_type == desired.resource_type
        && current.issued_by == desired.issued_by
        && current.priority == desired.transport_priority
        && current.stockpile_group == desired.stockpile_group
}

fn update_stockpile_request_if_needed(
    commands: &mut Commands,
    entity: Entity,
    current_request: &TransportRequest,
    current: ExistingStockpileRequestRuntime<'_>,
    desired: &StockpileRequestSpec<'_>,
    workers: usize,
) {
    let current = StockpileRequestRuntimeView::from(current);
    let desired_slots = desired.slots.resolve(workers);
    let inflight = super::to_u32_saturating(workers);
    let desired_state = request_state_for_workers(workers);
    let common = CommonRequestSpec {
        site_pos: desired.site_pos,
        issued_by: desired.issued_by,
        slots: desired_slots,
        priority: desired.job_priority,
        work_type: desired.work_type,
        inflight,
        state: desired_state,
    };
    let mut entity_commands = commands.entity(entity);
    update_common_header(&mut entity_commands, current.common, &common);
    if !stockpile_request_matches(current_request, desired) {
        entity_commands.try_insert(TransportRequest {
            kind: desired.kind,
            anchor: desired.anchor,
            resource_type: desired.resource_type,
            issued_by: desired.issued_by,
            priority: desired.transport_priority,
            stockpile_group: desired.stockpile_group.to_vec(),
        });
    }
    if current
        .receiver_tier
        .is_none_or(|current| current.0 != desired.receiver_policy_tier)
    {
        entity_commands.try_insert(ReceiverPolicyTier(desired.receiver_policy_tier));
    }
    update_common_runtime(&mut entity_commands, current.common, &common);
}

fn cap_committed_request_if_needed(
    commands: &mut Commands,
    entity: Entity,
    workers: usize,
    current: RequestSlotSnapshot<'_>,
) {
    let workers = super::to_u32_saturating(workers);
    let RequestSlotSnapshot {
        slots,
        demand,
        state,
    } = current;
    let mut entity_commands = commands.entity(entity);
    if slots.is_none_or(|current| current.max != workers) {
        entity_commands.try_insert(TaskSlots::new(workers));
    }
    if demand.is_none_or(|current| current.desired_slots != workers || current.inflight != workers)
    {
        entity_commands.try_insert(TransportDemand {
            desired_slots: workers,
            inflight: workers,
        });
    }
    if state.is_none_or(|current| *current != TransportRequestState::Claimed) {
        entity_commands.try_insert(TransportRequestState::Claimed);
    }
}

fn disable_workerless_stockpile_request_if_needed(
    commands: &mut Commands,
    entity: Entity,
    current: ExistingStockpileRequestRuntime<'_>,
) {
    let current = StockpileRequestRuntimeView::from(current);
    let mut entity_commands = commands.entity(entity);
    if current.common.designation.is_some() {
        entity_commands.try_remove::<Designation>();
    }
    if current.common.slots.is_some() {
        entity_commands.try_remove::<TaskSlots>();
    }
    if current.common.priority.is_some() {
        entity_commands.try_remove::<Priority>();
    }
    if current.receiver_tier.is_some() {
        entity_commands.try_remove::<ReceiverPolicyTier>();
    }
    if current
        .common
        .demand
        .is_none_or(|current| current.desired_slots != 0 || current.inflight != 0)
    {
        entity_commands.try_insert(TransportDemand {
            desired_slots: 0,
            inflight: 0,
        });
    }
}

/// Reconcile the shared stockpile-request lifecycle, including duplicate and committed handling.
#[inline]
pub fn reconcile_stockpile_request(
    commands: &mut Commands,
    entity: Entity,
    current_request: &TransportRequest,
    current: ExistingStockpileRequestRuntime<'_>,
    workers: usize,
    is_canonical: bool,
    desired: Option<&StockpileRequestSpec<'_>>,
) {
    if !reconcile_duplicate_request(
        commands,
        entity,
        workers,
        is_canonical,
        RequestSlotSnapshot::from_stockpile_runtime(current),
    ) {
        return;
    }

    if let Some(desired) = desired {
        update_stockpile_request_if_needed(
            commands,
            entity,
            current_request,
            current,
            desired,
            workers,
        );
    } else if workers == 0 {
        disable_workerless_stockpile_request_if_needed(commands, entity, current);
    } else {
        cap_committed_request_if_needed(
            commands,
            entity,
            workers,
            RequestSlotSnapshot::from_stockpile_runtime(current),
        );
    }
}

/// Spawn one request using the same semantic shape accepted by the reconciler.
#[inline]
pub fn spawn_stockpile_request(commands: &mut Commands, desired: StockpileRequestSpec<'_>) {
    let desired_slots = desired.slots.resolve(0);
    commands.spawn((
        Name::new(desired.name),
        Transform::from_xyz(desired.site_pos.x, desired.site_pos.y, 0.0),
        Visibility::Hidden,
        Designation {
            work_type: desired.work_type,
        },
        ManagedBy(desired.issued_by),
        TaskSlots::new(desired_slots),
        Priority(desired.job_priority),
        TransportRequest {
            kind: desired.kind,
            anchor: desired.anchor,
            resource_type: desired.resource_type,
            issued_by: desired.issued_by,
            priority: desired.transport_priority,
            stockpile_group: desired.stockpile_group.to_vec(),
        },
        ReceiverPolicyTier(desired.receiver_policy_tier),
        TransportDemand {
            desired_slots,
            inflight: 0,
        },
        TransportRequestState::Pending,
        TransportPolicy::default(),
    ));
}

/// 新規 request entity を spawn するためのスペック。
pub struct SpawnRequestSpec<TTarget> {
    pub name: &'static str,
    pub key: (Entity, ResourceType),
    pub site_pos: Vec2,
    pub issued_by: Entity,
    pub slots: RequestSlots,
    pub priority: u32,
    pub target: TTarget,
    pub kind: TransportRequestKind,
    pub work_type: WorkType,
}

/// 新規 request entity を spawn する
#[inline]
pub fn spawn_transport_request<TTarget: Component>(
    commands: &mut Commands,
    spec: SpawnRequestSpec<TTarget>,
) {
    let desired_slots = spec.slots.resolve(0);
    commands.spawn((
        Name::new(spec.name),
        Transform::from_xyz(spec.site_pos.x, spec.site_pos.y, 0.0),
        Visibility::Hidden,
        Designation {
            work_type: spec.work_type,
        },
        ManagedBy(spec.issued_by),
        TaskSlots::new(desired_slots),
        Priority(spec.priority),
        spec.target,
        TransportRequest {
            kind: spec.kind,
            anchor: spec.key.0,
            resource_type: spec.key.1,
            issued_by: spec.issued_by,
            priority: TransportPriority::Normal,
            stockpile_group: vec![],
        },
        TransportDemand {
            desired_slots,
            inflight: 0,
        },
        TransportRequestState::Pending,
        TransportPolicy::default(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_slots_keep_total_and_additional_capacity_distinct_test() {
        assert_eq!(RequestSlots::TotalSlots(3).resolve(2), 3);
        assert_eq!(RequestSlots::AdditionalSlots(3).resolve(2), 5);
        assert_eq!(
            RequestSlots::AdditionalSlots(usize::MAX).resolve(2),
            u32::MAX
        );
    }

    fn reconcile_duplicates(
        mut commands: Commands,
        requests: Query<(
            Entity,
            &TransportRequest,
            Option<&hw_core::relationships::TaskWorkers>,
            ExistingRequestRuntime<'static>,
        )>,
    ) {
        let canonical =
            select_canonical_requests(requests.iter().map(|(entity, request, workers, _)| {
                (
                    request.anchor,
                    entity,
                    workers.map_or(0, |workers| workers.len()),
                )
            }));
        for (entity, request, workers, current) in &requests {
            reconcile_duplicate_request(
                &mut commands,
                entity,
                workers.map_or(0, |workers| workers.len()),
                canonical[&request.anchor].0 == entity,
                RequestSlotSnapshot::from_runtime(current),
            );
        }
    }

    #[test]
    fn duplicate_reconcile_preserves_workers_and_stops_new_admission_test() {
        let mut app = App::new();
        app.add_systems(Update, reconcile_duplicates);
        let anchor = app.world_mut().spawn_empty().id();
        let spawn = |world: &mut World| {
            world
                .spawn((
                    TransportRequest {
                        kind: TransportRequestKind::DeliverToBlueprint,
                        anchor,
                        resource_type: ResourceType::Wood,
                        issued_by: anchor,
                        priority: TransportPriority::Normal,
                        stockpile_group: vec![],
                    },
                    TaskSlots::new(10),
                    TransportDemand {
                        desired_slots: 10,
                        inflight: 0,
                    },
                    TransportRequestState::Pending,
                ))
                .id()
        };
        let workerless = spawn(app.world_mut());
        let first = spawn(app.world_mut());
        let second = spawn(app.world_mut());
        let worker_a = app
            .world_mut()
            .spawn(hw_core::relationships::WorkingOn(first))
            .id();
        let worker_b = app
            .world_mut()
            .spawn(hw_core::relationships::WorkingOn(second))
            .id();
        let forward = select_canonical_requests([
            (anchor, workerless, 0),
            (anchor, first, 1),
            (anchor, second, 1),
        ]);
        let reverse = select_canonical_requests([
            (anchor, second, 1),
            (anchor, first, 1),
            (anchor, workerless, 0),
        ]);
        assert_eq!(forward, reverse);
        let canonical = forward[&anchor].0;
        assert_ne!(canonical, workerless);
        let duplicate = if canonical == first { second } else { first };
        app.update();
        assert!(app.world().get_entity(workerless).is_err());
        assert_eq!(app.world().get::<TaskSlots>(canonical).unwrap().max, 10);
        assert_eq!(app.world().get::<TaskSlots>(duplicate).unwrap().max, 1);
        let demand = app.world().get::<TransportDemand>(duplicate).unwrap();
        assert_eq!(
            (demand.desired_slots, demand.inflight, demand.remaining()),
            (1, 1, 0)
        );
        assert_eq!(
            *app.world().get::<TransportRequestState>(duplicate).unwrap(),
            TransportRequestState::Claimed
        );
        assert_eq!(
            app.world()
                .get::<hw_core::relationships::WorkingOn>(worker_a)
                .unwrap()
                .0,
            first
        );
        assert_eq!(
            app.world()
                .get::<hw_core::relationships::WorkingOn>(worker_b)
                .unwrap()
                .0,
            second
        );
        app.world_mut().clear_trackers();
        app.update();
        let mut changed = app.world_mut().query_filtered::<Entity, Or<(
            Changed<TaskSlots>,
            Changed<TransportDemand>,
            Changed<TransportRequestState>,
        )>>();
        assert_eq!(changed.iter(app.world()).count(), 0);
    }

    #[derive(Resource)]
    struct RequestFixture {
        request: Entity,
        owner: Entity,
        anchor: Entity,
        stockpile_group: Vec<Entity>,
    }

    fn reconcile_active_request(
        mut commands: Commands,
        fixture: Res<RequestFixture>,
        q_requests: Query<(&TransportRequest, ExistingRequestRuntime<'static>)>,
    ) {
        let (request, current) = q_requests.get(fixture.request).unwrap();
        update_request_runtime_if_needed(
            &mut commands,
            fixture.request,
            request,
            current,
            SemanticRequestSpec {
                key: (fixture.anchor, ResourceType::Wood),
                site_pos: Vec2::new(4.0, 8.0),
                issued_by: fixture.owner,
                slots: RequestSlots::TotalSlots(2),
                inflight: 0,
                priority: 3,
                transport_priority: TransportPriority::Normal,
                kind: TransportRequestKind::DeliverToBlueprint,
                work_type: WorkType::Haul,
                state: TransportRequestState::Pending,
            },
        );
    }

    fn disable_request(
        mut commands: Commands,
        fixture: Res<RequestFixture>,
        q_requests: Query<ExistingRequestRuntime<'static>, With<TransportRequest>>,
    ) {
        let current = q_requests.get(fixture.request).unwrap();
        disable_request_if_needed(&mut commands, fixture.request, current, Some(0));
    }

    fn reconcile_active_stockpile_request(
        mut commands: Commands,
        fixture: Res<RequestFixture>,
        q_requests: Query<(&TransportRequest, ExistingStockpileRequestRuntime<'static>)>,
    ) {
        let (request, current) = q_requests.get(fixture.request).unwrap();
        let desired = StockpileRequestSpec {
            name: "TransportRequest::DepositToStockpile",
            anchor: fixture.anchor,
            resource_type: ResourceType::Wood,
            site_pos: Vec2::new(4.0, 8.0),
            issued_by: fixture.owner,
            slots: RequestSlots::AdditionalSlots(2),
            job_priority: 0,
            transport_priority: TransportPriority::High,
            stockpile_group: &fixture.stockpile_group,
            receiver_policy_tier: TransportPriority::High,
            kind: TransportRequestKind::DepositToStockpile,
            work_type: WorkType::Haul,
        };
        reconcile_stockpile_request(
            &mut commands,
            fixture.request,
            request,
            current,
            0,
            true,
            Some(&desired),
        );
    }

    fn active_bundle(anchor: Entity, owner: Entity) -> impl Bundle {
        (
            Transform::from_xyz(4.0, 8.0, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            ManagedBy(owner),
            TaskSlots::new(2),
            Priority(3),
            TransportRequest {
                kind: TransportRequestKind::DeliverToBlueprint,
                anchor,
                resource_type: ResourceType::Wood,
                issued_by: owner,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TransportDemand {
                desired_slots: 2,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        )
    }

    fn fixture() -> App {
        let mut app = App::new();
        let owner = app.world_mut().spawn_empty().id();
        let anchor = app.world_mut().spawn_empty().id();
        let request = app.world_mut().spawn(active_bundle(anchor, owner)).id();
        app.insert_resource(RequestFixture {
            request,
            owner,
            anchor,
            stockpile_group: vec![anchor],
        });
        app
    }

    fn stockpile_fixture() -> App {
        let mut app = App::new();
        let owner = app.world_mut().spawn_empty().id();
        let anchor = app.world_mut().spawn_empty().id();
        let request = app
            .world_mut()
            .spawn((
                Transform::from_xyz(4.0, 8.0, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::Haul,
                },
                ManagedBy(owner),
                TaskSlots::new(2),
                Priority(0),
                TransportRequest {
                    kind: TransportRequestKind::DepositToStockpile,
                    anchor,
                    resource_type: ResourceType::Wood,
                    issued_by: owner,
                    priority: TransportPriority::High,
                    stockpile_group: vec![anchor],
                },
                ReceiverPolicyTier(TransportPriority::High),
                TransportDemand {
                    desired_slots: 2,
                    inflight: 0,
                },
                TransportRequestState::Pending,
                TransportPolicy::default(),
            ))
            .id();
        app.insert_resource(RequestFixture {
            request,
            owner,
            anchor,
            stockpile_group: vec![anchor],
        });
        app
    }

    #[test]
    fn active_reconciliation_repairs_missing_components_and_owner_relationship() {
        for stockpile in [false, true] {
            let mut app = if stockpile {
                stockpile_fixture()
            } else {
                fixture()
            };
            let request = app.world().resource::<RequestFixture>().request;
            let owner = app.world().resource::<RequestFixture>().owner;
            app.world_mut()
                .entity_mut(request)
                .remove::<(Visibility, ManagedBy, TransportPolicy)>();
            if stockpile {
                app.add_systems(Update, reconcile_active_stockpile_request);
            } else {
                app.add_systems(Update, reconcile_active_request);
            }
            app.update();
            assert_eq!(
                app.world().get::<Visibility>(request),
                Some(&Visibility::Hidden)
            );
            assert_eq!(app.world().get::<ManagedBy>(request).unwrap().0, owner);
            assert!(
                app.world()
                    .get::<hw_core::relationships::ManagedTasks>(owner)
                    .unwrap()
                    .contains(request)
            );
            assert!(app.world().get::<TransportPolicy>(request).is_some());
            app.world_mut().clear_trackers();
            app.update();
            let entity = app.world().entity(request);
            assert!(!entity.get_ref::<Visibility>().unwrap().is_changed());
            assert!(!entity.get_ref::<ManagedBy>().unwrap().is_changed());
            assert!(!entity.get_ref::<TransportPolicy>().unwrap().is_changed());
        }
    }

    #[test]
    fn active_reconciliation_repairs_only_changed_priority() {
        for stockpile in [false, true] {
            let mut app = if stockpile {
                stockpile_fixture()
            } else {
                fixture()
            };
            let request = app.world().resource::<RequestFixture>().request;
            app.world_mut().entity_mut(request).insert(Priority(99));
            if stockpile {
                app.add_systems(Update, reconcile_active_stockpile_request);
            } else {
                app.add_systems(Update, reconcile_active_request);
            }
            app.world_mut().clear_trackers();
            app.update();
            let entity = app.world().entity(request);
            assert_eq!(
                entity.get::<Priority>().unwrap().0,
                if stockpile { 0 } else { 3 }
            );
            assert!(!entity.get_ref::<TransportRequest>().unwrap().is_changed());
            assert!(!entity.get_ref::<TransportDemand>().unwrap().is_changed());
            assert!(
                !entity
                    .get_ref::<TransportRequestState>()
                    .unwrap()
                    .is_changed()
            );
        }
    }

    #[test]
    fn second_active_reconciliation_does_not_dirty_owned_components() {
        let mut app = fixture();
        app.add_systems(Update, reconcile_active_request);
        app.update();
        app.world_mut().clear_trackers();
        app.update();

        let request_entity = app.world().resource::<RequestFixture>().request;
        let request = app.world().entity(request_entity);
        assert!(!request.get_ref::<Transform>().unwrap().is_changed());
        assert!(!request.get_ref::<Visibility>().unwrap().is_changed());
        assert!(!request.get_ref::<Designation>().unwrap().is_changed());
        assert!(!request.get_ref::<ManagedBy>().unwrap().is_changed());
        assert!(!request.get_ref::<TaskSlots>().unwrap().is_changed());
        assert!(!request.get_ref::<Priority>().unwrap().is_changed());
        assert!(!request.get_ref::<TransportRequest>().unwrap().is_changed());
        assert!(!request.get_ref::<TransportDemand>().unwrap().is_changed());
        assert!(
            !request
                .get_ref::<TransportRequestState>()
                .unwrap()
                .is_changed()
        );
        assert!(!request.get_ref::<TransportPolicy>().unwrap().is_changed());
    }

    #[test]
    fn second_disable_reconciliation_queues_no_equivalent_demand_write() {
        let mut app = fixture();
        app.add_systems(Update, disable_request);
        app.update();
        app.world_mut().clear_trackers();
        app.update();

        let request_entity = app.world().resource::<RequestFixture>().request;
        let request = app.world().entity(request_entity);
        assert!(!request.contains::<Designation>());
        assert!(!request.contains::<TaskSlots>());
        assert!(!request.contains::<Priority>());
        let demand = request.get_ref::<TransportDemand>().unwrap();
        assert_eq!((demand.desired_slots, demand.inflight), (0, 0));
        assert!(!demand.is_changed());
    }

    #[test]
    fn second_stockpile_reconciliation_does_not_dirty_owned_components() {
        let mut app = stockpile_fixture();
        app.add_systems(Update, reconcile_active_stockpile_request);
        app.update();
        app.world_mut().clear_trackers();
        app.update();

        let request_entity = app.world().resource::<RequestFixture>().request;
        let request = app.world().entity(request_entity);
        assert!(!request.get_ref::<Transform>().unwrap().is_changed());
        assert!(!request.get_ref::<Visibility>().unwrap().is_changed());
        assert!(!request.get_ref::<Designation>().unwrap().is_changed());
        assert!(!request.get_ref::<ManagedBy>().unwrap().is_changed());
        assert!(!request.get_ref::<TaskSlots>().unwrap().is_changed());
        assert!(!request.get_ref::<Priority>().unwrap().is_changed());
        assert!(!request.get_ref::<TransportRequest>().unwrap().is_changed());
        assert!(
            !request
                .get_ref::<ReceiverPolicyTier>()
                .unwrap()
                .is_changed()
        );
        assert!(!request.get_ref::<TransportDemand>().unwrap().is_changed());
        assert!(
            !request
                .get_ref::<TransportRequestState>()
                .unwrap()
                .is_changed()
        );
        assert!(!request.get_ref::<TransportPolicy>().unwrap().is_changed());
    }
}
