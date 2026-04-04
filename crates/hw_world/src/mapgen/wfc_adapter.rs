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

// ── PatternId 定数（配列インデックスとして PatternTable に登録する順序） ─────
pub const TERRAIN_PATTERN_GRASS: PatternId = 0;
pub const TERRAIN_PATTERN_DIRT: PatternId = 1;
pub const TERRAIN_PATTERN_SAND: PatternId = 2;
pub const TERRAIN_PATTERN_RIVER: PatternId = 3;

// ── 完全中立リージョンバイアス定数 ───────────────────────────────────────────
/// 完全中立エリア内リージョンの 1 辺サイズ（マス）
const NEUTRAL_REGION_SIZE: i32 = 8;
/// 完全中立リージョン内の Grass/Dirt 変換確率（%）
const NEUTRAL_REGION_BIAS_PERCENT: u32 = 20;

// ── タイル重み定数 ────────────────────────────────────────────────────────────
pub const WEIGHT_GRASS: u32 = 9;
pub const WEIGHT_DIRT: u32 = 2;
/// Sand の重み（F4: 川隣接セルにしか Sand を配置しない前提のバイアス値）
pub const WEIGHT_SAND: u32 = SAND_ADJACENT_TO_RIVER_WEIGHT;
/// 川に隣接する Sand タイルの重み（F4 基準値）
pub const SAND_ADJACENT_TO_RIVER_WEIGHT: u32 = 10;
/// 将来、内陸 Sand を解禁する場合の予約値（2b 初版では未使用）
pub const SAND_NON_ADJACENT_WEIGHT: u32 = 1;

/// WFC ソルバーの最大 retry 回数
pub const MAX_WFC_RETRIES: u32 = 64;

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

/// ゲームロジックに基づく隣接ルールと重みを PatternTable として構築する。
///
/// 許可する隣接（対称・全 4 方向）:
/// - Grass ↔ Grass / Dirt / Sand
/// - Dirt ↔ Dirt / Sand
/// - Sand ↔ Sand / River
/// - River ↔ River
///
/// River ↔ Grass / River ↔ Dirt は禁止。
/// River のマスク外伝播は `WorldConstraints` で別途抑制する。
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

    // River は unweighted (None) にする。
    // forbid_pattern(coord, RIVER) 呼び出しが num_weighted を変えないため
    // observer の初期エントリが有効に保たれる。マスク外への出現は
    // river_forbidden_cells の forbid_pattern で防ぐ。
    let weights: [Option<NonZeroU32>; 4] = [
        NonZeroU32::new(WEIGHT_GRASS),
        NonZeroU32::new(WEIGHT_DIRT),
        NonZeroU32::new(WEIGHT_SAND),
        None, // River: unweighted（hard constraint が出現先を制御）
    ];

    let descriptions: Vec<PatternDescription> = (0..4_u32)
        .map(|id| {
            let nbrs = allowed[id as usize].clone();
            PatternDescription::new(
                weights[id as usize],
                CardinalDirectionTable::new_array([nbrs.clone(), nbrs.clone(), nbrs.clone(), nbrs]),
            )
        })
        .collect();

    PatternTable::from_vec(descriptions)
}

// ── WorldConstraints: ForbidPattern ──────────────────────────────────────────

/// WorldMasks の制約を ForbidPattern として WFC ソルバーに渡す。
///
/// ## 設計上の制約
/// wfc ライブラリは priority queue を `forbid()` 呼び出し **前** に初期化する。
/// そのため `forbid_pattern` で weighted パターン（Grass/Dirt/Sand）を直接除去すると
/// `num_weighted_compatible_patterns` が変わり、初期エントリが stale になる。
/// stale エントリは `choose_next_cell` でスキップされるため、該当セルが
/// 未 collapse のまま `collapse()` が `Ok(())` を返してしまう。
///
/// **適用可能な制約**: weighted パターンを **直接変更しない** ものに限る。
/// - `fixed_river`: River セル → `forbid_all_patterns_except(RIVER)` で固定
///   （cascade で隣接セルから Grass/Dirt を除去するが、propagation 経由なので
///   `entropy_changes_by_coord` に新エントリが追加され stale 問題が起きない）
/// - `river_forbidden_cells`: 非 River セルから River（**unweighted**）を除去
///   （unweighted なので `num_weighted` が変わらず初期エントリが有効なまま）
///
/// **適用不可（ポスト処理で対応）**:
/// - anchor セルの Sand 禁止 → MS-WFC-2d: `final_sand_mask` が anchor と交差しないため自然に満たされる
/// - 川非隣接セルの Sand 禁止 → MS-WFC-2d: `post_process_tiles()` が `final_sand_mask` 主導で処理
#[derive(Clone)]
pub struct WorldConstraints {
    fixed_river: Vec<Coord>,
    river_forbidden_cells: Vec<Coord>,
}

pub const CARDINAL_DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

