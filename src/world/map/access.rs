use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::world::map::WorldMap;

/// 読み取り専用の WorldMap resource access を system 境界で統一する。
#[derive(SystemParam)]
pub struct WorldMapRead<'w> {
    world_map: Res<'w, WorldMap>,
}

impl<'w> WorldMapRead<'w> {
    pub fn as_ref(&self) -> &WorldMap {
        self.world_map.as_ref()
    }
}

impl AsRef<WorldMap> for WorldMapRead<'_> {
    fn as_ref(&self) -> &WorldMap {
        self.world_map.as_ref()
    }
}

impl std::ops::Deref for WorldMapRead<'_> {
    type Target = WorldMap;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
