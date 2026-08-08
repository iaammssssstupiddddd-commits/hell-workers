//! Owner-safe cancellation for a constructing Soul Spa.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridGenerators, PowerConsumer, PowerGenerator,
    SoulSpaConstructionCancelOutcome, SoulSpaConstructionCancelRequest,
    SoulSpaConstructionCancelResult, SoulSpaPhase, SoulSpaSite, SoulSpaTile,
};
use hw_jobs::{Building, BuildingType, DeconstructionPending, Designation, TaskSlots};
use hw_logistics::construction_helpers::{ResourceItemVisualHandles, spawn_refund_items};
use hw_logistics::transport_request::{
    OwnerTransportCleanupResult, close_transport_requests_for_removed_owners,
    transport_requests_referencing_removed_owners,
};
use hw_logistics::{ResourceType, SharedResourceCache};
use hw_soul_ai::{ExactTaskTerminalRequest, ExactTaskTerminalResult, terminalize_exact_tasks};
use hw_visual::Building3dVisual;
use hw_world::map::WorldMapOwnerSnapshot;
use hw_world::{RoomDetectionState, WorldMap};

use crate::systems::energy::grid_recalc::EnergyUpdateDirty;
use crate::systems::jobs::exact_task_cleanup::prepare_owner_task_terminals;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TileSnapshot {
    entity: Entity,
    grid: (i32, i32),
}

struct PreparedCancellation {
    center: Vec2,
    refunded_bones: u32,
    owner_snapshot: WorldMapOwnerSnapshot,
    tiles: Vec<TileSnapshot>,
    removed_owners: Vec<Entity>,
    transport_requests: Vec<Entity>,
    terminal_requests: Vec<ExactTaskTerminalRequest>,
    visual_entities: Vec<Entity>,
}

/// Drains typed requests and resolves each site as one exclusive transaction.
pub fn soul_spa_construction_cancellation_system(world: &mut World) {
    let mut requests = world
        .resource_mut::<Messages<SoulSpaConstructionCancelRequest>>()
        .drain()
        .collect::<Vec<_>>();
    requests.sort_unstable_by_key(|request| request.target.to_bits());

    for request in requests {
        let result = match prepare_cancellation(world, request.target) {
            Ok(prepared) => apply_cancellation(world, request.target, prepared),
            Err(result) => result,
        };
        world
            .resource_mut::<Messages<SoulSpaConstructionCancelOutcome>>()
            .write(SoulSpaConstructionCancelOutcome {
                target: request.target,
                result,
            });
    }
    world.flush();
}

