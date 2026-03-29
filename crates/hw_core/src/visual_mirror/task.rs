use bevy::prelude::*;

/// Mirror of a Soul entity's current task for `hw_visual`.
/// Synced by `sync_soul_task_visual_system` in `hw_jobs` whenever `AssignedTask` changes.
#[derive(Component, Default, Debug, Clone)]
pub struct SoulTaskVisualState {
    pub phase: SoulTaskPhaseVisual,
    /// Current work progress in range `0.0–1.0`. `None` = no progress bar needed.
    pub progress: Option<f32>,
    /// Primary gizmo link target entity.
    pub link_target: Option<Entity>,
    /// Bucket transport link entity. When `Some`, overrides `link_target` for gizmo drawing.
    pub bucket_link: Option<Entity>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulTaskPhaseVisual {
    #[default]
    None,
    /// Gather with axe (WorkType::Chop)
    GatherChop,
    /// Gather with pickaxe (WorkType::Mine)
    GatherMine,
    Haul,
    HaulToBlueprint,
    Build,
    ReinforceFloor,
    PourFloor,
    FrameWall,
    CoatWall,
    Refine,
    CollectSand,
    CollectBone,
    MovePlant,
    BucketTransport,
    HaulToMixer,
    HaulWithWheelbarrow,
    GeneratePower,
}
