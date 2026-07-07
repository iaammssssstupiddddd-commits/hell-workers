use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};
use hw_world::{TerrainType, WorldMasks, grid_to_world};

// ── 境界種別 ─────────────────────────────────────────────────────────────────

/// 隣接する 2 種類の TerrainType ペア（無向）を表す列挙型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoundaryKind {
    GrassDirt,
    GrassSand,
    GrassRiver,
    DirtSand,
    DirtRiver,
    SandRiver,
    /// 同じ `TerrainType` の草どうしで、草／中立／土ゾーン（`terrain_zone_bias_byte`）が隣接で変わる境。
    GrassZoneTone,
    /// 同じ `TerrainType` の土どうしで、ゾーンバイアスが隣接で変わる境。
    DirtZoneTone,
    /// 同じ Sand どうしで shore/inland variant が隣接で変わる境。
    SandZoneTone,
}

impl BoundaryKind {
    /// 2 つの TerrainType から BoundaryKind を決定する（順序非依存）。
    /// 完全に同一タイルなら None。
    pub fn from_pair(a: TerrainType, b: TerrainType) -> Option<Self> {
        if a == b {
            return None;
        }
        // priority() でソートして無向ペアを一意に決定（River=0 < Sand=1 < Dirt=2 < Grass=3）
        let (lo, hi) = if a.priority() < b.priority() {
            (a, b)
        } else {
            (b, a)
        };
        match (lo, hi) {
            (TerrainType::River, TerrainType::Sand) => Some(Self::SandRiver),
            (TerrainType::River, TerrainType::Dirt) => Some(Self::DirtRiver),
            (TerrainType::River, TerrainType::Grass) => Some(Self::GrassRiver),
            (TerrainType::Sand, TerrainType::Dirt) => Some(Self::DirtSand),
            (TerrainType::Sand, TerrainType::Grass) => Some(Self::GrassSand),
            (TerrainType::Dirt, TerrainType::Grass) => Some(Self::GrassDirt),
            _ => None,
        }
    }

    /// この境界種別のインデックス（per-kind seed 生成用）。
    pub fn index(self) -> u32 {
        self as u32
    }
}

// ── データ型 ──────────────────────────────────────────────────────────────────

/// 隣接する 2 セル間の境界エッジ（ワールド座標）。
#[derive(Debug, Clone)]
pub struct BoundaryEdge {
    pub a: Vec2,
    pub b: Vec2,
    pub kind: BoundaryKind,
}

// ── M1: エッジ抽出と連結 ──────────────────────────────────────────────────────

#[inline]
pub(crate) fn zone_tone_boundary_kind(terrain: TerrainType, bias_a: u8, bias_b: u8) -> Option<BoundaryKind> {
    if bias_a == bias_b {
        return None;
    }
    match terrain {
        TerrainType::Grass => Some(BoundaryKind::GrassZoneTone),
        TerrainType::Dirt => Some(BoundaryKind::DirtZoneTone),
        _ => None,
    }
}

/// 粗い種別が両方草または両方土のときだけゾーン境界（亜種は一致不要。ゾーン境で亜種が違うことが多い）。
#[inline]
pub(crate) fn maybe_zone_tone_edge(
    t0: TerrainType,
    t1: TerrainType,
    bias_a: u8,
    bias_b: u8,
) -> Option<BoundaryKind> {
    let both_grass = matches!((t0, t1), (TerrainType::Grass, TerrainType::Grass));
    let both_dirt = matches!((t0, t1), (TerrainType::Dirt, TerrainType::Dirt));
    if !both_grass && !both_dirt {
        return None;
    }
    zone_tone_boundary_kind(t0, bias_a, bias_b)
}

/// グリッド座標のゾーンバイアスバイトを返す（grass zone=0, neutral=128, dirt zone=255）。
#[inline]
pub(crate) fn terrain_zone_bias_byte(masks: &WorldMasks, pos: (i32, i32)) -> u8 {
    if masks.grass_zone_mask.get(pos) {
        0
    } else if masks.dirt_zone_mask.get(pos) {
        255
    } else {
        128
    }
}

