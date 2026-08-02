//! ダイアログ UI

use super::UiAssets;
use crate::components::{
    LoadConfirmDialog, MenuAction, MenuButton, OperationDialog, OperationDialogScroll,
    OperationPolicyAllDisabledWarning, OperationPolicyAllowedButton, OperationPolicyAllowedText,
    OperationPolicyPriorityButton, OperationPolicyPriorityText, OperationPolicyRow, UiInputBlocker,
    UiInputCapture, UiNodeRegistry, UiSlot,
};
use crate::overlay::{LOAD_CONFIRM_LAYER, OPERATION_DIALOG_LAYER};
use crate::theme::UiTheme;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};
use hw_core::familiar::{FamiliarSettingsPatch, FamiliarWorkPriority};
use hw_core::jobs::WorkType;

/// ダイアログをスポーン
pub fn spawn_dialogs(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    spawn_operation_dialog(commands, game_assets, theme, parent_entity, ui_nodes);
    spawn_load_confirm_dialog(commands, game_assets, theme, parent_entity);
}

fn spawn_operation_dialog(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
) {
    let dialog_root = commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            FocusPolicy::Block,
            Pickable::default(),
            UiInputCapture,
            OperationDialog,
            OPERATION_DIALOG_LAYER,
            Name::new("Operation Dialog Capture"),
        ))
        .id();
    commands.entity(parent_entity).add_child(dialog_root);

    let dialog_shell = commands
        .spawn((
            Node {
                width: Val::Px(640.0),
                max_width: Val::Percent(92.0),
                height: Val::Percent(88.0),
                max_height: Val::Percent(88.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            Name::new("Operation Dialog Shell"),
        ))
        .id();
    commands.entity(dialog_root).add_child(dialog_shell);

    let dialog_panel = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(15.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(theme.colors.dialog_bg),
            BorderColor::all(theme.colors.dialog_border),
            Interaction::default(),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            Name::new("Operation Dialog Panel"),
        ))
        .id();
    commands.entity(dialog_shell).add_child(dialog_panel);

    commands.entity(dialog_panel).with_children(|parent| {
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(10.0)),
                ..default()
            })
            .with_children(|header| {
                header.spawn((
                    Text::new("Familiar Operation"),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: FontSize::Px(theme.typography.font_size_xl),
                        ..default()
                    },
                    TextColor(theme.colors.text_accent),
                ));

                header
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Px(24.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(theme.colors.button_default),
                        MenuButton(MenuAction::CloseDialog),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("X"),
                            TextFont {
                                font: game_assets.font_ui().clone().into(),
                                font_size: FontSize::Px(theme.typography.font_size_dialog_small),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
            });

        let familiar_name = parent
            .spawn((
                Text::new("Familiar Name"),
                TextFont {
                    font: game_assets.font_familiar().clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_title),
                    ..default()
                },
                TextColor(theme.colors.header_text),
                UiSlot::DialogFamiliarName,
                Node {
                    margin: UiRect::bottom(Val::Px(15.0)),
                    ..default()
                },
            ))
            .id();
        ui_nodes.set_slot(UiSlot::DialogFamiliarName, familiar_name);

        parent
            .spawn(Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            })
            .with_children(|row| {
                let scroll_area = row
                    .spawn((
                        Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            min_height: Val::Px(0.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(8.0),
                            padding: UiRect::right(Val::Px(8.0)),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        RelativeCursorPosition::default(),
                        UiInputBlocker,
                        ScrollArea,
                        OperationDialogScroll,
                        Name::new("Operation Dialog Scroll Area"),
                    ))
                    .id();
                row.commands().entity(scroll_area).with_children(|scroll| {
                    spawn_operation_controls(scroll, game_assets, theme, ui_nodes);
                    spawn_policy_editor(scroll, game_assets, theme);
                });

                row.spawn((
                    Node {
                        width: Val::Px(6.0),
                        height: Val::Percent(100.0),
                        margin: UiRect::left(Val::Px(4.0)),
                        ..default()
                    },
                    Scrollbar::new(scroll_area, ControlOrientation::Vertical, 20.0),
                ))
                .with_children(|scrollbar| {
                    scrollbar.spawn((
                        ScrollbarThumb {
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            border: UiRect::ZERO,
                        },
                        BackgroundColor(theme.colors.text_muted),
                    ));
                });
            });
    });
}

