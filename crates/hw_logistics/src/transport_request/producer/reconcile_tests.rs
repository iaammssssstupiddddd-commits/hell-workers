//! Producer contracts observed before and after the production Maintain stage.
use super::*;
use crate::transport_request::{
    TransportDemand, TransportRequestPlugin, TransportRequestSet, TransportRequestState,
};
use crate::{
    BelongsTo, BucketStorage, SharedResourceCache, Stockpile, StockpilePolicy, Wheelbarrow,
};
use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::{BUCKET_CAPACITY, MUD_MIXER_CAPACITY, TILE_SIZE};
use hw_core::events::SoulTaskUnassignRequest;
use hw_core::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use hw_core::relationships::{ParkedAt, StoredIn, WorkingOn};
use hw_core::system_sets::{GameSystemSet, SoulAiSystemSet};
use hw_jobs::construction::{FloorTileBlueprint, WallTileBlueprint};
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{
    Blueprint, Building, BuildingType, Designation, FloorConstructionSite, FloorTileState,
    ProvisionalWall, TaskSlots, WallConstructionSite, WallTileState,
};
use hw_spatial::{BlueprintSpatialGrid, FloorConstructionSpatialGrid, StockpileSpatialGrid};

#[derive(Clone, Debug)]
struct Snapshot {
    active: bool,
    slots: Option<u32>,
    desired: u32,
    inflight: u32,
}

#[derive(Resource, Default)]
struct BeforeMaintain(HashMap<Entity, Snapshot>);

fn record_decision(
    query: Query<(
        Entity,
        Option<&Designation>,
        Option<&TaskSlots>,
        &TransportDemand,
    )>,
    mut recorded: ResMut<BeforeMaintain>,
) {
    recorded.0 = query
        .iter()
        .map(|(entity, designation, slots, demand)| {
            (
                entity,
                Snapshot {
                    active: designation.is_some(),
                    slots: slots.map(|slots| slots.max),
                    desired: demand.desired_slots,
                    inflight: demand.inflight,
                },
            )
        })
        .collect();
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<SharedResourceCache>()
        .init_resource::<BlueprintSpatialGrid>()
        .init_resource::<FloorConstructionSpatialGrid>()
        .init_resource::<ResourceSpatialGrid>()
        .init_resource::<StockpileSpatialGrid>()
        .init_resource::<crate::tile_index::TileSiteIndex>()
        .init_resource::<BeforeMaintain>()
        .add_message::<SoulTaskUnassignRequest>()
        .configure_sets(Update, (GameSystemSet::Logic, GameSystemSet::Actor).chain())
        .configure_sets(Update, SoulAiSystemSet::Actor.in_set(GameSystemSet::Actor))
        .add_plugins(TransportRequestPlugin)
        .add_systems(
            Update,
            (ApplyDeferred, record_decision)
                .chain()
                .after(TransportRequestSet::Decide)
                .before(TransportRequestSet::Arbitrate)
                .in_set(GameSystemSet::Logic),
        );
    app
}

fn owner(world: &mut World, familiar: bool) -> Entity {
    let area = TaskArea::from_points(Vec2::splat(-256.0), Vec2::splat(256.0));
    if familiar {
        world
            .spawn((
                Familiar::default(),
                ActiveCommand {
                    command: FamiliarCommand::GatherResources,
                },
                area,
            ))
            .id()
    } else {
        world
            .spawn(Yard {
                min: area.bounds.min,
                max: area.bounds.max,
            })
            .id()
    }
}

fn request(
    world: &mut World,
    kind: TransportRequestKind,
    anchor: Entity,
    resource: ResourceType,
) -> Option<Entity> {
    world
        .query::<(Entity, &TransportRequest)>()
        .iter(world)
        .find_map(|(entity, req)| {
            (req.kind == kind && req.anchor == anchor && req.resource_type == resource)
                .then_some(entity)
        })
}

