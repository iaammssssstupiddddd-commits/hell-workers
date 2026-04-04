use std::collections::VecDeque;

use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;

// ── Protection band widths (wfc-ms0 §3.1) ────────────────────────────────────
/// アンカー外周の River 禁止帯幅（4 近傍 BFS 距離）
pub const PROTECTION_BAND_RIVER_WIDTH: u32 = 3;
/// アンカー外周の岩禁止帯幅
pub const PROTECTION_BAND_ROCK_WIDTH: u32 = 2;
/// アンカー外周の高密度木禁止帯幅
pub const PROTECTION_BAND_TREE_DENSE_WIDTH: u32 = 2;

/// 2D boolean グリッド（row-major, `x + y * width` indexing）。
#[derive(Debug, Clone)]
pub struct BitGrid {
    data: Vec<bool>,
    width: i32,
    height: i32,
}

impl BitGrid {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            data: vec![false; (width * height) as usize],
            width,
            height,
        }
    }

    /// MAP_WIDTH × MAP_HEIGHT で初期化するショートカット
    pub fn map_sized() -> Self {
        Self::new(MAP_WIDTH, MAP_HEIGHT)
    }

    pub fn get(&self, pos: GridPos) -> bool {
        match self.pos_to_idx(pos) {
            Some(i) => self.data[i],
            None => false,
        }
    }

    pub fn set(&mut self, pos: GridPos, val: bool) {
        if let Some(i) = self.pos_to_idx(pos) {
            self.data[i] = val;
        }
    }

    pub fn count_set(&self) -> usize {
        self.data.iter().filter(|&&b| b).count()
    }

    fn pos_to_idx(&self, pos: GridPos) -> Option<usize> {
        if pos.0 < 0 || pos.1 < 0 || pos.0 >= self.width || pos.1 >= self.height {
            return None;
        }
        Some((pos.1 * self.width + pos.0) as usize)
    }
}

impl std::ops::BitOrAssign<&BitGrid> for BitGrid {
    fn bitor_assign(&mut self, rhs: &BitGrid) {
        debug_assert_eq!(self.data.len(), rhs.data.len());
        for (a, b) in self.data.iter_mut().zip(&rhs.data) {
            *a |= b;
        }
    }
}

/// 各生成フェーズのマスク群。診断とデバッグに使う。
/// 各フィールドは該当セルが true のとき「そのカテゴリに属する」。
#[derive(Debug, Clone)]
pub struct WorldMasks {
    /// Site が占有するセル
    pub site_mask: BitGrid,
    /// Yard が占有するセル
    pub yard_mask: BitGrid,
    /// site_mask | yard_mask
    pub anchor_mask: BitGrid,
    /// anchor 外周の River 禁止帯（wfc-ms0 §3.1: PROTECTION_BAND_RIVER_WIDTH）
    pub river_protection_band: BitGrid,
    /// anchor 外周の岩禁止帯（wfc-ms0 §3.1: PROTECTION_BAND_ROCK_WIDTH）
    pub rock_protection_band: BitGrid,
    /// anchor 外周の高密度木禁止帯（wfc-ms0 §3.1: PROTECTION_BAND_TREE_DENSE_WIDTH）
    pub tree_dense_protection_band: BitGrid,
    /// 川タイル（WFC hard constraint として渡す）
    pub river_mask: BitGrid,
    /// 川の中心線点列（デバッグ表示・砂配置計算に使う）
    pub river_centerline: Vec<GridPos>,
    /// distance-field + growth を合成した「砂にしてよい元候補」
    /// （dist 1..=2 の base shoreline + dist==1 frontier からの bounded growth）
    pub sand_candidate_mask: BitGrid,
    /// seed 由来で candidate から削る連続 non-sand 領域
    pub sand_carve_mask: BitGrid,
    /// sand_candidate_mask から sand_carve_mask を除いた結果。post_process が最終的に Sand にするセル
    pub final_sand_mask: BitGrid,
    /// アンカーから遠い地点を起点にした Grass バイアスゾーン（MS-WFC-2.5）
    pub grass_zone_mask: BitGrid,
    /// アンカーに近い地点を起点にした Dirt バイアスゾーン（MS-WFC-2.5）
    pub dirt_zone_mask: BitGrid,
    /// grass_zone_mask 内に生成した内陸砂パッチ（砂浜とは独立、MS-WFC-2.5）
    pub inland_sand_mask: BitGrid,
    /// 川・砂浜の後段で確定する岩場パッチ（MS-WFC-3b）
    pub rock_field_mask: BitGrid,
    /// 各セルから最寄りの dirt_zone セルまでの 4 近傍最短距離（C: ゾーン端部グラデーション用）
    /// dirt_zone セル自体は 0、dirt_zone が空なら全セル u32::MAX
    pub dirt_zone_distance_field: Vec<u32>,
    /// 各セルから最寄りの grass_zone セルまでの 4 近傍最短距離（C: ゾーン端部グラデーション用）
    /// grass_zone セル自体は 0、grass_zone が空なら全セル u32::MAX
    pub grass_zone_distance_field: Vec<u32>,
}

