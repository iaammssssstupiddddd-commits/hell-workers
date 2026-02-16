use super::{TaskArea, TaskMode};
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::game_state::TaskContext;
use crate::interface::selection::SelectedEntity;
use bevy::prelude::*;

/// キーボードで使い魔に指示を与えるシステム
pub fn familiar_command_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    selected: Res<SelectedEntity>,
    q_familiars: Query<(), With<Familiar>>,
    mut q_active_commands: Query<(&mut ActiveCommand, Option<&TaskArea>), With<Familiar>>,
    mut task_context: ResMut<TaskContext>,
) {
    // 選択されたエンティティが使い魔の場合のみ処理（Soulは直接指示不可）
    let Some(entity) = selected.0 else { return };
    if q_familiars.get(entity).is_err() {
        return;
    }

    if keyboard.just_pressed(KeyCode::Digit1) || keyboard.just_pressed(KeyCode::KeyC) {
        task_context.0 = TaskMode::DesignateChop(None);
    } else if keyboard.just_pressed(KeyCode::Digit2) || keyboard.just_pressed(KeyCode::KeyM) {
        task_context.0 = TaskMode::DesignateMine(None);
    } else if keyboard.just_pressed(KeyCode::Digit3) || keyboard.just_pressed(KeyCode::KeyH) {
        task_context.0 = TaskMode::DesignateHaul(None);
    } else if keyboard.just_pressed(KeyCode::Digit4) || keyboard.just_pressed(KeyCode::KeyB) {
        task_context.0 = TaskMode::SelectBuildTarget;
    } else if keyboard.just_pressed(KeyCode::Digit0) || keyboard.just_pressed(KeyCode::Delete) {
        task_context.0 = TaskMode::CancelDesignation(None);
    } else if keyboard.just_pressed(KeyCode::Escape) {
        task_context.0 = TaskMode::None;
        if let Ok((mut active, area_opt)) = q_active_commands.get_mut(entity) {
            if matches!(active.command, FamiliarCommand::Idle) && area_opt.is_some() {
                active.command = FamiliarCommand::Patrol;
            } else {
                active.command = FamiliarCommand::Idle;
            }
        }
    }
}
