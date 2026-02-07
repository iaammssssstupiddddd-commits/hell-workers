use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, Destination};
use crate::entities::familiar::Familiar;
use crate::game_state::{BuildContext, PlayMode, TaskContext, ZoneContext};
use crate::interface::camera::MainCamera;
use crate::interface::ui::{MenuState, UiInputState};
use crate::systems::jobs::{Blueprint, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct SelectedEntity(pub Option<Entity>);

#[derive(Resource, Default)]
pub struct HoveredEntity(pub Option<Entity>);

#[derive(Component)]
pub struct SelectionIndicator;

pub fn handle_mouse_input(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_souls: Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: Query<(Entity, &GlobalTransform), With<Familiar>>,
    ui_input_state: Res<UiInputState>,
    mut selected_entity: ResMut<SelectedEntity>,
    mut q_dest: Query<&mut Destination>,
    mut q_active_command: Query<&mut crate::entities::familiar::ActiveCommand>,
) {
    // main.rsでrun_if(in_state(PlayMode::Normal))が設定されているため、
    // TaskModeのチェックは不要

    if ui_input_state.pointer_over_ui {
        return;
    }

    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };
    let Ok(window) = q_window.single() else {
        return;
    };

    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            if buttons.just_pressed(MouseButton::Left) {
                let mut found = false;

                // まず使い魔をチェック（優先）
                for (entity, transform) in q_familiars.iter() {
                    let pos = transform.translation().truncate();
                    if pos.distance(world_pos) < TILE_SIZE / 2.0 {
                        selected_entity.0 = Some(entity);
                        found = true;
                        info!("SELECTED: Familiar");
                        break;
                    }
                }

                // 次に人間をチェック
                if !found {
                    for (entity, transform) in q_souls.iter() {
                        let (entity, transform): (Entity, &GlobalTransform) = (entity, transform);
                        let pos = transform.translation().truncate();
                        if pos.distance(world_pos) < TILE_SIZE / 2.0 {
                            selected_entity.0 = Some(entity);
                            found = true;
                            info!("SELECTED: DamnedSoul");
                            break;
                        }
                    }
                }

                if !found {
                    selected_entity.0 = None;
                }
            }

            if buttons.just_pressed(MouseButton::Right) {
                if let Some(selected) = selected_entity.0 {
                    // 使い魔の場合のみ移動指示を出す（Soulは直接指示不可）
                    if q_familiars.get(selected).is_ok() {
                        if let Ok(mut dest) = q_dest.get_mut(selected) {
                            dest.0 = world_pos;
                            info!("ORDER: Move to {:?}", world_pos);

                            // 使い魔の場合、現在のAI作業を中断させる
                            if let Ok(mut active) = q_active_command.get_mut(selected) {
                                active.command = crate::entities::familiar::FamiliarCommand::Idle;
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn blueprint_placement(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut world_map: ResMut<WorldMap>,
    build_context: Res<BuildContext>,
    game_assets: Res<GameAssets>,
    mut commands: Commands,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    if let Some(building_type) = build_context.0 {
        if buttons.just_pressed(MouseButton::Left) {
            let Ok((camera, camera_transform)) = q_camera.single() else {
                return;
            };
            let Ok(window) = q_window.single() else {
                return;
            };

            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                    let grid = WorldMap::world_to_grid(world_pos);

                    // 建造物のサイズと占有グリッドの判定
                    let (occupied_grids, spawn_pos, custom_size) = match building_type {
                        BuildingType::Tank | BuildingType::MudMixer => {
                            let grids = vec![
                                grid,
                                (grid.0 + 1, grid.1),
                                (grid.0, grid.1 + 1),
                                (grid.0 + 1, grid.1 + 1),
                            ];
                            // 2x2 の中心座標
                            let base_pos = WorldMap::grid_to_world(grid.0, grid.1);
                            let center_pos = base_pos + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 0.5);
                            (grids, center_pos, Some(Vec2::splat(TILE_SIZE * 2.0)))
                        }
                        _ => {
                            let grids = vec![grid];
                            let center_pos = WorldMap::grid_to_world(grid.0, grid.1);
                            (grids, center_pos, Some(Vec2::splat(TILE_SIZE)))
                        }
                    };

                    // 配置可能かチェック（全占有予定マスが空きかつ通行可能か）
                    let can_place = occupied_grids.iter().all(|&g| {
                        !world_map.buildings.contains_key(&g)
                            && !world_map.stockpiles.contains_key(&g)
                            && world_map.is_walkable(g.0, g.1)
                    });

                    if can_place {
                        let texture = match building_type {
                            BuildingType::Wall => game_assets.wall_isolated.clone(),
                            BuildingType::Floor => game_assets.dirt.clone(),
                            BuildingType::Tank => game_assets.tank_empty.clone(),
                            BuildingType::MudMixer => game_assets.mud_mixer.clone(),
                        };

                        let entity = commands
                            .spawn((
                                Blueprint::new(building_type, occupied_grids.clone()),
                                crate::systems::jobs::Designation {
                                    work_type: crate::systems::jobs::WorkType::Build,
                                },
                                crate::systems::jobs::TaskSlots::new(1),
                                Sprite {
                                    image: texture,
                                    color: Color::srgba(1.0, 1.0, 1.0, 0.5),
                                    custom_size,
                                    ..default()
                                },
                                Transform::from_xyz(spawn_pos.x, spawn_pos.y, Z_AURA),
                                Name::new(format!("Blueprint ({:?})", building_type)),
                            ))
                            .id();

                        // 全グリッドを登録
                        for &g in &occupied_grids {
                            world_map.buildings.insert(g, entity);
                            // 建築中は障害物としても登録しておく
                            world_map.add_obstacle(g.0, g.1);
                        }
                        info!(
                            "BLUEPRINT: Placed {:?} at {:?}",
                            building_type, occupied_grids
                        );
                    }
                }
            }
        }
    }
}

pub fn update_selection_indicator(
    selected: Res<SelectedEntity>,
    mut q_indicator: Query<(Entity, &mut Transform), With<SelectionIndicator>>,
    q_transforms: Query<&GlobalTransform>,
    mut commands: Commands,
) {
    if let Some(entity) = selected.0 {
        if let Ok(target_transform) = q_transforms.get(entity) {
            if let Ok((_, mut indicator_transform)) = q_indicator.single_mut() {
                indicator_transform.translation = target_transform
                    .translation()
                    .truncate()
                    .extend(Z_SELECTION);
            } else {
                commands.spawn((
                    SelectionIndicator,
                    Sprite {
                        color: Color::srgba(1.0, 1.0, 0.0, 0.4),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 1.1)),
                        ..default()
                    },
                    Transform::from_translation(
                        target_transform
                            .translation()
                            .truncate()
                            .extend(Z_SELECTION),
                    ),
                ));
            }
        }
    } else {
        for (indicator_entity, _) in q_indicator.iter() {
            commands.entity(indicator_entity).despawn();
        }
    }
}

pub fn update_hover_entity(
    ui_input_state: Res<UiInputState>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_souls: Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: Query<(Entity, &GlobalTransform), With<Familiar>>,
    q_targets: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&crate::systems::jobs::Building>,
        ),
        Or<(
            With<crate::systems::jobs::Tree>,
            With<crate::systems::jobs::Rock>,
            With<crate::systems::logistics::ResourceItem>,
            With<crate::systems::jobs::Building>,
        )>,
    >,
    mut hovered_entity: ResMut<HoveredEntity>,
) {
    if ui_input_state.pointer_over_ui {
        hovered_entity.0 = None;
        return;
    }

    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };
    let Ok(window) = q_window.single() else {
        return;
    };

    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            let mut found = None;

            // 1. 使い魔
            for (entity, transform) in q_familiars.iter() {
                let pos = transform.translation().truncate();
                if pos.distance(world_pos) < TILE_SIZE / 2.0 {
                    found = Some(entity);
                    break;
                }
            }

            // 2. 魂
            if found.is_none() {
                for (entity, transform) in q_souls.iter() {
                    let (entity, transform): (Entity, &GlobalTransform) = (entity, transform);
                    let pos = transform.translation().truncate();
                    if pos.distance(world_pos) < TILE_SIZE / 2.0 {
                        found = Some(entity);
                        break;
                    }
                }
            }

            // 3. 資源・アイテム・建物
            if found.is_none() {
                for (entity, transform, building_opt) in q_targets.iter() {
                    let pos = transform.translation().truncate();
                    let radius = if let Some(building) = building_opt {
                        match building.kind {
                            crate::systems::jobs::BuildingType::Tank
                            | crate::systems::jobs::BuildingType::MudMixer => TILE_SIZE, // 2x2なので半径を大きく
                            _ => TILE_SIZE / 2.0,
                        }
                    } else {
                        TILE_SIZE / 2.0
                    };

                    if pos.distance(world_pos) < radius {
                        found = Some(entity);
                        break;
                    }
                }
            }

            if found != hovered_entity.0 {
                if let Some(e) = found {
                    info!("HOVER: Found entity {:?}", e);
                }
                hovered_entity.0 = found;
            }
        }
    }
}