impl WorldConstraints {
    /// `fill_river_from_seed()` 適用済みの `WorldMasks` から制約を構築する。
    pub fn from_masks(masks: &WorldMasks) -> Self {
        let mut fixed_river = Vec::new();
        let mut river_forbidden_cells = Vec::new();

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let coord = Coord::new(x, y);
                if masks.river_mask.get((x, y)) {
                    fixed_river.push(coord);
                } else {
                    river_forbidden_cells.push(coord);
                }
            }
        }

        WorldConstraints {
            fixed_river,
            river_forbidden_cells,
        }
    }
}

impl ForbidPattern for WorldConstraints {
    fn forbid<W: Wrap, R: Rng>(&mut self, fi: &mut ForbidInterface<W>, rng: &mut R) {
        // River マスクセル → River に固定。
        // cascade で隣接セルから Grass/Dirt が propagation 経由で除去され
        // entropy_changes_by_coord に正しいエントリが追加される。
        for &coord in &self.fixed_river {
            fi.forbid_all_patterns_except(coord, TERRAIN_PATTERN_RIVER, rng)
                .expect("river hard constraint caused contradiction");
        }
        // 非 River セル → River を禁止。River は unweighted なので
        // num_weighted が変わらず priority queue の初期エントリが stale にならない。
        for &coord in &self.river_forbidden_cells {
            fi.forbid_pattern(coord, TERRAIN_PATTERN_RIVER, rng)
                .expect("river forbid outside river_mask caused contradiction");
        }
        // NOTE: anchor Sand 禁止・川非隣接 Sand 禁止は forbid_pattern では
        // num_weighted を変えるため stale entry 問題が発生する。
        // これらは run_wfc() 内の post_process_tiles() でポスト処理する。
    }
}

// ── ヘルパー関数 ──────────────────────────────────────────────────────────────

/// `master_seed` と `attempt` から deterministic に sub_seed を導出する。
/// splitmix64 の 1 ステップを使ってビットを分散させる。
pub(crate) fn derive_sub_seed(master_seed: u64, attempt: u32) -> u64 {
    master_seed.wrapping_add((attempt as u64).wrapping_mul(0x9e3779b97f4a7c15))
}

/// WFC が全試行で収束しなかった場合の安全マップ（MS-WFC-2d 版）。
/// hard constraint（River マスク・anchor 禁止）は維持しつつ、
/// final_sand_mask 上は Sand、残りは Grass で埋める。
/// MS-WFC-2.5 以降はゾーンバイアスと inland_sand も適用する。
pub(crate) fn fallback_terrain(masks: &WorldMasks, master_seed: u64) -> Vec<TerrainType> {
    let mut tiles = vec![TerrainType::Grass; (MAP_WIDTH * MAP_HEIGHT) as usize];
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get((x, y)) {
                tiles[idx] = TerrainType::River;
            } else if masks.final_sand_mask.get((x, y)) {
                tiles[idx] = TerrainType::Sand;
            }
        }
    }
    // attempt 非依存の専用 seed で zone / inland_sand を適用
    let mut rng = StdRng::seed_from_u64(fallback_post_seed(master_seed));
    apply_zone_post_process(&mut tiles, masks, &mut rng);
    tiles
}

/// `fallback_terrain` 専用の post-process seed。attempt 非依存。
fn fallback_post_seed(master_seed: u64) -> u64 {
    master_seed ^ 0xfb7c_3a91_d5e2_4608
}