fn prepare_cancellation(
    world: &mut World,
    target: Entity,
) -> Result<PreparedCancellation, SoulSpaConstructionCancelResult> {
    let Some(building) = world.get::<Building>(target) else {
        return Err(if world.get_entity(target).is_ok() {
            SoulSpaConstructionCancelResult::OwnerMismatch
        } else {
            SoulSpaConstructionCancelResult::StaleTarget
        });
    };
    let Some(site) = world.get::<SoulSpaSite>(target) else {
        return Err(SoulSpaConstructionCancelResult::OwnerMismatch);
    };
    if site.phase != SoulSpaPhase::Constructing {
        return Err(SoulSpaConstructionCancelResult::PhaseUnavailable);
    }
    if building.kind != BuildingType::SoulSpa
        || building.is_provisional
        || site.bones_delivered > site.bones_required
        || world.get::<PowerGenerator>(target).is_none()
        || world.get::<PowerConsumer>(target).is_some()
        || world.get::<ConsumesFrom>(target).is_some()
        || world.get::<DeconstructionPending>(target).is_some()
    {
        return Err(SoulSpaConstructionCancelResult::OwnerMismatch);
    }
    let Some(center) = world
        .get::<Transform>(target)
        .map(|transform| transform.translation.truncate())
    else {
        return Err(SoulSpaConstructionCancelResult::OwnerMismatch);
    };
    if site.bones_delivered > 0 && world.get_resource::<ResourceItemVisualHandles>().is_none() {
        return Err(SoulSpaConstructionCancelResult::OwnerMismatch);
    }
    let refunded_bones = site.bones_delivered;

    let owner_snapshot = world
        .get_resource::<WorldMap>()
        .map(|map| map.snapshot_owner(target))
        .ok_or(SoulSpaConstructionCancelResult::OwnerMismatch)?;
    let anchor = WorldMap::world_to_grid(center);
    if !exact_rectangle(&owner_snapshot.building_grids, 2, 2)
        || !owner_snapshot.building_grids.contains(&anchor)
        || !owner_snapshot.floor_grids.is_empty()
        || !owner_snapshot.door_grids.is_empty()
        || !owner_snapshot.bridge_grids.is_empty()
        || !owner_snapshot.stockpile_grids.is_empty()
    {
        return Err(SoulSpaConstructionCancelResult::OwnerMismatch);
    }

    let tiles = prepare_tiles(world, target, &owner_snapshot)?;
    if !power_topology_matches(world, target) {
        return Err(SoulSpaConstructionCancelResult::OwnerMismatch);
    }

    let mut removed_owners = Vec::with_capacity(1 + tiles.len());
    removed_owners.push(target);
    removed_owners.extend(tiles.iter().map(|tile| tile.entity));
    removed_owners.sort_unstable_by_key(|entity| entity.to_bits());
    removed_owners.dedup();
    let transport_requests = transport_requests_referencing_removed_owners(world, &removed_owners);
    if transport_requests
        .iter()
        .any(|request| removed_owners.contains(request))
    {
        return Err(SoulSpaConstructionCancelResult::OwnerMismatch);
    }
    let mut cleanup_references = removed_owners.clone();
    cleanup_references.extend(transport_requests.iter().copied());
    cleanup_references.sort_unstable_by_key(|entity| entity.to_bits());
    cleanup_references.dedup();
    let terminal_requests = prepare_owner_task_terminals(world, &cleanup_references, None, &[])
        .map_err(|()| SoulSpaConstructionCancelResult::ActiveTaskMismatch)?;

    Ok(PreparedCancellation {
        center,
        refunded_bones,
        owner_snapshot,
        tiles,
        removed_owners,
        transport_requests,
        terminal_requests,
        visual_entities: building_visual_entities(world, target),
    })
}

fn apply_cancellation(
    world: &mut World,
    target: Entity,
    prepared: PreparedCancellation,
) -> SoulSpaConstructionCancelResult {
    let terminal_outcomes = terminalize_exact_tasks(world, &prepared.terminal_requests);
    if terminal_outcomes
        .iter()
        .any(|outcome| outcome.result != ExactTaskTerminalResult::Applied)
    {
        return SoulSpaConstructionCancelResult::ActiveTaskMismatch;
    }

    let transport_cleanup = close_transport_requests_for_removed_owners(
        world,
        &prepared.removed_owners,
        &prepared.transport_requests,
    );
    assert_eq!(
        transport_cleanup,
        OwnerTransportCleanupResult::Applied,
        "prevalidated Soul Spa request cleanup changed during exclusive apply"
    );
    if let Some(mut cache) = world.get_resource_mut::<SharedResourceCache>() {
        for &owner in &prepared.removed_owners {
            cache.clear_owner_reservations(owner);
        }
    }

    {
        let mut map = world.resource_mut::<WorldMap>();
        for grid in &prepared.owner_snapshot.building_grids {
            let cleared = map.clear_building_if_owned(*grid, target);
            debug_assert!(
                cleared,
                "validated Soul Spa owner changed during exclusive apply"
            );
        }
    }
    if let Some(mut rooms) = world.get_resource_mut::<RoomDetectionState>() {
        rooms.mark_dirty_many(prepared.owner_snapshot.building_grids.iter().copied());
    }

    for visual in prepared.visual_entities {
        if let Ok(entity) = world.get_entity_mut(visual) {
            entity.despawn();
        }
    }
    for tile in prepared.tiles {
        if let Ok(entity) = world.get_entity_mut(tile.entity) {
            entity.despawn();
        }
    }
    if let Ok(entity) = world.get_entity_mut(target) {
        entity.despawn();
    }
    if let Some(mut dirty) = world.get_resource_mut::<EnergyUpdateDirty>() {
        dirty.request_full_rebuild();
    }

    if prepared.refunded_bones > 0 {
        let mut queue = CommandQueue::default();
        {
            let handles = world.resource::<ResourceItemVisualHandles>();
            let mut commands = Commands::new(&mut queue, world);
            spawn_refund_items(
                &mut commands,
                handles,
                prepared.center,
                ResourceType::Bone,
                prepared.refunded_bones,
            );
        }
        queue.apply(world);
    }

    SoulSpaConstructionCancelResult::Canceled {
        refunded_bones: prepared.refunded_bones,
    }
}

