use super::*;

// ─── ボタンメニュー ───────────────────────────────────────────────────────────

pub(super) const BTN_H: f32 = 24.0;
pub(super) const BTN_GAP: f32 = 2.0;
pub(super) const SFONT: f32 = 11.0;
pub(super) const SEC_COL: Color = Color::Srgba(bevy::color::Srgba::new(0.55, 0.45, 0.70, 1.0));
pub(super) const VAL_COL: Color = Color::Srgba(bevy::color::Srgba::new(1.00, 0.80, 0.40, 1.0));
pub(super) const PANEL_BG: Color = Color::Srgba(bevy::color::Srgba::new(0.04, 0.04, 0.04, 0.82));

/// ボタン本体スポーン。w は Val::Percent(49.0) か Val::Percent(100.0) を使う。
pub(super) fn spawn_btn(
    p: &mut ChildSpawnerCommands,
    a: VisualTestAction,
    label: &str,
    w: Val,
    font: &Handle<Font>,
) {
    p.spawn((
        Button,
        Node {
            width: w,
            height: Val::Px(BTN_H),
            margin: UiRect {
                right: Val::Px(BTN_GAP),
                bottom: Val::Px(BTN_GAP),
                ..default()
            },
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(BTN_DEF),
        a,
    ))
    .with_children(|b| {
        b.spawn((
            Text::new(label),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(SFONT),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.9, 0.92)),
        ));
    });
}

/// セクションラベル（flex-wrap 親内で width:100% により改行）。
pub(super) fn sec_label(p: &mut ChildSpawnerCommands, text: &str, font: &Handle<Font>) {
    p.spawn((
        Text::new(text),
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(10.0),
            ..default()
        },
        TextColor(SEC_COL),
        Node {
            width: Val::Percent(100.0),
            margin: UiRect {
                top: Val::Px(8.0),
                bottom: Val::Px(3.0),
                ..default()
            },
            ..default()
        },
    ));
}
