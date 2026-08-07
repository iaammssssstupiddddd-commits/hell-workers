//! スタートアップ関連のプラグイン
//!
//! Phase 5: 責務を分割し、システム配線 + 呼び出しに集中。

mod asset_catalog;
#[cfg(feature = "profiling")]
mod perf_render_environment;
mod perf_scenario;
mod rtt_composite;
mod rtt_setup;
mod startup_systems;
mod visual_handles;

#[cfg(test)]
pub(crate) use asset_catalog::create_game_assets;
pub use perf_scenario::{
    PerfFamiliarPolicyMode, PerfOperationDialogMode, PerfRenderMode, PerfScenarioConfig,
    PerfScenarioRandomStreams, PerfScenarioSize, PerfWorkload,
};
#[cfg(feature = "profiling")]
pub(crate) use perf_scenario::{is_fixed_step_behavior, is_not_fixed_step_scenario};
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
    setup_perf_scenario_runtime_if_enabled, setup_perf_ui_mode_if_enabled,
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
            perf_render_environment::install(app);
            perf_scenario::install_renderdoc_capture(app);
            app.init_resource::<PerfScenarioApplied>()
                .init_resource::<perf_scenario::PerfScenarioDriverState>()
                .init_resource::<perf_scenario::IndoorLightFixtureState>()
                .init_resource::<perf_scenario::PerfBehaviorCapture>()
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
                        PerfScenarioSet::IndoorSettle,
                        PerfScenarioSet::FixtureSustain,
                        PerfScenarioSet::UiSetup,
                        PerfScenarioSet::InitialCheckpoint
                            .after(rtt_setup::sync_rtt_texture_size_to_window_and_quality),
                        PerfScenarioSet::Driver,
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
                        .run_if(perf_scenario::is_fixed_step_scenario),
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
                .add_systems(
                    Update,
                    (
                        crate::systems::jobs::building_completion_system,
                        bevy::ecs::schedule::ApplyDeferred,
                        perf_scenario::stabilize_indoor_light_actors_system,
                        perf_scenario::seed_indoor_light_static_door_states_system,
                        perf_scenario::prepare_indoor_light_soul_spa_system,
                        crate::systems::jobs::soul_spa_construction::soul_spa_tile_activate_system,
                        bevy::ecs::schedule::ApplyDeferred,
                        perf_scenario::assign_indoor_light_generator_system,
                        bevy::ecs::schedule::ApplyDeferred,
                        crate::systems::energy::grid_recalc::sync_power_allocation_mode_from_settings_system,
                        crate::systems::energy::grid_recalc::detect_energy_update_dirty_system,
                        crate::systems::energy::grid_lifecycle::reconcile_power_grid_topology_system,
                        bevy::ecs::schedule::ApplyDeferred,
                        crate::systems::energy::power_output::soul_spa_power_output_system,
                        crate::systems::energy::grid_recalc::grid_recalc_system,
                        bevy::ecs::schedule::ApplyDeferred,
                        hw_world::detect_rooms_immediately_system,
                        bevy::ecs::schedule::ApplyDeferred,
                        perf_scenario::validate_indoor_light_fixture_system,
                    )
                        .chain()
                        .in_set(PerfScenarioSet::IndoorSettle)
                        .run_if(perf_scenario::should_settle_indoor_light_fixture),
                )
                .add_systems(
                    Update,
                    perf_scenario::maintain_indoor_light_generator_vitals_system
                        .in_set(PerfScenarioSet::FixtureSustain)
                        .run_if(perf_scenario::should_maintain_indoor_light_generator_vitals),
                )
                .add_systems(
                    Update,
                    setup_perf_ui_mode_if_enabled.in_set(PerfScenarioSet::UiSetup),
                )
                .init_resource::<perf_scenario::PerfCapture>()
                .configure_sets(
                    Update,
                    PerfScenarioSet::Capture.after(GameSystemSet::Interface),
                )
                .add_systems(
                    Update,
                    perf_scenario::start_perf_capture_system
                        .in_set(PerfScenarioSet::InitialCheckpoint)
                        .run_if(perf_scenario::is_not_fixed_step_behavior)
                        .run_if(perf_scenario::is_not_renderdoc_capture),
                )
                .add_systems(
                    Update,
                    perf_scenario::arm_renderdoc_checkpoint_system
                        .in_set(PerfScenarioSet::InitialCheckpoint),
                )
                .add_systems(
                    Update,
                    perf_scenario::drive_perf_workload_system.in_set(PerfScenarioSet::Driver),
                )
                .add_systems(
                    Update,
                    perf_scenario::drive_perf_behavior_system
                        .in_set(PerfScenarioSet::Driver)
                        .run_if(is_fixed_step_behavior),
                )
                .add_systems(
                    Update,
                    perf_scenario::drive_perf_capture_system
                        .in_set(PerfScenarioSet::Capture)
                        .run_if(perf_scenario::is_not_fixed_step_behavior)
                        .run_if(perf_scenario::is_not_renderdoc_capture),
                )
                .add_systems(
                    Update,
                    perf_scenario::poll_renderdoc_capture_system
                        .in_set(PerfScenarioSet::Capture),
                )
                .add_systems(
                    Update,
                    perf_scenario::observe_perf_behavior_system
                        .in_set(PerfScenarioSet::Capture)
                        .run_if(is_fixed_step_behavior),
                )
                .add_systems(
                    FixedUpdate,
                    perf_scenario::count_perf_behavior_fixed_tick_system
                        .run_if(is_fixed_step_behavior),
                );
        }
    }
}
