//! 使い魔のスポーン

use bevy::prelude::*;
use rand::Rng;

use crate::assets::GameAssets;
use crate::entities::damned_soul::{Destination, Path};
use crate::entities::spawn_args;
use crate::world::map::{WorldMap, WorldMapRead};
use hw_core::constants::*;
use hw_world::find_nearby_walkable_grid;

use super::components::*;
use hw_visual::speech::FamiliarVoice;

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
    handles_3d: Res<crate::plugins::startup::Building3dHandles>,
    world_map: WorldMapRead,
    mut color_allocator: ResMut<FamiliarColorAllocator>,
) {
    for event in spawn_events.read() {
        let color_index = color_allocator.0 % 4;
        color_allocator.0 += 1;

        spawn_familiar_at(
            &mut commands,
            &game_assets,
            &handles_3d,
            world_map.as_ref(),
            event.position,
            event.familiar_type,
            color_index,
        );
    }
}

/// 指定座標に使い魔をスポーンする
pub fn spawn_familiar_at(
    commands: &mut Commands,
    game_assets: &GameAssets,
    handles_3d: &crate::plugins::startup::Building3dHandles,
    world_map: &WorldMap,
    pos: Vec2,
    familiar_type: FamiliarType,
    color_index: u32,
) {
    let spawn_grid = WorldMap::world_to_grid(pos);
    let actual_grid = find_nearby_walkable_grid(spawn_grid, world_map, 3);
    let actual_pos = WorldMap::grid_to_world(actual_grid.0, actual_grid.1);

    let familiar = Familiar::new(familiar_type, color_index);
    let familiar_name = familiar.name.clone();
    let command_radius = familiar.command_radius;

    let fam_entity = commands
        .spawn((
            familiar,
            hw_core::relationships::Commanding::default(),
            hw_core::relationships::ManagedTasks::default(),
            Transform::from_xyz(actual_pos.x, actual_pos.y, Z_CHARACTER + 0.5),
        ))
        .id();

    attach_familiar_shell(
        commands,
        fam_entity,
        &familiar_name,
        command_radius,
        actual_pos,
        game_assets,
        handles_3d,
    );

    info!(
        "SPAWN: Familiar '{}' ({:?}) at {:?}",
        familiar_name, familiar_type, actual_pos
    );
}

/// 使い魔の「シェル」を付与する: セーブ対象外の実行時コンポーネント
/// （AI 状態・アニメーション・移動・Sprite）と随伴エンティティ
/// （3D プロキシ・指揮範囲インジケーター）。
///
/// spawn 時とセーブデータのロード後（rehydrate）の両方から呼ばれる。
/// 永続化される simulation 状態（`Familiar` / `Commanding` / `ManagedTasks` /
/// `Transform`）はここに含めないこと（`systems/save/saving.rs` の allow-list 参照）。
pub fn attach_familiar_shell(
    commands: &mut Commands,
    fam_entity: Entity,
    familiar_name: &str,
    command_radius: f32,
    pos: Vec2,
    game_assets: &GameAssets,
    handles_3d: &crate::plugins::startup::Building3dHandles,
) {
    commands.entity(fam_entity).insert((
        Name::new(familiar_name.to_string()),
        FamiliarOperation::default(),
        ActiveCommand::default(),
        crate::systems::familiar_ai::FamiliarAiState::default(),
        hw_familiar_ai::familiar_ai::perceive::state_detection::FamiliarAiStateHistory::default(),
        Destination(pos),
        Path::default(),
        FamiliarAnimation::default(),
        FamiliarVoice::random(),
        Sprite {
            image: game_assets.familiar.clone(),
            custom_size: Some(Vec2::splat(TILE_SIZE * 0.9)),
            color: Color::WHITE,
            ..default()
        },
    ));

    // 3D プロキシ（Phase 2 プレースホルダー）
    commands.spawn((
        Mesh3d(handles_3d.familiar_mesh.clone()),
        MeshMaterial3d(handles_3d.familiar_material.clone()),
        Transform::from_xyz(pos.x, TILE_SIZE * 0.45, -pos.y),
        bevy::camera::visibility::RenderLayers::layer(LAYER_3D),
        hw_visual::visual3d::FamiliarProxy3d { owner: fam_entity },
        Name::new(format!("FamiliarProxy3d: {}", familiar_name)),
    ));

    commands.spawn((
        FamiliarRangeIndicator(fam_entity),
        AuraLayer::Border,
        Sprite {
            image: game_assets.aura_circle.clone(),
            color: Color::srgba(1.0, 0.3, 0.0, 0.3),
            custom_size: Some(Vec2::splat(command_radius * 2.0)),
            ..default()
        },
        Transform::from_translation(pos.extend(Z_AURA)),
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
        Transform::from_translation(pos.extend(Z_AURA + 0.01)),
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
        Transform::from_translation(pos.extend(Z_AURA + 0.03)),
    ));
}
