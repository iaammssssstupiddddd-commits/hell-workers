//! ソウルのスポーン関連システム

use super::*;
use crate::assets::GameAssets;
use crate::entities::{spawn_args, spawn_position};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::world::map::{RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MIN, WorldMap, WorldMapRead};
use hw_core::constants::*;
use rand::Rng;

pub use hw_core::population::PopulationManager;

fn initial_spawn_count() -> u32 {
    spawn_args::parse_spawn_count_from_args_or_env(
        "--spawn-souls",
        "HW_SPAWN_SOULS",
        SOUL_SPAWN_INITIAL,
    )
}

fn pick_river_south_bank_spawn(world_map: &WorldMap, rng: &mut impl Rng) -> Option<Vec2> {
    let south_y_max = RIVER_Y_MIN - 1;
    let south_y_min = 5; // マップ端の少し内側から開始

    hw_world::pick_random_walkable_grid_in_rect(
        world_map,
        RIVER_X_MIN,
        RIVER_X_MAX,
        south_y_min,
        south_y_max,
        64,
        rng,
    )
    .or_else(|| {
        hw_world::pick_random_walkable_grid_in_rect(
            world_map,
            RIVER_X_MIN,
            RIVER_X_MAX,
            south_y_min,
            south_y_max,
            256,
            rng,
        )
    })
    .map(|(x, y)| WorldMap::grid_to_world(x, y))
}

fn queue_river_spawn_events(
    spawn_events: &mut MessageWriter<DamnedSoulSpawnEvent>,
    world_map: &WorldMap,
    count: u32,
) -> u32 {
    let mut rng = rand::thread_rng();
    let mut spawned = 0;

    for _ in 0..count {
        if let Some(position) = pick_river_south_bank_spawn(world_map, &mut rng) {
            spawn_events.write(DamnedSoulSpawnEvent { position });
            spawned += 1;
        }
    }

    spawned
}

/// 人間をスポーンする
pub fn spawn_damned_souls(
    mut spawn_events: MessageWriter<DamnedSoulSpawnEvent>,
    world_map: WorldMapRead,
) {
    let spawn_count = initial_spawn_count();
    let spawned = queue_river_spawn_events(&mut spawn_events, &world_map, spawn_count);
    info!(
        "SPAWN_CONFIG: Souls requested={} spawned={} (river south bank)",
        spawn_count, spawned
    );
}

/// 人口管理の追跡
pub fn population_tracking_system(
    time: Res<Time>,
    mut population: ResMut<PopulationManager>,
    q_souls: Query<Entity, With<DamnedSoul>>,
    q_rest_areas: Query<&crate::systems::jobs::RestArea>,
) {
    population.current_count = q_souls.iter().count() as u32;
    population.population_cap = SOUL_POPULATION_BASE_CAP
        + q_rest_areas.iter().count() as u32 * SOUL_POPULATION_PER_REST_AREA;
    population.escape_cooldown_remaining =
        (population.escape_cooldown_remaining - time.delta_secs()).max(0.0);
}

/// 定期スポーン（人口上限と不足時ボーナスを考慮）
pub fn periodic_spawn_system(
    time: Res<Time>,
    world_map: WorldMapRead,
    mut population: ResMut<PopulationManager>,
    mut spawn_events: MessageWriter<DamnedSoulSpawnEvent>,
) {
    let current = population.current_count;
    let cap = population.population_cap.max(SOUL_POPULATION_BASE_CAP);

    if current == 0 {
        let emergency = SOUL_SPAWN_INITIAL.min(cap.max(1));
        let spawned = queue_river_spawn_events(&mut spawn_events, &world_map, emergency);
        if spawned > 0 {
            population.total_spawned += spawned;
            info!(
                "SOUL_POP: emergency_spawn={} current={} cap={}",
                spawned, current, cap
            );
        }
        population.spawn_timer.reset();
        return;
    }

    if !population.spawn_timer.tick(time.delta()).just_finished() {
        return;
    }

    if current >= cap {
        return;
    }

    let mut rng = rand::thread_rng();
    let mut spawn_count = rng.gen_range(SOUL_SPAWN_COUNT_MIN..=SOUL_SPAWN_COUNT_MAX);
    if current * 2 <= cap {
        spawn_count += 1;
    }
    spawn_count = spawn_count.min(cap.saturating_sub(current));
    if spawn_count == 0 {
        return;
    }

    let spawned = queue_river_spawn_events(&mut spawn_events, &world_map, spawn_count);
    if spawned > 0 {
        population.total_spawned += spawned;
        info!(
            "SOUL_POP: periodic_spawn={} current={} cap={}",
            spawned, current, cap
        );
    }
}

/// スポーンイベントを処理するシステム
pub fn soul_spawning_system(
    mut commands: Commands,
    mut spawn_events: MessageReader<DamnedSoulSpawnEvent>,
    game_assets: Res<GameAssets>,
    world_map: WorldMapRead,
) {
    for event in spawn_events.read() {
        spawn_damned_soul_at(
            &mut commands,
            &game_assets,
            world_map.as_ref(),
            event.position,
        );
    }
}

/// 指定座標にソウルをスポーンする（内部用ヘルパー）
pub fn spawn_damned_soul_at(
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    world_map: &WorldMap,
    pos: Vec2,
) {
    let spawn_grid = WorldMap::world_to_grid(pos);
    let actual_grid = spawn_position::find_nearby_walkable_grid(spawn_grid, world_map, 5);
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
        DreamState::default(),
        Name::new(format!("Soul: {}", soul_name)),
        identity,
        IdleState::default(),
        AssignedTask::default(),
        Sprite {
            image: game_assets.soul.clone(),
            custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
            color: sprite_color,
            ..default()
        },
        Transform::from_xyz(actual_pos.x, actual_pos.y, Z_CHARACTER),
        Destination(actual_pos),
        Path::default(),
        AnimationState::default(),
        hw_visual::speech::components::SoulEmotionState::default(),
        hw_visual::speech::conversation::components::ConversationInitiator {
            timer: Timer::from_seconds(CONVERSATION_CHECK_INTERVAL, TimerMode::Repeating),
        },
        crate::systems::logistics::Inventory::default(),
    ));

    info!("SPAWN: {} ({:?}) at {:?}", soul_name, gender, actual_pos);
}
