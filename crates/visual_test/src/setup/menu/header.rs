use super::*;

pub(super) fn spawn_header(p: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    p.spawn((
        Text::new("Visual Test"),
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(14.0),
            weight: FontWeight::BOLD,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            margin: UiRect::bottom(Val::Px(6.0)),
            width: Val::Percent(100.0),
            ..default()
        },
    ));
    // モード切替ボタン (2 列)
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        width: Val::Percent(100.0),
        ..default()
    })
    .with_children(|row| {
        spawn_btn(
            row,
            VisualTestAction::SetMode(AppMode::Soul),
            "SOUL",
            Val::Percent(49.0),
            font,
        );
        spawn_btn(
            row,
            VisualTestAction::SetMode(AppMode::Build),
            "BUILD",
            Val::Percent(49.0),
            font,
        );
    });
}