fn prepare_tiles(
    world: &mut World,
    target: Entity,
    owner_snapshot: &WorldMapOwnerSnapshot,
) -> Result<Vec<TileSnapshot>, SoulSpaConstructionCancelResult> {
    let mut query = world.query::<(
        Entity,
        &SoulSpaTile,
        Option<&ChildOf>,
        Option<&Designation>,
        Option<&TaskSlots>,
    )>();
    let mut snapshots = Vec::new();
    for (entity, tile, parent, designation, slots) in query.iter(world) {
        let child_of_target = parent.is_some_and(|parent| parent.parent() == target);
        if tile.parent_site != target && !child_of_target {
            continue;
        }
        if tile.parent_site != target
            || parent.is_some_and(|parent| parent.parent() != target)
            || designation.is_some()
            || slots.is_some()
        {
            return Err(SoulSpaConstructionCancelResult::OwnerMismatch);
        }
        snapshots.push(TileSnapshot {
            entity,
            grid: tile.grid_pos,
        });
    }
    snapshots.sort_unstable_by_key(|snapshot| snapshot.entity.to_bits());
    if snapshots.len() != 4 {
        return Err(SoulSpaConstructionCancelResult::OwnerMismatch);
    }
    let mut grids = snapshots.iter().map(|tile| tile.grid).collect::<Vec<_>>();
    grids.sort_unstable_by_key(|&(x, y)| (y, x));
    grids.dedup();
    if grids != owner_snapshot.building_grids {
        return Err(SoulSpaConstructionCancelResult::OwnerMismatch);
    }
    Ok(snapshots)
}

fn power_topology_matches(world: &mut World, target: Entity) -> bool {
    let source = world.get::<GeneratesFor>(target).map(|relation| relation.0);
    let mut query = world.query::<(Entity, &GridGenerators)>();
    let mut reverse_sources = query
        .iter(world)
        .filter_map(|(grid, generators)| {
            generators
                .iter()
                .any(|generator| *generator == target)
                .then_some(grid)
        })
        .collect::<Vec<_>>();
    reverse_sources.sort_unstable_by_key(|entity| entity.to_bits());
    reverse_sources == source.into_iter().collect::<Vec<_>>()
}

fn building_visual_entities(world: &mut World, target: Entity) -> Vec<Entity> {
    let mut query = world.query::<(Entity, &Building3dVisual)>();
    let mut visuals = query
        .iter(world)
        .filter_map(|(entity, visual)| (visual.owner == target).then_some(entity))
        .collect::<Vec<_>>();
    visuals.sort_unstable_by_key(|entity| entity.to_bits());
    visuals
}

