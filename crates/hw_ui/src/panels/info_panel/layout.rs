use crate::components::{
    InfoPanel, InfoPanelNodes, MenuAction, MenuButton, SoulRenameButton, SoulRenameFieldContainer,
    StockpileAcceptanceRowNodes, UiInputBlocker, UiNodeRegistry, UiSlot,
};
use crate::setup::UiAssets;
use crate::theme::UiTheme;
use bevy::input::mouse::MouseScrollUnit;
use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient, RelativeCursorPosition};
use hw_logistics::{STOCKPILE_ACCEPTANCE_RESOURCES, StockpilePolicyPatch};

use crate::intents::StockpilePolicyEditTarget;
use crate::power::PowerPriorityValue;

const INFO_PANEL_MAX_HEIGHT_VH: f32 = 58.0;

fn scroll_info_panel(
    on_scroll: On<Pointer<Scroll>>,
    mut query: Query<(&mut ScrollPosition, &ComputedNode), With<InfoPanel>>,
) {
    let Ok((mut scroll_position, node)) = query.get_mut(on_scroll.entity) else {
        return;
    };
    let delta_y = match on_scroll.unit {
        MouseScrollUnit::Line => on_scroll.y * 20.0,
        MouseScrollUnit::Pixel => on_scroll.y,
    };
    let max_offset = (node.content_size.y - node.size.y).max(0.0) * node.inverse_scale_factor;
    scroll_position.y = (scroll_position.y - delta_y).clamp(0.0, max_offset);
}

fn spawn_info_section_divider(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
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
                    font: game_assets.font_ui().clone().into(),
                    font_size: crate::theme::font_size_rem(theme.typography.font_size_xs),
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

fn spawn_stockpile_editor_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    width: Val,
    label: &str,
) -> (Entity, Entity) {
    let mut text_entity = Entity::PLACEHOLDER;
    let button = parent
        .spawn((
            Button,
            Node {
                width,
                min_height: Val::Px(28.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            MenuButton(MenuAction::ApplyStockpilePolicy {
                target: StockpilePolicyEditTarget::Single(Entity::PLACEHOLDER),
                patch: StockpilePolicyPatch::default(),
            }),
        ))
        .with_children(|button| {
            text_entity = button
                .spawn((
                    Text::new(label),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: crate::theme::font_size_rem(theme.typography.font_size_xs),
                        weight: FontWeight::SEMIBOLD,
                        ..default()
                    },
                    TextColor(theme.colors.text_primary_semantic),
                ))
                .id();
        })
        .id();
    (button, text_entity)
}

fn spawn_soul_spa_slot_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    label: &str,
) -> Entity {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(30.0),
                min_height: Val::Px(28.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            MenuButton(MenuAction::SetSoulSpaActiveSlots {
                target: Entity::PLACEHOLDER,
                active_slots: 0,
            }),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: crate::theme::font_size_rem(theme.typography.font_size_small),
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
        })
        .id()
}

fn spawn_power_priority_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
) -> (Entity, Entity) {
    let mut text_entity = Entity::PLACEHOLDER;
    let button = parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(28.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            MenuButton(MenuAction::SetPowerConsumerPriority {
                target: Entity::PLACEHOLDER,
                priority: PowerPriorityValue::Normal,
            }),
        ))
        .with_children(|button| {
            text_entity = button
                .spawn((
                    Text::new("Priority"),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: crate::theme::font_size_rem(theme.typography.font_size_xs),
                        weight: FontWeight::SEMIBOLD,
                        ..default()
                    },
                    TextColor(theme.colors.text_primary_semantic),
                ))
                .id();
        })
        .id();
    (button, text_entity)
}

