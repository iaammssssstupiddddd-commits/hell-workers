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
    CollectSand,
    CollectBone,
    Refine,
    HaulWaterToMixer,
    WheelbarrowHaul,
    ReinforceFloorTile,
    PourFloorTile,
    FrameWallTile,
    CoatWall,
    GeneratePower,
}
