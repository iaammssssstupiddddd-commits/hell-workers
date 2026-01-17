use super::TaskMode;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::game_state::TaskContext;
use crate::interface::selection::SelectedEntity;
use bevy::prelude::*;

/// キーボードで使い魔に指示を与えるシステム
pub fn familiar_command_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    selected: Res<SelectedEntity>,
    q_familiars: Query<Entity, With<Familiar>>,
    mut q_active_commands: Query<&mut ActiveCommand>,
    mut task_context: ResMut<TaskContext>,
) {
    // 選択されたエンティティが使い魔の場合のみ処理（Soulは直接指示不可）
    let Some(entity) = selected.0 else { return };
    if q_familiars.get(entity).is_err() {
        return;
    }

    if keyboard.just_pressed(KeyCode::Digit1) || keyboard.just_pressed(KeyCode::KeyC) {
        task_context.0 = TaskMode::DesignateChop(None);
        info!("TASK_MODE: 伐採対象を範囲指定（ドラッグ）またはクリックしてください");
    } else if keyboard.just_pressed(KeyCode::Digit2) || keyboard.just_pressed(KeyCode::KeyM) {
        task_context.0 = TaskMode::DesignateMine(None);
        info!("TASK_MODE: 採掘対象を範囲指定（ドラッグ）またはクリックしてください");
    } else if keyboard.just_pressed(KeyCode::Digit3) || keyboard.just_pressed(KeyCode::KeyH) {
        task_context.0 = TaskMode::DesignateHaul(None);
        info!("TASK_MODE: 運搬対象を範囲指定（ドラッグ）またはクリックしてください");
    } else if keyboard.just_pressed(KeyCode::Digit4) || keyboard.just_pressed(KeyCode::KeyB) {
        task_context.0 = TaskMode::SelectBuildTarget;
        info!("TASK_MODE: 建築対象を選択してください（Blueprintをクリック）");
    } else if keyboard.just_pressed(KeyCode::Digit0) || keyboard.just_pressed(KeyCode::Delete) {
        task_context.0 = TaskMode::CancelDesignation(None);
        info!("TASK_MODE: 指示をキャンセルする範囲を指定してください");
    } else if keyboard.just_pressed(KeyCode::Escape) {
        task_context.0 = TaskMode::None;
        // 待機状態に戻す
        if let Ok(mut active) = q_active_commands.get_mut(entity) {
            active.command = FamiliarCommand::Idle;
        }
        info!("TASK_MODE: キャンセル / 待機状態");
    }
}
