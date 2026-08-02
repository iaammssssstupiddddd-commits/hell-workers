use bevy::prelude::*;
use hw_logistics::transport_request::TransportPriority;
use hw_logistics::{ResourceType, StockpileAcceptance, StockpilePolicyState};

use crate::power::{
    PowerAllocationModeValue, PowerInspectionRole, PowerPriorityValue, PowerSupplyStateValue,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InspectionSoulGender {
    Male,
    Female,
}

#[derive(Clone, PartialEq)]
pub struct SoulInspectionFields {
    pub gender: Option<InspectionSoulGender>,
    pub motivation: String,
    pub stress: String,
    pub fatigue: String,
    pub dream: String,
    pub task: String,
    pub inventory: String,
    pub common: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct StockpileInspectionFields {
    pub state: StockpilePolicyState,
    pub current_amount: usize,
    pub incoming_amount: usize,
    pub capacity: usize,
    pub current_resource: Option<ResourceType>,
    pub acceptance: StockpileAcceptance,
    pub inbound_priority: TransportPriority,
    pub target_amount: usize,
    pub allow_export: bool,
}

#[derive(Clone, PartialEq)]
pub struct SoulSpaInspectionFields {
    pub operational: bool,
    pub bones_delivered: u32,
    pub bones_required: u32,
    pub occupied_slots: u32,
    pub active_slots: u32,
    pub max_active_slots: u32,
    pub output_watts: f32,
}

#[derive(Clone, PartialEq)]
pub struct PowerInspectionFields {
    pub role: PowerInspectionRole,
    pub grid: Option<Entity>,
    pub allocation_mode: Option<PowerAllocationModeValue>,
    pub generation_watts: Option<f32>,
    pub total_demand_watts: Option<f32>,
    pub served_demand_watts: Option<f32>,
    pub reserve_watts: Option<f32>,
    pub deficit_watts: Option<f32>,
    pub consumer_count: Option<usize>,
    pub supplied_count: Option<usize>,
    pub shed_count: Option<usize>,
    pub invalid_count: Option<usize>,
    pub shed_order_labels: Vec<String>,
    pub demand_watts: Option<f32>,
    pub priority: Option<PowerPriorityValue>,
    pub supply_state: Option<PowerSupplyStateValue>,
}

#[derive(Clone, PartialEq)]
pub struct EntityInspectionModel {
    pub entity: Entity,
    pub header: String,
    pub common_text: String,
    pub tooltip_lines: Vec<String>,
    pub soul: Option<SoulInspectionFields>,
    pub stockpile: Option<StockpileInspectionFields>,
    pub soul_spa: Option<SoulSpaInspectionFields>,
    pub power: Option<PowerInspectionFields>,
}

#[derive(Resource, Default, Clone, PartialEq)]
pub struct EntityInspectionViewModel {
    pub model: Option<EntityInspectionModel>,
}

impl EntityInspectionViewModel {
    pub fn set(&mut self, model: Option<EntityInspectionModel>) {
        self.model = model;
    }
}
