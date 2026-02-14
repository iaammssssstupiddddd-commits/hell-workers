//! スタートアップ関連のプラグイン
//!
//! Phase 5: 責務を分割し、システム配線 + 呼び出しに集中。

mod asset_catalog;
mod perf_scenario;

use asset_catalog::create_game_assets;
use perf_scenario::{PerfScenarioApplied, setup_perf_scenario_runtime_if_enabled, setup_perf_scenario_if_enabled};

use crate::assets::GameAssets;
use crate::entities::damned_soul::{DamnedSoulSpawnEvent, spawn_damned_souls};
use crate::entities::familiar::{FamiliarSpawnEvent};
use crate::game_state::{BuildContext, CompanionPlacementState, TaskContext, ZoneContext};
use crate::interface::camera::{MainCamera, PanCamera};
use crate::interface::selection::{HoveredEntity, SelectedEntity};
use crate::interface::ui::{MenuState, setup_ui};
use crate::systems::logistics::{ResourceLabels, initial_resource_spawner};
use crate::systems::spatial::{
    BlueprintSpatialGrid, FamiliarSpatialGrid, GatheringSpotSpatialGrid, ResourceSpatialGrid,
    SpatialGrid, SpatialGridOps, StockpileSpatialGrid,
};
use crate::systems::time::GameTime;
use crate::world::map::{WorldMap, spawn_map};
use bevy::prelude::*;
use bevy::render::view::NoIndirectDrawing;
use std::time::Instant;

pub struct StartupPlugin;

impl Plugin for StartupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldMap>()
            .init_resource::<SelectedEntity>()
            .init_resource::<HoveredEntity>()
            .init_resource::<MenuState>()
            .init_resource::<BuildContext>()
            .init_resource::<ZoneContext>()
            .init_resource::<CompanionPlacementState>()
            .init_resource::<ResourceLabels>()
            .init_resource::<GameTime>()
            .init_resource::<TaskContext>()
            .init_resource::<SpatialGrid>()
            .init_resource::<FamiliarSpatialGrid>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<GatheringSpotSpatialGrid>()
            .init_resource::<BlueprintSpatialGrid>()
            .init_resource::<StockpileSpatialGrid>()
            .init_resource::<PerfScenarioApplied>()
            .add_systems(Startup, (setup, initialize_gizmo_config))
            .add_systems(
                PostStartup,
                (
                    log_post_startup_begin,
                    spawn_map_timed,
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

fn log_post_startup_begin() {
    info!("STARTUP_TIMING: PostStartup begin");
}

fn spawn_map_timed(commands: Commands, game_assets: Res<GameAssets>, world_map: ResMut<WorldMap>) {
    let start = Instant::now();
    spawn_map(commands, game_assets, world_map);
    info!(
        "STARTUP_TIMING: spawn_map finished in {} ms",
        start.elapsed().as_millis()
    );
}

fn initial_resource_spawner_timed(
    commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: ResMut<WorldMap>,
) {
    let start = Instant::now();
    initial_resource_spawner(commands, game_assets, world_map);
    info!(
        "STARTUP_TIMING: initial_resource_spawner finished in {} ms",
        start.elapsed().as_millis()
    );
}

/// Phase 5: camera/resources 初期化 + asset catalog 生成を呼び出す
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    let start = Instant::now();

    // camera/resources 初期化
    commands.spawn((
        Camera2d,
        MainCamera,
        PanCamera::default(),
        NoIndirectDrawing,
    ));

    // asset catalog 生成
    let game_assets = create_game_assets(&asset_server, &mut *images);
    commands.insert_resource(game_assets);

    info!(
        "STARTUP_TIMING: setup (camera + assets + resources) finished in {} ms",
        start.elapsed().as_millis()
    );
}

fn initialize_gizmo_config(mut config_store: ResMut<GizmoConfigStore>) {
    for (_, config, _) in config_store.iter_mut() {
        config.enabled = true;
        config.line.width = 1.0;
    }
}

fn populate_resource_spatial_grid(
    mut resource_grid: ResMut<ResourceSpatialGrid>,
    q_resources: Query<
        (Entity, &Transform, Option<&Visibility>),
        With<crate::systems::logistics::ResourceItem>,
    >,
) {
    let start = Instant::now();
    let mut registered_count = 0;
    for (entity, transform, visibility) in q_resources.iter() {
        let should_register = visibility
            .map(|v| *v != bevy::prelude::Visibility::Hidden)
            .unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
            registered_count += 1;
        }
    }
    info!(
        "RESOURCE_GRID: Populated {} existing resources into grid",
        registered_count
    );
    info!(
        "STARTUP_TIMING: populate_resource_spatial_grid finished in {} ms",
        start.elapsed().as_millis()
    );
}

fn spawn_entities(spawn_events: MessageWriter<DamnedSoulSpawnEvent>) {
    let start = Instant::now();
    spawn_damned_souls(spawn_events);
    info!(
        "STARTUP_TIMING: spawn_entities finished in {} ms",
        start.elapsed().as_millis()
    );
}

fn spawn_familiar_wrapper(spawn_events: MessageWriter<FamiliarSpawnEvent>) {
    let start = Instant::now();
    crate::entities::familiar::spawn_familiar(spawn_events);
    info!(
        "STARTUP_TIMING: spawn_familiar_wrapper finished in {} ms",
        start.elapsed().as_millis()
    );
}
