use crate::relationships::WorkingOn;
use crate::systems::jobs::{Building, BuildingType, Designation};
use crate::systems::soul_ai::execute::task_execution::{
    common::clear_task_and_path,
    common::{is_near_target_or_dest, update_destination_to_adjacent},
    context::TaskExecutionContext,
    types::{AssignedTask, MovePlantData, MovePlantPhase},
};
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;

#[derive(Component, Debug, Clone, Copy)]
pub struct MovePlanned {
    pub task_entity: Entity,
}

#[derive(Component, Debug, Clone)]
pub struct PendingBuildingMove {
    pub old_occupied: Vec<(i32, i32)>,
    pub new_occupied: Vec<(i32, i32)>,
    pub companion_anchor: Option<(i32, i32)>,
}

#[derive(Component, Debug, Clone)]
pub struct MovePlantReservation {
    pub occupied: Vec<(i32, i32)>,
}

pub fn handle_move_plant_task(
    ctx: &mut TaskExecutionContext,
    data: MovePlantData,
    commands: &mut Commands,
    world_map: &WorldMap,
) {
    match data.phase {
        MovePlantPhase::GoToBuilding => {
            let Ok((building_transform, _, _)) = ctx.queries.storage.buildings.get(data.building)
            else {
                cleanup_move_task(ctx, commands, data.task_entity, data.building);
                return;
            };

            let building_pos = building_transform.translation.truncate();
            let soul_pos = ctx.soul_pos();
            let reachable = update_destination_to_adjacent(
                ctx.dest,
                building_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );

            if !reachable {
                cleanup_move_task(ctx, commands, data.task_entity, data.building);
                return;
            }

            if is_near_target_or_dest(soul_pos, building_pos, ctx.dest.0)
                || soul_pos.distance(building_pos) <= TILE_SIZE * 1.5
            {
                *ctx.task = AssignedTask::MovePlant(MovePlantData {
                    phase: MovePlantPhase::Moving,
                    ..data
                });
                ctx.dest.0 = soul_pos;
                ctx.path.waypoints.clear();
                ctx.path.current_index = 0;
            }
        }
        MovePlantPhase::Moving => {
            let Ok((building_transform, building, _)) =
                ctx.queries.storage.buildings.get(data.building)
            else {
                cleanup_move_task(ctx, commands, data.task_entity, data.building);
                return;
            };

            let old_anchor =
                anchor_grid_for_kind(building.kind, building_transform.translation.truncate());
            let new_anchor = data.destination_grid;
            let old_occupied = occupied_grids_for_kind(building.kind, old_anchor);
            let new_occupied = occupied_grids_for_kind(building.kind, new_anchor);

            let mut next_transform = *building_transform;
            let destination_pos = spawn_pos_for_kind(building.kind, new_anchor);
            next_transform.translation.x = destination_pos.x;
            next_transform.translation.y = destination_pos.y;

            commands.entity(data.building).insert((
                next_transform,
                PendingBuildingMove {
                    old_occupied,
                    new_occupied,
                    companion_anchor: data.companion_anchor,
                },
            ));

            *ctx.task = AssignedTask::MovePlant(MovePlantData {
                phase: MovePlantPhase::Done,
                ..data
            });
        }
        MovePlantPhase::Done => {
            commands.entity(data.task_entity).remove::<Designation>();
            commands.entity(data.task_entity).despawn();
            cleanup_move_task(ctx, commands, data.task_entity, data.building);
        }
    }
}

fn cleanup_move_task(
    ctx: &mut TaskExecutionContext,
    commands: &mut Commands,
    task_entity: Entity,
    building_entity: Entity,
) {
    commands.entity(task_entity).remove::<Designation>();
    commands.entity(task_entity).despawn();
    commands.entity(building_entity).remove::<MovePlanned>();
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
    clear_task_and_path(ctx.task, ctx.path);
}

pub fn apply_pending_building_move_system(
    mut commands: Commands,
    mut world_map: WorldMapWrite,
    q_pending: Query<(Entity, &PendingBuildingMove, Option<&Children>), With<Building>>,
    mut q_obstacles: Query<&mut crate::systems::jobs::ObstaclePosition>,
    mut q_stockpiles: Query<
        (
            Entity,
            &crate::systems::logistics::BelongsTo,
            &mut Transform,
        ),
        With<crate::systems::logistics::BucketStorage>,
    >,
    mut q_attached_entities: Query<
        (&crate::systems::logistics::BelongsTo, &mut Transform),
        (
            Without<crate::systems::logistics::BucketStorage>,
            Without<Building>,
        ),
    >,
) {
    for (building_entity, pending, children_opt) in q_pending.iter() {
        world_map.release_building_footprint_if_owned(
            building_entity,
            pending.old_occupied.iter().copied(),
        );
        world_map.set_building_occupancies(building_entity, pending.new_occupied.iter().copied());

        if let Some(children) = children_opt {
            let mut next_positions = pending.new_occupied.clone();
            next_positions.sort_unstable();
            let mut index = 0usize;
            for child in children {
                if let Ok(mut pos) = q_obstacles.get_mut(*child) {
                    if let Some((gx, gy)) = next_positions.get(index) {
                        pos.0 = *gx;
                        pos.1 = *gy;
                        index += 1;
                    }
                }
            }
        }

        if let Some(delta) = anchor_delta(pending) {
            relocate_bucket_storages_for_tank(
                building_entity,
                pending,
                delta,
                &mut world_map,
                &mut q_stockpiles,
            );
            relocate_attached_entities_for_building(
                building_entity,
                delta,
                &mut q_attached_entities,
            );
        }

        commands
            .entity(building_entity)
            .remove::<(PendingBuildingMove, MovePlanned)>();
    }
}

