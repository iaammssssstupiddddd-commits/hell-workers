//! 使い魔のスポーン

use bevy::prelude::*;
use rand::Rng;

use crate::assets::GameAssets;
use crate::entities::damned_soul::{Destination, Path};
use crate::entities::spawn_args;
use crate::plugins::startup::{PerfScenarioConfig, PerfScenarioRandomStreams};
use crate::world::map::{WorldMap, WorldMapRead};
use hw_core::constants::*;
#[cfg(feature = "profiling")]
use hw_core::simulation_rng::SimulationRandomState;
use hw_world::find_nearby_walkable_grid;

use super::components::*;
use hw_visual::speech::FamiliarVoice;

/// 使い魔のスポーンイベント
#[derive(Message)]
pub struct FamiliarSpawnEvent {
    pub position: Vec2,
    pub familiar_type: FamiliarType,
    /// 固定 step 監査でだけ actor-local RNG に使う fixture spawn 順。
    pub simulation_random_key: Option<u64>,
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct FamiliarSpawnParams<'w> {
    game_assets: Res<'w, GameAssets>,
    world_map: WorldMapRead<'w>,
    color_allocator: ResMut<'w, FamiliarColorAllocator>,
    perf_config: Res<'w, PerfScenarioConfig>,
    perf_rngs: ResMut<'w, PerfScenarioRandomStreams>,
}

struct FamiliarSpawnInput {
    position: Vec2,
    familiar_type: FamiliarType,
    color_index: u32,
    voice: FamiliarVoice,
    simulation_random_key: Option<u64>,
}

struct FamiliarShellInput<'a> {
    entity: Entity,
    name: &'a str,
    command_radius: f32,
    position: Vec2,
    voice: FamiliarVoice,
}

/// 使い魔をスポーンする
pub fn spawn_familiar(
    mut spawn_events: MessageWriter<FamiliarSpawnEvent>,
    perf_config: Res<PerfScenarioConfig>,
    mut perf_rngs: ResMut<PerfScenarioRandomStreams>,
) {
    let spawn_count = if perf_config.enabled() {
        perf_config.familiar_count as usize
    } else {
        spawn_args::parse_spawn_count_from_args_or_env("--spawn-familiars", "HW_SPAWN_FAMILIARS", 2)
            as usize
    };

    if perf_config.enabled() {
        queue_familiar_spawn_events(
            &mut spawn_events,
            spawn_count,
            &mut perf_rngs.familiars,
            perf_config.uses_fixed_timesteps().then_some(0),
        );
    } else {
        let mut rng = rand::thread_rng();
        queue_familiar_spawn_events(&mut spawn_events, spawn_count, &mut rng, None);
    }

    info!("SPAWN_CONFIG: Familiars requested={spawn_count}");
}

fn queue_familiar_spawn_events(
    spawn_events: &mut MessageWriter<FamiliarSpawnEvent>,
    spawn_count: usize,
    rng: &mut impl Rng,
    simulation_random_key_start: Option<u64>,
) {
    for spawn_index in 0..spawn_count {
        let x = rng.gen_range(-120.0..120.0);
        let y = rng.gen_range(-120.0..120.0);
        spawn_events.write(FamiliarSpawnEvent {
            position: Vec2::new(x, y),
            familiar_type: FamiliarType::Imp,
            simulation_random_key: simulation_random_key_start
                .map(|start| start.wrapping_add(spawn_index as u64)),
        });
    }
}

/// 使い魔のスポーンを処理するシステム
pub fn familiar_spawning_system(
    mut commands: Commands,
    mut spawn_events: MessageReader<FamiliarSpawnEvent>,
    mut params: FamiliarSpawnParams,
) {
    for event in spawn_events.read() {
        let color_index = params.color_allocator.0 % 4;
        params.color_allocator.0 += 1;
        let voice = if params.perf_config.enabled() {
            FamiliarVoice::from_rng(&mut params.perf_rngs.familiar_voices)
        } else {
            FamiliarVoice::random()
        };

        spawn_familiar_at(
            &mut commands,
            &params.game_assets,
            params.world_map.as_ref(),
            FamiliarSpawnInput {
                position: event.position,
                familiar_type: event.familiar_type,
                color_index,
                voice,
                simulation_random_key: params
                    .perf_config
                    .uses_fixed_timesteps()
                    .then_some(event.simulation_random_key)
                    .flatten(),
            },
        );
    }
}

