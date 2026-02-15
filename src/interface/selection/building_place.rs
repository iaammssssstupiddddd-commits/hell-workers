use crate::assets::GameAssets;
use crate::constants::*;
use crate::game_state::{
    BuildContext, CompanionParentKind, CompanionPlacement, CompanionPlacementKind,
    CompanionPlacementState,
};
use crate::interface::camera::MainCamera;
use crate::interface::ui::UiInputState;
use crate::systems::jobs::{Blueprint, BuildingType, SandPile};
use crate::world::map::WorldMap;
use bevy::prelude::*;

const COMPANION_PLACEMENT_RADIUS_TILES: f32 = 5.0;
const MUD_MIXER_NEARBY_SANDPILE_TILES: i32 = 3;
const TANK_NEARBY_BUCKET_STORAGE_TILES: i32 = 3;

pub fn blueprint_placement(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut world_map: ResMut<WorldMap>,
    build_context: Res<BuildContext>,
    mut companion_state: ResMut<CompanionPlacementState>,
    q_blueprints: Query<(Entity, &Blueprint, &Transform)>,
    q_sand_piles: Query<&Transform, With<SandPile>>,
    game_assets: Res<GameAssets>,
    mut commands: Commands,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };
    let Ok(window) = q_window.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };
    let grid = WorldMap::world_to_grid(world_pos);

    // companion 配置中は通常建築を抑止
    if let Some(active) = companion_state.0.clone() {
        let dist = world_pos.distance(active.center);
        if dist > active.radius {
            info!(
                "COMPANION: Placement out of range ({:.1} > {:.1})",
                dist, active.radius
            );
            return;
        }

        match active.kind {
            CompanionPlacementKind::BucketStorage => {
                let parent_type = parent_building_type(active.parent_kind);
                let parent_occupied_grids =
                    occupied_grids_for_building(parent_type, active.parent_anchor);

                let Some((parent_blueprint, _, _)) = place_building_blueprint(
                    &mut commands,
                    &mut world_map,
                    &game_assets,
                    parent_type,
                    active.parent_anchor,
                ) else {
                    warn!(
                        "COMPANION: failed to confirm parent blueprint before bucket storage placement"
                    );
                    return;
                };
                if try_place_bucket_storage_companion(
                    &mut commands,
                    &mut world_map,
                    parent_blueprint,
                    &parent_occupied_grids,
                    grid,
                ) {
                    companion_state.0 = None;
                    info!("COMPANION: Bucket storage placed for tank blueprint");
                } else {
                    // 親Blueprintの確定に成功したが companion が置けない場合は巻き戻す
                    for &(gx, gy) in &parent_occupied_grids {
                        world_map.buildings.remove(&(gx, gy));
                        world_map.remove_obstacle(gx, gy);
                    }
                    commands.entity(parent_blueprint).despawn();
                }
            }
            CompanionPlacementKind::SandPile => {
                let parent_type = parent_building_type(active.parent_kind);
                let parent_occupied_grids =
                    occupied_grids_for_building(parent_type, active.parent_anchor);
                if parent_occupied_grids.contains(&grid) {
                    info!("COMPANION: SandPile cannot be placed on parent blueprint area");
                    return;
                }
                if place_building_blueprint(
                    &mut commands,
                    &mut world_map,
                    &game_assets,
                    BuildingType::SandPile,
                    grid,
                )
                .is_some()
                {
                    if place_building_blueprint(
                        &mut commands,
                        &mut world_map,
                        &game_assets,
                        parent_type,
                        active.parent_anchor,
                    )
                    .is_some()
                    {
                        companion_state.0 = None;
                        info!("COMPANION: SandPile and parent blueprint confirmed");
                    } else {
                        warn!("COMPANION: SandPile placed but failed to confirm parent blueprint");
                    }
                }
            }
        }
        return;
    }

    let Some(building_type) = build_context.0 else {
        return;
    };
    let occupied_grids = occupied_grids_for_building(building_type, grid);
    let spawn_pos = building_spawn_pos(building_type, grid);

    if building_type == BuildingType::Tank {
        companion_state.0 = Some(make_companion_placement(
            CompanionParentKind::Tank,
            grid,
            CompanionPlacementKind::BucketStorage,
            spawn_pos,
        ));
        info!("COMPANION: Tank placed, waiting for bucket storage placement");
    } else if building_type == BuildingType::MudMixer
        && !has_nearby_sandpile(&occupied_grids, &q_sand_piles, &q_blueprints, None)
    {
        companion_state.0 = Some(make_companion_placement(
            CompanionParentKind::MudMixer,
            grid,
            CompanionPlacementKind::SandPile,
            spawn_pos,
        ));
        info!("COMPANION: MudMixer needs nearby SandPile, placement requested");
    } else {
        let _ = place_building_blueprint(
            &mut commands,
            &mut world_map,
            &game_assets,
            building_type,
            grid,
        );
    }
}

