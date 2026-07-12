//! 開発用デバッグパネル
//!
//! ロジック確認のための 3D 表示切り替えボタン・即時ビルドトグルを提供する。

use crate::assets::GameAssets;
use crate::systems::visual::terrain_lod::{LodLevel, TerrainLodMetrics, TerrainLodState};
use bevy::prelude::*;
use hw_core::quality::{QualitySettings, RttQualityPreset};
use hw_ui::components::{UiInputBlocker, UiMountSlot, UiNodeRegistry, UiSlot};
use hw_ui::theme::UiTheme;
use hw_ui::widgets::{TextFieldConfig, TextFieldRole, spawn_text_field};

/// LOD インジケーターテキストのマーカー
#[derive(Component)]
pub struct LodIndicatorText;

/// RtT / Soul mask / Light 状態表示テキストのマーカー
#[derive(Component)]
pub struct RenderPerfStatusText;

/// Soul mask トグルボタンのマーカー
#[derive(Component)]
pub struct ToggleSoulMaskButton;

/// RtT directional light トグルボタンのマーカー
#[derive(Component)]
pub struct ToggleRttLightButton;

/// 追加 RtT directional light トグルボタンのマーカー
#[derive(Component)]
pub struct ToggleRttExtraLightButton;

/// RtT terrain トグルボタンのマーカー
#[derive(Component)]
pub struct ToggleRttTerrainButton;

/// RtT scene object トグルボタンのマーカー
#[derive(Component)]
pub struct ToggleRttSceneObjectsButton;

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
    mut ui_nodes: ResMut<UiNodeRegistry>,
    settings: Res<hw_core::GameSettings>,
    game_assets: Res<GameAssets>,
    theme: Res<UiTheme>,
) {
    let Some((top_left, _)) = q_slots
        .iter()
        .find(|(_, slot)| **slot == UiMountSlot::TopLeft)
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
                        font_size: FontSize::Px(11.0),
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
                BackgroundColor(Color::srgb(0.12, 0.10, 0.18)),
                BorderColor::all(Color::srgb(0.28, 0.24, 0.42)),
                ToggleRttExtraLightButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("Light2: OFF"),
                    TextFont {
                        font_size: FontSize::Px(11.0),
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
                        font_size: FontSize::Px(11.0),
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
                BackgroundColor(Color::srgb(0.30, 0.24, 0.08)),
                BorderColor::all(Color::srgb(0.55, 0.45, 0.18)),
                ToggleSoulMaskButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("Mask: ON"),
                    TextFont {
                        font_size: FontSize::Px(11.0),
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
                BackgroundColor(Color::srgb(0.20, 0.22, 0.08)),
                BorderColor::all(Color::srgb(0.42, 0.48, 0.18)),
                ToggleRttLightButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("Light: ON"),
                    TextFont {
                        font_size: FontSize::Px(11.0),
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
                BackgroundColor(Color::srgb(0.12, 0.20, 0.14)),
                BorderColor::all(Color::srgb(0.24, 0.42, 0.30)),
                ToggleRttTerrainButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("Terrain: ON"),
                    TextFont {
                        font_size: FontSize::Px(11.0),
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
                BackgroundColor(Color::srgb(0.12, 0.16, 0.22)),
                BorderColor::all(Color::srgb(0.24, 0.34, 0.44)),
                ToggleRttSceneObjectsButton,
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new("Objs: ON"),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });

        // ── セパレーター ──
        parent.spawn((
            Node {
                height: Val::Px(1.0),
                margin: UiRect::axes(Val::Px(0.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.35, 0.35, 0.35, 0.6)),
        ));

        // ── FPS ──
        let fps_entity = parent
            .spawn((
                Text::new("FPS: --"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.85, 0.6)),
                UiSlot::FpsText,
                if settings.fps_display_enabled {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ))
            .id();
        ui_nodes.set_slot(UiSlot::FpsText, fps_entity);

        // ── LOD ──
        parent.spawn((
            Text::new("LOD:0 rtt:0.0px"),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgb(0.6, 0.6, 0.9)),
            LodIndicatorText,
        ));

        parent.spawn((
            Text::new("RTT:H Mask:ON Light:ON Light2:OFF Terrain:ON Objs:ON"),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgb(0.85, 0.75, 0.55)),
            RenderPerfStatusText,
        ));

        parent
            .spawn(Node {
                width: Val::Px(180.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_text_field(
                    row,
                    game_assets.as_ref(),
                    &theme,
                    TextFieldConfig {
                        initial_text: "",
                        role: TextFieldRole::DevPoc,
                        max_characters: Some(64),
                        select_all_on_focus: false,
                    },
                );
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

/// Soul mask ボタンのクリックを処理
pub fn toggle_soul_mask_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleSoulMaskButton>)>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            perf_toggles.soul_mask_enabled = !perf_toggles.soul_mask_enabled;
        }
    }
}

/// RtT light ボタンのクリックを処理
pub fn toggle_rtt_light_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleRttLightButton>)>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            perf_toggles.directional_light_enabled = !perf_toggles.directional_light_enabled;
        }
    }
}