/// 指定座標に使い魔をスポーンする
fn spawn_familiar_at(
    commands: &mut Commands,
    game_assets: &GameAssets,
    world_map: &WorldMap,
    input: FamiliarSpawnInput,
) {
    let spawn_grid = WorldMap::world_to_grid(input.position);
    let actual_grid = find_nearby_walkable_grid(spawn_grid, world_map, 3);
    let actual_pos = WorldMap::grid_to_world(actual_grid.0, actual_grid.1);

    let familiar = Familiar::new(input.familiar_type, input.color_index);
    let familiar_name = familiar.name.clone();
    let command_radius = familiar.command_radius;

    #[cfg(not(feature = "profiling"))]
    let _ = input.simulation_random_key;

    let fam_entity = commands
        .spawn((
            familiar,
            FamiliarOperation::default(),
            FamiliarPolicy::default(),
            hw_core::relationships::Commanding::default(),
            hw_core::relationships::ManagedTasks::default(),
            Transform::from_xyz(actual_pos.x, actual_pos.y, Z_CHARACTER + 0.5),
        ))
        .id();
    #[cfg(feature = "profiling")]
    if let Some(key) = input.simulation_random_key {
        commands
            .entity(fam_entity)
            .insert(SimulationRandomState::new(key));
    }

    attach_familiar_shell_with_voice(
        commands,
        FamiliarShellInput {
            entity: fam_entity,
            name: &familiar_name,
            command_radius,
            position: actual_pos,
            voice: input.voice,
        },
        game_assets,
    );

    info!(
        "SPAWN: Familiar '{}' ({:?}) at {:?}",
        familiar_name, input.familiar_type, actual_pos
    );
}

/// 使い魔の「シェル」を付与する: セーブ対象外の実行時コンポーネント
/// （AI 状態・アニメーション・移動・foreground Sprite）と随伴の
/// 指揮範囲インジケーター。Familiarは3D scene rootを持たない。
///
/// spawn 時とセーブデータのロード後（rehydrate）の両方から呼ばれる。
/// 永続化される simulation 状態（`Familiar` / `FamiliarOperation` /
/// `FamiliarPolicy` / `Commanding` / `ManagedTasks` / `Transform`）はここに
/// 含めないこと（`systems/save/schema.rs` の allow-list 参照）。
pub fn attach_familiar_shell(
    commands: &mut Commands,
    fam_entity: Entity,
    familiar_name: &str,
    command_radius: f32,
    pos: Vec2,
    game_assets: &GameAssets,
) {
    attach_familiar_shell_with_voice(
        commands,
        FamiliarShellInput {
            entity: fam_entity,
            name: familiar_name,
            command_radius,
            position: pos,
            voice: FamiliarVoice::random(),
        },
        game_assets,
    );
}