fn stockpile(world: &mut World, owner: Entity, position: Vec2, capacity: usize) -> Entity {
    let entity = world
        .spawn((
            Transform::from_translation(position.extend(0.0)),
            Stockpile {
                capacity,
                resource_type: Some(ResourceType::Wood),
            },
            StockpilePolicy::for_capacity(capacity),
            BelongsTo(owner),
        ))
        .id();
    world
        .resource_mut::<StockpileSpatialGrid>()
        .insert(entity, position);
    entity
}

fn fixture(kind: TransportRequestKind, resource: ResourceType) -> (App, Entity, Entity) {
    use TransportRequestKind::*;
    let mut app = app();
    let familiar = matches!(kind, ReturnWheelbarrow | DeliverToFloorConstruction);
    let owner = owner(app.world_mut(), familiar);
    let area = TaskArea::from_points(Vec2::ZERO, Vec2::ZERO);
    let mut auxiliary = Entity::PLACEHOLDER;
    let anchor = match kind {
        DeliverToBlueprint => {
            let mut blueprint = Blueprint::new(BuildingType::Tank, vec![(0, 0)]);
            blueprint.required_materials = HashMap::from([(resource, 3)]);
            blueprint.flexible_material_requirement = None;
            let entity = app
                .world_mut()
                .spawn((Transform::default(), blueprint))
                .id();
            app.world_mut()
                .resource_mut::<BlueprintSpatialGrid>()
                .insert(entity, Vec2::ZERO);
            entity
        }
        GatherWaterToTank | DeliverToMixerSolid | DeliverWaterToMixer | ReturnBucket => {
            let entity = app
                .world_mut()
                .spawn((
                    Transform::default(),
                    Stockpile {
                        capacity: 3 * BUCKET_CAPACITY as usize,
                        resource_type: Some(ResourceType::Water),
                    },
                ))
                .id();
            if matches!(kind, DeliverToMixerSolid | DeliverWaterToMixer) {
                app.world_mut()
                    .entity_mut(entity)
                    .insert(MudMixerStorage::default());
            }
            if kind == ReturnBucket {
                app.world_mut().spawn((
                    BucketStorage,
                    BelongsTo(entity),
                    Stockpile {
                        capacity: 3,
                        resource_type: None,
                    },
                ));
                auxiliary = app
                    .world_mut()
                    .spawn((
                        Visibility::Visible,
                        ResourceItem(ResourceType::BucketEmpty),
                        BelongsTo(entity),
                    ))
                    .id();
            }
            entity
        }
        ReturnWheelbarrow => {
            auxiliary = app.world_mut().spawn(Transform::default()).id();
            app.world_mut()
                .spawn((
                    Transform::from_xyz(80.0, 0.0, 0.0),
                    Wheelbarrow { capacity: 8 },
                    ParkedAt(auxiliary),
                ))
                .id()
        }
        DeliverToFloorConstruction => {
            let site = app
                .world_mut()
                .spawn((
                    Transform::default(),
                    FloorConstructionSite::new(area, Vec2::ZERO, 1),
                ))
                .id();
            auxiliary = app
                .world_mut()
                .spawn(FloorTileBlueprint::new(site, (0, 0)))
                .id();
            app.world_mut()
                .resource_mut::<FloorConstructionSpatialGrid>()
                .insert(site, Vec2::ZERO);
            site
        }
        DeliverToWallConstruction => {
            let site = app
                .world_mut()
                .spawn((
                    Transform::default(),
                    WallConstructionSite::new(area, Vec2::ZERO, 1),
                ))
                .id();
            auxiliary = app
                .world_mut()
                .spawn(WallTileBlueprint::new(site, (0, 0)))
                .id();
            site
        }
        DeliverToProvisionalWall => app
            .world_mut()
            .spawn((
                Transform::default(),
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: true,
                },
                ProvisionalWall::default(),
            ))
            .id(),
        DepositToStockpile => {
            let cell = stockpile(app.world_mut(), owner, Vec2::ZERO, 3);
            app.world_mut().spawn((
                Transform::from_xyz(TILE_SIZE, 0.0, 0.0),
                Visibility::Visible,
                ResourceItem(ResourceType::Wood),
            ));
            cell
        }
        ConsolidateStockpile => {
            let receiver = stockpile(app.world_mut(), owner, Vec2::ZERO, 10);
            auxiliary = stockpile(app.world_mut(), owner, Vec2::new(TILE_SIZE, 0.0), 10);
            for _ in 0..8 {
                app.world_mut()
                    .spawn((ResourceItem(ResourceType::Wood), StoredIn(receiver)));
            }
            app.world_mut()
                .spawn((ResourceItem(ResourceType::Wood), StoredIn(auxiliary)));
            receiver
        }
        _ => unreachable!("separate fixture covers retired Batch and root SoulSpa"),
    };
    (app, anchor, auxiliary)
}

