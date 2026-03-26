use hw_ui::UiIntent;

use super::super::intent_context::{
    IntentFamiliarQueries, IntentModeCtx, IntentSelectionCtx, ensure_familiar_selected,
};
use super::super::mode;
use crate::systems::command::TaskMode;

pub(crate) fn handle_mode_select(
    intent: UiIntent,
    mode_ctx: &mut IntentModeCtx<'_>,
    sel_ctx: &mut IntentSelectionCtx<'_>,
    familiar_queries: &IntentFamiliarQueries<'_, '_>,
) {
    match intent {
        UiIntent::SelectBuild(kind) => {
            mode::set_build_mode(
                kind,
                &mut mode_ctx.next_play_mode,
                &mut mode_ctx.build_context,
                &mut mode_ctx.zone_context,
                &mut mode_ctx.task_context,
            );
        }
        UiIntent::SelectFloorPlace => {
            mode::set_floor_place_mode(
                &mut mode_ctx.next_play_mode,
                &mut mode_ctx.build_context,
                &mut mode_ctx.zone_context,
                &mut mode_ctx.task_context,
            );
        }
        UiIntent::SelectZone(kind) => {
            mode::set_zone_mode(
                kind,
                &mut mode_ctx.next_play_mode,
                &mut mode_ctx.build_context,
                &mut mode_ctx.zone_context,
                &mut mode_ctx.task_context,
            );
        }
        UiIntent::RemoveZone(kind) => {
            mode::set_zone_removal_mode(
                kind,
                &mut mode_ctx.next_play_mode,
                &mut mode_ctx.build_context,
                &mut mode_ctx.zone_context,
                &mut mode_ctx.task_context,
            );
        }
        UiIntent::SelectTaskMode(task_mode) => {
            ensure_familiar_selected(
                &mut sel_ctx.selected_entity,
                &familiar_queries.q_familiars_for_area,
                "Task designation",
            );
            mode::set_task_mode(
                task_mode,
                &mut mode_ctx.next_play_mode,
                &mut mode_ctx.build_context,
                &mut mode_ctx.zone_context,
                &mut mode_ctx.task_context,
            );
        }
        UiIntent::SelectAreaTask => {
            ensure_familiar_selected(
                &mut sel_ctx.selected_entity,
                &familiar_queries.q_familiars_for_area,
                "Area Edit",
            );
            mode::set_area_task_mode(
                &mut mode_ctx.next_play_mode,
                &mut mode_ctx.build_context,
                &mut mode_ctx.zone_context,
                &mut mode_ctx.task_context,
            );
        }
        UiIntent::SelectDreamPlanting => {
            mode::set_task_mode(
                TaskMode::DreamPlanting(None),
                &mut mode_ctx.next_play_mode,
                &mut mode_ctx.build_context,
                &mut mode_ctx.zone_context,
                &mut mode_ctx.task_context,
            );
        }
        _ => {}
    }
}
