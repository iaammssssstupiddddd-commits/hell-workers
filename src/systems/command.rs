use crate::constants::*;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::interface::camera::MainCamera;
use crate::interface::selection::SelectedEntity;
use crate::systems::jobs::{Designation, DesignationCreatedEvent, IssuedBy, Rock, Tree, WorkType};
use crate::systems::logistics::ResourceItem;
use bevy::prelude::*;

/// タスクモード - どのタスクを指定中か
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub enum TaskMode {
    #[default]
    None, // 通常モード
    DesignateChop,               // 伐採指示モード
    DesignateMine,               // 採掘指示モード
    DesignateHaul,               // 運搬指示モード
    SelectBuildTarget,           // 建築対象選択中
    AreaSelection(Option<Vec2>), // エリア選択モード (始点)
}
/// タスクエリア - 使い魔が担当するエリア
#[derive(Component, Clone)]
pub struct TaskArea {
    pub min: Vec2,
    pub max: Vec2,
}

impl TaskArea {
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) / 2.0
    }
    pub fn size(&self) -> Vec2 {
        (self.max - self.min).abs()
    }
    pub fn contains(&self, pos: Vec2) -> bool {
        pos.x >= self.min.x && pos.x <= self.max.x && pos.y >= self.min.y && pos.y <= self.max.y
    }
}

/// タスクエリア表示用
#[derive(Component)]
pub struct TaskAreaIndicator(pub Entity); // 親の使い魔Entity

/// キーボードで使い魔に指示を与えるシステム
/// 1 キー = 収集エリア選択モード
/// 2 キー = 建築対象選択モード
/// 3 キー = 運搬対象選択モード
/// Escape = キャンセル/待機に戻る
pub fn familiar_command_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    selected: Res<SelectedEntity>,
    q_familiars: Query<Entity, With<Familiar>>,
    mut q_active_commands: Query<&mut ActiveCommand>,
    mut task_mode: ResMut<TaskMode>,
) {
    // 選択されたエンティティが使い魔かチェック
    let Some(entity) = selected.0 else { return };
    if q_familiars.get(entity).is_err() {
        return;
    };

    if keyboard.just_pressed(KeyCode::Digit1) || keyboard.just_pressed(KeyCode::KeyC) {
        *task_mode = TaskMode::DesignateChop;
        info!("TASK_MODE: 伐採対象を選択してください（木をクリック）");
    } else if keyboard.just_pressed(KeyCode::Digit2) || keyboard.just_pressed(KeyCode::KeyM) {
        *task_mode = TaskMode::DesignateMine;
        info!("TASK_MODE: 採掘対象を選択してください（岩をクリック）");
    } else if keyboard.just_pressed(KeyCode::Digit3) || keyboard.just_pressed(KeyCode::KeyH) {
        *task_mode = TaskMode::DesignateHaul;
        info!("TASK_MODE: 運搬対象を選択してください（アイテムをクリック）");
    } else if keyboard.just_pressed(KeyCode::Digit4) || keyboard.just_pressed(KeyCode::KeyB) {
        *task_mode = TaskMode::SelectBuildTarget;
        info!("TASK_MODE: 建築対象を選択してください（Blueprintをクリック）");
    } else if keyboard.just_pressed(KeyCode::Escape) {
        *task_mode = TaskMode::None;
        // 待機状態に戻す
        if let Ok(mut active) = q_active_commands.get_mut(entity) {
            active.command = FamiliarCommand::Idle;
        }
        info!("TASK_MODE: キャンセル / 待機状態");
    }
}