/// 追加 RtT light ボタンのクリックを処理
pub fn toggle_rtt_extra_light_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleRttExtraLightButton>)>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            perf_toggles.extra_directional_light_enabled =
                !perf_toggles.extra_directional_light_enabled;
        }
    }
}

/// RtT terrain ボタンのクリックを処理
pub fn toggle_rtt_terrain_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleRttTerrainButton>)>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            perf_toggles.terrain_enabled = !perf_toggles.terrain_enabled;
        }
    }
}

/// RtT scene object ボタンのクリックを処理
pub fn toggle_rtt_scene_objects_button_system(
    q_button: Query<&Interaction, (Changed<Interaction>, With<ToggleRttSceneObjectsButton>)>,
    mut perf_toggles: ResMut<crate::RenderPerfToggles>,
) {
    for interaction in q_button.iter() {
        if *interaction == Interaction::Pressed {
            perf_toggles.scene_objects_enabled = !perf_toggles.scene_objects_enabled;
        }
    }
}

/// LOD インジケーターテキストを毎フレーム更新する
pub fn update_lod_indicator_system(
    metrics: Res<TerrainLodMetrics>,
    state: Res<TerrainLodState>,
    mut q_text: Query<&mut Text, With<LodIndicatorText>>,
) {
    let level = match state.level {
        LodLevel::Lod0 => "0",
        LodLevel::Lod1 => "1",
        LodLevel::Lod1Lite => "1L",
        LodLevel::Lod2 => "2",
    };
    let new_text = format!("LOD:{} rtt:{:.1}px", level, metrics.tile_rtt_px);
    for mut text in q_text.iter_mut() {
        text.0 = new_text.clone();
    }
}

/// RtT 品質と固定費トグルの状態を DevPanel に表示する。
pub fn update_render_perf_status_system(
    quality: Res<QualitySettings>,
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_text: Query<&mut Text, With<RenderPerfStatusText>>,
) {
    if !quality.is_changed() && !perf_toggles.is_changed() {
        return;
    }

    let rtt = match quality.rtt {
        RttQualityPreset::High => "H",
        RttQualityPreset::Medium => "M",
        RttQualityPreset::Low => "L",
    };
    let mask = if perf_toggles.soul_mask_enabled {
        "ON"
    } else {
        "OFF"
    };
    let light = if perf_toggles.directional_light_enabled {
        "ON"
    } else {
        "OFF"
    };
    let light2 = if perf_toggles.extra_directional_light_enabled {
        "ON"
    } else {
        "OFF"
    };
    let terrain = if perf_toggles.terrain_enabled {
        "ON"
    } else {
        "OFF"
    };
    let scene_objects = if perf_toggles.scene_objects_enabled {
        "ON"
    } else {
        "OFF"
    };
    let text = format!(
        "RTT:{rtt} Mask:{mask} Light:{light} Light2:{light2} Terrain:{terrain} Objs:{scene_objects}"
    );

    for mut label in q_text.iter_mut() {
        label.0 = text.clone();
    }
}

