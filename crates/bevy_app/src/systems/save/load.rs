//! ワールドのロード（exclusive system）。
//!
//! 1. `saves/world.scn.ron` を読み込み `DynamicWorld` へデシリアライズする。
//! 2. 既存のセーブ対象エンティティ（`collect_persisted_entities`）を全て despawn する。
//! 3. `DynamicWorld::write_to_world_with` で新しいエンティティ/リソースを書き込む。
//! 4. キャッシュ系リソース（空間グリッド・予約キャッシュ等）をデフォルトへリセットする
//!    （次フレーム以降、既存システムが自然に再構築する）。
//! 5. セーブ対象外だった `AssignedTask` を、`DamnedSoul` を持つ全エンティティに
//!    `None` で明示的に再挿入する（タスク割当クエリが `AssignedTask` の存在を
//!    前提にしているため）。
//!
//! # 設計上の逸脱（plan からの変更点）
//! plan は Relationship の `RelationshipHookMode::Skip` を踏まえた明示的な
//! reconcile パス（`Commanding` 等の RelationshipTarget を `CommandedBy` 等から
//! 再構築する）を想定していたが、本実装では RelationshipTarget 型自体も
//! allow-list に含めて直接シリアライズ/デシリアライズしているため、
//! 追加の reconcile は不要と判断した（保存時点で Source/Target 両方が整合した
//! スナップショットとして保存されるため）。

use bevy::ecs::entity::EntityHashMap;
use bevy::ecs::reflect::AppTypeRegistry;
use bevy::prelude::*;

use hw_core::soul::DamnedSoul;
use hw_jobs::AssignedTask;

use hw_logistics::resource_cache::SharedResourceCache;
use hw_logistics::tile_index::TileSiteIndex;
use hw_logistics::transport_request::TransportRequestMetrics;
use hw_logistics::transport_request::producer::active_unit_cache::{
    CachedActiveFamiliars, CachedActiveYards,
};
use hw_logistics::transport_request::producer::tile_wait_cache::{
    FloorTileWaitingCache, WallTileWaitingCache,
};
use hw_spatial::blueprint::BlueprintSpatialGrid;
use hw_spatial::designation::DesignationSpatialGrid;
use hw_spatial::familiar::FamiliarSpatialGrid;
use hw_spatial::floor_construction::FloorConstructionSpatialGrid;
use hw_spatial::gathering::GatheringSpotSpatialGrid;
use hw_spatial::resource::ResourceSpatialGrid;
use hw_spatial::soul::SpatialGrid;
use hw_spatial::stockpile::StockpileSpatialGrid;
use hw_spatial::transport_request::TransportRequestSpatialGrid;
use hw_world::room_detection::{RoomDetectionState, RoomTileLookup, RoomValidationState};

use crate::world::map::GeneratedWorldLayoutResource;
use crate::world::regrowth::{RegrowthManager, configure_regrowth_from_generated_layout};
use hw_familiar_ai::familiar_ai::decide::resources::ReachabilityFrameCache;

use bevy_world_serialization::DynamicWorld;
use bevy_world_serialization::serde::WorldDeserializer;

use super::entities::collect_persisted_entities;
use super::rehydrate::rehydrate_after_load;
use super::state::{SAVE_FILE_PATH, SavedWorldgenSeed};

