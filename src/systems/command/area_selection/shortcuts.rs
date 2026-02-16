use super::apply::{apply_area_and_record_history, apply_task_area_to_familiar};
use super::geometry::{area_from_center_and_size, hotkey_slot_index};
use super::state::{AreaEditClipboard, AreaEditHistory, AreaEditPresets};
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar};
use crate::game_state::TaskContext;
use crate::interface::selection::SelectedEntity;
use crate::systems::command::{TaskArea, TaskMode};
use bevy::prelude::*;

pub fn task_area_edit_history_shortcuts_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    task_context: Res<TaskContext>,
    mut selected_entity: ResMut<SelectedEntity>,
    mut area_edit_history: ResMut<AreaEditHistory>,
    mut area_edit_clipboard: ResMut<AreaEditClipboard>,
    mut area_edit_presets: ResMut<AreaEditPresets>,
    q_familiar_exists: Query<(), With<Familiar>>,
    q_task_areas: Query<&TaskArea, With<Familiar>>,
    mut q_familiars: Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    mut commands: Commands,
) {
    if !matches!(task_context.0, TaskMode::AreaSelection(_)) {
        return;
    }

    let ctrl_pressed =
        keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let alt_pressed = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);

    if alt_pressed && let Some(slot) = hotkey_slot_index(&keyboard) {
        let Some(selected) = selected_entity.0 else {
            return;
        };
        if q_familiar_exists.get(selected).is_err() {
            return;
        }

        let Some(preset_size) = area_edit_presets.get_size(slot) else {
            return;
        };

        let before = q_task_areas.get(selected).ok().cloned();
        let center = if let Some(area) = before.as_ref() {
            area.center()
        } else if let Ok((_, dest)) = q_familiars.get_mut(selected) {
            dest.0
        } else {
            return;
        };

        let new_area = area_from_center_and_size(center, preset_size);
        apply_area_and_record_history(
            selected,
            &new_area,
            before.clone(),
            &mut commands,
            &mut q_familiars,
            &mut area_edit_history,
        );
        return;
    }

    if !ctrl_pressed {
        return;
    }

    if let Some(slot) = hotkey_slot_index(&keyboard) {
        if let Some(selected) = selected_entity.0
            && q_familiar_exists.get(selected).is_ok()
        {
            if let Ok(area) = q_task_areas.get(selected) {
                area_edit_presets.save_size(slot, area.size());
            }
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyC) {
        if let Some(selected) = selected_entity.0
            && q_familiar_exists.get(selected).is_ok()
        {
            area_edit_clipboard.area = q_task_areas.get(selected).ok().cloned();
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyV) {
        let Some(selected) = selected_entity.0 else {
            return;
        };
        if q_familiar_exists.get(selected).is_err() {
            return;
        }

        let Some(copied_area) = area_edit_clipboard.area.clone() else {
            return;
        };

        let before = q_task_areas.get(selected).ok().cloned();
        apply_area_and_record_history(
            selected,
            &copied_area,
            before,
            &mut commands,
            &mut q_familiars,
            &mut area_edit_history,
        );
        return;
    }

    let redo_via_shift_z = keyboard.just_pressed(KeyCode::KeyZ)
        && (keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight));

    if keyboard.just_pressed(KeyCode::KeyY) || redo_via_shift_z {
        if let Some(entry) = area_edit_history.redo_stack.pop() {
            let familiar_entity = entry.familiar_entity;
            apply_task_area_to_familiar(
                familiar_entity,
                entry.after.as_ref(),
                &mut commands,
                &mut q_familiars,
            );
            selected_entity.0 = Some(familiar_entity);
            area_edit_history.undo_stack.push(entry);
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyZ)
        && let Some(entry) = area_edit_history.undo_stack.pop()
    {
        let familiar_entity = entry.familiar_entity;
        super::apply::apply_task_area_to_familiar(
            familiar_entity,
            entry.before.as_ref(),
            &mut commands,
            &mut q_familiars,
        );
        selected_entity.0 = Some(familiar_entity);
        area_edit_history.redo_stack.push(entry);
    }
}
