use bevy::prelude::*;

use crate::world::map::WorldMap;

pub use hw_core::area::AreaBounds;

#[derive(Component, Clone, Debug)]
pub struct Site {
    pub min: Vec2,
    pub max: Vec2,
}

impl Site {
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    pub fn bounds(&self) -> AreaBounds {
        AreaBounds { min: self.min, max: self.max }
    }
}

impl From<&Site> for AreaBounds {
    fn from(site: &Site) -> Self {
        AreaBounds { min: site.min, max: site.max }
    }
}

#[derive(Component, Clone, Debug)]
pub struct Yard {
    pub min: Vec2,
    pub max: Vec2,
}

impl Yard {
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    pub fn bounds(&self) -> AreaBounds {
        AreaBounds { min: self.min, max: self.max }
    }

    pub fn width_tiles(&self) -> usize {
        let min_grid = WorldMap::world_to_grid(self.min).0;
        let max_grid = WorldMap::world_to_grid(self.max).0;
        max_grid.saturating_sub(min_grid) as usize + 1
    }

    pub fn height_tiles(&self) -> usize {
        let min_grid = WorldMap::world_to_grid(self.min).1;
        let max_grid = WorldMap::world_to_grid(self.max).1;
        max_grid.saturating_sub(min_grid) as usize + 1
    }

    pub fn has_minimum_size(&self, min_width: f32, min_height: f32) -> bool {
        self.width_tiles() as f32 >= min_width && self.height_tiles() as f32 >= min_height
    }
}

impl From<&Yard> for AreaBounds {
    fn from(yard: &Yard) -> Self {
        AreaBounds { min: yard.min, max: yard.max }
    }
}


#[derive(Component, Clone, Debug)]
pub struct PairedYard(pub Entity);

#[derive(Component, Clone, Debug)]
pub struct PairedSite(pub Entity);
