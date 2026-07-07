//! 設定画面 UI（BSN ルート + ui_widgets Slider/Checkbox）

use super::UiAssets;
use crate::components::{
    MenuAction, MenuButton, SettingsCheckboxMarker, SettingsCheckmarkMarker,
    SettingsDefaultSpeedButton, SettingsField, SettingsPanel, SettingsSliderMarker,
    SettingsSliderThumbMarker, UiInputBlocker,
};
use crate::theme::UiTheme;
use bevy::prelude::*;
use bevy::scene::bsn;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::{Checkbox, Slider, SliderRange, SliderStep, SliderValue};
use hw_core::game_state::TimeSpeed;

/// 設定パネル初期値（hw_core::GameSettings への依存を避ける DTO）
#[derive(Clone, Copy, Debug)]
pub struct SettingsPanelInitial {
    pub ui_scale: f32,
    pub camera_pan_speed: f32,
    pub camera_mouse_pan_enabled: bool,
    pub default_time_speed: TimeSpeed,
    pub debug_gizmos_enabled: bool,
    pub fps_display_enabled: bool,
}

struct SliderRowSpec<'a> {
    label: &'a str,
    field: SettingsField,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
}

pub fn spawn_settings_panel(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
    initial: SettingsPanelInitial,
) {
    let panel = commands
        .spawn_scene(bsn! {
            SettingsPanel
        })
        .id();

    commands.entity(panel).insert((
        UiInputBlocker,
        ZIndex(36),
        Node {
            display: Display::None,
            width: Val::Px(380.0),
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(45.0),
            margin: UiRect::left(Val::Px(-190.0)),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(16.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(theme.colors.dialog_bg),
        BorderColor::all(theme.colors.dialog_border),
        Interaction::default(),
        RelativeCursorPosition::default(),
    ));

    commands.entity(parent_entity).add_child(panel);

    commands.entity(panel).with_children(|parent| {
        parent.spawn((
            Text::new("Settings"),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: FontSize::Px(theme.typography.font_size_xl),
                ..default()
            },
            TextColor(theme.colors.text_accent),
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                align_self: AlignSelf::Center,
                ..default()
            },
        ));

        spawn_slider_row(
            parent,
            game_assets,
            theme,
            SliderRowSpec {
                label: "UI Scale",
                field: SettingsField::UiScale,
                value: initial.ui_scale,
                min: 0.85,
                max: 1.25,
                step: 0.05,
            },
        );
        spawn_slider_row(
            parent,
            game_assets,
            theme,
            SliderRowSpec {
                label: "Camera Pan Speed",
                field: SettingsField::CameraPanSpeed,
                value: initial.camera_pan_speed,
                min: 200.0,
                max: 1000.0,
                step: 50.0,
            },
        );

        spawn_checkbox_row(
            parent,
            game_assets,
            theme,
            "Mouse Drag Pan",
            SettingsField::CameraMousePan,
            initial.camera_mouse_pan_enabled,
        );

        spawn_default_speed_row(parent, game_assets, theme, initial.default_time_speed);

        spawn_checkbox_row(
            parent,
            game_assets,
            theme,
            "Debug Gizmos",
            SettingsField::DebugGizmos,
            initial.debug_gizmos_enabled,
        );
        spawn_checkbox_row(
            parent,
            game_assets,
            theme,
            "Show FPS",
            SettingsField::FpsDisplay,
            initial.fps_display_enabled,
        );

        parent
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(36.0),
                    margin: UiRect::top(Val::Px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(theme.colors.button_default),
                BorderColor::all(theme.colors.dialog_border),
                MenuButton(MenuAction::CloseSettings),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Close"),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: FontSize::Px(theme.typography.font_size_base),
                        ..default()
                    },
                    TextColor(theme.colors.text_primary_semantic),
                ));
            });
    });
}

fn spawn_slider_row(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    spec: SliderRowSpec<'_>,
) {
    let SliderRowSpec {
        label,
        field,
        value,
        min,
        max,
        step,
    } = spec;
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_sm),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));

            row.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(24.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                Slider::default(),
                SettingsSliderMarker(field),
                SliderValue(value),
                SliderRange::from_range(min..=max),
                SliderStep(step),
            ))
            .with_children(|slider| {
                slider.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(4.0),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(theme.colors.border_default),
                ));

                slider.spawn((
                    Node {
                        width: Val::Px(14.0),
                        height: Val::Px(14.0),
                        position_type: PositionType::Absolute,
                        left: Val::Percent(0.0),
                        top: Val::Px(5.0),
                        margin: UiRect::left(Val::Px(-7.0)),
                        border_radius: BorderRadius::all(Val::Px(7.0)),
                        ..default()
                    },
                    BackgroundColor(theme.colors.accent_ember_bright),
                    SettingsSliderThumbMarker(field),
                ));
            });
        });
}

fn spawn_checkbox_row(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    label: &str,
    field: SettingsField,
    checked: bool,
) {
    let mut entity = parent.spawn((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        },
        Checkbox,
        SettingsCheckboxMarker(field),
    ));

    if checked {
        entity.insert(bevy::ui::Checked);
    }

    entity.with_children(|row| {
        row.spawn((
            Node {
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.colors.bg_elevated),
            BorderColor::all(theme.colors.border_default),
        ))
        .with_children(|box_node| {
            box_node.spawn((
                Node {
                    width: Val::Px(10.0),
                    height: Val::Px(10.0),
                    display: if checked {
                        Display::Flex
                    } else {
                        Display::None
                    },
                    ..default()
                },
                BackgroundColor(theme.colors.accent_ember),
                SettingsCheckmarkMarker(field),
            ));
        });

        row.spawn((
            Text::new(label),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: FontSize::Px(theme.typography.font_size_sm),
                ..default()
            },
            TextColor(theme.colors.text_primary_semantic),
        ));
    });
}

fn spawn_default_speed_row(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    selected: TimeSpeed,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new("Default Game Speed"),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_sm),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));

            row.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|speed_row| {
                let speeds = [
                    (TimeSpeed::Paused, "||"),
                    (TimeSpeed::Normal, ">"),
                    (TimeSpeed::Fast, ">>"),
                    (TimeSpeed::Super, ">>>"),
                ];

                for (speed, label) in speeds {
                    let active = speed == selected;
                    speed_row
                        .spawn((
                            Button,
                            Node {
                                width: Val::Px(36.0),
                                height: Val::Px(28.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(if active {
                                theme.colors.speed_button_active
                            } else {
                                theme.colors.button_default
                            }),
                            BorderColor::all(theme.colors.time_control_border),
                            SettingsDefaultSpeedButton(speed),
                            MenuButton(MenuAction::SetDefaultTimeSpeed(speed)),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(label),
                                TextFont {
                                    font: game_assets.font_ui().clone().into(),
                                    font_size: FontSize::Px(theme.typography.font_size_sm),
                                    ..default()
                                },
                                TextColor(theme.colors.text_primary_semantic),
                            ));
                        });
                }
            });
        });
}
