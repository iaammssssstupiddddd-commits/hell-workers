//! 情報パネル - 動的spawn/despawn方式
//!
//! 選択変更時にパネルをdespawnし、エンティティタイプに応じた新パネルをspawnする。

use crate::constants::ESCAPE_STRESS_THRESHOLD;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::entities::familiar::Familiar;
use crate::interface::ui::components::{InfoPanel, UiInputBlocker, UiSlot};
use crate::interface::ui::theme::UiTheme;
use crate::relationships::CommandedBy;
use crate::systems::jobs::Blueprint;
use crate::systems::soul_ai::idle::escaping::is_escape_threat_close;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::FamiliarSpatialGrid;
use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient, RelativeCursorPosition};

// ============================================================
// Spawn helpers
// ============================================================

fn spawn_panel_root<'a>(commands: &'a mut Commands, theme: &UiTheme) -> EntityCommands<'a> {
    commands.spawn((
        Node {
            width: Val::Px(theme.sizes.info_panel_width),
            height: Val::Auto,
            position_type: PositionType::Absolute,
            right: Val::Px(theme.spacing.panel_margin_x),
            top: Val::Px(theme.spacing.panel_top),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(theme.spacing.panel_padding)),
            ..default()
        },
        BackgroundGradient::from(LinearGradient {
            angle: 0.0,
            stops: vec![
                ColorStop::new(theme.panels.info_panel.top, Val::Percent(0.0)),
                ColorStop::new(theme.panels.info_panel.bottom, Val::Percent(100.0)),
            ],
            ..default()
        }),
        RelativeCursorPosition::default(),
        UiInputBlocker,
        InfoPanel,
    ))
}

fn spawn_soul_info_panel(
    commands: &mut Commands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
    header: &str,
    gender_image: Option<Handle<Image>>,
    motivation: &str,
    stress: &str,
    fatigue: &str,
    task: &str,
    inventory: &str,
    common: &str,
) {
    spawn_panel_root(commands, theme).with_children(|parent| {
        // Header row
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(5.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Text::new(header),
                    TextFont {
                        font: game_assets.font_ui.clone(),
                        font_size: theme.typography.font_size_title,
                        ..default()
                    },
                    TextColor(theme.colors.text_accent),
                    UiSlot::Header,
                ));
                if let Some(image) = gender_image {
                    row.spawn((
                        ImageNode::new(image),
                        Node {
                            width: Val::Px(16.0),
                            height: Val::Px(16.0),
                            margin: UiRect::left(Val::Px(8.0)),
                            ..default()
                        },
                        UiSlot::GenderIcon,
                    ));
                } else {
                    row.spawn((
                        ImageNode::default(),
                        Node {
                            width: Val::Px(16.0),
                            height: Val::Px(16.0),
                            margin: UiRect::left(Val::Px(8.0)),
                            display: Display::None,
                            ..default()
                        },
                        UiSlot::GenderIcon,
                    ));
                }
            });

        // Soul Stats Column
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|col| {
                col.spawn((
                    Text::new(motivation),
                    TextFont {
                        font: game_assets.font_ui.clone(),
                        font_size: theme.typography.font_size_small,
                        ..default()
                    },
                    UiSlot::StatMotivation,
                ));
                // Stress row
                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        ImageNode::new(game_assets.icon_stress.clone()),
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            margin: UiRect::right(Val::Px(4.0)),
                            ..default()
                        },
                    ));
                    row.spawn((
                        Text::new(stress),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_small,
                            ..default()
                        },
                        UiSlot::StatStress,
                    ));
                });
                // Fatigue row
                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        ImageNode::new(game_assets.icon_fatigue.clone()),
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            margin: UiRect::right(Val::Px(4.0)),
                            ..default()
                        },
                    ));
                    row.spawn((
                        Text::new(fatigue),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_small,
                            ..default()
                        },
                        UiSlot::StatFatigue,
                    ));
                });
                // Task row
                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    margin: UiRect::top(Val::Px(5.0)),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Text::new(task),
                        TextFont {
                            font: game_assets.font_ui.clone(),
                            font_size: theme.typography.font_size_small,
                            ..default()
                        },
                        UiSlot::TaskText,
                    ));
                });
                // Inventory
                col.spawn((
                    Text::new(inventory),
                    TextFont {
                        font: game_assets.font_ui.clone(),
                        font_size: theme.typography.font_size_small,
                        ..default()
                    },
                    UiSlot::InventoryText,
                ));
            });

        // Common text
        parent.spawn((
            Text::new(common),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_item,
                ..default()
            },
            TextColor(theme.colors.text_primary),
            UiSlot::CommonText,
        ));
    });
}

fn spawn_simple_info_panel(
    commands: &mut Commands,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
    header: &str,
    common: &str,
) {
    spawn_panel_root(commands, theme).with_children(|parent| {
        // Header
        parent
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(5.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Text::new(header),
                    TextFont {
                        font: game_assets.font_ui.clone(),
                        font_size: theme.typography.font_size_title,
                        ..default()
                    },
                    TextColor(theme.colors.text_accent),
                    UiSlot::Header,
                ));
            });

        // Common text
        parent.spawn((
            Text::new(common),
            TextFont {
                font: game_assets.font_ui.clone(),
                font_size: theme.typography.font_size_item,
                ..default()
            },
            TextColor(theme.colors.text_primary),
            UiSlot::CommonText,
        ));
    });
}