fn spawn_operation_controls(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    ui_nodes: &mut UiNodeRegistry,
) {
    parent.spawn((
        Text::new("Work Standards"),
        TextFont {
            font: game_assets.font_ui().clone().into(),
            font_size: FontSize::Px(theme.typography.font_size_dialog_small),
            weight: FontWeight::SEMIBOLD,
            ..default()
        },
        TextColor(theme.colors.text_secondary),
    ));
    parent.spawn((
        Text::new("Recruit Fatigue Threshold"),
        TextFont {
            font: game_assets.font_ui().clone().into(),
            font_size: FontSize::Px(theme.typography.font_size_dialog_tiny),
            ..default()
        },
        TextColor(theme.colors.text_secondary),
    ));
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                padding: UiRect::vertical(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(theme.colors.overlay_row_bg),
        ))
        .with_children(|row| {
            spawn_operation_button(
                row,
                game_assets,
                theme,
                "-",
                MenuAction::ApplyFamiliarSettings {
                    patch: FamiliarSettingsPatch::AdjustFatigueThreshold { steps: -1 },
                },
                34.0,
            );
            let threshold = row
                .spawn((
                    Text::new("100%"),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: FontSize::Px(theme.typography.font_size_title),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        min_width: Val::Px(150.0),
                        margin: UiRect::horizontal(Val::Px(16.0)),
                        ..default()
                    },
                    TextLayout::new(Justify::Center, LineBreak::NoWrap),
                    UiSlot::DialogThresholdText,
                ))
                .id();
            ui_nodes.set_slot(UiSlot::DialogThresholdText, threshold);
            spawn_operation_button(
                row,
                game_assets,
                theme,
                "+",
                MenuAction::ApplyFamiliarSettings {
                    patch: FamiliarSettingsPatch::AdjustFatigueThreshold { steps: 1 },
                },
                34.0,
            );
        });

    parent.spawn((
        Text::new("Max Controlled Souls:"),
        TextFont {
            font: game_assets.font_ui().clone().into(),
            font_size: FontSize::Px(theme.typography.font_size_dialog_tiny),
            ..default()
        },
        TextColor(theme.colors.text_secondary),
    ));

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                padding: UiRect::vertical(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(theme.colors.overlay_row_bg),
        ))
        .with_children(|row| {
            spawn_operation_button(
                row,
                game_assets,
                theme,
                "-",
                MenuAction::ApplyFamiliarSettings {
                    patch: FamiliarSettingsPatch::AdjustMaxControlledSoul { delta: -1 },
                },
                34.0,
            );
            let max_soul = row
                .spawn((
                    Text::new("2"),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: FontSize::Px(theme.typography.font_size_title),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        min_width: Val::Px(150.0),
                        margin: UiRect::horizontal(Val::Px(16.0)),
                        ..default()
                    },
                    TextLayout::new(Justify::Center, LineBreak::NoWrap),
                    UiSlot::DialogMaxSoulText,
                ))
                .id();
            ui_nodes.set_slot(UiSlot::DialogMaxSoulText, max_soul);
            spawn_operation_button(
                row,
                game_assets,
                theme,
                "+",
                MenuAction::ApplyFamiliarSettings {
                    patch: FamiliarSettingsPatch::AdjustMaxControlledSoul { delta: 1 },
                },
                34.0,
            );
        });
}

