use bevy::prelude::*;
use hw_logistics::transport_request::TransportPriority;
use hw_logistics::{ResourceType, StockpileAcceptance, StockpilePolicyState};

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
pub struct EntityInspectionModel {
    pub entity: Entity,
    pub header: String,
    pub common_text: String,
    pub tooltip_lines: Vec<String>,
    pub soul: Option<SoulInspectionFields>,
    pub stockpile: Option<StockpileInspectionFields>,
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
