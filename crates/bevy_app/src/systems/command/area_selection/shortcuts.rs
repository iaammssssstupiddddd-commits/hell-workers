use super::apply::{apply_area_and_record_history, apply_task_area_to_familiar};
use super::geometry::area_from_center_and_size;
use super::{AreaEditClipboard, AreaEditHistory, AreaEditPresets};
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar};
use crate::input_actions::{InputAction, ResolvedInputFrame};
use crate::interface::selection::SelectedEntity;
use crate::systems::command::TaskArea;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_world::zones::Site;

#[derive(SystemParam)]
pub struct ShortcutResources<'w> {
    resolved_frame: Res<'w, ResolvedInputFrame>,
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
    q_familiars:
        Query<'w, 's, (&'static mut ActiveCommand, &'static mut Destination), With<Familiar>>,
}

pub fn task_area_edit_history_shortcuts_system(
    mut res: ShortcutResources,
    mut queries: ShortcutQueries,
    mut commands: Commands,
) {
    let Some(action) = res
        .resolved_frame
        .actions()
        .iter()
        .copied()
        .find(|action| is_area_edit_action(*action))
    else {
        return;
    };

    if let Some(slot) = load_preset_slot(action) {
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

    if let Some(slot) = save_preset_slot(action) {
        if let Some(selected) = res.selected_entity.0
            && queries.q_familiar_exists.get(selected).is_ok()
            && let Ok(area) = queries.q_task_areas.get(selected)
        {
            res.area_edit_presets.save_size(slot, area.size());
        }
        return;
    }

    if action == InputAction::AreaCopy {
        if let Some(selected) = res.selected_entity.0
            && queries.q_familiar_exists.get(selected).is_ok()
        {
            res.area_edit_clipboard.area = queries.q_task_areas.get(selected).ok().cloned();
        }
        return;
    }

    if action == InputAction::AreaPaste {
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

    if action == InputAction::AreaRedo {
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

    if action == InputAction::AreaUndo
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

fn is_area_edit_action(action: InputAction) -> bool {
    matches!(
        action,
        InputAction::AreaCopy
            | InputAction::AreaPaste
            | InputAction::AreaUndo
            | InputAction::AreaRedo
            | InputAction::AreaSavePreset1
            | InputAction::AreaSavePreset2
            | InputAction::AreaSavePreset3
            | InputAction::AreaLoadPreset1
            | InputAction::AreaLoadPreset2
            | InputAction::AreaLoadPreset3
    )
}

fn load_preset_slot(action: InputAction) -> Option<usize> {
    match action {
        InputAction::AreaLoadPreset1 => Some(0),
        InputAction::AreaLoadPreset2 => Some(1),
        InputAction::AreaLoadPreset3 => Some(2),
        _ => None,
    }
}

fn save_preset_slot(action: InputAction) -> Option<usize> {
    match action {
        InputAction::AreaSavePreset1 => Some(0),
        InputAction::AreaSavePreset2 => Some(1),
        InputAction::AreaSavePreset3 => Some(2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_actions::{InputModifiers, ResolvedInputFrame};
    use crate::test_support::minimal_app;

    fn shortcut_app(action: InputAction) -> (App, Entity) {
        let mut app = minimal_app();
        app.init_resource::<SelectedEntity>()
            .init_resource::<ResolvedInputFrame>()
            .init_resource::<AreaEditHistory>()
            .init_resource::<AreaEditClipboard>()
            .init_resource::<AreaEditPresets>()
            .add_systems(Update, task_area_edit_history_shortcuts_system);
        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                ActiveCommand::default(),
                Destination(Vec2::ZERO),
                TaskArea::from_points(Vec2::ZERO, Vec2::new(4.0, 6.0)),
            ))
            .id();
        app.world_mut().resource_mut::<SelectedEntity>().0 = Some(familiar);
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(InputModifiers::default(), vec![action], None, true);
        (app, familiar)
    }

    #[test]
    fn resolved_area_copy_reaches_the_existing_consumer() {
        let (mut app, _) = shortcut_app(InputAction::AreaCopy);

        app.update();

        assert!(app.world().resource::<AreaEditClipboard>().area.is_some());
    }

    #[test]
    fn resolved_preset_action_owns_its_exact_slot() {
        let (mut app, _) = shortcut_app(InputAction::AreaSavePreset2);

        app.update();

        assert_eq!(
            app.world().resource::<AreaEditPresets>().slots,
            [None, Some(Vec2::new(4.0, 6.0)), None]
        );
    }
}
