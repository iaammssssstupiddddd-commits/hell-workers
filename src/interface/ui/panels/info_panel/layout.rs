use crate::interface::ui::components::{
    InfoPanel, InfoPanelNodes, MenuAction, MenuButton, UiInputBlocker, UiNodeRegistry, UiSlot,
};
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient, RelativeCursorPosition};

fn spawn_info_section_divider(
    parent: &mut ChildSpawnerCommands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
    label: &str,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            margin: UiRect {
                top: Val::Px(6.0),
                bottom: Val::Px(4.0),
                ..default()
            },
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(theme.colors.border_default),
            ));
            row.spawn((
                Text::new(label),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_xs,
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TextColor(theme.colors.text_secondary_semantic),
            ));
            row.spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(theme.colors.border_default),
            ));
        });
}

pub fn spawn_info_panel_ui(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
    info_panel_nodes: &mut InfoPanelNodes,
) {
    let root = commands
        .spawn((
            Node {
                width: Val::Px(theme.sizes.info_panel_width),
                min_width: Val::Px(theme.sizes.info_panel_min_width),
                max_width: Val::Px(theme.sizes.info_panel_max_width),
                height: Val::Auto,
                position_type: PositionType::Absolute,
                right: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.spacing.panel_top),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme.spacing.panel_padding)),
                border: UiRect::all(Val::Px(theme.sizes.panel_border_width)),
                border_radius: BorderRadius::all(Val::Px(theme.sizes.panel_corner_radius)),
                display: Display::None,
                ..default()
            },
            BackgroundGradient::from(LinearGradient {
                angle: 0.0,
                stops: vec![
                    ColorStop::new(theme.panels.info_panel.top, Val::Percent(0.0)),
                    ColorStop::new(theme.panels.info_panel.bottom, Val::Percent(100.0)),
                ],
                ..default()
            }),
            BorderColor::all(theme.colors.border_default),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            InfoPanel,
            UiSlot::InfoPanelRoot,
        ))
        .id();
    commands.entity(parent_entity).add_child(root);
    ui_nodes.set_slot(UiSlot::InfoPanelRoot, root);
    info_panel_nodes.root = Some(root);

    commands.entity(root).with_children(|parent| {
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                margin: UiRect::bottom(Val::Px(5.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|left| {
                    let header = left
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_title,
                                weight: FontWeight::BOLD,
                                ..default()
                            },
                            TextColor(theme.colors.panel_accent_info_panel),
                            UiSlot::Header,
                        ))
                        .id();
                    ui_nodes.set_slot(UiSlot::Header, header);
                    info_panel_nodes.header = Some(header);

                    let gender = left
                        .spawn((
                            ImageNode::default(),
                            Node {
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                margin: UiRect::left(Val::Px(8.0)),
                                display: Display::None,
                                ..default()
                            },
                            UiSlot::GenderIcon,
                        ))
                        .id();
                    ui_nodes.set_slot(UiSlot::GenderIcon, gender);
                    info_panel_nodes.gender_icon = Some(gender);
                });

                let unpin_button = row
                    .spawn((
                        Button,
                        Node {
                            display: Display::None,
                            min_height: Val::Px(24.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            padding: UiRect::horizontal(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(theme.colors.button_default),
                        MenuButton(MenuAction::ClearInspectPin),
                        UiSlot::InfoPanelUnpinButton,
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("Unpin"),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_xs,
                                weight: FontWeight::SEMIBOLD,
                                ..default()
                            },
                            TextColor(theme.colors.text_primary_semantic),
                        ));
                    })
                    .id();
                ui_nodes.set_slot(UiSlot::InfoPanelUnpinButton, unpin_button);
                info_panel_nodes.unpin_button = Some(unpin_button);
            });

        let stats = parent
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                UiSlot::InfoPanelStatsGroup,
            ))
            .with_children(|col| {
                spawn_info_section_divider(col, game_assets, theme, "Status");

                let motivation = col
                    .spawn((
                        Text::new(""),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_small,
                            ..default()
                        },
                        UiSlot::StatMotivation,
                    ))
                    .id();
                ui_nodes.set_slot(UiSlot::StatMotivation, motivation);
                info_panel_nodes.motivation = Some(motivation);

                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        ImageNode::new(game_assets.icon_stress.clone()),
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            margin: UiRect::right(Val::Px(4.0)),
                            ..default()
                        },
                    ));
                    let stress = row
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_small,
                                ..default()
                            },
                            UiSlot::StatStress,
                        ))
                        .id();
                    ui_nodes.set_slot(UiSlot::StatStress, stress);
                    info_panel_nodes.stress = Some(stress);
                });

                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        ImageNode::new(game_assets.icon_fatigue.clone()),
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            margin: UiRect::right(Val::Px(4.0)),
                            ..default()
                        },
                    ));
                    let fatigue = row
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_small,
                                ..default()
                            },
                            UiSlot::StatFatigue,
                        ))
                        .id();
                    ui_nodes.set_slot(UiSlot::StatFatigue, fatigue);
                    info_panel_nodes.fatigue = Some(fatigue);
                });

                spawn_info_section_divider(col, game_assets, theme, "Current Task");

                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    margin: UiRect::top(Val::Px(5.0)),
                    ..default()
                })
                .with_children(|row| {
                    let task = row
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_small,
                                ..default()
                            },
                            UiSlot::TaskText,
                        ))
                        .id();
                    ui_nodes.set_slot(UiSlot::TaskText, task);
                    info_panel_nodes.task = Some(task);
                });

                spawn_info_section_divider(col, game_assets, theme, "Inventory");

                let inventory = col
                    .spawn((
                        Text::new(""),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_small,
                            ..default()
                        },
                        UiSlot::InventoryText,
                    ))
                    .id();
                ui_nodes.set_slot(UiSlot::InventoryText, inventory);
                info_panel_nodes.inventory = Some(inventory);
            })
            .id();
        ui_nodes.set_slot(UiSlot::InfoPanelStatsGroup, stats);
        info_panel_nodes.stats_group = Some(stats);

        let common = parent
            .spawn((
                Text::new(""),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_item,
                    ..default()
                },
                TextColor(theme.colors.text_primary),
                UiSlot::CommonText,
            ))
            .id();
        ui_nodes.set_slot(UiSlot::CommonText, common);
        info_panel_nodes.common = Some(common);
    });
}
