use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum FloorConstructionPhase {
    /// Placing bones as reinforcement
    Reinforcing,
    /// Pouring mud as concrete
    Pouring,
    /// Waiting for poured tiles to cure while area is blocked
    Curing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum FloorTileState {
    /// Waiting for bones to be delivered
    WaitingBones,
    /// Bones delivered, ready for worker to reinforce
    ReinforcingReady,
    /// Worker is actively reinforcing
    Reinforcing { progress: u8 },
    /// Reinforcing complete, waiting for phase transition
    ReinforcedComplete,
    /// Waiting for mud to be delivered (after phase transition)
    WaitingMud,
    /// Mud delivered, ready for worker to pour
    PouringReady,
    /// Worker is actively pouring
    Pouring { progress: u8 },
    /// Construction complete
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum WallConstructionPhase {
    /// Build provisional wall frame using wood
    Framing,
    /// Coat provisional wall with stasis mud to finalize
    Coating,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum WallTileState {
    /// Waiting for wood to be delivered
    WaitingWood,
    /// Wood delivered, ready for worker to frame
    FramingReady,
    /// Worker is actively framing
    Framing { progress: u8 },
    /// Framing complete and provisional wall is spawned
    FramedProvisional,
    /// Waiting for mud to be delivered (after phase transition)
    WaitingMud,
    /// Mud delivered, ready for worker to coat
    CoatingReady,
    /// Worker is actively coating
    Coating { progress: u8 },
    /// Construction complete
    Complete,
}