// ============================================================
// Data computation helpers
// ============================================================

fn format_task_str(task: &AssignedTask) -> String {
    match task {
        AssignedTask::None => "Idle".to_string(),
        AssignedTask::Gather(data) => format!("Gather ({:?})", data.phase),
        AssignedTask::Haul(data) => format!("Haul ({:?})", data.phase),
        AssignedTask::HaulToBlueprint(data) => format!("HaulToBp ({:?})", data.phase),
        AssignedTask::Build(data) => format!("Build ({:?})", data.phase),
        AssignedTask::GatherWater(data) => format!("GatherWater ({:?})", data.phase),
        AssignedTask::CollectSand(data) => format!("CollectSand ({:?})", data.phase),
        AssignedTask::Refine(data) => format!("Refine ({:?})", data.phase),
        AssignedTask::HaulToMixer(data) => format!("HaulToMixer ({:?})", data.phase),
        AssignedTask::HaulWaterToMixer(data) => format!("HaulWaterToMixer ({:?})", data.phase),
    }
}

fn format_inventory_str(
    inventory_opt: Option<&crate::systems::logistics::Inventory>,
    q_items: &Query<&crate::systems::logistics::ResourceItem>,
) -> String {
    if let Some(crate::systems::logistics::Inventory(Some(item_entity))) = inventory_opt {
        if let Ok(item) = q_items.get(*item_entity) {
            format!("Carrying: {:?}", item.0)
        } else {
            format!("Carrying: Entity {:?}", item_entity)
        }
    } else {
        "Carrying: None".to_string()
    }
}

fn format_escape_info(
    soul: &DamnedSoul,
    transform: &Transform,
    idle: &IdleState,
    under_command: Option<&CommandedBy>,
    familiar_grid: &FamiliarSpatialGrid,
    q_familiars_escape: &Query<(&Transform, &Familiar)>,
) -> String {
    let escape_threat_close = is_escape_threat_close(
        transform.translation.truncate(),
        familiar_grid,
        q_familiars_escape,
    );
    let escape_allowed = under_command.is_none()
        && idle.behavior != IdleBehavior::ExhaustedGathering
        && soul.stress > ESCAPE_STRESS_THRESHOLD
        && escape_threat_close;
    format!(
        "Idle: {:?}\nEscape: {}\n- stress_ok: {}\n- threat_close: {}\n- commanded: {}\n- exhausted: {}",
        idle.behavior,
        if escape_allowed { "eligible" } else { "blocked" },
        soul.stress > ESCAPE_STRESS_THRESHOLD,
        escape_threat_close,
        under_command.is_some(),
        idle.behavior == IdleBehavior::ExhaustedGathering
    )
}

fn gender_image_handle(
    identity_opt: Option<&crate::entities::damned_soul::SoulIdentity>,
    game_assets: &crate::assets::GameAssets,
) -> Option<Handle<Image>> {
    identity_opt.map(|identity| match identity.gender {
        crate::entities::damned_soul::Gender::Male => game_assets.icon_male.clone(),
        crate::entities::damned_soul::Gender::Female => game_assets.icon_female.clone(),
    })
}

fn compute_simple_panel_data(
    entity: Entity,
    q_blueprints: &Query<&Blueprint>,
    q_familiars: &Query<(&Familiar, &crate::entities::familiar::FamiliarOperation)>,
    q_items: &Query<&crate::systems::logistics::ResourceItem>,
    q_trees: &Query<&crate::systems::jobs::Tree>,
    q_rocks: &Query<&crate::systems::jobs::Rock>,
) -> Option<(String, String)> {
    if let Ok(bp) = q_blueprints.get(entity) {
        Some((
            "Blueprint Info".to_string(),
            format!("Type: {:?}\nProgress: {:.0}%", bp.kind, bp.progress * 100.0),
        ))
    } else if let Ok((familiar, op)) = q_familiars.get(entity) {
        Some((
            familiar.name.clone(),
            format!(
                "Type: {:?}\nRange: {:.0} tiles\nFatigue Threshold: {:.0}%",
                familiar.familiar_type,
                familiar.command_radius / 16.0,
                op.fatigue_threshold * 100.0
            ),
        ))
    } else if let Ok(item) = q_items.get(entity) {
        Some(("Resource Item".to_string(), format!("Type: {:?}", item.0)))
    } else if q_trees.get(entity).is_ok() {
        Some(("Tree".to_string(), "Natural resource: Wood".to_string()))
    } else if q_rocks.get(entity).is_ok() {
        Some(("Rock".to_string(), "Natural resource: Stone".to_string()))
    } else {
        None
    }
}

// ============================================================
// System
// ============================================================

