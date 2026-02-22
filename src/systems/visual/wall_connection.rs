use crate::assets::GameAssets;
use crate::systems::jobs::{Blueprint, Building, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashSet;

pub struct WallConnectionPlugin;

impl Plugin for WallConnectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            wall_connections_system.in_set(crate::systems::GameSystemSet::Visual),
        );
    }
}

/// 壁の接続更新を行うシステム
/// 壁（Building）や壁の設計図（Blueprint）が追加されたとき、
/// 自身と隣接する壁の見た目を更新する。
fn wall_connections_system(
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
    q_new_buildings: Query<
        (Entity, &Transform, &Building),
        Or<(Added<Building>, Changed<Building>)>,
    >,
    q_new_blueprints: Query<(Entity, &Transform, &Blueprint), Added<Blueprint>>,
    // 状態チェック用クエリ（Spriteを含まない）
    q_walls_check: Query<
        (Option<&Building>, Option<&Blueprint>),
        Or<(With<Building>, With<Blueprint>)>,
    >,
    // スプライト更新用クエリ
    mut q_sprites: Query<&mut Sprite>,
) {
    let mut update_targets = HashSet::new();

    // 1. 新しく完成した壁/扉があれば、その座標と周囲を更新対象に追加
    for (_entity, transform, building) in q_new_buildings.iter() {
        if matches!(building.kind, BuildingType::Wall | BuildingType::Door) {
            let (x, y) = WorldMap::world_to_grid(transform.translation.truncate());
            add_neighbors_to_update(x, y, &mut update_targets);
        }
    }

    // 2. 新しく配置された壁/扉の設計図があれば、その座標と周囲を更新対象に追加
    for (_entity, _transform, blueprint) in q_new_blueprints.iter() {
        // Blueprint::kind を確認
        if matches!(blueprint.kind, BuildingType::Wall | BuildingType::Door) {
            for &(gx, gy) in &blueprint.occupied_grids {
                add_neighbors_to_update(gx, gy, &mut update_targets);
            }
        }
    }

    if update_targets.is_empty() {
        return;
    }

    // 3. 対象の座標にあるエンティティのスプライトを更新
    for (gx, gy) in update_targets {
        // マップからその座標にあるエンティティを取得
        if let Some(&entity) = world_map.buildings.get(&(gx, gy)) {
            if is_wall(gx, gy, &world_map, &q_walls_check) {
                let is_plain_wall = q_walls_check.get(entity).ok().is_some_and(
                    |(building_opt, blueprint_opt)| {
                        building_opt.is_some_and(|b| b.kind == BuildingType::Wall)
                            || blueprint_opt.is_some_and(|bp| bp.kind == BuildingType::Wall)
                    },
                );
                if !is_plain_wall {
                    continue;
                }
                if let Ok(mut sprite) = q_sprites.get_mut(entity) {
                    update_wall_sprite(
                        entity,
                        gx,
                        gy,
                        &mut sprite,
                        &world_map,
                        &q_walls_check,
                        &game_assets,
                    );
                }
            }
        }
    }
}

/// 指定座標とその4近傍を更新対象セットに追加
fn add_neighbors_to_update(x: i32, y: i32, targets: &mut HashSet<(i32, i32)>) {
    targets.insert((x, y));
    targets.insert((x, y + 1));
    targets.insert((x, y - 1));
    targets.insert((x + 1, y));
    targets.insert((x - 1, y));
}

