use hw_core::constants::{
    MAP_HEIGHT, MAP_WIDTH, SITE_HEIGHT_TILES, SITE_WIDTH_TILES, YARD_INITIAL_HEIGHT_TILES,
    YARD_INITIAL_WIDTH_TILES, YARD_MIN_HEIGHT_TILES, YARD_MIN_WIDTH_TILES,
};
use hw_core::world::GridPos;
use std::fmt;

/// 矩形グリッド領域（両端 inclusive）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridRect {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32, // inclusive
    pub max_y: i32, // inclusive
}

impl GridRect {
    pub fn contains(&self, pos: GridPos) -> bool {
        pos.0 >= self.min_x && pos.0 <= self.max_x && pos.1 >= self.min_y && pos.1 <= self.max_y
    }

    /// セル数（面積）
    pub fn area(&self) -> usize {
        ((self.max_x - self.min_x + 1) * (self.max_y - self.min_y + 1)) as usize
    }

    /// 全セルを row-major でイテレートする
    pub fn iter_cells(&self) -> impl Iterator<Item = GridPos> + '_ {
        let (min_x, min_y, max_x, max_y) = (self.min_x, self.min_y, self.max_x, self.max_y);
        (min_y..=max_y).flat_map(move |y| (min_x..=max_x).map(move |x| (x, y)))
    }
}

/// マップ上の固定アンカー配置。pure data（Bevy・WorldMap 依存なし）。
#[derive(Debug, Clone)]
pub struct AnchorLayout {
    /// Site が占有する矩形（両端 inclusive）
    pub site: GridRect,
    /// Yard が占有する矩形（両端 inclusive）
    pub yard: GridRect,
    /// Yard 内固定の初期木材座標（全点が yard 内に収まる）
    pub initial_wood_positions: Vec<GridPos>,
    /// Yard 内固定の猫車置き場フットプリント（2×2, 両端 inclusive）
    pub wheelbarrow_parking: GridRect,
}

/// 固定アンカー定数が無効なときの理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorLayoutError {
    SiteTooSmallForMinYard,
    YardInitialTooSmall,
    SiteOutOfBounds,
    YardOutOfBounds,
}

impl fmt::Display for AnchorLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SiteTooSmallForMinYard => write!(
                f,
                "site size {}x{} is smaller than yard minimum {}x{}",
                SITE_WIDTH_TILES, SITE_HEIGHT_TILES, YARD_MIN_WIDTH_TILES, YARD_MIN_HEIGHT_TILES
            ),
            Self::YardInitialTooSmall => write!(
                f,
                "initial yard size {}x{} is smaller than minimum {}x{}",
                YARD_INITIAL_WIDTH_TILES,
                YARD_INITIAL_HEIGHT_TILES,
                YARD_MIN_WIDTH_TILES,
                YARD_MIN_HEIGHT_TILES
            ),
            Self::SiteOutOfBounds => write!(
                f,
                "site configured size does not fit map ({:?}x{:?})",
                SITE_WIDTH_TILES, SITE_HEIGHT_TILES
            ),
            Self::YardOutOfBounds => write!(
                f,
                "yard configured size does not fit map ({:?}x{:?})",
                YARD_INITIAL_WIDTH_TILES, YARD_INITIAL_HEIGHT_TILES
            ),
        }
    }
}

impl AnchorLayout {
    /// 現行定数から固定配置を計算して返す。
    ///
    /// `bevy_app::compute_site_yard_layout()` もこの関数を基準に構築する。
    ///
    /// `SITE_WIDTH_TILES` 等は `f32` のため `as i32` キャストを行う（整数値なので截捨は影響なし）。
    /// `MAP_WIDTH` / `MAP_HEIGHT` は `i32` なので直接使用可。
    pub fn fixed() -> Self {
        Self::try_fixed().expect("AnchorLayout::fixed() requires valid site/yard constants")
    }

