use crate::theme::UiTheme;
use bevy::prelude::*;

pub fn update_interaction_color(
    interaction: Interaction,
    color: &mut BackgroundColor,
    theme: &UiTheme,
) {
    *color = match interaction {
        Interaction::Pressed => BackgroundColor(theme.colors.button_pressed),
        Interaction::Hovered => BackgroundColor(theme.colors.button_hover),
        Interaction::None => BackgroundColor(theme.colors.button_default),
    };
}

pub fn despawn_context_menus(
    commands: &mut Commands,
    q_context_menu: &Query<Entity, With<crate::components::ContextMenu>>,
) {
    for entity in q_context_menu.iter() {
        commands.entity(entity).despawn();
    }
}