fn exact_rectangle(grids: &[(i32, i32)], width: i32, height: i32) -> bool {
    if grids.len() != (width * height) as usize {
        return false;
    }
    let Some(min_x) = grids.iter().map(|grid| grid.0).min() else {
        return false;
    };
    let Some(min_y) = grids.iter().map(|grid| grid.1).min() else {
        return false;
    };
    (min_y..min_y + height)
        .flat_map(|y| (min_x..min_x + width).map(move |x| (x, y)))
        .all(|grid| {
            grids
                .binary_search_by_key(&(grid.1, grid.0), |&(x, y)| (y, x))
                .is_ok()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::{
        OnTaskAbandoned, ResourceReservationRequest, TaskCompletedVisualMessage,
    };
    use hw_core::relationships::WorkingOn;
    use hw_core::soul::{DamnedSoul, Path};
    use hw_jobs::{
        ActiveTaskIdentity, AssignedTask, HaulToBlueprintData, HaulToBpPhase, TargetSoulSpaSite,
        WorkType,
    };
    use hw_logistics::transport_request::{
        TransportPriority, TransportRequest, TransportRequestKind,
    };
    use hw_logistics::{Inventory, ResourceItem};

    #[derive(Clone)]
    struct SpaFixture {
        target: Entity,
        tiles: Vec<Entity>,
        visual: Entity,
        grids: Vec<(i32, i32)>,
        center: Vec2,
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<WorldMap>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<RoomDetectionState>()
            .init_resource::<EnergyUpdateDirty>()
            .insert_resource(ResourceItemVisualHandles {
                icon_bone_small: default(),
                icon_wood_small: default(),
                icon_rock_small: default(),
                icon_sand_small: default(),
                icon_stasis_mud_small: default(),
            })
            .add_message::<ResourceReservationRequest>()
            .add_message::<OnTaskAbandoned>()
            .add_message::<TaskCompletedVisualMessage>()
            .add_message::<SoulSpaConstructionCancelRequest>()
            .add_message::<SoulSpaConstructionCancelOutcome>()
            .add_systems(Update, soul_spa_construction_cancellation_system);
        app
    }

    fn spawn_site(app: &mut App, delivered: u32, phase: SoulSpaPhase) -> SpaFixture {
        let lower_left = (10, 11);
        let grids = vec![
            lower_left,
            (lower_left.0 + 1, lower_left.1),
            (lower_left.0, lower_left.1 + 1),
            (lower_left.0 + 1, lower_left.1 + 1),
        ];
        let center = WorldMap::grid_to_world(lower_left.0, lower_left.1)
            + Vec2::splat(hw_core::constants::TILE_SIZE * 0.5);
        let target = app
            .world_mut()
            .spawn((
                Building {
                    kind: BuildingType::SoulSpa,
                    is_provisional: false,
                },
                SoulSpaSite {
                    phase,
                    bones_required: 12,
                    bones_delivered: delivered,
                    active_slots: 4,
                },
                Transform::from_translation(center.extend(0.0)),
            ))
            .id();
        let tiles = grids
            .iter()
            .copied()
            .map(|grid| {
                app.world_mut()
                    .spawn((
                        SoulSpaTile {
                            parent_site: target,
                            grid_pos: grid,
                        },
                        Transform::from_translation(
                            WorldMap::grid_to_world(grid.0, grid.1).extend(0.0),
                        ),
                    ))
                    .id()
            })
            .collect::<Vec<_>>();
        let visual = app
            .world_mut()
            .spawn(Building3dVisual { owner: target })
            .id();
        for &grid in &grids {
            app.world_mut()
                .resource_mut::<WorldMap>()
                .set_building(grid, target);
        }
        SpaFixture {
            target,
            tiles,
            visual,
            grids,
            center,
        }
    }

    fn take_outcomes(app: &mut App) -> Vec<SoulSpaConstructionCancelOutcome> {
        app.world_mut()
            .resource_mut::<Messages<SoulSpaConstructionCancelOutcome>>()
            .drain()
            .collect()
    }

    fn bone_count(app: &mut App) -> usize {
        let mut query = app.world_mut().query::<&ResourceItem>();
        query
            .iter(app.world())
            .filter(|item| item.0 == ResourceType::Bone)
            .count()
    }

    #[test]
    fn cancellation_refunds_exact_delivered_bones_and_is_not_repeatable() {
        for delivered in [0, 7, 12] {
            let mut app = app();
            let fixture = spawn_site(&mut app, delivered, SoulSpaPhase::Constructing);
            let existing_bone = app
                .world_mut()
                .spawn((
                    ResourceItem(ResourceType::Bone),
                    Transform::from_translation(fixture.center.extend(0.0)),
                ))
                .id();
            for _ in 0..2 {
                app.world_mut()
                    .write_message(SoulSpaConstructionCancelRequest {
                        target: fixture.target,
                    });
            }

            app.update();

            assert!(app.world().get_entity(fixture.target).is_err());
            assert!(app.world().get_entity(fixture.visual).is_err());
            assert!(app.world().get_entity(existing_bone).is_ok());
            for tile in &fixture.tiles {
                assert!(app.world().get_entity(*tile).is_err());
            }
            for grid in &fixture.grids {
                assert_eq!(
                    app.world().resource::<WorldMap>().building_entity(*grid),
                    None
                );
            }
            assert_eq!(bone_count(&mut app), delivered as usize + 1);
            assert_eq!(
                take_outcomes(&mut app),
                vec![
                    SoulSpaConstructionCancelOutcome {
                        target: fixture.target,
                        result: SoulSpaConstructionCancelResult::Canceled {
                            refunded_bones: delivered,
                        },
                    },
                    SoulSpaConstructionCancelOutcome {
                        target: fixture.target,
                        result: SoulSpaConstructionCancelResult::StaleTarget,
                    },
                ]
            );
            let dirty = app.world().resource::<EnergyUpdateDirty>();
            assert!(dirty.topology_reconcile_due);
            assert!(dirty.power_output_due);
            assert!(dirty.grid_recalc_due);
        }
    }

    #[test]
    fn cancellation_terminalizes_request_worker_before_removing_the_site() {
        let mut app = app();
        let fixture = spawn_site(&mut app, 3, SoulSpaPhase::Constructing);
        let request = app
            .world_mut()
            .spawn((
                TransportRequest {
                    kind: TransportRequestKind::DeliverToSoulSpa,
                    anchor: fixture.target,
                    resource_type: ResourceType::Bone,
                    issued_by: fixture.target,
                    priority: TransportPriority::Normal,
                    stockpile_group: vec![],
                },
                TargetSoulSpaSite(fixture.target),
                Designation {
                    work_type: WorkType::Haul,
                },
                TaskSlots::new(1),
            ))
            .id();
        let item = app.world_mut().spawn(ResourceItem(ResourceType::Bone)).id();
        let identity = ActiveTaskIdentity::new(request, request, WorkType::Haul);
        let worker = app
            .world_mut()
            .spawn((
                Transform::default(),
                DamnedSoul::default(),
                AssignedTask::HaulToBlueprint(HaulToBlueprintData {
                    item,
                    blueprint: fixture.target,
                    phase: HaulToBpPhase::GoingToItem,
                }),
                Path::default(),
                Inventory::default(),
                identity,
                WorkingOn(request),
            ))
            .id();
        app.world_mut().flush();
        app.world_mut()
            .write_message(SoulSpaConstructionCancelRequest {
                target: fixture.target,
            });

        app.update();

        assert!(app.world().get_entity(request).is_err());
        assert!(matches!(
            app.world().get::<AssignedTask>(worker),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<WorkingOn>(worker).is_none());
        assert!(app.world().get::<ActiveTaskIdentity>(worker).is_none());
        assert!(app.world().get_entity(item).is_ok());
        assert_eq!(
            take_outcomes(&mut app)[0].result,
            SoulSpaConstructionCancelResult::Canceled { refunded_bones: 3 }
        );
    }

    #[test]
    fn operational_or_owner_mismatched_sites_are_unchanged() {
        let mut operational_app = app();
        let operational = spawn_site(&mut operational_app, 12, SoulSpaPhase::Operational);
        operational_app
            .world_mut()
            .write_message(SoulSpaConstructionCancelRequest {
                target: operational.target,
            });
        operational_app.update();
        assert!(
            operational_app
                .world()
                .get_entity(operational.target)
                .is_ok()
        );
        assert_eq!(
            take_outcomes(&mut operational_app)[0].result,
            SoulSpaConstructionCancelResult::PhaseUnavailable
        );
        assert_eq!(bone_count(&mut operational_app), 0);

        let mut mismatch_app = app();
        let mismatch = spawn_site(&mut mismatch_app, 5, SoulSpaPhase::Constructing);
        let replacement = mismatch_app.world_mut().spawn_empty().id();
        mismatch_app
            .world_mut()
            .resource_mut::<WorldMap>()
            .set_building(mismatch.grids[0], replacement);
        mismatch_app
            .world_mut()
            .write_message(SoulSpaConstructionCancelRequest {
                target: mismatch.target,
            });
        mismatch_app.update();
        assert!(mismatch_app.world().get_entity(mismatch.target).is_ok());
        assert_eq!(
            take_outcomes(&mut mismatch_app)[0].result,
            SoulSpaConstructionCancelResult::OwnerMismatch
        );
        assert_eq!(bone_count(&mut mismatch_app), 0);
    }
}