/// Step 4（ゾーンバイアス）、Step 4.5（rock field dirt 強制）、
/// Step 5（inland sand）を共通化したヘルパ。
/// `post_process_tiles` と `fallback_terrain` 両方から呼ぶ。
fn apply_zone_post_process(tiles: &mut [TerrainType], masks: &WorldMasks, rng: &mut StdRng) {
    // Step 4: zone bias（B: 確率的フリップ・強制率を範囲でランダム化）
    // + C: ゾーン端部グラデーションバイアス
    // + 完全中立リージョンバイアス（8×8 タイル単位でリージョンを Grass/Dirt 寄りに振り分け）
    let region_seed: u64 = Rng::r#gen::<u64>(rng);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get((x, y))
                || tiles[idx] == TerrainType::River
                || tiles[idx] == TerrainType::Sand
            {
                continue;
            }
            if masks.grass_zone_mask.get((x, y)) {
                // B: Grass ゾーン → 強制率を [ENFORCE_MIN, ENFORCE_MAX] からランダムに選ぶ
                let threshold = rng.gen_range(ZONE_GRASS_ENFORCE_MIN..=ZONE_GRASS_ENFORCE_MAX);
                if tiles[idx] == TerrainType::Dirt && rng.gen_range(0..100) < threshold {
                    tiles[idx] = TerrainType::Grass;
                }
            } else if masks.dirt_zone_mask.get((x, y)) {
                // B: Dirt ゾーン → 強制率を [ENFORCE_MIN, ENFORCE_MAX] からランダムに選ぶ
                let threshold = rng.gen_range(ZONE_DIRT_ENFORCE_MIN..=ZONE_DIRT_ENFORCE_MAX);
                if tiles[idx] == TerrainType::Grass && rng.gen_range(0..100) < threshold {
                    tiles[idx] = TerrainType::Dirt;
                }
            } else {
                // C: ゾーン端部 ZONE_GRADIENT_WIDTH マス以内の中立セルにグラデーションバイアス
                // 両ゾーンが範囲内の場合は近い方を優先
                let dirt_dist = masks.dirt_zone_distance_field[idx];
                let grass_dist = masks.grass_zone_distance_field[idx];
                let dirt_near = dirt_dist <= ZONE_GRADIENT_WIDTH;
                let grass_near = grass_dist <= ZONE_GRADIENT_WIDTH;
                if dirt_near
                    && (!grass_near || dirt_dist <= grass_dist)
                    && tiles[idx] == TerrainType::Grass
                    && rng.gen_range(0..100) < ZONE_GRADIENT_DIRT_BIAS_PERCENT
                {
                    tiles[idx] = TerrainType::Dirt;
                } else if grass_near
                    && tiles[idx] == TerrainType::Dirt
                    && rng.gen_range(0..100) < ZONE_GRADIENT_GRASS_BIAS_PERCENT
                {
                    tiles[idx] = TerrainType::Grass;
                } else if !dirt_near && !grass_near {
                    // 完全中立: 8×8 リージョン単位で Grass/Dirt 寄りに振り分け
                    let rx = (x / NEUTRAL_REGION_SIZE) as u64;
                    let ry = (y / NEUTRAL_REGION_SIZE) as u64;
                    let h = rx
                        .wrapping_mul(2_654_435_761u64)
                        .wrapping_add(ry.wrapping_mul(1_234_567_891u64))
                        .wrapping_add(region_seed);
                    if h & 1 == 0 {
                        if tiles[idx] == TerrainType::Dirt
                            && rng.gen_range(0..100) < NEUTRAL_REGION_BIAS_PERCENT
                        {
                            tiles[idx] = TerrainType::Grass;
                        }
                    } else if tiles[idx] == TerrainType::Grass
                        && rng.gen_range(0..100) < NEUTRAL_REGION_BIAS_PERCENT
                    {
                        tiles[idx] = TerrainType::Dirt;
                    }
                }
            }
        }
    }
    // Step 4.5: rock fields（zone bias 後に Dirt を強制）
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if masks.rock_field_mask.get((x, y)) {
                tiles[(y * MAP_WIDTH + x) as usize] = TerrainType::Dirt;
            }
        }
    }

    // Step 5: inland sand（zone bias 後の状態を参照）
    const OCTILE_DIRS: [(i32, i32); 8] = [
        (0, 1),
        (0, -1),
        (1, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if !masks.inland_sand_mask.get((x, y)) {
                continue;
            }
            if tiles[idx] == TerrainType::River || tiles[idx] == TerrainType::Sand {
                continue;
            }
            let all_grass = OCTILE_DIRS.iter().all(|&(dx, dy)| {
                let nx = x + dx;
                let ny = y + dy;
                if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                    return false;
                }
                tiles[(ny * MAP_WIDTH + nx) as usize] == TerrainType::Grass
            });
            if all_grass {
                tiles[idx] = TerrainType::Sand;
            }
        }
    }
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
pub fn run_wfc(masks: &WorldMasks, seed: u64, attempt: u32) -> Result<Vec<TerrainType>, WfcError> {
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

/// WFC 後のポスト処理（MS-WFC-2d 版）。
///
/// wfc ライブラリの制約として、weighted パターンの `forbid_pattern` は
/// priority queue を stale にするため `WorldConstraints::forbid()` では適用できない。
/// 代わりにここで `final_sand_mask` を主軸として以下の制約を強制する:
///
/// 処理順:
/// 1. river_mask セルは常に River のまま（WFC で固定済み）
/// 2. final_sand_mask セルは強制 Sand（WFC 結果に関わらず上書き）
/// 3. final_sand_mask 外で terrain == Sand の stray Sand を Grass/Dirt に置換
/// 4. ゾーンバイアス + inland sand（apply_zone_post_process に委譲）
fn post_process_tiles(tiles: &mut [TerrainType], masks: &WorldMasks, rng: &mut StdRng) {
    let total = WEIGHT_GRASS + WEIGHT_DIRT;
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get((x, y)) {
                // River は WFC で固定済み。変更しない。
                continue;
            }
            if masks.final_sand_mask.get((x, y)) {
                // マスク上のセルは必ず Sand に揃える
                tiles[idx] = TerrainType::Sand;
            } else if tiles[idx] == TerrainType::Sand {
                // マスク外の stray Sand を Grass/Dirt に落とす
                let r = rng.gen_range(0..total);
                tiles[idx] = if r < WEIGHT_GRASS {
                    TerrainType::Grass
                } else {
                    TerrainType::Dirt
                };
            }
        }
    }
    // ── 追加: Step 4/5 を共通ヘルパに委譲（§3.6 Option A） ─────────────────
    apply_zone_post_process(tiles, masks, rng);
}
