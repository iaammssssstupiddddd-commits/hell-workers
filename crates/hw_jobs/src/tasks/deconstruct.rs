use bevy::prelude::*;

/// Runtime-only payload for one deconstruction assignment.
///
/// The durable source of truth is the `DeconstructionOrder`; active assignments
/// are rebuilt from that order after loading.
#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct DeconstructData {
    pub order: Entity,
    pub target: Entity,
    pub phase: DeconstructPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum DeconstructPhase {
    #[default]
    GoingToTarget,
    Dismantling {
        progress: f32,
    },
    AwaitingCommit,
}
