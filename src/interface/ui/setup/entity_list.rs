//! 左パネル UI (Entity List / Task List のタブ切り替え)

use crate::interface::ui::components::*;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient, RelativeCursorPosition};

pub fn spawn_entity_list_panel(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(theme.sizes.entity_list_panel_width),
                min_width: Val::Px(theme.sizes.entity_list_min_width),
                max_width: Val::Px(theme.sizes.entity_list_max_width),
                height: Val::Px(420.0),
                min_height: Val::Px(220.0),
                max_height: Val::Percent(theme.sizes.entity_list_max_height_percent),
                position_type: PositionType::Absolute,
                left: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.spacing.panel_top),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme.spacing.panel_padding)),
                border: UiRect::all(Val::Px(theme.sizes.panel_border_width)),
                border_radius: BorderRadius::all(Val::Px(theme.sizes.panel_corner_radius)),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundGradient::from(LinearGradient {
                angle: 0.0,
                stops: vec![
                    ColorStop::new(theme.panels.entity_list.top, Val::Percent(0.0)),
                    ColorStop::new(theme.panels.entity_list.bottom, Val::Percent(100.0)),
                ],
                ..default()
            }),
            BorderColor::all(theme.colors.border_default),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            EntityListPanel,
        ))
        .id();
    commands.entity(parent_entity).add_child(panel);

    commands.entity(panel).with_children(|parent| {
        // ヘッダー行（タブバー + 最小化ボタン）
        parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(24.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(theme.colors.bg_elevated),
            ))
            .with_children(|header| {
                // タブバー
                spawn_left_panel_tab_bar(header, game_assets, theme);

                // 最小化ボタン
                header
                    .spawn((
                        Button,
                        Node {
                            width: Val::Px(theme.sizes.fold_button_size),
                            height: Val::Px(theme.sizes.fold_button_size),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(theme.colors.button_default),
                        EntityListMinimizeButton,
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("-"),
                            TextFont {
                                font: game_assets.font_ui.clone(),
                                font_size: theme.typography.font_size_base,
                                weight: FontWeight::BOLD,
                                ..default()
                            },
                            TextColor(theme.colors.text_primary_semantic),
                            EntityListMinimizeButtonLabel,
                        ));
                    });
            });

        // エンティティリスト ボディ（EntityList モード時に表示）
        parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    position_type: PositionType::Relative,
                    ..default()
                },
                BackgroundColor(theme.colors.bg_surface),
                EntityListBody,
            ))
            .with_children(|body| {
                // 使い魔リストコンテナ (動的に中身を追加される)
                body.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    FamiliarListContainer,
                    Name::new("Familiar List Container"),
                ));

                // 未所属ソウルセクション
                body.spawn((
                    Node {
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        margin: UiRect::top(Val::Px(10.0)),
                        ..default()
                    },
                    UnassignedSoulSection,
                ))
                .with_children(|section| {
                    // セクションヘッダー
                    section
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(24.0),
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(Val::Px(5.0)),
                                ..default()
                            },
                            BackgroundColor(theme.colors.button_default),
                            SectionToggle(EntityListSectionType::Unassigned),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                ImageNode::new(game_assets.icon_arrow_down.clone()),
                                Node {
                                    width: Val::Px(theme.sizes.fold_icon_size),
                                    height: Val::Px(theme.sizes.fold_icon_size),
                                    margin: UiRect::right(Val::Px(4.0)),
                                    ..default()
                                },
                                UnassignedSectionArrowIcon,
                            ));
                            button.spawn((
                                Text::new("Unassigned Souls"),
                                TextFont {
                                    font: game_assets.font_ui.clone(),
                                    font_size: theme.typography.font_size_base,
                                    ..default()
                                },
                                TextColor(theme.colors.text_primary_semantic),
                            ));
                        });

                    // 未所属ソウルリストコンテナ
                    section.spawn((
                        Node {
                            flex_grow: 1.0,
                            min_height: Val::Px(0.0),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        RelativeCursorPosition::default(),
                        UiInputBlocker,
                        UiScrollArea { speed: 28.0 },
                        UnassignedSoulContent,
                    ));
                });

                // スクロール可能であることを示す固定ヒント
                body.spawn((
                    Text::new("Scroll: Mouse Wheel"),
                    TextFont {
                        font: game_assets.font_ui.clone(),
                        font_size: theme.typography.font_size_xs,
                        ..default()
                    },
                    TextColor(theme.colors.text_secondary_semantic),
                    Node {
                        display: Display::None,
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                    IgnoreScroll(BVec2::new(false, true)),
                    EntityListScrollHint,
                ));
            });

        // タスクリスト ボディ（TaskList モード時に表示）
        parent.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip_y(),
                display: Display::None,
                ..default()
            },
            TaskListBody,
        ));
    });
}

fn spawn_left_panel_tab_bar(
    parent: &mut ChildSpawnerCommands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            spawn_left_panel_tab_button(
                row,
                game_assets,
                theme,
                "Entities",
                LeftPanelMode::EntityList,
                true,
            );
            spawn_left_panel_tab_button(
                row,
                game_assets,
                theme,
                "Tasks",
                LeftPanelMode::TaskList,
                false,
            );
        });
}

fn spawn_left_panel_tab_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
    label: &str,
    mode: LeftPanelMode,
    is_active: bool,
) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border: UiRect::bottom(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(if is_active {
                theme.colors.text_accent_semantic
            } else {
                Color::NONE
            }),
            LeftPanelTabButton(mode),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_sm,
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TextColor(if is_active {
                    theme.colors.text_accent_semantic
                } else {
                    theme.colors.text_secondary_semantic
                }),
            ));
        });
}