/// エリア選択システム - クリックでエリアを指定
pub fn task_area_selection_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_ui: Query<&Interaction, With<Button>>,
    selected: Res<SelectedEntity>,
    mut task_mode: ResMut<TaskMode>,
    mut q_familiars: Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    q_targets: Query<(
        Entity,
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
    )>,
    mut commands: Commands,
    mut ev_created: EventWriter<DesignationCreatedEvent>,
) {
    if q_ui.iter().any(|i| *i != Interaction::None) {
        return;
    }

    if *task_mode == TaskMode::None {
        return;
    }

    if buttons.just_pressed(MouseButton::Left) {
        info!(
            "CLICK: World click detected while task_mode={:?}",
            *task_mode
        );
        let (camera, camera_transform) = q_camera.single();
        let window = q_window.single();

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                match *task_mode {
                    TaskMode::AreaSelection(None) => {
                        *task_mode = TaskMode::AreaSelection(Some(world_pos));
                    }
                    TaskMode::AreaSelection(Some(start_pos)) => {
                        let min_x = f32::min(start_pos.x, world_pos.x);
                        let max_x = f32::max(start_pos.x, world_pos.x);
                        let min_y = f32::min(start_pos.y, world_pos.y);
                        let max_y = f32::max(start_pos.y, world_pos.y);
                        let min = Vec2::new(min_x, min_y);
                        let max = Vec2::new(max_x, max_y);
                        let center = (min + max) / 2.0;

                        if let Some(fam_entity) = selected.0 {
                            if let Ok((mut active_command, mut familiar_dest)) =
                                q_familiars.get_mut(fam_entity)
                            {
                                commands.entity(fam_entity).insert(TaskArea { min, max });
                                familiar_dest.0 = center;
                                active_command.command = FamiliarCommand::Patrol;
                                info!(
                                    "AREA_ASSIGNMENT: Familiar {:?} assigned to rectangular area",
                                    fam_entity
                                );
                            }
                        }

                        *task_mode = TaskMode::None;
                    }
                    _ => {
                        // 使い魔が選択されていることを確認
                        let Some(fam_entity) = selected.0 else {
                            return;
                        };
                        if !q_familiars.contains(fam_entity) {
                            return;
                        }

                        let mut found_target = false;
                        // クリック位置に近いエンティティを探す
                        for (target_entity, transform, tree, rock, item) in q_targets.iter() {
                            let dist = transform.translation.truncate().distance(world_pos);
                            if dist < TILE_SIZE * 0.8 {
                                match *task_mode {
                                    TaskMode::DesignateChop if tree.is_some() => {
                                        commands.entity(target_entity).insert((
                                            Designation {
                                                work_type: WorkType::Chop,
                                            },
                                            IssuedBy(fam_entity),
                                        ));
                                        ev_created.send(DesignationCreatedEvent {
                                            entity: target_entity,
                                            work_type: WorkType::Chop,
                                            issued_by: fam_entity,
                                        });
                                        info!(
                                            "DESIGNATION: 伐採指示を出しました at {:?}",
                                            world_pos
                                        );
                                        found_target = true;
                                    }
                                    TaskMode::DesignateMine if rock.is_some() => {
                                        commands.entity(target_entity).insert((
                                            Designation {
                                                work_type: WorkType::Mine,
                                            },
                                            IssuedBy(fam_entity),
                                        ));
                                        ev_created.send(DesignationCreatedEvent {
                                            entity: target_entity,
                                            work_type: WorkType::Mine,
                                            issued_by: fam_entity,
                                        });
                                        info!(
                                            "DESIGNATION: 採掘指示を出しました at {:?}",
                                            world_pos
                                        );
                                        found_target = true;
                                    }
                                    TaskMode::DesignateHaul if item.is_some() => {
                                        commands.entity(target_entity).insert((
                                            Designation {
                                                work_type: WorkType::Haul,
                                            },
                                            IssuedBy(fam_entity),
                                        ));
                                        ev_created.send(DesignationCreatedEvent {
                                            entity: target_entity,
                                            work_type: WorkType::Haul,
                                            issued_by: fam_entity,
                                        });
                                        info!(
                                            "DESIGNATION: 運搬指示を出しました at {:?}",
                                            world_pos
                                        );
                                        found_target = true;
                                    }
                                    _ => {}
                                }
                            }
                        }

                        if found_target {
                            // 使い魔が選択されている場合は、その方向へ向かわせる（監督動作）
                            if let Some(entity) = selected.0 {
                                if let Ok((mut active_command, mut familiar_dest)) =
                                    q_familiars.get_mut(entity)
                                {
                                    familiar_dest.0 = world_pos;
                                    active_command.command = FamiliarCommand::Patrol;
                                }
                            }
                            *task_mode = TaskMode::None;
                        }
                    }
                }
            }
        }
    }
}

/// エリア選択の表示システム
pub fn area_selection_indicator_system(
    task_mode: Res<TaskMode>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut q_indicator: Query<
        (Entity, &mut Transform, &mut Sprite, &mut Visibility),
        With<AreaSelectionIndicator>,
    >,
    mut commands: Commands,
) {
    if let TaskMode::AreaSelection(Some(start_pos)) = *task_mode {
        let (camera, camera_transform) = q_camera.single();
        let window = q_window.single();

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                let center = (start_pos + world_pos) / 2.0;
                let size = (start_pos - world_pos).abs();

                if let Ok((_, mut transform, mut sprite, mut visibility)) =
                    q_indicator.get_single_mut()
                {
                    transform.translation = center.extend(0.6);
                    sprite.custom_size = Some(size);
                    *visibility = Visibility::Visible;
                } else {
                    commands.spawn((
                        AreaSelectionIndicator,
                        Sprite {
                            color: Color::srgba(1.0, 1.0, 1.0, 0.2),
                            custom_size: Some(size),
                            ..default()
                        },
                        Transform::from_translation(center.extend(0.6)),
                    ));
                }
            }
        }
    } else {
        if let Ok((_, _, _, mut visibility)) = q_indicator.get_single_mut() {
            *visibility = Visibility::Hidden;
        }
    }
}

