// WorkType の説明文言

use crate::systems::jobs::{
    Blueprint, BonePile, Building, BuildingType, Rock, SandPile, Tree, WorkType,
};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::transport_request::{TransportRequest, TransportRequestKind};
use bevy::prelude::*;

pub struct TaskComponentRefs<'a> {
    pub blueprint: Option<&'a Blueprint>,
    pub transport_req: Option<&'a TransportRequest>,
    pub resource_item: Option<&'a ResourceItem>,
    pub tree: Option<&'a Tree>,
    pub rock: Option<&'a Rock>,
    pub _sand_pile: Option<&'a SandPile>,
    pub bone_pile: Option<&'a BonePile>,
    pub deconstruction_target: Option<&'a Building>,
}

pub fn generate_task_description(wt: WorkType, entity: Entity, refs: TaskComponentRefs) -> String {
    let TaskComponentRefs {
        blueprint,
        transport_req,
        resource_item,
        tree,
        rock,
        _sand_pile: _,
        bone_pile,
        deconstruction_target,
    } = refs;
    match wt {
        WorkType::Build => {
            if let Some(bp) = blueprint {
                match bp.kind {
                    BuildingType::Wall => "Construct Wall".to_string(),
                    BuildingType::Door => "Construct Door".to_string(),
                    BuildingType::Floor => "Construct Floor".to_string(),
                    BuildingType::Tank => "Construct Tank".to_string(),
                    BuildingType::MudMixer => "Construct Mixer".to_string(),
                    BuildingType::RestArea => "Construct RestArea".to_string(),
                    BuildingType::Bridge => "Construct Bridge".to_string(),
                    BuildingType::SandPile => "Construct SandPile".to_string(),
                    BuildingType::BonePile => "Construct BonePile".to_string(),
                    BuildingType::WheelbarrowParking => "Construct Parking".to_string(),
                    BuildingType::SoulSpa => "Construct Soul Spa".to_string(),
                    BuildingType::OutdoorLamp => "Construct Outdoor Lamp".to_string(),
                }
            } else {
                format!("Construct {:?}", entity)
            }
        }
        WorkType::Move => "Move Building".to_string(),
        WorkType::Mine => {
            if rock.is_some() {
                "Mine Rock".to_string()
            } else {
                "Mine".to_string()
            }
        }
        WorkType::Chop => {
            if tree.is_some() {
                "Chop Tree".to_string()
            } else {
                "Chop".to_string()
            }
        }
        WorkType::Haul => {
            if let Some(req) = transport_req {
                if req.kind == TransportRequestKind::DeliverToBlueprint {
                    format!("Haul {:?} to Build", req.resource_type)
                } else if req.kind == TransportRequestKind::DeliverToWallConstruction {
                    format!("Haul {:?} to Wall", req.resource_type)
                } else if req.kind == TransportRequestKind::DeliverToProvisionalWall {
                    "Haul StasisMud to Wall".to_string()
                } else if req.kind == TransportRequestKind::DeliverToSoulSpa {
                    format!("Haul {:?} to Soul Spa", req.resource_type)
                } else {
                    format!("Haul {:?} (Req)", req.resource_type)
                }
            } else if let Some(item) = resource_item {
                format!("Haul {:?}", item.0)
            } else {
                "Haul".to_string()
            }
        }
        WorkType::HaulToMixer => {
            if let Some(req) = transport_req {
                format!("Haul {:?} to Mixer", req.resource_type)
            } else {
                "Haul to Mixer".to_string()
            }
        }
        WorkType::HaulWaterToMixer => "Haul Water to Mixer".to_string(),
        WorkType::GatherWater => "Gather Water".to_string(),
        WorkType::Refine => "Refine".to_string(),
        WorkType::WheelbarrowHaul => "Wheelbarrow Haul".to_string(),
        WorkType::CollectBone => {
            if bone_pile.is_some() {
                "Collect Bone Pile".to_string()
            } else {
                "Collect Bone".to_string()
            }
        }
        WorkType::ReinforceFloorTile => "Reinforce Floor".to_string(),
        WorkType::PourFloorTile => "Pour Floor".to_string(),
        WorkType::FrameWallTile => "Frame Wall".to_string(),
        WorkType::CoatWall => "Coat Wall".to_string(),
        WorkType::GeneratePower => "Generate Power".to_string(),
        WorkType::Deconstruct => deconstruction_target.map_or_else(
            || "Deconstruct Building".to_string(),
            |building| format!("Deconstruct {}", building_type_label(building.kind)),
        ),
    }
}

const fn building_type_label(kind: BuildingType) -> &'static str {
    match kind {
        BuildingType::Wall => "Wall",
        BuildingType::Door => "Door",
        BuildingType::Floor => "Floor",
        BuildingType::Tank => "Tank",
        BuildingType::MudMixer => "Mixer",
        BuildingType::RestArea => "Rest Area",
        BuildingType::Bridge => "Bridge",
        BuildingType::SandPile => "Sand Pile",
        BuildingType::BonePile => "Bone Pile",
        BuildingType::WheelbarrowParking => "Wheelbarrow Parking",
        BuildingType::SoulSpa => "Soul Spa",
        BuildingType::OutdoorLamp => "Outdoor Lamp",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_logistics::ResourceType;
    use hw_logistics::transport_request::TransportPriority;

    #[test]
    fn soul_spa_delivery_identifies_its_owner_in_the_dashboard() {
        let request = TransportRequest {
            kind: TransportRequestKind::DeliverToSoulSpa,
            anchor: Entity::PLACEHOLDER,
            resource_type: ResourceType::Bone,
            issued_by: Entity::PLACEHOLDER,
            priority: TransportPriority::Normal,
            stockpile_group: Vec::new(),
        };

        assert_eq!(
            generate_task_description(
                WorkType::Haul,
                Entity::PLACEHOLDER,
                TaskComponentRefs {
                    blueprint: None,
                    transport_req: Some(&request),
                    resource_item: None,
                    tree: None,
                    rock: None,
                    _sand_pile: None,
                    bone_pile: None,
                    deconstruction_target: None,
                },
            ),
            "Haul Bone to Soul Spa"
        );
    }
}
