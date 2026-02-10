//! エンティティリストUIノードのスポーン

use super::{
    FamiliarRowViewModel, FamiliarSectionNodes, SoulRowViewModel, StressBucket, TaskVisual,
};
use crate::entities::damned_soul::Gender;
use crate::interface::ui::components::*;
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
            flex_shrink: 0.0,
            margin: UiRect::top(Val::Px(theme.sizes.familiar_section_margin_top)),
            ..default()
        },))
        .id();
    commands.entity(parent_container).add_child(root);

    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(theme.sizes.header_height),
            flex_shrink: 0.0,
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
                flex_shrink: 0.0,
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
                flex_shrink: 0.0,
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
            TextColor(theme.colors.accent_soul),
        ))
        .id();
    commands.entity(familiar_button).add_child(header_text);

    let adjust_container = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: Val::Px(theme.spacing.margin_small),
            padding: UiRect::right(Val::Px(theme.spacing.margin_small)),
            ..default()
        })
        .id();
    commands.entity(header).add_child(adjust_container);

    let decrease_button = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            FamiliarMaxSoulAdjustButton {
                familiar: familiar.entity,
                delta: -1,
            },
        ))
        .id();
    commands.entity(adjust_container).add_child(decrease_button);
    let decrease_text = commands
        .spawn((
            Text::new("-"),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_base,
                weight: FontWeight::BOLD,
                ..default()
            },
            TextColor(theme.colors.text_primary_semantic),
        ))
        .id();
    commands.entity(decrease_button).add_child(decrease_text);

    let increase_button = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            FamiliarMaxSoulAdjustButton {
                familiar: familiar.entity,
                delta: 1,
            },
        ))
        .id();
    commands.entity(adjust_container).add_child(increase_button);
    let increase_text = commands
        .spawn((
            Text::new("+"),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_base,
                weight: FontWeight::BOLD,
                ..default()
            },
            TextColor(theme.colors.text_primary_semantic),
        ))
        .id();
    commands.entity(increase_button).add_child(increase_text);

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

pub(super) fn spawn_empty_squad_hint_entity(
    commands: &mut Commands,
    parent_entity: Entity,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) -> Entity {
    let mut result = Entity::PLACEHOLDER;
    commands
        .entity(parent_entity)
        .with_children(|members_parent| {
            result = members_parent
                .spawn((
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
                ))
                .id();
        });
    result
}