fn spawn_policy_editor(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    parent.spawn((
        Text::new("New Work Assignment Policy"),
        TextFont {
            font: game_assets.font_ui().clone().into(),
            font_size: FontSize::Px(theme.typography.font_size_dialog_small),
            weight: FontWeight::SEMIBOLD,
            ..default()
        },
        TextColor(theme.colors.text_secondary),
        Node {
            margin: UiRect::top(Val::Px(10.0)),
            ..default()
        },
    ));

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|row| {
            spawn_operation_button(
                row,
                game_assets,
                theme,
                "Enable all",
                MenuAction::ApplyFamiliarSettings {
                    patch: FamiliarSettingsPatch::SetAllWorkAllowed { allowed: true },
                },
                120.0,
            );
            spawn_operation_button(
                row,
                game_assets,
                theme,
                "Disable all",
                MenuAction::ApplyFamiliarSettings {
                    patch: FamiliarSettingsPatch::SetAllWorkAllowed { allowed: false },
                },
                120.0,
            );
        });

    parent.spawn((
        Text::new(
            "All work is disabled. Current work and self-maintenance continue; no new work is assigned.",
        ),
        TextFont {
            font: game_assets.font_ui().clone().into(),
            font_size: FontSize::Px(theme.typography.font_size_dialog_tiny),
            weight: FontWeight::SEMIBOLD,
            ..default()
        },
        TextColor(theme.colors.status_warning),
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
        Node {
            display: Display::None,
            width: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        OperationPolicyAllDisabledWarning,
    ));

    for work_type in WorkType::ALL {
        spawn_policy_row(parent, game_assets, theme, work_type);
    }
}

fn spawn_policy_row(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    work_type: WorkType,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(38.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(theme.colors.overlay_row_bg),
            OperationPolicyRow(work_type),
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(crate::panels::task_list::work_type_label(&work_type)),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_dialog_tiny),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(120.0),
                    ..default()
                },
            ));

            row.spawn((
                Button,
                Node {
                    width: Val::Px(94.0),
                    height: Val::Px(28.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(theme.colors.button_default),
                MenuButton(MenuAction::ApplyFamiliarSettings {
                    patch: FamiliarSettingsPatch::SetWorkAllowed {
                        work_type,
                        allowed: false,
                    },
                }),
                OperationPolicyAllowedButton(work_type),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Enabled"),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: FontSize::Px(theme.typography.font_size_dialog_tiny),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    OperationPolicyAllowedText(work_type),
                ));
            });

            row.spawn((
                Button,
                Node {
                    width: Val::Px(84.0),
                    height: Val::Px(28.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(theme.colors.button_default),
                MenuButton(MenuAction::ApplyFamiliarSettings {
                    patch: FamiliarSettingsPatch::SetWorkPriority {
                        work_type,
                        priority: FamiliarWorkPriority::High,
                    },
                }),
                OperationPolicyPriorityButton(work_type),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("Normal"),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: FontSize::Px(theme.typography.font_size_dialog_tiny),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    OperationPolicyPriorityText(work_type),
                ));
            });
        });
}

