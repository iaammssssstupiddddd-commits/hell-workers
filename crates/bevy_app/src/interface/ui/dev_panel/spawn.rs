use super::*;

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

    let minimize_button = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_self: AlignSelf::FlexEnd,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.18, 0.18, 0.18)),
            BorderColor::all(Color::srgb(0.42, 0.42, 0.42)),
            DevPanelMinimizeButton,
            Name::new("DevPanelMinimizeButton"),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("-"),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    weight: FontWeight::BOLD,
                    ..default()
                },
                TextColor(Color::WHITE),
                DevPanelMinimizeButtonLabel,
            ));
        })
        .id();

    let body = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            DevPanelBody,
            Name::new("DevPanelBody"),
        ))
        .id();
    commands
        .entity(panel)
        .add_children(&[minimize_button, body]);

    commands.entity(body).with_children(|parent| {
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
