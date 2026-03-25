use super::apply::{apply_area_and_record_history, apply_task_area_to_familiar};
use super::geometry::{area_from_center_and_size, hotkey_slot_index};
use super::{AreaEditClipboard, AreaEditHistory, AreaEditPresets};
use crate::app_contexts::TaskContext;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar};
use crate::interface::selection::SelectedEntity;
use crate::systems::command::{TaskArea, TaskMode};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_world::zones::Site;

#[derive(SystemParam)]
pub struct ShortcutResources<'w> {
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    task_context: Res<'w, TaskContext>,
    selected_entity: ResMut<'w, SelectedEntity>,
    area_edit_history: ResMut<'w, AreaEditHistory>,
    area_edit_clipboard: ResMut<'w, AreaEditClipboard>,
    area_edit_presets: ResMut<'w, AreaEditPresets>,
}

#[derive(SystemParam)]
pub struct ShortcutQueries<'w, 's> {
    q_familiar_exists: Query<'w, 's, (), With<Familiar>>,
    q_task_areas: Query<'w, 's, &'static TaskArea, With<Familiar>>,
    q_sites: Query<'w, 's, &'static Site>,
    q_familiars: Query<'w, 's, (&'static mut ActiveCommand, &'static mut Destination), With<Familiar>>,
}

pub fn task_area_edit_history_shortcuts_system(
    mut res: ShortcutResources,
    mut queries: ShortcutQueries,
    mut commands: Commands,
) {
    if !matches!(res.task_context.0, TaskMode::AreaSelection(_)) {
        return;
    }

    let ctrl_pressed =
        res.keyboard.pressed(KeyCode::ControlLeft) || res.keyboard.pressed(KeyCode::ControlRight);
    let alt_pressed = res.keyboard.pressed(KeyCode::AltLeft) || res.keyboard.pressed(KeyCode::AltRight);

    if alt_pressed && let Some(slot) = hotkey_slot_index(&res.keyboard) {
        let Some(selected) = res.selected_entity.0 else {
            return;
        };
        if queries.q_familiar_exists.get(selected).is_err() {
            return;
        }

        let Some(preset_size) = res.area_edit_presets.get_size(slot) else {
            return;
        };

        let before = queries.q_task_areas.get(selected).ok().cloned();
        let center = if let Some(area) = before.as_ref() {
            area.center()
        } else if let Ok((_, dest)) = queries.q_familiars.get_mut(selected) {
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
            &mut queries.q_familiars,
            &mut res.area_edit_history,
            &queries.q_sites,
        );
        return;
    }

    if !ctrl_pressed {
        return;
    }

    if let Some(slot) = hotkey_slot_index(&res.keyboard) {
        if let Some(selected) = res.selected_entity.0
            && queries.q_familiar_exists.get(selected).is_ok()
            && let Ok(area) = queries.q_task_areas.get(selected) {
                res.area_edit_presets.save_size(slot, area.size());
            }
        return;
    }

    if res.keyboard.just_pressed(KeyCode::KeyC) {
        if let Some(selected) = res.selected_entity.0
            && queries.q_familiar_exists.get(selected).is_ok()
        {
            res.area_edit_clipboard.area = queries.q_task_areas.get(selected).ok().cloned();
        }
        return;
    }

    if res.keyboard.just_pressed(KeyCode::KeyV) {
        let Some(selected) = res.selected_entity.0 else {
            return;
        };
        if queries.q_familiar_exists.get(selected).is_err() {
            return;
        }

        let Some(copied_area) = res.area_edit_clipboard.area.clone() else {
            return;
        };

        let before = queries.q_task_areas.get(selected).ok().cloned();
        apply_area_and_record_history(
            selected,
            &copied_area,
            before,
            &mut commands,
            &mut queries.q_familiars,
            &mut res.area_edit_history,
            &queries.q_sites,
        );
        return;
    }

    let redo_via_shift_z = res.keyboard.just_pressed(KeyCode::KeyZ)
        && (res.keyboard.pressed(KeyCode::ShiftLeft) || res.keyboard.pressed(KeyCode::ShiftRight));

    if res.keyboard.just_pressed(KeyCode::KeyY) || redo_via_shift_z {
        if let Some(entry) = res.area_edit_history.redo_stack.pop() {
            let familiar_entity = entry.familiar_entity;
            apply_task_area_to_familiar(
                familiar_entity,
                entry.after.as_ref(),
                &mut commands,
                &mut queries.q_familiars,
            );
            res.selected_entity.0 = Some(familiar_entity);
            res.area_edit_history.undo_stack.push(entry);
        }
        return;
    }

    if res.keyboard.just_pressed(KeyCode::KeyZ)
        && let Some(entry) = res.area_edit_history.undo_stack.pop()
    {
        let familiar_entity = entry.familiar_entity;
        super::apply::apply_task_area_to_familiar(
            familiar_entity,
            entry.before.as_ref(),
            &mut commands,
            &mut queries.q_familiars,
        );
        res.selected_entity.0 = Some(familiar_entity);
        res.area_edit_history.redo_stack.push(entry);
    }
}
