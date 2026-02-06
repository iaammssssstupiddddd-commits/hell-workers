use crate::interface::ui::components::*;
use crate::interface::ui::theme::*;
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
) {
    for (interaction, toggle, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(COLOR_SECTION_TOGGLE_PRESSED);
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
                *color = BackgroundColor(COLOR_BUTTON_HOVER);
            }
            Interaction::None => {
                *color = BackgroundColor(COLOR_BUTTON_DEFAULT);
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
) {
    for (interaction, item, mut node, mut bg, mut border_color) in q_souls.iter_mut() {
        let is_selected = selected_entity.0 == Some(item.0);
        apply_row_highlight(
            &mut node,
            &mut bg,
            &mut border_color,
            *interaction,
            is_selected,
            COLOR_LIST_ITEM_DEFAULT,
            COLOR_LIST_ITEM_HOVER,
            COLOR_LIST_ITEM_SELECTED,
            COLOR_LIST_ITEM_SELECTED_HOVER,
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
            COLOR_FAMILIAR_BUTTON_BG,
            COLOR_FAMILIAR_HEADER_HOVER,
            COLOR_FAMILIAR_HEADER_SELECTED,
            COLOR_FAMILIAR_HEADER_SELECTED_HOVER,
        );
    }
}

fn apply_row_highlight(
    node: &mut Node,
    bg: &mut BackgroundColor,
    border_color: &mut BorderColor,
    interaction: Interaction,
    is_selected: bool,
    default_color: Color,
    hover_color: Color,
    selected_color: Color,
    selected_hover_color: Color,
) {
    let is_hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
    bg.0 = match (is_selected, is_hovered) {
        (true, true) => selected_hover_color,
        (true, false) => selected_color,
        (false, true) => hover_color,
        (false, false) => default_color,
    };

    if is_selected {
        node.border.left = Val::Px(LIST_SELECTION_BORDER_WIDTH);
        *border_color = BorderColor::all(COLOR_LIST_SELECTION_BORDER);
    } else {
        node.border.left = Val::Px(0.0);
        *border_color = BorderColor::all(Color::NONE);
    }
}

pub fn entity_list_scroll_system(
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    mut q_scroll_area: Query<
        (&RelativeCursorPosition, &mut ScrollPosition),
        With<UnassignedSoulContent>,
    >,
) {
    let Ok((cursor, mut scroll_position)) = q_scroll_area.single_mut() else {
        return;
    };
    if !cursor.cursor_over() {
        return;
    }

    let mut delta_y = 0.0;
    for event in mouse_wheel_events.read() {
        let unit_scale = match event.unit {
            MouseScrollUnit::Line => 28.0,
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
