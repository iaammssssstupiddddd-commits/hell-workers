//! タイル待機キャッシュ
//!
//! Floor/Wall 工事タイルの「不足量合計」を site ごとに集計した HashMap キャッシュ。
//! Bevy の変更検知（`Changed<*TileBlueprint>`）を使用し、
//! タイル状態が変化したフレームでのみ全タイル走査を行う。
//! 変化のないフレームでは auto_haul_system がキャッシュを読むだけになる。

use bevy::prelude::*;
use hw_core::constants::{
    FLOOR_BONES_PER_TILE, FLOOR_MUD_PER_TILE, WALL_MUD_PER_TILE, WALL_WOOD_PER_TILE,
};
use hw_jobs::construction::{FloorTileBlueprint, WallTileBlueprint};
use hw_jobs::{FloorTileState, WallTileState};
use std::collections::HashMap;

/// Floor 工事: site エンティティ → (bones_needed 合計, mud_needed 合計)
#[derive(Resource, Default)]
pub struct FloorTileWaitingCache {
    pub map: HashMap<Entity, (u32, u32)>,
}

/// Wall 工事: site エンティティ → (wood_needed 合計, mud_needed 合計)
#[derive(Resource, Default)]
pub struct WallTileWaitingCache {
    pub map: HashMap<Entity, (u32, u32)>,
}

/// FloorTileBlueprint が変化したフレームでのみキャッシュを再構築する。
pub fn update_floor_tile_waiting_cache_system(
    q_changed: Query<(), Changed<FloorTileBlueprint>>,
    q_all_tiles: Query<&FloorTileBlueprint>,
    mut cache: ResMut<FloorTileWaitingCache>,
) {
    if q_changed.is_empty() {
        return;
    }
    cache.map.clear();
    for tile in q_all_tiles.iter() {
        match tile.state {
            FloorTileState::WaitingBones => {
                let needed = FLOOR_BONES_PER_TILE.saturating_sub(tile.bones_delivered);
                if needed > 0 {
                    cache.map.entry(tile.parent_site).or_insert((0, 0)).0 += needed;
                }
            }
            FloorTileState::WaitingMud => {
                let needed = FLOOR_MUD_PER_TILE.saturating_sub(tile.mud_delivered);
                if needed > 0 {
                    cache.map.entry(tile.parent_site).or_insert((0, 0)).1 += needed;
                }
            }
            _ => {}
        }
    }
}

/// WallTileBlueprint が変化したフレームでのみキャッシュを再構築する。
pub fn update_wall_tile_waiting_cache_system(
    q_changed: Query<(), Changed<WallTileBlueprint>>,
    q_all_tiles: Query<&WallTileBlueprint>,
    mut cache: ResMut<WallTileWaitingCache>,
) {
    if q_changed.is_empty() {
        return;
    }
    cache.map.clear();
    for tile in q_all_tiles.iter() {
        match tile.state {
            WallTileState::WaitingWood => {
                let needed = WALL_WOOD_PER_TILE.saturating_sub(tile.wood_delivered);
                if needed > 0 {
                    cache.map.entry(tile.parent_site).or_insert((0, 0)).0 += needed;
                }
            }
            WallTileState::WaitingMud => {
                let needed = WALL_MUD_PER_TILE.saturating_sub(tile.mud_delivered);
                if needed > 0 {
                    cache.map.entry(tile.parent_site).or_insert((0, 0)).1 += needed;
                }
            }
            _ => {}
        }
    }
}
