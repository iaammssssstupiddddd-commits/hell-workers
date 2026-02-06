//! 情報パネル更新

use crate::constants::ESCAPE_STRESS_THRESHOLD;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::entities::familiar::Familiar;
use crate::interface::ui::components::*;
use crate::relationships::CommandedBy;
use crate::systems::jobs::Blueprint;
use crate::systems::soul_ai::idle::escaping::is_escape_threat_close;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::FamiliarSpatialGrid;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

#[derive(SystemParam)]
pub struct InfoPanelParams<'w, 's> {
    pub q_header: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelTaskText>,
            Without<InfoPanelInventoryText>,
        ),
    >,
    pub q_gender_icon: Query<
        'w,
        's,
        (&'static mut ImageNode, &'static mut Node),
        (With<InfoPanelGenderIcon>, Without<InfoPanel>),
    >,
    pub q_stat_motivation: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelStatMotivation>,
            Without<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelTaskText>,
            Without<InfoPanelInventoryText>,
        ),
    >,
    pub q_stat_stress: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelStatStress>,
            Without<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelTaskText>,
            Without<InfoPanelInventoryText>,
        ),
    >,
    pub q_stat_fatigue: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelStatFatigue>,
            Without<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelTaskText>,
            Without<InfoPanelInventoryText>,
        ),
    >,
    pub q_task: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelTaskText>,
            Without<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelInventoryText>,
        ),
    >,
    pub q_inv: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelInventoryText>,
            Without<InfoPanelHeader>,
            Without<InfoPanelCommonText>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelTaskText>,
        ),
    >,
    pub q_common: Query<
        'w,
        's,
        &'static mut Text,
        (
            With<InfoPanelCommonText>,
            Without<InfoPanelHeader>,
            Without<InfoPanelStatMotivation>,
            Without<InfoPanelStatStress>,
            Without<InfoPanelStatFatigue>,
            Without<InfoPanelTaskText>,
            Without<InfoPanelInventoryText>,
        ),
    >,
}

