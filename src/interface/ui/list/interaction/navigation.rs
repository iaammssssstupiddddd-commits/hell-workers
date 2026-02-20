use crate::game_state::TaskContext;
use crate::interface::ui::components::{
    EntityListScrollHint, FamiliarListItem, SoulListItem, UiScrollArea, UnassignedFolded,
    UnassignedSectionArrowIcon, UnassignedSoulContent, UnassignedSoulSection,
};
use crate::interface::ui::theme::UiTheme;
use crate::systems::command::TaskMode;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

pub fn entity_list_scroll_system(
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    mut q_scroll_areas: Query<(&RelativeCursorPosition, &mut ScrollPosition, &UiScrollArea)>,
    theme: Res<UiTheme>,
) {
    let Some((_, mut scroll_position, scroll_area)) = q_scroll_areas
        .iter_mut()
        .find(|(cursor, _, _)| cursor.cursor_over())
    else {
        return;
    };

    let mut delta_y = 0.0;
    for event in mouse_wheel_events.read() {
        let unit_scale = match event.unit {
            MouseScrollUnit::Line => scroll_area.speed,
            MouseScrollUnit::Pixel => 1.0,
        };
        delta_y += event.y * unit_scale;
    }
    if delta_y.abs() <= f32::EPSILON {
        return;
    }

    let step = theme.sizes.soul_item_height.max(1.0);
    let target = (scroll_position.y - delta_y).max(0.0);
    scroll_position.y = (target / step).round() * step;
}

pub fn entity_list_tab_focus_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    task_context: Res<TaskContext>,
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    q_soul_items: Query<&SoulListItem>,
    q_familiar_items: Query<&FamiliarListItem>,
    mut q_camera: Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: Query<&GlobalTransform>,
) {
    if !keyboard.just_pressed(KeyCode::Tab) {
        return;
    }

    let in_area_task_mode = matches!(task_context.0, TaskMode::AreaSelection(_));
    let mut candidates: Vec<Entity> = if in_area_task_mode {
        q_familiar_items.iter().map(|item| item.0).collect()
    } else {
        q_familiar_items
            .iter()
            .map(|item| item.0)
            .chain(q_soul_items.iter().map(|item| item.0))
            .collect()
    };
    candidates.sort_by_key(|entity| entity.index());
    candidates.dedup();
    if candidates.is_empty() {
        return;
    }

    let reverse = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let current_index = selected_entity
        .0
        .and_then(|selected| candidates.iter().position(|&entity| entity == selected));
    let next_index = if reverse {
        current_index
            .map(|idx| idx.saturating_sub(1))
            .unwrap_or(candidates.len().saturating_sub(1))
    } else {
        current_index
            .map(|idx| (idx + 1) % candidates.len())
            .unwrap_or(0)
    };

    super::super::selection_focus::select_entity_and_focus_camera(
        candidates[next_index],
        "tab-focus",
        &mut selected_entity,
        &mut q_camera,
        &q_transforms,
    );
}

pub fn entity_list_scroll_hint_visibility_system(
    q_unassigned_section: Query<Has<UnassignedFolded>, With<UnassignedSoulSection>>,
    q_unassigned_content: Query<&ComputedNode, With<UnassignedSoulContent>>,
    mut q_hint_nodes: Query<&mut Node, With<EntityListScrollHint>>,
) {
    let unassigned_folded = q_unassigned_section.iter().next().unwrap_or(false);
    let has_overflow = if unassigned_folded {
        false
    } else {
        q_unassigned_content
            .iter()
            .next()
            .is_some_and(|computed| computed.content_size().y > computed.size().y + 1.0)
    };

    let desired = if has_overflow {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in q_hint_nodes.iter_mut() {
        if node.display != desired {
            node.display = desired;
        }
    }
}

pub fn update_unassigned_arrow_icon_system(
    game_assets: Res<crate::assets::GameAssets>,
    unassigned_folded_query: Query<
        Has<UnassignedFolded>,
        (With<UnassignedSoulSection>, Changed<UnassignedFolded>),
    >,
    mut q_arrow: Query<&mut ImageNode, With<UnassignedSectionArrowIcon>>,
) {
    if let Some(is_folded) = unassigned_folded_query.iter().next() {
        for mut icon in q_arrow.iter_mut() {
            icon.image = if is_folded {
                game_assets.icon_arrow_right.clone()
            } else {
                game_assets.icon_arrow_down.clone()
            };
        }
    }
}
