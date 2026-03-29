use bevy::prelude::*;

use crate::constants::TILE_SIZE;
use crate::game_state::TaskMode;

/// 矩形領域の共通データ型。
/// Site / Yard / TaskArea が共有する「min-max 矩形」を型消去して扱うために使う。
/// Component ではなく plain struct。
#[derive(Clone, Debug, PartialEq)]
pub struct AreaBounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl AreaBounds {
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    pub fn from_points(a: Vec2, b: Vec2) -> Self {
        Self {
            min: Vec2::new(a.x.min(b.x), a.y.min(b.y)),
            max: Vec2::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) / 2.0
    }

    pub fn size(&self) -> Vec2 {
        (self.max - self.min).abs()
    }

    pub fn contains(&self, pos: Vec2) -> bool {
        pos.x >= self.min.x && pos.x <= self.max.x && pos.y >= self.min.y && pos.y <= self.max.y
    }

    pub fn contains_with_margin(&self, pos: Vec2, margin: f32) -> bool {
        let m = margin.abs();
        pos.x >= self.min.x - m
            && pos.x <= self.max.x + m
            && pos.y >= self.min.y - m
            && pos.y <= self.max.y + m
    }
}

/// タスクエリア - 使い魔が担当するエリア
#[derive(Component, Clone, Debug, PartialEq)]
pub struct TaskArea {
    pub bounds: AreaBounds,
}

impl TaskArea {
    pub fn from_points(a: Vec2, b: Vec2) -> Self {
        Self {
            bounds: AreaBounds::from_points(a, b),
        }
    }

    pub fn center(&self) -> Vec2 {
        self.bounds.center()
    }

    pub fn size(&self) -> Vec2 {
        self.bounds.size()
    }

    pub fn contains(&self, pos: Vec2) -> bool {
        self.bounds.contains(pos)
    }

    pub fn contains_with_margin(&self, pos: Vec2, margin: f32) -> bool {
        self.bounds.contains_with_margin(pos, margin)
    }

    pub fn contains_border(&self, pos: Vec2, thickness: f32) -> bool {
        let in_outer = self.bounds.contains_with_margin(pos, thickness);
        let inner = AreaBounds::new(
            self.bounds.min + Vec2::splat(thickness),
            self.bounds.max - Vec2::splat(thickness),
        );
        let in_inner = inner.contains(pos);
        in_outer && !in_inner
    }

    pub fn bounds(&self) -> AreaBounds {
        self.bounds.clone()
    }

    pub fn min(&self) -> Vec2 {
        self.bounds.min
    }

    pub fn max(&self) -> Vec2 {
        self.bounds.max
    }
}

impl From<&TaskArea> for AreaBounds {
    fn from(area: &TaskArea) -> Self {
        area.bounds.clone()
    }
}

impl From<AreaBounds> for TaskArea {
    fn from(bounds: AreaBounds) -> Self {
        TaskArea { bounds }
    }
}

// ---------------------------------------------------------------------------
// Pure helper functions (moved from src/systems/command/area_selection/geometry.rs)
// ---------------------------------------------------------------------------

/// `TaskMode` のドラッグ開始座標を取り出す。
pub fn get_drag_start(mode: TaskMode) -> Option<Vec2> {
    match mode {
        TaskMode::AreaSelection(s) => s,
        TaskMode::DesignateChop(s) => s,
        TaskMode::DesignateMine(s) => s,
        TaskMode::DesignateHaul(s) => s,
        TaskMode::CancelDesignation(s) => s,
        TaskMode::ZonePlacement(_, s) => s,
        TaskMode::ZoneRemoval(_, s) => s,
        TaskMode::FloorPlace(s) => s,
        TaskMode::WallPlace(s) => s,
        TaskMode::DreamPlanting(s) => s,
        TaskMode::SoulSpaPlace(s) => s,
        _ => None,
    }
}

/// ドラッグ方向から壁ライン用 `TaskArea` を生成する。
pub fn wall_line_area(start_pos: Vec2, end_pos: Vec2) -> TaskArea {
    let delta = end_pos - start_pos;
    if delta.length_squared() <= f32::EPSILON {
        return TaskArea::from_points(start_pos, start_pos + Vec2::splat(TILE_SIZE));
    }

    if delta.x.abs() >= delta.y.abs() {
        let y_dir = if delta.y < 0.0 { -1.0 } else { 1.0 };
        TaskArea::from_points(
            start_pos,
            Vec2::new(end_pos.x, start_pos.y + TILE_SIZE * y_dir),
        )
    } else {
        let x_dir = if delta.x < 0.0 { -1.0 } else { 1.0 };
        TaskArea::from_points(
            start_pos,
            Vec2::new(start_pos.x + TILE_SIZE * x_dir, end_pos.y),
        )
    }
}

/// command/area_selection 内部で使う中心座標ベースの `TaskArea` 生成 helper。
#[doc(hidden)]
pub fn area_from_center_and_size(center: Vec2, size: Vec2) -> TaskArea {
    let half = size.abs() * 0.5;
    TaskArea::from_points(center - half, center + half)
}

/// エリア内に含まれる座標の数を数える。
pub fn count_positions_in_area(area: &TaskArea, positions: impl Iterator<Item = Vec2>) -> usize {
    const AREA_CONTAINS_MARGIN: f32 = 0.1;
    positions
        .filter(|&pos| area.contains_with_margin(pos, AREA_CONTAINS_MARGIN))
        .count()
}

/// 選択エリアと他エリアの重複サマリーを返す。`(重複数, 最大重複率)` のタプル。
pub fn overlap_summary_from_areas(
    selected_entity: Entity,
    selected_area: &TaskArea,
    areas: impl Iterator<Item = (Entity, TaskArea)>,
) -> Option<(usize, f32)> {
    let selected_size = selected_area.size();
    let selected_area_value = selected_size.x.abs() * selected_size.y.abs();
    if selected_area_value <= f32::EPSILON {
        return None;
    }

    let mut overlap_count = 0usize;
    let mut max_ratio = 0.0f32;

    for (entity, area) in areas {
        if entity == selected_entity {
            continue;
        }

        let overlap_w = (selected_area.max().x.min(area.max().x)
            - selected_area.min().x.max(area.min().x))
        .max(0.0);
        let overlap_h = (selected_area.max().y.min(area.max().y)
            - selected_area.min().y.max(area.min().y))
        .max(0.0);
        let overlap_area = overlap_w * overlap_h;
        if overlap_area <= f32::EPSILON {
            continue;
        }

        overlap_count += 1;
        let ratio = (overlap_area / selected_area_value).clamp(0.0, 1.0);
        if ratio > max_ratio {
            max_ratio = ratio;
        }
    }

    Some((overlap_count, max_ratio))
}
