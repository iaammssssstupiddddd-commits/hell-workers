use super::*;

// ── PatternId 定数（配列インデックスとして PatternTable に登録する順序） ─────
pub const TERRAIN_PATTERN_GRASS: PatternId = 0;
pub const TERRAIN_PATTERN_DIRT: PatternId = 1;
pub const TERRAIN_PATTERN_SAND: PatternId = 2;
pub const TERRAIN_PATTERN_RIVER: PatternId = 3;

// ── 完全中立リージョンバイアス定数 ───────────────────────────────────────────
/// 完全中立エリア内リージョンの 1 辺サイズ（マス）
pub(super) const NEUTRAL_REGION_SIZE: i32 = 8;
/// 完全中立リージョン内の Grass/Dirt 変換確率（%）
pub(super) const NEUTRAL_REGION_BIAS_PERCENT: u32 = 20;

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
