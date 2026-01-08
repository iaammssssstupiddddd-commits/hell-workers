use crate::constants::*;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::interface::camera::MainCamera;
use crate::interface::selection::SelectedEntity;
use crate::systems::jobs::{
    Designation, DesignationCreatedEvent, IssuedBy, Rock, TaskSlots, Tree, WorkType,
};
use crate::systems::logistics::ResourceItem;
use crate::systems::work::GlobalTaskQueue;
use bevy::prelude::*;

/// タスクモード - どのタスクを指定中か
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub enum TaskMode {
    #[default]
    None, // 通常モード
    DesignateChop(Option<Vec2>),     // 伐採指示モード (ドラッグ開始位置)
    DesignateMine(Option<Vec2>),     // 採掘指示モード (ドラッグ開始位置)
    DesignateHaul(Option<Vec2>),     // 運搬指示モード (ドラッグ開始位置)
    CancelDesignation(Option<Vec2>), // 指示キャンセルモード (ドラッグ開始位置)
    SelectBuildTarget,               // 建築対象選択中
    AreaSelection(Option<Vec2>),     // エリア選択モード (始点)
    AssignTask(Option<Vec2>),        // 未アサインタスクを使い魔に割り当てるモード
}
/// タスクエリア - 使い魔が担当するエリア
#[derive(Component, Clone, Debug)]
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
        *task_mode = TaskMode::DesignateChop(None);
        info!("TASK_MODE: 伐採対象を範囲指定（ドラッグ）またはクリックしてください");
    } else if keyboard.just_pressed(KeyCode::Digit2) || keyboard.just_pressed(KeyCode::KeyM) {
        *task_mode = TaskMode::DesignateMine(None);
        info!("TASK_MODE: 採掘対象を範囲指定（ドラッグ）またはクリックしてください");
    } else if keyboard.just_pressed(KeyCode::Digit3) || keyboard.just_pressed(KeyCode::KeyH) {
        *task_mode = TaskMode::DesignateHaul(None);
        info!("TASK_MODE: 運搬対象を範囲指定（ドラッグ）またはクリックしてください");
    } else if keyboard.just_pressed(KeyCode::Digit4) || keyboard.just_pressed(KeyCode::KeyB) {
        *task_mode = TaskMode::SelectBuildTarget;
        info!("TASK_MODE: 建築対象を選択してください（Blueprintをクリック）");
    } else if keyboard.just_pressed(KeyCode::Digit0) || keyboard.just_pressed(KeyCode::Delete) {
        *task_mode = TaskMode::CancelDesignation(None);
        info!("TASK_MODE: 指示をキャンセルする範囲を指定してください");
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
    keyboard: Res<ButtonInput<KeyCode>>,
    // 未アサインタスク割り当て用
    q_unassigned: Query<(Entity, &Transform, &Designation), Without<IssuedBy>>,
    mut global_queue: ResMut<GlobalTaskQueue>,
    mut queue: ResMut<crate::systems::work::TaskQueue>,
) {
    if q_ui.iter().any(|i| *i != Interaction::None) {
        return;
    }

    if *task_mode == TaskMode::None {
        return;
    }

    if buttons.just_pressed(MouseButton::Left) {
        let (camera, camera_transform) = q_camera.single();
        let window = q_window.single();
        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                match *task_mode {
                    TaskMode::AreaSelection(None) => {
                        *task_mode = TaskMode::AreaSelection(Some(world_pos))
                    }
                    TaskMode::DesignateChop(None) => {
                        *task_mode = TaskMode::DesignateChop(Some(world_pos))
                    }
                    TaskMode::DesignateMine(None) => {
                        *task_mode = TaskMode::DesignateMine(Some(world_pos))
                    }
                    TaskMode::DesignateHaul(None) => {
                        *task_mode = TaskMode::DesignateHaul(Some(world_pos))
                    }
                    TaskMode::CancelDesignation(None) => {
                        *task_mode = TaskMode::CancelDesignation(Some(world_pos))
                    }
                    TaskMode::AssignTask(None) => {
                        *task_mode = TaskMode::AssignTask(Some(world_pos))
                    }
                    _ => {}
                }
            }
        }
    }

    if buttons.just_released(MouseButton::Left) {
        let (camera, camera_transform) = q_camera.single();
        let window = q_window.single();

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                match *task_mode {
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

                                // 範囲内の未アサインタスクも割り当てる
                                let mut assigned_count = 0;
                                for (task_entity, task_transform, designation) in
                                    q_unassigned.iter()
                                {
                                    let pos = task_transform.translation.truncate();
                                    if pos.x >= min_x - 0.1
                                        && pos.x <= max_x + 0.1
                                        && pos.y >= min_y - 0.1
                                        && pos.y <= max_y + 0.1
                                    {
                                        commands.entity(task_entity).insert(IssuedBy(fam_entity));
                                        global_queue.remove(task_entity);
                                        queue.add(
                                            fam_entity,
                                            crate::systems::work::PendingTask {
                                                entity: task_entity,
                                                work_type: designation.work_type,
                                                priority: 0,
                                            },
                                        );
                                        assigned_count += 1;
                                    }
                                }
                                if assigned_count > 0 {
                                    info!(
                                        "AREA_ASSIGNMENT: Also assigned {} unassigned task(s) to Familiar {:?}",
                                        assigned_count, fam_entity
                                    );
                                }
                            }
                        }
                        *task_mode = TaskMode::None;
                    }
                    TaskMode::DesignateChop(Some(start_pos))
                    | TaskMode::DesignateMine(Some(start_pos))
                    | TaskMode::DesignateHaul(Some(start_pos))
                    | TaskMode::CancelDesignation(Some(start_pos)) => {
                        let min_x = f32::min(start_pos.x, world_pos.x);
                        let max_x = f32::max(start_pos.x, world_pos.x);
                        let min_y = f32::min(start_pos.y, world_pos.y);
                        let max_y = f32::max(start_pos.y, world_pos.y);

                        let work_type = match *task_mode {
                            TaskMode::DesignateChop(_) => Some(WorkType::Chop),
                            TaskMode::DesignateMine(_) => Some(WorkType::Mine),
                            TaskMode::DesignateHaul(_) => Some(WorkType::Haul),
                            _ => None,
                        };

                        let priority = if keyboard.pressed(KeyCode::ShiftLeft)
                            || keyboard.pressed(KeyCode::ShiftRight)
                        {
                            1
                        } else {
                            0
                        };
                        let fam_entity = selected.0;

                        for (target_entity, transform, tree, rock, item) in q_targets.iter() {
                            let pos = transform.translation.truncate();
                            // 少しだけマージンを持たせる (0.1タイル分)
                            if pos.x >= min_x - 0.1
                                && pos.x <= max_x + 0.1
                                && pos.y >= min_y - 0.1
                                && pos.y <= max_y + 0.1
                            {
                                if let Some(wt) = work_type {
                                    let match_found = match wt {
                                        WorkType::Chop => tree.is_some(),
                                        WorkType::Mine => rock.is_some(),
                                        WorkType::Haul => item.is_some(),
                                        _ => false,
                                    };

                                    if match_found {
                                        // 使い魔が選択されていれば IssuedBy を付与
                                        if let Some(issued_by) = fam_entity {
                                            commands.entity(target_entity).insert((
                                                Designation { work_type: wt },
                                                IssuedBy(issued_by),
                                                TaskSlots::new(1),
                                            ));
                                            info!(
                                                "DESIGNATION: Created {:?} for {:?} (assigned to {:?})",
                                                wt, target_entity, issued_by
                                            );
                                        } else {
                                            // 未アサインの場合は Designation のみ
                                            commands.entity(target_entity).insert((
                                                Designation { work_type: wt },
                                                TaskSlots::new(1),
                                            ));
                                            info!(
                                                "DESIGNATION: Created {:?} for {:?} (unassigned)",
                                                wt, target_entity
                                            );
                                        }
                                        ev_created.send(DesignationCreatedEvent {
                                            entity: target_entity,
                                            work_type: wt,
                                            issued_by: fam_entity, // Option<Entity>
                                            priority,
                                        });
                                    }
                                } else {
                                    // キャンセルモード
                                    commands.entity(target_entity).remove::<Designation>();
                                    commands.entity(target_entity).remove::<TaskSlots>();
                                    commands.entity(target_entity).remove::<IssuedBy>();
                                }
                            }
                        }

                        // ドラッグ終了後にモードは維持するが、開始位置をクリア
                        *task_mode = match *task_mode {
                            TaskMode::DesignateChop(_) => TaskMode::DesignateChop(None),
                            TaskMode::DesignateMine(_) => TaskMode::DesignateMine(None),
                            TaskMode::DesignateHaul(_) => TaskMode::DesignateHaul(None),
                            TaskMode::CancelDesignation(_) => TaskMode::CancelDesignation(None),
                            _ => TaskMode::None,
                        };
                    }
                    _ => {}
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
    let drag_start = match *task_mode {
        TaskMode::AreaSelection(s) => s,
        TaskMode::DesignateChop(s) => s,
        TaskMode::DesignateMine(s) => s,
        TaskMode::DesignateHaul(s) => s,
        TaskMode::CancelDesignation(s) => s,
        _ => None,
    };

    if let Some(start_pos) = drag_start {
        let (camera, camera_transform) = q_camera.single();
        let window = q_window.single();

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                let center = (start_pos + world_pos) / 2.0;
                let size = (start_pos - world_pos).abs();

                let color = match *task_mode {
                    TaskMode::AreaSelection(_) => Color::srgba(1.0, 1.0, 1.0, 0.2), // 白
                    TaskMode::CancelDesignation(_) => Color::srgba(1.0, 0.2, 0.2, 0.3), // 赤
                    _ => Color::srgba(0.2, 1.0, 0.2, 0.3),                          // 緑系
                };

                if let Ok((_, mut transform, mut sprite, mut visibility)) =
                    q_indicator.get_single_mut()
                {
                    transform.translation = center.extend(0.6);
                    sprite.custom_size = Some(size);
                    sprite.color = color;
                    *visibility = Visibility::Visible;
                } else {
                    commands.spawn((
                        AreaSelectionIndicator,
                        Sprite {
                            color: color,
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

/// 指定（Designation）の削除を検知してインジケーターを削除
pub fn update_designation_indicator_system(
    mut commands: Commands,
    mut removed: RemovedComponents<Designation>,
    q_indicators: Query<(Entity, &DesignationIndicator)>,
) {
    for entity in removed.read() {
        for (indicator_entity, indicator) in q_indicators.iter() {
            if indicator.0 == entity {
                commands.entity(indicator_entity).despawn();
            }
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

/// 未アサインタスクを使い魔に割り当てるシステム
pub fn assign_task_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_ui: Query<&Interaction, With<Button>>,
    selected: Res<SelectedEntity>,
    mut task_mode: ResMut<TaskMode>,
    mut global_queue: ResMut<GlobalTaskQueue>,
    mut queue: ResMut<crate::systems::work::TaskQueue>,
    mut commands: Commands,
    q_designations: Query<(Entity, &Transform, &Designation), Without<IssuedBy>>,
    q_familiars: Query<Entity, With<Familiar>>,
) {
    if q_ui.iter().any(|i| *i != Interaction::None) {
        return;
    }

    // AssignTask モードでなければ何もしない
    let TaskMode::AssignTask(Some(start_pos)) = *task_mode else {
        return;
    };

    if !buttons.just_released(MouseButton::Left) {
        return;
    }

    info!("ASSIGN_TASK: Drag released, processing assignment...");

    let Ok((camera, camera_transform)) = q_camera.get_single() else {
        return;
    };
    let Ok(window) = q_window.get_single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    // 使い魔が選択されていなければ何もしない
    let Some(fam_entity) = selected.0 else {
        info!("ASSIGN_TASK: No entity selected");
        *task_mode = TaskMode::AssignTask(None);
        return;
    };

    // 選択されたエンティティが使い魔かどうかを確認
    if q_familiars.get(fam_entity).is_err() {
        info!(
            "ASSIGN_TASK: Selected entity {:?} is not a familiar",
            fam_entity
        );
        *task_mode = TaskMode::AssignTask(None);
        return;
    }

    let min_x = f32::min(start_pos.x, world_pos.x);
    let max_x = f32::max(start_pos.x, world_pos.x);
    let min_y = f32::min(start_pos.y, world_pos.y);
    let max_y = f32::max(start_pos.y, world_pos.y);

    info!(
        "ASSIGN_TASK: Searching in area ({:.1},{:.1}) to ({:.1},{:.1})",
        min_x, min_y, max_x, max_y
    );

    let unassigned_count = q_designations.iter().count();
    info!(
        "ASSIGN_TASK: Found {} unassigned designations total",
        unassigned_count
    );

    let mut assigned_count = 0;

    for (entity, transform, designation) in q_designations.iter() {
        let pos = transform.translation.truncate();
        if pos.x >= min_x - 0.1
            && pos.x <= max_x + 0.1
            && pos.y >= min_y - 0.1
            && pos.y <= max_y + 0.1
        {
            // IssuedBy を付与して使い魔に割り当て
            commands.entity(entity).insert(IssuedBy(fam_entity));

            // GlobalQueue から削除して TaskQueue に追加
            global_queue.remove(entity);
            queue.add(
                fam_entity,
                crate::systems::work::PendingTask {
                    entity,
                    work_type: designation.work_type,
                    priority: 0,
                },
            );

            assigned_count += 1;
            info!(
                "ASSIGN_TASK: Assigned {:?} ({:?}) to Familiar {:?}",
                entity, designation.work_type, fam_entity
            );
        }
    }

    if assigned_count > 0 {
        info!(
            "ASSIGN_TASK: Assigned {} task(s) to Familiar {:?}",
            assigned_count, fam_entity
        );
    } else {
        info!("ASSIGN_TASK: No tasks found in selected area");
    }

    *task_mode = TaskMode::AssignTask(None);
}
