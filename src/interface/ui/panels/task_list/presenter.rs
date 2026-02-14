//! WorkType のアイコン・ラベル・説明文言

use crate::systems::jobs::{Blueprint, BuildingType, Rock, SandPile, Tree, WorkType};
use crate::systems::logistics::transport_request::{TransportRequest, TransportRequestKind};
use crate::systems::logistics::ResourceItem;
use bevy::prelude::*;

use crate::interface::ui::theme::UiTheme;

pub fn work_type_label(wt: &WorkType) -> &'static str {
    match wt {
        WorkType::Chop => "Chop",
        WorkType::Mine => "Mine",
        WorkType::Build => "Build",
        WorkType::Haul => "Haul",
        WorkType::HaulToMixer => "Haul (Mixer)",
        WorkType::GatherWater => "Water",
        WorkType::CollectSand => "Sand",
        WorkType::Refine => "Refine",
        WorkType::HaulWaterToMixer => "Water (Mixer)",
        WorkType::WheelbarrowHaul => "Wheelbarrow",
    }
}

pub fn get_work_type_icon(
    wt: &WorkType,
    game_assets: &crate::assets::GameAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match wt {
        WorkType::Chop => (game_assets.icon_axe.clone(), theme.colors.chop),
        WorkType::Mine => (game_assets.icon_pick.clone(), theme.colors.mine),
        WorkType::Build => (game_assets.icon_hammer.clone(), theme.colors.build),
        WorkType::Haul | WorkType::HaulToMixer | WorkType::WheelbarrowHaul => {
            (game_assets.icon_haul.clone(), theme.colors.haul)
        }
        WorkType::GatherWater | WorkType::HaulWaterToMixer => {
            (game_assets.icon_haul.clone(), theme.colors.water)
        }
        WorkType::CollectSand => (game_assets.icon_pick.clone(), theme.colors.gather_default),
        WorkType::Refine => (game_assets.icon_hammer.clone(), theme.colors.build),
    }
}

pub fn generate_task_description(
    wt: WorkType,
    entity: Entity,
    blueprint: Option<&Blueprint>,
    transport_req: Option<&TransportRequest>,
    resource_item: Option<&ResourceItem>,
    tree: Option<&Tree>,
    rock: Option<&Rock>,
    sand_pile: Option<&SandPile>,
) -> String {
    match wt {
        WorkType::Build => {
            if let Some(bp) = blueprint {
                match bp.kind {
                    BuildingType::Wall => "Construct Wall".to_string(),
                    BuildingType::Floor => "Construct Floor".to_string(),
                    BuildingType::Tank => "Construct Tank".to_string(),
                    BuildingType::MudMixer => "Construct Mixer".to_string(),
                    BuildingType::SandPile => "Construct SandPile".to_string(),
                    BuildingType::WheelbarrowParking => "Construct Parking".to_string(),
                }
            } else {
                format!("Construct {:?}", entity)
            }
        }
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
    }
}
