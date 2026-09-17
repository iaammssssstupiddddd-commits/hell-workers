//! 左パネル UI (Entity List / Task List のタブ切り替え)

use super::UiAssets;
use crate::components::*;
use crate::theme::UiTheme;
use crate::widgets::{TextFieldConfig, TextFieldRole, spawn_text_field};
use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient, RelativeCursorPosition};
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};

pub fn spawn_entity_list_panel(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
) {
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(theme.sizes.entity_list_panel_width),
                min_width: Val::Px(theme.sizes.entity_list_min_width),
                max_width: Val::Px(theme.sizes.entity_list_max_width),
                height: Val::Px(crate::list::resize::ENTITY_LIST_DEFAULT_HEIGHT),
                min_height: Val::Px(220.0),
                max_height: Val::Percent(theme.sizes.entity_list_max_height_percent),
                position_type: PositionType::Absolute,
                right: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.spacing.panel_top),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme.spacing.panel_padding)),
                border: UiRect::all(Val::Px(theme.sizes.panel_border_width)),
                border_radius: BorderRadius::all(Val::Px(theme.sizes.panel_corner_radius)),
                overflow: Overflow::clip_y(),
                display: Display::None,
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
        // Navigation and tabs share one row so the roster keeps its visible capacity.
        parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(24.0),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(theme.colors.bg_elevated),
            ))
            .with_children(|header| {
                crate::shell::workspace_button(
                    header,
                    game_assets,
                    theme,
                    "戻る",
                    crate::shell::WorkspaceAction::Back,
                );
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
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_base,
                                ),
                                weight: FontWeight::BOLD,
                                ..default()
                            },
                            TextColor(theme.colors.text_primary_semantic),
                            EntityListMinimizeButtonLabel,
                        ));
                    });
                crate::shell::workspace_button(
                    header,
                    game_assets,
                    theme,
                    "閉じる",
                    crate::shell::WorkspaceAction::Close,
                );
            });

        parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    flex_shrink: 0.0,
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
                EntityListSearchRow,
            ))
            .with_children(|row| {
                row.spawn((
                    Text::new("検索"),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: crate::theme::font_size_rem(theme.typography.font_size_xs),
                        ..default()
                    },
                    TextColor(theme.colors.text_secondary_semantic),
                    Node {
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
                spawn_text_field(
                    row,
                    game_assets,
                    theme,
                    TextFieldConfig {
                        initial_text: "",
                        role: TextFieldRole::EntityListSearch,
                        max_characters: Some(64),
                        select_all_on_focus: false,
                    },
                );
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
                body.spawn(Node {
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    ..default()
                })
                .with_children(|row| {
                    let scroll = row
                        .spawn((
                            Node {
                                flex_grow: 1.0,
                                min_width: Val::Px(0.0),
                                min_height: Val::Px(0.0),
                                flex_direction: FlexDirection::Column,
                                overflow: Overflow::scroll_y(),
                                padding: UiRect::right(Val::Px(4.0)),
                                ..default()
                            },
                            ScrollArea,
                            EntityListScrollArea,
                            UiInputBlocker,
                            RelativeCursorPosition::default(),
                            Name::new("Entities Scroll Area"),
                        ))
                        .with_children(|body| {
                            // 使い魔リストコンテナ (動的に中身を追加される)
                            body.spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                FamiliarListContainer,
                                Name::new("Familiar List Container"),
                            ));

                            // 未所属ソウルセクション
                            body.spawn((
                                Node {
                                    flex_shrink: 0.0,
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
                                            ImageNode::new(game_assets.icon_arrow_down().clone()),
                                            Node {
                                                width: Val::Px(theme.sizes.fold_icon_size),
                                                height: Val::Px(theme.sizes.fold_icon_size),
                                                margin: UiRect::right(Val::Px(4.0)),
                                                ..default()
                                            },
                                            UnassignedSectionArrowIcon,
                                        ));
                                        button.spawn((
                                            Text::new("未所属の魂"),
                                            TextFont {
                                                font: game_assets.font_ui().clone().into(),
                                                font_size: crate::theme::font_size_rem(
                                                    theme.typography.font_size_base,
                                                ),
                                                ..default()
                                            },
                                            TextColor(theme.colors.text_primary_semantic),
                                        ));
                                    });

                                section.spawn((
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                    UnassignedSoulContent,
                                ));
                            });
                        })
                        .id();
                    row.spawn((
                        Node {
                            width: Val::Px(6.0),
                            ..default()
                        },
                        Scrollbar::new(scroll, ControlOrientation::Vertical, 20.0),
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

                // スクロール可能であることを示す固定ヒント
                body.spawn((
                    Text::new("ホイールでスクロール"),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: crate::theme::font_size_rem(theme.typography.font_size_xs),
                        ..default()
                    },
                    TextColor(theme.colors.text_secondary_semantic),
                    Node {
                        display: Display::None,
                        flex_shrink: 0.0,
                        align_self: AlignSelf::End,
                        margin: UiRect::top(Val::Px(4.0)),
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
    game_assets: &dyn UiAssets,
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
                "使い魔・魂",
                LeftPanelMode::EntityList,
                true,
            );
            spawn_left_panel_tab_button(
                row,
                game_assets,
                theme,
                "仕事",
                LeftPanelMode::TaskList,
                false,
            );
        });
}

fn spawn_left_panel_tab_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
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
                    font: game_assets.font_ui().clone().into(),
                    font_size: crate::theme::font_size_rem(theme.typography.font_size_sm),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::test_support::TestAssets;

    fn fixture(mut commands: Commands, theme: Res<UiTheme>) {
        let root = commands.spawn(Node::default()).id();
        spawn_entity_list_panel(&mut commands, &TestAssets::default(), &theme, root);
    }

    #[test]
    fn all_groups_share_one_scroll_area_and_search_stays_outside() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiTheme>()
            .add_systems(Startup, fixture);
        app.update();
        let scroll = app
            .world_mut()
            .query_filtered::<Entity, With<ScrollArea>>()
            .single(app.world())
            .expect("one scroll area, no nested unassigned scroller");
        assert!(app.world().get::<EntityListScrollArea>(scroll).is_some());
        let groups: Vec<_> = app.world_mut().query_filtered::<Entity,
            Or<(With<FamiliarListContainer>, With<UnassignedSoulSection>)>>()
            .iter(app.world()).collect();
        assert_eq!(groups.len(), 2);
        for group in groups {
            assert_eq!(app.world().get::<ChildOf>(group).unwrap().parent(), scroll);
        }
        let search = app
            .world_mut()
            .query_filtered::<&ChildOf, With<EntityListSearchRow>>()
            .single(app.world())
            .unwrap();
        assert!(
            app.world()
                .get::<EntityListPanel>(search.parent())
                .is_some()
        );
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
