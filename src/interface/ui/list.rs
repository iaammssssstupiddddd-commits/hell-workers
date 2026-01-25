//! エンティティリストの動的更新システム

use crate::entities::damned_soul::{DamnedSoul, Gender, SoulIdentity};
use crate::entities::familiar::{Familiar, FamiliarOperation, UnderCommand};
use crate::interface::ui::components::*;
use crate::relationships::Commanding;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::jobs::WorkType;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::prelude::*;

// ============================================================
// UI定数
// ============================================================

mod ui_constants {
    use super::*;

    // サイズ定数
    pub const HEADER_HEIGHT: f32 = 24.0;
    pub const SOUL_ITEM_HEIGHT: f32 = 20.0;
    pub const ICON_SIZE: f32 = 16.0;
    pub const FOLD_ICON_SIZE: f32 = 12.0;
    pub const FOLD_BUTTON_SIZE: f32 = 20.0;
    pub const FAMILIAR_SECTION_MARGIN_TOP: f32 = 4.0;
    pub const SQUAD_MEMBER_LEFT_MARGIN: f32 = 15.0;
    pub const EMPTY_SQUAD_LEFT_MARGIN: f32 = 15.0;

    // マージン定数
    pub const MARGIN_SMALL: f32 = 2.0;
    pub const MARGIN_MEDIUM: f32 = 4.0;
    pub const MARGIN_LARGE: f32 = 6.0;
    pub const TEXT_LEFT_PADDING: f32 = 4.0;

    // フォントサイズ
    pub const FONT_SIZE_HEADER: f32 = 14.0;
    pub const FONT_SIZE_ITEM: f32 = 12.0;
    pub const FONT_SIZE_SMALL: f32 = 10.0;

    // 色定数
    pub const COLOR_MALE: Color = Color::srgb(0.4, 0.7, 1.0);
    pub const COLOR_FEMALE: Color = Color::srgb(1.0, 0.5, 0.7);
    pub const COLOR_IDLE: Color = Color::srgb(0.6, 0.6, 0.6);
    pub const COLOR_CHOP: Color = Color::srgb(0.6, 0.4, 0.2);
    pub const COLOR_MINE: Color = Color::srgb(0.7, 0.7, 0.7);
    pub const COLOR_GATHER_DEFAULT: Color = Color::srgb(1.0, 0.7, 0.3);
    pub const COLOR_HAUL: Color = Color::srgb(0.5, 1.0, 0.5);
    pub const COLOR_BUILD: Color = Color::srgb(0.8, 0.6, 0.2);
    pub const COLOR_HAUL_TO_BP: Color = Color::srgb(0.8, 0.8, 0.3);
    pub const COLOR_WATER: Color = Color::srgb(0.3, 0.5, 1.0);
    pub const COLOR_STRESS_HIGH: Color = Color::srgb(1.0, 0.0, 0.0);
    pub const COLOR_STRESS_MEDIUM: Color = Color::srgb(1.0, 0.5, 0.0);
    pub const COLOR_FATIGUE_ICON: Color = Color::srgb(0.6, 0.6, 1.0);
    pub const COLOR_FATIGUE_TEXT: Color = Color::srgb(0.7, 0.7, 1.0);
    pub const COLOR_STRESS_ICON: Color = Color::srgb(1.0, 0.9, 0.2);
    pub const COLOR_HEADER_TEXT: Color = Color::srgb(0.8, 0.8, 1.0);
    pub const COLOR_EMPTY_TEXT: Color = Color::srgb(0.5, 0.5, 0.5);
    pub const COLOR_FOLD_BUTTON_BG: Color = Color::srgba(0.3, 0.3, 0.5, 0.6);
    pub const COLOR_FAMILIAR_BUTTON_BG: Color = Color::srgba(0.2, 0.2, 0.4, 0.6);
    pub const COLOR_SECTION_TOGGLE_PRESSED: Color = Color::srgba(0.5, 0.5, 0.5, 0.8);
}

use ui_constants::*;

// ============================================================
// ヘルパー関数
// ============================================================

