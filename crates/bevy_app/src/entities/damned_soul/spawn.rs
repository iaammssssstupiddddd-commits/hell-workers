//! ソウルのスポーン関連システム

use super::*;
use crate::entities::spawn_args;
use crate::plugins::startup::{PerfScenarioConfig, PerfScenarioRandomStreams};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::world::map::{RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MIN, WorldMap, WorldMapRead};
use hw_core::constants::*;
#[cfg(feature = "profiling")]
use hw_core::simulation_rng::SimulationRandomState;
use hw_core::visual_mirror::logistics::InventoryItemVisual;
use hw_core::visual_mirror::task::SoulTaskVisualState;
use hw_world::find_nearby_walkable_grid;
use rand::Rng;

pub use hw_core::population::PopulationManager;

fn initial_spawn_count(perf_config: &PerfScenarioConfig) -> u32 {
    if perf_config.enabled() {
        perf_config.soul_count
    } else {
        spawn_args::parse_spawn_count_from_args_or_env(
            "--spawn-souls",
            "HW_SPAWN_SOULS",
            SOUL_SPAWN_INITIAL,
        )
    }
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
    rng: &mut impl Rng,
    simulation_random_key_start: Option<u64>,
) -> u32 {
    let mut spawned = 0;

    for spawn_index in 0..count {
        if let Some(position) = pick_river_south_bank_spawn(world_map, rng) {
            spawn_events.write(DamnedSoulSpawnEvent {
                position,
                simulation_random_key: simulation_random_key_start
                    .map(|start| start.wrapping_add(u64::from(spawn_index))),
            });
            spawned += 1;
        }
    }

    spawned
}