fn set_demand(
    world: &mut World,
    kind: TransportRequestKind,
    resource: ResourceType,
    anchor: Entity,
    auxiliary: Entity,
    enabled: bool,
) {
    use TransportRequestKind::*;
    match kind {
        DeliverToBlueprint => {
            world
                .get_mut::<Blueprint>(anchor)
                .unwrap()
                .delivered_materials
                .insert(resource, if enabled { 0 } else { 3 });
        }
        GatherWaterToTank | DeliverWaterToMixer => {
            let stored: Vec<_> = world
                .query::<(Entity, &StoredIn)>()
                .iter(world)
                .filter_map(|(item, stored)| (stored.0 == anchor).then_some(item))
                .collect();
            for item in stored {
                world.entity_mut(item).despawn();
            }
            if !enabled {
                for _ in 0..3 * BUCKET_CAPACITY {
                    world.spawn((ResourceItem(ResourceType::Water), StoredIn(anchor)));
                }
            }
        }
        DeliverToMixerSolid => {
            let mut storage = world.get_mut::<MudMixerStorage>(anchor).unwrap();
            let amount = if enabled { 0 } else { MUD_MIXER_CAPACITY };
            match resource {
                ResourceType::Sand => storage.sand = amount,
                ResourceType::Rock => storage.rock = amount,
                _ => unreachable!(),
            }
        }
        ReturnBucket => {
            *world.get_mut::<Visibility>(auxiliary).unwrap() = if enabled {
                Visibility::Visible
            } else {
                Visibility::Hidden
            }
        }
        ReturnWheelbarrow => {
            world.get_mut::<Transform>(anchor).unwrap().translation.x =
                if enabled { 80.0 } else { 0.0 }
        }
        DeliverToFloorConstruction => {
            world
                .get_mut::<FloorTileBlueprint>(auxiliary)
                .unwrap()
                .bones_delivered = if enabled {
                0
            } else {
                hw_core::constants::FLOOR_BONES_PER_TILE
            }
        }
        DeliverToWallConstruction => {
            world
                .get_mut::<WallTileBlueprint>(auxiliary)
                .unwrap()
                .wood_delivered = if enabled {
                0
            } else {
                hw_core::constants::WALL_WOOD_PER_TILE
            }
        }
        DeliverToProvisionalWall => {
            world
                .get_mut::<ProvisionalWall>(anchor)
                .unwrap()
                .mud_delivered = !enabled
        }
        DepositToStockpile => {
            world
                .get_mut::<StockpilePolicy>(anchor)
                .unwrap()
                .target_amount = if enabled { 3 } else { 0 }
        }
        ConsolidateStockpile => {
            for cell in [anchor, auxiliary] {
                world.get_mut::<StockpilePolicy>(cell).unwrap().allow_export = enabled;
            }
        }
        _ => unreachable!(),
    }
    // Keep construction tiles in a material-waiting phase; this test changes actual demand,
    // and does not run a construction worker that advances the next phase.
    if kind == DeliverToFloorConstruction {
        world
            .get_mut::<FloorTileBlueprint>(auxiliary)
            .unwrap()
            .state = FloorTileState::WaitingBones;
    }
    if kind == DeliverToWallConstruction {
        world.get_mut::<WallTileBlueprint>(auxiliary).unwrap().state = WallTileState::WaitingWood;
    }
}

