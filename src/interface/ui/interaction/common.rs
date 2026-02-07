use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

pub(super) fn update_interaction_color(
    interaction: Interaction,
    color: &mut BackgroundColor,
    theme: &UiTheme,
) {
    *color = match interaction {
        Interaction::Pressed => BackgroundColor(theme.colors.interactive_active),
        Interaction::Hovered => BackgroundColor(theme.colors.interactive_hover),
        Interaction::None => BackgroundColor(theme.colors.interactive_default),
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
