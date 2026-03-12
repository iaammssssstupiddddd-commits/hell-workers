//! ツールチップのターゲット判定

use hw_ui::components::MenuButton;
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TooltipTarget {
    UiButton(Entity),
    WorldEntity(Entity),
    PlacementFailure,
}

pub(crate) fn is_tooltip_suppressed_for_expanded_menu(
    menu_button: Option<&MenuButton>,
    menu_state: hw_ui::components::MenuState,
) -> bool {
    let Some(menu_button) = menu_button else {
        return false;
    };
    use hw_ui::components::{MenuAction, MenuState};
    matches!(
        (menu_state, menu_button.0),
        (MenuState::Architect, MenuAction::ToggleArchitect)
            | (MenuState::Zones, MenuAction::ToggleZones)
            | (MenuState::Orders, MenuAction::ToggleOrders)
    )
}
