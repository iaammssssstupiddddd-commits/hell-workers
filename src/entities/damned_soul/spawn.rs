//! ソウルのスポーン関連システム

use super::*;
use crate::assets::GameAssets;
use crate::constants::*;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::world::map::{RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MIN, WorldMap};
use rand::Rng;
use std::env;

/// Soul の人口管理状態
#[derive(Resource)]
pub struct PopulationManager {
    pub current_count: u32,
    pub population_cap: u32,
    pub total_spawned: u32,
    pub total_escaped: u32,
    pub escape_cooldown_remaining: f32,
    spawn_timer: Timer,
}

impl Default for PopulationManager {
    fn default() -> Self {
        Self {
            current_count: 0,
            population_cap: SOUL_POPULATION_BASE_CAP,
            total_spawned: 0,
            total_escaped: 0,
            escape_cooldown_remaining: 0.0,
            spawn_timer: Timer::from_seconds(SOUL_SPAWN_INTERVAL, TimerMode::Repeating),
        }
    }
}

impl PopulationManager {
    pub fn can_start_escape(&self) -> bool {
        self.escape_cooldown_remaining <= f32::EPSILON
    }

    pub fn start_escape_cooldown(&mut self) {
        self.escape_cooldown_remaining = SOUL_ESCAPE_GLOBAL_COOLDOWN;
    }
}

fn parse_spawn_souls_from_args() -> Option<u32> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--spawn-souls" {
            let value = args.next()?;
            if let Ok(parsed) = value.parse::<u32>() {
                return Some(parsed);
            }
        }
    }
    None
}

fn initial_spawn_count() -> u32 {
    parse_spawn_souls_from_args().unwrap_or_else(|| {
        env::var("HW_SPAWN_SOULS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(SOUL_SPAWN_INITIAL)
    })
}

fn pick_river_south_bank_spawn(world_map: &WorldMap, rng: &mut impl Rng) -> Option<Vec2> {
    let south_y_max = RIVER_Y_MIN - 1;
    let south_y_min = 5; // マップ端の少し内側から開始

    for _ in 0..64 {
        let x = rng.gen_range(RIVER_X_MIN..=RIVER_X_MAX);
        let y = rng.gen_range(south_y_min..=south_y_max);
        if world_map.is_walkable(x, y) {
            return Some(WorldMap::grid_to_world(x, y));
        }
    }

    for _ in 0..256 {
        let x = rng.gen_range(RIVER_X_MIN..=RIVER_X_MAX);
        let y = rng.gen_range(south_y_min..=south_y_max);
        if world_map.is_walkable(x, y) {
            return Some(WorldMap::grid_to_world(x, y));
        }
    }

    None
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
    world_map: Res<WorldMap>,
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
    world_map: Res<WorldMap>,
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
        crate::systems::visual::speech::components::SoulEmotionState::default(),
        crate::systems::visual::speech::conversation::components::ConversationInitiator {
            timer: Timer::from_seconds(CONVERSATION_CHECK_INTERVAL, TimerMode::Repeating),
        },
        crate::systems::logistics::Inventory::default(),
    ));

    info!("SPAWN: {} ({:?}) at {:?}", soul_name, gender, actual_pos);
}
