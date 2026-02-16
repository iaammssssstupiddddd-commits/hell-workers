//! 使い魔のスポーン

use bevy::prelude::*;
use rand::Rng;

use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{Destination, Path};
use crate::entities::spawn_args;
use crate::entities::spawn_position;
use crate::world::map::WorldMap;

use super::components::*;
use super::voice::FamiliarVoice;

/// 使い魔のスポーンイベント
#[derive(Message)]
pub struct FamiliarSpawnEvent {
    pub position: Vec2,
    pub familiar_type: FamiliarType,
}

/// 使い魔をスポーンする
pub fn spawn_familiar(mut spawn_events: MessageWriter<FamiliarSpawnEvent>) {
    let spawn_count = spawn_args::parse_spawn_count_from_args_or_env(
        "--spawn-familiars",
        "HW_SPAWN_FAMILIARS",
        2,
    ) as usize;

    let mut rng = rand::thread_rng();
    for _ in 0..spawn_count {
        let x = rng.gen_range(-120.0..120.0);
        let y = rng.gen_range(-120.0..120.0);
        spawn_events.write(FamiliarSpawnEvent {
            position: Vec2::new(x, y),
            familiar_type: FamiliarType::Imp,
        });
    }

    info!("SPAWN_CONFIG: Familiars requested={}", spawn_count);
}

/// 使い魔のスポーンを処理するシステム
pub fn familiar_spawning_system(
    mut commands: Commands,
    mut spawn_events: MessageReader<FamiliarSpawnEvent>,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
    mut color_allocator: ResMut<FamiliarColorAllocator>,
) {
    for event in spawn_events.read() {
        let color_index = color_allocator.0 % 4;
        color_allocator.0 += 1;

        spawn_familiar_at(
            &mut commands,
            &game_assets,
            &world_map,
            event.position,
            event.familiar_type,
            color_index,
        );
    }
}

/// 指定座標に使い魔をスポーンする
pub fn spawn_familiar_at(
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    world_map: &Res<WorldMap>,
    pos: Vec2,
    familiar_type: FamiliarType,
    color_index: u32,
) {
    let spawn_grid = WorldMap::world_to_grid(pos);
    let actual_grid = spawn_position::find_nearby_walkable_grid(spawn_grid, world_map, 3);
    let actual_pos = WorldMap::grid_to_world(actual_grid.0, actual_grid.1);

    let familiar = Familiar::new(familiar_type, color_index);
    let familiar_name = familiar.name.clone();
    let command_radius = familiar.command_radius;

    let fam_entity = commands
        .spawn((
            familiar,
            Name::new(familiar_name.clone()),
            FamiliarOperation::default(),
            ActiveCommand::default(),
            crate::systems::familiar_ai::FamiliarAiState::default(),
            crate::systems::familiar_ai::perceive::state_detection::FamiliarAiStateHistory::default(
            ),
            crate::relationships::Commanding::default(),
            crate::relationships::ManagedTasks::default(),
            Destination(actual_pos),
            Path::default(),
            FamiliarAnimation::default(),
            FamiliarVoice::random(),
            Sprite {
                image: game_assets.familiar.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.9)),
                color: Color::WHITE,
                ..default()
            },
            Transform::from_xyz(actual_pos.x, actual_pos.y, Z_CHARACTER + 0.5),
        ))
        .id();

    commands.spawn((
        FamiliarRangeIndicator(fam_entity),
        AuraLayer::Border,
        Sprite {
            image: game_assets.aura_circle.clone(),
            color: Color::srgba(1.0, 0.3, 0.0, 0.3),
            custom_size: Some(Vec2::splat(command_radius * 2.0)),
            ..default()
        },
        Transform::from_translation(actual_pos.extend(Z_AURA)),
    ));

    commands.spawn((
        FamiliarRangeIndicator(fam_entity),
        AuraLayer::Outline,
        Sprite {
            image: game_assets.aura_ring.clone(),
            color: Color::srgba(1.0, 1.0, 0.0, 0.0),
            custom_size: Some(Vec2::splat(command_radius * 2.0)),
            ..default()
        },
        Transform::from_translation(actual_pos.extend(Z_AURA + 0.01)),
    ));

    commands.spawn((
        FamiliarAura { pulse_timer: 0.0 },
        FamiliarRangeIndicator(fam_entity),
        AuraLayer::Pulse,
        Sprite {
            image: game_assets.aura_circle.clone(),
            color: Color::srgba(1.0, 0.6, 0.0, 0.15),
            custom_size: Some(Vec2::splat(command_radius * 1.8)),
            ..default()
        },
        Transform::from_translation(actual_pos.extend(Z_AURA + 0.03)),
    ));

    info!(
        "SPAWN: Familiar '{}' ({:?}) at {:?}",
        familiar_name, familiar_type, actual_pos
    );
}
