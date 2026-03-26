use hw_ui::UiIntent;
use hw_ui::components::MenuState;

use super::super::intent_context::IntentModeCtx;
use super::super::mode;

pub(crate) fn handle_toggle(intent: UiIntent, ctx: &mut IntentModeCtx<'_>) {
    let (menu, reset_play_mode) = match intent {
        UiIntent::ToggleArchitect => (MenuState::Architect, false),
        UiIntent::ToggleOrders => (MenuState::Orders, true),
        UiIntent::ToggleZones => (MenuState::Zones, true),
        UiIntent::ToggleDream => (MenuState::Dream, false),
        _ => return,
    };
    mode::toggle_menu_and_reset_mode(
        &mut ctx.menu_state,
        menu,
        &mut ctx.next_play_mode,
        &mut ctx.build_context,
        &mut ctx.zone_context,
        &mut ctx.task_context,
        reset_play_mode,
    );
}
