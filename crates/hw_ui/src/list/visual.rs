use super::drag_state::DragState;
use crate::components::{FamiliarListItem, SoulListItem};
use crate::selection::SelectedEntity;
use crate::theme::UiTheme;
use bevy::prelude::*;

type SoulListItemQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static SoulListItem,
        &'static mut Node,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    With<Button>,
>;

type FamiliarListItemQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static FamiliarListItem,
        &'static mut Node,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    (With<Button>, Without<SoulListItem>),
>;

type SoulChangedQuery<'w, 's> =
    Query<'w, 's, (), Or<(Changed<Interaction>, Added<SoulListItem>)>>;
type FamiliarChangedQuery<'w, 's> =
    Query<'w, 's, (), Or<(Changed<Interaction>, Added<FamiliarListItem>)>>;

pub fn entity_list_visual_feedback_system(
    selected_entity: Res<SelectedEntity>,
    drag_state: Res<DragState>,
    q_soul_changed: SoulChangedQuery,
    q_familiar_changed: FamiliarChangedQuery,
    mut q_souls: SoulListItemQuery<'_, '_>,
    mut q_familiars: FamiliarListItemQuery<'_, '_>,
    theme: Res<UiTheme>,
) {
    if !selected_entity.is_changed()
        && !drag_state.is_changed()
        && q_soul_changed.is_empty()
        && q_familiar_changed.is_empty()
    {
        return;
    }

    for (interaction, item, mut node, mut bg, mut border_color) in q_souls.iter_mut() {
        let is_selected = selected_entity.0 == Some(item.0);
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

    for (interaction, item, mut node, mut bg, mut border_color) in q_familiars.iter_mut() {
        let is_selected = selected_entity.0 == Some(item.0);
        let is_drop_target = drag_state.is_dragging() && drag_state.drop_target() == Some(item.0);
        apply_row_highlight(
            &mut node,
            &mut bg,
            &mut border_color,
            RowHighlightState {
                interaction: *interaction,
                is_selected,
                is_drop_target,
                is_familiar_row: true,
            },
            &theme,
        );
    }
}

/// リスト行のハイライト状態
pub struct RowHighlightState {
    pub interaction: Interaction,
    pub is_selected: bool,
    pub is_drop_target: bool,
    pub is_familiar_row: bool,
}

/// リスト行の選択・ホバー状態に応じたハイライト適用（タスクリスト等で再利用）
pub fn apply_row_highlight(
    node: &mut Node,
    bg: &mut BackgroundColor,
    border_color: &mut BorderColor,
    state: RowHighlightState,
    theme: &UiTheme,
) {
    let RowHighlightState { interaction, is_selected, is_drop_target, is_familiar_row } = state;
    if is_drop_target {
        bg.0 = theme.colors.list_item_selected_hover;
        node.border.left = Val::Px(theme.sizes.list_selection_border_width);
        *border_color = BorderColor::all(theme.colors.accent_soul_bright);
        return;
    }

    let is_hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
    bg.0 = if is_familiar_row {
        match (is_selected, is_hovered) {
            (true, true) => theme.colors.familiar_header_selected_hover,
            (true, false) => theme.colors.familiar_header_selected,
            (false, true) => theme.colors.familiar_header_hover,
            (false, false) => theme.colors.familiar_button_bg,
        }
    } else {
        match (is_selected, is_hovered) {
            (true, true) => theme.colors.list_item_selected_hover,
            (true, false) => theme.colors.list_item_selected,
            (false, true) => theme.colors.list_item_hover,
            (false, false) => theme.colors.list_item_default,
        }
    };

    if is_selected {
        node.border.left = Val::Px(theme.sizes.list_selection_border_width);
        *border_color = BorderColor::all(theme.colors.list_selection_border);
    } else {
        node.border.left = Val::Px(0.0);
        *border_color = BorderColor::all(Color::NONE);
    }
}