/// 性別に応じたアイコンハンドルと色を取得
fn get_gender_icon_and_color(
    gender: Gender,
    game_assets: &crate::assets::GameAssets,
) -> (Handle<Image>, Color) {
    match gender {
        Gender::Male => (game_assets.icon_male.clone(), COLOR_MALE),
        Gender::Female => (game_assets.icon_female.clone(), COLOR_FEMALE),
    }
}

/// タスクに応じたアイコンハンドルと色を取得
fn get_task_icon_and_color(
    task: &AssignedTask,
    game_assets: &crate::assets::GameAssets,
) -> (Handle<Image>, Color) {
    match task {
        AssignedTask::None => (game_assets.icon_idle.clone(), COLOR_IDLE),
        AssignedTask::Gather { work_type, .. } => match work_type {
            WorkType::Chop => (game_assets.icon_axe.clone(), COLOR_CHOP),
            WorkType::Mine => (game_assets.icon_pick.clone(), COLOR_MINE),
            _ => (game_assets.icon_pick.clone(), COLOR_GATHER_DEFAULT),
        },
        AssignedTask::Haul { .. } => (game_assets.icon_haul.clone(), COLOR_HAUL),
        AssignedTask::Build { .. } => (game_assets.icon_pick.clone(), COLOR_BUILD),
        AssignedTask::HaulToBlueprint { .. } => (game_assets.icon_haul.clone(), COLOR_HAUL_TO_BP),
        AssignedTask::GatherWater { .. } => (game_assets.icon_haul.clone(), COLOR_WATER),
    }
}

/// ストレス値に応じた色を取得
fn get_stress_color(stress: f32) -> Color {
    if stress > 0.8 {
        COLOR_STRESS_HIGH
    } else if stress > 0.5 {
        COLOR_STRESS_MEDIUM
    } else {
        Color::WHITE
    }
}