/// Sand タイルの shore/inland variant バイトを返す（shore=171, inland=172, regular=170）。
#[inline]
pub(crate) fn terrain_sand_variant_byte(masks: &WorldMasks, pos: (i32, i32)) -> u8 {
    let is_final = masks.final_sand_mask.get(pos);
    let is_inland = masks.inland_sand_mask.get(pos);
    if is_final && !is_inland {
        171 // shore
    } else if is_inland {
        172 // inland
    } else {
        170 // regular
    }
}
/// terrain_tiles（row-major: y*MAP_WIDTH+x）と `WorldMasks` から全境界エッジを抽出する。
///
/// - **粗いカテゴリ**が変わる境（`BoundaryKind::from_pair`）
/// - **草↔草／土↔土**（亜種は問わない）で `terrain_zone_bias_byte`（草ゾーン／中立／土ゾーン）が隣接で変わる境
pub fn extract_boundary_edges(
    terrain_tiles: &[TerrainType],
    masks: &WorldMasks,
) -> Vec<BoundaryEdge> {
    let w = MAP_WIDTH as usize;
    let h = MAP_HEIGHT as usize;
    let half = TILE_SIZE / 2.0;
    let mut edges = Vec::new();

    // 水平エッジ: セル (x, y) と (x, y+1) の境界
    for y in 0..h - 1 {
        for x in 0..w {
            let t0 = terrain_tiles[y * w + x];
            let t1 = terrain_tiles[(y + 1) * w + x];
            let gx = x as i32;
            let gy = y as i32;
            if let Some(kind) = BoundaryKind::from_pair(t0, t1) {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x - half, center.y + half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind,
                });
            } else if let Some(kind) = maybe_zone_tone_edge(
                t0,
                t1,
                terrain_zone_bias_byte(masks, (gx, gy)),
                terrain_zone_bias_byte(masks, (gx, gy + 1)),
            ) {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x - half, center.y + half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind,
                });
            } else if t0 == TerrainType::Sand
                && t1 == TerrainType::Sand
                && terrain_sand_variant_byte(masks, (gx, gy))
                    != terrain_sand_variant_byte(masks, (gx, gy + 1))
            {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x - half, center.y + half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind: BoundaryKind::SandZoneTone,
                });
            }
        }
    }

    // 垂直エッジ: セル (x, y) と (x+1, y) の境界
    for y in 0..h {
        for x in 0..w - 1 {
            let t0 = terrain_tiles[y * w + x];
            let t1 = terrain_tiles[y * w + x + 1];
            let gx = x as i32;
            let gy = y as i32;
            if let Some(kind) = BoundaryKind::from_pair(t0, t1) {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x + half, center.y - half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind,
                });
            } else if let Some(kind) = maybe_zone_tone_edge(
                t0,
                t1,
                terrain_zone_bias_byte(masks, (gx, gy)),
                terrain_zone_bias_byte(masks, (gx + 1, gy)),
            ) {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x + half, center.y - half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind,
                });
            } else if t0 == TerrainType::Sand
                && t1 == TerrainType::Sand
                && terrain_sand_variant_byte(masks, (gx, gy))
                    != terrain_sand_variant_byte(masks, (gx + 1, gy))
            {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x + half, center.y - half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind: BoundaryKind::SandZoneTone,
                });
            }
        }
    }

    edges
}

#[cfg(test)]
mod tests {
    use super::{maybe_zone_tone_edge, zone_tone_boundary_kind, BoundaryKind};
    use hw_world::TerrainType;

    #[test]
    fn from_pair_grass_dirt_is_grass_dirt() {
        let g = TerrainType::Grass;
        let d = TerrainType::Dirt;
        assert_eq!(BoundaryKind::from_pair(g, d), Some(BoundaryKind::GrassDirt));
        assert_eq!(BoundaryKind::from_pair(d, g), Some(BoundaryKind::GrassDirt));
    }

    #[test]
    fn from_pair_grass_variants_no_edge() {
        // 同一の Grass どうしは境界なし
        let a = TerrainType::Grass;
        let b = TerrainType::Grass;
        assert_eq!(BoundaryKind::from_pair(a, b), None);
    }

    #[test]
    fn from_pair_identical_grass_none() {
        let a = TerrainType::Grass;
        assert_eq!(BoundaryKind::from_pair(a, a), None);
    }

    #[test]
    fn zone_tone_same_bias_none() {
        let g = TerrainType::Grass;
        assert_eq!(zone_tone_boundary_kind(g, 0, 0), None);
    }

    #[test]
    fn zone_tone_grass_zone_vs_neutral() {
        let g = TerrainType::Grass;
        assert_eq!(
            zone_tone_boundary_kind(g, 0, 128),
            Some(BoundaryKind::GrassZoneTone)
        );
    }

    #[test]
    fn zone_tone_dirt_zone_vs_neutral() {
        let d = TerrainType::Dirt;
        assert_eq!(
            zone_tone_boundary_kind(d, 255, 128),
            Some(BoundaryKind::DirtZoneTone)
        );
    }

    #[test]
    fn maybe_zone_tone_grass_different_variants_still_zone_edge() {
        // flat enum では Grass どうし同士のゾーン境界をテスト
        let a = TerrainType::Grass;
        let b = TerrainType::Grass;
        assert_eq!(
            maybe_zone_tone_edge(a, b, 0, 128),
            Some(BoundaryKind::GrassZoneTone)
        );
    }
}