fn attach_familiar_shell_with_voice(
    commands: &mut Commands,
    input: FamiliarShellInput<'_>,
    game_assets: &GameAssets,
) {
    commands.entity(input.entity).insert((
        Name::new(input.name.to_string()),
        ActiveCommand::default(),
        crate::systems::familiar_ai::FamiliarAiState::default(),
        hw_familiar_ai::familiar_ai::perceive::state_detection::FamiliarAiStateHistory::default(),
        Destination(input.position),
        Path::default(),
        FamiliarAnimation::default(),
        input.voice,
        // Sprite は logical root ではなく child に置く。root は movement /
        // spatial index 用の translation と hierarchy visibility だけを持つ。
        Visibility::Inherited,
    ));

    let visual_child = commands
        .spawn((
            hw_visual::FamiliarVisualOwner {
                owner: input.entity,
            },
            hw_visual::FamiliarVisualOffset::default(),
            Sprite {
                image: game_assets.familiar.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.9)),
                color: Color::WHITE,
                ..default()
            },
            Transform::default(),
            Name::new(format!("FamiliarVisual: {}", input.name)),
        ))
        .id();
    commands.entity(input.entity).add_child(visual_child);

    commands.spawn((
        FamiliarRangeIndicator(input.entity),
        AuraLayer::Border,
        Sprite {
            image: game_assets.aura_circle.clone(),
            color: Color::srgba(1.0, 0.3, 0.0, 0.3),
            custom_size: Some(Vec2::splat(input.command_radius * 2.0)),
            ..default()
        },
        Transform::from_translation(input.position.extend(Z_AURA)),
    ));

    commands.spawn((
        FamiliarRangeIndicator(input.entity),
        AuraLayer::Outline,
        Sprite {
            image: game_assets.aura_ring.clone(),
            color: Color::srgba(1.0, 1.0, 0.0, 0.0),
            custom_size: Some(Vec2::splat(input.command_radius * 2.0)),
            ..default()
        },
        Transform::from_translation(input.position.extend(Z_AURA + 0.01)),
    ));

    commands.spawn((
        FamiliarAura { pulse_timer: 0.0 },
        FamiliarRangeIndicator(input.entity),
        AuraLayer::Pulse,
        Sprite {
            image: game_assets.aura_circle.clone(),
            color: Color::srgba(1.0, 0.6, 0.0, 0.15),
            custom_size: Some(Vec2::splat(input.command_radius * 1.8)),
            ..default()
        },
        Transform::from_translation(input.position.extend(Z_AURA + 0.03)),
    ));
}

#[cfg(test)]
mod tests {
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::ecs::world::CommandQueue;
    use bevy::prelude::*;
    use hw_core::familiar::{FamiliarWorkPriority, FamiliarWorkRule};
    use hw_core::jobs::WorkType;
    use hw_core::relationships::{Commanding, ManagedTasks};

    use super::*;
    use crate::plugins::startup::create_game_assets;

    #[test]
    fn repeated_shell_attach_does_not_overwrite_durable_familiar_settings() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Image>()
            .init_asset::<Font>()
            .init_asset::<Gltf>()
            .init_asset::<WorldAsset>();

        let asset_server = app.world().resource::<AssetServer>().clone();
        let game_assets = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            create_game_assets(&asset_server, &mut images)
        };
        let expected_operation = FamiliarOperation {
            fatigue_threshold: 0.7,
            max_controlled_soul: 5,
        };
        let mut expected_policy = FamiliarPolicy::default();
        expected_policy.set_rule(
            WorkType::Mine,
            FamiliarWorkRule {
                allowed: false,
                priority: FamiliarWorkPriority::High,
            },
        );
        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                expected_operation.clone(),
                expected_policy.clone(),
                Commanding::default(),
                ManagedTasks::default(),
                Transform::default(),
            ))
            .id();

        let mut command_queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut command_queue, app.world());
            for _ in 0..2 {
                attach_familiar_shell_with_voice(
                    &mut commands,
                    FamiliarShellInput {
                        entity: familiar,
                        name: "Saved Familiar",
                        command_radius: TILE_SIZE * 7.0,
                        position: Vec2::ZERO,
                        voice: FamiliarVoice::random(),
                    },
                    &game_assets,
                );
            }
        }
        command_queue.apply(app.world_mut());

        assert_eq!(
            app.world().get::<FamiliarOperation>(familiar),
            Some(&expected_operation)
        );
        assert_eq!(
            app.world().get::<FamiliarPolicy>(familiar),
            Some(&expected_policy)
        );
        assert_eq!(
            app.world()
                .get::<Commanding>(familiar)
                .unwrap()
                .iter()
                .count(),
            0
        );
        assert_eq!(
            app.world()
                .get::<ManagedTasks>(familiar)
                .unwrap()
                .iter()
                .count(),
            0
        );
    }
}
