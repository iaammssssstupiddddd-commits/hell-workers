use super::{
    FamiliarRowViewModel, FamiliarSectionNodes, SoulRowViewModel, StressBucket, TaskVisual,
};
use crate::entities::damned_soul::{DamnedSoul, Gender, SoulIdentity};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::interface::ui::components::*;
use crate::interface::ui::theme::UiTheme;
use crate::relationships::CommandedBy;
use crate::relationships::Commanding;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::jobs::WorkType;
use crate::systems::soul_ai::task_execution::AssignedTask;
use bevy::prelude::*;

/// 性別に応じたアイコンハンドルと色を取得
fn get_gender_icon_and_color(
    gender: Gender,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match gender {
        Gender::Male => (game_assets.icon_male.clone(), theme.colors.male),
        Gender::Female => (game_assets.icon_female.clone(), theme.colors.female),
    }
}

/// タスクに応じたアイコンハンドルと色を取得
fn get_task_icon_and_color(
    task: TaskVisual,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match task {
        TaskVisual::Idle => (game_assets.icon_idle.clone(), theme.colors.idle),
        TaskVisual::Chop => (game_assets.icon_axe.clone(), theme.colors.chop),
        TaskVisual::Mine => (game_assets.icon_pick.clone(), theme.colors.mine),
        TaskVisual::GatherDefault => (game_assets.icon_pick.clone(), theme.colors.gather_default),
        TaskVisual::Haul => (game_assets.icon_haul.clone(), theme.colors.haul),
        TaskVisual::Build => (game_assets.icon_pick.clone(), theme.colors.build),
        TaskVisual::HaulToBlueprint => (game_assets.icon_haul.clone(), theme.colors.haul_to_bp),
        TaskVisual::Water => (game_assets.icon_haul.clone(), theme.colors.water),
    }
}

/// ストレス値に応じた色を取得
fn get_stress_color(bucket: StressBucket, theme: &UiTheme) -> Color {
    match bucket {
        StressBucket::Low => Color::WHITE,
        StressBucket::Medium => theme.colors.stress_medium,
        StressBucket::High => theme.colors.stress_high,
    }
}

pub(super) fn familiar_state_label(ai_state: &FamiliarAiState) -> &'static str {
    match ai_state {
        FamiliarAiState::Idle => "Idle",
        FamiliarAiState::SearchingTask => "Searching",
        FamiliarAiState::Scouting { .. } => "Scouting",
        FamiliarAiState::Supervising { .. } => "Supervising",
    }
}

pub(super) fn task_visual(task: &AssignedTask) -> TaskVisual {
    match task {
        AssignedTask::None => TaskVisual::Idle,
        AssignedTask::Gather(data) => match data.work_type {
            WorkType::Chop => TaskVisual::Chop,
            WorkType::Mine => TaskVisual::Mine,
            _ => TaskVisual::GatherDefault,
        },
        AssignedTask::Haul { .. } => TaskVisual::Haul,
        AssignedTask::Build { .. } => TaskVisual::Build,
        AssignedTask::HaulToBlueprint { .. } => TaskVisual::HaulToBlueprint,
        AssignedTask::GatherWater { .. } => TaskVisual::Water,
        AssignedTask::CollectSand { .. } => TaskVisual::GatherDefault,
        AssignedTask::Refine { .. } => TaskVisual::Build,
        AssignedTask::HaulToMixer { .. } => TaskVisual::HaulToBlueprint,
        AssignedTask::HaulWaterToMixer { .. } => TaskVisual::Water,
    }
}

pub(super) fn stress_bucket(stress: f32) -> StressBucket {
    if stress > 0.8 {
        StressBucket::High
    } else if stress > 0.5 {
        StressBucket::Medium
    } else {
        StressBucket::Low
    }
}

pub(super) fn build_soul_view_model(
    soul_entity: Entity,
    soul: &DamnedSoul,
    task: &AssignedTask,
    identity: &SoulIdentity,
) -> SoulRowViewModel {
    SoulRowViewModel {
        entity: soul_entity,
        name: identity.name.clone(),
        gender: identity.gender,
        fatigue_text: format!("{:.0}%", soul.fatigue * 100.0),
        stress_text: format!("{:.0}%", soul.stress * 100.0),
        stress_bucket: stress_bucket(soul.stress),
        task_visual: task_visual(task),
    }
}

