use crate::setup::UiAssets;
use crate::theme::UiTheme;
use bevy::prelude::*;
use hw_core::jobs::WorkType;

/// Work types whose producer and completion path are currently player-reachable.
///
/// Keep this exhaustive so adding a domain variant requires an explicit UI
/// publication decision. Deconstruct is player-reachable through its Orders
/// producer and dedicated completion consumer.
pub(crate) const fn has_player_workflow(work_type: WorkType) -> bool {
    match work_type {
        WorkType::Chop
        | WorkType::Mine
        | WorkType::Build
        | WorkType::Move
        | WorkType::Haul
        | WorkType::HaulToMixer
        | WorkType::GatherWater
        | WorkType::CollectBone
        | WorkType::Refine
        | WorkType::HaulWaterToMixer
        | WorkType::WheelbarrowHaul
        | WorkType::ReinforceFloorTile
        | WorkType::PourFloorTile
        | WorkType::FrameWallTile
        | WorkType::CoatWall
        | WorkType::GeneratePower
        | WorkType::Deconstruct => true,
    }
}

pub(crate) fn player_reachable_work_types() -> impl Iterator<Item = WorkType> {
    WorkType::ALL
        .into_iter()
        .filter(|work_type| has_player_workflow(*work_type))
}

pub fn work_type_label(wt: &WorkType) -> &'static str {
    match wt {
        WorkType::Chop => "Chop",
        WorkType::Mine => "Mine",
        WorkType::Build => "Build",
        WorkType::Move => "Move",
        WorkType::Haul => "Haul",
        WorkType::HaulToMixer => "Haul (Mixer)",
        WorkType::GatherWater => "Water",
        WorkType::CollectBone => "Bone",
        WorkType::Refine => "Refine",
        WorkType::HaulWaterToMixer => "Water (Mixer)",
        WorkType::WheelbarrowHaul => "Wheelbarrow",
        WorkType::ReinforceFloorTile => "Reinforce",
        WorkType::PourFloorTile => "Pour",
        WorkType::FrameWallTile => "Frame",
        WorkType::CoatWall => "Coat",
        WorkType::GeneratePower => "Generate",
        WorkType::Deconstruct => "Deconstruct",
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
        WorkType::CollectBone => (
            assets.icon_bone_small().clone(),
            theme.colors.gather_default,
        ),
        WorkType::Refine => (assets.icon_hammer().clone(), theme.colors.build),
        WorkType::ReinforceFloorTile
        | WorkType::PourFloorTile
        | WorkType::FrameWallTile
        | WorkType::CoatWall
        | WorkType::GeneratePower
        | WorkType::Deconstruct => (assets.icon_hammer().clone(), theme.colors.build),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deconstruction_is_published_with_the_connected_player_workflow() {
        let visible: Vec<_> = player_reachable_work_types().collect();
        assert_eq!(visible, WorkType::ALL);
        assert_eq!(visible.last(), Some(&WorkType::Deconstruct));
    }
}
