use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;

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
            anchor_mask,
            river_protection_band: BitGrid::map_sized(),      // MS-WFC-2a で設定
            rock_protection_band: BitGrid::map_sized(),       // MS-WFC-2a で設定
            tree_dense_protection_band: BitGrid::map_sized(), // MS-WFC-2a で設定
            river_mask: BitGrid::map_sized(),                 // MS-WFC-2a で設定
            river_centerline: Vec::new(),                     // MS-WFC-2a で設定
        }
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
