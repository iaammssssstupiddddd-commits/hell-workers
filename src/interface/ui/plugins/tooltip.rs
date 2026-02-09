use crate::interface::selection::blueprint_placement;
use crate::interface::selection::update_selection_indicator;
use crate::interface::ui::interaction::hover_tooltip_system;
use crate::systems::GameSystemSet;
use bevy::prelude::*;

pub struct UiTooltipPlugin;

impl Plugin for UiTooltipPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            hover_tooltip_system
                .after(update_selection_indicator)
                .before(blueprint_placement)
                .in_set(GameSystemSet::Interface),
        );
    }
}
