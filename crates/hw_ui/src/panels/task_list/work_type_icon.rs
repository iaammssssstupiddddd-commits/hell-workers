use crate::setup::UiAssets;
use crate::theme::UiTheme;
use bevy::prelude::*;
use hw_core::jobs::WorkType;

pub fn work_type_label(wt: &WorkType) -> &'static str {
    match wt {
        WorkType::Chop => "Chop",
        WorkType::Mine => "Mine",
        WorkType::Build => "Build",
        WorkType::Move => "Move",
        WorkType::Haul => "Haul",
        WorkType::HaulToMixer => "Haul (Mixer)",
        WorkType::GatherWater => "Water",
        WorkType::CollectSand => "Sand",
        WorkType::Refine => "Refine",
        WorkType::HaulWaterToMixer => "Water (Mixer)",
        WorkType::WheelbarrowHaul => "Wheelbarrow",
        WorkType::CollectBone => "Bone",
        WorkType::ReinforceFloorTile => "Reinforce",
        WorkType::PourFloorTile => "Pour",
        WorkType::FrameWallTile => "Frame",
        WorkType::CoatWall => "Coat",
    }
}

pub fn work_type_icon(
    wt: &WorkType,
    assets: &dyn UiAssets,
    theme: &UiTheme,
) -> (Handle<Image>, Color) {
    match wt {
        WorkType::Chop => (assets.icon_axe().clone(), theme.colors.chop),
        WorkType::Mine => (assets.icon_pick().clone(), theme.colors.mine),
        WorkType::Build => (assets.icon_hammer().clone(), theme.colors.build),
        WorkType::Move => (assets.icon_hammer().clone(), theme.colors.build),
        WorkType::Haul | WorkType::HaulToMixer | WorkType::WheelbarrowHaul => {
            (assets.icon_haul().clone(), theme.colors.haul)
        }
        WorkType::GatherWater | WorkType::HaulWaterToMixer => {
            (assets.icon_haul().clone(), theme.colors.water)
        }
        WorkType::CollectSand => (assets.icon_pick().clone(), theme.colors.gather_default),
        WorkType::CollectBone => (assets.icon_bone_small().clone(), theme.colors.gather_default),
        WorkType::Refine => (assets.icon_hammer().clone(), theme.colors.build),
        WorkType::ReinforceFloorTile
        | WorkType::PourFloorTile
        | WorkType::FrameWallTile
        | WorkType::CoatWall => (assets.icon_hammer().clone(), theme.colors.build),
    }
}