/// ソウルリストアイテムを構築
///
/// `parent`は`with_children`のクロージャ内で使用される`ChildBuilder`です
macro_rules! build_soul_list_item {
    ($parent:expr, $soul_entity:expr, $soul:expr, $task:expr, $identity:expr, $game_assets:expr, $left_margin:expr) => {{
        let (gender_handle, gender_color) =
            get_gender_icon_and_color($identity.gender, $game_assets);
        let (task_handle, task_color) = get_task_icon_and_color($task, $game_assets);
        let stress_color = get_stress_color($soul.stress);

        $parent
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(SOUL_ITEM_HEIGHT),
                    align_items: AlignItems::Center,
                    margin: if $left_margin > 0.0 {
                        UiRect::left(Val::Px($left_margin))
                    } else {
                        UiRect::default()
                    },
                    ..default()
                },
                BackgroundColor(Color::NONE),
                SoulListItem($soul_entity),
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
                        width: Val::Px(ICON_SIZE),
                        height: Val::Px(ICON_SIZE),
                        margin: UiRect::right(Val::Px(MARGIN_MEDIUM)),
                        ..default()
                    },
                ));
                // Name
                item.spawn((
                    Text::new($identity.name.clone()),
                    TextFont {
                        font: $game_assets.font_soul_name.clone(),
                        font_size: FONT_SIZE_ITEM,
                        ..default()
                    },
                    TextColor(stress_color),
                    Node {
                        margin: UiRect::right(Val::Px(MARGIN_LARGE)),
                        ..default()
                    },
                ));
                // Fatigue icon & %
                item.spawn((
                    ImageNode {
                        image: $game_assets.icon_fatigue.clone(),
                        color: COLOR_FATIGUE_ICON,
                        ..default()
                    },
                    Node {
                        width: Val::Px(ICON_SIZE),
                        height: Val::Px(ICON_SIZE),
                        margin: UiRect::right(Val::Px(MARGIN_SMALL)),
                        ..default()
                    },
                ));
                item.spawn((
                    Text::new(format!("{:.0}%", $soul.fatigue * 100.0)),
                    TextFont {
                        font_size: FONT_SIZE_SMALL,
                        ..default()
                    },
                    TextColor(COLOR_FATIGUE_TEXT),
                    Node {
                        margin: UiRect::right(Val::Px(MARGIN_LARGE)),
                        ..default()
                    },
                ));
                // Stress icon & %
                item.spawn((
                    ImageNode {
                        image: $game_assets.icon_stress.clone(),
                        color: COLOR_STRESS_ICON,
                        ..default()
                    },
                    Node {
                        width: Val::Px(ICON_SIZE),
                        height: Val::Px(ICON_SIZE),
                        margin: UiRect::right(Val::Px(MARGIN_SMALL)),
                        ..default()
                    },
                ));
                item.spawn((
                    Text::new(format!("{:.0}%", $soul.stress * 100.0)),
                    TextFont {
                        font_size: FONT_SIZE_SMALL,
                        ..default()
                    },
                    TextColor(stress_color),
                    Node {
                        margin: UiRect::right(Val::Px(MARGIN_LARGE)),
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
                        width: Val::Px(ICON_SIZE),
                        height: Val::Px(ICON_SIZE),
                        ..default()
                    },
                ));
            });
    }};
}

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
                        margin: UiRect::top(Val::Px(FAMILIAR_SECTION_MARGIN_TOP)),
                        ..default()
                    },))
                    .with_children(|header_node| {
                        // ヘッダー行（アイコンと名前を横に並べる）
                        header_node
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(HEADER_HEIGHT),
                                align_items: AlignItems::Center,
                                flex_direction: FlexDirection::Row,
                                ..default()
                            })
                            .with_children(|row| {
                                // 折りたたみアイコンボタン
                                row.spawn((
                                    Button,
                                    Node {
                                        width: Val::Px(FOLD_BUTTON_SIZE),
                                        height: Val::Px(FOLD_BUTTON_SIZE),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    BackgroundColor(COLOR_FOLD_BUTTON_BG),
                                    SectionToggle(EntityListSectionType::Familiar(fam_entity)),
                                ))
                                .with_children(|icon_btn| {
                                    icon_btn.spawn((
                                        ImageNode {
                                            image: fold_icon,
                                            ..default()
                                        },
                                        Node {
                                            width: Val::Px(FOLD_ICON_SIZE),
                                            height: Val::Px(FOLD_ICON_SIZE),
                                            ..default()
                                        },
                                    ));
                                });

                                // 名前ボタン（選択用）
                                row.spawn((
                                    Button,
                                    Node {
                                        flex_grow: 1.0,
                                        height: Val::Px(HEADER_HEIGHT),
                                        align_items: AlignItems::Center,
                                        padding: UiRect::left(Val::Px(TEXT_LEFT_PADDING)),
                                        ..default()
                                    },
                                    BackgroundColor(COLOR_FAMILIAR_BUTTON_BG),
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
                                            font: game_assets.font_familiar.clone(),
                                            font_size: FONT_SIZE_HEADER,
                                            ..default()
                                        },
                                        TextColor(COLOR_HEADER_TEXT),
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
                                            font: game_assets.font_ui.clone(),
                                            font_size: FONT_SIZE_ITEM,
                                            ..default()
                                        },
                                        TextColor(COLOR_EMPTY_TEXT),
                                        Node {
                                            margin: UiRect::left(Val::Px(EMPTY_SQUAD_LEFT_MARGIN)),
                                            ..default()
                                        },
                                    ));
                                } else {
                                    for &soul_entity in commanding.iter() {
                                        if let Ok((_, soul, task, identity, _)) =
                                            q_all_souls.get(soul_entity)
                                        {
                                            build_soul_list_item!(
                                                header_node,
                                                soul_entity,
                                                soul,
                                                task,
                                                identity,
                                                &game_assets,
                                                SQUAD_MEMBER_LEFT_MARGIN
                                            );
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
                        build_soul_list_item!(
                            parent,
                            soul_entity,
                            soul,
                            task,
                            identity,
                            &game_assets,
                            0.0 // 未所属はマージンなし
                        );
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
                *color = BackgroundColor(COLOR_SECTION_TOGGLE_PRESSED);
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
