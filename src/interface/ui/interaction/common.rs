use crate::interface::ui::theme::{COLOR_BUTTON_DEFAULT, COLOR_BUTTON_HOVER, COLOR_BUTTON_PRESSED};
use bevy::prelude::*;

pub(super) fn update_interaction_color(interaction: Interaction, color: &mut BackgroundColor) {
    *color = match interaction {
        Interaction::Pressed => BackgroundColor(COLOR_BUTTON_PRESSED),
        Interaction::Hovered => BackgroundColor(COLOR_BUTTON_HOVER),
        Interaction::None => BackgroundColor(COLOR_BUTTON_DEFAULT),
    };
}

pub(super) fn despawn_context_menus(
    commands: &mut Commands,
    q_context_menu: &Query<Entity, With<crate::interface::ui::components::ContextMenu>>,
) {
    for entity in q_context_menu.iter() {
        commands.entity(entity).despawn();
    }
}
