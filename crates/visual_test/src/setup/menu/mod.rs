use super::*;

mod building;
mod camera;
mod header;
mod soul;
mod widgets;

use building::spawn_build_section;
use camera::spawn_camera_section;
use header::spawn_header;
use soul::spawn_soul_section;
use widgets::{
    BTN_GAP, BTN_H, DIM_COL, PANEL_BG, SFONT, VAL_COL, param_row, sec_label, small_btn, spawn_btn,
    val_text,
};

pub(super) fn spawn_menu_ui(commands: &mut Commands, font: Handle<Font>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(MENU_WIDTH),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            ScrollPosition::default(),
            MenuPanel,
        ))
        .with_children(|p| {
            spawn_header(p, &font);
            spawn_camera_section(p, &font);
            spawn_soul_section(p, &font);
            spawn_build_section(p, &font);
        });

    commands.spawn((
        Text::new("[H] メニュー表示"),
        TextFont {
            font: font.into(),
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.55)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
            ..default()
        },
        Visibility::Hidden,
        MenuHint,
    ));
}
