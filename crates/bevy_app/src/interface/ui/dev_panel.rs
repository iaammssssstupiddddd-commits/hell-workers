//! 開発用デバッグパネル
//!
//! ロジック確認のための 3D 表示切り替えボタン・即時ビルドトグルを提供する。

use bevy::prelude::*;
use hw_ui::components::{UiInputBlocker, UiMountSlot};

/// 3D表示切り替えボタンのマーカー
#[derive(Component)]
pub struct ToggleRender3dButton;

/// 即時ビルドトグルボタンのマーカー
#[derive(Component)]
pub struct InstantBuildButton;

/// 開発用パネルをスポーン（TopLeft スロットに配置）
pub fn spawn_dev_panel_system(
    mut commands: Commands,
    q_slots: Query<(Entity, &UiMountSlot)>,
) {
    let Some((top_left, _)) = q_slots.iter().find(|(_, slot)| **slot == UiMountSlot::TopLeft)
    else {
        return;
    };

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                top: Val::Px(8.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.08, 0.08, 0.80)),
            BorderColor::all(Color::srgba(0.35, 0.35, 0.35, 0.80)),
            UiInputBlocker,
            Name::new("DevPanel"),
        ))
        .id();
    commands.entity(top_left).add_child(panel);

    commands.entity(panel).with_children(|parent| {
        parent
            .spawn((
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.15, 0.35, 0.15)),
                BorderColor::all(Color::srgb(0.35, 0.55, 0.35)),
                ToggleRender3dButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("3D: ON"),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });

        parent
            .spawn((
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.25, 0.25, 0.25)),
                BorderColor::all(Color::srgb(0.45, 0.45, 0.45)),
                InstantBuildButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("IBuild: OFF"),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
    });
}

/// 3D表示ボタンのクリックを処理
pub fn toggle_render3d_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleRender3dButton>)>,
    mut render3d: ResMut<crate::Render3dVisible>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            render3d.0 = !render3d.0;
        }
    }
}

/// 3D表示ボタンのラベルと色を Render3dVisible に合わせて更新
pub fn update_render3d_button_visual_system(
    render3d: Res<crate::Render3dVisible>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleRender3dButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !render3d.is_changed() {
        return;
    }
    for (children, mut bg, mut border) in q_button.iter_mut() {
        if render3d.0 {
            *bg = BackgroundColor(Color::srgb(0.15, 0.35, 0.15));
            *border = BorderColor::all(Color::srgb(0.35, 0.55, 0.35));
        } else {
            *bg = BackgroundColor(Color::srgb(0.35, 0.15, 0.15));
            *border = BorderColor::all(Color::srgb(0.55, 0.35, 0.35));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if render3d.0 {
                    "3D: ON".to_string()
                } else {
                    "3D: OFF".to_string()
                };
            }
        }
    }
}

/// 即時ビルドボタンのクリックを処理
pub fn toggle_instant_build_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<InstantBuildButton>)>,
    mut instant_build: ResMut<crate::DebugInstantBuild>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            instant_build.0 = !instant_build.0;
        }
    }
}

/// 即時ビルドボタンのラベルと色を DebugInstantBuild に合わせて更新
pub fn update_instant_build_button_visual_system(
    instant_build: Res<crate::DebugInstantBuild>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<InstantBuildButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !instant_build.is_changed() {
        return;
    }
    for (children, mut bg, mut border) in q_button.iter_mut() {
        if instant_build.0 {
            *bg = BackgroundColor(Color::srgb(0.35, 0.20, 0.05));
            *border = BorderColor::all(Color::srgb(0.60, 0.35, 0.10));
        } else {
            *bg = BackgroundColor(Color::srgb(0.25, 0.25, 0.25));
            *border = BorderColor::all(Color::srgb(0.45, 0.45, 0.45));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if instant_build.0 {
                    "IBuild: ON".to_string()
                } else {
                    "IBuild: OFF".to_string()
                };
            }
        }
    }
}