#[test]
fn zero_demand_policy_survives_production_maintain_and_reactivation_test() {
    use TransportRequestKind::*;
    let cases = [
        (DeliverToBlueprint, ResourceType::Wood),
        (GatherWaterToTank, ResourceType::Water),
        (DeliverToMixerSolid, ResourceType::Sand),
        (DeliverToMixerSolid, ResourceType::Rock),
        (DeliverWaterToMixer, ResourceType::Water),
        (ReturnBucket, ResourceType::BucketEmpty),
        (ReturnWheelbarrow, ResourceType::Wheelbarrow),
        (DeliverToFloorConstruction, ResourceType::Bone),
        (DeliverToWallConstruction, ResourceType::Wood),
        (DeliverToProvisionalWall, ResourceType::StasisMud),
        (DepositToStockpile, ResourceType::Wood),
        (ConsolidateStockpile, ResourceType::Wood),
    ];
    for (kind, resource) in cases {
        for has_worker in [false, true] {
            let (mut app, anchor, auxiliary) = fixture(kind, resource);
            app.update();
            let original = request(app.world_mut(), kind, anchor, resource)
                .expect("producer generated request");
            let worker = has_worker.then(|| app.world_mut().spawn(WorkingOn(original)).id());
            app.update();
            let positive = app.world().resource::<BeforeMaintain>().0[&original].clone();
            set_demand(app.world_mut(), kind, resource, anchor, auxiliary, false);
            app.update();
            let preserves_demand = matches!(
                kind,
                DeliverToBlueprint | GatherWaterToTank | DeliverToMixerSolid | DeliverWaterToMixer
            );
            let stockpile = matches!(kind, DepositToStockpile | ConsolidateStockpile);
            let before = app.world().resource::<BeforeMaintain>().0.get(&original);
            if kind == ReturnWheelbarrow && !has_worker {
                assert!(before.is_none());
            } else {
                let before = before.unwrap();
                assert_eq!(
                    before.active,
                    has_worker && (preserves_demand || stockpile),
                    "{kind:?}/{resource:?}/{has_worker}: {before:?}"
                );
                if preserves_demand {
                    assert!(before.desired > 0);
                    assert_eq!(
                        (before.desired, before.inflight),
                        (positive.desired, positive.inflight)
                    );
                    assert_eq!(before.slots.is_some(), has_worker);
                    if has_worker {
                        assert_eq!(before.slots, positive.slots);
                    }
                } else {
                    assert_eq!(
                        before.desired,
                        u32::from(stockpile && has_worker),
                        "{kind:?}"
                    );
                    assert_eq!(before.inflight, u32::from(has_worker), "{kind:?}");
                    assert_eq!(
                        before.slots,
                        (stockpile && has_worker).then_some(1),
                        "{kind:?}"
                    );
                }
            }
            let retained = preserves_demand || has_worker;
            assert_eq!(
                app.world().get_entity(original).is_ok(),
                retained,
                "{kind:?}"
            );
            if let Some(worker) = worker {
                assert_eq!(app.world().get::<WorkingOn>(worker).unwrap().0, original);
            }
            set_demand(app.world_mut(), kind, resource, anchor, auxiliary, true);
            // One-unit provisional-wall demand is entirely committed while its worker remains.
            if kind == DeliverToProvisionalWall
                && let Some(worker) = worker
            {
                app.world_mut().entity_mut(worker).remove::<WorkingOn>();
            }
            app.update();
            let renewed =
                request(app.world_mut(), kind, anchor, resource).expect("demand restored");
            assert_eq!(renewed == original, retained, "{kind:?}");
            assert!(
                app.world().get::<Designation>(renewed).is_some(),
                "{kind:?}"
            );
            assert!(
                app.world().get::<TaskSlots>(renewed).unwrap().max > 0,
                "{kind:?}"
            );
            assert!(
                app.world()
                    .get::<TransportDemand>(renewed)
                    .unwrap()
                    .desired_slots
                    > 0,
                "{kind:?}"
            );
        }
    }
}

