use crate::assets::GameAssets;
use crate::entities::damned_soul::{DamnedSoulSpawnEvent, spawn_damned_souls};
use crate::entities::familiar::FamiliarSpawnEvent;
use crate::systems::logistics::{ResourceItem, initial_resource_spawner};
use crate::systems::visual::elevation_view::ElevationDirection;
use crate::world::map::{
    WorldMapRead, WorldMapWrite, spawn_map, terrain_border::spawn_terrain_borders,
};
use bevy::camera::{RenderTarget, visibility::RenderLayers};
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::constants::{LAYER_2D, LAYER_3D, LAYER_OVERLAY, VIEW_HEIGHT, Z_OFFSET};
use hw_spatial::{ResourceSpatialGrid, SpatialGridOps};
use hw_ui::camera::MainCamera;

use super::asset_catalog::create_game_assets;
use super::rtt_setup::{self, Camera3dRtt, RttTextures};

pub(super) fn spawn_map_timed(
    commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: WorldMapWrite,
) {
    spawn_map(commands, game_assets, world_map);
}

pub(super) fn initial_resource_spawner_timed(
    commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: WorldMapWrite,
) {
    initial_resource_spawner(commands, game_assets, world_map);
}

/// Phase 5: camera/resources 初期化 + asset catalog 生成を呼び出す
pub(super) fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
) {
    // --- RtT オフスクリーンテクスチャ生成 ---
    let viewport_size = q_window
        .single()
        .map(rtt_setup::RttViewportSize::from_window)
        .unwrap_or(rtt_setup::RttViewportSize {
            width: 1280,
            height: 720,
        });
    let rtt_handle =
        rtt_setup::create_rtt_texture(viewport_size.width, viewport_size.height, &mut images);
    commands.insert_resource(RttTextures {
        texture_3d: rtt_handle.clone(),
    });
    commands.insert_resource(viewport_size);

    // --- Camera2d（既存: メイン描画・スクリーン出力） ---
    commands.spawn((
        Camera2d,
        MainCamera,
        PanCamera::default(),
        RenderLayers::layer(LAYER_2D),
    ));

    // --- Overlay Camera（常時アクティブ: RtT composite sprite 専用）---
    // 矢視モードで MainCamera を無効化しても composite sprite を表示し続ける。
    // order=1: MainCamera(order=0) の後に描画することで上書き合成する。
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            ..default()
        },
        RenderLayers::layer(LAYER_OVERLAY),
    ));

    // --- Camera3d（RtT: オフスクリーン3D描画）---
    // TopDown の初期値は camera_sync.rs の定数と揃える。
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection::default_3d()),
        {
            let mut transform = Transform::from_translation(Vec3::new(0.0, VIEW_HEIGHT, Z_OFFSET));
            transform.rotation = ElevationDirection::TopDown.camera_rotation();
            transform
        },
        RenderTarget::Image(rtt_handle.into()),
        RenderLayers::layer(LAYER_3D),
        Camera3dRtt,
    ));

    // --- asset catalog 生成 ---
    let game_assets = create_game_assets(&asset_server, &mut *images);
    commands.insert_resource(game_assets);
}

pub(super) fn initialize_gizmo_config(mut config_store: ResMut<GizmoConfigStore>) {
    for (_, config, _) in config_store.iter_mut() {
        config.enabled = false;
        config.line.width = 1.0;
    }
}

pub(super) fn spawn_terrain_borders_if_enabled(
    commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: WorldMapRead,
) {
    if skip_terrain_borders() {
        info!("STARTUP: terrain borders spawn skipped");
        return;
    }

    spawn_terrain_borders(commands, game_assets, world_map);
}

fn skip_terrain_borders() -> bool {
    if std::env::var("HW_DISABLE_TERRAIN_BORDERS").is_ok_and(|v| {
        matches!(
            v.as_str(),
            "1" | "true" | "TRUE" | "on" | "ON" | "yes" | "YES"
        )
    }) {
        return true;
    }

    std::env::args().any(|arg| arg == "--disable-terrain-borders")
}

pub(super) fn populate_resource_spatial_grid(
    mut resource_grid: ResMut<ResourceSpatialGrid>,
    q_resources: Query<(Entity, &Transform, Option<&Visibility>), With<ResourceItem>>,
) {
    for (entity, transform, visibility) in q_resources.iter() {
        let should_register = visibility
            .map(|v| *v != bevy::prelude::Visibility::Hidden)
            .unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
        }
    }
}

pub(super) fn spawn_entities(
    spawn_events: MessageWriter<DamnedSoulSpawnEvent>,
    world_map: WorldMapRead,
) {
    spawn_damned_souls(spawn_events, world_map);
}

pub(super) fn spawn_familiar_wrapper(spawn_events: MessageWriter<FamiliarSpawnEvent>) {
    crate::entities::familiar::spawn_familiar(spawn_events);
}