fn place_building_blueprint(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    game_assets: &GameAssets,
    building_type: BuildingType,
    grid: (i32, i32),
) -> Option<(Entity, Vec<(i32, i32)>, Vec2)> {
    let occupied_grids = occupied_grids_for_building(building_type, grid);
    let spawn_pos = building_spawn_pos(building_type, grid);
    let custom_size = Some(building_size(building_type));

    let can_place = occupied_grids.iter().all(|&g| {
        !world_map.buildings.contains_key(&g)
            && !world_map.stockpiles.contains_key(&g)
            && world_map.is_walkable(g.0, g.1)
    });
    if !can_place {
        return None;
    }

    let texture = match building_type {
        BuildingType::Wall => game_assets.wall_isolated.clone(),
        BuildingType::Floor => game_assets.stone.clone(),
        BuildingType::Tank => game_assets.tank_empty.clone(),
        BuildingType::MudMixer => game_assets.mud_mixer.clone(),
        BuildingType::SandPile => game_assets.sand_pile.clone(),
        BuildingType::BonePile => game_assets.bone_pile.clone(),
        BuildingType::WheelbarrowParking => game_assets.wheelbarrow_parking.clone(),
    };

    let entity = commands
        .spawn((
            Blueprint::new(building_type, occupied_grids.clone()),
            crate::systems::jobs::Designation {
                work_type: crate::systems::jobs::WorkType::Build,
            },
            crate::systems::jobs::TaskSlots::new(1),
            Sprite {
                image: texture,
                color: Color::srgba(1.0, 1.0, 1.0, 0.5),
                custom_size,
                ..default()
            },
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, Z_AURA),
            Name::new(format!("Blueprint ({:?})", building_type)),
        ))
        .id();

    for &g in &occupied_grids {
        world_map.buildings.insert(g, entity);
        world_map.add_obstacle(g.0, g.1);
    }
    info!(
        "BLUEPRINT: Placed {:?} at {:?}",
        building_type, occupied_grids
    );

    Some((entity, occupied_grids, spawn_pos))
}

fn try_place_bucket_storage_companion(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    parent_blueprint: Entity,
    parent_occupied_grids: &[(i32, i32)],
    anchor_grid: (i32, i32),
) -> bool {
    let storage_grids = [anchor_grid, (anchor_grid.0 + 1, anchor_grid.1)];
    let is_near_parent = storage_grids.iter().all(|&storage_grid| {
        parent_occupied_grids.iter().any(|&parent_grid| {
            grid_is_nearby(parent_grid, storage_grid, TANK_NEARBY_BUCKET_STORAGE_TILES)
        })
    });
    if !is_near_parent {
        return false;
    }

    let can_place = storage_grids.iter().all(|&(gx, gy)| {
        !world_map.buildings.contains_key(&(gx, gy))
            && !world_map.stockpiles.contains_key(&(gx, gy))
            && world_map.is_walkable(gx, gy)
    });
    if !can_place {
        return false;
    }

    for (gx, gy) in storage_grids {
        let pos = WorldMap::grid_to_world(gx, gy);
        let storage_entity = commands
            .spawn((
                crate::systems::logistics::Stockpile {
                    capacity: 10,
                    resource_type: None,
                },
                crate::systems::logistics::BucketStorage,
                crate::systems::logistics::PendingBelongsToBlueprint(parent_blueprint),
                Sprite {
                    color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, Z_MAP + 0.01),
                Name::new("Pending Tank Bucket Storage"),
            ))
            .id();
        world_map.stockpiles.insert((gx, gy), storage_entity);
    }
    true
}