/// 人間をスポーンする
pub fn spawn_damned_souls(
    mut spawn_events: MessageWriter<DamnedSoulSpawnEvent>,
    world_map: WorldMapRead,
    perf_config: Res<PerfScenarioConfig>,
    mut perf_rngs: ResMut<PerfScenarioRandomStreams>,
) {
    let spawn_count = initial_spawn_count(&perf_config);
    let spawned = if perf_config.enabled() {
        queue_river_spawn_events(
            &mut spawn_events,
            &world_map,
            spawn_count,
            &mut perf_rngs.souls,
            perf_config.uses_fixed_timesteps().then_some(0),
        )
    } else {
        let mut rng = rand::thread_rng();
        queue_river_spawn_events(&mut spawn_events, &world_map, spawn_count, &mut rng, None)
    };
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
    perf_config: Res<PerfScenarioConfig>,
) {
    if perf_config.enabled() {
        return;
    }

    let current = population.current_count;
    let cap = population.population_cap.max(SOUL_POPULATION_BASE_CAP);

    if current == 0 {
        let emergency = SOUL_SPAWN_INITIAL.min(cap.max(1));
        let mut rng = rand::thread_rng();
        let spawned =
            queue_river_spawn_events(&mut spawn_events, &world_map, emergency, &mut rng, None);
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

    let spawned =
        queue_river_spawn_events(&mut spawn_events, &world_map, spawn_count, &mut rng, None);
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
    handles_3d: Res<crate::plugins::startup::Building3dHandles>,
    world_map: WorldMapRead,
    perf_config: Res<PerfScenarioConfig>,
    mut perf_rngs: ResMut<PerfScenarioRandomStreams>,
) {
    for event in spawn_events.read() {
        let identity = if perf_config.enabled() {
            SoulIdentity::from_rng(&mut perf_rngs.soul_traits)
        } else {
            SoulIdentity::random()
        };
        spawn_damned_soul_at_with_identity(
            &mut commands,
            &handles_3d,
            world_map.as_ref(),
            event.position,
            identity,
            perf_config
                .uses_fixed_timesteps()
                .then_some(event.simulation_random_key)
                .flatten(),
            !perf_config.omits_3d_scene_roots(),
        );
    }
}

fn spawn_damned_soul_at_with_identity(
    commands: &mut Commands,
    handles_3d: &crate::plugins::startup::Building3dHandles,
    world_map: &WorldMap,
    pos: Vec2,
    identity: SoulIdentity,
    simulation_random_key: Option<u64>,
    spawn_3d_scene_roots: bool,
) {
    #[cfg(not(feature = "profiling"))]
    let _ = simulation_random_key;

    let spawn_grid = WorldMap::world_to_grid(pos);
    let actual_grid = find_nearby_walkable_grid(spawn_grid, world_map, 5);
    let actual_pos = WorldMap::grid_to_world(actual_grid.0, actual_grid.1);

    let soul_name = identity.name.clone();
    let gender = identity.gender;

    let soul_entity = commands
        .spawn((
            DamnedSoul::default(),
            DreamState::default(),
            identity,
            IdleState::default(),
            AssignedTask::default(),
            Transform::from_xyz(actual_pos.x, actual_pos.y, Z_CHARACTER),
            crate::systems::logistics::Inventory::default(),
        ))
        .id();
    #[cfg(feature = "profiling")]
    if let Some(key) = simulation_random_key {
        commands
            .entity(soul_entity)
            .insert(SimulationRandomState::new(key));
    }

    attach_soul_shell_with_scene_roots(
        commands,
        soul_entity,
        &soul_name,
        actual_pos,
        handles_3d,
        spawn_3d_scene_roots,
    );

    info!("SPAWN: {} ({:?}) at {:?}", soul_name, gender, actual_pos);
}

/// Soul の「シェル」を付与する: セーブ対象外の実行時コンポーネント
/// （ビジュアル・アニメーション・移動・UI リンク）と GLB 表示用の随伴エンティティ。
///
/// spawn 時とセーブデータのロード後（rehydrate）の両方から呼ばれる。
/// セーブ/ロードで永続化される simulation 状態（`DamnedSoul` / `IdleState` /
/// `Inventory` 等）はここに含めないこと（`systems/save/schema.rs` の allow-list 参照）。
pub fn attach_soul_shell(
    commands: &mut Commands,
    soul_entity: Entity,
    soul_name: &str,
    pos: Vec2,
    handles_3d: &crate::plugins::startup::Building3dHandles,
) {
    attach_soul_shell_with_scene_roots(commands, soul_entity, soul_name, pos, handles_3d, true);
}

/// `attach_soul_shell` の perf fixture 専用の内部変種。
///
/// セーブ再hydrateを含む公開APIは常に3D rootを付与する。CPU-only perfでは
/// scene 展開・proxy同期・RTT対象を計測に混ぜないため、spawn時だけ省略する。
fn attach_soul_shell_with_scene_roots(
    commands: &mut Commands,
    soul_entity: Entity,
    soul_name: &str,
    pos: Vec2,
    handles_3d: &crate::plugins::startup::Building3dHandles,
    spawn_3d_scene_roots: bool,
) {
    commands.entity(soul_entity).insert((
        SoulUiLinks::default(),
        Name::new(format!("Soul: {}", soul_name)),
        InventoryItemVisual::default(),
        SoulTaskVisualState::default(),
        // Mesh2d 子（例: DreamParticle）が InheritedVisibility を持つため、親にも Visibility が必要（Bevy B0004）。
        Visibility::Inherited,
        Destination(pos),
        Path::default(),
        AnimationState::default(),
        hw_visual::SoulAnimVisualState::default(),
        hw_visual::speech::components::SoulEmotionState::default(),
        hw_visual::speech::conversation::components::ConversationInitiator {
            timer: Timer::from_seconds(CONVERSATION_CHECK_INTERVAL, TimerMode::Repeating),
        },
    ));

    if !spawn_3d_scene_roots {
        return;
    }

    // Soul の通常表示は GLB SceneRoot を RtT に流し、2D Sprite は持たない。
    commands.spawn((
        WorldAssetRoot(handles_3d.soul_scene.clone()),
        Transform::from_xyz(pos.x, 0.0, -pos.y).with_scale(Vec3::splat(SOUL_GLB_SCALE)),
        bevy::camera::visibility::RenderLayers::layer(LAYER_3D),
        hw_visual::visual3d::SoulProxy3d {
            owner: soul_entity,
            billboard: false,
        },
        Name::new(format!("SoulProxy3d: {}", soul_name)),
    ));

    commands.spawn((
        WorldAssetRoot(handles_3d.soul_scene.clone()),
        Transform::from_xyz(pos.x, 0.0, -pos.y).with_scale(Vec3::splat(SOUL_GLB_SCALE)),
        bevy::camera::visibility::RenderLayers::layer(LAYER_3D_SOUL_MASK),
        hw_visual::visual3d::SoulMaskProxy3d { owner: soul_entity },
        Name::new(format!("SoulMaskProxy3d: {}", soul_name)),
    ));

    commands.spawn((
        WorldAssetRoot(handles_3d.soul_scene.clone()),
        Transform::from_xyz(pos.x, 0.0, -pos.y).with_scale(Vec3::splat(SOUL_GLB_SCALE)),
        bevy::camera::visibility::RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SOUL_SHADOW]),
        hw_visual::visual3d::SoulShadowProxy3d { owner: soul_entity },
        Name::new(format!("SoulShadowProxy3d: {}", soul_name)),
    ));
}
