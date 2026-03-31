//! 障害物除去後のテレインマテリアル差し替えシステム。
//!
//! `obstacle_cleanup_system`（hw_world）が発行する `TerrainChangedEvent` を受信し、
//! 対応するタイルエンティティの `MeshMaterial3d<SectionMaterial>` を差し替える。

use crate::plugins::startup::Terrain3dHandles;
use crate::world::map::Tile;
use bevy::prelude::*;
use hw_visual::SectionMaterial;
use hw_world::{TerrainChangedEvent, TerrainType, WorldMapRead};

pub fn terrain_material_sync_system(
    world_map: WorldMapRead,
    terrain_handles: Res<Terrain3dHandles>,
    mut events: MessageReader<TerrainChangedEvent>,
    mut q_tiles: Query<&mut MeshMaterial3d<SectionMaterial>, With<Tile>>,
) {
    for ev in events.read() {
        let Some(tile_entity) = world_map.tile_entity_at_idx(ev.idx) else {
            continue;
        };
        let Some(terrain) = world_map.terrain_at_idx(ev.idx) else {
            continue;
        };
        let Ok(mut mat_handle) = q_tiles.get_mut(tile_entity) else {
            continue;
        };
        *mat_handle = MeshMaterial3d(terrain_to_material(terrain, &terrain_handles));
    }
}

fn terrain_to_material(
    terrain: TerrainType,
    handles: &Terrain3dHandles,
) -> Handle<SectionMaterial> {
    match terrain {
        TerrainType::Grass => handles.grass.clone(),
        TerrainType::Dirt => handles.dirt.clone(),
        TerrainType::Sand => handles.sand.clone(),
        TerrainType::River => handles.river.clone(),
    }
}
