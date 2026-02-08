use super::{
    EntityInspectionQuery, InspectionAccumulator, SoulInspectionFields, format_escape_info,
    format_inventory_str, format_task_str,
};
use bevy::prelude::*;

impl EntityInspectionQuery<'_, '_> {
    pub(super) fn build_soul_model(
        &self,
        entity: Entity,
        model: &mut InspectionAccumulator,
    ) -> bool {
        let Ok((soul, task, transform, idle, under_command, inventory_opt, identity_opt)) =
            self.q_souls.get(entity)
        else {
            return false;
        };

        let name = identity_opt
            .map(|identity| identity.name.clone())
            .unwrap_or("Damned Soul".to_string());
        let motivation = format!("Motivation: {:.0}%", soul.motivation * 100.0);
        let stress = format!("Stress: {:.0}%", soul.stress * 100.0);
        let fatigue = format!("Fatigue: {:.0}%", soul.fatigue * 100.0);
        let task_str = format!("Task: {}", format_task_str(task));
        let inventory = format_inventory_str(inventory_opt, &self.q_items);
        let common = format_escape_info(
            soul,
            transform,
            idle,
            under_command,
            &self.familiar_grid,
            &self.q_familiars_escape,
        );

        model.header = name.clone();
        model.push_tooltip(format!("Soul: {}", name));
        model.push_tooltip(motivation.clone());
        model.push_tooltip(stress.clone());
        model.push_tooltip(task_str.clone());
        model.push_tooltip(inventory.clone());
        model.push_common(common.clone());

        model.soul_fields = Some(SoulInspectionFields {
            gender: identity_opt.map(|identity| identity.gender),
            motivation,
            stress,
            fatigue,
            task: task_str,
            inventory,
            common,
        });

        true
    }

    pub(super) fn build_blueprint_model(
        &self,
        entity: Entity,
        model: &mut InspectionAccumulator,
    ) -> bool {
        let Ok(bp) = self.q_blueprints.get(entity) else {
            return false;
        };

        model.header = "Blueprint Info".to_string();
        model.push_common(format!("Type: {:?}", bp.kind));
        model.push_common(format!("Progress: {:.0}%", bp.progress * 100.0));
        model.push_tooltip("Target: Blueprint".to_string());
        true
    }

    pub(super) fn build_familiar_model(
        &self,
        entity: Entity,
        model: &mut InspectionAccumulator,
    ) -> bool {
        let Ok((familiar, op)) = self.q_familiars.get(entity) else {
            return false;
        };

        model.header = familiar.name.clone();
        model.push_common(format!("Type: {:?}", familiar.familiar_type));
        model.push_common(format!(
            "Range: {:.0} tiles",
            familiar.command_radius / 16.0
        ));
        model.push_common(format!(
            "Fatigue Threshold: {:.0}%",
            op.fatigue_threshold * 100.0
        ));
        model.push_tooltip(format!("Familiar: {}", familiar.name));
        true
    }

    pub(super) fn build_item_model(
        &self,
        entity: Entity,
        model: &mut InspectionAccumulator,
    ) -> bool {
        let Ok(item) = self.q_items.get(entity) else {
            return false;
        };

        model.header = "Resource Item".to_string();
        let line = format!("Type: {:?}", item.0);
        model.push_common(line.clone());
        model.push_tooltip(format!("Item: {:?}", item.0));
        true
    }

    pub(super) fn build_tree_model(
        &self,
        entity: Entity,
        model: &mut InspectionAccumulator,
    ) -> bool {
        if self.q_trees.get(entity).is_err() {
            return false;
        }

        model.header = "Tree".to_string();
        model.push_common("Natural resource: Wood".to_string());
        model.push_tooltip("Target: Tree".to_string());
        true
    }

    pub(super) fn build_rock_model(
        &self,
        entity: Entity,
        model: &mut InspectionAccumulator,
    ) -> bool {
        if self.q_rocks.get(entity).is_err() {
            return false;
        }

        model.header = "Rock".to_string();
        model.push_common("Natural resource: Stone".to_string());
        model.push_tooltip("Target: Rock".to_string());
        true
    }

    pub(super) fn append_building_model(&self, entity: Entity, model: &mut InspectionAccumulator) {
        let Ok((building, stockpile_opt, stored_items_opt, mixer_storage_opt)) =
            self.q_buildings.get(entity)
        else {
            return;
        };

        if model.header.is_empty() {
            model.header = format!("Building: {:?}", building.kind);
        }

        let mut building_info = format!("Building: {:?}", building.kind);
        if building.is_provisional {
            building_info.push_str(" (Provisional)");
        }
        if let Some(stockpile) = stockpile_opt {
            let current = stored_items_opt
                .map(|stored_items| stored_items.len())
                .unwrap_or(0);
            let resource_name = stockpile
                .resource_type
                .map(|resource| format!("{:?}", resource))
                .unwrap_or_else(|| "Items".to_string());
            building_info = format!(
                "{}: {} ({}/{})",
                building_info, resource_name, current, stockpile.capacity
            );
        }
        model.push_tooltip(building_info.clone());

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
            model.push_tooltip(storage_line.clone());
        }
    }

    pub(super) fn append_designation_model(
        &self,
        entity: Entity,
        model: &mut InspectionAccumulator,
    ) {
        let Ok((designation, issued_by_opt, task_workers_opt)) = self.q_designations.get(entity)
        else {
            return;
        };

        let task_line = format!("Task: {:?}", designation.work_type);
        model.push_tooltip(task_line.clone());

        if let Some(issued_by) = issued_by_opt
            && let Ok((familiar, _)) = self.q_familiars.get(issued_by.0)
        {
            let line = format!("Issued by: {}", familiar.name);
            model.push_tooltip(line.clone());
        }

        if let Some(workers) = task_workers_opt {
            let worker_names: Vec<String> = workers
                .iter()
                .filter_map(|&soul_entity| {
                    self.q_souls
                        .get(soul_entity)
                        .ok()
                        .map(|(_, _, _, _, _, _, identity_opt)| {
                            identity_opt
                                .map(|identity| identity.name.clone())
                                .unwrap_or("Unknown".to_string())
                        })
                })
                .collect();

            if !worker_names.is_empty() {
                let line = format!("Assigned to: {}", worker_names.join(", "));
                model.push_tooltip(line.clone());
            }
        }
    }
}
