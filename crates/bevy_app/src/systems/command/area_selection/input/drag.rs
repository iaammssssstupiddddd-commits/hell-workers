use super::super::AreaEditHistory;
use super::super::apply::{apply_area_and_record_history, assign_unassigned_tasks_in_area};
use super::super::geometry::clamp_area_to_site;
use super::transitions::should_exit_after_apply;
use crate::app_contexts::TaskContext;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::systems::command::TaskMode;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::game_state::PlayMode;
use hw_core::relationships::ManagedBy;
use hw_ui::area_edit::{AreaEditSession, apply_area_edit_drag};
use hw_ui::camera::{MainCamera, world_cursor_pos};
use hw_world::zones::Site;

pub(super) struct ActiveDragCtx<'a> {
    pub(super) buttons: &'a ButtonInput<MouseButton>,
    pub(super) keyboard: &'a ButtonInput<KeyCode>,
    pub(super) task_context: &'a mut TaskContext,
    pub(super) next_play_mode: &'a mut NextState<PlayMode>,
    pub(super) area_edit_session: &'a mut AreaEditSession,
    pub(super) area_edit_history: &'a mut AreaEditHistory,
}

pub(super) fn handle_active_drag_input(
    ctx: &mut ActiveDragCtx<'_>,
    q_window: &Query<&Window, With<PrimaryWindow>>,
    q_camera: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_familiars: &mut Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    q_sites: &Query<&Site>,
    q_unassigned: &Query<
        (Entity, &Transform, &crate::systems::jobs::Designation),
        Without<ManagedBy>,
    >,
    commands: &mut Commands,
) -> bool {
    let Some(active_drag) = ctx.area_edit_session.active_drag.clone() else {
        return false;
    };

    if ctx.buttons.pressed(MouseButton::Left)
        && let Some(world_pos) = world_cursor_pos(q_window, q_camera)
    {
        let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);
        let updated_area =
            clamp_area_to_site(&apply_area_edit_drag(&active_drag, snapped_pos), q_sites);

        commands
            .entity(active_drag.familiar_entity)
            .insert(updated_area.clone());
        if let Ok((mut active_command, mut familiar_dest)) =
            q_familiars.get_mut(active_drag.familiar_entity)
        {
            familiar_dest.0 = updated_area.center();
            active_command.command = FamiliarCommand::Patrol;
        }
    }

    if ctx.buttons.just_released(MouseButton::Left) {
        let applied_area = world_cursor_pos(q_window, q_camera)
            .map(WorldMap::snap_to_grid_edge)
            .map(|snapped| {
                clamp_area_to_site(&apply_area_edit_drag(&active_drag, snapped), q_sites)
            })
            .unwrap_or_else(|| active_drag.original_area.clone());

        if applied_area != active_drag.original_area {
            apply_area_and_record_history(
                active_drag.familiar_entity,
                &applied_area,
                Some(active_drag.original_area.clone()),
                commands,
                q_familiars,
                ctx.area_edit_history,
                q_sites,
            );

            assign_unassigned_tasks_in_area(
                commands,
                active_drag.familiar_entity,
                &applied_area,
                q_unassigned,
            );
        }

        ctx.area_edit_session.active_drag = None;
        if should_exit_after_apply(ctx.keyboard) {
            ctx.task_context.0 = TaskMode::None;
            ctx.next_play_mode.set(PlayMode::Normal);
        } else {
            ctx.task_context.0 = TaskMode::AreaSelection(None);
        }
        return true;
    }

    if ctx.buttons.pressed(MouseButton::Left) {
        return true;
    }

    ctx.area_edit_session.active_drag = None;
    false
}
