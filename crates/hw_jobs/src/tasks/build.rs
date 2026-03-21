use bevy::prelude::*;

#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct BuildData {
    pub blueprint: Entity,
    pub phase: BuildPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum BuildPhase {
    #[default]
    GoingToBlueprint,
    Building {
        progress: f32,
    },
    Done,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct ReinforceFloorTileData {
    pub tile: Entity,
    pub site: Entity,
    pub phase: ReinforceFloorPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum ReinforceFloorPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpBones,
    GoingToTile,
    Reinforcing {
        progress_bp: u16,
    },
    Done,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct PourFloorTileData {
    pub tile: Entity,
    pub site: Entity,
    pub phase: PourFloorPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum PourFloorPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpMud,
    GoingToTile,
    Pouring {
        progress_bp: u16,
    },
    Done,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct FrameWallTileData {
    pub tile: Entity,
    pub site: Entity,
    pub phase: FrameWallPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum FrameWallPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpWood,
    GoingToTile,
    Framing {
        progress_bp: u16,
    },
    Done,
}

#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub struct CoatWallData {
    pub tile: Entity,
    pub site: Entity,
    pub wall: Entity,
    pub phase: CoatWallPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum CoatWallPhase {
    #[default]
    GoingToMaterialCenter,
    PickingUpMud,
    GoingToTile,
    Coating {
        progress_bp: u16,
    },
    Done,
}
