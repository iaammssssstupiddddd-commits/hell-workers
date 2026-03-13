//! 建設完了イベントの Observer
//!
//! `bevy_app` の `building_completion_system` が `BuildingCompletedEvent` を発行した後、
//! この Observer が WorldMap の更新と ObstaclePosition の spawn を担当する。
//! Observer は hw_soul_ai に配置する（hw_world と hw_jobs 両方に依存できるため）。

use bevy::prelude::*;
use hw_core::soul::DamnedSoul;
use hw_jobs::events::BuildingCompletedEvent;
use hw_jobs::{BuildingType, ObstaclePosition};
use hw_world::{WorldMap, WorldMapWrite};

pub fn on_building_completed(
    trigger: On<BuildingCompletedEvent>,
    mut commands: Commands,
    mut world_map: WorldMapWrite,
    mut q_souls: Query<(&mut Transform, Entity), With<DamnedSoul>>,
) {
    let ev = trigger.event();
    let building_entity = ev.building_entity;
    let kind = ev.kind;
    let occupied_grids = &ev.occupied_grids;

    let is_obstacle = matches!(
        kind,
        BuildingType::Bridge
            | BuildingType::Door
            | BuildingType::Wall
            | BuildingType::Tank
            | BuildingType::MudMixer
            | BuildingType::RestArea
            | BuildingType::SandPile
            | BuildingType::BonePile
            | BuildingType::WheelbarrowParking
    );

    if !is_obstacle {
        return;
    }

    if kind != BuildingType::Bridge {
        commands.entity(building_entity).with_children(|parent| {
            for &(gx, gy) in occupied_grids {
                parent.spawn((ObstaclePosition(gx, gy), Name::new("Building Obstacle")));
            }
        });
    }

    world_map.register_completed_building_footprint(kind, building_entity, occupied_grids.iter().copied());

    for &(gx, gy) in occupied_grids {
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

                    if world_map.is_walkable(nx, ny) && !occupied_grids.contains(&(nx, ny)) {
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
