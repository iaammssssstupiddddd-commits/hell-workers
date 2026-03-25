// クリック、タブ、可視状態、ハイライト

use crate::camera::MainCamera;
use crate::components::{
    EntityListBody, LeftPanelMode, LeftPanelTabButton, TaskListBody, TaskListItem,
};
use crate::list::{RowHighlightState, apply_row_highlight, focus_camera_on_entity};
use crate::panels::info_panel::InfoPanelPinState;
use crate::theme::UiTheme;
use bevy::prelude::*;

type TaskListItemQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static TaskListItem,
        &'static mut Node,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    With<Button>,
>;

type TaskChangedQuery<'w, 's> =
    Query<'w, 's, (), Or<(Changed<Interaction>, Added<TaskListItem>)>>;

pub fn task_list_visual_feedback_system(
    pin_state: Res<InfoPanelPinState>,
    q_changed: TaskChangedQuery,
    mut q_items: TaskListItemQuery<'_, '_>,
    theme: Res<UiTheme>,
) {
    if !pin_state.is_changed() && q_changed.is_empty() {
        return;
    }

    for (interaction, item, mut node, mut bg, mut border_color) in q_items.iter_mut() {
        let is_selected = pin_state.entity == Some(item.0);
        apply_row_highlight(
            &mut node,
            &mut bg,
            &mut border_color,
            RowHighlightState {
                interaction: *interaction,
                is_selected,
                is_drop_target: false,
                is_familiar_row: false,
            },
            &theme,
        );
    }
}

pub fn left_panel_tab_system(
    mut mode: ResMut<LeftPanelMode>,
    theme: Res<UiTheme>,
    interactions: Query<(&Interaction, &LeftPanelTabButton), Changed<Interaction>>,
    tab_buttons: Query<(Entity, &LeftPanelTabButton, &Children)>,
    mut text_colors: Query<&mut TextColor>,
    mut border_colors: Query<&mut BorderColor>,
) {
    for (interaction, tab) in &interactions {
        if *interaction == Interaction::Pressed && *mode != tab.0 {
            *mode = tab.0;
        }
    }

    if mode.is_changed() {
        for (button_entity, tab, children) in &tab_buttons {
            let is_active = tab.0 == *mode;

            if let Some(child) = children.iter().next()
                && let Ok(mut color) = text_colors.get_mut(child) {
                    color.0 = if is_active {
                        theme.colors.text_accent_semantic
                    } else {
                        theme.colors.text_secondary_semantic
                    };
                }

            if let Ok(mut border) = border_colors.get_mut(button_entity) {
                *border = BorderColor::all(if is_active {
                    theme.colors.text_accent_semantic
                } else {
                    Color::NONE
                });
            }
        }
    }
}

pub fn left_panel_visibility_system(
    mode: Res<LeftPanelMode>,
    mut entity_list_bodies: Query<&mut Node, (With<EntityListBody>, Without<TaskListBody>)>,
    mut task_list_bodies: Query<&mut Node, (With<TaskListBody>, Without<EntityListBody>)>,
) {
    if !mode.is_changed() {
        return;
    }

    match *mode {
        LeftPanelMode::EntityList => {
            for mut node in &mut entity_list_bodies {
                if node.display != Display::Flex {
                    node.display = Display::Flex;
                }
            }
            for mut node in &mut task_list_bodies {
                if node.display != Display::None {
                    node.display = Display::None;
                }
            }
        }
        LeftPanelMode::TaskList => {
            for mut node in &mut entity_list_bodies {
                if node.display != Display::None {
                    node.display = Display::None;
                }
            }
            for mut node in &mut task_list_bodies {
                if node.display != Display::Flex {
                    node.display = Display::Flex;
                }
            }
        }
    }
}

pub fn task_list_click_system(
    mut pin_state: ResMut<InfoPanelPinState>,
    interactions: Query<(&Interaction, &TaskListItem), Changed<Interaction>>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
    target_transforms: Query<&GlobalTransform, Without<MainCamera>>,
) {
    for (interaction, item) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let target_entity = item.0;
        focus_camera_on_entity(target_entity, &mut camera_query, &target_transforms);
        pin_state.entity = Some(target_entity);
    }
}
