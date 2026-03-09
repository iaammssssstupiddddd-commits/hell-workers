use bevy::prelude::*;

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