fn spawn_stockpile_acceptance_row(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    resource_type: hw_logistics::ResourceType,
) -> StockpileAcceptanceRowNodes {
    let mut text_entity = Entity::PLACEHOLDER;
    let button = parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(48.5),
                min_height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                padding: UiRect::horizontal(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            MenuButton(MenuAction::ApplyStockpilePolicy {
                target: StockpilePolicyEditTarget::Single(Entity::PLACEHOLDER),
                patch: StockpilePolicyPatch::default(),
            }),
        ))
        .with_children(|button| {
            text_entity = button
                .spawn((
                    Text::new(""),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: crate::theme::font_size_rem(theme.typography.font_size_xs),
                        weight: FontWeight::SEMIBOLD,
                        ..default()
                    },
                    TextColor(theme.colors.text_primary_semantic),
                ))
                .id();
        })
        .id();

    StockpileAcceptanceRowNodes {
        resource_type,
        button,
        text: text_entity,
    }
}

pub fn spawn_info_panel_ui(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
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
                max_height: Val::Vh(INFO_PANEL_MAX_HEIGHT_VH),
                position_type: PositionType::Absolute,
                right: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.spacing.panel_top),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme.spacing.panel_padding)),
                border: UiRect::all(Val::Px(theme.sizes.panel_border_width)),
                border_radius: BorderRadius::all(Val::Px(theme.sizes.panel_corner_radius)),
                overflow: Overflow::scroll_y(),
                display: Display::None,
                ..default()
            },
            ScrollPosition::default(),
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
        .observe(scroll_info_panel)
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
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_title,
                                ),
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

                    let rename_button = left
                        .spawn((
                            Button,
                            Node {
                                display: Display::None,
                                width: Val::Px(22.0),
                                height: Val::Px(22.0),
                                margin: UiRect::left(Val::Px(6.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(theme.colors.button_default),
                            SoulRenameButton,
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new("✎"),
                                TextFont {
                                    font: game_assets.font_ui().clone().into(),
                                    font_size: crate::theme::font_size_rem(
                                        theme.typography.font_size_xs,
                                    ),
                                    ..default()
                                },
                                TextColor(theme.colors.text_primary_semantic),
                            ));
                        })
                        .id();
                    info_panel_nodes.rename_button = Some(rename_button);
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
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_xs,
                                ),
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

        let rename_field_container = parent
            .spawn((
                Node {
                    display: Display::None,
                    width: Val::Percent(100.0),
                    margin: UiRect::bottom(Val::Px(5.0)),
                    ..default()
                },
                SoulRenameFieldContainer,
            ))
            .id();
        info_panel_nodes.rename_field_container = Some(rename_field_container);

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
                            font: game_assets.font_ui().clone().into(),
                            font_size: crate::theme::font_size_rem(
                                theme.typography.font_size_small,
                            ),
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
                        ImageNode::new(game_assets.icon_stress().clone()),
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
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_small,
                                ),
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
                        ImageNode::new(game_assets.icon_fatigue().clone()),
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
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_small,
                                ),
                                ..default()
                            },
                            UiSlot::StatFatigue,
                        ))
                        .id();
                    ui_nodes.set_slot(UiSlot::StatFatigue, fatigue);
                    info_panel_nodes.fatigue = Some(fatigue);
                });

                let dream = col
                    .spawn((
                        Text::new(""),
                        TextFont {
                            font: game_assets.font_ui().clone().into(),
                            font_size: crate::theme::font_size_rem(
                                theme.typography.font_size_small,
                            ),
                            ..default()
                        },
                        UiSlot::StatDream,
                    ))
                    .id();
                ui_nodes.set_slot(UiSlot::StatDream, dream);
                info_panel_nodes.dream = Some(dream);

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
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_small,
                                ),
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
                            font: game_assets.font_ui().clone().into(),
                            font_size: crate::theme::font_size_rem(
                                theme.typography.font_size_small,
                            ),
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

        let stockpile_group = parent
            .spawn(Node {
                display: Display::None,
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                ..default()
            })
            .with_children(|column| {
                spawn_info_section_divider(column, game_assets, theme, "Stockpile Policy");

                info_panel_nodes.stockpile_state = Some(
                    column
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_small,
                                ),
                                weight: FontWeight::SEMIBOLD,
                                ..default()
                            },
                            TextColor(theme.colors.text_primary_semantic),
                        ))
                        .id(),
                );
                info_panel_nodes.stockpile_current = Some(
                    column
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_small,
                                ),
                                ..default()
                            },
                            TextColor(theme.colors.text_primary_semantic),
                        ))
                        .id(),
                );

                spawn_info_section_divider(column, game_assets, theme, "Accepted Resources");

                info_panel_nodes.stockpile_acceptance_summary = Some(
                    column
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_xs,
                                ),
                                weight: FontWeight::SEMIBOLD,
                                ..default()
                            },
                            TextColor(theme.colors.text_secondary_semantic),
                        ))
                        .id(),
                );

                column
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|row| {
                        let (all_button, _) = spawn_stockpile_editor_button(
                            row,
                            game_assets,
                            theme,
                            Val::Percent(50.0),
                            "Allow All",
                        );
                        info_panel_nodes.stockpile_acceptance_all_button = Some(all_button);

                        let (none_button, _) = spawn_stockpile_editor_button(
                            row,
                            game_assets,
                            theme,
                            Val::Percent(50.0),
                            "Clear All",
                        );
                        info_panel_nodes.stockpile_acceptance_none_button = Some(none_button);
                    });

                column
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: Val::Px(4.0),
                        column_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|checklist| {
                        for resource_type in STOCKPILE_ACCEPTANCE_RESOURCES {
                            info_panel_nodes.stockpile_acceptance_rows.push(
                                spawn_stockpile_acceptance_row(
                                    checklist,
                                    game_assets,
                                    theme,
                                    resource_type,
                                ),
                            );
                        }
                    });

                column
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|row| {
                        let (decrease_button, _) = spawn_stockpile_editor_button(
                            row,
                            game_assets,
                            theme,
                            Val::Px(30.0),
                            "−",
                        );
                        info_panel_nodes.stockpile_target_decrease_button = Some(decrease_button);

                        info_panel_nodes.stockpile_target_text = Some(
                            row.spawn((
                                Text::new("Target"),
                                TextFont {
                                    font: game_assets.font_ui().clone().into(),
                                    font_size: crate::theme::font_size_rem(
                                        theme.typography.font_size_small,
                                    ),
                                    weight: FontWeight::SEMIBOLD,
                                    ..default()
                                },
                                TextColor(theme.colors.text_primary_semantic),
                                Node {
                                    flex_grow: 1.0,
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                            ))
                            .id(),
                        );

                        let (increase_button, _) = spawn_stockpile_editor_button(
                            row,
                            game_assets,
                            theme,
                            Val::Px(30.0),
                            "+",
                        );
                        info_panel_nodes.stockpile_target_increase_button = Some(increase_button);
                    });

                let (priority_button, priority_text) = spawn_stockpile_editor_button(
                    column,
                    game_assets,
                    theme,
                    Val::Percent(100.0),
                    "Inbound Priority",
                );
                info_panel_nodes.stockpile_priority_button = Some(priority_button);
                info_panel_nodes.stockpile_priority_text = Some(priority_text);

                let (export_button, export_text) = spawn_stockpile_editor_button(
                    column,
                    game_assets,
                    theme,
                    Val::Percent(100.0),
                    "Export",
                );
                info_panel_nodes.stockpile_export_button = Some(export_button);
                info_panel_nodes.stockpile_export_text = Some(export_text);

                spawn_info_section_divider(column, game_assets, theme, "Batch Edit");
                let (area_button, _) = spawn_stockpile_editor_button(
                    column,
                    game_assets,
                    theme,
                    Val::Percent(100.0),
                    "Apply Policy to Area",
                );
                info_panel_nodes.stockpile_area_button = Some(area_button);
            })
            .id();
        info_panel_nodes.stockpile_group = Some(stockpile_group);

        let soul_spa_group = parent
            .spawn(Node {
                display: Display::None,
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                ..default()
            })
            .with_children(|column| {
                spawn_info_section_divider(column, game_assets, theme, "Soul Energy");
                info_panel_nodes.soul_spa_status = Some(
                    column
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_small,
                                ),
                                weight: FontWeight::SEMIBOLD,
                                ..default()
                            },
                            TextColor(theme.colors.text_primary_semantic),
                        ))
                        .id(),
                );
                info_panel_nodes.soul_spa_output = Some(
                    column
                        .spawn((
                            Text::new(""),
                            TextFont {
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_small,
                                ),
                                ..default()
                            },
                            TextColor(theme.colors.text_primary_semantic),
                        ))
                        .id(),
                );

                info_panel_nodes.soul_spa_controls = Some(
                    column
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(5.0),
                            ..default()
                        })
                        .with_children(|row| {
                            info_panel_nodes.soul_spa_slots_decrease_button =
                                Some(spawn_soul_spa_slot_button(row, game_assets, theme, "−"));
                            info_panel_nodes.soul_spa_slots_text = Some(
                                row.spawn((
                                    Text::new("Active slots"),
                                    TextFont {
                                        font: game_assets.font_ui().clone().into(),
                                        font_size: crate::theme::font_size_rem(
                                            theme.typography.font_size_small,
                                        ),
                                        weight: FontWeight::SEMIBOLD,
                                        ..default()
                                    },
                                    TextColor(theme.colors.text_primary_semantic),
                                    Node {
                                        flex_grow: 1.0,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                ))
                                .id(),
                            );
                            info_panel_nodes.soul_spa_slots_increase_button =
                                Some(spawn_soul_spa_slot_button(row, game_assets, theme, "+"));
                        })
                        .id(),
                );
            })
            .id();
        info_panel_nodes.soul_spa_group = Some(soul_spa_group);

        let power_group = parent
            .spawn(Node {
                display: Display::None,
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                ..default()
            })
            .with_children(|column| {
                spawn_info_section_divider(column, game_assets, theme, "Power Grid");
                let text_bundle = || {
                    (
                        TextFont {
                            font: game_assets.font_ui().clone().into(),
                            font_size: crate::theme::font_size_rem(
                                theme.typography.font_size_small,
                            ),
                            ..default()
                        },
                        TextColor(theme.colors.text_primary_semantic),
                    )
                };
                info_panel_nodes.power_connection =
                    Some(column.spawn((Text::new(""), text_bundle())).id());
                info_panel_nodes.power_flow =
                    Some(column.spawn((Text::new(""), text_bundle())).id());
                info_panel_nodes.power_state =
                    Some(column.spawn((Text::new(""), text_bundle())).id());
                let (button, text) = spawn_power_priority_button(column, game_assets, theme);
                info_panel_nodes.power_priority_button = Some(button);
                info_panel_nodes.power_priority_text = Some(text);
            })
            .id();
        info_panel_nodes.power_group = Some(power_group);

        let common = parent
            .spawn((
                Text::new(""),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: crate::theme::font_size_rem(theme.typography.font_size_item),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::SoulRenameState;
    use crate::models::inspection::{
        EntityInspectionModel, EntityInspectionViewModel, PowerInspectionFields,
        SoulSpaInspectionFields, StockpileInspectionFields,
    };
    use crate::panels::info_panel::{InfoPanelPinState, InfoPanelState, info_panel_system};
    use crate::selection::SelectedEntity;
    use hw_logistics::transport_request::TransportPriority;
    use hw_logistics::{ResourceType, StockpileAcceptance, StockpilePolicyState};

    #[derive(Resource, Default)]
    struct TestAssets {
        font: Handle<Font>,
        image: Handle<Image>,
    }

    impl UiAssets for TestAssets {
        fn font_ui(&self) -> &Handle<Font> {
            &self.font
        }
        fn font_familiar(&self) -> &Handle<Font> {
            &self.font
        }
        fn font_soul_name(&self) -> &Handle<Font> {
            &self.font
        }
        fn icon_arrow_down(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_arrow_right(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_idle(&self) -> &Handle<Image> {
            &self.image
        }
        fn glow_circle(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_stress(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_fatigue(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_male(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_female(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_axe(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_pick(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_hammer(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_haul(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_bone_small(&self) -> &Handle<Image> {
            &self.image
        }
    }

    fn spawn_panel(
        mut commands: Commands,
        theme: Res<UiTheme>,
        mut ui_nodes: ResMut<UiNodeRegistry>,
        mut info_nodes: ResMut<InfoPanelNodes>,
    ) {
        let parent = commands.spawn(Node::default()).id();
        spawn_info_panel_ui(
            &mut commands,
            &TestAssets::default(),
            &theme,
            parent,
            &mut ui_nodes,
            &mut info_nodes,
        );
    }

    #[test]
    fn stockpile_acceptance_uses_static_resource_checklist_rows() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiTheme>()
            .init_resource::<UiNodeRegistry>()
            .init_resource::<InfoPanelNodes>()
            .add_systems(Startup, spawn_panel);

        app.update();

        let info_nodes = app.world().resource::<InfoPanelNodes>();
        assert!(info_nodes.stockpile_acceptance_summary.is_some());
        assert!(info_nodes.stockpile_acceptance_all_button.is_some());
        assert!(info_nodes.stockpile_acceptance_none_button.is_some());
        assert_eq!(
            info_nodes
                .stockpile_acceptance_rows
                .iter()
                .map(|row| row.resource_type)
                .collect::<Vec<_>>(),
            STOCKPILE_ACCEPTANCE_RESOURCES
        );
        for row in &info_nodes.stockpile_acceptance_rows {
            assert!(app.world().entity(row.button).contains::<Button>());
            assert!(app.world().entity(row.button).contains::<MenuButton>());
            assert!(app.world().entity(row.text).contains::<Text>());
        }

        let root = info_nodes.root.unwrap();
        let root_entity = app.world().entity(root);
        let root_node = root_entity.get::<Node>().unwrap();
        assert_eq!(root_node.max_height, Val::Vh(INFO_PANEL_MAX_HEIGHT_VH));
        assert_eq!(root_node.overflow.y, OverflowAxis::Scroll);
        assert!(root_entity.contains::<ScrollPosition>());

        let theme = app.world().resource::<UiTheme>();
        let viewport_height = 720.0;
        let max_supported_scale = 1.25;
        let panel_bottom = theme.spacing.panel_top * max_supported_scale
            + viewport_height * INFO_PANEL_MAX_HEIGHT_VH / 100.0;
        let bottom_bar_top =
            viewport_height - theme.spacing.bottom_bar_height * max_supported_scale;
        assert!(
            panel_bottom <= bottom_bar_top,
            "info panel must stay above the bottom bar at 1280x720 / UiScale 1.25"
        );
    }

    #[test]
    fn stockpile_checklist_row_binds_target_and_toggled_acceptance_patch() {
        let mut app = App::new();
        let stockpile = app.world_mut().spawn_empty().id();

        app.add_plugins(MinimalPlugins)
            .insert_resource(TestAssets::default())
            .init_resource::<UiTheme>()
            .init_resource::<UiNodeRegistry>()
            .init_resource::<InfoPanelNodes>()
            .init_resource::<SelectedEntity>()
            .init_resource::<InfoPanelPinState>()
            .init_resource::<InfoPanelState>()
            .init_resource::<SoulRenameState>()
            .insert_resource(EntityInspectionViewModel {
                model: Some(EntityInspectionModel {
                    entity: stockpile,
                    header: "Stockpile".to_string(),
                    common_text: String::new(),
                    tooltip_lines: Vec::new(),
                    soul: None,
                    stockpile: Some(StockpileInspectionFields {
                        state: StockpilePolicyState::Accepting,
                        current_amount: 0,
                        incoming_amount: 0,
                        capacity: 5,
                        current_resource: None,
                        acceptance: StockpileAcceptance::Only(ResourceType::Wood),
                        inbound_priority: TransportPriority::Normal,
                        target_amount: 5,
                        allow_export: true,
                    }),
                    soul_spa: None,
                    power: None,
                }),
            })
            .add_systems(Startup, spawn_panel)
            .add_systems(Update, info_panel_system::<TestAssets>);

        app.update();

        let (button, text) = {
            let nodes = app.world().resource::<InfoPanelNodes>();
            let row = nodes
                .stockpile_acceptance_rows
                .iter()
                .find(|row| row.resource_type == ResourceType::Rock)
                .unwrap();
            (row.button, row.text)
        };
        assert_eq!(app.world().get::<Text>(text).unwrap().0, "[ ] Rock");

        let action = app.world().get::<MenuButton>(button).unwrap().0;
        let MenuAction::ApplyStockpilePolicy { target, patch } = action else {
            panic!("expected stockpile policy action");
        };
        assert_eq!(target, StockpilePolicyEditTarget::Single(stockpile));

        let acceptance = patch.acceptance.expect("acceptance patch");
        assert!(acceptance.accepts(ResourceType::Wood));
        assert!(acceptance.accepts(ResourceType::Rock));
        assert_eq!(acceptance.allowed_count(), 2);
        assert_eq!(patch.inbound_priority, None);
        assert_eq!(patch.target_amount, None);
        assert_eq!(patch.allow_export, None);
    }

    #[test]
    fn soul_spa_editor_shows_draining_and_binds_exact_slot_intents() {
        let mut app = App::new();
        let soul_spa = app.world_mut().spawn_empty().id();

        app.add_plugins(MinimalPlugins)
            .insert_resource(TestAssets::default())
            .init_resource::<UiTheme>()
            .init_resource::<UiNodeRegistry>()
            .init_resource::<InfoPanelNodes>()
            .init_resource::<SelectedEntity>()
            .init_resource::<InfoPanelPinState>()
            .init_resource::<InfoPanelState>()
            .init_resource::<SoulRenameState>()
            .insert_resource(EntityInspectionViewModel {
                model: Some(EntityInspectionModel {
                    entity: soul_spa,
                    header: "Soul Spa".to_string(),
                    common_text: String::new(),
                    tooltip_lines: Vec::new(),
                    soul: None,
                    stockpile: None,
                    soul_spa: Some(SoulSpaInspectionFields {
                        operational: true,
                        bones_delivered: 10,
                        bones_required: 10,
                        occupied_slots: 4,
                        active_slots: 2,
                        max_active_slots: 4,
                        output_watts: 4.0,
                    }),
                    power: None,
                }),
            })
            .add_systems(Startup, spawn_panel)
            .add_systems(Update, info_panel_system::<TestAssets>);

        app.update();

        let (status, decrease, increase, controls) = {
            let nodes = app.world().resource::<InfoPanelNodes>();
            (
                nodes.soul_spa_status.unwrap(),
                nodes.soul_spa_slots_decrease_button.unwrap(),
                nodes.soul_spa_slots_increase_button.unwrap(),
                nodes.soul_spa_controls.unwrap(),
            )
        };
        assert_eq!(
            app.world().get::<Text>(status).unwrap().0,
            "Draining (4 active / 2 configured)"
        );
        assert_eq!(
            app.world().get::<Node>(controls).unwrap().display,
            Display::Flex
        );

        assert!(matches!(
            app.world().get::<MenuButton>(decrease).unwrap().0,
            MenuAction::SetSoulSpaActiveSlots {
                target,
                active_slots: 1,
            } if target == soul_spa
        ));
        assert!(matches!(
            app.world().get::<MenuButton>(increase).unwrap().0,
            MenuAction::SetSoulSpaActiveSlots {
                target,
                active_slots: 3,
            } if target == soul_spa
        ));

        {
            let mut view_model = app.world_mut().resource_mut::<EntityInspectionViewModel>();
            let fields = view_model
                .model
                .as_mut()
                .and_then(|model| model.soul_spa.as_mut())
                .unwrap();
            fields.operational = false;
            fields.bones_delivered = 7;
            fields.bones_required = 20;
            fields.output_watts = 0.0;
        }
        app.update();

        assert_eq!(
            app.world().get::<Text>(status).unwrap().0,
            "Status: Constructing (7/20 bones)"
        );
        let nodes = app.world().resource::<InfoPanelNodes>();
        assert_eq!(
            app.world()
                .get::<Node>(nodes.soul_spa_output.unwrap())
                .unwrap()
                .display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(controls).unwrap().display,
            Display::None
        );
    }

    #[test]
    fn power_consumer_section_explains_shed_reason_and_binds_next_priority() {
        use crate::power::{
            PowerAllocationModeValue, PowerInspectionRole, PowerPriorityValue,
            PowerShedReasonValue, PowerSupplyStateValue,
        };

        let mut app = App::new();
        let consumer = app.world_mut().spawn_empty().id();
        let grid = app.world_mut().spawn_empty().id();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TestAssets::default())
            .init_resource::<UiTheme>()
            .init_resource::<UiNodeRegistry>()
            .init_resource::<InfoPanelNodes>()
            .init_resource::<SelectedEntity>()
            .init_resource::<InfoPanelPinState>()
            .init_resource::<InfoPanelState>()
            .init_resource::<SoulRenameState>()
            .insert_resource(EntityInspectionViewModel {
                model: Some(EntityInspectionModel {
                    entity: consumer,
                    header: "Outdoor Lamp".to_string(),
                    common_text: String::new(),
                    tooltip_lines: Vec::new(),
                    soul: None,
                    stockpile: None,
                    soul_spa: None,
                    power: Some(PowerInspectionFields {
                        role: PowerInspectionRole::Consumer,
                        grid: Some(grid),
                        allocation_mode: Some(PowerAllocationModeValue::PriorityPrefix),
                        generation_watts: Some(1.0),
                        total_demand_watts: Some(1.5),
                        served_demand_watts: Some(1.0),
                        reserve_watts: Some(0.0),
                        deficit_watts: Some(0.5),
                        consumer_count: Some(2),
                        supplied_count: Some(1),
                        shed_count: Some(1),
                        invalid_count: Some(0),
                        shed_order_labels: vec!["(2, 1)".to_string()],
                        demand_watts: Some(0.5),
                        priority: Some(PowerPriorityValue::Normal),
                        supply_state: Some(PowerSupplyStateValue::Shed {
                            reason: PowerShedReasonValue::RestoreMargin,
                        }),
                    }),
                }),
            })
            .add_systems(Startup, spawn_panel)
            .add_systems(Update, info_panel_system::<TestAssets>);

        app.update();

        let nodes = app.world().resource::<InfoPanelNodes>();
        assert_eq!(
            app.world()
                .get::<Node>(nodes.power_group.unwrap())
                .unwrap()
                .display,
            Display::Flex
        );
        assert!(
            app.world()
                .get::<Text>(nodes.power_flow.unwrap())
                .unwrap()
                .0
                .contains("1.0W of 1.5W served [Priority prefix]")
        );
        let flow = &app
            .world()
            .get::<Text>(nodes.power_flow.unwrap())
            .unwrap()
            .0;
        assert!(flow.contains("0.5W deficit"));
        assert!(flow.contains("Consumers: 1/2 supplied, 1 shed, 0 invalid"));
        assert!(flow.contains("Shed order: (2, 1)"));
        assert!(
            app.world()
                .get::<Text>(nodes.power_state.unwrap())
                .unwrap()
                .0
                .contains("waiting for restore margin")
        );
        assert!(matches!(
            app.world()
                .get::<MenuButton>(nodes.power_priority_button.unwrap())
                .unwrap()
                .0,
            MenuAction::SetPowerConsumerPriority {
                target,
                priority: PowerPriorityValue::High,
            } if target == consumer
        ));
    }
}
