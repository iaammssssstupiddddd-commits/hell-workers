use super::super::{SoulRowViewModel, StressBucket, TaskVisual};
use crate::entities::damned_soul::Gender;
use crate::interface::ui::components::SoulListItem;
use crate::interface::ui::theme::UiTheme;
use bevy::prelude::*;

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

fn get_stress_color(bucket: StressBucket, theme: &UiTheme) -> Color {
    match bucket {
        StressBucket::Low => Color::WHITE,
        StressBucket::Medium => theme.colors.stress_medium,
        StressBucket::High => theme.colors.stress_high,
    }
}

fn get_dream_color(dream_empty: bool, theme: &UiTheme) -> Color {
    if dream_empty {
        theme.colors.stress_medium
    } else {
        theme.colors.fatigue_text
    }
}

fn stress_weight(bucket: StressBucket) -> FontWeight {
    match bucket {
        StressBucket::High => FontWeight::BOLD,
        _ => FontWeight::default(),
    }
}

pub(super) fn spawn_soul_list_item(
    parent: &mut ChildSpawnerCommands,
    soul_vm: &SoulRowViewModel,
    game_assets: &crate::assets::GameAssets,
    left_margin: f32,
    theme: &UiTheme,
) -> Entity {
    let (gender_handle, gender_color) =
        get_gender_icon_and_color(soul_vm.gender, game_assets, theme);
    let (task_handle, task_color) =
        get_task_icon_and_color(soul_vm.task_visual, game_assets, theme);
    let stress_color = get_stress_color(soul_vm.stress_bucket, theme);
    let dream_color = get_dream_color(soul_vm.dream_empty, theme);

    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(theme.sizes.soul_item_height),
                flex_shrink: 0.0,
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
                    flex_shrink: 0.0,
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
                    flex_shrink: 0.0,
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
                    flex_shrink: 0.0,
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
                    flex_shrink: 0.0,
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
                    flex_shrink: 0.0,
                    margin: UiRect::right(Val::Px(theme.spacing.margin_small)),
                    ..default()
                },
            ));
            item.spawn((
                Text::new(soul_vm.stress_text.clone()),
                TextFont {
                    font_size: theme.typography.font_size_small,
                    weight: stress_weight(soul_vm.stress_bucket),
                    ..default()
                },
                TextColor(stress_color),
                Node {
                    flex_shrink: 0.0,
                    margin: UiRect::right(Val::Px(theme.spacing.margin_large)),
                    ..default()
                },
            ));
            // children[6]: dream text
            item.spawn((
                Text::new(soul_vm.dream_text.clone()),
                TextFont {
                    font_size: theme.typography.font_size_small,
                    ..default()
                },
                TextColor(dream_color),
                Node {
                    flex_shrink: 0.0,
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
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
        })
        .id()
}

pub(super) fn spawn_soul_list_item_entity(
    commands: &mut Commands,
    parent_entity: Entity,
    soul_vm: &SoulRowViewModel,
    game_assets: &crate::assets::GameAssets,
    left_margin: f32,
    theme: &UiTheme,
) -> Entity {
    let mut result = Entity::PLACEHOLDER;
    commands.entity(parent_entity).with_children(|parent| {
        result = spawn_soul_list_item(parent, soul_vm, game_assets, left_margin, theme);
    });
    result
}
