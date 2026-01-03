mod assets;
mod constants;
mod entities;
mod interface;
mod systems;
mod world;

use crate::assets::GameAssets;
use crate::world::map::{WorldMap, spawn_map};
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use std::time::Duration;

// 新システム
use crate::entities::damned_soul::{
    animation_system, pathfinding_system, soul_movement, spawn_damned_souls,
};
use crate::entities::familiar::{
    familiar_movement, spawn_familiar, update_familiar_range_indicator,
};
use crate::systems::command::{
    TaskMode, area_selection_indicator_system, designation_visual_system,
    familiar_command_input_system, familiar_command_visual_system, task_area_indicator_system,
    task_area_selection_system, update_designation_indicator_system,
};
use crate::systems::idle::{idle_behavior_system, idle_visual_system};
use crate::systems::jobs::{DesignationCreatedEvent, TaskCompletedEvent};
use crate::systems::motivation::{
    familiar_hover_visualization_system, fatigue_system, motivation_system,
};
use crate::systems::work::{
    SpatialGrid, TaskQueue, cleanup_commanded_souls_system, queue_management_system,
    task_area_auto_haul_system, task_delegation_system, task_execution_system,
    update_spatial_grid_system,
};

// 既存システム
use crate::interface::camera::{MainCamera, camera_movement, camera_zoom};
use crate::interface::selection::{
    BuildMode, SelectedEntity, blueprint_placement, handle_mouse_input, update_selection_indicator,
};
use crate::interface::ui::{
    MenuState, familiar_context_menu_system, info_panel_system, menu_visibility_system, setup_ui,
    ui_interaction_system, update_mode_text_system,
};
use crate::systems::jobs::building_completion_system;
use crate::systems::logistics::{
    ResourceLabels, ZoneMode, initial_resource_spawner, item_spawner_system,
    resource_count_display_system, zone_placement,
};
use crate::systems::time::{
    GameTime, game_time_system, time_control_keyboard_system, time_control_ui_system,
};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Hell Workers".into(),
                        resolution: (1280.0, 720.0).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::INFO,
                    filter: "wgpu=error,bevy_app=info".to_string(),
                    ..default()
                }),
        )
        // Resources from various modules
        .init_resource::<WorldMap>()
        .init_resource::<SelectedEntity>()
        .init_resource::<MenuState>()
        .init_resource::<BuildMode>()
        .init_resource::<ZoneMode>()
        .init_resource::<ResourceLabels>()
        .init_resource::<GameTime>()
        .init_resource::<TaskMode>()
        .init_resource::<SpatialGrid>()
        .init_resource::<TaskQueue>()
        .add_event::<DesignationCreatedEvent>()
        .add_event::<TaskCompletedEvent>()
        // Startup systems
        .add_systems(Startup, setup)
        .add_systems(
            PostStartup,
            (
                spawn_map,
                spawn_entities,
                spawn_familiar_wrapper,
                setup_ui,
                initial_resource_spawner,
            )
                .chain(),
        )
        // Update systems
        .add_systems(
            Update,
            (
                camera_movement,
                camera_zoom,
                handle_mouse_input,
                blueprint_placement,
                zone_placement,
                item_spawner_system,
                ui_interaction_system,
                menu_visibility_system,
                info_panel_system,
                update_mode_text_system,
                familiar_context_menu_system,
                update_selection_indicator,
                update_familiar_range_indicator,
                resource_count_display_system,
                game_time_system,
                time_control_keyboard_system,
                time_control_ui_system,
                // Cache & Queue update systems (毎フレーム実行)
                (update_spatial_grid_system, queue_management_system),
                // Hell Workers core systems & Logic chain
                (
                    familiar_command_input_system,
                    task_area_selection_system,
                    task_area_indicator_system,
                    area_selection_indicator_system,
                    designation_visual_system,
                    update_designation_indicator_system,
                    familiar_command_visual_system,
                    motivation_system,
                    fatigue_system,
                    cleanup_commanded_souls_system,
                    task_execution_system,
                    familiar_hover_visualization_system,
                    idle_behavior_system,
                    idle_visual_system,
                    pathfinding_system,
                    soul_movement,
                    familiar_movement,
                    task_delegation_system,
                )
                    .chain(),
                (building_completion_system, animation_system).chain(),
            ),
        )
        // Timer-based systems for performance optimization (0.5s interval)
        .add_systems(
            Update,
            (task_area_auto_haul_system,).run_if(on_timer(Duration::from_millis(500))),
        )
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((Camera2d, MainCamera));

    let game_assets = GameAssets {
        grass: asset_server.load("textures/grass.jpg"),
        dirt: asset_server.load("textures/dirt.jpg"),
        stone: asset_server.load("textures/stone.jpg"),
        colonist: asset_server.load("textures/colonist.jpg"),
        wall: asset_server.load("textures/stone.jpg"), // Placeholder
        wood: asset_server.load("textures/dirt.jpg"),  // Placeholder
    };
    commands.insert_resource(game_assets);
}

/// エンティティ（使い魔と人間）をスポーン
fn spawn_entities(commands: Commands, game_assets: Res<GameAssets>, world_map: Res<WorldMap>) {
    // 人間をスポーン
    spawn_damned_souls(commands, game_assets, world_map);
}

/// 使い魔をスポーン（別システムとして実行）
fn spawn_familiar_wrapper(
    commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
) {
    spawn_familiar(commands, game_assets, world_map);
}
