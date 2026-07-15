//! ワールドのロード（exclusive system）。
//!
//! 1. 外部header、worldgen seed、DynamicWorld schemaを検証して`PreparedLoad`を作る。
//! 2. staging Worldへ適用して、Reflect registryの静的contractをpreflightする。
//! 3. rehydrate前提を検証後、rollback snapshotを取り、旧persisted entityを置換する。
//! 4. 成功時とrollback復旧時の両方でcache reset、`AssignedTask`復元、rehydrateを実行する。
//!
//! # 設計上の逸脱（plan からの変更点）
//! plan は Relationship の `RelationshipHookMode::Skip` を踏まえた明示的な
//! reconcile パス（`Commanding` 等の RelationshipTarget を `CommandedBy` 等から
//! 再構築する）を想定していたが、本実装では RelationshipTarget 型自体も
//! allow-list に含めて直接シリアライズ/デシリアライズしているため、
//! 追加の reconcile は不要と判断した（保存時点で Source/Target 両方が整合した
//! スナップショットとして保存されるため）。

use std::fmt;
use std::path::Path;

use bevy::ecs::reflect::AppTypeRegistry;
use bevy::prelude::*;

use hw_core::soul::DamnedSoul;
use hw_jobs::AssignedTask;

use bevy_world_serialization::DynamicWorld;
use bevy_world_serialization::serde::WorldDeserializer;

use crate::world::map::GeneratedWorldLayoutResource;

use super::format::{SaveFormat, SaveFormatError, decode_save_file};
use super::rehydrate::{rehydrate_after_load, validate_rehydrate_prerequisites};
use super::reset::reset_runtime_caches;
use super::schema::{
    DynamicWorldSchemaError, discard_runtime_derived_components, validate_persisted_world,
};
use super::state::{SavePath, SavedWorldgenSeed};
use super::transaction::{preflight_dynamic_world, replace_persisted_world};

struct PreparedLoad {
    format: SaveFormat,
    dynamic_world: DynamicWorld,
}

#[derive(Debug)]
enum LoadPreparationError {
    Format(SaveFormatError),
    MissingPrerequisite(&'static str),
    BodySyntax(String),
    Deserialize(String),
    SeedMismatch { saved: u64, current: u64 },
    Schema(DynamicWorldSchemaError),
}

impl fmt::Display for LoadPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(error) => write!(formatter, "invalid save format: {error}"),
            Self::MissingPrerequisite(resource) => {
                write!(
                    formatter,
                    "load prerequisite resource is unavailable: {resource}"
                )
            }
            Self::BodySyntax(error) => write!(formatter, "invalid DynamicWorld RON body: {error}"),
            Self::Deserialize(error) => {
                write!(formatter, "DynamicWorld deserialization failed: {error}")
            }
            Self::SeedMismatch { saved, current } => write!(
                formatter,
                "worldgen seed mismatch (save={saved}, session={current}); restart with HELL_WORKERS_WORLDGEN_SEED={saved} before loading"
            ),
            Self::Schema(error) => write!(formatter, "invalid save schema: {error}"),
        }
    }
}

impl From<SaveFormatError> for LoadPreparationError {
    fn from(error: SaveFormatError) -> Self {
        Self::Format(error)
    }
}

pub fn load_world_system(world: &mut World) {
    // Reset the trigger immediately so a failure below doesn't leave the
    // state stuck (which would block all future save/load requests).
    *world.resource_mut::<super::state::SaveLoadState>() = super::state::SaveLoadState::Idle;

    let save_path = world.resource::<SavePath>().as_path().to_path_buf();
    let contents = match read_save_file(&save_path) {
        Ok(s) => s,
        Err(err) => {
            error!("Failed to read save file {}: {err}", save_path.display());
            return;
        }
    };

    let prepared = match prepare_load_from_str(world, &contents) {
        Ok(prepared) => prepared,
        Err(err) => {
            error!("Load aborted for {}: {err}", save_path.display());
            return;
        }
    };

    let Some(type_registry) = world.get_resource::<AppTypeRegistry>().cloned() else {
        error!(
            "Load aborted for {}: AppTypeRegistry is unavailable after preparation",
            save_path.display()
        );
        return;
    };
    let registry = type_registry.read();
    if let Err(error) = preflight_dynamic_world(&prepared.dynamic_world, &registry) {
        error!(
            "Load aborted for {}: preflight failed: {error}",
            save_path.display()
        );
        return;
    }
    drop(registry);

    if let Err(error) = validate_rehydrate_prerequisites(world) {
        error!(
            "Load aborted for {}: rehydrate prerequisites failed: {error}",
            save_path.display()
        );
        return;
    }

    let registry = type_registry.read();
    let commit_result = replace_persisted_world(
        world,
        &prepared.dynamic_world,
        &registry,
        finalize_loaded_world,
    );
    drop(registry);

    match commit_result {
        Ok(()) => {
            let format = match prepared.format {
                SaveFormat::LegacyV0 => "legacy v0",
                SaveFormat::V1(_) => "v1",
            };
            info!("World loaded from {} ({format})", save_path.display());
        }
        Err(error @ super::transaction::CommitError::Recovered { .. }) => {
            warn!(
                "Load failed after live apply for {}; rollback recovery completed: {error}",
                save_path.display()
            );
        }
        Err(error) => {
            error!(
                "Load failed after live apply for {}; rollback recovery failed: {error}",
                save_path.display()
            );
        }
    }
}

