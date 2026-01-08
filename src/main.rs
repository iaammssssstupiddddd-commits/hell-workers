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
    DamnedSoulSpawnEvent, animation_system, pathfinding_system, soul_movement,
    soul_spawning_system, spawn_damned_souls,
};
use crate::entities::familiar::{
    FamiliarSpawnEvent, FamiliarType, familiar_movement, familiar_spawning_system, spawn_familiar,
    update_familiar_range_indicator,
};
use crate::systems::command::{
    TaskMode, area_selection_indicator_system, assign_task_system, designation_visual_system,
    familiar_command_input_system, familiar_command_visual_system, task_area_indicator_system,
    task_area_selection_system, update_designation_indicator_system,
};
use crate::systems::familiar_ai::{familiar_ai_system, following_familiar_system};
use crate::systems::idle::{gathering_separation_system, idle_behavior_system, idle_visual_system};
use crate::systems::jobs::{DesignationCreatedEvent, TaskCompletedEvent};
use crate::systems::motivation::{
    familiar_hover_visualization_system, fatigue_system, motivation_system,
};
use crate::systems::spatial::{
    FamiliarSpatialGrid, ResourceSpatialGrid, SpatialGrid, update_familiar_spatial_grid_system,
    update_resource_spatial_grid_system, update_spatial_grid_system,
};
use crate::systems::task_execution::task_execution_system;
use crate::systems::task_queue::{GlobalTaskQueue, TaskQueue, queue_management_system};
use crate::systems::visuals::{
    progress_bar_system, soul_status_visual_system, sync_progress_bar_position_system,
    task_link_system, update_progress_bar_fill_system,
};
use crate::systems::work::{
    AutoHaulCounter, cleanup_commanded_souls_system, task_area_auto_haul_system,
};

