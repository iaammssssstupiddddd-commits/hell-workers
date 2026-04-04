//! WFC ソルバーへのアダプタ骨格（MS-WFC-2a）。
//!
//! - `TerrainType` ↔ `wfc::PatternId` の固定マッピング
//! - `PatternTable<PatternDescription>` による隣接ルール定義
//! - `WorldConstraints: ForbidPattern` として river 固定セルと Site/Yard 制約を記述
//! - ソルバー呼び出しシグネチャ（実装は MS-WFC-2b）

use std::num::NonZeroU32;

use direction::CardinalDirectionTable;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use rand::Rng;
use wfc::{
    Coord, ForbidInterface, ForbidPattern, GlobalStats, PatternDescription, PatternId, PatternTable,
    PropagateError,
};
use wfc::wrap::Wrap;

use crate::terrain::TerrainType;
use crate::world_masks::WorldMasks;

// ── PatternId 定数（配列インデックスとして PatternTable に登録する順序） ─────
pub const TERRAIN_PATTERN_GRASS: PatternId = 0;
pub const TERRAIN_PATTERN_DIRT: PatternId = 1;
pub const TERRAIN_PATTERN_SAND: PatternId = 2;
pub const TERRAIN_PATTERN_RIVER: PatternId = 3;

// ── 砂の重み定数（F4: 川隣接を主、それ以外は低頻度; 実使用は MS-WFC-2b） ───
/// 川に隣接する Sand タイルの重み
pub const SAND_ADJACENT_TO_RIVER_WEIGHT: u32 = 10;
/// 川に隣接しない Sand タイルの重み
pub const SAND_NON_ADJACENT_WEIGHT: u32 = 1;

// ── TerrainType ↔ PatternId 変換 ─────────────────────────────────────────────

/// `TerrainType` ↔ `wfc::PatternId` の固定変換。
pub struct TerrainTileMapping;

impl TerrainTileMapping {
    pub fn to_pattern_id(terrain: TerrainType) -> PatternId {
        match terrain {
            TerrainType::Grass => TERRAIN_PATTERN_GRASS,
            TerrainType::Dirt => TERRAIN_PATTERN_DIRT,
            TerrainType::Sand => TERRAIN_PATTERN_SAND,
            TerrainType::River => TERRAIN_PATTERN_RIVER,
        }
    }

    pub fn from_pattern_id(id: PatternId) -> Option<TerrainType> {
        match id {
            TERRAIN_PATTERN_GRASS => Some(TerrainType::Grass),
            TERRAIN_PATTERN_DIRT => Some(TerrainType::Dirt),
            TERRAIN_PATTERN_SAND => Some(TerrainType::Sand),
            TERRAIN_PATTERN_RIVER => Some(TerrainType::River),
            _ => None,
        }
    }
}

// ── 隣接ルール ────────────────────────────────────────────────────────────────

/// ゲームロジックに基づく隣接ルールを PatternTable として構築する。
///
/// - River の隣は Sand のみ（Grass/Dirt は River に直接隣接不可）
/// - 隣接ルールは対称（A → B が許可なら B → A も許可）
/// - 4 方向すべて同一ルール（等方的地形）
/// - weight はすべて 1（タイル重み調整は MS-WFC-2b で行う）
pub fn build_pattern_table() -> PatternTable<PatternDescription> {
    let allowed_pairs: &[(PatternId, PatternId)] = &[
        (TERRAIN_PATTERN_GRASS, TERRAIN_PATTERN_GRASS),
        (TERRAIN_PATTERN_GRASS, TERRAIN_PATTERN_DIRT),
        (TERRAIN_PATTERN_GRASS, TERRAIN_PATTERN_SAND),
        (TERRAIN_PATTERN_DIRT, TERRAIN_PATTERN_DIRT),
        (TERRAIN_PATTERN_DIRT, TERRAIN_PATTERN_SAND),
        (TERRAIN_PATTERN_SAND, TERRAIN_PATTERN_SAND),
        (TERRAIN_PATTERN_SAND, TERRAIN_PATTERN_RIVER),
        (TERRAIN_PATTERN_RIVER, TERRAIN_PATTERN_RIVER),
    ];

    let mut allowed: [Vec<PatternId>; 4] = [vec![], vec![], vec![], vec![]];
    for &(a, b) in allowed_pairs {
        allowed[a as usize].push(b);
        if a != b {
            allowed[b as usize].push(a);
        }
    }

    let w = NonZeroU32::new(1).unwrap();
    let descriptions: Vec<PatternDescription> = (0..4_u32)
        .map(|id| {
            let nbrs = allowed[id as usize].clone();
            PatternDescription::new(
                Some(w),
                CardinalDirectionTable::new_array([
                    nbrs.clone(),
                    nbrs.clone(),
                    nbrs.clone(),
                    nbrs,
                ]),
            )
        })
        .collect();

    PatternTable::from_vec(descriptions)
}