fn has_nearby_sandpile(
    mixer_occupied_grids: &[(i32, i32)],
    q_sand_piles: &Query<&Transform, With<SandPile>>,
    q_blueprints: &Query<(Entity, &Blueprint, &Transform)>,
    ignore_blueprint: Option<Entity>,
) -> bool {
    if q_sand_piles.iter().any(|transform| {
        let sand_grid = WorldMap::world_to_grid(transform.translation.truncate());
        mixer_occupied_grids
            .iter()
            .any(|&(mx, my)| grid_is_nearby((mx, my), sand_grid, MUD_MIXER_NEARBY_SANDPILE_TILES))
    }) {
        return true;
    }

    q_blueprints.iter().any(|(entity, bp, transform)| {
        if Some(entity) == ignore_blueprint {
            return false;
        }
        if bp.kind != BuildingType::SandPile {
            return false;
        }
        let sand_grid = WorldMap::world_to_grid(transform.translation.truncate());
        mixer_occupied_grids
            .iter()
            .any(|&(mx, my)| grid_is_nearby((mx, my), sand_grid, MUD_MIXER_NEARBY_SANDPILE_TILES))
    })
}

fn make_companion_placement(
    parent_kind: CompanionParentKind,
    parent_anchor: (i32, i32),
    kind: CompanionPlacementKind,
    center: Vec2,
) -> CompanionPlacement {
    CompanionPlacement {
        parent_kind,
        parent_anchor,
        kind,
        center,
        radius: TILE_SIZE * COMPANION_PLACEMENT_RADIUS_TILES,
        required: true,
    }
}

fn parent_building_type(parent_kind: CompanionParentKind) -> BuildingType {
    match parent_kind {
        CompanionParentKind::Tank => BuildingType::Tank,
        CompanionParentKind::MudMixer => BuildingType::MudMixer,
    }
}

fn occupied_grids_for_building(building_type: BuildingType, grid: (i32, i32)) -> Vec<(i32, i32)> {
    match building_type {
        BuildingType::Tank | BuildingType::MudMixer | BuildingType::WheelbarrowParking => vec![
            grid,
            (grid.0 + 1, grid.1),
            (grid.0, grid.1 + 1),
            (grid.0 + 1, grid.1 + 1),
        ],
        _ => vec![grid],
    }
}

fn building_spawn_pos(building_type: BuildingType, grid: (i32, i32)) -> Vec2 {
    let base_pos = WorldMap::grid_to_world(grid.0, grid.1);
    match building_type {
        BuildingType::Tank | BuildingType::MudMixer | BuildingType::WheelbarrowParking => {
            base_pos + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 0.5)
        }
        _ => base_pos,
    }
}

fn building_size(building_type: BuildingType) -> Vec2 {
    match building_type {
        BuildingType::Tank | BuildingType::MudMixer | BuildingType::WheelbarrowParking => {
            Vec2::splat(TILE_SIZE * 2.0)
        }
        _ => Vec2::splat(TILE_SIZE),
    }
}

fn grid_is_nearby(base: (i32, i32), target: (i32, i32), tiles: i32) -> bool {
    (target.0 - base.0).abs() <= tiles && (target.1 - base.1).abs() <= tiles
}