// 既存システム
use crate::interface::camera::{MainCamera, camera_movement, camera_zoom};
use crate::interface::selection::{
    BuildMode, HoveredEntity, SelectedEntity, blueprint_placement, handle_mouse_input,
    update_hover_entity, update_selection_indicator,
};
use crate::interface::ui_interaction::{
    hover_tooltip_system, task_summary_ui_system, ui_interaction_system, update_mode_text_system,
    update_operation_dialog_system,
};
use crate::interface::ui_panels::{
    familiar_context_menu_system, info_panel_system, menu_visibility_system,
};
use crate::interface::ui_setup::{MenuState, setup_ui};
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
        .init_resource::<HoveredEntity>()
        .init_resource::<MenuState>()
        .init_resource::<BuildMode>()
        .init_resource::<ZoneMode>()
        .init_resource::<ResourceLabels>()
        .init_resource::<GameTime>()
        .init_resource::<TaskMode>()
        .init_resource::<SpatialGrid>()
        .init_resource::<FamiliarSpatialGrid>()
        .init_resource::<ResourceSpatialGrid>()
        .init_resource::<AutoHaulCounter>()
        .init_resource::<TaskQueue>()
        .init_resource::<GlobalTaskQueue>()
        .add_event::<DesignationCreatedEvent>()
        .add_event::<TaskCompletedEvent>()
        .add_event::<DamnedSoulSpawnEvent>()
        .add_event::<FamiliarSpawnEvent>()
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
                initialize_familiar_spatial_grid,
                initialize_resource_spatial_grid,
                populate_resource_spatial_grid,
            )
                .chain(),
        )
        // Update systems - Interface & Global
        .add_systems(
            Update,
            (
                // カメラ & 入力
                (camera_movement, camera_zoom, handle_mouse_input),
                // 選択 & 配置
                (
                    update_hover_entity,
                    update_selection_indicator,
                    hover_tooltip_system,
                    blueprint_placement,
                    zone_placement,
                    item_spawner_system,
                ),
                // UI ボタン・メニュー系
                (
                    ui_interaction_system,
                    menu_visibility_system,
                    info_panel_system,
                ),
                // ステータス・テキスト更新系
                (
                    update_mode_text_system,
                    familiar_context_menu_system,
                    task_summary_ui_system,
                    update_operation_dialog_system,
                    resource_count_display_system,
                ),
                // 使い魔・時間・その他
                (
                    update_familiar_range_indicator,
                    game_time_system,
                    time_control_keyboard_system,
                    time_control_ui_system,
                    debug_spawn_system,
                ),
            ),
        )
        // Update systems - Core Logic & Visuals
        .add_systems(
            Update,
            (
                // ロジックチェーン (直列実行を強制して不整合を防ぐ)
                (
                    cleanup_commanded_souls_system,
                    update_spatial_grid_system,
                    update_familiar_spatial_grid_system,
                    update_resource_spatial_grid_system,
                    queue_management_system,
                    familiar_ai_system,        // 新ステートマシンAI
                    following_familiar_system, // 部下の追従システム
                    // task_delegation_system, // 既存の自動委譲（AIに移行したため無効化）
                    task_execution_system,
                    assign_task_system,
                )
                    .chain(),
                // 視覚系などの非依存システム
                (progress_bar_system, update_progress_bar_fill_system),
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
                    gathering_separation_system,
                    pathfinding_system,
                    soul_movement,
                    familiar_movement,
                    soul_spawning_system,
                    familiar_spawning_system,
                )
                    .chain(),
                // 表示同期システム (移動の後に実行してジッターを防ぐ)
                (
                    sync_progress_bar_position_system,
                    soul_status_visual_system,
                    task_link_system,
                    building_completion_system,
                    animation_system,
                )
                    .chain(),
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
    let aura_circle = create_circular_gradient_texture(&mut *images);
    // 円形リング（外枠）テクスチャを動的生成
    let aura_ring = create_circular_outline_texture(&mut *images);

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

/// FamiliarSpatialGridを初期化（最大command_radius * 2のセルサイズで）
fn initialize_familiar_spatial_grid(mut familiar_grid: ResMut<FamiliarSpatialGrid>) {
    use crate::constants::TILE_SIZE;
    // 最大command_radiusはTILE_SIZE * 10.0（Taskmaster）なので、
    // グリッドサイズはTILE_SIZE * 20.0以上にする
    *familiar_grid = FamiliarSpatialGrid::new(TILE_SIZE * 20.0);
}

/// ResourceSpatialGridを初期化
fn initialize_resource_spatial_grid(mut resource_grid: ResMut<ResourceSpatialGrid>) {
    use crate::constants::TILE_SIZE;
    // 検索範囲はTILE_SIZE * 15.0なので、グリッドサイズはそれより大きくする
    *resource_grid = ResourceSpatialGrid::new(TILE_SIZE * 20.0);
}

/// 起動時に既存のリソースをResourceSpatialGridに登録
fn populate_resource_spatial_grid(
    mut resource_grid: ResMut<ResourceSpatialGrid>,
    q_resources: Query<
        (Entity, &Transform, Option<&Visibility>),
        With<crate::systems::logistics::ResourceItem>,
    >,
) {
    let mut registered_count = 0;
    let mut skipped_count = 0;
    for (entity, transform, visibility) in q_resources.iter() {
        // Visibility::Hiddenのリソース（拾われている）は除外、それ以外は登録
        let should_register = visibility
            .map(|v| *v != bevy::prelude::Visibility::Hidden)
            .unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
            registered_count += 1;
        } else {
            skipped_count += 1;
        }
    }
    info!(
        "RESOURCE_GRID: Populated {}/{} existing resources into grid (skipped: {})",
        registered_count,
        q_resources.iter().count(),
        skipped_count
    );
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
fn spawn_entities(spawn_events: EventWriter<DamnedSoulSpawnEvent>) {
    // 人間をスポーン
    spawn_damned_souls(spawn_events);
}

/// 使い魔をスポーン（別システムとして実行）
fn spawn_familiar_wrapper(spawn_events: EventWriter<FamiliarSpawnEvent>) {
    spawn_familiar(spawn_events);
}

/// デバッグ用のスポーンシステム
fn debug_spawn_system(
    buttons: Res<ButtonInput<KeyCode>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut soul_spawn_events: EventWriter<DamnedSoulSpawnEvent>,
    mut familiar_spawn_events: EventWriter<FamiliarSpawnEvent>,
) {
    let mut spawn_pos = Vec2::ZERO;

    // マウスカーソル位置を取得
    if let Ok(window) = q_window.get_single() {
        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok((camera, camera_transform)) = q_camera.get_single() {
                if let Ok(pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                    spawn_pos = pos;
                }
            }
        }
    }

    if buttons.just_pressed(KeyCode::KeyP) {
        soul_spawn_events.send(DamnedSoulSpawnEvent {
            position: spawn_pos,
        });
        info!("DEBUG_SPAWN: Soul at {:?}", spawn_pos);
    }

    if buttons.just_pressed(KeyCode::KeyO) {
        familiar_spawn_events.send(FamiliarSpawnEvent {
            position: spawn_pos,
            familiar_type: FamiliarType::Imp,
        });
        info!("DEBUG_SPAWN: Familiar at {:?}", spawn_pos);
    }
}
