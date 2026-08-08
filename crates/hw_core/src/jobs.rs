use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum WorkType {
    #[default]
    Chop,
    Mine,
    Build,
    Move,
    Haul,
    HaulToMixer,
    GatherWater,
    CollectBone,
    Refine,
    HaulWaterToMixer,
    WheelbarrowHaul,
    ReinforceFloorTile,
    PourFloorTile,
    FrameWallTile,
    CoatWall,
    GeneratePower,
    Deconstruct,
}

impl WorkType {
    pub const ALL: [Self; 17] = [
        Self::Chop,
        Self::Mine,
        Self::Build,
        Self::Move,
        Self::Haul,
        Self::HaulToMixer,
        Self::GatherWater,
        Self::CollectBone,
        Self::Refine,
        Self::HaulWaterToMixer,
        Self::WheelbarrowHaul,
        Self::ReinforceFloorTile,
        Self::PourFloorTile,
        Self::FrameWallTile,
        Self::CoatWall,
        Self::GeneratePower,
        Self::Deconstruct,
    ];

    pub const COUNT: usize = Self::ALL.len();

    #[must_use]
    pub const fn stable_index(self) -> usize {
        match self {
            Self::Chop => 0,
            Self::Mine => 1,
            Self::Build => 2,
            Self::Move => 3,
            Self::Haul => 4,
            Self::HaulToMixer => 5,
            Self::GatherWater => 6,
            Self::CollectBone => 7,
            Self::Refine => 8,
            Self::HaulWaterToMixer => 9,
            Self::WheelbarrowHaul => 10,
            Self::ReinforceFloorTile => 11,
            Self::PourFloorTile => 12,
            Self::FrameWallTile => 13,
            Self::CoatWall => 14,
            Self::GeneratePower => 15,
            Self::Deconstruct => 16,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn work_type_all_is_unique_exhaustive_and_stably_indexed() {
        assert_eq!(WorkType::ALL.len(), WorkType::COUNT);
        assert_eq!(
            WorkType::ALL.iter().copied().collect::<HashSet<_>>().len(),
            WorkType::COUNT
        );

        for (index, work_type) in WorkType::ALL.into_iter().enumerate() {
            assert_eq!(work_type.stable_index(), index);
        }
        assert_eq!(WorkType::Deconstruct.stable_index(), 16);
    }
}
