use crate::world::map::WorldMap;
use hw_core::constants::*;
use hw_world::{AnchorLayout, AnchorLayoutError};
use std::fmt;

/// Site と Yard のグリッド配置を表す純粋データ構造。
/// `WorldMap` や `Commands` には依存しない。
pub struct SiteYardLayout {
    pub site_min_x: i32,
    pub site_min_y: i32,
    pub site_max_x: i32,
    pub site_max_y: i32,
    pub yard_min_x: i32,
    pub yard_min_y: i32,
    pub yard_max_x: i32,
    pub yard_max_y: i32,
}

/// Site/Yard レイアウト計算が失敗した理由。
#[derive(Debug)]
pub enum SiteYardLayoutError {
    SiteTooSmallForMinYard,
    YardInitialTooSmall,
    SiteOutOfBounds,
    YardOutOfBounds,
}

impl fmt::Display for SiteYardLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SiteYardLayoutError::SiteTooSmallForMinYard => write!(
                f,
                "site size {}x{} is smaller than yard minimum {}x{}",
                SITE_WIDTH_TILES, SITE_HEIGHT_TILES, YARD_MIN_WIDTH_TILES, YARD_MIN_HEIGHT_TILES
            ),
            SiteYardLayoutError::YardInitialTooSmall => write!(
                f,
                "initial yard size {}x{} is smaller than minimum {}x{}",
                YARD_INITIAL_WIDTH_TILES,
                YARD_INITIAL_HEIGHT_TILES,
                YARD_MIN_WIDTH_TILES,
                YARD_MIN_HEIGHT_TILES
            ),
            SiteYardLayoutError::SiteOutOfBounds => write!(
                f,
                "site configured size does not fit map ({:?}x{:?})",
                SITE_WIDTH_TILES, SITE_HEIGHT_TILES
            ),
            SiteYardLayoutError::YardOutOfBounds => write!(
                f,
                "yard configured size does not fit map ({:?}x{:?})",
                YARD_INITIAL_WIDTH_TILES, YARD_INITIAL_HEIGHT_TILES
            ),
        }
    }
}

impl From<AnchorLayoutError> for SiteYardLayoutError {
    fn from(value: AnchorLayoutError) -> Self {
        match value {
            AnchorLayoutError::SiteTooSmallForMinYard => Self::SiteTooSmallForMinYard,
            AnchorLayoutError::YardInitialTooSmall => Self::YardInitialTooSmall,
            AnchorLayoutError::SiteOutOfBounds => Self::SiteOutOfBounds,
            AnchorLayoutError::YardOutOfBounds => Self::YardOutOfBounds,
        }
    }
}

/// Site と Yard のグリッド配置を計算する。
/// `hw_world::AnchorLayout::try_fixed()` を単一ソースとして使い、
/// bevy_app 側が必要とする矩形レイアウトへ変換する pure 関数。
pub fn compute_site_yard_layout() -> Result<SiteYardLayout, SiteYardLayoutError> {
    let anchor = AnchorLayout::try_fixed().map_err(SiteYardLayoutError::from)?;

    Ok(SiteYardLayout {
        site_min_x: anchor.site.min_x,
        site_min_y: anchor.site.min_y,
        site_max_x: anchor.site.max_x,
        site_max_y: anchor.site.max_y,
        yard_min_x: anchor.yard.min_x,
        yard_min_y: anchor.yard.min_y,
        yard_max_x: anchor.yard.max_x,
        yard_max_y: anchor.yard.max_y,
    })
}

/// WheelbarrowParking の 2x2 占有グリッドを表す純粋データ構造。
pub struct ParkingLayout {
    pub base: (i32, i32),
    pub occupied: [(i32, i32); 4],
}

/// WheelbarrowParking のグリッド占有を計算し、全マスが通行可能なら `Some` を返す。
pub fn compute_parking_layout(base: (i32, i32), world_map: &WorldMap) -> Option<ParkingLayout> {
    let occupied = [
        base,
        (base.0 + 1, base.1),
        (base.0, base.1 + 1),
        (base.0 + 1, base.1 + 1),
    ];

    if occupied
        .iter()
        .any(|(gx, gy)| !world_map.is_walkable(*gx, *gy))
    {
        return None;
    }

    Some(ParkingLayout { base, occupied })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_site_yard_layout_matches_anchor_layout() {
        let anchor = AnchorLayout::fixed();
        let layout = compute_site_yard_layout().expect("valid site/yard layout");

        assert_eq!(layout.site_min_x, anchor.site.min_x);
        assert_eq!(layout.site_min_y, anchor.site.min_y);
        assert_eq!(layout.site_max_x, anchor.site.max_x);
        assert_eq!(layout.site_max_y, anchor.site.max_y);
        assert_eq!(layout.yard_min_x, anchor.yard.min_x);
        assert_eq!(layout.yard_min_y, anchor.yard.min_y);
        assert_eq!(layout.yard_max_x, anchor.yard.max_x);
        assert_eq!(layout.yard_max_y, anchor.yard.max_y);
    }
}