impl WorldMasks {
    /// アンカー情報から site/yard/anchor マスクを初期化する。
    ///
    /// `river_*` / 各 `*_protection_band` フィールドは **MS-WFC-2a** で埋める。
    /// 帯の幾何は wfc-ms0-invariant-spec §3.1.1（アンカー外周からの 4 近傍距離）に従い、
    /// `anchor_mask` から純粋関数で BitGrid を生成する実装を推奨する。
    pub fn from_anchor(anchor: &crate::anchor::AnchorLayout) -> Self {
        let mut site_mask = BitGrid::map_sized();
        let mut yard_mask = BitGrid::map_sized();
        let mut anchor_mask = BitGrid::map_sized();

        for pos in anchor.site.iter_cells() {
            site_mask.set(pos, true);
            anchor_mask.set(pos, true);
        }
        for pos in anchor.yard.iter_cells() {
            yard_mask.set(pos, true);
            anchor_mask.set(pos, true);
        }

        WorldMasks {
            site_mask,
            yard_mask,
            anchor_mask: anchor_mask.clone(),
            river_protection_band: compute_protection_band(
                &anchor_mask,
                PROTECTION_BAND_RIVER_WIDTH,
            ),
            rock_protection_band: compute_protection_band(&anchor_mask, PROTECTION_BAND_ROCK_WIDTH),
            tree_dense_protection_band: compute_protection_band(
                &anchor_mask,
                PROTECTION_BAND_TREE_DENSE_WIDTH,
            ),
            river_mask: BitGrid::map_sized(), // fill_river_from_seed で設定
            river_centerline: Vec::new(),     // fill_river_from_seed で設定
            sand_candidate_mask: BitGrid::map_sized(), // fill_sand_from_river_seed で設定
            sand_carve_mask: BitGrid::map_sized(), // fill_sand_from_river_seed で設定
            final_sand_mask: BitGrid::map_sized(), // fill_sand_from_river_seed で設定
            grass_zone_mask: BitGrid::map_sized(), // fill_terrain_zones_from_seed で設定
            dirt_zone_mask: BitGrid::map_sized(), // fill_terrain_zones_from_seed で設定
            inland_sand_mask: BitGrid::map_sized(), // fill_terrain_zones_from_seed で設定
            rock_field_mask: BitGrid::map_sized(), // fill_rock_fields_from_seed で設定
            dirt_zone_distance_field: Vec::new(), // fill_terrain_zones_from_seed で設定
            grass_zone_distance_field: Vec::new(), // fill_terrain_zones_from_seed で設定
        }
    }

    /// `from_anchor` 済みの `anchor_mask` と `river_protection_band` を参照し、
    /// seed から deterministic に `river_mask` と `river_centerline` を生成して設定する。
    ///
    /// # Panics
    /// `from_anchor` が先に呼ばれていない場合（anchor_mask が空）に debug_assert で検出する。
    pub fn fill_river_from_seed(&mut self, seed: u64) {
        debug_assert!(
            self.anchor_mask.count_set() > 0,
            "fill_river_from_seed は from_anchor の後に呼ぶこと"
        );
        let (river_mask, centerline) =
            crate::river::generate_river_mask(seed, &self.anchor_mask, &self.river_protection_band);
        self.river_mask = river_mask;
        self.river_centerline = centerline;
    }

    /// `fill_river_from_seed()` 適用済みの `river_mask` を参照し、
    /// seed から deterministic に 3 つの砂マスクを生成して設定する。
    ///
    /// # Panics
    /// `fill_river_from_seed` が先に呼ばれていない場合（river_mask が空）に debug_assert で検出する。
    pub fn fill_sand_from_river_seed(&mut self, seed: u64) {
        debug_assert!(
            self.river_mask.count_set() > 0,
            "fill_sand_from_river_seed は fill_river_from_seed の後に呼ぶこと"
        );
        let (candidate, carve, final_mask) = crate::river::generate_sand_masks(
            seed,
            &self.river_mask,
            &self.anchor_mask,
            &self.river_protection_band,
        );
        self.sand_candidate_mask = candidate;
        self.sand_carve_mask = carve;
        self.final_sand_mask = final_mask;
    }