fn spawn_operation_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    label: &str,
    action: MenuAction,
    width: f32,
) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(width),
                height: Val::Px(30.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            MenuButton(action),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_dialog_tiny),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn spawn_load_confirm_dialog(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let dialog_root = commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            FocusPolicy::Block,
            Pickable::default(),
            UiInputCapture,
            LoadConfirmDialog,
            LOAD_CONFIRM_LAYER,
            Name::new("Load Confirm Capture"),
        ))
        .id();
    commands.entity(parent_entity).add_child(dialog_root);

    let dialog_panel = commands
        .spawn((
            Node {
                width: Val::Px(360.0),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(40.0),
                margin: UiRect::left(Val::Px(-180.0)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(theme.colors.dialog_bg),
            BorderColor::all(theme.colors.dialog_border),
            Interaction::default(),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            Name::new("Load Confirm Panel"),
        ))
        .id();
    commands.entity(dialog_root).add_child(dialog_panel);

    commands.entity(dialog_panel).with_children(|parent| {
        parent.spawn((
            Text::new("Load saved game?"),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: FontSize::Px(theme.typography.font_size_xl),
                ..default()
            },
            TextColor(theme.colors.text_accent),
            Node {
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
        ));

        parent.spawn((
            Text::new("Current progress will be lost. This cannot be undone."),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: FontSize::Px(theme.typography.font_size_dialog_small),
                ..default()
            },
            TextColor(theme.colors.text_secondary),
            Node {
                margin: UiRect::bottom(Val::Px(16.0)),
                ..default()
            },
        ));

        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Button,
                    Node {
                        width: Val::Px(90.0),
                        height: Val::Px(32.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.colors.button_default),
                    MenuButton(MenuAction::CancelLoadConfirm),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Cancel"),
                        TextFont {
                            font: game_assets.font_ui().clone().into(),
                            font_size: FontSize::Px(theme.typography.font_size_dialog_small),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

                row.spawn((
                    Button,
                    Node {
                        width: Val::Px(90.0),
                        height: Val::Px(32.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(theme.colors.button_default),
                    MenuButton(MenuAction::ConfirmLoadGame),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Load"),
                        TextFont {
                            font: game_assets.font_ui().clone().into(),
                            font_size: FontSize::Px(theme.typography.font_size_dialog_small),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });
    });
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::setup::test_support::TestAssets;

    fn spawn_operation_fixture(mut commands: Commands, theme: Res<UiTheme>) {
        let parent = commands.spawn(Node::default()).id();
        let mut ui_nodes = UiNodeRegistry::default();
        spawn_operation_dialog(
            &mut commands,
            &TestAssets::default(),
            &theme,
            parent,
            &mut ui_nodes,
        );
    }

    #[test]
    fn operation_dialog_uses_bounded_scroll_and_every_work_type_row() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiTheme>()
            .add_systems(Startup, spawn_operation_fixture);
        app.update();

        let (scroll_entity, scroll_node, scroll_position) = app
            .world_mut()
            .query_filtered::<
                (Entity, &Node, &ScrollPosition),
                (With<ScrollArea>, With<OperationDialogScroll>),
            >()
            .single(app.world())
            .expect("one operation scroll area");
        assert_eq!(scroll_node.overflow.y, OverflowAxis::Scroll);
        assert_eq!(scroll_position.0, Vec2::ZERO);

        let scrollbar = app
            .world_mut()
            .query::<&Scrollbar>()
            .single(app.world())
            .expect("one operation scrollbar");
        assert_eq!(scrollbar.target, scroll_entity);
        assert_eq!(scrollbar.orientation, ControlOrientation::Vertical);

        let rows = app
            .world_mut()
            .query::<&OperationPolicyRow>()
            .iter(app.world())
            .map(|row| row.0)
            .collect::<HashSet<_>>();
        assert_eq!(rows.len(), WorkType::ALL.len());
        assert_eq!(rows, WorkType::ALL.into_iter().collect());

        let all_actions_are_targetless = app
            .world_mut()
            .query::<&MenuButton>()
            .iter(app.world())
            .filter(|button| {
                matches!(
                    button.0,
                    MenuAction::ApplyFamiliarSettings { .. }
                        | MenuAction::ApplyFamiliarSettingsFor { .. }
                )
            })
            .all(|button| matches!(button.0, MenuAction::ApplyFamiliarSettings { .. }));
        assert!(all_actions_are_targetless);
    }

    #[test]
    fn operation_dialog_centers_a_percentage_bounded_shell_without_fixed_offset() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiTheme>()
            .add_systems(Startup, spawn_operation_fixture);
        app.update();

        let root = app
            .world_mut()
            .query_filtered::<&Node, With<OperationDialog>>()
            .single(app.world())
            .unwrap();
        assert_eq!(root.justify_content, JustifyContent::Center);
        assert_eq!(root.align_items, AlignItems::Center);

        let shell = app
            .world_mut()
            .query::<(&Name, &Node)>()
            .iter(app.world())
            .find_map(|(name, node)| (name.as_str() == "Operation Dialog Shell").then_some(node))
            .expect("operation dialog shell");
        assert_eq!(shell.width, Val::Px(640.0));
        assert_eq!(shell.max_width, Val::Percent(92.0));
        assert_eq!(shell.height, Val::Percent(88.0));
        assert_eq!(shell.position_type, PositionType::Relative);
        assert_eq!(shell.margin, UiRect::ZERO);
    }
}