pub fn info_panel_system(
    game_assets: Res<crate::assets::GameAssets>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    mut q_panel: Query<&mut Node, (With<InfoPanel>, Without<InfoPanelGenderIcon>)>,
    mut params: InfoPanelParams,
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
    let Ok(mut panel_node) = q_panel.single_mut() else {
        return;
    };
    panel_node.display = Display::None;

    // Reset common text and gender icon
    if let Ok(mut common) = params.q_common.single_mut() {
        common.0 = "".to_string();
    }
    if let Ok((_icon, mut node)) = params.q_gender_icon.single_mut() {
        node.display = Display::None;
    }

    if let Some(entity) = selected.0 {
        if let Ok((soul, task, transform, idle, under_command, inventory_opt, identity_opt)) =
            q_souls.get(entity)
        {
            panel_node.display = Display::Flex;

            if let Ok(mut header) = params.q_header.single_mut() {
                header.0 = if let Some(identity) = identity_opt {
                    identity.name.clone()
                } else {
                    "Damned Soul".to_string()
                };
            }

            if let Some(identity) = identity_opt {
                if let Ok((mut icon, mut node)) = params.q_gender_icon.single_mut() {
                    node.display = Display::Flex;
                    icon.image = match identity.gender {
                        crate::entities::damned_soul::Gender::Male => game_assets.icon_male.clone(),
                        crate::entities::damned_soul::Gender::Female => {
                            game_assets.icon_female.clone()
                        }
                    };
                }
            }

            if let Ok(mut t) = params.q_stat_motivation.single_mut() {
                t.0 = format!("Motivation: {:.0}%", soul.motivation * 100.0);
            }
            if let Ok(mut t) = params.q_stat_stress.single_mut() {
                t.0 = format!("Stress: {:.0}%", soul.stress * 100.0);
            }
            if let Ok(mut t) = params.q_stat_fatigue.single_mut() {
                t.0 = format!("Fatigue: {:.0}%", soul.fatigue * 100.0);
            }

            let task_str = match task {
                AssignedTask::None => "Idle".to_string(),
                AssignedTask::Gather(data) => format!("Gather ({:?})", data.phase),
                AssignedTask::Haul(data) => format!("Haul ({:?})", data.phase),
                AssignedTask::HaulToBlueprint(data) => format!("HaulToBp ({:?})", data.phase),
                AssignedTask::Build(data) => format!("Build ({:?})", data.phase),
                AssignedTask::GatherWater(data) => format!("GatherWater ({:?})", data.phase),
                AssignedTask::CollectSand(data) => format!("CollectSand ({:?})", data.phase),
                AssignedTask::Refine(data) => format!("Refine ({:?})", data.phase),
                AssignedTask::HaulToMixer(data) => format!("HaulToMixer ({:?})", data.phase),
                AssignedTask::HaulWaterToMixer(data) => {
                    format!("HaulWaterToMixer ({:?})", data.phase)
                }
            };
            if let Ok(mut t) = params.q_task.single_mut() {
                t.0 = format!("Task: {}", task_str);
            }

            let escape_threat_close = is_escape_threat_close(
                transform.translation.truncate(),
                &familiar_grid,
                &q_familiars_escape,
            );
            let escape_allowed = under_command.is_none()
                && idle.behavior != IdleBehavior::ExhaustedGathering
                && soul.stress > ESCAPE_STRESS_THRESHOLD
                && escape_threat_close;
            if let Ok(mut common) = params.q_common.single_mut() {
                common.0 = format!(
                    "Idle: {:?}\nEscape: {}\n- stress_ok: {}\n- threat_close: {}\n- commanded: {}\n- exhausted: {}",
                    idle.behavior,
                    if escape_allowed {
                        "eligible"
                    } else {
                        "blocked"
                    },
                    soul.stress > ESCAPE_STRESS_THRESHOLD,
                    escape_threat_close,
                    under_command.is_some(),
                    idle.behavior == IdleBehavior::ExhaustedGathering
                );
            }

            let inv_str = if let Some(crate::systems::logistics::Inventory(Some(item_entity))) =
                inventory_opt
            {
                if let Ok(item) = q_items.get(*item_entity) {
                    format!("Carrying: {:?}", item.0)
                } else {
                    format!("Carrying: Entity {:?}", item_entity)
                }
            } else {
                "Carrying: None".to_string()
            };
            if let Ok(mut t) = params.q_inv.single_mut() {
                t.0 = inv_str;
            }
        } else if let Ok(mut common) = params.q_common.single_mut() {
            if let Ok(bp) = q_blueprints.get(entity) {
                panel_node.display = Display::Flex;
                if let Ok(mut header) = params.q_header.single_mut() {
                    header.0 = "Blueprint Info".to_string();
                }
                common.0 = format!("Type: {:?}\nProgress: {:.0}%", bp.kind, bp.progress * 100.0);
            } else if let Ok((familiar, op)) = q_familiars.get(entity) {
                panel_node.display = Display::Flex;
                if let Ok(mut header) = params.q_header.single_mut() {
                    header.0 = familiar.name.clone();
                }
                common.0 = format!(
                    "Type: {:?}\nRange: {:.0} tiles\nFatigue Threshold: {:.0}%",
                    familiar.familiar_type,
                    familiar.command_radius / 16.0,
                    op.fatigue_threshold * 100.0
                );
            } else if let Ok(item) = q_items.get(entity) {
                panel_node.display = Display::Flex;
                if let Ok(mut header) = params.q_header.single_mut() {
                    header.0 = "Resource Item".to_string();
                }
                common.0 = format!("Type: {:?}", item.0);
            } else if let Ok(_) = q_trees.get(entity) {
                panel_node.display = Display::Flex;
                if let Ok(mut header) = params.q_header.single_mut() {
                    header.0 = "Tree".to_string();
                }
                common.0 = "Natural resource: Wood".to_string();
            } else if let Ok(_) = q_rocks.get(entity) {
                panel_node.display = Display::Flex;
                if let Ok(mut header) = params.q_header.single_mut() {
                    header.0 = "Rock".to_string();
                }
                common.0 = "Natural resource: Stone".to_string();
            }
        }
    }
}
