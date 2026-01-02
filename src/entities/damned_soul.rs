use bevy::prelude::*;
use crate::constants::*;
use crate::assets::GameAssets;
use crate::world::map::WorldMap;
use crate::world::pathfinding::find_path;
use crate::systems::work::AssignedTask;

/// 地獄に堕ちた人間（怠惰な魂）
#[derive(Component)]
pub struct DamnedSoul {
    #[allow(dead_code)]
    pub sin_type: SinType,
    pub laziness: f32,      // 怠惰レベル (0.0-1.0) - 高いほど怠惰
    pub motivation: f32,    // やる気 (0.0-1.0) - 高いほど働く
    pub fatigue: f32,       // 疲労 (0.0-1.0) - 高いほど疲れている
}

impl Default for DamnedSoul {
    fn default() -> Self {
        Self {
            sin_type: SinType::Sloth,
            laziness: 0.7,      // デフォルトで怠惰
            motivation: 0.1,    // デフォルトでやる気なし
            fatigue: 0.0,
        }
    }
}

/// 落ちた理由（将来拡張用）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum SinType {
    #[default]
    Sloth,    // 怠惰
    Greed,    // 強欲
    Wrath,    // 憤怒
}

/// 怠惰状態のコンポーネント
#[derive(Component)]
pub struct IdleState {
    pub idle_timer: f32,
    pub behavior: IdleBehavior,
    pub behavior_duration: f32,  // 現在の行動をどれくらい続けるか
}

impl Default for IdleState {
    fn default() -> Self {
        Self {
            idle_timer: 0.0,
            behavior: IdleBehavior::Wandering,
            behavior_duration: 3.0,
        }
    }
}

/// 怠惰行動の種類（将来拡張用：Chatting, Dancing等）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdleBehavior {
    #[default]
    Wandering,    // うろうろ
    Sitting,      // 座り込み
    Sleeping,     // 寝ている
    // 将来: Chatting, Dancing
}

/// 移動先
#[derive(Component)]
pub struct Destination(pub Vec2);

/// 経路
#[derive(Component, Default)]
pub struct Path {
    pub waypoints: Vec<Vec2>,
    pub current_index: usize,
}

/// アニメーション状態
#[derive(Component)]
pub struct AnimationState {
    pub is_moving: bool,
    pub facing_right: bool,
    pub bob_timer: f32,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            is_moving: false,
            facing_right: true,
            bob_timer: 0.0,
        }
    }
}

/// 人間をスポーンする
pub fn spawn_damned_souls(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
) {
    // 3体の人間をスポーン
    let spawn_positions = [
        Vec2::new(-50.0, -50.0),
        Vec2::new(50.0, 0.0),
        Vec2::new(0.0, 50.0),
    ];

    for (i, spawn_pos) in spawn_positions.iter().enumerate() {
        // 歩ける場所を探す
        let spawn_grid = WorldMap::world_to_grid(*spawn_pos);
        let mut actual_grid = spawn_grid;
        'search: for dx in -5..=5 {
            for dy in -5..=5 {
                let test = (spawn_grid.0 + dx, spawn_grid.1 + dy);
                if world_map.is_walkable(test.0, test.1) {
                    actual_grid = test;
                    break 'search;
                }
            }
        }
        let actual_pos = WorldMap::grid_to_world(actual_grid.0, actual_grid.1);

        commands.spawn((
            DamnedSoul::default(),
            IdleState::default(),
            AssignedTask::default(),
            crate::systems::logistics::Inventory(None), // インベントリを追加
            Sprite {
                image: game_assets.colonist.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
                ..default()
            },
            Transform::from_xyz(actual_pos.x, actual_pos.y, 1.0),
            Destination(actual_pos),
            Path::default(),
            AnimationState::default(),
        ));

        info!("SPAWN: DamnedSoul {} at {:?}", i, actual_pos);
    }
}

/// 経路探索システム
pub fn pathfinding_system(
    world_map: Res<WorldMap>,
    mut query: Query<(Entity, &Transform, &Destination, &mut Path), Changed<Destination>>,
) {
    for (entity, transform, destination, mut path) in query.iter_mut() {
        let current_pos = transform.translation.truncate();
        let start_grid = WorldMap::world_to_grid(current_pos);
        let goal_grid = WorldMap::world_to_grid(destination.0);

        // すでに同じ目的地への経路を持っている場合はスキップ
        if let Some(last) = path.waypoints.last() {
            if last.distance_squared(destination.0) < 1.0 {
                continue;
            }
        }

        if start_grid == goal_grid {
            path.waypoints = vec![destination.0];
            path.current_index = 0;
            continue;
        }

        if let Some(grid_path) = find_path(&world_map, start_grid, goal_grid) {
            path.waypoints = grid_path.iter()
                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                .collect();
            path.current_index = 0;
            debug!("PATH: Soul {:?} found new path ({} steps)", entity, path.waypoints.len());
        } else {
            debug!("PATH: Soul {:?} failed to find path to {:?}", entity, goal_grid);
            path.waypoints.clear();
        }
    }
}

/// 移動システム
pub fn soul_movement(
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut Path, &mut AnimationState, &DamnedSoul)>,
) {
    for (entity, mut transform, mut path, mut anim, soul) in query.iter_mut() {
        if path.current_index < path.waypoints.len() {
            let target = path.waypoints[path.current_index];
            let current_pos = transform.translation.truncate();
            let to_target = target - current_pos;
            let distance = to_target.length();

            if distance > 2.0 {
                // やる気が高いほど速く動く
                let base_speed = 60.0;
                let motivation_bonus = soul.motivation * 40.0;
                let laziness_penalty = soul.laziness * 30.0;
                let speed = (base_speed + motivation_bonus - laziness_penalty).max(20.0);

                let move_dist = (speed * time.delta_secs()).min(distance);
                let direction = to_target.normalize();
                let velocity = direction * move_dist;
                transform.translation += velocity.extend(0.0);

                anim.is_moving = true;
                if direction.x.abs() > 0.1 {
                    anim.facing_right = direction.x > 0.0;
                }
            } else {
                path.current_index += 1;
                if path.current_index >= path.waypoints.len() {
                    debug!("MOVE: Soul {:?} reached final destination", entity);
                }
            }
        } else {
            anim.is_moving = false;
        }
    }
}

/// アニメーションシステム
pub fn animation_system(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Sprite, &mut AnimationState, &DamnedSoul)>,
) {
    for (mut transform, mut sprite, mut anim, soul) in query.iter_mut() {
        sprite.flip_x = !anim.facing_right;

        if anim.is_moving {
            anim.bob_timer += time.delta_secs() * 10.0;
            let bob = (anim.bob_timer.sin() * 0.05) + 1.0;
            transform.scale = Vec3::new(1.0, bob, 1.0);
        } else {
            // 怠惰なほどゆっくり呼吸
            let breath_speed = 2.0 - soul.laziness;
            anim.bob_timer += time.delta_secs() * breath_speed;
            let breath = (anim.bob_timer.sin() * 0.02) + 1.0;
            transform.scale = Vec3::splat(breath);
        }
    }
}