// ── WorldConstraints: ForbidPattern ──────────────────────────────────────────

/// WorldMasks の制約を ForbidPattern として WFC ソルバーに渡す。
///
/// - `river_mask` が true のセル → RIVER に固定（`forbid_all_patterns_except`）
/// - `anchor_mask`（site | yard）が true のセル → River / Sand を禁止（`forbid_pattern`）
#[derive(Clone)]
pub struct WorldConstraints {
    fixed_river: Vec<Coord>,
    anchor_cells: Vec<Coord>,
}

impl WorldConstraints {
    /// WorldMasks の river_mask と anchor_mask から制約を構築する。
    ///
    /// **注意**: `fill_river_from_seed()` 適用済みの `masks` を渡すこと。
    pub fn from_masks(masks: &WorldMasks) -> Self {
        let mut fixed_river = Vec::new();
        let mut anchor_cells = Vec::new();

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let pos = (x, y);
                let coord = Coord::new(x, y);
                if masks.river_mask.get(pos) {
                    fixed_river.push(coord);
                }
                if masks.anchor_mask.get(pos) {
                    anchor_cells.push(coord);
                }
            }
        }

        WorldConstraints {
            fixed_river,
            anchor_cells,
        }
    }
}

impl ForbidPattern for WorldConstraints {
    fn forbid<W: Wrap, R: Rng>(&mut self, fi: &mut ForbidInterface<W>, rng: &mut R) {
        for &coord in &self.fixed_river {
            fi.forbid_all_patterns_except(coord, TERRAIN_PATTERN_RIVER, rng)
                .expect("river hard constraint caused contradiction");
        }
        for &coord in &self.anchor_cells {
            fi.forbid_pattern(coord, TERRAIN_PATTERN_RIVER, rng)
                .expect("anchor river forbid caused contradiction");
            fi.forbid_pattern(coord, TERRAIN_PATTERN_SAND, rng)
                .expect("anchor sand forbid caused contradiction");
        }
    }
}

// ── ソルバー呼び出しシグネチャ（実装は MS-WFC-2b） ──────────────────────────

#[derive(Debug)]
pub enum WfcError {
    Contradiction,
}

/// ソルバーを呼び出して TerrainType グリッドを返す（実装は MS-WFC-2b）。
///
/// # 引数
/// - `masks`: `fill_river_from_seed()` 適用済みの WorldMasks
/// - `seed`: サブシード（`master_seed + attempt * OFFSET` などで caller が計算する）
/// - `attempt`: 試行回数（ログ用）
#[allow(unused_variables)]
pub fn run_wfc(
    masks: &WorldMasks,
    seed: u64,
    attempt: u32,
) -> Result<Vec<TerrainType>, WfcError> {
    // MS-WFC-2b での実装イメージ（ここでは todo! のまま）:
    //   let table = build_pattern_table();
    //   let global_stats = GlobalStats::new(table);
    //   let constraints = WorldConstraints::from_masks(masks);
    //   let size = Size::new(MAP_WIDTH as u32, MAP_HEIGHT as u32);
    //   let mut rng = StdRng::seed_from_u64(seed);
    //   let mut run = RunOwn::new_forbid(size, &global_stats, constraints, &mut rng);
    //   run.collapse(&mut rng).map_err(|_| WfcError::Contradiction)?;
    //   // wave_cell_ref で各セルの確定パターンを取り出して TerrainType に変換
    todo!("MS-WFC-2b で実装: GlobalStats::new(build_pattern_table()) + RunOwn::new_forbid + collapse")
}

// ── コンパイル確認用インポート（2b まで直接使わないがシグネチャ検証のため） ─
const _: fn() = || {
    let _ = GlobalStats::new;
    let _ = PropagateError::Contradiction;
};
