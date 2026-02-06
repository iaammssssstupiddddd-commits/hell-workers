use super::super::{Blueprint, BuildingType, ObstaclePosition};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub(super) fn update_world_for_completed_building(
    commands: &mut Commands,
    building_entity: Entity,
    bp: &Blueprint,
    world_map: &mut WorldMap,
    q_souls: &mut Query<
        (&mut Transform, Entity),
        (
            With<crate::entities::damned_soul::DamnedSoul>,
            Without<super::super::Blueprint>,
        ),
    >,
) {
    let is_obstacle = matches!(
        bp.kind,
        BuildingType::Wall | BuildingType::Tank | BuildingType::MudMixer
    );

    if !is_obstacle {
        return;
    }

    commands.entity(building_entity).with_children(|parent| {
        for &(gx, gy) in &bp.occupied_grids {
            parent.spawn((ObstaclePosition(gx, gy), Name::new("Building Obstacle")));
        }
    });

    for &(gx, gy) in &bp.occupied_grids {
        world_map.add_obstacle(gx, gy);
        world_map.buildings.insert((gx, gy), building_entity);
    }

    for &(gx, gy) in &bp.occupied_grids {
        for (mut soul_transform, soul_entity) in q_souls.iter_mut() {
            let soul_pos = soul_transform.translation.truncate();
            let (sgx, sgy) = WorldMap::world_to_grid(soul_pos);

            if sgx == gx && sgy == gy {
                let directions = [
                    (0, 1),
                    (0, -1),
                    (1, 0),
                    (-1, 0),
                    (1, 1),
                    (1, -1),
                    (-1, 1),
                    (-1, -1),
                ];

                let mut found = false;
                for (dx, dy) in directions {
                    let nx = gx + dx;
                    let ny = gy + dy;

                    if world_map.is_walkable(nx, ny) && !bp.occupied_grids.contains(&(nx, ny)) {
                        let new_pos = WorldMap::grid_to_world(nx, ny);
                        soul_transform.translation.x = new_pos.x;
                        soul_transform.translation.y = new_pos.y;
                        info!(
                            "BUILD: Soul {:?} was pushed out to ({}, {})",
                            soul_entity, nx, ny
                        );
                        found = true;
                        break;
                    }
                }

                if !found {
                    warn!(
                        "BUILD: Soul {:?} is stuck and could not find simple push-out position!",
                        soul_entity
                    );
                }
            }
        }
    }
}