    /// `fill_sand_from_river_seed()` 適用済みの `final_sand_mask` を参照し、
    /// seed から deterministic に terrain zone masks と inland_sand_mask を生成して設定する。
    ///
    /// # Panics
    /// `fill_sand_from_river_seed` が先に呼ばれていない場合（final_sand_mask が空）に
    /// debug_assert で検出する。
    pub fn fill_terrain_zones_from_seed(&mut self, seed: u64) {
        debug_assert!(
            self.final_sand_mask.count_set() > 0,
            "fill_terrain_zones_from_seed は fill_sand_from_river_seed の後に呼ぶこと（final_sand_mask 非空を期待）"
        );
        let (grass, dirt, inland_sand) = crate::terrain_zones::generate_terrain_zone_masks(
            seed,
            &self.anchor_mask,
            &self.river_mask,
            &self.river_protection_band,
            &self.final_sand_mask,
        );
        self.grass_zone_mask = grass;
        self.dirt_zone_mask = dirt;
        self.inland_sand_mask = inland_sand;
        self.dirt_zone_distance_field =
            crate::terrain_zones::compute_zone_distance_field(&self.dirt_zone_mask);
        self.grass_zone_distance_field =
            crate::terrain_zones::compute_zone_distance_field(&self.grass_zone_mask);
    }

    /// `fill_terrain_zones_from_seed()` 適用済みの inland_sand を含むマスク群を参照し、
    /// seed から deterministic に岩場マスクを生成して設定する。
    pub fn fill_rock_fields_from_seed(&mut self, seed: u64) {
        self.rock_field_mask = crate::rock_fields::generate_rock_field_mask(
            seed,
            &self.anchor_mask,
            &self.rock_protection_band,
            &self.river_mask,
            &self.final_sand_mask,
            &self.inland_sand_mask,
        );
    }

    /// debug report 用の合成保護帯。
    /// wfc-ms0 でいう `protection_band` はこの合成結果に相当する。
    pub fn combined_protection_band(&self) -> BitGrid {
        let mut combined = self.river_protection_band.clone();
        combined |= &self.rock_protection_band;
        combined |= &self.tree_dense_protection_band;
        combined
    }
}

/// anchor_mask の外周から 4 近傍 BFS で距離変換し、
/// 距離 1..=width のセルを true にした BitGrid を返す。
///
/// wfc-ms0 §3.1.1 準拠:
/// - アンカー占有セル自体は含まない（d = 0 相当）
/// - マップ外は到達不可
pub fn compute_protection_band(anchor_mask: &BitGrid, width: u32) -> BitGrid {
    let w = MAP_WIDTH;
    let h = MAP_HEIGHT;
    let mut band = BitGrid::map_sized();
    let mut dist: Vec<u32> = vec![u32::MAX; (w * h) as usize];
    let mut queue: VecDeque<GridPos> = VecDeque::new();

    const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

    // アンカー境界に隣接する非アンカーセルを距離 1 としてキューに積む
    for y in 0..h {
        for x in 0..w {
            if !anchor_mask.get((x, y)) {
                continue;
            }
            for (dx, dy) in DIRS {
                let nx = x + dx;
                let ny = y + dy;
                if nx < 0 || nx >= w || ny < 0 || ny >= h {
                    continue;
                }
                let idx = (ny * w + nx) as usize;
                if !anchor_mask.get((nx, ny)) && dist[idx] == u32::MAX {
                    dist[idx] = 1;
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    // BFS で距離を伝播; width を超えたセルは band に含めない
    while let Some(pos) = queue.pop_front() {
        let d = dist[(pos.1 * w + pos.0) as usize];
        if d > width {
            continue;
        }
        band.set(pos, true);
        for (dx, dy) in DIRS {
            let nx = pos.0 + dx;
            let ny = pos.1 + dy;
            if nx < 0 || nx >= w || ny < 0 || ny >= h {
                continue;
            }
            let idx = (ny * w + nx) as usize;
            if !anchor_mask.get((nx, ny)) && dist[idx] == u32::MAX {
                dist[idx] = d + 1;
                queue.push_back((nx, ny));
            }
        }
    }

    band
}