fn read_save_file(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

/// Validates the external format and seed guard before DynamicWorld parsing.
fn prepare_load_from_str(
    world: &World,
    contents: &str,
) -> Result<PreparedLoad, LoadPreparationError> {
    let decoded = decode_save_file(contents)?;
    let format = decoded.format;
    if let SaveFormat::V1(header) = format {
        validate_worldgen_seed(world, header.worldgen_seed)?;
    }

    let type_registry = world.get_resource::<AppTypeRegistry>().cloned().ok_or(
        LoadPreparationError::MissingPrerequisite(std::any::type_name::<AppTypeRegistry>()),
    )?;
    let registry = type_registry.read();
    let mut ron_deserializer = ron::de::Deserializer::from_str(decoded.body)
        .map_err(|error| LoadPreparationError::BodySyntax(error.to_string()))?;
    let mut asset_server = world.get_resource::<AssetServer>().cloned().ok_or(
        LoadPreparationError::MissingPrerequisite(std::any::type_name::<AssetServer>()),
    )?;
    let mut dynamic_world = {
        use serde::de::DeserializeSeed;
        let deserializer = WorldDeserializer {
            type_registry: &registry,
            load_from_path: &mut asset_server,
        };
        deserializer
            .deserialize(&mut ron_deserializer)
            .map_err(|error| LoadPreparationError::Deserialize(error.to_string()))?
    };

    if format == SaveFormat::LegacyV0 {
        match extract_saved_worldgen_seed(&dynamic_world) {
            Some(saved_seed) => validate_worldgen_seed(world, saved_seed)?,
            None => warn!(
                "Save file has no worldgen seed (legacy v0); terrain visuals may not match the loaded WorldMap"
            ),
        }
        remove_legacy_saved_worldgen_seed(&mut dynamic_world);
    }

    discard_runtime_derived_components(&mut dynamic_world);
    validate_persisted_world(&dynamic_world).map_err(LoadPreparationError::Schema)?;

    Ok(PreparedLoad {
        format,
        dynamic_world,
    })
}

fn validate_worldgen_seed(world: &World, saved_seed: u64) -> Result<(), LoadPreparationError> {
    let current_seed = world
        .get_resource::<GeneratedWorldLayoutResource>()
        .ok_or(LoadPreparationError::MissingPrerequisite(
            std::any::type_name::<GeneratedWorldLayoutResource>(),
        ))?
        .master_seed;
    if saved_seed == current_seed {
        Ok(())
    } else {
        Err(LoadPreparationError::SeedMismatch {
            saved: saved_seed,
            current: current_seed,
        })
    }
}

/// `SavedWorldgenSeed` is only an input to legacy v0 validation. Never apply
/// it to the live v1-era world after its value has been checked.
fn remove_legacy_saved_worldgen_seed(dynamic_world: &mut DynamicWorld) {
    use std::any::TypeId;

    dynamic_world.resources.retain(|resource| {
        resource
            .get_represented_type_info()
            .is_none_or(|info| info.type_id() != TypeId::of::<SavedWorldgenSeed>())
    });
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

/// セーブ対象外の `AssignedTask` を、`DamnedSoul` を持つ全エンティティへ
/// `None` で再挿入する（既に持っている場合は上書きしない）。
fn restore_default_assigned_task(world: &mut World) {
    let mut query = world.query_filtered::<Entity, (With<DamnedSoul>, Without<AssignedTask>)>();
    let souls_without_task: Vec<Entity> = query.iter(world).collect();
    for entity in souls_without_task {
        world.entity_mut(entity).insert(AssignedTask::default());
    }
}

fn finalize_loaded_world(world: &mut World) -> Result<(), String> {
    reset_runtime_caches(world);
    restore_default_assigned_task(world);
    rehydrate_after_load(world).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::map::GeneratedWorldLayoutResource;
    use hw_world::GeneratedWorldLayout;

    use super::super::format::{SaveHeader, encode_save_file};

    #[test]
    fn v1_seed_mismatch_is_rejected_before_dynamic_world_deserialization() {
        let mut world = World::new();
        world.insert_resource(GeneratedWorldLayoutResource {
            master_seed: 7,
            layout: GeneratedWorldLayout::stub(7),
        });
        let contents = encode_save_file(
            SaveHeader::current(8),
            "this is deliberately not DynamicWorld RON",
        );

        assert!(matches!(
            prepare_load_from_str(&world, &contents),
            Err(LoadPreparationError::SeedMismatch {
                saved: 8,
                current: 7,
            })
        ));
    }

    #[test]
    fn malformed_body_is_reported_before_asset_loading() {
        let mut world = World::new();
        world.init_resource::<AppTypeRegistry>();

        assert!(matches!(
            prepare_load_from_str(&world, "#![enable(not_a_real_ron_extension)]"),
            Err(LoadPreparationError::BodySyntax(_))
        ));
    }

    #[test]
    fn legacy_seed_resource_is_not_applied_to_the_live_world() {
        let mut dynamic_world = DynamicWorld {
            resources: vec![Box::new(SavedWorldgenSeed(42))],
            entities: Vec::new(),
        };

        remove_legacy_saved_worldgen_seed(&mut dynamic_world);

        assert!(dynamic_world.resources.is_empty());
    }
}