pub fn cleanup_selection_references_system(
    mut selected_entity: ResMut<SelectedEntity>,
    mut hovered_entity: ResMut<HoveredEntity>,
    q_exists: Query<(), ()>,
) {
    if let Some(entity) = selected_entity.0
        && q_exists.get(entity).is_err()
    {
        selected_entity.0 = None;
    }

    if let Some(entity) = hovered_entity.0
        && q_exists.get(entity).is_err()
    {
        hovered_entity.0 = None;
    }
}

/// Escキーでビルド/ゾーン/タスクモードを解除し、PlayMode::Normalに戻す
/// 共通仕様: Normalに戻る際はMenuStateもHiddenに戻す
pub fn build_mode_cancel_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    play_mode: Res<State<PlayMode>>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut build_context: ResMut<BuildContext>,
    mut zone_context: ResMut<ZoneContext>,
    mut task_context: ResMut<TaskContext>,
    mut menu_state: ResMut<MenuState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        let current_mode = play_mode.get();
        if *current_mode == PlayMode::BuildingPlace {
            build_context.0 = None;
            next_play_mode.set(PlayMode::Normal);
            *menu_state = MenuState::Hidden;
            info!("STATE: Cancelled BuildingPlace -> Normal, Menu hidden");
        } else if *current_mode == PlayMode::ZonePlace {
            zone_context.0 = None;
            next_play_mode.set(PlayMode::Normal);
            *menu_state = MenuState::Hidden;
            info!("STATE: Cancelled ZonePlace -> Normal, Menu hidden");
        } else if *current_mode == PlayMode::TaskDesignation {
            task_context.0 = crate::systems::command::TaskMode::None;
            next_play_mode.set(PlayMode::Normal);
            *menu_state = MenuState::Hidden;
            info!("STATE: Cancelled TaskDesignation -> Normal, Menu hidden");
        }
    }
}
