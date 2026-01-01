use bevy::prelude::*;
use crate::constants::*;
use crate::entities::damned_soul::{Destination, Path};
use crate::entities::familiar::{Familiar, ActiveCommand, FamiliarCommand};
use crate::interface::selection::SelectedEntity;
use crate::interface::camera::MainCamera;

/// タスクモード - どのタスクを指定中か
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub enum TaskMode {
    #[default]
    None,               // 通常モード
    SelectGatherArea,   // 収集エリア選択中
    SelectBuildTarget,  // 建築対象選択中
    SelectHaulTarget,   // 運搬対象選択中
}

/// タスクエリア - 使い魔が担当するエリア
#[derive(Component, Clone)]
pub struct TaskArea {
    pub center: Vec2,
    pub radius: f32,
}

/// タスクエリア表示用
#[derive(Component)]
pub struct TaskAreaIndicator(pub Entity);  // 親の使い魔Entity

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
    if q_familiars.get(entity).is_err() { return };

    if keyboard.just_pressed(KeyCode::Digit1) || keyboard.just_pressed(KeyCode::KeyG) {
        *task_mode = TaskMode::SelectGatherArea;
        info!("TASK_MODE: 収集エリアを選択してください（左クリックで指定）");
    } else if keyboard.just_pressed(KeyCode::Digit2) || keyboard.just_pressed(KeyCode::KeyB) {
        *task_mode = TaskMode::SelectBuildTarget;
        info!("TASK_MODE: 建築対象を選択してください（Blueprintをクリック）");
    } else if keyboard.just_pressed(KeyCode::Digit3) || keyboard.just_pressed(KeyCode::KeyH) {
        *task_mode = TaskMode::SelectHaulTarget;
        info!("TASK_MODE: 運搬対象を選択してください（Stockpileをクリック）");
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
    mut q_familiars: Query<(&mut ActiveCommand, Option<&mut TaskArea>, &mut Destination), With<Familiar>>,
    mut commands: Commands,
) {
    // UIがクリックされている場合は無視
    for interaction in q_ui.iter() {
        if *interaction != Interaction::None {
            return;
        }
    }

    // タスクモードがNoneなら何もしない
    if *task_mode == TaskMode::None {
        return;
    }

    // 使い魔が選択されていない場合は何もしない
    let Some(entity) = selected.0 else { return };
    let Ok((mut active_command, existing_area, mut familiar_dest)) = q_familiars.get_mut(entity) else { return };

    if buttons.just_pressed(MouseButton::Left) {
        let (camera, camera_transform) = q_camera.single();
        let window = q_window.single();

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                match *task_mode {
                    TaskMode::SelectGatherArea => {
                        // 収集エリアを設定し、使い魔をそこへ向かわせる（半径を拡大 5 -> 8）
                        let area = TaskArea {
                            center: world_pos,
                            radius: TILE_SIZE * 8.0,
                        };
                        
                        if existing_area.is_some() {
                            commands.entity(entity).remove::<TaskArea>();
                        }
                        commands.entity(entity).insert(area.clone());
                        
                        active_command.command = FamiliarCommand::GatherResources;
                        familiar_dest.0 = world_pos;
                        info!("TASK: 収集エリアを設定し、使い魔が移動開始 at {:?}", world_pos);
                    }
                    TaskMode::SelectBuildTarget => {
                        // TODO: Blueprint選択
                        active_command.command = FamiliarCommand::Patrol;  // 仮
                        info!("TASK: 建築監督モード（未実装）");
                    }
                    TaskMode::SelectHaulTarget => {
                        // TODO: Stockpile選択
                        active_command.command = FamiliarCommand::Patrol;  // 仮
                        info!("TASK: 運搬監督モード（未実装）");
                    }
                    TaskMode::None => {}
                }
                
                *task_mode = TaskMode::None;
            }
        }
    }
}

/// タスクエリアの表示システム
pub fn task_area_indicator_system(
    q_familiars: Query<(Entity, &Transform, &TaskArea), With<Familiar>>,
    mut q_indicators: Query<(Entity, &TaskAreaIndicator, &mut Transform, &mut Visibility), Without<Familiar>>,
    mut commands: Commands,
) {
    // 既存のインジケーターを更新
    for (indicator_entity, indicator, mut transform, mut visibility) in q_indicators.iter_mut() {
        if let Ok((_, _, task_area)) = q_familiars.get(indicator.0) {
            transform.translation = task_area.center.extend(0.2);
            *visibility = Visibility::Visible;
        } else {
            // 使い魔が存在しないかTaskAreaがない
            commands.entity(indicator_entity).despawn();
        }
    }

    // 新しいTaskAreaにインジケーターを作成
    for (fam_entity, _, task_area) in q_familiars.iter() {
        let has_indicator = q_indicators.iter().any(|(_, ind, _, _)| ind.0 == fam_entity);
        
        if !has_indicator {
            commands.spawn((
                TaskAreaIndicator(fam_entity),
                Sprite {
                    color: Color::srgba(0.0, 1.0, 0.0, 0.15),  // 緑の半透明
                    custom_size: Some(Vec2::splat(task_area.radius * 2.0)),
                    ..default()
                },
                Transform::from_translation(task_area.center.extend(0.2)),
            ));
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
            sprite.color = Color::srgb(1.0, 1.0, 1.0);  // 白く光る
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

/// タスクモード中のUIヒント表示用
#[derive(Component)]
pub struct TaskModeHint;
