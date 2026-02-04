//! ソウルのスポーン関連システム

use super::*;
use crate::assets::GameAssets;
use crate::constants::*;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::world::map::WorldMap;
use rand::Rng;

use super::observers::*;

/// 人間をスポーンする
pub fn spawn_damned_souls(mut spawn_events: MessageWriter<DamnedSoulSpawnEvent>) {
    let mut rng = rand::thread_rng();
    for _ in 0..10 {
        let x = rng.gen_range(-100.0..100.0);
        let y = rng.gen_range(-100.0..100.0);
        spawn_events.write(DamnedSoulSpawnEvent {
            position: Vec2::new(x, y),
        });
    }
}

/// スポーンイベントを処理するシステム
pub fn soul_spawning_system(
    mut commands: Commands,
    mut spawn_events: MessageReader<DamnedSoulSpawnEvent>,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
) {
    for event in spawn_events.read() {
        spawn_damned_soul_at(&mut commands, &game_assets, &world_map, event.position);
    }
}

/// 指定座標にソウルをスポーンする（内部用ヘルパー）
pub fn spawn_damned_soul_at(
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    world_map: &Res<WorldMap>,
    pos: Vec2,
) {
    let spawn_grid = WorldMap::world_to_grid(pos);
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

    let identity = SoulIdentity::random();
    let soul_name = identity.name.clone();
    let gender = identity.gender;

    let sprite_color = match gender {
        Gender::Male => Color::srgb(0.9, 0.9, 1.0), // わずかに青み
        Gender::Female => Color::srgb(1.0, 0.9, 0.9), // わずかに赤み
    };

    commands
        .spawn((
            DamnedSoul::default(),
            SoulUiLinks::default(),
            Name::new(format!("Soul: {}", soul_name)),
            identity,
            IdleState::default(),
            AssignedTask::default(),
            Sprite {
                image: game_assets.colonist.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
                color: sprite_color,
                ..default()
            },
            Transform::from_xyz(actual_pos.x, actual_pos.y, Z_CHARACTER),
            Destination(actual_pos),
            Path::default(),
            AnimationState::default(),
            crate::systems::visual::speech::components::SoulEmotionState::default(),
            crate::systems::visual::speech::conversation::components::ConversationInitiator {
                timer: Timer::from_seconds(CONVERSATION_CHECK_INTERVAL, TimerMode::Repeating),
            },
            crate::systems::logistics::Inventory::default(),
        ))
        .observe(on_task_assigned)
        .observe(on_task_completed)
        .observe(on_soul_recruited)
        .observe(on_stress_breakdown)
        .observe(on_exhausted)
        .observe(crate::systems::visual::speech::observers::on_released_from_service)
        .observe(crate::systems::visual::speech::observers::on_gathering_joined)
        .observe(crate::systems::visual::speech::observers::on_task_abandoned);

    info!("SPAWN: {} ({:?}) at {:?}", soul_name, gender, actual_pos);
}
