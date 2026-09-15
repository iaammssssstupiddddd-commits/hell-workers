use crate::theme::UiTheme;
use bevy::prelude::*;

/// Keep the shared pointer and keyboard button surface usable at the base UI scale.
pub fn ensure_button_hit_targets(mut buttons: Query<&mut Node, With<Button>>) {
    for mut node in &mut buttons {
        if matches!(node.min_width, Val::Auto)
            || matches!(node.min_width, Val::Px(width) if width < 32.0)
        {
            node.min_width = Val::Px(32.0);
        }
        if matches!(node.min_height, Val::Auto)
            || matches!(node.min_height, Val::Px(height) if height < 32.0)
        {
            node.min_height = Val::Px(32.0);
        }
    }
}

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
