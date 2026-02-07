use super::resize::{ENTITY_LIST_DEFAULT_HEIGHT, ENTITY_LIST_MIN_HEIGHT};
use crate::interface::ui::components::{
    EntityListBody, EntityListMinimizeButton, EntityListMinimizeButtonLabel, EntityListPanel,
};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

#[derive(Resource)]
pub struct EntityListMinimizeState {
    pub minimized: bool,
    pub expanded_height: f32,
}

impl Default for EntityListMinimizeState {
    fn default() -> Self {
        Self {
            minimized: false,
            expanded_height: ENTITY_LIST_DEFAULT_HEIGHT,
        }
    }
}

fn minimized_panel_height(theme: &UiTheme) -> f32 {
    theme.spacing.panel_padding * 2.0 + 28.0
}

pub fn entity_list_minimize_toggle_system(
    mut q_button: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<Button>,
            With<EntityListMinimizeButton>,
        ),
    >,
    mut q_panel_node: Query<&mut Node, (With<EntityListPanel>, Without<EntityListBody>)>,
    mut q_body_node: Query<&mut Node, (With<EntityListBody>, Without<EntityListPanel>)>,
    mut q_label_text: Query<&mut Text, With<EntityListMinimizeButtonLabel>>,
    mut state: ResMut<EntityListMinimizeState>,
    theme: Res<UiTheme>,
) {
    let Ok(mut panel_node) = q_panel_node.single_mut() else {
        return;
    };
    let Ok(mut body_node) = q_body_node.single_mut() else {
        return;
    };
    let Ok(mut label_text) = q_label_text.single_mut() else {
        return;
    };

    for (interaction, mut bg_color) in q_button.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *bg_color = BackgroundColor(theme.colors.interactive_active);
                state.minimized = !state.minimized;

                if state.minimized {
                    if let Val::Px(current_height) = panel_node.height {
                        if current_height >= ENTITY_LIST_MIN_HEIGHT {
                            state.expanded_height = current_height;
                        }
                    }

                    let collapsed_height = minimized_panel_height(&theme);
                    panel_node.height = Val::Px(collapsed_height);
                    panel_node.min_height = Val::Px(collapsed_height);
                    body_node.display = Display::None;
                    label_text.0 = "+".to_string();
                } else {
                    panel_node.height = Val::Px(state.expanded_height.max(ENTITY_LIST_MIN_HEIGHT));
                    panel_node.min_height = Val::Px(ENTITY_LIST_MIN_HEIGHT);
                    body_node.display = Display::Flex;
                    label_text.0 = "-".to_string();
                }
            }
            Interaction::Hovered => {
                *bg_color = BackgroundColor(theme.colors.interactive_hover);
            }
            Interaction::None => {
                *bg_color = BackgroundColor(theme.colors.interactive_default);
            }
        }
    }
}
