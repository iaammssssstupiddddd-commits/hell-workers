// エンティティリストUIノードのスポーン helpers

use super::models::{
    FamiliarRowViewModel, FamiliarSectionNodes, SoulGender, SoulRowViewModel, StressBucket,
    TaskVisual,
};
use crate::components::{
    EntityListSectionType, FamiliarListItem, FamiliarMaxSoulAdjustButton, MenuAction, MenuButton,
    SectionToggle, SoulListItem, UiTooltip,
};
use crate::setup::UiAssets;
use crate::theme::UiTheme;
use bevy::prelude::*;

// ============================================================
// Node generation helpers (private)
// ============================================================

/// アイコン画像ノード（ImageNode + Node）をスポーンする。
fn spawn_icon(
    parent: &mut ChildSpawnerCommands,
    image: Handle<Image>,
    color: Color,
    size: f32,
    placement: Node,
) {
    parent.spawn((
        ImageNode {
            image,
            color,
            ..default()
        },
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            flex_shrink: 0.0,
            ..placement
        },
    ));
}

/// テキストノード（Text + TextFont + TextColor + Node）をスポーンする。
/// `font`: `None` のときは Bevy デフォルトフォントを使用。
fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    text: String,
    font: Option<Handle<Font>>,
    font_size: f32,
    color: Color,
    weight: FontWeight,
    placement: Node,
) {
    let mut text_font = TextFont {
        font_size: FontSize::Px(font_size),
        weight,
        ..default()
    };
    if let Some(f) = font {
        text_font.font = f.into();
    }
    parent.spawn((
        Text::new(text),
        text_font,
        TextColor(color),
        bevy::text::LineHeight::Px(16.0),
        Node {
            width: Val::Percent(100.0),
            min_width: Val::Px(0.0),
            ..placement
        },
    ));
}

/// familiar の使役数調整ボタン（-/+）をスポーンして `parent` に追加する。
fn spawn_adjust_button(
    commands: &mut Commands,
    parent: Entity,
    familiar: Entity,
    delta: i8,
    label: &str,
    theme: &UiTheme,
    assets: &dyn UiAssets,
) {
    let btn = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            FamiliarMaxSoulAdjustButton { familiar, delta },
        ))
        .id();
    commands.entity(parent).add_child(btn);
    commands.entity(btn).with_children(|b| {
        b.spawn((
            Text::new(label),
            TextFont {
                font: assets.font_ui().clone().into(),
                font_size: FontSize::Px(theme.typography.font_size_base),
                weight: FontWeight::BOLD,
                ..default()
            },
            TextColor(theme.colors.text_primary_semantic),
        ));
    });
}

// ============================================================
// Familiar Section
// ============================================================

pub fn spawn_familiar_section(
    commands: &mut Commands,
    parent_container: Entity,
    familiar: &FamiliarRowViewModel,
    assets: &dyn UiAssets,
    theme: &UiTheme,
) -> FamiliarSectionNodes {
    let fold_icon_handle = if familiar.is_folded {
        assets.icon_arrow_right().clone()
    } else {
        assets.icon_arrow_down().clone()
    };

    let root = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::bottom(Val::Px(2.0)),
            margin: UiRect::top(Val::Px(theme.sizes.familiar_section_margin_top)),
            ..default()
        },))
        .id();
    commands.entity(parent_container).add_child(root);

    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(theme.sizes.header_height),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            display: Display::Grid,
            grid_template_columns: vec![
                GridTrack::px(32.0),
                GridTrack::flex(1.0),
                GridTrack::px(68.0),
                GridTrack::px(48.0),
            ],
            column_gap: Val::Px(4.0),
            padding: UiRect {
                right: Val::Px(4.0),
                top: Val::Px(2.0),
                bottom: Val::Px(2.0),
                ..default()
            },
            ..default()
        })
        .id();
    commands.entity(root).add_child(header);

    let fold_button = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(theme.sizes.fold_button_size),
                grid_column: GridPlacement::start(1),
                grid_row: GridPlacement::start(1),
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
                grid_column: GridPlacement::start(2),
                grid_row: GridPlacement::start(1),
                min_width: Val::Px(0.0),
                min_height: Val::Px(theme.sizes.header_height),
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
    commands.entity(header).with_children(|parent| {
        spawn_entity_focus_button(
            parent,
            familiar.entity,
            assets,
            theme,
            Node {
                grid_column: GridPlacement::start(4),
                grid_row: GridPlacement::start(1),
                ..default()
            },
        );
    });

    let header_text = commands
        .spawn((
            Text::new(familiar.label.clone()),
            TextFont {
                font: assets.font_ui().clone().into(),
                font_size: FontSize::Px(theme.typography.font_size_base),
                weight: FontWeight::SEMIBOLD,
                ..default()
            },
            TextColor(theme.colors.accent_soul),
            Node {
                width: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                ..default()
            },
        ))
        .id();
    commands.entity(familiar_button).add_child(header_text);

    let adjust_container = commands
        .spawn(Node {
            grid_column: GridPlacement::start(3),
            grid_row: GridPlacement::start(1),
            flex_direction: FlexDirection::Row,
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: Val::Px(theme.spacing.margin_small),
            ..default()
        })
        .id();
    commands.entity(header).add_child(adjust_container);

    spawn_adjust_button(
        commands,
        adjust_container,
        familiar.entity,
        -1i8,
        "-",
        theme,
        assets,
    );
    spawn_adjust_button(
        commands,
        adjust_container,
        familiar.entity,
        1i8,
        "+",
        theme,
        assets,
    );

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

