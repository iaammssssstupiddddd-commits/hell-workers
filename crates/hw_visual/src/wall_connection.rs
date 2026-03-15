use crate::handles::WallVisualHandles;
use crate::layer::VisualLayerKind;
use bevy::prelude::*;
use hw_core::visual_mirror::construction::BlueprintVisualState;
use hw_jobs::{Building, BuildingType};
use hw_world::{WorldMap, WorldMapRead};
use std::collections::HashSet;

/// 壁の接続更新を行うシステム
pub fn wall_connections_system(
    wall_handles: Res<WallVisualHandles>,
    world_map: WorldMapRead,
    q_new_buildings: Query<
        (Entity, &Transform, &Building),
        Or<(Added<Building>, Changed<Building>)>,
    >,
    q_new_blueprints: Query<
        (Entity, &Transform, &BlueprintVisualState),
        Added<BlueprintVisualState>,
    >,
    q_walls_check: Query<
        (Option<&Building>, Option<&BlueprintVisualState>),
        Or<(With<Building>, With<BlueprintVisualState>)>,
    >,
    q_children: Query<&Children>,
    mut q_visual_layers: Query<(&VisualLayerKind, &mut Sprite)>,
    // Blueprint エンティティは Sprite を直接持つ（子構造ではない）
    mut q_blueprint_sprites: Query<&mut Sprite, Without<VisualLayerKind>>,
) {
    let mut update_targets = HashSet::new();

    for (_entity, transform, building) in q_new_buildings.iter() {
        if matches!(building.kind, BuildingType::Wall | BuildingType::Door) {
            let (x, y) = WorldMap::world_to_grid(transform.translation.truncate());
            add_neighbors_to_update(x, y, &mut update_targets);
        }
    }

    for (_entity, _transform, state) in q_new_blueprints.iter() {
        if state.is_wall_or_door {
            for &(gx, gy) in &state.occupied_grids {
                add_neighbors_to_update(gx, gy, &mut update_targets);
            }
        }
    }

    if update_targets.is_empty() {
        return;
    }

    for (gx, gy) in update_targets {
        if let Some(entity) = world_map.building_entity((gx, gy)) {
            if is_wall(gx, gy, world_map.as_ref(), &q_walls_check) {
                let is_plain_wall =
                    q_walls_check
                        .get(entity)
                        .ok()
                        .is_some_and(|(building_opt, blueprint_opt)| {
                            building_opt.is_some_and(|b| b.kind == BuildingType::Wall)
                                || blueprint_opt.is_some_and(|s| s.is_plain_wall)
                        });
                if !is_plain_wall {
                    continue;
                }

                // 完成した Building は Sprite を VisualLayerKind::Struct 子エンティティに持つ
                let mut updated = false;
                if let Ok(children) = q_children.get(entity) {
                    for child in children.iter() {
                        if let Ok((kind, mut sprite)) = q_visual_layers.get_mut(child) {
                            if *kind == VisualLayerKind::Struct {
                                update_wall_sprite(
                                    entity,
                                    gx,
                                    gy,
                                    &mut sprite,
                                    world_map.as_ref(),
                                    &q_walls_check,
                                    &wall_handles,
                                );
                                updated = true;
                                break;
                            }
                        }
                    }
                }
                // Blueprint エンティティは Sprite を直接持つ
                if !updated {
                    if let Ok(mut sprite) = q_blueprint_sprites.get_mut(entity) {
                        update_wall_sprite(
                            entity,
                            gx,
                            gy,
                            &mut sprite,
                            world_map.as_ref(),
                            &q_walls_check,
                            &wall_handles,
                        );
                    }
                }
            }
        }
    }
}

fn add_neighbors_to_update(x: i32, y: i32, targets: &mut HashSet<(i32, i32)>) {
    targets.insert((x, y));
    targets.insert((x, y + 1));
    targets.insert((x, y - 1));
    targets.insert((x + 1, y));
    targets.insert((x - 1, y));
}

