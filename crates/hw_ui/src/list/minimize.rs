use super::resize::{ENTITY_LIST_DEFAULT_HEIGHT, ENTITY_LIST_MIN_HEIGHT};
use crate::components::{
    EntityListMinimizeButton, EntityListMinimizeButtonLabel, EntityListPanel, UiInputState,
};
use crate::theme::UiTheme;
use bevy::prelude::*;

type MinimizeButtonQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Ref<'static, Interaction>,
        &'static mut BackgroundColor,
    ),
    (With<Button>, With<EntityListMinimizeButton>),
>;

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
    theme.spacing.panel_padding * 2.0 + theme.sizes.fold_button_size.max(32.0) + 8.0
}

/// External HUD navigation can expand a previously minimized management page.
pub fn sync_entity_list_minimize_system(
    state: Res<EntityListMinimizeState>,
    theme: Res<UiTheme>,
    mut panels: Query<&mut Node, With<EntityListPanel>>,
    mut labels: Query<&mut Text, With<EntityListMinimizeButtonLabel>>,
) {
    if !state.is_changed() && !theme.is_changed() {
        return;
    }
    for mut node in &mut panels {
        let min = if state.minimized {
            minimized_panel_height(&theme)
        } else {
            ENTITY_LIST_MIN_HEIGHT
        };
        node.height = Val::Px(if state.minimized {
            min
        } else {
            state.expanded_height.max(min)
        });
        node.min_height = Val::Px(min);
    }
    for mut label in &mut labels {
        label.0 = if state.minimized { "+" } else { "-" }.to_owned();
    }
}

pub fn entity_list_minimize_toggle_system(
    mut q_button: MinimizeButtonQuery<'_, '_>,
    mut q_panel_node: Query<&mut Node, With<EntityListPanel>>,
    mut q_label_text: Query<&mut Text, With<EntityListMinimizeButtonLabel>>,
    mut state: ResMut<EntityListMinimizeState>,
    theme: Res<UiTheme>,
    ui_input_state: Res<UiInputState>,
) {
    if ui_input_state.world_input_captured {
        return;
    }
    let Ok(mut panel_node) = q_panel_node.single_mut() else {
        return;
    };
    let Ok(mut label_text) = q_label_text.single_mut() else {
        return;
    };

    for (entity, interaction, mut bg_color) in q_button.iter_mut() {
        if interaction.is_changed() || theme.is_changed() {
            crate::interaction::common::update_interaction_color(
                *interaction,
                &mut bg_color,
                &theme,
            );
        }
        if ui_input_state.button_activated(entity) {
            state.minimized = !state.minimized;

            if state.minimized {
                if let Val::Px(current_height) = panel_node.height
                    && current_height >= ENTITY_LIST_MIN_HEIGHT
                {
                    state.expanded_height = current_height;
                }

                let collapsed_height = minimized_panel_height(&theme);
                panel_node.height = Val::Px(collapsed_height);
                panel_node.min_height = Val::Px(collapsed_height);
                label_text.0 = "+".to_string();
            } else {
                panel_node.height = Val::Px(state.expanded_height.max(ENTITY_LIST_MIN_HEIGHT));
                panel_node.min_height = Val::Px(ENTITY_LIST_MIN_HEIGHT);
                label_text.0 = "-".to_string();
            }
        }
    }
}
