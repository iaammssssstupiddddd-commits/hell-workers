use crate::constants::ESCAPE_STRESS_THRESHOLD;
use crate::entities::damned_soul::{DamnedSoul, Gender, IdleBehavior, IdleState};
use crate::entities::familiar::Familiar;
use crate::relationships::CommandedBy;
use crate::relationships::TaskWorkers;
use crate::systems::jobs::Blueprint;
use crate::systems::soul_ai::idle::escaping::is_escape_threat_close;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::FamiliarSpatialGrid;
use bevy::prelude::*;

#[derive(Clone, PartialEq)]
pub struct SoulInspectionFields {
    pub gender: Option<Gender>,
    pub motivation: String,
    pub stress: String,
    pub fatigue: String,
    pub task: String,
    pub inventory: String,
    pub common: String,
}

#[derive(Clone, PartialEq)]
pub struct EntityInspectionModel {
    pub header: String,
    pub common_text: String,
    pub tooltip_lines: Vec<String>,
    pub soul: Option<SoulInspectionFields>,
}

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

pub fn build_entity_inspection_model(
    entity: Entity,
    q_souls: &Query<(
        &DamnedSoul,
        &AssignedTask,
        &Transform,
        &IdleState,
        Option<&CommandedBy>,
        Option<&crate::systems::logistics::Inventory>,
        Option<&crate::entities::damned_soul::SoulIdentity>,
    )>,
    q_blueprints: &Query<&Blueprint>,
    q_familiars: &Query<(&Familiar, &crate::entities::familiar::FamiliarOperation)>,
    q_familiars_escape: &Query<(&Transform, &Familiar)>,
    familiar_grid: &FamiliarSpatialGrid,
    q_items: &Query<&crate::systems::logistics::ResourceItem>,
    q_trees: &Query<&crate::systems::jobs::Tree>,
    q_rocks: &Query<&crate::systems::jobs::Rock>,
    q_designations: &Query<(
        &crate::systems::jobs::Designation,
        Option<&crate::systems::jobs::IssuedBy>,
        Option<&TaskWorkers>,
    )>,
    q_buildings: &Query<(
        &crate::systems::jobs::Building,
        Option<&crate::systems::logistics::Stockpile>,
        Option<&crate::relationships::StoredItems>,
        Option<&crate::systems::jobs::MudMixerStorage>,
    )>,
) -> Option<EntityInspectionModel> {
    let mut tooltip_lines = Vec::new();
    let mut header = String::new();
    let mut common_lines = Vec::new();
    let mut soul_fields = None;

    if let Ok((soul, task, transform, idle, under_command, inventory_opt, identity_opt)) =
        q_souls.get(entity)
    {
        let name = identity_opt
            .map(|i| i.name.clone())
            .unwrap_or("Damned Soul".to_string());
        let motivation = format!("Motivation: {:.0}%", soul.motivation * 100.0);
        let stress = format!("Stress: {:.0}%", soul.stress * 100.0);
        let fatigue = format!("Fatigue: {:.0}%", soul.fatigue * 100.0);
        let task_str = format!("Task: {}", format_task_str(task));
        let inventory = format_inventory_str(inventory_opt, q_items);
        let common = format_escape_info(
            soul,
            transform,
            idle,
            under_command,
            familiar_grid,
            q_familiars_escape,
        );

        header = name.clone();
        tooltip_lines.push(format!("Soul: {}", name));
        tooltip_lines.push(motivation.clone());
        tooltip_lines.push(stress.clone());
        tooltip_lines.push(task_str.clone());
        tooltip_lines.push(inventory.clone());
        common_lines.push(common.clone());

        soul_fields = Some(SoulInspectionFields {
            gender: identity_opt.map(|identity| identity.gender),
            motivation,
            stress,
            fatigue,
            task: task_str,
            inventory,
            common,
        });
    } else if let Ok(bp) = q_blueprints.get(entity) {
        header = "Blueprint Info".to_string();
        common_lines.push(format!("Type: {:?}", bp.kind));
        common_lines.push(format!("Progress: {:.0}%", bp.progress * 100.0));
        tooltip_lines.push("Target: Blueprint".to_string());
    } else if let Ok((familiar, op)) = q_familiars.get(entity) {
        header = familiar.name.clone();
        common_lines.push(format!("Type: {:?}", familiar.familiar_type));
        common_lines.push(format!("Range: {:.0} tiles", familiar.command_radius / 16.0));
        common_lines.push(format!(
            "Fatigue Threshold: {:.0}%",
            op.fatigue_threshold * 100.0
        ));
        tooltip_lines.push(format!("Familiar: {}", familiar.name));
    } else if let Ok(item) = q_items.get(entity) {
        header = "Resource Item".to_string();
        let line = format!("Type: {:?}", item.0);
        common_lines.push(line.clone());
        tooltip_lines.push(format!("Item: {:?}", item.0));
    } else if q_trees.get(entity).is_ok() {
        header = "Tree".to_string();
        common_lines.push("Natural resource: Wood".to_string());
        tooltip_lines.push("Target: Tree".to_string());
    } else if q_rocks.get(entity).is_ok() {
        header = "Rock".to_string();
        common_lines.push("Natural resource: Stone".to_string());
        tooltip_lines.push("Target: Rock".to_string());
    }

    if let Ok((building, stockpile_opt, stored_items_opt, mixer_storage_opt)) = q_buildings.get(entity) {
        if header.is_empty() {
            header = format!("Building: {:?}", building.kind);
        }

        let mut building_info = format!("Building: {:?}", building.kind);
        if building.is_provisional {
            building_info.push_str(" (Provisional)");
        }
        if let Some(stockpile) = stockpile_opt {
            let current = stored_items_opt.map(|si| si.len()).unwrap_or(0);
            let resource_name = stockpile
                .resource_type
                .map(|r| format!("{:?}", r))
                .unwrap_or_else(|| "Items".to_string());
            building_info = format!(
                "{}: {} ({}/{})",
                building_info, resource_name, current, stockpile.capacity
            );
        }
        tooltip_lines.push(building_info.clone());
        common_lines.push(building_info);

        if let Some(storage) = mixer_storage_opt {
            let water_count = match (stockpile_opt, stored_items_opt) {
                (Some(stockpile), Some(stored_items))
                    if stockpile.resource_type
                        == Some(crate::systems::logistics::ResourceType::Water) =>
                {
                    stored_items.len()
                }
                _ => 0,
            };
            let storage_line = format!(
                "Storage: Sand {}, Rock {}, Water {}",
                storage.sand, storage.rock, water_count
            );
            tooltip_lines.push(storage_line.clone());
            common_lines.push(storage_line);
        }
    }

    if let Ok((des, issued_by_opt, task_workers_opt)) = q_designations.get(entity) {
        let task_line = format!("Task: {:?}", des.work_type);
        tooltip_lines.push(task_line.clone());
        common_lines.push(task_line);

        if let Some(issued_by) = issued_by_opt {
            if let Ok((fam, _)) = q_familiars.get(issued_by.0) {
                let line = format!("Issued by: {}", fam.name);
                tooltip_lines.push(line.clone());
                common_lines.push(line);
            }
        }

        if let Some(workers) = task_workers_opt {
            let worker_names: Vec<String> = workers
                .iter()
                .filter_map(|&soul_entity| {
                    q_souls.get(soul_entity).ok().map(|(_, _, _, _, _, _, identity_opt)| {
                        identity_opt
                            .map(|i| i.name.clone())
                            .unwrap_or("Unknown".to_string())
                    })
                })
                .collect();

            if !worker_names.is_empty() {
                let line = format!("Assigned to: {}", worker_names.join(", "));
                tooltip_lines.push(line.clone());
                common_lines.push(line);
            }
        }
    }

    if header.is_empty() && tooltip_lines.is_empty() {
        return None;
    }

    if tooltip_lines.is_empty() {
        tooltip_lines.push(header.clone());
    }

    Some(EntityInspectionModel {
        header,
        common_text: common_lines.join("\n"),
        tooltip_lines,
        soul: soul_fields,
    })
}