/// Soul mask ボタンのラベルと色を同期する。
pub fn update_soul_mask_button_visual_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleSoulMaskButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for (children, mut bg, mut border) in q_button.iter_mut() {
        if perf_toggles.soul_mask_enabled {
            *bg = BackgroundColor(Color::srgb(0.30, 0.24, 0.08));
            *border = BorderColor::all(Color::srgb(0.55, 0.45, 0.18));
        } else {
            *bg = BackgroundColor(Color::srgb(0.18, 0.12, 0.08));
            *border = BorderColor::all(Color::srgb(0.40, 0.26, 0.18));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if perf_toggles.soul_mask_enabled {
                    "Mask: ON".to_string()
                } else {
                    "Mask: OFF".to_string()
                };
            }
        }
    }
}

/// RtT light ボタンのラベルと色を同期する。
pub fn update_rtt_light_button_visual_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleRttLightButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for (children, mut bg, mut border) in q_button.iter_mut() {
        if perf_toggles.directional_light_enabled {
            *bg = BackgroundColor(Color::srgb(0.20, 0.22, 0.08));
            *border = BorderColor::all(Color::srgb(0.42, 0.48, 0.18));
        } else {
            *bg = BackgroundColor(Color::srgb(0.12, 0.12, 0.08));
            *border = BorderColor::all(Color::srgb(0.28, 0.28, 0.18));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if perf_toggles.directional_light_enabled {
                    "Light: ON".to_string()
                } else {
                    "Light: OFF".to_string()
                };
            }
        }
    }
}

/// 追加 RtT light ボタンのラベルと色を同期する。
pub fn update_rtt_extra_light_button_visual_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleRttExtraLightButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for (children, mut bg, mut border) in q_button.iter_mut() {
        if perf_toggles.extra_directional_light_enabled {
            *bg = BackgroundColor(Color::srgb(0.20, 0.14, 0.30));
            *border = BorderColor::all(Color::srgb(0.42, 0.32, 0.56));
        } else {
            *bg = BackgroundColor(Color::srgb(0.12, 0.10, 0.18));
            *border = BorderColor::all(Color::srgb(0.28, 0.24, 0.42));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if perf_toggles.extra_directional_light_enabled {
                    "Light2: ON".to_string()
                } else {
                    "Light2: OFF".to_string()
                };
            }
        }
    }
}

/// RtT terrain ボタンのラベルと色を同期する。
pub fn update_rtt_terrain_button_visual_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleRttTerrainButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for (children, mut bg, mut border) in q_button.iter_mut() {
        if perf_toggles.terrain_enabled {
            *bg = BackgroundColor(Color::srgb(0.12, 0.20, 0.14));
            *border = BorderColor::all(Color::srgb(0.24, 0.42, 0.30));
        } else {
            *bg = BackgroundColor(Color::srgb(0.10, 0.12, 0.10));
            *border = BorderColor::all(Color::srgb(0.22, 0.26, 0.22));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if perf_toggles.terrain_enabled {
                    "Terrain: ON".to_string()
                } else {
                    "Terrain: OFF".to_string()
                };
            }
        }
    }
}

/// RtT scene object ボタンのラベルと色を同期する。
pub fn update_rtt_scene_objects_button_visual_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    mut q_button: Query<
        (&Children, &mut BackgroundColor, &mut BorderColor),
        With<ToggleRttSceneObjectsButton>,
    >,
    mut q_text: Query<&mut Text>,
) {
    if !perf_toggles.is_changed() {
        return;
    }

    for (children, mut bg, mut border) in q_button.iter_mut() {
        if perf_toggles.scene_objects_enabled {
            *bg = BackgroundColor(Color::srgb(0.12, 0.16, 0.22));
            *border = BorderColor::all(Color::srgb(0.24, 0.34, 0.44));
        } else {
            *bg = BackgroundColor(Color::srgb(0.10, 0.10, 0.14));
            *border = BorderColor::all(Color::srgb(0.22, 0.22, 0.30));
        }
        for child in children.iter() {
            if let Ok(mut text) = q_text.get_mut(child) {
                text.0 = if perf_toggles.scene_objects_enabled {
                    "Objs: ON".to_string()
                } else {
                    "Objs: OFF".to_string()
                };
            }
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
