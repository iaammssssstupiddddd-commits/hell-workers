//! スタートアップ関連のプラグイン
//!
//! Phase 5: 責務を分割し、システム配線 + 呼び出しに集中。

mod asset_catalog;
mod perf_scenario;
mod rtt_composite;
mod rtt_setup;
mod visual_handles;

pub use rtt_composite::RttCompositeSprite;
pub use rtt_setup::{Camera3dRtt, RttTextures, RttViewportSize};
pub use visual_handles::Building3dHandles;

use asset_catalog::create_game_assets;
use perf_scenario::{
    PerfScenarioApplied, setup_perf_scenario_if_enabled, setup_perf_scenario_runtime_if_enabled,
};

use crate::app_contexts::{
    BuildContext, CompanionPlacementState, MoveContext, MovePlacementState, TaskContext,
    ZoneContext,
};
use crate::assets::GameAssets;
use crate::entities::damned_soul::{DamnedSoulSpawnEvent, spawn_damned_souls};
use crate::entities::familiar::FamiliarSpawnEvent;
use crate::interface::selection::{HoveredEntity, SelectedEntity};
use crate::interface::ui::{MenuState, setup_ui};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::{
    ResourceCountDisplayTimer, ResourceLabels, initial_resource_spawner,
};
use crate::systems::spatial::{FloorConstructionSpatialGrid, GatheringSpotSpatialGrid};
use crate::systems::visual::elevation_view::ElevationDirection;
use crate::world::map::{
    WorldMap, WorldMapRead, WorldMapWrite, spawn_map, terrain_border::spawn_terrain_borders,
};
use bevy::camera::{RenderTarget, visibility::RenderLayers};
use bevy::camera_controller::pan_camera::PanCamera;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::GameTime;
use hw_core::constants::{LAYER_2D, LAYER_3D, LAYER_OVERLAY, VIEW_HEIGHT, Z_OFFSET};
use hw_spatial::SpatialGridOps;
use hw_spatial::{
    BlueprintSpatialGrid, FamiliarSpatialGrid, ResourceSpatialGrid, SpatialGrid,
    StockpileSpatialGrid,
};
use hw_ui::camera::MainCamera;
use hw_ui::components::ArchitectCategoryState;

pub struct StartupPlugin;

impl Plugin for StartupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldMap>()
            .init_resource::<SelectedEntity>()
            .init_resource::<HoveredEntity>()
            .init_resource::<MenuState>()
            .init_resource::<ArchitectCategoryState>()
            .init_resource::<BuildContext>()
            .init_resource::<MoveContext>()
            .init_resource::<MovePlacementState>()
            .init_resource::<ZoneContext>()
            .init_resource::<CompanionPlacementState>()
            .init_resource::<ResourceLabels>()
            .init_resource::<ResourceCountDisplayTimer>()
            .init_resource::<GameTime>()
            .init_resource::<TaskContext>()
            .init_resource::<SpatialGrid>()
            .init_resource::<FamiliarSpatialGrid>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<GatheringSpotSpatialGrid>()
            .init_resource::<BlueprintSpatialGrid>()
            .init_resource::<FloorConstructionSpatialGrid>()
            .init_resource::<StockpileSpatialGrid>()
            .init_resource::<PerfScenarioApplied>()
            .add_systems(Startup, (setup, initialize_gizmo_config))
            .add_systems(
                PostStartup,
                (
                    visual_handles::init_visual_handles,
                    spawn_map_timed,
                    spawn_terrain_borders_if_enabled,
                    initial_resource_spawner_timed,
                    spawn_entities,
                    spawn_familiar_wrapper,
                    setup_perf_scenario_if_enabled,
                    setup_ui,
                    crate::interface::ui::dev_panel::spawn_dev_panel_system,
                    populate_resource_spatial_grid,
                    rtt_composite::spawn_rtt_composite_sprite,
                )
                    .chain(),
            )
            .add_systems(Update, (
                setup_perf_scenario_runtime_if_enabled,
                (
                    rtt_setup::sync_rtt_texture_size_to_window,
                    rtt_composite::sync_rtt_output_bindings,
                )
                    .chain(),
            ));
    }
}

fn spawn_map_timed(commands: Commands, game_assets: Res<GameAssets>, world_map: WorldMapWrite) {
    spawn_map(commands, game_assets, world_map);
}

fn initial_resource_spawner_timed(
    commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: WorldMapWrite,
) {
    initial_resource_spawner(commands, game_assets, world_map);
}

/// Phase 5: camera/resources 初期化 + asset catalog 生成を呼び出す
fn setup(
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
    commands.insert_resource(RttTextures { texture_3d: rtt_handle.clone() });
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

fn initialize_gizmo_config(mut config_store: ResMut<GizmoConfigStore>) {
    for (_, config, _) in config_store.iter_mut() {
        config.enabled = false;
        config.line.width = 1.0;
    }
}

fn spawn_terrain_borders_if_enabled(
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

fn populate_resource_spatial_grid(
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

fn spawn_entities(spawn_events: MessageWriter<DamnedSoulSpawnEvent>, world_map: WorldMapRead) {
    spawn_damned_souls(spawn_events, world_map);
}

fn spawn_familiar_wrapper(spawn_events: MessageWriter<FamiliarSpawnEvent>) {
    crate::entities::familiar::spawn_familiar(spawn_events);
}