pub(super) fn build_familiar_row_view_model(
    fam_entity: Entity,
    familiar: &Familiar,
    op: &FamiliarOperation,
    ai_state: &FamiliarAiState,
    commanding_opt: Option<&Commanding>,
    is_folded: bool,
    q_all_souls: &Query<
        (
            Entity,
            &DamnedSoul,
            &AssignedTask,
            &SoulIdentity,
            Option<&CommandedBy>,
        ),
        Without<Familiar>,
    >,
) -> FamiliarRowViewModel {
    let squad_count = commanding_opt.map(|c| c.len()).unwrap_or(0);
    let mut souls = Vec::new();
    let mut show_empty = false;

    if !is_folded {
        if let Some(commanding) = commanding_opt {
            if commanding.is_empty() {
                show_empty = true;
            } else {
                for &soul_entity in commanding.iter() {
                    if let Ok((_, soul, task, identity, _)) = q_all_souls.get(soul_entity) {
                        souls.push(build_soul_view_model(soul_entity, soul, task, identity));
                    }
                }
            }
        }
    }

    FamiliarRowViewModel {
        entity: fam_entity,
        label: format!(
            "{} ({}/{}) [{}]",
            familiar.name,
            squad_count,
            op.max_controlled_soul,
            familiar_state_label(ai_state)
        ),
        is_folded,
        show_empty,
        souls,
    }
}

pub(super) fn spawn_soul_list_item(
    parent: &mut ChildSpawnerCommands,
    soul_vm: &SoulRowViewModel,
    game_assets: &crate::assets::GameAssets,
    left_margin: f32,
    theme: &UiTheme,
) {
    let (gender_handle, gender_color) = get_gender_icon_and_color(soul_vm.gender, game_assets, theme);
    let (task_handle, task_color) = get_task_icon_and_color(soul_vm.task_visual, game_assets, theme);
    let stress_color = get_stress_color(soul_vm.stress_bucket, theme);

    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(theme.sizes.soul_item_height),
                align_items: AlignItems::Center,
                border: UiRect::left(Val::Px(0.0)),
                margin: if left_margin > 0.0 {
                    UiRect::left(Val::Px(left_margin))
                } else {
                    UiRect::default()
                },
                ..default()
            },
            BackgroundColor(theme.colors.list_item_default),
            BorderColor::all(Color::NONE),
            SoulListItem(soul_vm.entity),
        ))
        .with_children(|item| {
            item.spawn((
                ImageNode {
                    image: gender_handle,
                    color: gender_color,
                    ..default()
                },
                Node {
                    width: Val::Px(theme.sizes.icon_size),
                    height: Val::Px(theme.sizes.icon_size),
                    margin: UiRect::right(Val::Px(theme.spacing.margin_medium)),
                    ..default()
                },
            ));
            item.spawn((
                Text::new(soul_vm.name.clone()),
                TextFont {
                    font: game_assets.font_soul_name.clone(),
                    font_size: theme.typography.font_size_item,
                    ..default()
                },
                TextColor(stress_color),
                Node {
                    margin: UiRect::right(Val::Px(theme.spacing.margin_large)),
                    ..default()
                },
            ));
            item.spawn((
                ImageNode {
                    image: game_assets.icon_fatigue.clone(),
                    color: theme.colors.fatigue_icon,
                    ..default()
                },
                Node {
                    width: Val::Px(theme.sizes.icon_size),
                    height: Val::Px(theme.sizes.icon_size),
                    margin: UiRect::right(Val::Px(theme.spacing.margin_small)),
                    ..default()
                },
            ));
            item.spawn((
                Text::new(soul_vm.fatigue_text.clone()),
                TextFont {
                    font_size: theme.typography.font_size_small,
                    ..default()
                },
                TextColor(theme.colors.fatigue_text),
                Node {
                    margin: UiRect::right(Val::Px(theme.spacing.margin_large)),
                    ..default()
                },
            ));
            item.spawn((
                ImageNode {
                    image: game_assets.icon_stress.clone(),
                    color: theme.colors.stress_icon,
                    ..default()
                },
                Node {
                    width: Val::Px(theme.sizes.icon_size),
                    height: Val::Px(theme.sizes.icon_size),
                    margin: UiRect::right(Val::Px(theme.spacing.margin_small)),
                    ..default()
                },
            ));
            item.spawn((
                Text::new(soul_vm.stress_text.clone()),
                TextFont {
                    font_size: theme.typography.font_size_small,
                    ..default()
                },
                TextColor(stress_color),
                Node {
                    margin: UiRect::right(Val::Px(theme.spacing.margin_large)),
                    ..default()
                },
            ));
            item.spawn((
                ImageNode {
                    image: task_handle,
                    color: task_color,
                    ..default()
                },
                Node {
                    width: Val::Px(theme.sizes.icon_size),
                    height: Val::Px(theme.sizes.icon_size),
                    ..default()
                },
            ));
        });
}

