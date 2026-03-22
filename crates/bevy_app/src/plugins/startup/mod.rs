//! スタートアップ関連のプラグイン
//!
//! Phase 5: 責務を分割し、システム配線 + 呼び出しに集中。

mod asset_catalog;
mod perf_scenario;
mod rtt_composite;
mod rtt_setup;
mod startup_systems;
mod visual_handles;

pub use rtt_composite::RttCompositeSprite;
pub use rtt_setup::{Camera3dRtt, RttTextures, RttViewportSize};
pub use visual_handles::Building3dHandles;

use perf_scenario::{
    PerfScenarioApplied, setup_perf_scenario_if_enabled, setup_perf_scenario_runtime_if_enabled,
};
use startup_systems::{
    initialize_gizmo_config, initial_resource_spawner_timed, populate_resource_spatial_grid,
    setup, spawn_entities, spawn_familiar_wrapper, spawn_map_timed,
    spawn_terrain_borders_if_enabled,
};

use crate::app_contexts::{
    BuildContext, CompanionPlacementState, MoveContext, MovePlacementState, TaskContext,
    ZoneContext,
};
use crate::interface::selection::{HoveredEntity, SelectedEntity};
use crate::interface::ui::{MenuState, setup_ui};
use crate::systems::logistics::{ResourceCountDisplayTimer, ResourceLabels};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::GameTime;
use hw_spatial::{
    BlueprintSpatialGrid, FamiliarSpatialGrid, FloorConstructionSpatialGrid,
    GatheringSpotSpatialGrid, ResourceSpatialGrid, SpatialGrid, StockpileSpatialGrid,
};
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
