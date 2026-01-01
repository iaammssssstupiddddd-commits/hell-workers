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
use crate::systems::command::{familiar_command_input_system, familiar_command_visual_system};

// 既存システム
use crate::systems::jobs::{job_assignment_system, construction_work_system, building_completion_system};
use crate::systems::logistics::{zone_placement, item_spawner_system, hauling_system, resource_count_display_system, ResourceLabels, ZoneMode};
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
        // Startup systems
        .add_systems(Startup, setup)
        .add_systems(PostStartup, (spawn_map, spawn_entities, spawn_familiar_wrapper, setup_ui).chain())
        // Update systems
        .add_systems(Update, (
            camera_movement, 
            camera_zoom, 
            log_periodically,
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
                familiar_command_visual_system,
                motivation_system,
                fatigue_system,
                idle_behavior_system,
                idle_visual_system,
            ).chain(),
            // Logic chain
            (
                job_assignment_system, 
                hauling_system, 
                pathfinding_system, 
                soul_movement,
                familiar_movement,
                construction_work_system, 
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

fn log_periodically(
    time: Res<Time>,
    mut timer: Local<f32>,
    query_cam: Query<&Transform, With<MainCamera>>,
    query_souls: Query<(&Transform, &crate::entities::damned_soul::DamnedSoul)>,
    query_familiars: Query<&Transform, With<crate::entities::familiar::Familiar>>,
    game_assets: Res<GameAssets>,
    asset_server: Res<AssetServer>,
) {
    *timer += time.delta_secs();
    if *timer > 3.0 {
        if let Ok(cam_transform) = query_cam.get_single() {
            info!("CAMERA_POS: x: {:.1}, y: {:.1}", cam_transform.translation.x, cam_transform.translation.y);
        }
        
        for (soul_transform, soul) in query_souls.iter() {
            info!("SOUL: pos({:.0},{:.0}) motivation:{:.2} laziness:{:.2} fatigue:{:.2}", 
                soul_transform.translation.x, soul_transform.translation.y,
                soul.motivation, soul.laziness, soul.fatigue);
        }

        for fam_transform in query_familiars.iter() {
            info!("FAMILIAR: pos({:.0},{:.0})", 
                fam_transform.translation.x, fam_transform.translation.y);
        }

        let grass_load = asset_server.get_load_state(&game_assets.grass);
        info!("ASSET_LOAD_STATE: Grass:{:?}", grass_load);
        
        *timer = 0.0;
    }
}
