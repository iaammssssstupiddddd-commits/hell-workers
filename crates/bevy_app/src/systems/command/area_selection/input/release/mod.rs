mod area;
mod cancel;
mod designation;
mod dream;

use area::{AreaReleaseCtx, collect_indicator_entities, handle_release_area_selection};
use cancel::handle_release_cancel_designation;
use designation::{DesignationReleaseCtx, handle_release_designation};
use dream::handle_release_dream_planting;

use super::super::queries::{
    DesignationTargetQuery, FloorTileBlueprintQuery, UnassignedDesignationQuery,
    WallTileBlueprintQuery,
};
use super::super::{AreaEditHistory, AreaEditSession};
use crate::app_contexts::TaskContext;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar};
use crate::systems::command::{AreaSelectionIndicator, TaskArea, TaskMode};
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
use hw_world::zones::Site;

pub(super) struct ReleaseCtx<'a> {
    pub(super) task_context: &'a mut TaskContext,
    pub(super) selected_entity: Option<Entity>,
    pub(super) world_pos: Vec2,
    pub(super) keyboard: &'a ButtonInput<KeyCode>,
    pub(super) next_play_mode: &'a mut NextState<PlayMode>,
    pub(super) area_edit_session: &'a mut AreaEditSession,
    pub(super) area_edit_history: &'a mut AreaEditHistory,
}

pub(super) fn handle_left_just_released_input(
    ctx: &mut ReleaseCtx<'_>,
    q_familiar_areas: &Query<&TaskArea, With<Familiar>>,
    q_sites: &Query<&Site>,
    q_familiars: &mut Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    q_target_sets: &mut bevy::ecs::system::ParamSet<(
        DesignationTargetQuery<'_, '_>,
        FloorTileBlueprintQuery<'_, '_>,
        WallTileBlueprintQuery<'_, '_>,
    )>,
    q_aux: &mut bevy::ecs::system::ParamSet<(
        UnassignedDesignationQuery<'_, '_>,
        Query<Entity, With<AreaSelectionIndicator>>,
    )>,
    commands: &mut Commands,
) {
    match ctx.task_context.0 {
        TaskMode::AreaSelection(Some(start_pos)) => {
            let indicator_entities: Vec<Entity> = { collect_indicator_entities(&q_aux.p1()) };
            let q_unassigned = q_aux.p0();
            let mut area_ctx = AreaReleaseCtx {
                task_context: ctx.task_context,
                selected_entity: ctx.selected_entity,
                world_pos: ctx.world_pos,
                start_pos,
                keyboard: ctx.keyboard,
                next_play_mode: ctx.next_play_mode,
                area_edit_history: ctx.area_edit_history,
            };
            handle_release_area_selection(
                &mut area_ctx,
                q_familiar_areas,
                q_sites,
                q_familiars,
                &indicator_entities,
                &q_unassigned,
                commands,
            );
        }
        TaskMode::DesignateChop(Some(start_pos))
        | TaskMode::DesignateMine(Some(start_pos))
        | TaskMode::DesignateHaul(Some(start_pos)) => {
            let mode = ctx.task_context.0;
            let q_targets = q_target_sets.p0();
            handle_release_designation(
                ctx.task_context,
                DesignationReleaseCtx {
                    selected_entity: ctx.selected_entity,
                    world_pos: ctx.world_pos,
                    start_pos,
                    mode,
                },
                q_familiars,
                &q_targets,
                commands,
            );
        }
        TaskMode::CancelDesignation(Some(start_pos)) => {
            handle_release_cancel_designation(
                ctx.task_context,
                ctx.selected_entity,
                ctx.world_pos,
                start_pos,
                q_target_sets,
                commands,
            );
        }
        TaskMode::DreamPlanting(Some(start_pos)) => {
            handle_release_dream_planting(ctx.task_context, ctx.world_pos, start_pos, ctx.area_edit_session);
        }
        _ => {}
    }
}
