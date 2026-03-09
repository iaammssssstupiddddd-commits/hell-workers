use bevy::prelude::*;

/// Outcome of processing selection input for a single frame.
/// Pure data — no ECS mutations. Root adapters read this and apply side effects.
#[derive(Debug, Clone)]
pub enum SelectionIntent {
    /// Select the given entity.
    Select(Entity),
    /// Deselect whatever is currently selected.
    ClearSelection,
    /// Begin task-area resize for a familiar.
    StartAreaSelection { familiar: Entity },
    /// Issue a move command to a familiar.
    MoveFamiliar { familiar: Entity, destination: Vec2 },
    /// No action this frame.
    None,
}
