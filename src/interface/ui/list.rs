//! エンティティリストの動的更新システム

use crate::entities::damned_soul::{DamnedSoul, Gender, SoulIdentity};
use crate::entities::familiar::{Familiar, FamiliarOperation, UnderCommand};
use crate::interface::ui::components::*;
use crate::relationships::Commanding;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::jobs::WorkType;
use crate::systems::work::AssignedTask;
use bevy::prelude::*;

/// シンプルな実装: 定期的にUIをクリアして再構築する
pub fn rebuild_entity_list_system(
    mut commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    q_familiars: Query<(
        Entity,
        &Familiar,
        &FamiliarOperation,
        &FamiliarAiState,
        Option<&Commanding>,
    )>,
    q_all_souls: Query<
        (
            Entity,
            &DamnedSoul,
            &AssignedTask,
            &SoulIdentity,
            Option<&UnderCommand>,
        ),
        Without<Familiar>,
    >,
    q_fam_container: Query<Entity, With<FamiliarListContainer>>,
    q_unassigned_container: Query<Entity, With<UnassignedSoulContent>>,
    q_children: Query<&Children>,
    fold_state: Res<EntityListFoldState>,
) {
    // Note: Update frequency is controlled by on_timer in interface.rs

    let fam_container_entity = if let Some(e) = q_fam_container.iter().next() {
        e
    } else {
        return;
    };
    let unassigned_content_entity = if let Some(e) = q_unassigned_container.iter().next() {
        e
    } else {
        return;
    };

    // コンテナの中身をクリア
    if let Ok(children) = q_children.get(fam_container_entity) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    if let Ok(children) = q_children.get(unassigned_content_entity) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    // 使い魔リスト構築
    for (fam_entity, familiar, op, ai_state, commanding_opt) in q_familiars.iter() {
        let is_folded = fold_state.folded_familiars.contains(&fam_entity);
        let fold_icon = if is_folded {
            game_assets.icon_arrow_right.clone()
        } else {
            game_assets.icon_arrow_down.clone()
        };
        let squad_count = commanding_opt.map(|c| c.len()).unwrap_or(0);

        let state_str = match ai_state {
            FamiliarAiState::Idle => "Idle",
            FamiliarAiState::SearchingTask => "Searching",
            FamiliarAiState::Scouting { .. } => "Scouting",
            FamiliarAiState::Supervising { .. } => "Supervising",
        };

        commands
            .entity(fam_container_entity)
            .with_children(|parent| {
                parent
                    .spawn((Node {
                        flex_direction: FlexDirection::Column,
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },))
                    .with_children(|header_node| {
                        // ヘッダー行（アイコンと名前を横に並べる）
                        header_node
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(24.0),
                                align_items: AlignItems::Center,
                                flex_direction: FlexDirection::Row,
                                ..default()
                            })
                            .with_children(|row| {
                                // 折りたたみアイコンボタン
                                row.spawn((
                                    Button,
                                    Node {
                                        width: Val::Px(20.0),
                                        height: Val::Px(20.0),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.3, 0.3, 0.5, 0.6)),
                                    SectionToggle(EntityListSectionType::Familiar(fam_entity)),
                                ))
                                .with_children(|icon_btn| {
                                    icon_btn.spawn((
                                        ImageNode {
                                            image: fold_icon,
                                            ..default()
                                        },
                                        Node {
                                            width: Val::Px(12.0),
                                            height: Val::Px(12.0),
                                            ..default()
                                        },
                                    ));
                                });

                                // 名前ボタン（選択用）
                                row.spawn((
                                    Button,
                                    Node {
                                        flex_grow: 1.0,
                                        height: Val::Px(24.0),
                                        align_items: AlignItems::Center,
                                        padding: UiRect::left(Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.2, 0.2, 0.4, 0.6)),
                                    FamiliarListItem(fam_entity),
                                ))
                                .with_children(|name_btn| {
                                    name_btn.spawn((
                                        Text::new(format!(
                                            "{} ({}/{}) [{}]",
                                            familiar.name,
                                            squad_count,
                                            op.max_controlled_soul,
                                            state_str
                                        )),
                                        TextFont {
                                            font_size: 14.0,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.8, 0.8, 1.0)),
                                    ));
                                });
                            });

                        // 分隊メンバー
                        if !is_folded {
                            if let Some(commanding) = commanding_opt {
                                if commanding.is_empty() {
                                    header_node.spawn((
                                        Text::new("  (empty)"),
                                        TextFont {
                                            font_size: 12.0,
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                                        Node {
                                            margin: UiRect::left(Val::Px(15.0)),
                                            ..default()
                                        },
                                    ));
                                } else {
                                    for &soul_entity in commanding.iter() {
                                        if let Ok((_, soul, task, identity, _)) =
                                            q_all_souls.get(soul_entity)
                                        {
                                            let (gender_handle, gender_color) =
                                                match identity.gender {
                                                    Gender::Male => (
                                                        game_assets.icon_male.clone(),
                                                        Color::srgb(0.4, 0.7, 1.0),
                                                    ),
                                                    Gender::Female => (
                                                        game_assets.icon_female.clone(),
                                                        Color::srgb(1.0, 0.5, 0.7),
                                                    ),
                                                };
                                            let (task_handle, task_color) = match task {
                                                AssignedTask::None => (
                                                    game_assets.icon_idle.clone(),
                                                    Color::srgb(0.6, 0.6, 0.6),
                                                ),
                                                AssignedTask::Gather { work_type, .. } => {
                                                    match work_type {
                                                        WorkType::Chop => (
                                                            game_assets.icon_axe.clone(),
                                                            Color::srgb(0.6, 0.4, 0.2), // 茶色
                                                        ),
                                                        WorkType::Mine => (
                                                            game_assets.icon_pick.clone(),
                                                            Color::srgb(0.7, 0.7, 0.7), // 灰色
                                                        ),
                                                        _ => (
                                                            game_assets.icon_pick.clone(),
                                                            Color::srgb(1.0, 0.7, 0.3), // デフォルト
                                                        ),
                                                    }
                                                }
                                                AssignedTask::Haul { .. } => (
                                                    game_assets.icon_haul.clone(),
                                                    Color::srgb(0.5, 1.0, 0.5),
                                                ),
                                            };
                                            let stress_color = if soul.stress > 0.8 {
                                                Color::srgb(1.0, 0.0, 0.0)
                                            } else if soul.stress > 0.5 {
                                                Color::srgb(1.0, 0.5, 0.0)
                                            } else {
                                                Color::WHITE
                                            };

                                            header_node
                                                .spawn((
                                                    Button,
                                                    Node {
                                                        width: Val::Percent(100.0),
                                                        height: Val::Px(20.0),
                                                        align_items: AlignItems::Center,
                                                        margin: UiRect::left(Val::Px(15.0)),
                                                        ..default()
                                                    },
                                                    BackgroundColor(Color::NONE),
                                                    SoulListItem(soul_entity),
                                                ))
                                                .with_children(|item| {
                                                    // Gender Icon
                                                    item.spawn((
                                                        ImageNode {
                                                            image: gender_handle,
                                                            color: gender_color,
                                                            ..default()
                                                        },
                                                        Node {
                                                            width: Val::Px(16.0),
                                                            height: Val::Px(16.0),
                                                            margin: UiRect::right(Val::Px(4.0)),
                                                            ..default()
                                                        },
                                                    ));
                                                    // Name
                                                    item.spawn((
                                                        Text::new(identity.name.clone()),
                                                        TextFont {
                                                            font_size: 12.0,
                                                            ..default()
                                                        },
                                                        TextColor(stress_color),
                                                        Node {
                                                            margin: UiRect::right(Val::Px(6.0)),
                                                            ..default()
                                                        },
                                                    ));
                                                    // Fatigue icon & %
                                                    item.spawn((
                                                        ImageNode {
                                                            image: game_assets.icon_fatigue.clone(),
                                                            color: Color::srgb(0.6, 0.6, 1.0),
                                                            ..default()
                                                        },
                                                        Node {
                                                            width: Val::Px(16.0),
                                                            height: Val::Px(16.0),
                                                            margin: UiRect::right(Val::Px(2.0)),
                                                            ..default()
                                                        },
                                                    ));
                                                    item.spawn((
                                                        Text::new(format!(
                                                            "{:.0}%",
                                                            soul.fatigue * 100.0
                                                        )),
                                                        TextFont {
                                                            font_size: 10.0,
                                                            ..default()
                                                        },
                                                        TextColor(Color::srgb(0.7, 0.7, 1.0)),
                                                        Node {
                                                            margin: UiRect::right(Val::Px(6.0)),
                                                            ..default()
                                                        },
                                                    ));
                                                    // Stress icon & %
                                                    item.spawn((
                                                        ImageNode {
                                                            image: game_assets.icon_stress.clone(),
                                                            color: Color::srgb(1.0, 0.9, 0.2),
                                                            ..default()
                                                        },
                                                        Node {
                                                            width: Val::Px(16.0),
                                                            height: Val::Px(16.0),
                                                            margin: UiRect::right(Val::Px(2.0)),
                                                            ..default()
                                                        },
                                                    ));
                                                    item.spawn((
                                                        Text::new(format!(
                                                            "{:.0}%",
                                                            soul.stress * 100.0
                                                        )),
                                                        TextFont {
                                                            font_size: 10.0,
                                                            ..default()
                                                        },
                                                        TextColor(stress_color),
                                                        Node {
                                                            margin: UiRect::right(Val::Px(6.0)),
                                                            ..default()
                                                        },
                                                    ));
                                                    // Task icon
                                                    item.spawn((
                                                        ImageNode {
                                                            image: task_handle,
                                                            color: task_color,
                                                            ..default()
                                                        },
                                                        Node {
                                                            width: Val::Px(16.0),
                                                            height: Val::Px(16.0),
                                                            ..default()
                                                        },
                                                    ));
                                                });
                                        }
                                    }
                                }
                            }
                        }
                    });
            });
    }

    // 未所属ソウル
    if !fold_state.unassigned_folded {
        for (soul_entity, soul, task, identity, under_command) in q_all_souls.iter() {
            if under_command.is_none() {
                commands
                    .entity(unassigned_content_entity)
                    .with_children(|parent| {
                        let (gender_handle, gender_color) = match identity.gender {
                            Gender::Male => {
                                (game_assets.icon_male.clone(), Color::srgb(0.4, 0.7, 1.0))
                            }
                            Gender::Female => {
                                (game_assets.icon_female.clone(), Color::srgb(1.0, 0.5, 0.7))
                            }
                        };
                        let (task_handle, task_color) = match task {
                            AssignedTask::None => {
                                (game_assets.icon_idle.clone(), Color::srgb(0.6, 0.6, 0.6))
                            }
                            AssignedTask::Gather { work_type, .. } => match work_type {
                                WorkType::Chop => (
                                    game_assets.icon_axe.clone(),
                                    Color::srgb(0.6, 0.4, 0.2), // 茶色
                                ),
                                WorkType::Mine => (
                                    game_assets.icon_pick.clone(),
                                    Color::srgb(0.7, 0.7, 0.7), // 灰色
                                ),
                                _ => (
                                    game_assets.icon_pick.clone(),
                                    Color::srgb(1.0, 0.7, 0.3), // デフォルト
                                ),
                            },
                            AssignedTask::Haul { .. } => {
                                (game_assets.icon_haul.clone(), Color::srgb(0.5, 1.0, 0.5))
                            }
                        };
                        let stress_color = if soul.stress > 0.8 {
                            Color::srgb(1.0, 0.0, 0.0)
                        } else if soul.stress > 0.5 {
                            Color::srgb(1.0, 0.5, 0.0)
                        } else {
                            Color::WHITE
                        };

                        parent
                            .spawn((
                                Button,
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(20.0),
                                    align_items: AlignItems::Center,
                                    ..default()
                                },
                                BackgroundColor(Color::NONE),
                                SoulListItem(soul_entity),
                            ))
                            .with_children(|item| {
                                // Gender Icon
                                item.spawn((
                                    ImageNode {
                                        image: gender_handle,
                                        color: gender_color,
                                        ..default()
                                    },
                                    Node {
                                        width: Val::Px(16.0),
                                        height: Val::Px(16.0),
                                        margin: UiRect::right(Val::Px(4.0)),
                                        ..default()
                                    },
                                ));
                                // Name
                                item.spawn((
                                    Text::new(identity.name.clone()),
                                    TextFont {
                                        font_size: 12.0,
                                        ..default()
                                    },
                                    TextColor(stress_color),
                                    Node {
                                        margin: UiRect::right(Val::Px(6.0)),
                                        ..default()
                                    },
                                ));
                                // Fatigue icon & %
                                item.spawn((
                                    ImageNode {
                                        image: game_assets.icon_fatigue.clone(),
                                        color: Color::srgb(0.6, 0.6, 1.0),
                                        ..default()
                                    },
                                    Node {
                                        width: Val::Px(16.0),
                                        height: Val::Px(16.0),
                                        margin: UiRect::right(Val::Px(2.0)),
                                        ..default()
                                    },
                                ));
                                item.spawn((
                                    Text::new(format!("{:.0}%", soul.fatigue * 100.0)),
                                    TextFont {
                                        font_size: 10.0,
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.7, 0.7, 1.0)),
                                    Node {
                                        margin: UiRect::right(Val::Px(6.0)),
                                        ..default()
                                    },
                                ));
                                // Stress icon & %
                                item.spawn((
                                    ImageNode {
                                        image: game_assets.icon_stress.clone(),
                                        color: Color::srgb(1.0, 0.9, 0.2),
                                        ..default()
                                    },
                                    Node {
                                        width: Val::Px(16.0),
                                        height: Val::Px(16.0),
                                        margin: UiRect::right(Val::Px(2.0)),
                                        ..default()
                                    },
                                ));
                                item.spawn((
                                    Text::new(format!("{:.0}%", soul.stress * 100.0)),
                                    TextFont {
                                        font_size: 10.0,
                                        ..default()
                                    },
                                    TextColor(stress_color),
                                    Node {
                                        margin: UiRect::right(Val::Px(6.0)),
                                        ..default()
                                    },
                                ));
                                // Task icon
                                item.spawn((
                                    ImageNode {
                                        image: task_handle,
                                        color: task_color,
                                        ..default()
                                    },
                                    Node {
                                        width: Val::Px(16.0),
                                        height: Val::Px(16.0),
                                        ..default()
                                    },
                                ));
                            });
                    });
            }
        }
    }
}

