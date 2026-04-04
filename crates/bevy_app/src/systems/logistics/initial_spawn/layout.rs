use crate::world::map::WorldMap;
use hw_world::AnchorLayout;

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

/// `hw_world` のアンカー矩形を `SiteYardLayout` に写す（ワールド生成結果と表示を一致させる）。
pub fn site_yard_layout_from_anchor(anchor: &AnchorLayout) -> SiteYardLayout {
    SiteYardLayout {
        site_min_x: anchor.site.min_x,
        site_min_y: anchor.site.min_y,
        site_max_x: anchor.site.max_x,
        site_max_y: anchor.site.max_y,
        yard_min_x: anchor.yard.min_x,
        yard_min_y: anchor.yard.min_y,
        yard_max_x: anchor.yard.max_x,
        yard_max_y: anchor.yard.max_y,
    }
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
    fn site_yard_layout_from_anchor_matches_fixed() {
        let anchor = AnchorLayout::fixed();
        let layout = site_yard_layout_from_anchor(&anchor);

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