pub fn info_panel_system(
    mut commands: Commands,
    game_assets: Res<crate::assets::GameAssets>,
    theme: Res<UiTheme>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    q_existing: Query<Entity, With<InfoPanel>>,
    mut q_slots: Query<(&UiSlot, &mut Text)>,
    mut q_gender: Query<(&UiSlot, &mut ImageNode, &mut Node), Without<InfoPanel>>,
    q_souls: Query<(
        &DamnedSoul,
        &AssignedTask,
        &Transform,
        &IdleState,
        Option<&CommandedBy>,
        Option<&crate::systems::logistics::Inventory>,
        Option<&crate::entities::damned_soul::SoulIdentity>,
    )>,
    q_blueprints: Query<&Blueprint>,
    q_familiars: Query<(&Familiar, &crate::entities::familiar::FamiliarOperation)>,
    q_familiars_escape: Query<(&Transform, &Familiar)>,
    familiar_grid: Res<FamiliarSpatialGrid>,
    q_items: Query<&crate::systems::logistics::ResourceItem>,
    q_trees: Query<&crate::systems::jobs::Tree>,
    q_rocks: Query<&crate::systems::jobs::Rock>,
) {
    // Selection changed: despawn old panel and spawn new one with initial data
    if selected.is_changed() {
        for e in q_existing.iter() {
            commands.entity(e).despawn();
        }

        let Some(entity) = selected.0 else { return };

        if let Ok((soul, task, transform, idle, under_command, inventory_opt, identity_opt)) =
            q_souls.get(entity)
        {
            let header = identity_opt
                .map(|i| i.name.clone())
                .unwrap_or("Damned Soul".to_string());
            let gi = gender_image_handle(identity_opt, &game_assets);
            let motivation = format!("Motivation: {:.0}%", soul.motivation * 100.0);
            let stress = format!("Stress: {:.0}%", soul.stress * 100.0);
            let fatigue = format!("Fatigue: {:.0}%", soul.fatigue * 100.0);
            let task_s = format!("Task: {}", format_task_str(task));
            let inv = format_inventory_str(inventory_opt, &q_items);
            let common = format_escape_info(
                soul,
                transform,
                idle,
                under_command,
                &familiar_grid,
                &q_familiars_escape,
            );
            spawn_soul_info_panel(
                &mut commands,
                &game_assets,
                &theme,
                &header,
                gi,
                &motivation,
                &stress,
                &fatigue,
                &task_s,
                &inv,
                &common,
            );
        } else if let Some((header, common)) = compute_simple_panel_data(
            entity,
            &q_blueprints,
            &q_familiars,
            &q_items,
            &q_trees,
            &q_rocks,
        ) {
            spawn_simple_info_panel(&mut commands, &game_assets, &theme, &header, &common);
        }
        return;
    }

    // Update existing panel data
    let Some(entity) = selected.0 else { return };

    if let Ok((soul, task, transform, idle, under_command, inventory_opt, identity_opt)) =
        q_souls.get(entity)
    {
        let header = identity_opt
            .map(|i| i.name.clone())
            .unwrap_or("Damned Soul".to_string());
        let motivation = format!("Motivation: {:.0}%", soul.motivation * 100.0);
        let stress = format!("Stress: {:.0}%", soul.stress * 100.0);
        let fatigue = format!("Fatigue: {:.0}%", soul.fatigue * 100.0);
        let task_s = format!("Task: {}", format_task_str(task));
        let inv = format_inventory_str(inventory_opt, &q_items);
        let common = format_escape_info(
            soul,
            transform,
            idle,
            under_command,
            &familiar_grid,
            &q_familiars_escape,
        );

        // Update gender icon
        if let Some(identity) = identity_opt {
            for (slot, mut icon, mut node) in q_gender.iter_mut() {
                if *slot == UiSlot::GenderIcon {
                    node.display = Display::Flex;
                    icon.image = match identity.gender {
                        crate::entities::damned_soul::Gender::Male => {
                            game_assets.icon_male.clone()
                        }
                        crate::entities::damned_soul::Gender::Female => {
                            game_assets.icon_female.clone()
                        }
                    };
                }
            }
        }

        for (slot, mut text) in q_slots.iter_mut() {
            match slot {
                UiSlot::Header => text.0 = header.clone(),
                UiSlot::StatMotivation => text.0 = motivation.clone(),
                UiSlot::StatStress => text.0 = stress.clone(),
                UiSlot::StatFatigue => text.0 = fatigue.clone(),
                UiSlot::TaskText => text.0 = task_s.clone(),
                UiSlot::CommonText => text.0 = common.clone(),
                UiSlot::InventoryText => text.0 = inv.clone(),
                _ => {}
            }
        }
    } else if let Some((header_str, common_str)) = compute_simple_panel_data(
        entity,
        &q_blueprints,
        &q_familiars,
        &q_items,
        &q_trees,
        &q_rocks,
    ) {
        for (slot, mut text) in q_slots.iter_mut() {
            match slot {
                UiSlot::Header => text.0 = header_str.clone(),
                UiSlot::CommonText => text.0 = common_str.clone(),
                _ => {}
            }
        }
    }
}