#[test]
fn inactive_mixers_and_retired_batch_requests_preserve_only_committed_workers_test() {
    use TransportRequestKind::*;
    for kind in [DeliverToMixerSolid, DeliverWaterToMixer, BatchWheelbarrow] {
        for has_worker in [false, true] {
            let resource = if kind == DeliverToMixerSolid {
                ResourceType::Sand
            } else {
                ResourceType::Water
            };
            let (mut app, anchor, _) = fixture(DeliverWaterToMixer, resource);
            app.update();
            let original = if kind == BatchWheelbarrow {
                let issuer = app
                    .world_mut()
                    .query_filtered::<Entity, With<Yard>>()
                    .single(app.world())
                    .unwrap();
                app.world_mut()
                    .spawn((
                        TransportRequest {
                            kind,
                            anchor,
                            resource_type: resource,
                            issued_by: issuer,
                            priority: TransportPriority::Normal,
                            stockpile_group: vec![],
                        },
                        TransportDemand {
                            desired_slots: 1,
                            inflight: 0,
                        },
                        TransportRequestState::Pending,
                    ))
                    .id()
            } else {
                request(app.world_mut(), kind, anchor, resource).unwrap()
            };
            let worker = has_worker.then(|| app.world_mut().spawn(WorkingOn(original)).id());
            if kind != BatchWheelbarrow {
                let order = app.world_mut().spawn_empty().id();
                app.world_mut()
                    .entity_mut(anchor)
                    .insert(hw_jobs::DeconstructionPending { order });
            }
            app.update();
            app.update();
            assert_eq!(
                app.world().get_entity(original).is_ok(),
                has_worker,
                "{kind:?}"
            );
            if let Some(worker) = worker {
                assert_eq!(app.world().get::<WorkingOn>(worker).unwrap().0, original);
            }
            if kind != BatchWheelbarrow {
                app.world_mut()
                    .entity_mut(anchor)
                    .remove::<hw_jobs::DeconstructionPending>();
                app.update();
                let renewed = request(app.world_mut(), kind, anchor, resource).unwrap();
                assert_eq!(renewed == original, has_worker);
                assert!(app.world().get::<Designation>(renewed).is_some());
            }
        }
    }
}

#[derive(Resource)]
struct ActorMutation {
    target: Entity,
    remove_worker: bool,
    remove_issuer: bool,
}

fn mutate_actor(mut commands: Commands, mutation: Option<Res<ActorMutation>>) {
    let Some(mutation) = mutation else { return };
    if mutation.remove_worker {
        commands.entity(mutation.target).remove::<WorkingOn>();
    } else if mutation.remove_issuer {
        commands.entity(mutation.target).remove::<Yard>();
    } else {
        commands.entity(mutation.target).try_despawn();
    }
    commands.remove_resource::<ActorMutation>();
}

#[test]
fn automatic_maintain_observes_actor_worker_and_owner_removal_test() {
    use TransportRequestKind::ReturnBucket;
    for mode in ["worker", "anchor", "issuer"] {
        let (mut app, anchor, bucket) = fixture(ReturnBucket, ResourceType::BucketEmpty);
        app.add_systems(Update, mutate_actor.in_set(SoulAiSystemSet::Actor));
        app.update();
        let request = request(
            app.world_mut(),
            ReturnBucket,
            anchor,
            ResourceType::BucketEmpty,
        )
        .unwrap();
        let worker = app.world_mut().spawn(WorkingOn(request)).id();
        app.update();
        if mode == "worker" {
            set_demand(
                app.world_mut(),
                ReturnBucket,
                ResourceType::BucketEmpty,
                anchor,
                bucket,
                false,
            );
        }
        let issuer = app
            .world()
            .get::<TransportRequest>(request)
            .unwrap()
            .issued_by;
        app.insert_resource(ActorMutation {
            target: match mode {
                "worker" => worker,
                "anchor" => anchor,
                _ => issuer,
            },
            remove_worker: mode == "worker",
            remove_issuer: mode == "issuer",
        });
        app.update();
        assert!(app.world().get_entity(request).is_err(), "{mode}");
        let messages: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<SoulTaskUnassignRequest>>()
            .drain()
            .collect();
        assert_eq!(messages.len(), usize::from(mode != "worker"), "{mode}");
        if let Some(message) = messages.first() {
            assert_eq!(message.soul_entity, worker);
            assert!(message.emit_abandoned);
        }
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Messages<SoulTaskUnassignRequest>>()
                .drain()
                .next()
                .is_none()
        );
    }
}