pub fn spawn_empty_squad_hint_entity(
    commands: &mut Commands,
    parent_entity: Entity,
    assets: &dyn UiAssets,
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
                        font: assets.font_ui().clone().into(),
                        font_size: FontSize::Px(theme.typography.font_size_item),
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

// ============================================================
// Soul Row
// ============================================================

// Keep direct children in the value-sync order; grid placement owns presentation only.
fn soul_cell(row: i16, column: i16, span: u16) -> Node {
    Node {
        grid_row: GridPlacement::start(row),
        grid_column: GridPlacement::start_span(column, span),
        min_width: Val::Px(0.0),
        align_self: AlignSelf::Center,
        justify_self: JustifySelf::Start,
        ..default()
    }
}

fn get_gender_icon_and_color(
    gender: SoulGender,
    assets: &dyn UiAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match gender {
        SoulGender::Male => (assets.icon_male().clone(), theme.colors.male),
        SoulGender::Female => (assets.icon_female().clone(), theme.colors.female),
    }
}

fn get_task_icon_and_color(
    task: TaskVisual,
    assets: &dyn UiAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match task {
        TaskVisual::Idle => (assets.icon_idle().clone(), theme.colors.idle),
        TaskVisual::Chop => (assets.icon_axe().clone(), theme.colors.chop),
        TaskVisual::Mine => (assets.icon_pick().clone(), theme.colors.mine),
        TaskVisual::GatherDefault => (assets.icon_pick().clone(), theme.colors.gather_default),
        TaskVisual::Haul => (assets.icon_haul().clone(), theme.colors.haul),
        TaskVisual::Build => (assets.icon_pick().clone(), theme.colors.build),
        TaskVisual::HaulToBlueprint => (assets.icon_haul().clone(), theme.colors.haul_to_bp),
        TaskVisual::Water => (assets.icon_haul().clone(), theme.colors.water),
        TaskVisual::GeneratePower => (assets.icon_fatigue().clone(), theme.colors.text_accent),
        TaskVisual::Deconstruct => (assets.icon_hammer().clone(), theme.colors.stress_high),
        TaskVisual::Move => (assets.icon_haul().clone(), theme.colors.build),
        TaskVisual::Refine => (assets.icon_hammer().clone(), theme.colors.build),
        TaskVisual::CollectBone => (
            assets.icon_bone_small().clone(),
            theme.colors.gather_default,
        ),
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

pub fn spawn_soul_list_item(
    parent: &mut ChildSpawnerCommands,
    soul_vm: &SoulRowViewModel,
    assets: &dyn UiAssets,
    left_margin: f32,
    theme: &UiTheme,
) -> Entity {
    let (gender_handle, gender_color) = get_gender_icon_and_color(soul_vm.gender, assets, theme);
    let (task_handle, task_color) = get_task_icon_and_color(soul_vm.task_visual, assets, theme);
    let stress_color = get_stress_color(soul_vm.stress_bucket, theme);
    let dream_color = get_dream_color(soul_vm.dream_empty, theme);

    parent
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(theme.sizes.soul_item_height),
                display: Display::Grid,
                grid_template_columns: vec![
                    GridTrack::px(12.0),
                    GridTrack::px(30.0),
                    GridTrack::px(12.0),
                    GridTrack::px(30.0),
                    GridTrack::flex(1.0),
                    GridTrack::px(12.0),
                    GridTrack::px(36.0),
                    GridTrack::px(48.0),
                ],
                column_gap: Val::Px(2.0),
                row_gap: Val::Px(0.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                border: UiRect::left(Val::Px(0.0)),
                padding: UiRect {
                    left: Val::Px(left_margin.min(8.0) + 4.0),
                    right: Val::Px(4.0),
                    top: Val::Px(1.0),
                    bottom: Val::Px(1.0),
                },
                margin: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(theme.colors.list_item_default),
            BorderColor::all(Color::NONE),
            SoulListItem(soul_vm.entity),
        ))
        .with_children(|item| {
            spawn_icon(
                item,
                gender_handle,
                gender_color,
                theme.sizes.icon_size,
                soul_cell(1, 1, 1),
            );
            spawn_text(
                item,
                soul_vm.name.clone(),
                Some(assets.font_ui().clone()),
                theme.typography.font_size_item,
                stress_color,
                FontWeight::default(),
                soul_cell(1, 2, 4),
            );
            spawn_icon(
                item,
                assets.icon_fatigue().clone(),
                theme.colors.fatigue_icon,
                theme.sizes.icon_size,
                soul_cell(2, 1, 1),
            );
            spawn_text(
                item,
                soul_vm.fatigue_text.clone(),
                Some(assets.font_ui().clone()),
                theme.typography.font_size_small,
                theme.colors.fatigue_text,
                FontWeight::default(),
                soul_cell(2, 2, 1),
            );
            spawn_icon(
                item,
                assets.icon_stress().clone(),
                theme.colors.stress_icon,
                theme.sizes.icon_size,
                soul_cell(2, 3, 1),
            );
            spawn_text(
                item,
                soul_vm.stress_text.clone(),
                Some(assets.font_ui().clone()),
                theme.typography.font_size_small,
                stress_color,
                stress_weight(soul_vm.stress_bucket),
                soul_cell(2, 4, 1),
            );
            // children[6]: dream text
            spawn_text(
                item,
                soul_vm.dream_text.clone(),
                Some(assets.font_ui().clone()),
                theme.typography.font_size_small,
                dream_color,
                FontWeight::default(),
                soul_cell(2, 5, 3),
            );
            spawn_icon(
                item,
                task_handle,
                task_color,
                theme.sizes.icon_size,
                soul_cell(1, 6, 1),
            );
            spawn_text(
                item,
                soul_vm.task_visual.label().into(),
                Some(assets.font_ui().clone()),
                theme.typography.font_size_small,
                task_color,
                FontWeight::default(),
                soul_cell(1, 7, 1),
            );
            spawn_entity_focus_button(
                item,
                soul_vm.entity,
                assets,
                theme,
                Node {
                    grid_row: GridPlacement::start_span(1, 2),
                    ..soul_cell(1, 8, 1)
                },
            );
        })
        .id()
}

pub(crate) fn spawn_entity_focus_button(
    parent: &mut ChildSpawnerCommands,
    target: Entity,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    placement: Node,
) {
    parent
        .spawn((
            Button,
            MenuButton(MenuAction::FocusEntity(target)),
            UiTooltip::new("選択と固定対象を変えずに、現在の場所へカメラを移動します"),
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                width: Val::Px(48.0),
                min_height: Val::Px(32.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..placement
            },
            BackgroundColor(theme.colors.button_default),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new("現地へ"),
                TextFont {
                    font: assets.font_ui().clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_xs),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
        });
}

pub fn spawn_soul_list_item_entity(
    commands: &mut Commands,
    parent_entity: Entity,
    soul_vm: &SoulRowViewModel,
    assets: &dyn UiAssets,
    left_margin: f32,
    theme: &UiTheme,
) -> Entity {
    let mut result = Entity::PLACEHOLDER;
    commands.entity(parent_entity).with_children(|parent| {
        result = spawn_soul_list_item(parent, soul_vm, assets, left_margin, theme);
    });
    result
}