/// タスクエリアの表示システム
pub fn task_area_indicator_system(
    q_familiars: Query<(Entity, &Transform, &TaskArea), With<Familiar>>,
    mut q_indicators: Query<
        (
            Entity,
            &TaskAreaIndicator,
            &mut Transform,
            &mut Visibility,
            &mut Sprite,
        ),
        Without<Familiar>,
    >,
    mut commands: Commands,
) {
    // 既存のインジケーターを更新
    for (indicator_entity, indicator, mut transform, mut visibility, mut sprite) in
        q_indicators.iter_mut()
    {
        if let Ok((_, _, task_area)) = q_familiars.get(indicator.0) {
            transform.translation = task_area.center().extend(0.2);
            sprite.custom_size = Some(task_area.size());
            *visibility = Visibility::Visible;
        } else {
            // 使い魔が存在しないかTaskAreaがない
            commands.entity(indicator_entity).despawn();
        }
    }

    // 新しいTaskAreaにインジケーターを作成
    for (fam_entity, _, task_area) in q_familiars.iter() {
        let has_indicator = q_indicators
            .iter()
            .any(|(_, ind, _, _, _)| ind.0 == fam_entity);

        if !has_indicator {
            commands.spawn((
                TaskAreaIndicator(fam_entity),
                Sprite {
                    color: Color::srgba(0.0, 1.0, 0.0, 0.15), // 緑の半透明
                    custom_size: Some(task_area.size()),
                    ..default()
                },
                Transform::from_translation(task_area.center().extend(0.2)),
            ));
        }
    }
}

/// 指定（Designation）の可視化
pub fn designation_visual_system(
    mut commands: Commands,
    q_designated: Query<(Entity, &Transform, &Designation), Changed<Designation>>,
) {
    for (entity, transform, designation) in q_designated.iter() {
        // インジケーターを作成（RimWorldの白い枠線や赤いバツ印のイメージ）
        let color = match designation.work_type {
            WorkType::Chop => Color::srgb(0.0, 1.0, 0.0), // 緑
            WorkType::Mine => Color::srgb(1.0, 0.0, 0.0), // 赤
            WorkType::Haul => Color::srgb(0.0, 0.0, 1.0), // 青
            _ => Color::WHITE,
        };

        commands.spawn((
            DesignationIndicator(entity),
            Sprite {
                color: color.with_alpha(0.3),
                custom_size: Some(Vec2::splat(TILE_SIZE * 1.1)),
                ..default()
            },
            Transform::from_translation(transform.translation.truncate().extend(0.5)),
        ));
    }
}

#[derive(Component)]
pub struct DesignationIndicator(pub Entity);

#[derive(Component)]
pub struct AreaSelectionIndicator;

pub fn update_designation_indicator_system(
    mut commands: Commands,
    q_designated: Query<Entity, With<Designation>>,
    q_indicators: Query<(Entity, &DesignationIndicator)>,
) {
    for (indicator_entity, indicator) in q_indicators.iter() {
        if q_designated.get(indicator.0).is_err() {
            commands.entity(indicator_entity).despawn();
        }
    }
}

/// 使い魔コマンドのビジュアルフィードバック
pub fn familiar_command_visual_system(
    task_mode: Res<TaskMode>,
    mut q_familiars: Query<(&ActiveCommand, &mut Sprite), With<Familiar>>,
) {
    for (command, mut sprite) in q_familiars.iter_mut() {
        // タスクモード中は点滅
        if *task_mode != TaskMode::None {
            sprite.color = Color::srgb(1.0, 1.0, 1.0); // 白く光る
            return;
        }

        match command.command {
            FamiliarCommand::Idle => {
                sprite.color = Color::srgb(0.6, 0.2, 0.2);
            }
            FamiliarCommand::GatherResources => {
                sprite.color = Color::srgb(1.0, 0.6, 0.2);
            }
            FamiliarCommand::Patrol => {
                sprite.color = Color::srgb(1.0, 0.3, 0.3);
            }
            FamiliarCommand::Construct(_) => {
                sprite.color = Color::srgb(1.0, 1.0, 0.3);
            }
        }
    }
}