pub fn load_world_system(world: &mut World) {
    // Reset the trigger immediately so a failure below doesn't leave the
    // state stuck (which would block all future save/load requests).
    *world.resource_mut::<super::state::SaveLoadState>() = super::state::SaveLoadState::Idle;

    let contents = match std::fs::read_to_string(SAVE_FILE_PATH) {
        Ok(s) => s,
        Err(err) => {
            error!("Failed to read save file {SAVE_FILE_PATH}: {err}");
            return;
        }
    };

    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();

    let mut ron_deserializer = match ron::de::Deserializer::from_str(&contents) {
        Ok(d) => d,
        Err(err) => {
            error!("Failed to parse save file {SAVE_FILE_PATH}: {err}");
            return;
        }
    };

    let mut asset_server = world.resource::<AssetServer>().clone();
    let dynamic_world = {
        use serde::de::DeserializeSeed;
        let deserializer = WorldDeserializer {
            type_registry: &registry,
            load_from_path: &mut asset_server,
        };
        match deserializer.deserialize(&mut ron_deserializer) {
            Ok(w) => w,
            Err(err) => {
                error!("Failed to deserialize save file {SAVE_FILE_PATH}: {err}");
                return;
            }
        }
    };

    // 地形チャンク等のビジュアルは起動時 seed から生成されセーブに含まれないため、
    // seed が一致しないセッションへのロードは論理と表示が食い違う。ここで中止する。
    match extract_saved_worldgen_seed(&dynamic_world) {
        Some(saved_seed) => {
            let current_seed = world.resource::<GeneratedWorldLayoutResource>().master_seed;
            if saved_seed != current_seed {
                error!(
                    "Load aborted: worldgen seed mismatch (save={saved_seed}, session={current_seed}). \
                     Restart with HELL_WORKERS_WORLDGEN_SEED={saved_seed} and load again."
                );
                return;
            }
        }
        None => {
            warn!(
                "Save file has no worldgen seed (old format); terrain visuals may not match the loaded WorldMap"
            );
        }
    }

    // 既存のシミュレーションエンティティを全て despawn してから書き込む。
    let old_entities = collect_persisted_entities(world);
    for entity in old_entities {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }

    let mut entity_map: EntityHashMap<Entity> = EntityHashMap::default();
    if let Err(err) = dynamic_world.write_to_world_with(world, &mut entity_map, &registry) {
        error!("Failed to write loaded world: {err}");
        return;
    }
    drop(registry);

    rebuild_transient_caches(world);
    restore_default_assigned_task(world);
    rehydrate_after_load(world);

    info!("World loaded from {SAVE_FILE_PATH}");
}

/// デシリアライズ済み `DynamicWorld` から `SavedWorldgenSeed` を取り出す。
/// リソースは reflect 表現（`DynamicTupleStruct` 等）のため、
/// 具象ダウンキャストと reflect フィールド読みの両方を試す。
fn extract_saved_worldgen_seed(dynamic_world: &DynamicWorld) -> Option<u64> {
    use bevy::reflect::{FromReflect, ReflectRef, TypePath};

    dynamic_world.resources.iter().find_map(|resource| {
        let info = resource.get_represented_type_info()?;
        if info.type_path() != SavedWorldgenSeed::type_path() {
            return None;
        }
        if let Some(seed) = resource.try_downcast_ref::<SavedWorldgenSeed>() {
            return Some(seed.0);
        }
        if let ReflectRef::TupleStruct(tuple_struct) = resource.reflect_ref() {
            let field = tuple_struct.field(0)?;
            return field
                .try_downcast_ref::<u64>()
                .copied()
                .or_else(|| u64::from_reflect(field));
        }
        None
    })
}

/// セーブ対象外のキャッシュ/空間グリッド系リソースをデフォルトへリセットする。
/// 既存の各システムが次フレーム以降に自然に再構築する前提。
fn rebuild_transient_caches(world: &mut World) {
    world.insert_resource(SharedResourceCache::default());
    world.insert_resource(TileSiteIndex::default());
    world.insert_resource(TransportRequestMetrics::default());
    world.insert_resource(CachedActiveFamiliars::default());
    world.insert_resource(CachedActiveYards::default());
    world.insert_resource(FloorTileWaitingCache::default());
    world.insert_resource(WallTileWaitingCache::default());
    world.insert_resource(RoomDetectionState::default());
    world.insert_resource(RoomTileLookup::default());
    world.insert_resource(RoomValidationState::default());
    world.insert_resource(GatheringSpotSpatialGrid::default());
    world.insert_resource(BlueprintSpatialGrid::default());
    world.insert_resource(DesignationSpatialGrid::default());
    world.insert_resource(FamiliarSpatialGrid::default());
    world.insert_resource(FloorConstructionSpatialGrid::default());
    world.insert_resource(ResourceSpatialGrid::default());
    world.insert_resource(SpatialGrid::default());
    world.insert_resource(StockpileSpatialGrid::default());
    world.insert_resource(TransportRequestSpatialGrid::default());
    world.insert_resource(ReachabilityFrameCache::default());

    let mut regrowth = RegrowthManager::default();
    if let Some(generated_layout) = world.get_resource::<GeneratedWorldLayoutResource>() {
        configure_regrowth_from_generated_layout(&mut regrowth, &generated_layout.layout);
    }
    world.insert_resource(regrowth);
}

/// セーブ対象外の `AssignedTask` を、`DamnedSoul` を持つ全エンティティへ
/// `None` で再挿入する（既に持っている場合は上書きしない）。
fn restore_default_assigned_task(world: &mut World) {
    let mut query = world.query_filtered::<Entity, (With<DamnedSoul>, Without<AssignedTask>)>();
    let souls_without_task: Vec<Entity> = query.iter(world).collect();
    for entity in souls_without_task {
        world.entity_mut(entity).insert(AssignedTask::default());
    }
}
