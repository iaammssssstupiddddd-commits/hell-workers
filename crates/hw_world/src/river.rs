use crate::layout::{RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MAX, RIVER_Y_MIN};
use crate::world_masks::BitGrid;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use std::collections::HashSet;
use std::collections::VecDeque;

mod channel;
mod legacy;
mod sand;

pub use channel::{
    RIVER_MAX_WIDTH, RIVER_MIN_WIDTH, RIVER_START_Y_MAX, RIVER_START_Y_MIN,
    RIVER_TOTAL_TILES_TARGET_MAX, RIVER_TOTAL_TILES_TARGET_MIN, RIVER_Y_CLAMP_MAX,
    RIVER_Y_CLAMP_MIN, generate_river_mask, preview_river_min_y,
};
pub use legacy::{generate_fixed_river_tiles, generate_sand_tiles};
pub use sand::{
    SAND_BASE_DIST_MAX, SAND_BASE_DIST_MIN, SAND_CARVE_MAX_RATIO_PERCENT,
    SAND_CARVE_REGION_SIZE_MAX, SAND_CARVE_REGION_SIZE_MIN, SAND_CARVE_SEED_COUNT_MAX,
    SAND_CARVE_SEED_COUNT_MIN, SAND_GROWTH_DIST_MAX, SAND_GROWTH_REGION_AREA_MAX,
    SAND_GROWTH_SEED_COUNT_MAX, SAND_GROWTH_SEED_COUNT_MIN, SAND_GROWTH_STEP_LIMIT,
    SAND_SHORE_MAX_DISTANCE, generate_sand_masks,
};

#[cfg(test)]
use sand::CARDINAL_DIRS_4;

#[cfg(test)]
mod tests;
