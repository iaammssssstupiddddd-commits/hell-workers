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

    pub fn is_changed(&self) -> bool {
        self.world_map.is_changed()
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

/// 変更可能な WorldMap resource access を system 境界で統一する。
#[derive(SystemParam)]
pub struct WorldMapWrite<'w> {
    world_map: ResMut<'w, WorldMap>,
}

impl<'w> WorldMapWrite<'w> {
    pub fn as_ref(&self) -> &WorldMap {
        self.world_map.as_ref()
    }

    pub fn as_mut(&mut self) -> &mut WorldMap {
        self.world_map.as_mut()
    }

    pub fn is_changed(&self) -> bool {
        self.world_map.is_changed()
    }
}

impl AsRef<WorldMap> for WorldMapWrite<'_> {
    fn as_ref(&self) -> &WorldMap {
        self.world_map.as_ref()
    }
}

impl AsMut<WorldMap> for WorldMapWrite<'_> {
    fn as_mut(&mut self) -> &mut WorldMap {
        self.world_map.as_mut()
    }
}

impl std::ops::Deref for WorldMapWrite<'_> {
    type Target = WorldMap;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl std::ops::DerefMut for WorldMapWrite<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}
