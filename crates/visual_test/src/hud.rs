use bevy::prelude::*;

use crate::types::*;

pub fn apply_menu_visibility(
    state: Res<TestState>,
    mut q_panel: Query<&mut Visibility, With<MenuPanel>>,
    mut q_hint: Query<&mut Visibility, (With<MenuHint>, Without<MenuPanel>)>,
) {
    if !state.is_changed() {
        return;
    }
    if let Ok(mut vis) = q_panel.single_mut() {
        *vis = if state.menu_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut vis) = q_hint.single_mut() {
        *vis = if state.menu_visible {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

/// VisualTestAction ボタンの背景色を TestState に合わせて毎フレーム更新。
pub fn update_button_states(
    state: Res<TestState>,
    mut q_btns: Query<(&VisualTestAction, &mut BackgroundColor, &Interaction)>,
) {
    for (action, mut bg, interaction) in q_btns.iter_mut() {
        let selected = is_selected(action, &state);
        *bg = match (selected, interaction) {
            (true, Interaction::Hovered) => BackgroundColor(BTN_ACT_H),
            (true, _) => BackgroundColor(BTN_ACT),
            (false, Interaction::Pressed) => BackgroundColor(BTN_PRESS),
            (false, Interaction::Hovered) => BackgroundColor(BTN_HOVER),
            (false, Interaction::None) => BackgroundColor(BTN_DEF),
        };
    }
}

/// DynamicTextKind を持つテキストエンティティを TestState / TestElev で更新。
pub fn update_dynamic_texts(
    state: Res<TestState>,
    mut q_texts: Query<(&mut Text, &DynamicTextKind)>,
) {
    if !state.is_changed() {
        return;
    }
    for (mut text, kind) in q_texts.iter_mut() {
        **text = match kind {
            DynamicTextKind::CursorPos => {
                format!("({}, {})", state.building_cursor.0, state.building_cursor.1)
            }
        };
    }
}

fn is_selected(action: &VisualTestAction, state: &TestState) -> bool {
    match action {
        VisualTestAction::SetBuildingKind(k) => *k == state.building_kind,
        _ => false,
    }
}
