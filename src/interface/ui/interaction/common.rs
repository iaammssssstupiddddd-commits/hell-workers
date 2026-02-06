use bevy::prelude::*;

pub(super) fn update_interaction_color(interaction: Interaction, color: &mut BackgroundColor) {
    *color = match interaction {
        Interaction::Pressed => BackgroundColor(Color::srgb(0.5, 0.5, 0.5)),
        Interaction::Hovered => BackgroundColor(Color::srgb(0.4, 0.4, 0.4)),
        Interaction::None => BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
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