fn update_wall_sprite(
    wall_entity: Entity,
    x: i32,
    y: i32,
    sprite: &mut Sprite,
    world_map: &WorldMap,
    q_walls_check: &Query<
        (Option<&Building>, Option<&BlueprintVisualState>),
        Or<(With<Building>, With<BlueprintVisualState>)>,
    >,
    wall_handles: &WallVisualHandles,
) {
    let up = is_wall(x, y + 1, world_map, q_walls_check);
    let down = is_wall(x, y - 1, world_map, q_walls_check);
    let left = is_wall(x - 1, y, world_map, q_walls_check);
    let right = is_wall(x + 1, y, world_map, q_walls_check);

    let is_provisional = is_provisional_wall(wall_entity, q_walls_check);

    let (texture, flip_x, flip_y) = if is_provisional {
        match (up, down, left, right) {
            (false, false, false, false) => (wall_handles.stone_isolated.clone(), false, false),
            (false, false, true, false) => {
                (wall_handles.stone_horizontal_left.clone(), false, false)
            }
            (false, false, false, true) => {
                (wall_handles.stone_horizontal_right.clone(), false, false)
            }
            (false, false, true, true) => {
                (wall_handles.stone_horizontal_both.clone(), false, false)
            }
            (true, false, false, false) => (wall_handles.stone_vertical_top.clone(), false, false),
            (false, true, false, false) => {
                (wall_handles.stone_vertical_bottom.clone(), false, false)
            }
            (true, true, false, false) => (wall_handles.stone_vertical_both.clone(), false, false),
            (true, false, true, false) => (wall_handles.stone_corner_tl.clone(), false, false),
            (true, false, false, true) => (wall_handles.stone_corner_tr.clone(), false, false),
            (false, true, true, false) => (wall_handles.stone_corner_bl.clone(), false, false),
            (false, true, false, true) => (wall_handles.stone_corner_br.clone(), false, false),
            (true, true, true, false) => (wall_handles.stone_t_left.clone(), false, false),
            (true, true, false, true) => (wall_handles.stone_t_right.clone(), false, false),
            (true, false, true, true) => (wall_handles.stone_t_up.clone(), false, false),
            (false, true, true, true) => (wall_handles.stone_t_down.clone(), false, false),
            (true, true, true, true) => (wall_handles.stone_cross.clone(), false, false),
        }
    } else {
        match (up, down, left, right) {
            (false, false, false, false) => (wall_handles.mud_isolated.clone(), false, false),
            (false, false, true, false) => (wall_handles.mud_end_right.clone(), false, false),
            (false, false, false, true) => (wall_handles.mud_end_left.clone(), false, false),
            (false, false, true, true) => (wall_handles.mud_horizontal.clone(), false, false),
            (true, false, false, false) => (wall_handles.mud_end_bottom.clone(), false, false),
            (false, true, false, false) => (wall_handles.mud_end_top.clone(), false, false),
            (true, true, false, false) => (wall_handles.mud_vertical.clone(), false, false),
            (true, false, true, false) => (wall_handles.mud_corner_tl.clone(), false, false),
            (true, false, false, true) => (wall_handles.mud_corner_tr.clone(), false, false),
            (false, true, true, false) => (wall_handles.mud_corner_bl.clone(), false, false),
            (false, true, false, true) => (wall_handles.mud_corner_br.clone(), false, false),
            (true, true, true, false) => (wall_handles.mud_t_left.clone(), false, false),
            (true, true, false, true) => (wall_handles.mud_t_right.clone(), false, false),
            (true, false, true, true) => (wall_handles.mud_t_up.clone(), false, false),
            (false, true, true, true) => (wall_handles.mud_t_down.clone(), false, false),
            (true, true, true, true) => (wall_handles.mud_cross.clone(), false, false),
        }
    };

    sprite.image = texture;
    sprite.flip_x = flip_x;
    sprite.flip_y = flip_y;
    sprite.color = if is_provisional {
        Color::srgba(1.0, 0.75, 0.4, 0.85)
    } else {
        Color::WHITE
    };
}

fn is_provisional_wall(
    entity: Entity,
    q_walls_check: &Query<
        (Option<&Building>, Option<&BlueprintVisualState>),
        Or<(With<Building>, With<BlueprintVisualState>)>,
    >,
) -> bool {
    q_walls_check
        .get(entity)
        .ok()
        .and_then(|(building_opt, _)| building_opt)
        .is_some_and(|building| building.kind == BuildingType::Wall && building.is_provisional)
}

fn is_wall(
    x: i32,
    y: i32,
    world_map: &WorldMap,
    q_walls_check: &Query<
        (Option<&Building>, Option<&BlueprintVisualState>),
        Or<(With<Building>, With<BlueprintVisualState>)>,
    >,
) -> bool {
    if let Some(entity) = world_map.building_entity((x, y)) {
        if let Ok((building_opt, blueprint_opt)) = q_walls_check.get(entity) {
            if let Some(b) = building_opt {
                return matches!(b.kind, BuildingType::Wall | BuildingType::Door);
            }
            if let Some(s) = blueprint_opt {
                return s.is_wall_or_door;
            }
        }
    }
    false
}