    /// 固定アンカー定数を検証しつつレイアウトを返す。
    pub fn try_fixed() -> Result<Self, AnchorLayoutError> {
        let site_w = SITE_WIDTH_TILES as i32;
        let site_h = SITE_HEIGHT_TILES as i32;
        let yard_w = YARD_INITIAL_WIDTH_TILES as i32;
        let yard_h = YARD_INITIAL_HEIGHT_TILES as i32;

        if SITE_WIDTH_TILES < YARD_MIN_WIDTH_TILES || SITE_HEIGHT_TILES < YARD_MIN_HEIGHT_TILES {
            return Err(AnchorLayoutError::SiteTooSmallForMinYard);
        }

        if YARD_INITIAL_WIDTH_TILES < YARD_MIN_WIDTH_TILES
            || YARD_INITIAL_HEIGHT_TILES < YARD_MIN_HEIGHT_TILES
        {
            return Err(AnchorLayoutError::YardInitialTooSmall);
        }

        let site_min_x = (MAP_WIDTH - site_w) / 2; // = 30
        let site_min_y = (MAP_HEIGHT - site_h) / 2; // = 40
        let site_max_x = site_min_x + site_w - 1; // = 69
        let site_max_y = site_min_y + site_h - 1; // = 59

        if site_min_x < 0 || site_min_y < 0 || site_max_x >= MAP_WIDTH || site_max_y >= MAP_HEIGHT
        {
            return Err(AnchorLayoutError::SiteOutOfBounds);
        }

        let yard_min_x = site_max_x + 1; // = 70
        let yard_min_y = site_min_y; // = 40
        let yard_max_x = yard_min_x + yard_w - 1; // = 89
        let yard_max_y = yard_min_y + yard_h - 1; // = 59

        if yard_max_x >= MAP_WIDTH || yard_max_y >= MAP_HEIGHT {
            return Err(AnchorLayoutError::YardOutOfBounds);
        }

        Ok(AnchorLayout {
            site: GridRect {
                min_x: site_min_x,
                min_y: site_min_y,
                max_x: site_max_x,
                max_y: site_max_y,
            },
            yard: GridRect {
                min_x: yard_min_x,
                min_y: yard_min_y,
                max_x: yard_max_x,
                max_y: yard_max_y,
            },
            // Yard 左端付近（yard_min_x + 1〜5、中央 y 付近）に 5 点配置
            // 旧 INITIAL_WOOD_POSITIONS は Site 内 (48-53, 46-52) — 誤り（MS-WFC-1 §2.2）
            initial_wood_positions: vec![
                (yard_min_x + 1, yard_min_y + 5), // (71, 45)
                (yard_min_x + 2, yard_min_y + 4), // (72, 44)
                (yard_min_x + 3, yard_min_y + 6), // (73, 46)
                (yard_min_x + 4, yard_min_y + 3), // (74, 43)
                (yard_min_x + 5, yard_min_y + 8), // (75, 48)
            ],
            // 旧 INITIAL_WHEELBARROW_PARKING_GRID = (58, 58) は Site 内 — 誤り（MS-WFC-1 §2.2）
            // Yard 中央寄りの 2×2 空間を確保: (82,52)-(83,53)
            wheelbarrow_parking: GridRect {
                min_x: yard_min_x + 12,
                min_y: yard_min_y + 12,
                max_x: yard_min_x + 13,
                max_y: yard_min_y + 13,
            },
        })
    }

    /// Site と Yard の合成マスク判定
    pub fn is_anchor_cell(&self, pos: GridPos) -> bool {
        self.site.contains(pos) || self.yard.contains(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_layout_fixed_wood_in_yard() {
        let layout = AnchorLayout::fixed();
        for pos in &layout.initial_wood_positions {
            assert!(
                layout.yard.contains(*pos),
                "initial_wood_positions {:?} is not inside Yard {:?}",
                pos,
                layout.yard
            );
        }
    }

    #[test]
    fn anchor_layout_fixed_parking_in_yard() {
        let layout = AnchorLayout::fixed();
        for pos in layout.wheelbarrow_parking.iter_cells() {
            assert!(
                layout.yard.contains(pos),
                "parking footprint {:?} not inside Yard",
                pos
            );
        }
    }

    #[test]
    fn anchor_layout_fixed_site_not_in_yard() {
        let layout = AnchorLayout::fixed();
        // 旧誤り位置 (58,58) が Site 内にあること（退行テスト）
        assert!(layout.site.contains((58, 58)));
        assert!(!layout.yard.contains((58, 58)));
    }

    #[test]
    fn anchor_layout_try_fixed_matches_fixed() {
        let via_result = AnchorLayout::try_fixed().expect("valid constants");
        let via_fixed = AnchorLayout::fixed();

        assert_eq!(via_result.site, via_fixed.site);
        assert_eq!(via_result.yard, via_fixed.yard);
        assert_eq!(via_result.initial_wood_positions, via_fixed.initial_wood_positions);
        assert_eq!(via_result.wheelbarrow_parking, via_fixed.wheelbarrow_parking);
    }
}
