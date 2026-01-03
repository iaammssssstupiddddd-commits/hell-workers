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
    TaskMode, area_selection_indicator_system, assign_task_system, designation_visual_system,
    familiar_command_input_system, familiar_command_visual_system, task_area_indicator_system,
    task_area_selection_system, update_designation_indicator_system,
};
use crate::systems::idle::{idle_behavior_system, idle_visual_system};
use crate::systems::jobs::{DesignationCreatedEvent, TaskCompletedEvent};
use crate::systems::motivation::{
    familiar_hover_visualization_system, fatigue_system, motivation_system,
};
use crate::systems::visuals::{
    progress_bar_system, soul_status_visual_system, sync_progress_bar_position_system,
    task_link_system,
};
use crate::systems::work::{
    GlobalTaskQueue, SpatialGrid, TaskQueue, cleanup_commanded_souls_system,
    queue_management_system, task_area_auto_haul_system, task_delegation_system,
    task_execution_system, update_spatial_grid_system,
};

// 既存システム
use crate::interface::camera::{MainCamera, camera_movement, camera_zoom};
use crate::interface::selection::{
    BuildMode, SelectedEntity, blueprint_placement, handle_mouse_input, update_selection_indicator,
};
use crate::interface::ui::{
    MenuState, familiar_context_menu_system, info_panel_system, menu_visibility_system, setup_ui,
    task_summary_ui_system, ui_interaction_system, update_mode_text_system,
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
        .init_resource::<GlobalTaskQueue>()
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
        // Update systems - Interface & Global
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
                task_summary_ui_system,
                update_selection_indicator,
                update_familiar_range_indicator,
                resource_count_display_system,
                game_time_system,
                time_control_keyboard_system,
                time_control_ui_system,
            ),
        )
        // Update systems - Core Logic & Visuals
        .add_systems(
            Update,
            (
                // Cache & Queue update systems (毎フレーム実行)
                (
                    update_spatial_grid_system,
                    queue_management_system,
                    task_delegation_system,
                    task_execution_system,
                    cleanup_commanded_souls_system,
                    progress_bar_system,
                    sync_progress_bar_position_system,
                    task_link_system,
                    soul_status_visual_system,
                    assign_task_system,
                ),
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
                    familiar_hover_visualization_system,
                    idle_behavior_system,
                    idle_visual_system,
                    pathfinding_system,
                    soul_movement,
                    familiar_movement,
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

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.spawn((Camera2d, MainCamera));

    // 円形グラデーションテクスチャを動的生成
    let aura_circle = create_circular_gradient_texture(&mut images);
    // 円形リング（外枠）テクスチャを動的生成
    let aura_ring = create_circular_outline_texture(&mut images);

    let game_assets = GameAssets {
        grass: asset_server.load("textures/grass.jpg"),
        dirt: asset_server.load("textures/dirt.jpg"),
        stone: asset_server.load("textures/stone.jpg"),
        colonist: asset_server.load("textures/colonist.jpg"),
        wall: asset_server.load("textures/stone.jpg"), // Placeholder
        wood: asset_server.load("textures/dirt.jpg"),  // Placeholder
        aura_circle,
        aura_ring,
    };
    commands.insert_resource(game_assets);
}

/// 円形リング（外枠）テクスチャを生成
fn create_circular_outline_texture(images: &mut Assets<Image>) -> Handle<Image> {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let size = 128u32;
    let center = size as f32 / 2.0;
    let thickness = 2.0; // 線の太さ
    let mut data = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();

            // 外側の境界付近だけ不透明にする
            let dist_from_edge = (distance - (center - thickness)).abs();
            let alpha = if dist_from_edge < thickness {
                let factor = 1.0 - (dist_from_edge / thickness);
                (factor * 255.0) as u8
            } else {
                0
            };

            // RGBA: 白いリング
            data.push(255); // R
            data.push(255); // G
            data.push(255); // B
            data.push(alpha); // A
        }
    }

    let image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    );

    images.add(image)
}

/// 円形グラデーションテクスチャを生成
fn create_circular_gradient_texture(images: &mut Assets<Image>) -> Handle<Image> {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let size = 128u32;
    let center = size as f32 / 2.0;
    let mut data = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt() / center;

            // 円形グラデーション（中心から外側へ透明に）
            let alpha = if distance <= 1.0 {
                ((1.0 - distance).powf(0.5) * 255.0) as u8
            } else {
                0
            };

            // RGBA: 白い円形グラデーション
            data.push(255); // R
            data.push(255); // G
            data.push(255); // B
            data.push(alpha); // A
        }
    }

    let image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    );

    images.add(image)
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
