use hw_ui::UiIntent;

use super::super::intent_context::{
    IntentFamiliarQueries, IntentModeCtx, IntentSelectionCtx, ensure_familiar_selected,
};
use super::super::mode;
use crate::systems::command::TaskMode;

pub(crate) fn handle_mode_select(
    intent: UiIntent,
    mode_ctx: &mut IntentModeCtx<'_, '_>,
    sel_ctx: &mut IntentSelectionCtx<'_>,
    familiar_queries: &IntentFamiliarQueries<'_, '_>,
) {
    mode_ctx.cancel_active_mode_if_needed();
    match intent {
        UiIntent::SelectBuild(kind) => {
            mode::set_build_mode(
                kind,
                &mut mode_ctx.cleanup.next_play_mode,
                &mut mode_ctx.cleanup.build_context,
                &mut mode_ctx.cleanup.zone_context,
                &mut mode_ctx.cleanup.task_context,
            );
        }
        UiIntent::SelectFloorPlace => {
            mode::set_floor_place_mode(
                &mut mode_ctx.cleanup.next_play_mode,
                &mut mode_ctx.cleanup.build_context,
                &mut mode_ctx.cleanup.zone_context,
                &mut mode_ctx.cleanup.task_context,
            );
        }
        UiIntent::SelectZone(kind) => {
            mode::set_zone_mode(
                kind,
                &mut mode_ctx.cleanup.next_play_mode,
                &mut mode_ctx.cleanup.build_context,
                &mut mode_ctx.cleanup.zone_context,
                &mut mode_ctx.cleanup.task_context,
            );
        }
        UiIntent::RemoveZone(kind) => {
            mode::set_zone_removal_mode(
                kind,
                &mut mode_ctx.cleanup.next_play_mode,
                &mut mode_ctx.cleanup.build_context,
                &mut mode_ctx.cleanup.zone_context,
                &mut mode_ctx.cleanup.task_context,
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
                &mut mode_ctx.cleanup.next_play_mode,
                &mut mode_ctx.cleanup.build_context,
                &mut mode_ctx.cleanup.zone_context,
                &mut mode_ctx.cleanup.task_context,
            );
        }
        UiIntent::SelectAreaTask => {
            ensure_familiar_selected(
                &mut sel_ctx.selected_entity,
                &familiar_queries.q_familiars_for_area,
                "Area Edit",
            );
            mode::set_area_task_mode(
                &mut mode_ctx.cleanup.next_play_mode,
                &mut mode_ctx.cleanup.build_context,
                &mut mode_ctx.cleanup.zone_context,
                &mut mode_ctx.cleanup.task_context,
            );
        }
        UiIntent::SelectDreamPlanting => {
            mode::set_task_mode(
                TaskMode::DreamPlanting(None),
                &mut mode_ctx.cleanup.next_play_mode,
                &mut mode_ctx.cleanup.build_context,
                &mut mode_ctx.cleanup.zone_context,
                &mut mode_ctx.cleanup.task_context,
            );
        }
        UiIntent::BeginStockpilePolicyRangeEdit { patch }
            if mode_ctx.cleanup.begin_stockpile_policy_range_edit(patch) =>
        {
            mode::set_task_mode(
                TaskMode::StockpilePolicyEdit(None),
                &mut mode_ctx.cleanup.next_play_mode,
                &mut mode_ctx.cleanup.build_context,
                &mut mode_ctx.cleanup.zone_context,
                &mut mode_ctx.cleanup.task_context,
            );
        }
        _ => {}
    }
}
