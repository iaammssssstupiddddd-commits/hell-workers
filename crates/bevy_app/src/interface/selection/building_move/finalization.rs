use crate::systems::jobs::{Building, BuildingType, Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::TransportRequest;
use crate::systems::soul_ai::execute::task_execution::context::TaskUnassignQueries;
use crate::systems::soul_ai::execute::task_execution::move_plant::{
    MovePlanned, MovePlantReservation,
};
use crate::systems::soul_ai::execute::task_execution::types::{AssignedTask, MovePlantTask};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_soul_ai::unassign_task;
use hw_ui::selection::{move_occupied_grids, move_spawn_pos};

use super::context::{MoveOpCtx, SoulTaskQuery};

pub(super) fn finalize_move_request(
    op: &mut MoveOpCtx<'_, '_, '_, '_, '_, '_>,
    target_entity: Entity,
    building: &Building,
    transform: &Transform,
    destination_grid: (i32, i32),
    companion_anchor: Option<(i32, i32)>,
) {
    cancel_tasks_and_requests_for_moved_building(
        op.commands,
        target_entity,
        op.q_transport_requests,
        op.q_souls,
        op.task_queries,
        &*op.world_map,
    );

    let destination_pos = move_spawn_pos(building.kind, destination_grid);
    let (texture, size) = match building.kind {
        BuildingType::Tank => (
            op.game_assets.tank_empty.clone(),
            Vec2::splat(TILE_SIZE * 2.0),
        ),
        BuildingType::MudMixer => (
            op.game_assets.mud_mixer.clone(),
            Vec2::splat(TILE_SIZE * 2.0),
        ),
        _ => return,
    };
    let destination_occupied = move_occupied_grids(building.kind, destination_grid);
    let companion_occupied = companion_anchor
        .map(|anchor| vec![anchor, (anchor.0 + 1, anchor.1)])
        .unwrap_or_default();
    let mut reserved_occupied = destination_occupied.clone();
    reserved_occupied.extend(companion_occupied.iter().copied());
    let task_entity = op.commands.spawn_empty().id();
    op.world_map
        .add_grid_obstacles(reserved_occupied.iter().copied());

    op.commands.entity(task_entity).with_children(|parent| {
        for &(gx, gy) in &reserved_occupied {
            parent.spawn((
                crate::systems::jobs::ObstaclePosition(gx, gy),
                Name::new("Move Reservation Obstacle"),
            ));
        }
    });
    op.commands.entity(task_entity).insert((
        Designation {
            work_type: WorkType::Move,
        },
        TaskSlots::new(1),
        Priority(10),
        MovePlantReservation {
            occupied: reserved_occupied,
        },
        MovePlantTask {
            building: target_entity,
            destination_grid,
            destination_pos,
            companion_anchor,
        },
        Sprite {
            image: texture,
            color: Color::srgba(1.0, 1.0, 1.0, 0.35),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(
            destination_pos.x,
            destination_pos.y,
            transform.translation.z,
        ),
        Name::new("Move Plant Task"),
    ));
    op.commands
        .entity(target_entity)
        .insert(MovePlanned { task_entity });
}

fn cancel_tasks_and_requests_for_moved_building(
    commands: &mut Commands,
    building_entity: Entity,
    q_transport_requests: &Query<(Entity, &TransportRequest)>,
    q_souls: &mut SoulTaskQuery,
    task_queries: &mut TaskUnassignQueries,
    world_map: &WorldMap,
) {
    commands.entity(building_entity).remove::<(
        Designation,
        TaskSlots,
        Priority,
        hw_core::relationships::ManagedBy,
    )>();

    for (request_entity, request) in q_transport_requests.iter() {
        if request.anchor == building_entity {
            commands.entity(request_entity).despawn();
        }
    }

    for (soul_entity, transform, mut task, mut path, mut inventory) in q_souls.iter_mut() {
        if task_targets_building(&task, building_entity) {
            unassign_task(
                commands,
                hw_soul_ai::SoulDropCtx {
                    soul_entity,
                    drop_pos: transform.translation.truncate(),
                    inventory: inventory.as_deref_mut(),
                    dropped_item_res: None,
                },
                &mut task,
                &mut path,
                task_queries,
                world_map,
                false,
            );
        }
    }
}

fn task_targets_building(task: &AssignedTask, building_entity: Entity) -> bool {
    if let Some(data) = task.bucket_transport_data() {
        if matches!(
            data.source,
            crate::systems::soul_ai::execute::task_execution::types::BucketTransportSource::Tank {
                tank,
                ..
            } if tank == building_entity
        ) {
            return true;
        }

        match data.destination {
            crate::systems::soul_ai::execute::task_execution::types::BucketTransportDestination::Tank(tank) => {
                if tank == building_entity {
                    return true;
                }
            }
            crate::systems::soul_ai::execute::task_execution::types::BucketTransportDestination::Mixer(mixer) => {
                if mixer == building_entity {
                    return true;
                }
            }
        }
    }

    match task {
        AssignedTask::Refine(data) => data.mixer == building_entity,
        AssignedTask::HaulToMixer(data) => data.mixer == building_entity,
        AssignedTask::MovePlant(data) => data.building == building_entity,
        _ => false,
    }
}
