//! 設定画面 UI（BSN ルート + ui_widgets Slider/Checkbox）

use super::UiAssets;
use crate::components::{
    MenuAction, MenuButton, SettingsCheckboxMarker, SettingsCheckmarkMarker,
    SettingsDefaultSpeedButton, SettingsField, SettingsPanel, SettingsSliderMarker,
    SettingsSliderThumbMarker, SettingsValueText, UiInputBlocker, UiInputCapture,
};
use crate::overlay::SETTINGS_LAYER;
use crate::theme::UiTheme;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::scene::bsn;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use bevy::ui_widgets::{
    Checkbox, ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb, Slider, SliderRange,
    SliderStep, SliderValue,
};
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
    pub power_priority_enabled: bool,
    pub autosave_enabled: bool,
    pub autosave_interval_slider: f32,
    pub autosave_generations_slider: f32,
    pub notification_duration_slider: f32,
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
    let capture_root = commands
        .spawn_scene(bsn! {
            SettingsPanel
        })
        .id();

    commands.entity(capture_root).insert((
        UiInputCapture,
        SETTINGS_LAYER,
        Node {
            display: Display::None,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        FocusPolicy::Block,
        Pickable::default(),
        Name::new("Settings Capture"),
    ));

    commands.entity(parent_entity).add_child(capture_root);

    let panel = commands
        .spawn((
            UiInputBlocker,
            Node {
                width: Val::Px(380.0),
                max_width: Val::Percent(92.0),
                max_height: Val::Percent(88.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(theme.colors.dialog_bg.with_alpha(1.0)),
            BorderColor::all(theme.colors.dialog_border),
            Interaction::default(),
            RelativeCursorPosition::default(),
            Name::new("Settings Panel"),
        ))
        .id();

    commands.entity(capture_root).add_child(panel);

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
                flex_shrink: 0.0,
                ..default()
            },
        ));

        parent.spawn((
            crate::components::SettingsSaveStatusText,
            Text::new("変更は画面を閉じると保存されます。"),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: FontSize::Px(theme.typography.font_size_sm),
                ..default()
            },
            TextColor(theme.colors.text_primary_semantic),
            Node {
                flex_shrink: 0.0,
                ..default()
            },
        ));

        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(0.0),
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|row| {
                let scroll_area = row
                    .spawn((
                        Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            min_height: Val::Px(0.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(10.0),
                            overflow: Overflow::scroll_y(),
                            padding: UiRect::right(Val::Px(8.0)),
                            ..default()
                        },
                        ScrollArea,
                        UiInputBlocker,
                        RelativeCursorPosition::default(),
                        Name::new("Settings Scroll Area"),
                    ))
                    .with_children(|parent| {
                        spawn_settings_controls(parent, game_assets, theme, initial);
                    })
                    .id();
                row.spawn((
                    Node {
                        width: Val::Px(6.0),
                        ..default()
                    },
                    Scrollbar::new(scroll_area, ControlOrientation::Vertical, 20.0),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        ScrollbarThumb {
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            border: UiRect::ZERO,
                        },
                        BackgroundColor(theme.colors.text_muted),
                    ));
                });
            });
        spawn_settings_close(parent, game_assets, theme);
    });
}

