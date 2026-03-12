use crate::world::map::WorldMap;
use hw_core::constants::*;
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

/// Site と Yard のグリッド配置を計算する。
/// 定数のみを参照する pure 関数。
pub fn compute_site_yard_layout() -> Result<SiteYardLayout, SiteYardLayoutError> {
    let site_width = SITE_WIDTH_TILES as i32;
    let site_height = SITE_HEIGHT_TILES as i32;
    let yard_width = YARD_INITIAL_WIDTH_TILES as i32;
    let yard_height = YARD_INITIAL_HEIGHT_TILES as i32;

    if SITE_WIDTH_TILES < YARD_MIN_WIDTH_TILES || SITE_HEIGHT_TILES < YARD_MIN_HEIGHT_TILES {
        return Err(SiteYardLayoutError::SiteTooSmallForMinYard);
    }

    if YARD_INITIAL_WIDTH_TILES < YARD_MIN_WIDTH_TILES
        || YARD_INITIAL_HEIGHT_TILES < YARD_MIN_HEIGHT_TILES
    {
        return Err(SiteYardLayoutError::YardInitialTooSmall);
    }

    let site_min_x = (MAP_WIDTH - site_width) / 2;
    let site_min_y = (MAP_HEIGHT - site_height) / 2;
    let site_max_x = site_min_x + site_width - 1;
    let site_max_y = site_min_y + site_height - 1;

    if site_min_x < 0 || site_min_y < 0 || site_max_x >= MAP_WIDTH || site_max_y >= MAP_HEIGHT {
        return Err(SiteYardLayoutError::SiteOutOfBounds);
    }

    let yard_min_x = site_max_x + 1;
    let yard_min_y = site_min_y;
    let yard_max_x = yard_min_x + yard_width - 1;
    let yard_max_y = yard_min_y + yard_height - 1;

    if yard_max_x >= MAP_WIDTH || yard_max_y >= MAP_HEIGHT {
        return Err(SiteYardLayoutError::YardOutOfBounds);
    }

    Ok(SiteYardLayout {
        site_min_x,
        site_min_y,
        site_max_x,
        site_max_y,
        yard_min_x,
        yard_min_y,
        yard_max_x,
        yard_max_y,
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

    if occupied.iter().any(|(gx, gy)| !world_map.is_walkable(*gx, *gy)) {
        return None;
    }

    Some(ParkingLayout { base, occupied })
}
