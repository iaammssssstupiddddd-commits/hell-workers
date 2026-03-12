// WorkType の説明文言

use crate::systems::jobs::{Blueprint, BonePile, BuildingType, Rock, SandPile, Tree, WorkType};
use crate::systems::logistics::ResourceItem;
use crate::systems::logistics::transport_request::{TransportRequest, TransportRequestKind};
use bevy::prelude::*;

pub fn generate_task_description(
    wt: WorkType,
    entity: Entity,
    blueprint: Option<&Blueprint>,
    transport_req: Option<&TransportRequest>,
    resource_item: Option<&ResourceItem>,
    tree: Option<&Tree>,
    rock: Option<&Rock>,
    sand_pile: Option<&SandPile>,
    bone_pile: Option<&BonePile>,
) -> String {
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
        WorkType::CollectSand => {
            if sand_pile.is_some() {
                "Collect Sand".to_string()
            } else {
                "Collect Sand".to_string()
            }
        }
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
    }
}