fn spawn_settings_controls(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    initial: SettingsPanelInitial,
) {
    spawn_slider_row(
        parent,
        game_assets,
        theme,
        SliderRowSpec {
            label: "UI倍率（即時反映）",
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
            label: "カメラ移動速度（即時反映）",
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
    spawn_slider_row(
        parent,
        game_assets,
        theme,
        SliderRowSpec {
            label: "通知の表示時間（新しい通知から反映）",
            field: SettingsField::NotificationDuration,
            value: initial.notification_duration_slider,
            min: 0.0,
            max: 2.0,
            step: 1.0,
        },
    );

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
    spawn_checkbox_row(
        parent,
        game_assets,
        theme,
        "Power priority allocation",
        SettingsField::PowerPriority,
        initial.power_priority_enabled,
    );

    spawn_checkbox_row(
        parent,
        game_assets,
        theme,
        "Autosave",
        SettingsField::AutosaveEnabled,
        initial.autosave_enabled,
    );
    spawn_slider_row(
        parent,
        game_assets,
        theme,
        SliderRowSpec {
            label: "自動保存間隔（次の保存待ち時間に反映）",
            field: SettingsField::AutosaveInterval,
            value: initial.autosave_interval_slider,
            min: 0.0,
            max: 3.0,
            step: 1.0,
        },
    );
    spawn_slider_row(
        parent,
        game_assets,
        theme,
        SliderRowSpec {
            label: "自動保存の保持数（次の自動保存時に反映）",
            field: SettingsField::AutosaveGenerations,
            value: initial.autosave_generations_slider,
            min: 1.0,
            max: 5.0,
            step: 1.0,
        },
    );
}

fn spawn_settings_close(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(36.0),
                flex_shrink: 0.0,
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
                Text::default(),
                SettingsValueText(field),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_sm),
                    ..default()
                },
                TextColor(theme.colors.text_accent),
            ));

            row.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(32.0),
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
            min_height: Val::Px(32.0),
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
                Text::new("起動時のゲーム速度（次回起動時に反映）"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::test_support::TestAssets;

    fn fixture(mut commands: Commands, theme: Res<UiTheme>) {
        let root = commands.spawn(Node::default()).id();
        spawn_settings_panel(
            &mut commands,
            &TestAssets::default(),
            &theme,
            root,
            SettingsPanelInitial {
                ui_scale: 1.0,
                camera_pan_speed: 500.0,
                camera_mouse_pan_enabled: true,
                default_time_speed: TimeSpeed::Normal,
                debug_gizmos_enabled: false,
                fps_display_enabled: false,
                power_priority_enabled: true,
                autosave_enabled: false,
                autosave_interval_slider: 1.0,
                notification_duration_slider: 0.0,
                autosave_generations_slider: 1.0,
            },
        );
    }

    #[test]
    fn settings_close_and_title_stay_outside_the_shared_control_scroller() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins((
                bevy::asset::AssetPlugin::default(),
                bevy::scene::ScenePlugin,
            ))
            .init_resource::<UiTheme>()
            .add_systems(Startup, fixture);
        app.update();
        let scroll = app
            .world_mut()
            .query_filtered::<Entity, With<ScrollArea>>()
            .single(app.world())
            .unwrap();
        let panel = app
            .world_mut()
            .query::<(Entity, &Name)>()
            .iter(app.world())
            .find(|(_, name)| name.as_str() == "Settings Panel")
            .unwrap()
            .0;
        let controls: Vec<_> = app.world_mut().query_filtered::<Entity,
            Or<(With<SettingsSliderMarker>, With<SettingsCheckboxMarker>)>>()
            .iter(app.world()).collect();
        assert!(!controls.is_empty());
        for control in controls {
            let mut ancestor = control;
            while ancestor != scroll {
                ancestor = app
                    .world()
                    .get::<ChildOf>(ancestor)
                    .expect("every setting must be inside the scroller")
                    .parent();
            }
        }
        let close = app
            .world_mut()
            .query::<(&MenuButton, &ChildOf, &Node)>()
            .iter(app.world())
            .find(|(button, _, _)| matches!(button.0, MenuAction::CloseSettings))
            .unwrap();
        assert_eq!(close.1.parent(), panel);
        assert_eq!(close.2.flex_shrink, 0.0);
        let title = app
            .world_mut()
            .query::<(&Text, &ChildOf)>()
            .iter(app.world())
            .find(|(text, _)| text.0 == "Settings")
            .unwrap();
        assert_eq!(title.1.parent(), panel);
        assert_eq!(
            app.world_mut()
                .query::<&Scrollbar>()
                .single(app.world())
                .unwrap()
                .target,
            scroll
        );
    }
}
