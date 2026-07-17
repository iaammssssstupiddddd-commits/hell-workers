use super::*;

pub(super) fn spawn_camera_section(p: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    sec_label(p, "─ カメラ ─", font);

    // 矢視ボタン（DynamicTextKind::ViewDir でラベル更新）
    p.spawn((
        Button,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(BTN_H),
            margin: UiRect::bottom(Val::Px(BTN_GAP)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(BTN_DEF),
        VisualTestAction::NextView,
    ))
    .with_children(|b| {
        b.spawn((
            Text::new("TopDown  [V]"),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(SFONT),
                ..default()
            },
            TextColor(Color::srgb(0.9, 0.9, 0.92)),
            DynamicTextKind::ViewDir,
        ));
    });

    // HEIGHT 行
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        margin: UiRect::bottom(Val::Px(BTN_GAP)),
        ..default()
    })
    .with_children(|row| {
        row.spawn((
            Text::new("H:"),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(SFONT),
                ..default()
            },
            TextColor(DIM_COL),
            Node {
                min_width: Val::Px(22.0),
                ..default()
            },
        ));
        small_btn(row, VisualTestAction::HeightDown, "−", font);
        val_text(row, "150", DynamicTextKind::Height, font);
        small_btn(row, VisualTestAction::HeightUp, "+", font);
        small_btn(row, VisualTestAction::ResetElevation, "O", font);
    });

    // OFFSET 行
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        margin: UiRect::bottom(Val::Px(BTN_GAP)),
        ..default()
    })
    .with_children(|row| {
        row.spawn((
            Text::new("Off:"),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(SFONT),
                ..default()
            },
            TextColor(DIM_COL),
            Node {
                min_width: Val::Px(22.0),
                ..default()
            },
        ));
        small_btn(row, VisualTestAction::OffsetDown, "−", font);
        val_text(row, "90", DynamicTextKind::Offset, font);
        small_btn(row, VisualTestAction::OffsetUp, "+", font);
    });
}
