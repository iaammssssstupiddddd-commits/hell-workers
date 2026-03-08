use bevy::prelude::*;

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

#[derive(Clone, PartialEq)]
pub struct EntityInspectionModel {
    pub header: String,
    pub common_text: String,
    pub tooltip_lines: Vec<String>,
    pub soul: Option<SoulInspectionFields>,
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
