//! エンティティリスト UI (Familiar & Soul List)

use crate::interface::ui::components::*;
use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient};

pub fn spawn_entity_list_panel(
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
) {
    commands
        .spawn((
            Node {
                width: Val::Px(300.0),
                height: Val::Auto,
                max_height: Val::Percent(70.0),
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                top: Val::Px(120.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundGradient::from(LinearGradient {
                angle: 0.0, // 左から右
                stops: vec![
                    ColorStop::new(Color::srgba(0.1, 0.3, 0.5, 0.9), Val::Percent(0.0)), // 青っぽい
                    ColorStop::new(Color::srgba(0.0, 0.0, 0.0, 0.8), Val::Percent(100.0)),
                ],
                ..default()
            }),
            EntityListPanel,
        ))
        .with_children(|parent| {
            // パネルタイトル
            parent.spawn((
                Text::new("Entity List"),
                TextFont {
                    font: game_assets.font_ui.clone(),
                    font_size: crate::constants::FONT_SIZE_HEADER,
                    ..default()
                },
                TextColor(Color::srgb(0.0, 1.0, 1.0)),
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
                            BackgroundColor(Color::srgba(0.3, 0.3, 0.3, 0.5)),
                            SectionToggle(EntityListSectionType::Unassigned),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                ImageNode::new(game_assets.icon_arrow_down.clone()),
                                Node {
                                    width: Val::Px(12.0),
                                    height: Val::Px(12.0),
                                    margin: UiRect::right(Val::Px(4.0)),
                                    ..default()
                                },
                                UnassignedSectionArrowIcon,
                            ));
                            button.spawn((
                                Text::new("Unassigned Souls"),
                                TextFont {
                                    font: game_assets.font_ui.clone(),
                                    font_size: crate::constants::FONT_SIZE_SMALL,
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });

                    // 未所属ソウルリストコンテナ
                    section.spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        UnassignedSoulContent,
                    ));
                });
        });
}
