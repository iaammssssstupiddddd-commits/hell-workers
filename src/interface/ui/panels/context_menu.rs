//! コンテキストメニュー管理

use crate::interface::ui::components::*;
use bevy::prelude::*;

pub fn familiar_context_menu_system(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<crate::interface::camera::MainCamera>>,
    q_familiars: Query<&GlobalTransform, With<crate::entities::familiar::Familiar>>,
    q_context_menu: Query<Entity, With<ContextMenu>>,
    q_ui_interaction: Query<&Interaction, With<Button>>,
) {
    if buttons.just_pressed(MouseButton::Left) {
        if q_ui_interaction.iter().any(|i| *i != Interaction::None) {
            return;
        }

        for entity in q_context_menu.iter() {
            commands.entity(entity).despawn();
        }

        let Ok((camera, camera_transform)) = q_camera.single() else {
            return;
        };
        let Ok(window) = q_window.single() else {
            return;
        };

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                let mut clicked_familiar = false;
                for transform in q_familiars.iter() {
                    let pos = transform.translation().truncate();
                    if pos.distance(world_pos) < crate::constants::TILE_SIZE / 2.0 {
                        clicked_familiar = true;
                        break;
                    }
                }

                if clicked_familiar {
                    commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(cursor_pos.x),
                                top: Val::Px(cursor_pos.y),
                                width: Val::Px(100.0),
                                height: Val::Auto,
                                flex_direction: FlexDirection::Column,
                                padding: UiRect::all(Val::Px(5.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 0.9)),
                            ContextMenu,
                        ))
                        .with_children(|parent| {
                            parent
                                .spawn((
                                    Button,
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(30.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                                    MenuButton(MenuAction::SelectAreaTask),
                                ))
                                .with_children(|button| {
                                    button.spawn((
                                        Text::new("Task"),
                                        TextFont {
                                            font_size: 14.0,
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                });
                            parent
                                .spawn((
                                    Button,
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(30.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        margin: UiRect::top(Val::Px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                                    MenuButton(MenuAction::OpenOperationDialog),
                                ))
                                .with_children(|button| {
                                    button.spawn((
                                        Text::new("Operation"),
                                        TextFont {
                                            font_size: 14.0,
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                });
                        });
                }
            }
        }
    }
}