/// 単一の壁のスプライトを、周囲の状況に合わせて更新
fn update_wall_sprite(
    wall_entity: Entity,
    x: i32,
    y: i32,
    sprite: &mut Sprite,
    world_map: &WorldMap,
    q_walls_check: &Query<
        (Option<&Building>, Option<&Blueprint>),
        Or<(With<Building>, With<Blueprint>)>,
    >,
    game_assets: &GameAssets,
) {
    // Check connections
    let up = is_wall(x, y + 1, world_map, q_walls_check);
    let down = is_wall(x, y - 1, world_map, q_walls_check);
    let left = is_wall(x - 1, y, world_map, q_walls_check);
    let right = is_wall(x + 1, y, world_map, q_walls_check);

    let is_provisional = is_provisional_wall(wall_entity, q_walls_check);

    let (texture, flip_x, flip_y) = if is_provisional {
        // 仮設（木の壁）
        match (up, down, left, right) {
            (false, false, false, false) => (game_assets.wall_isolated.clone(), false, false),
            (false, false, true, false) => (game_assets.wall_horizontal_left.clone(), false, false),
            (false, false, false, true) => {
                (game_assets.wall_horizontal_right.clone(), false, false)
            }
            (false, false, true, true) => (game_assets.wall_horizontal_both.clone(), false, false),
            (true, false, false, false) => (game_assets.wall_vertical_top.clone(), false, false),
            (false, true, false, false) => (game_assets.wall_vertical_bottom.clone(), false, false),
            (true, true, false, false) => (game_assets.wall_vertical_both.clone(), false, false),
            (true, false, true, false) => (game_assets.wall_corner_top_left.clone(), false, false),
            (true, false, false, true) => (game_assets.wall_corner_top_right.clone(), false, false),
            (false, true, true, false) => {
                (game_assets.wall_corner_bottom_left.clone(), false, false)
            }
            (false, true, false, true) => {
                (game_assets.wall_corner_bottom_right.clone(), false, false)
            }
            (true, true, true, false) => (game_assets.wall_t_left.clone(), false, false),
            (true, true, false, true) => (game_assets.wall_t_right.clone(), false, false),
            (true, false, true, true) => (game_assets.wall_t_up.clone(), false, false),
            (false, true, true, true) => (game_assets.wall_t_down.clone(), false, false),
            (true, true, true, true) => (game_assets.wall_cross.clone(), false, false),
        }
    } else {
        // 本設（泥の壁）
        match (up, down, left, right) {
            (false, false, false, false) => (game_assets.mud_wall_isolated.clone(), false, false),
            // Horizontal
            (false, false, true, false) => (game_assets.mud_wall_end_right.clone(), false, false),
            (false, false, false, true) => (game_assets.mud_wall_end_left.clone(), false, false),
            (false, false, true, true) => (game_assets.mud_wall_horizontal.clone(), false, false),
            // Vertical
            (true, false, false, false) => (game_assets.mud_wall_end_bottom.clone(), false, false),
            (false, true, false, false) => (game_assets.mud_wall_end_top.clone(), false, false),
            (true, true, false, false) => (game_assets.mud_wall_vertical.clone(), false, false),
            // Corners
            (true, false, true, false) => {
                (game_assets.mud_wall_corner_top_left.clone(), false, false)
            }
            (true, false, false, true) => {
                (game_assets.mud_wall_corner_top_right.clone(), false, false)
            }
            (false, true, true, false) => (
                game_assets.mud_wall_corner_bottom_left.clone(),
                false,
                false,
            ),
            (false, true, false, true) => (
                game_assets.mud_wall_corner_bottom_right.clone(),
                false,
                false,
            ),
            // T-Junctions
            (true, true, true, false) => (game_assets.mud_wall_t_left.clone(), false, false),
            (true, true, false, true) => (game_assets.mud_wall_t_right.clone(), false, false),
            (true, false, true, true) => (game_assets.mud_wall_t_up.clone(), false, false),
            (false, true, true, true) => (game_assets.mud_wall_t_down.clone(), false, false),
            // Cross
            (true, true, true, true) => (game_assets.mud_wall_cross.clone(), false, false),
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
        (Option<&Building>, Option<&Blueprint>),
        Or<(With<Building>, With<Blueprint>)>,
    >,
) -> bool {
    q_walls_check
        .get(entity)
        .ok()
        .and_then(|(building_opt, _)| building_opt)
        .is_some_and(|building| building.kind == BuildingType::Wall && building.is_provisional)
}

/// 座標(x, y)に「壁」または「壁の設計図」があるか確認
fn is_wall(
    x: i32,
    y: i32,
    world_map: &WorldMap,
    q_walls_check: &Query<
        (Option<&Building>, Option<&Blueprint>),
        Or<(With<Building>, With<Blueprint>)>,
    >,
) -> bool {
    if let Some(&entity) = world_map.buildings.get(&(x, y)) {
        if let Ok((building_opt, blueprint_opt)) = q_walls_check.get(entity) {
            if let Some(b) = building_opt {
                return matches!(b.kind, BuildingType::Wall | BuildingType::Door);
            }
            if let Some(bp) = blueprint_opt {
                return matches!(bp.kind, BuildingType::Wall | BuildingType::Door);
            }
        }
    }
    false
}