fn anchor_delta(pending: &PendingBuildingMove) -> Option<(i32, i32)> {
    let old_anchor = pending.old_occupied.iter().min().copied()?;
    let new_anchor = pending.new_occupied.iter().min().copied()?;
    Some((new_anchor.0 - old_anchor.0, new_anchor.1 - old_anchor.1))
}

fn relocate_bucket_storages_for_tank(
    building_entity: Entity,
    pending: &PendingBuildingMove,
    delta: (i32, i32),
    world_map: &mut WorldMap,
    q_stockpiles: &mut Query<
        (
            Entity,
            &crate::systems::logistics::BelongsTo,
            &mut Transform,
        ),
        With<crate::systems::logistics::BucketStorage>,
    >,
) {
    let mut stockpiles: Vec<(Entity, (i32, i32))> = q_stockpiles
        .iter()
        .filter_map(|(entity, belongs_to, _)| {
            (belongs_to.0 == building_entity)
                .then_some(entity)
                .and_then(|entity| {
                    world_map
                        .stockpile_entries()
                        .find_map(|(grid, e)| (*e == entity).then_some((entity, *grid)))
                })
        })
        .collect();
    stockpiles.sort_by_key(|(_, grid)| *grid);
    if stockpiles.is_empty() {
        return;
    }

    let requested_grids = pending
        .companion_anchor
        .map(|anchor| vec![anchor, (anchor.0 + 1, anchor.1)])
        .unwrap_or_default();

    for (index, (stockpile_entity, old_grid)) in stockpiles.iter().enumerate() {
        let new_grid = requested_grids
            .get(index)
            .copied()
            .unwrap_or((old_grid.0 + delta.0, old_grid.1 + delta.1));

        world_map.move_stockpile_tile(*stockpile_entity, *old_grid, new_grid);

        if let Ok((_, _, mut transform)) = q_stockpiles.get_mut(*stockpile_entity) {
            let pos = WorldMap::grid_to_world(new_grid.0, new_grid.1);
            transform.translation.x = pos.x;
            transform.translation.y = pos.y;
        }
    }
}

fn relocate_attached_entities_for_building(
    building_entity: Entity,
    delta: (i32, i32),
    q_attached_entities: &mut Query<
        (&crate::systems::logistics::BelongsTo, &mut Transform),
        (
            Without<crate::systems::logistics::BucketStorage>,
            Without<Building>,
        ),
    >,
) {
    let offset = Vec2::new(delta.0 as f32 * TILE_SIZE, delta.1 as f32 * TILE_SIZE);
    for (belongs_to, mut transform) in q_attached_entities.iter_mut() {
        if belongs_to.0 != building_entity {
            continue;
        }
        transform.translation.x += offset.x;
        transform.translation.y += offset.y;
    }
}

fn is_two_by_two(kind: BuildingType) -> bool {
    matches!(
        kind,
        BuildingType::Tank
            | BuildingType::MudMixer
            | BuildingType::RestArea
            | BuildingType::WheelbarrowParking
    )
}

fn anchor_grid_for_kind(kind: BuildingType, world_pos: Vec2) -> (i32, i32) {
    if is_two_by_two(kind) {
        WorldMap::world_to_grid(world_pos - Vec2::splat(TILE_SIZE * 0.5))
    } else {
        WorldMap::world_to_grid(world_pos)
    }
}

fn spawn_pos_for_kind(kind: BuildingType, anchor_grid: (i32, i32)) -> Vec2 {
    let base = WorldMap::grid_to_world(anchor_grid.0, anchor_grid.1);
    if is_two_by_two(kind) {
        base + Vec2::splat(TILE_SIZE * 0.5)
    } else {
        base
    }
}

fn occupied_grids_for_kind(kind: BuildingType, anchor_grid: (i32, i32)) -> Vec<(i32, i32)> {
    if is_two_by_two(kind) {
        vec![
            anchor_grid,
            (anchor_grid.0 + 1, anchor_grid.1),
            (anchor_grid.0, anchor_grid.1 + 1),
            (anchor_grid.0 + 1, anchor_grid.1 + 1),
        ]
    } else {
        vec![anchor_grid]
    }
}
