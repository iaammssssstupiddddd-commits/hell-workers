//! スタートアップ関連のプラグイン
//!
//! Phase 5: 責務を分割し、システム配線 + 呼び出しに集中。

mod asset_catalog;
mod perf_scenario;
mod rtt_composite;
mod rtt_setup;
mod startup_systems;
mod visual_handles;

pub use perf_scenario::{
    PerfRenderMode, PerfScenarioConfig, PerfScenarioRandomStreams, PerfScenarioSize, PerfWorkload,
};
#[cfg(feature = "profiling")]
pub(crate) use perf_scenario::{is_fixed_step_audit, is_not_fixed_step_audit};
pub use rtt_composite::RttCompositeSprite;
pub(crate) use rtt_composite::composite_logical_size;
pub use rtt_setup::{
    Camera3dRtt, Camera3dSoulMaskRtt, RttDirectionalLight, RttExtraDirectionalLight, RttRuntime,
    RttViewportSize,
};
pub use visual_handles::{Building3dHandles, CharacterHandles, Terrain3dHandles};

use crate::world::map::{build_terrain_feature_map, build_terrain_id_map, spawn_boundary_meshes};
#[cfg(feature = "profiling")]
use perf_scenario::{
    PerfScenarioApplied, PerfScenarioSet, setup_perf_scenario_if_enabled,
    setup_perf_scenario_runtime_if_enabled,
};
use startup_systems::{
    initial_resource_spawner_timed, initialize_gizmo_config, populate_resource_spatial_grid, setup,
    spawn_entities, spawn_familiar_wrapper, spawn_map_timed, spawn_terrain_chunks_timed,
};

use crate::app_contexts::{
    BuildContext, CompanionPlacementState, MoveContext, MovePlacementState, TaskContext,
    ZoneContext,
};
use crate::interface::selection::{HoveredEntity, SelectedEntity};
use crate::interface::ui::{MenuState, setup_ui};
#[cfg(feature = "profiling")]
use crate::systems::GameSystemSet;
use crate::systems::logistics::{ResourceCountDisplayTimer, ResourceLabels};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;
use hw_core::GameTime;
use hw_core::quality::{QualitySettings, RttQualityPreset};
use hw_spatial::{
    BlueprintSpatialGrid, FamiliarSpatialGrid, FloorConstructionSpatialGrid,
    GatheringSpotSpatialGrid, ResourceSpatialGrid, SpatialGrid, StockpileSpatialGrid,
};
use hw_ui::components::ArchitectCategoryState;

pub struct StartupPlugin;

impl Plugin for StartupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldMap>()
            .register_type::<QualitySettings>()
            .register_type::<RttQualityPreset>()
            .init_resource::<QualitySettings>()
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
            .init_resource::<PerfScenarioConfig>()
            .init_resource::<PerfScenarioRandomStreams>()
            .add_plugins(Material2dPlugin::<rtt_composite::RttCompositeMaterial>::default())
            .add_systems(Startup, (setup, initialize_gizmo_config))
            .add_systems(
                PostStartup,
                (
                    build_terrain_feature_map,
                    build_terrain_id_map,
                    visual_handles::init_visual_handles,
                    spawn_map_timed,
                    spawn_terrain_chunks_timed,
                    spawn_boundary_meshes,
                    initial_resource_spawner_timed,
                    spawn_entities,
                    spawn_familiar_wrapper,
                    setup_ui,
                    crate::interface::ui::dev_panel::spawn_dev_panel_system,
                    populate_resource_spatial_grid,
                    rtt_composite::spawn_rtt_composite_sprite,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    rtt_setup::sync_rtt_texture_size_to_window_and_quality,
                    rtt_composite::sync_rtt_output_bindings,
                    rtt_composite::sync_rtt_composite_perf_params_system,
                )
                    .chain(),
            );

        #[cfg(feature = "profiling")]
        {
            app.init_resource::<PerfScenarioApplied>()
                .add_systems(
                    PostStartup,
                    setup_perf_scenario_if_enabled
                        .after(spawn_familiar_wrapper)
                        .before(setup_ui),
                )
                .configure_sets(
                    Update,
                    (
                        PerfScenarioSet::FixtureSpawn,
                        PerfScenarioSet::FixtureApply,
                        PerfScenarioSet::Setup,
                        PerfScenarioSet::Apply,
                        PerfScenarioSet::InitialCheckpoint,
                    )
                        .chain()
                        .before(GameSystemSet::Input),
                )
                .add_systems(
                    Update,
                    (
                        crate::entities::damned_soul::spawn::soul_spawning_system,
                        crate::entities::familiar::familiar_spawning_system,
                    )
                        .in_set(PerfScenarioSet::FixtureSpawn)
                        .run_if(is_fixed_step_audit),
                )
                .add_systems(
                    Update,
                    bevy::ecs::schedule::ApplyDeferred.in_set(PerfScenarioSet::FixtureApply),
                )
                .add_systems(
                    Update,
                    setup_perf_scenario_runtime_if_enabled.in_set(PerfScenarioSet::Setup),
                )
                .add_systems(
                    Update,
                    bevy::ecs::schedule::ApplyDeferred.in_set(PerfScenarioSet::Apply),
                )
                .init_resource::<perf_scenario::PerfCapture>()
                .configure_sets(
                    Update,
                    PerfScenarioSet::Capture.after(GameSystemSet::Interface),
                )
                .add_systems(
                    Update,
                    perf_scenario::start_perf_capture_system
                        .in_set(PerfScenarioSet::InitialCheckpoint),
                )
                .add_systems(
                    Update,
                    perf_scenario::drive_perf_capture_system.in_set(PerfScenarioSet::Capture),
                );
        }
    }
}
