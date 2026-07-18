use crate::interface::selection::update_selection_indicator;
use crate::interface::ui::interaction::hover_tooltip_system;
use bevy::prelude::*;
use hw_ui::selection::PlacementFeedbackSet;

pub type UiTooltipPlugin = hw_ui::plugins::tooltip::UiTooltipPlugin;

pub fn ui_tooltip_plugin() -> UiTooltipPlugin {
    UiTooltipPlugin::new(register_ui_tooltip_plugin_systems)
}

fn register_ui_tooltip_plugin_systems(app: &mut App) {
    app.add_systems(
        Update,
        hover_tooltip_system
            .after(update_selection_indicator)
            .in_set(PlacementFeedbackSet::Present),
    );
}
