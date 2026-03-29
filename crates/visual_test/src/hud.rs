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

/// モード切替時にソウル/ビルドセクションの表示を切り替える。
pub fn update_section_visibility(
    state: Res<TestState>,
    mut q_soul: Query<&mut Node, (With<SoulSectionNode>, Without<BuildSectionNode>)>,
    mut q_build: Query<&mut Node, (With<BuildSectionNode>, Without<SoulSectionNode>)>,
) {
    if !state.is_changed() {
        return;
    }
    let soul_d = if state.mode == AppMode::Soul {
        Display::Flex
    } else {
        Display::None
    };
    let build_d = if state.mode == AppMode::Build {
        Display::Flex
    } else {
        Display::None
    };
    for mut n in &mut q_soul {
        n.display = soul_d;
    }
    for mut n in &mut q_build {
        n.display = build_d;
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
    elev: Res<TestElev>,
    mut q_texts: Query<(&mut Text, &DynamicTextKind)>,
) {
    if !state.is_changed() && !elev.is_changed() {
        return;
    }
    for (mut text, kind) in q_texts.iter_mut() {
        **text = match kind {
            DynamicTextKind::ViewDir => format!("{}  [V]", elev.dir.label()),
            DynamicTextKind::Height => format!("{:.0}", state.view_height),
            DynamicTextKind::Offset => format!("{:.0}", state.z_offset),
            DynamicTextKind::Ghost => format!("{:.2}", state.ghost_alpha),
            DynamicTextKind::Rim => format!("{:.2}", state.rim_strength),
            DynamicTextKind::Posterize => format!("{:.1}", state.posterize_steps),
            DynamicTextKind::CursorPos => {
                format!("({}, {})", state.building_cursor.0, state.building_cursor.1)
            }
        };
    }
}

fn is_selected(action: &VisualTestAction, state: &TestState) -> bool {
    match action {
        VisualTestAction::SetMode(m) => *m == state.mode,
        VisualTestAction::SetFace(e) => matches!(state.face_mode, FaceMode::Single(f) if f == *e),
        VisualTestAction::SetFaceAll => matches!(state.face_mode, FaceMode::AllDifferent),
        VisualTestAction::SetAnimation(i) => *i == state.anim_clip_idx,
        VisualTestAction::SetMotion(m) => *m == state.motion,
        VisualTestAction::SetBuildingKind(k) => *k == state.building_kind,
        _ => false,
    }
}
