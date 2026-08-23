//! 障害物除去後の terrain metadata texture 更新システム。
//!
//! `obstacle_sync_system`（hw_world）が発行する `TerrainChangedEvent` を受信し、
//! 対応するセルの terrain id と rock field feature を同期する。

use crate::world::map::{TerrainFeatureMap, TerrainIdMap, terrain_type_to_id_byte};
use bevy::prelude::*;
use hw_core::constants::MAP_WIDTH;
use hw_world::{TerrainChangedEvent, WorldMapRead};

pub fn terrain_metadata_sync_system(
    world_map: WorldMapRead,
    terrain_id_map: Res<TerrainIdMap>,
    terrain_feature_map: Res<TerrainFeatureMap>,
    mut images: ResMut<Assets<Image>>,
    mut events: MessageReader<TerrainChangedEvent>,
) {
    for ev in events.read() {
        let Some(terrain) = world_map.terrain_at_idx(ev.idx) else {
            continue;
        };
        let x = ev.idx % MAP_WIDTH as usize;
        let y = ev.idx / MAP_WIDTH as usize;
        let pixel_idx = y * MAP_WIDTH as usize + x;
        if let Some(mut image) = images.get_mut(&terrain_id_map.image)
            && let Some(data) = image.data.as_mut()
            && let Some(terrain_id) = data.get_mut(pixel_idx)
        {
            *terrain_id = terrain_type_to_id_byte(terrain);
        }
        if let Some(mut image) = images.get_mut(&terrain_feature_map.image)
            && let Some(data) = image.data.as_mut()
        {
            clear_rock_field_feature(data, pixel_idx);
        }
    }
}

fn clear_rock_field_feature(data: &mut [u8], pixel_idx: usize) {
    const CHANNELS_PER_PIXEL: usize = 4;
    const ROCK_FIELD_CHANNEL: usize = 2;
    let channel_idx = pixel_idx * CHANNELS_PER_PIXEL + ROCK_FIELD_CHANNEL;
    if let Some(rock_field) = data.get_mut(channel_idx) {
        *rock_field = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    use hw_core::constants::MAP_HEIGHT;
    use hw_world::{TerrainType, WorldMap};

    #[test]
    fn clearing_rock_field_preserves_other_feature_channels() {
        let mut data = vec![10, 20, 255, 40, 50, 60, 255, 80];

        clear_rock_field_feature(&mut data, 1);

        assert_eq!(data, vec![10, 20, 255, 40, 50, 60, 0, 80]);
    }

    #[test]
    fn terrain_change_syncs_dirt_id_and_clears_rock_field_feature() {
        let size = Extent3d {
            width: MAP_WIDTH as u32,
            height: MAP_HEIGHT as u32,
            depth_or_array_layers: 1,
        };
        let mut images = Assets::<Image>::default();
        let terrain_id = images.add(Image::new_fill(
            size,
            TextureDimension::D2,
            &[0],
            TextureFormat::R8Unorm,
            default(),
        ));
        let terrain_feature = images.add(Image::new_fill(
            size,
            TextureDimension::D2,
            &[10, 20, 255, 40],
            TextureFormat::Rgba8Unorm,
            default(),
        ));
        let mut map = WorldMap::default();
        let idx = map.pos_to_idx(3, 4).unwrap();
        map.set_terrain_at_idx(idx, TerrainType::Dirt);

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<TerrainChangedEvent>()
            .insert_resource(map)
            .insert_resource(images)
            .insert_resource(TerrainIdMap {
                image: terrain_id.clone(),
            })
            .insert_resource(TerrainFeatureMap {
                image: terrain_feature.clone(),
            })
            .add_systems(Update, terrain_metadata_sync_system);
        app.world_mut().write_message(TerrainChangedEvent { idx });

        app.update();

        let images = app.world().resource::<Assets<Image>>();
        let id_data = images.get(&terrain_id).unwrap().data.as_ref().unwrap();
        assert_eq!(id_data[idx], terrain_type_to_id_byte(TerrainType::Dirt));
        let feature_data = images.get(&terrain_feature).unwrap().data.as_ref().unwrap();
        let feature_start = idx * 4;
        assert_eq!(
            &feature_data[feature_start..feature_start + 4],
            &[10, 20, 0, 40]
        );
    }
}
