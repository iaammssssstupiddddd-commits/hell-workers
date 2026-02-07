//! エンティティリスト UI (Familiar & Soul List)

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
                height: Val::Auto,
                max_height: Val::Percent(theme.sizes.entity_list_max_height_percent),
                position_type: PositionType::Absolute,
                left: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.spacing.panel_top),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(theme.spacing.panel_padding)),
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
            RelativeCursorPosition::default(),
            UiInputBlocker,
            EntityListPanel,
        ))
        .id();
    commands.entity(parent_entity).add_child(panel);

    commands
        .entity(panel)
        .with_children(|parent| {
            // パネルタイトル
            parent.spawn((
                Text::new("Entity List"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_lg, // Semantic size
                    weight: FontWeight::BOLD, // Font Variation
                    ..default()
                },
                // Use panel accent color for the title
                TextColor(theme.colors.panel_accent_entity_list),
                Node {
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
            ));

            // 使い魔リストコンテナ (動的に中身を追加される)
            parent.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                FamiliarListContainer,
                Name::new("Familiar List Container"),
            ));

            // 未所属ソウルセクション
            parent
                .spawn((
                    Node {
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
                            BackgroundColor(theme.colors.interactive_default), // Semantic
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
                                    font_size: theme.typography.font_size_base, // Semantic
                                    ..default()
                                },
                                TextColor(theme.colors.text_primary_semantic), // Semantic
                            ));
                        });

                    // 未所属ソウルリストコンテナ
                    section.spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            max_height: Val::Px(220.0),
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
            parent.spawn((
                Text::new("Scroll: Mouse Wheel"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: theme.typography.font_size_xs, // Semantic
                    ..default()
                },
                TextColor(theme.colors.text_secondary_semantic), // Semantic
                Node {
                    align_self: AlignSelf::End,
                    margin: UiRect::top(Val::Px(8.0)),
                    ..default()
                },
                IgnoreScroll(BVec2::new(false, true)), // Bevy 0.18 Feature
                EntityListScrollHint,
            ));
        });
}
