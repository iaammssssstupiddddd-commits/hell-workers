//! 障害物除去後の terrain id map 更新システム。
//!
//! `obstacle_cleanup_system`（hw_world）が発行する `TerrainChangedEvent` を受信し、
//! 対応するセルの `terrain_id_map` ピクセルを更新する。

use crate::world::map::{TerrainIdMap, terrain_type_to_id_byte};
use bevy::prelude::*;
use hw_core::constants::MAP_WIDTH;
use hw_world::{TerrainChangedEvent, WorldMapRead};

pub fn terrain_id_map_sync_system(
    world_map: WorldMapRead,
    terrain_id_map: Res<TerrainIdMap>,
    mut images: ResMut<Assets<Image>>,
    mut events: MessageReader<TerrainChangedEvent>,
) {
    for ev in events.read() {
        let Some(terrain) = world_map.terrain_at_idx(ev.idx) else {
            continue;
        };
        let Some(image) = images.get_mut(&terrain_id_map.image) else {
            continue;
        };
        let Some(data) = image.data.as_mut() else {
            continue;
        };

        let x = ev.idx % MAP_WIDTH as usize;
        let y = ev.idx / MAP_WIDTH as usize;
        let pixel_idx = y * MAP_WIDTH as usize + x;
        data[pixel_idx] = terrain_type_to_id_byte(terrain);
    }
}