pub(super) fn spawn_soul_list_item_entity(
    commands: &mut Commands,
    parent_entity: Entity,
    soul_vm: &SoulRowViewModel,
    game_assets: &crate::assets::GameAssets,
    left_margin: f32,
    theme: &UiTheme,
) -> Entity {
    let (gender_handle, gender_color) = get_gender_icon_and_color(soul_vm.gender, game_assets, theme);
    let (task_handle, task_color) = get_task_icon_and_color(soul_vm.task_visual, game_assets, theme);
    let stress_color = get_stress_color(soul_vm.stress_bucket, theme);

    let row = commands
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(theme.sizes.soul_item_height),
                align_items: AlignItems::Center,
                border: UiRect::left(Val::Px(0.0)),
                margin: if left_margin > 0.0 {
                    UiRect::left(Val::Px(left_margin))
                } else {
                    UiRect::default()
                },
                ..default()
            },
            BackgroundColor(theme.colors.list_item_default),
            BorderColor::all(Color::NONE),
            SoulListItem(soul_vm.entity),
        ))
        .id();
    commands.entity(parent_entity).add_child(row);

    let gender_icon = commands
        .spawn((
            ImageNode {
                image: gender_handle,
                color: gender_color,
                ..default()
            },
            Node {
                width: Val::Px(theme.sizes.icon_size),
                height: Val::Px(theme.sizes.icon_size),
                margin: UiRect::right(Val::Px(theme.spacing.margin_medium)),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(gender_icon);

    let name = commands
        .spawn((
            Text::new(soul_vm.name.clone()),
            TextFont {
                font: game_assets.font_soul_name.clone(),
                font_size: theme.typography.font_size_item,
                ..default()
            },
            TextColor(stress_color),
            Node {
                margin: UiRect::right(Val::Px(theme.spacing.margin_large)),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(name);

    let fatigue_icon = commands
        .spawn((
            ImageNode {
                image: game_assets.icon_fatigue.clone(),
                color: theme.colors.fatigue_icon,
                ..default()
            },
            Node {
                width: Val::Px(theme.sizes.icon_size),
                height: Val::Px(theme.sizes.icon_size),
                margin: UiRect::right(Val::Px(theme.spacing.margin_small)),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(fatigue_icon);

    let fatigue_text = commands
        .spawn((
            Text::new(soul_vm.fatigue_text.clone()),
            TextFont {
                font_size: theme.typography.font_size_small,
                ..default()
            },
            TextColor(theme.colors.fatigue_text),
            Node {
                margin: UiRect::right(Val::Px(theme.spacing.margin_large)),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(fatigue_text);

    let stress_icon = commands
        .spawn((
            ImageNode {
                image: game_assets.icon_stress.clone(),
                color: theme.colors.stress_icon,
                ..default()
            },
            Node {
                width: Val::Px(theme.sizes.icon_size),
                height: Val::Px(theme.sizes.icon_size),
                margin: UiRect::right(Val::Px(theme.spacing.margin_small)),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(stress_icon);

    let stress_text = commands
        .spawn((
            Text::new(soul_vm.stress_text.clone()),
            TextFont {
                font_size: theme.typography.font_size_small,
                ..default()
            },
            TextColor(stress_color),
            Node {
                margin: UiRect::right(Val::Px(theme.spacing.margin_large)),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(stress_text);

    let task_icon = commands
        .spawn((
            ImageNode {
                image: task_handle,
                color: task_color,
                ..default()
            },
            Node {
                width: Val::Px(theme.sizes.icon_size),
                height: Val::Px(theme.sizes.icon_size),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(task_icon);

    row
}

pub(super) fn spawn_familiar_section(
    commands: &mut Commands,
    parent_container: Entity,
    familiar: &FamiliarRowViewModel,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) -> FamiliarSectionNodes {
    let fold_icon_handle = if familiar.is_folded {
        game_assets.icon_arrow_right.clone()
    } else {
        game_assets.icon_arrow_down.clone()
    };

    let root = commands
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            margin: UiRect::top(Val::Px(theme.sizes.familiar_section_margin_top)),
            ..default()
        },))
        .id();
    commands.entity(parent_container).add_child(root);

    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(theme.sizes.header_height),
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();
    commands.entity(root).add_child(header);

    let fold_button = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(theme.sizes.fold_button_size),
                height: Val::Px(theme.sizes.fold_button_size),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.fold_button_bg),
            SectionToggle(EntityListSectionType::Familiar(familiar.entity)),
        ))
        .id();
    commands.entity(header).add_child(fold_button);

    let fold_icon = commands
        .spawn((
            ImageNode {
                image: fold_icon_handle,
                ..default()
            },
            Node {
                width: Val::Px(theme.sizes.fold_icon_size),
                height: Val::Px(theme.sizes.fold_icon_size),
                ..default()
            },
        ))
        .id();
    commands.entity(fold_button).add_child(fold_icon);

    let familiar_button = commands
        .spawn((
            Button,
            Node {
                flex_grow: 1.0,
                height: Val::Px(theme.sizes.header_height),
                align_items: AlignItems::Center,
                border: UiRect::left(Val::Px(0.0)),
                padding: UiRect::left(Val::Px(theme.spacing.text_left_padding)),
                ..default()
            },
            BackgroundColor(theme.colors.familiar_button_bg),
            BorderColor::all(Color::NONE),
            FamiliarListItem(familiar.entity),
        ))
        .id();
    commands.entity(header).add_child(familiar_button);

    let header_text = commands
        .spawn((
            Text::new(familiar.label.clone()),
            TextFont {
                font: game_assets.font_familiar.clone(),
                font_size: theme.typography.font_size_header,
                ..default()
            },
            TextColor(theme.colors.header_text),
        ))
        .id();
    commands.entity(familiar_button).add_child(header_text);

    let members_container = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    commands.entity(root).add_child(members_container);

    FamiliarSectionNodes {
        root,
        header_text,
        fold_icon,
        members_container,
    }
}

pub(super) fn clear_children(
    commands: &mut Commands,
    q_children: &Query<&Children>,
    parent: Entity,
) {
    if let Ok(children) = q_children.get(parent) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
}

pub(super) fn sync_familiar_section_content(
    commands: &mut Commands,
    q_children: &Query<&Children>,
    familiar: &FamiliarRowViewModel,
    nodes: FamiliarSectionNodes,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) {
    clear_children(commands, q_children, nodes.members_container);

    if familiar.is_folded {
        return;
    }

    commands
        .entity(nodes.members_container)
        .with_children(|members_parent| {
            if familiar.show_empty {
                members_parent.spawn((
                    Text::new("  (empty)"),
                    TextFont {
                        font: game_assets.font_ui.clone(),
                        font_size: theme.typography.font_size_item,
                        ..default()
                    },
                    TextColor(theme.colors.empty_text),
                    Node {
                        margin: UiRect::left(Val::Px(theme.sizes.empty_squad_left_margin)),
                        ..default()
                    },
                ));
                return;
            }

            for soul_vm in &familiar.souls {
                spawn_soul_list_item(
                    members_parent,
                    soul_vm,
                    game_assets,
                    theme.sizes.squad_member_left_margin,
                    theme,
                );
            }
        });
}

pub(super) fn select_entity_and_focus_camera(
    target: Entity,
    label: &str,
    selected_entity: &mut ResMut<crate::interface::selection::SelectedEntity>,
    q_camera: &mut Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: &Query<&GlobalTransform>,
) {
    selected_entity.0 = Some(target);
    info!("LIST: Selected {} {:?}", label, target);

    if let Ok(target_transform) = q_transforms.get(target) {
        if let Some(mut cam_transform) = q_camera.iter_mut().next() {
            let target_pos = target_transform.translation().truncate();
            cam_transform.translation.x = target_pos.x;
            cam_transform.translation.y = target_pos.y;
        }
    }
}
