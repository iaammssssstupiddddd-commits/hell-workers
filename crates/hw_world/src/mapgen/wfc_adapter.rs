//! WFC ソルバーアダプタ（MS-WFC-2b）。
//!
//! - `TerrainType` ↔ `wfc::PatternId` の固定マッピング
//! - `PatternTable<PatternDescription>` による隣接ルール定義
//! - `WorldConstraints: ForbidPattern` として river 固定・anchor 禁止・
//!   マスク外 River 伝播防止を記述
//! - `run_wfc()` によるソルバー呼び出しと deterministic retry

use std::num::NonZeroU32;

use crate::terrain_zones::{
    ZONE_DIRT_ENFORCE_MAX, ZONE_DIRT_ENFORCE_MIN, ZONE_GRADIENT_DIRT_BIAS_PERCENT,
    ZONE_GRADIENT_GRASS_BIAS_PERCENT, ZONE_GRADIENT_WIDTH, ZONE_GRASS_ENFORCE_MAX,
    ZONE_GRASS_ENFORCE_MIN,
};
use direction::CardinalDirectionTable;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use wfc::wrap::{Wrap, WrapNone};
use wfc::{
    Coord, ForbidInterface, ForbidPattern, GlobalStats, PatternDescription, PatternId,
    PatternTable, RunOwn, Size,
};

use crate::terrain::TerrainType;
use crate::world_masks::WorldMasks;

mod postprocess;
mod rules;
mod visual_cross;

pub(crate) use postprocess::fallback_terrain;
pub use rules::{
    CARDINAL_DIRS, MAX_WFC_RETRIES, SAND_ADJACENT_TO_RIVER_WEIGHT, SAND_NON_ADJACENT_WEIGHT,
    TERRAIN_PATTERN_DIRT, TERRAIN_PATTERN_GRASS, TERRAIN_PATTERN_RIVER, TERRAIN_PATTERN_SAND,
    TerrainTileMapping, WEIGHT_DIRT, WEIGHT_GRASS, WEIGHT_SAND, WorldConstraints,
    build_pattern_table,
};
pub(crate) use visual_cross::fix_zone_mask_crosses;

use postprocess::post_process_tiles;
use rules::{NEUTRAL_REGION_BIAS_PERCENT, NEUTRAL_REGION_SIZE};
use visual_cross::enforce_no_visual_cross_2x2;
#[cfg(test)]
use visual_cross::has_any_visual_cross_2x2;

// ── ヘルパー関数 ──────────────────────────────────────────────────────────────

/// `master_seed` と `attempt` から deterministic に sub_seed を導出する。
/// splitmix64 の 1 ステップを使ってビットを分散させる。
pub(crate) fn derive_sub_seed(master_seed: u64, attempt: u32) -> u64 {
    master_seed.wrapping_add((attempt as u64).wrapping_mul(0x9e3779b97f4a7c15))
}

// ── ソルバー呼び出し ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum WfcError {
    Contradiction,
}

/// WFC ソルバーを呼び出して TerrainType グリッドを返す。
///
/// ## 制約の適用方法
/// wfc ライブラリの priority queue は `ForbidPattern::forbid()` 前に初期化されるため、
/// weighted パターンを `forbid_pattern` で直接除去すると stale entry 問題が発生する。
/// そのため WFC では River の固定・伝播防止のみ行い、Sand/anchor 制約は
/// WFC 完了後に `post_process_tiles()` でポスト処理する。
///
/// # 引数
/// - `masks`: `fill_river_from_seed()` 適用済みの WorldMasks
/// - `seed`: サブシード（caller が `derive_sub_seed` で計算する）
/// - `attempt`: 試行回数（将来のログ用）
pub fn run_wfc(
    masks: &mut WorldMasks,
    seed: u64,
    attempt: u32,
) -> Result<Vec<TerrainType>, WfcError> {
    let _ = attempt; // 将来 tracing::debug! に差し替え可

    let table = build_pattern_table();
    let global_stats = GlobalStats::new(table);
    let constraints = WorldConstraints::from_masks(masks);
    let size = Size::new(MAP_WIDTH as u32, MAP_HEIGHT as u32);
    let mut rng = StdRng::seed_from_u64(seed);

    let mut run = RunOwn::new_wrap_forbid(size, &global_stats, WrapNone, constraints, &mut rng);
    run.collapse(&mut rng)
        .map_err(|_| WfcError::Contradiction)?;

    let wave = run.into_wave();
    // Grid::iter() は row-major (idx = y * width + x)。WorldMasks と同じ並び。
    let mut tiles = wave
        .grid()
        .iter()
        .map(|cell| {
            let pid = cell
                .chosen_pattern_id()
                .expect("WFC: cell not collapsed after successful collapse");
            TerrainTileMapping::from_pattern_id(pid).expect("WFC: unknown PatternId in result")
        })
        .collect::<Vec<TerrainType>>();

    post_process_tiles(&mut tiles, masks, &mut rng);
    Ok(tiles)
}

#[cfg(test)]
mod tests;
