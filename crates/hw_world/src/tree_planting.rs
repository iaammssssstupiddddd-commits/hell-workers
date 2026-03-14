use hw_core::constants::DREAM_TREE_COST_PER_TREE;

/// Planning data for a dream-tree planting operation.
///
/// Built by `build_dream_tree_planting_plan()` in `bevy_app`. That builder
/// function depends on `bevy_app`-specific types (`GameAssets`, `DreamPool`,
/// etc.) and therefore cannot move to a Leaf crate. Only this pure data
/// struct lives here so that other Leaf crates can reference the plan type
/// without a circular dependency.
#[derive(Debug, Clone)]
pub struct DreamTreePlantingPlan {
    pub width_tiles: u32,
    pub height_tiles: u32,
    pub min_square_side: u32,
    pub planned_spawn: u32,
    pub cap_remaining: u32,
    pub affordable: u32,
    pub candidate_count: u32,
    pub selected_tiles: Vec<(i32, i32)>,
}

impl DreamTreePlantingPlan {
    pub fn final_spawn(&self) -> u32 {
        self.selected_tiles.len() as u32
    }

    pub fn cost(&self) -> f32 {
        self.final_spawn() as f32 * DREAM_TREE_COST_PER_TREE
    }
}
