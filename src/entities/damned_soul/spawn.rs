//! ソウルのスポーン関連システム

use super::*;
use crate::assets::GameAssets;
use crate::constants::*;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::world::map::WorldMap;
use rand::Rng;
use std::env;

fn parse_spawn_souls_from_args() -> Option<usize> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--spawn-souls" {
            let value = args.next()?;
            if let Ok(parsed) = value.parse::<usize>() {
                return Some(parsed);
            }
        }
    }
    None
}

/// 人間をスポーンする
pub fn spawn_damned_souls(mut spawn_events: MessageWriter<DamnedSoulSpawnEvent>) {
    let mut rng = rand::thread_rng();
    let spawn_count = parse_spawn_souls_from_args().unwrap_or_else(|| {
        env::var("HW_SPAWN_SOULS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10)
    });

    for _ in 0..spawn_count {
        let x = rng.gen_range(-100.0..100.0);
        let y = rng.gen_range(-100.0..100.0);
        spawn_events.write(DamnedSoulSpawnEvent {
            position: Vec2::new(x, y),
        });
    }

    info!("SPAWN_CONFIG: Souls requested={}", spawn_count);
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

    commands.spawn((
        DamnedSoul::default(),
        SoulUiLinks::default(),
        Name::new(format!("Soul: {}", soul_name)),
        identity,
        IdleState::default(),
        AssignedTask::default(),
        Sprite {
            image: game_assets.soul_move.clone(),
            custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
            color: sprite_color,
            texture_atlas: Some(TextureAtlas {
                layout: game_assets.soul_layout.clone(),
                index: 0,
            }),
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
    ));

    info!("SPAWN: {} ({:?}) at {:?}", soul_name, gender, actual_pos);
}
