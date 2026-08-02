use super::{
    EntityInspectionQuery, InspectionAccumulator, SoulInspectionFields, format_escape_info,
    format_inventory_str, format_task_str,
};
use crate::entities::damned_soul::Gender;
use bevy::prelude::*;
use hw_core::constants::{DREAM_DRAIN_RATE_REST, DREAM_MAX, MUD_MIXER_MUD_CAPACITY};
use hw_energy::{
    PowerAllocationMode, PowerGridAllocationSummary, PowerPriority, PowerShedReason,
    PowerSupplyState, SOUL_SPA_MAX_ACTIVE_SLOTS, SoulSpaPhase,
};
use hw_logistics::{StockpilePolicyState, derive_stockpile_policy_state};
use hw_ui::models::inspection::{
    InspectionSoulGender, PowerInspectionFields, SoulSpaInspectionFields, StockpileInspectionFields,
};
use hw_ui::power::{
    PowerAllocationModeValue, PowerInspectionRole, PowerPriorityValue, PowerShedReasonValue,
    PowerSupplyStateValue,
};

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
        let dream = format!("Dream: {:.0}/{:.0}", soul.dream, DREAM_MAX);
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
        model.push_tooltip(dream.clone());
        model.push_tooltip(task_str.clone());
        model.push_tooltip(inventory.clone());
        model.push_common(common.clone());

        model.soul_fields = Some(SoulInspectionFields {
            gender: identity_opt.map(|identity| match identity.gender {
                Gender::Male => InspectionSoulGender::Male,
                Gender::Female => InspectionSoulGender::Female,
            }),
            motivation,
            stress,
            fatigue,
            dream,
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
        let threshold = if op.recruit_fatigue_threshold().is_some() {
            format!("{:.0}%", op.fatigue_threshold * 100.0)
        } else {
            "0% (Recruit Off)".to_string()
        };
        model.push_common(format!("Fatigue Threshold: {threshold}"));
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

    pub(super) fn build_stockpile_model(
        &self,
        entity: Entity,
        model: &mut InspectionAccumulator,
    ) -> bool {
        let Ok((stockpile, policy, stored_items, incoming)) = self.q_stockpiles.get(entity) else {
            return false;
        };
        let current_amount = stored_items.map_or(0, hw_core::relationships::StoredItems::len);
        let incoming_amount = incoming.map_or(0, hw_core::relationships::IncomingDeliveries::len);
        let state = derive_stockpile_policy_state(
            *policy,
            stockpile.capacity,
            current_amount,
            stockpile.resource_type,
            incoming_amount,
        );
        let resource = stockpile
            .resource_type
            .map(|resource| resource.display_name().to_string())
            .unwrap_or_else(|| "Empty".to_string());
        let export = if state == StockpilePolicyState::Draining && !policy.allow_export {
            "Off (draining override active)"
        } else if policy.allow_export {
            "On"
        } else {
            "Off"
        };

        model.header = "Stockpile".to_string();
        model.push_common("Player-managed stockpile cell".to_string());
        model.push_tooltip(format!("State: {state:?}"));
        model.push_tooltip(format!(
            "Stored: {current_amount}/{} ({resource}) | Incoming: {incoming_amount}",
            stockpile.capacity
        ));
        model.push_tooltip(format!("Target: {}", policy.target_amount));
        let acceptance = if policy.acceptance.is_all() {
            "All".to_string()
        } else if policy.acceptance.is_none() {
            "None".to_string()
        } else {
            policy
                .acceptance
                .accepted_resources()
                .map(|resource| resource.display_name())
                .collect::<Vec<_>>()
                .join(", ")
        };
        model.push_tooltip(format!("Acceptance: {acceptance}"));
        model.push_tooltip(format!("Inbound priority: {:?}", policy.inbound_priority));
        model.push_tooltip(format!("Export: {export}"));
        model.stockpile_fields = Some(StockpileInspectionFields {
            state,
            current_amount,
            incoming_amount,
            capacity: stockpile.capacity,
            current_resource: stockpile.resource_type,
            acceptance: policy.acceptance,
            inbound_priority: policy.inbound_priority,
            target_amount: policy.target_amount,
            allow_export: policy.allow_export,
        });
        true
    }

    pub(super) fn append_building_model(&self, entity: Entity, model: &mut InspectionAccumulator) {
        let Ok((
            building,
            provisional_wall_opt,
            stockpile_opt,
            stored_items_opt,
            mixer_storage_opt,
            rest_area_opt,
            rest_area_occupants_opt,
        )) = self.q_buildings.get(entity)
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

        if building.kind == crate::systems::jobs::BuildingType::Wall && building.is_provisional {
            let wall_status = provisional_wall_opt
                .map(|provisional| {
                    if provisional.mud_delivered {
                        "Wall Upgrade: Mud delivered (ready to coat)"
                    } else {
                        "Wall Upgrade: Waiting for StasisMud"
                    }
                })
                .unwrap_or("Wall Upgrade: Pending");
            model.push_tooltip(wall_status.to_string());
        }

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
                "Storage: Sand {}, Rock {}, Water {}, Mud {}/{}",
                storage.sand, storage.rock, water_count, storage.mud, MUD_MIXER_MUD_CAPACITY
            );
            model.push_tooltip(storage_line.clone());
        }

        if let Some(rest_area) = rest_area_opt {
            let resting_count = rest_area_occupants_opt
                .map(hw_core::relationships::RestAreaOccupants::len)
                .unwrap_or(0)
                .min(rest_area.capacity);
            let dream_rate = resting_count as f32 * DREAM_DRAIN_RATE_REST;
            let line = format!(
                "Resting: {}/{} | Dream: {:.2}/s",
                resting_count, rest_area.capacity, dream_rate
            );
            model.push_tooltip(line);
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

    pub(super) fn append_power_consumer_model(
        &self,
        entity: Entity,
        model: &mut InspectionAccumulator,
    ) {
        let Ok((consumer, policy, supply_state, consumes_from, _unpowered, _transform)) =
            self.q_power_consumers.get(entity)
        else {
            return;
        };

        if model.header.is_empty() {
            model.header = "Power Consumer".to_string();
        }

        model.push_tooltip(format!("Demand: {:.1}W", consumer.demand));
        model.push_tooltip(format!(
            "Priority: {}",
            policy
                .map(|policy| format!("{:?}", policy.priority))
                .unwrap_or_else(|| "Missing policy".to_string())
        ));
        model.push_tooltip(format!(
            "Supply: {}",
            supply_state_label(supply_state.copied())
        ));

        let grid_snapshot =
            consumes_from.and_then(|relation| self.q_power_grids.get(relation.0).ok());
        let shed_order_labels = grid_snapshot
            .and_then(|(_, summary)| summary)
            .map(|summary| self.power_shed_order_labels(summary))
            .unwrap_or_default();
        if let Some((grid, summary)) = grid_snapshot {
            model.push_tooltip(format!(
                "Grid: {:.1}W generation / {:.1}W demand",
                grid.generation, grid.consumption,
            ));
            if let Some(summary) = summary {
                model.push_tooltip(format!(
                    "Allocated: {:.1}W [{:?}]",
                    summary.served_demand, summary.mode
                ));
            }
        } else {
            model.push_tooltip("Connection: Disconnected".to_string());
        }

        model.power_fields = Some(power_inspection_fields(
            PowerInspectionRole::Consumer,
            consumes_from.map(|relation| relation.0),
            grid_snapshot,
            Some(consumer.demand),
            policy.map(|policy| policy.priority),
            supply_state.copied(),
            shed_order_labels,
        ));
    }

    pub(super) fn append_soul_spa_model(&self, entity: Entity, model: &mut InspectionAccumulator) {
        let Ok((site, generator, generates_for_opt)) = self.q_soul_spas.get(entity) else {
            return;
        };

        if model.header.is_empty() {
            model.header = "Soul Spa".to_string();
        }

        let occupied_slots = self
            .q_soul_spa_tiles
            .iter()
            .filter(|(tile, workers)| {
                tile.parent_site == entity && workers.is_some_and(|workers| !workers.is_empty())
            })
            .count() as u32;

        match site.phase {
            SoulSpaPhase::Constructing => {
                model.push_tooltip(format!(
                    "Status: Constructing ({}/{})",
                    site.bones_delivered, site.bones_required
                ));
            }
            SoulSpaPhase::Operational => {
                if occupied_slots > site.active_slots {
                    model.push_tooltip(format!(
                        "Draining ({occupied_slots} active / {} configured)",
                        site.active_slots
                    ));
                } else {
                    model.push_tooltip("Status: Operational".to_string());
                    model.push_tooltip(format!(
                        "Active: {occupied_slots}/{} souls",
                        site.active_slots
                    ));
                }
                model.push_tooltip(format!("Output: {:.1}W", generator.current_output));
                if let Some(gen_for) = generates_for_opt
                    && let Ok((grid, summary)) = self.q_power_grids.get(gen_for.0)
                {
                    model.push_tooltip(format!(
                        "Grid: {:.1}W generation / {:.1}W demand",
                        grid.generation, grid.consumption,
                    ));
                    if let Some(summary) = summary {
                        model.push_tooltip(format!(
                            "Allocated: {:.1}W [{:?}]",
                            summary.served_demand, summary.mode
                        ));
                    }
                }
            }
        }

        model.soul_spa_fields = Some(SoulSpaInspectionFields {
            operational: site.phase == SoulSpaPhase::Operational,
            bones_delivered: site.bones_delivered,
            bones_required: site.bones_required,
            occupied_slots,
            active_slots: site.active_slots,
            max_active_slots: SOUL_SPA_MAX_ACTIVE_SLOTS,
            output_watts: generator.current_output,
        });
        if site.phase != SoulSpaPhase::Operational {
            return;
        }
        let grid_snapshot =
            generates_for_opt.and_then(|relation| self.q_power_grids.get(relation.0).ok());
        let shed_order_labels = grid_snapshot
            .and_then(|(_, summary)| summary)
            .map(|summary| self.power_shed_order_labels(summary))
            .unwrap_or_default();
        model.power_fields = Some(power_inspection_fields(
            PowerInspectionRole::Generator,
            generates_for_opt.map(|relation| relation.0),
            grid_snapshot,
            None,
            None,
            None,
            shed_order_labels,
        ));
    }

    fn power_shed_order_labels(&self, summary: &PowerGridAllocationSummary) -> Vec<String> {
        summary
            .shed_order
            .iter()
            .map(|entity| {
                self.q_power_consumers
                    .get(*entity)
                    .ok()
                    .and_then(|(_, _, _, _, _, transform)| transform)
                    .map(|transform| {
                        let (x, y) = hw_world::WorldMap::world_to_grid(transform.translation.xy());
                        format!("({x}, {y})")
                    })
                    .unwrap_or_else(|| format!("consumer {}", entity.to_bits()))
            })
            .collect()
    }
}

fn power_inspection_fields(
    role: PowerInspectionRole,
    grid_entity: Option<Entity>,
    grid_snapshot: Option<(&hw_energy::PowerGrid, Option<&PowerGridAllocationSummary>)>,
    demand_watts: Option<f32>,
    priority: Option<PowerPriority>,
    supply_state: Option<PowerSupplyState>,
    shed_order_labels: Vec<String>,
) -> PowerInspectionFields {
    let mut fields = PowerInspectionFields {
        role,
        grid: grid_entity,
        allocation_mode: None,
        generation_watts: None,
        total_demand_watts: None,
        served_demand_watts: None,
        reserve_watts: None,
        deficit_watts: None,
        consumer_count: None,
        supplied_count: None,
        shed_count: None,
        invalid_count: None,
        shed_order_labels,
        demand_watts,
        priority: priority.map(power_priority_value),
        supply_state: supply_state.map(power_supply_state_value),
    };
    if let Some((grid, summary)) = grid_snapshot {
        fields.generation_watts = Some(grid.generation);
        fields.total_demand_watts = Some(grid.consumption);
        if let Some(summary) = summary {
            fields.allocation_mode = Some(power_allocation_mode_value(summary.mode));
            fields.generation_watts = Some(summary.generation);
            fields.total_demand_watts = Some(summary.total_demand);
            fields.served_demand_watts = Some(summary.served_demand);
            fields.reserve_watts = Some((summary.generation - summary.total_demand).max(0.0));
            fields.deficit_watts = Some((summary.total_demand - summary.generation).max(0.0));
            fields.consumer_count = Some(summary.consumer_count);
            fields.supplied_count = Some(summary.supplied_count);
            fields.shed_count = Some(summary.shed_count);
            fields.invalid_count = Some(summary.invalid_count);
        }
    }
    fields
}

const fn power_priority_value(priority: PowerPriority) -> PowerPriorityValue {
    match priority {
        PowerPriority::Low => PowerPriorityValue::Low,
        PowerPriority::Normal => PowerPriorityValue::Normal,
        PowerPriority::High => PowerPriorityValue::High,
    }
}

const fn power_allocation_mode_value(mode: PowerAllocationMode) -> PowerAllocationModeValue {
    match mode {
        PowerAllocationMode::LegacyAllOrNone => PowerAllocationModeValue::LegacyAllOrNone,
        PowerAllocationMode::PriorityPrefix => PowerAllocationModeValue::PriorityPrefix,
    }
}

const fn power_supply_state_value(state: PowerSupplyState) -> PowerSupplyStateValue {
    match state {
        PowerSupplyState::Supplied => PowerSupplyStateValue::Supplied,
        PowerSupplyState::Shed { reason } => PowerSupplyStateValue::Shed {
            reason: power_shed_reason_value(reason),
        },
        PowerSupplyState::Disconnected => PowerSupplyStateValue::Disconnected,
        PowerSupplyState::InvalidDemand => PowerSupplyStateValue::InvalidDemand,
    }
}

const fn power_shed_reason_value(reason: PowerShedReason) -> PowerShedReasonValue {
    match reason {
        PowerShedReason::InsufficientGeneration => PowerShedReasonValue::InsufficientGeneration,
        PowerShedReason::RestoreMargin => PowerShedReasonValue::RestoreMargin,
        PowerShedReason::LegacyGlobalDeficit => PowerShedReasonValue::LegacyGlobalDeficit,
    }
}

fn supply_state_label(state: Option<PowerSupplyState>) -> &'static str {
    match state {
        Some(PowerSupplyState::Supplied) => "Supplied",
        Some(PowerSupplyState::Shed {
            reason: hw_energy::PowerShedReason::InsufficientGeneration,
        }) => "Shed: insufficient generation",
        Some(PowerSupplyState::Shed {
            reason: hw_energy::PowerShedReason::RestoreMargin,
        }) => "Shed: waiting for restore margin",
        Some(PowerSupplyState::Shed {
            reason: hw_energy::PowerShedReason::LegacyGlobalDeficit,
        }) => "Shed: legacy grid deficit",
        Some(PowerSupplyState::Disconnected) => "Disconnected",
        Some(PowerSupplyState::InvalidDemand) => "Invalid demand",
        None => "Rebuilding",
    }
}
