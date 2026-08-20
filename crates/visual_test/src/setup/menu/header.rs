use super::*;

pub(super) fn spawn_header(p: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    p.spawn((
        Text::new("TopDown Building Test"),
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
}
