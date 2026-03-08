//! スタートアップ関連のプラグイン
//!
//! Phase 5: 責務を分割し、システム配線 + 呼び出しに集中。

mod asset_catalog;
mod perf_scenario;

use asset_catalog::create_game_assets;
use perf_scenario::{
    PerfScenarioApplied, setup_perf_scenario_if_enabled, setup_perf_scenario_runtime_if_enabled,
};

use crate::assets::GameAssets;
use crate::entities::damned_soul::{DamnedSoulSpawnEvent, spawn_damned_souls};
use crate::entities::familiar::FamiliarSpawnEvent;
use crate::app_contexts::{BuildContext, CompanionPlacementState, MoveContext, MovePlacementState, TaskContext, ZoneContext};
use crate::interface::camera::{MainCamera, PanCamera};
use crate::interface::selection::{HoveredEntity, SelectedEntity};
use crate::interface::ui::{MenuState, components::ArchitectCategoryState, setup_ui};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::{
    ResourceCountDisplayTimer, ResourceLabels, initial_resource_spawner,
};
use crate::systems::spatial::{FloorConstructionSpatialGrid, GatheringSpotSpatialGrid};
use hw_spatial::{
    BlueprintSpatialGrid, FamiliarSpatialGrid, ResourceSpatialGrid, SpatialGrid,
    StockpileSpatialGrid,
};
use hw_spatial::SpatialGridOps;
use crate::systems::time::GameTime;
use crate::world::map::{
    WorldMap, WorldMapRead, WorldMapWrite, spawn_map, terrain_border::spawn_terrain_borders,
};
use bevy::prelude::*;

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
                    spawn_map_timed,
                    spawn_terrain_borders_if_enabled,
                    initial_resource_spawner_timed,
                    spawn_entities,
                    spawn_familiar_wrapper,
                    setup_perf_scenario_if_enabled,
                    setup_ui,
                    populate_resource_spatial_grid,
                )
                    .chain(),
            )
            .add_systems(Update, setup_perf_scenario_runtime_if_enabled);
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
) {
    // camera/resources 初期化
        commands.spawn((Camera2d, MainCamera, PanCamera::default()));

    // asset catalog 生成
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
        matches!(v.as_str(), "1" | "true" | "TRUE" | "on" | "ON" | "yes" | "YES")
    }) {
        return true;
    }

    std::env::args().any(|arg| arg == "--disable-terrain-borders")
}

fn populate_resource_spatial_grid(
    mut resource_grid: ResMut<ResourceSpatialGrid>,
    q_resources: Query<
        (Entity, &Transform, Option<&Visibility>),
        With<ResourceItem>,
    >,
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