/// エンティティリストのインタラクション
pub fn entity_list_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &SectionToggle, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut soul_list_interaction: Query<
        (&Interaction, &SoulListItem),
        (
            Changed<Interaction>,
            With<Button>,
            Without<FamiliarListItem>,
        ),
    >,
    mut familiar_list_interaction: Query<
        (&Interaction, &FamiliarListItem),
        (Changed<Interaction>, With<Button>, Without<SoulListItem>),
    >,
    mut fold_state: ResMut<EntityListFoldState>,
    mut selected_entity: ResMut<crate::interface::selection::SelectedEntity>,
    mut q_camera: Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: Query<&GlobalTransform>,
) {
    // セクション切り替え
    for (interaction, toggle, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgba(0.5, 0.5, 0.5, 0.8));
                match toggle.0 {
                    EntityListSectionType::Familiar(entity) => {
                        if fold_state.folded_familiars.contains(&entity) {
                            fold_state.folded_familiars.remove(&entity);
                        } else {
                            fold_state.folded_familiars.insert(entity);
                        }
                    }
                    EntityListSectionType::Unassigned => {
                        fold_state.unassigned_folded = !fold_state.unassigned_folded;
                    }
                }
            }
            // Note: Hover/None の色変更は削除（リスト再構築時のちらつき防止）
            _ => {}
        }
    }

    // ソウル選択
    for (interaction, item) in soul_list_interaction.iter_mut() {
        if *interaction == Interaction::Pressed {
            selected_entity.0 = Some(item.0);
            info!("LIST: Selected soul {:?}", item.0);

            // カメラ移動
            if let Ok(target_transform) = q_transforms.get(item.0) {
                if let Some(mut cam_transform) = q_camera.iter_mut().next() {
                    let target_pos = target_transform.translation().truncate();
                    cam_transform.translation.x = target_pos.x;
                    cam_transform.translation.y = target_pos.y;
                }
            }
        }
    }

    // 使い魔選択
    for (interaction, item) in familiar_list_interaction.iter_mut() {
        if *interaction == Interaction::Pressed {
            selected_entity.0 = Some(item.0);
            info!("LIST: Selected familiar {:?}", item.0);

            // カメラ移動
            if let Ok(target_transform) = q_transforms.get(item.0) {
                if let Some(mut cam_transform) = q_camera.iter_mut().next() {
                    let target_pos = target_transform.translation().truncate();
                    cam_transform.translation.x = target_pos.x;
                    cam_transform.translation.y = target_pos.y;
                }
            }
        }
    }
}

/// 未所属ソウルセクションの矢印アイコンを折りたたみ状態に応じて更新
pub fn update_unassigned_arrow_icon_system(
    game_assets: Res<crate::assets::GameAssets>,
    fold_state: Res<EntityListFoldState>,
    mut q_arrow: Query<&mut ImageNode, With<UnassignedSectionArrowIcon>>,
) {
    if !fold_state.is_changed() {
        return;
    }
    for mut icon in q_arrow.iter_mut() {
        icon.image = if fold_state.unassigned_folded {
            game_assets.icon_arrow_right.clone()
        } else {
            game_assets.icon_arrow_down.clone()
        };
    }
}
