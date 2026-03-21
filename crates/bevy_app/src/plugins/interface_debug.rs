use crate::entities::damned_soul::DamnedSoulSpawnEvent;
use crate::entities::familiar::{FamiliarSpawnEvent, FamiliarType};
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::wall_construction::components::{
    WallConstructionPhase, WallConstructionSite, WallTileBlueprint, WallTileState,
};
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
use crate::systems::visual::wall_orientation_aid::attach_wall_orientation_aid;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, Z_MAP};
use hw_ui::camera::MainCamera;
use hw_visual::visual3d::Building3dVisual;

pub fn debug_spawn_system(
    buttons: Res<ButtonInput<KeyCode>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut soul_spawn_events: MessageWriter<DamnedSoulSpawnEvent>,
    mut familiar_spawn_events: MessageWriter<FamiliarSpawnEvent>,
) {
    let mut spawn_pos = Vec2::ZERO;

    if let Ok(window) = q_window.single()
        && let Some(cursor_pos) = window.cursor_position()
        && let Ok((camera, camera_transform)) = q_camera.single()
        && let Ok(pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos)
    {
        spawn_pos = pos;
    }

    if buttons.just_pressed(KeyCode::KeyP) {
        soul_spawn_events.write(DamnedSoulSpawnEvent {
            position: spawn_pos,
        });
    }

    if buttons.just_pressed(KeyCode::KeyO) {
        familiar_spawn_events.write(FamiliarSpawnEvent {
            position: spawn_pos,
            familiar_type: FamiliarType::Imp,
        });
    }
}

/// IBuild トグルが ON の間、WallConstructionSite を即時完成させる。
///
/// wall_framed_tile_spawn_system の直前に実行される。全タイルを Complete にして
/// phase を Coating に強制移行することで、同フレーム内の completion_system が cleanup する。
pub fn debug_instant_complete_walls_system(
    debug: Res<crate::DebugInstantBuild>,
    mut q_sites: Query<(Entity, &mut WallConstructionSite)>,
    mut q_tiles: Query<(Entity, &mut WallTileBlueprint)>,
    mut q_buildings: Query<&mut Building>,
    handles_3d: Res<Building3dHandles>,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
) {
    if !debug.0 {
        return;
    }

    for (site_entity, mut site) in q_sites.iter_mut() {
        let mut framed_count = 0u32;

        for (_, mut tile) in q_tiles
            .iter_mut()
            .filter(|(_, t)| t.parent_site == site_entity)
        {
            if tile.spawned_wall.is_none() {
                // 未 spawn（フレーミング前） → 完成済み Building + 3D visual を直接 spawn
                let world_pos = WorldMap::grid_to_world(tile.grid_pos.0, tile.grid_pos.1);
                let wall_entity = commands
                    .spawn((
                        Building {
                            kind: BuildingType::Wall,
                            is_provisional: false,
                        },
                        Transform::from_translation(world_pos.extend(Z_MAP + 0.01)),
                        Visibility::default(),
                        Name::new("Building (Wall)"),
                    ))
                    .id();
                let visual_entity = commands
                    .spawn((
                        Mesh3d(handles_3d.wall_mesh.clone()),
                        MeshMaterial3d(handles_3d.wall_material.clone()),
                        Transform::from_xyz(world_pos.x, TILE_SIZE / 2.0, -world_pos.y),
                        handles_3d.render_layers.clone(),
                        Building3dVisual { owner: wall_entity },
                        Name::new("Building3dVisual (Wall)"),
                    ))
                    .id();
                attach_wall_orientation_aid(&mut commands, visual_entity, &handles_3d);
                world_map.reserve_building_footprint(
                    BuildingType::Wall,
                    wall_entity,
                    std::iter::once(tile.grid_pos),
                );
                tile.spawned_wall = Some(wall_entity);
            } else if let Some(wall_entity) = tile.spawned_wall {
                // 既存 provisional 壁 → 完成済みに昇格
                if let Ok(mut building) = q_buildings.get_mut(wall_entity) {
                    building.is_provisional = false;
                }
                commands.entity(wall_entity).remove::<ProvisionalWall>();
            }

            tile.state = WallTileState::Complete;
            framed_count += 1;
        }

        // カウンタを揃えて completion_system のログを正確にする
        site.tiles_framed = framed_count;
        site.tiles_coated = framed_count;
        // Coating フェーズに強制移行 → wall_construction_completion_system が同フレームで cleanup
        site.phase = WallConstructionPhase::Coating;
    }
}
