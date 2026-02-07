use crate::interface::ui::components::*;
use crate::interface::ui::theme::UiTheme;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

/// エンティティリストのインタラクション
pub fn entity_list_interaction_system(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &SectionToggle, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut soul_list_interaction: Query<
        (&Interaction, &SoulListItem),
        (
            Changed<Interaction>,
            With<Button>,
            Without<FamiliarListItem>,
        ),
    >,
    mut familiar_list_interaction: Query<
        (&Interaction, &FamiliarListItem),
        (Changed<Interaction>, With<Button>, Without<SoulListItem>),
    >,
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    mut q_camera: Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: Query<&GlobalTransform>,
    q_folded: Query<Has<SectionFolded>>,
    unassigned_folded_query: Query<(Entity, Has<UnassignedFolded>), With<UnassignedSoulSection>>,
    theme: Res<UiTheme>,
) {
    for (interaction, toggle, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(theme.colors.interactive_active);
                match toggle.0 {
                    EntityListSectionType::Familiar(entity) => {
                        if q_folded.get(entity).unwrap_or(false) {
                            commands.entity(entity).remove::<SectionFolded>();
                        } else {
                            commands.entity(entity).insert(SectionFolded);
                        }
                    }
                    EntityListSectionType::Unassigned => {
                        let mut any_toggled = false;
                        for (unassigned_entity, has_folded) in unassigned_folded_query.iter() {
                            if has_folded {
                                commands
                                    .entity(unassigned_entity)
                                    .remove::<UnassignedFolded>();
                            } else {
                                commands.entity(unassigned_entity).insert(UnassignedFolded);
                            }
                            any_toggled = true;
                        }
                        if !any_toggled {
                            warn!("LIST: UnassignedSoulSection not found for toggling!");
                        }
                    }
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(theme.colors.interactive_hover);
            }
            Interaction::None => {
                *color = BackgroundColor(theme.colors.interactive_default);
            }
        }
    }

    for (interaction, item) in soul_list_interaction.iter_mut() {
        if *interaction == Interaction::Pressed {
            super::helpers::select_entity_and_focus_camera(
                item.0,
                "soul",
                &mut selected_entity,
                &mut q_camera,
                &q_transforms,
            );
        }
    }

    for (interaction, item) in familiar_list_interaction.iter_mut() {
        if *interaction == Interaction::Pressed {
            super::helpers::select_entity_and_focus_camera(
                item.0,
                "familiar",
                &mut selected_entity,
                &mut q_camera,
                &q_transforms,
            );
        }
    }
}

pub fn entity_list_visual_feedback_system(
    selected_entity: Res<crate::interface::selection::SelectedEntity>,
    q_soul_changed: Query<(), Or<(Changed<Interaction>, Added<SoulListItem>)>>,
    q_familiar_changed: Query<(), Or<(Changed<Interaction>, Added<FamiliarListItem>)>>,
    mut q_souls: Query<
        (
            &Interaction,
            &SoulListItem,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<Button>,
    >,
    mut q_familiars: Query<
        (
            &Interaction,
            &FamiliarListItem,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (With<Button>, Without<SoulListItem>),
    >,
    theme: Res<UiTheme>,
) {
    if !selected_entity.is_changed() && q_soul_changed.is_empty() && q_familiar_changed.is_empty() {
        return;
    }

    for (interaction, item, mut node, mut bg, mut border_color) in q_souls.iter_mut() {
        let is_selected = selected_entity.0 == Some(item.0);
        apply_row_highlight(
            &mut node,
            &mut bg,
            &mut border_color,
            *interaction,
            is_selected,
            &theme,
        );
    }

    for (interaction, item, mut node, mut bg, mut border_color) in q_familiars.iter_mut() {
        let is_selected = selected_entity.0 == Some(item.0);
        apply_row_highlight(
            &mut node,
            &mut bg,
            &mut border_color,
            *interaction,
            is_selected,
            &theme,
        );
    }
}

fn apply_row_highlight(
    node: &mut Node,
    bg: &mut BackgroundColor,
    border_color: &mut BorderColor,
    interaction: Interaction,
    is_selected: bool,
    theme: &UiTheme,
) {
    let is_hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
    bg.0 = match (is_selected, is_hovered) {
        (true, true) => theme.colors.list_item_selected_hover,
        (true, false) => theme.colors.list_item_selected,
        (false, true) => theme.colors.list_item_hover,
        (false, false) => theme.colors.list_item_default,
    };

    if is_selected {
        node.border.left = Val::Px(theme.sizes.list_selection_border_width);
        *border_color = BorderColor::all(theme.colors.list_selection_border);
    } else {
        node.border.left = Val::Px(0.0);
        *border_color = BorderColor::all(Color::NONE);
    }
}

pub fn entity_list_scroll_system(
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    mut q_scroll_areas: Query<(&RelativeCursorPosition, &mut ScrollPosition, &UiScrollArea)>,
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

    // Wheel up should move list content down (toward earlier rows).
    scroll_position.y = (scroll_position.y - delta_y).max(0.0);
}

pub fn entity_list_tab_focus_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    q_soul_items: Query<&SoulListItem>,
    q_familiar_items: Query<&FamiliarListItem>,
    mut q_camera: Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: Query<&GlobalTransform>,
) {
    if !keyboard.just_pressed(KeyCode::Tab) {
        return;
    }

    let mut candidates: Vec<Entity> = q_familiar_items
        .iter()
        .map(|item| item.0)
        .chain(q_soul_items.iter().map(|item| item.0))
        .collect();
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
    let target = candidates[next_index];

    super::helpers::select_entity_and_focus_camera(
        target,
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

/// 未所属ソウルセクションの矢印アイコンを折りたたみ状態に応じて更新
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
