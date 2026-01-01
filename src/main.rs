mod constants;
mod assets;
mod world;
mod entities;
mod systems;
mod interface;

use bevy::prelude::*;
use crate::assets::GameAssets;
use crate::world::map::{WorldMap, spawn_map};

// 新システム
use crate::entities::damned_soul::{spawn_damned_souls, pathfinding_system, soul_movement, animation_system};
use crate::entities::familiar::{spawn_familiar, update_familiar_range_indicator, familiar_movement};
use crate::systems::motivation::{motivation_system, fatigue_system};
use crate::systems::idle::{idle_behavior_system, idle_visual_system};
use crate::systems::command::{familiar_command_input_system, familiar_command_visual_system, task_area_selection_system, task_area_indicator_system, TaskMode};
use crate::systems::work::{task_delegation_system, task_execution_system};

// 既存システム
use crate::systems::jobs::building_completion_system;
use crate::systems::logistics::{zone_placement, item_spawner_system, initial_resource_spawner, resource_count_display_system, ResourceLabels, ZoneMode};
use crate::systems::time::{game_time_system, time_control_keyboard_system, time_control_ui_system, GameTime};
use crate::interface::ui::{setup_ui, ui_interaction_system, menu_visibility_system, info_panel_system, MenuState};
use crate::interface::camera::{camera_movement, camera_zoom, MainCamera};
use crate::interface::selection::{handle_mouse_input, blueprint_placement, update_selection_indicator, SelectedEntity, BuildMode};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Hell Workers".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }).set(bevy::log::LogPlugin {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,bevy_app=debug".to_string(),
            ..default()
        }))
        // Resources from various modules
        .init_resource::<WorldMap>()
        .init_resource::<SelectedEntity>()
        .init_resource::<MenuState>()
        .init_resource::<BuildMode>()
        .init_resource::<ZoneMode>()
        .init_resource::<ResourceLabels>()
        .init_resource::<GameTime>()
        .init_resource::<TaskMode>()
        // Startup systems
        .add_systems(Startup, setup)
        .add_systems(PostStartup, (spawn_map, spawn_entities, spawn_familiar_wrapper, setup_ui, initial_resource_spawner).chain())
        // Update systems
        .add_systems(Update, (
            camera_movement, 
            camera_zoom, 
            handle_mouse_input,
            blueprint_placement,
            zone_placement,
            item_spawner_system,
            ui_interaction_system,
            menu_visibility_system,
            info_panel_system,
            update_selection_indicator,
            update_familiar_range_indicator,
            resource_count_display_system,
            game_time_system,
            time_control_keyboard_system,
            time_control_ui_system,
            // Hell Workers core systems
            (
                familiar_command_input_system,
                task_area_selection_system,
                task_area_indicator_system,
                familiar_command_visual_system,
                motivation_system,  // やる気を先に更新
                fatigue_system,
                task_delegation_system,  // その後タスク割り当て
                task_execution_system,   // タスク実行
                idle_behavior_system,
                idle_visual_system,
            ).chain(),
            // Logic chain
            (
                pathfinding_system, 
                soul_movement,
                familiar_movement,
                building_completion_system, 
                animation_system
            ).chain(),
        ))
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
        wood: asset_server.load("textures/dirt.jpg"), // Placeholder
    };
    commands.insert_resource(game_assets);
}

/// エンティティ（使い魔と人間）をスポーン
fn spawn_entities(
    commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
) {
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
