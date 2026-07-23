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
    DynamicWorldSchemaError, discard_legacy_reserved_for_task, discard_runtime_derived_components,
    validate_persisted_world,
};
use super::state::{SaveLoadFailureKind, SaveLoadResult, SavePath, SavedWorldgenSeed};
use super::transaction::{CommitError, preflight_dynamic_world, replace_persisted_world};

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

#[derive(Debug)]
enum LoadExecutionError {
    Read(std::io::Error),
    Preparation(LoadPreparationError),
    MissingPrerequisite(&'static str),
    Preflight(String),
    RehydratePrerequisite(String),
    Commit(CommitError),
}

impl LoadExecutionError {
    fn failure_kind(&self) -> SaveLoadFailureKind {
        match self {
            Self::Read(error) if error.kind() == std::io::ErrorKind::NotFound => {
                SaveLoadFailureKind::LoadNotFound
            }
            Self::Read(_) => SaveLoadFailureKind::LoadRead,
            Self::Preparation(LoadPreparationError::Format(
                SaveFormatError::UnsupportedVersion { .. },
            )) => SaveLoadFailureKind::UnsupportedFormat,
            Self::Preparation(LoadPreparationError::MissingPrerequisite(_))
            | Self::MissingPrerequisite(_)
            | Self::RehydratePrerequisite(_) => SaveLoadFailureKind::MissingPrerequisite,
            Self::Preparation(LoadPreparationError::SeedMismatch { .. }) => {
                SaveLoadFailureKind::SeedMismatch
            }
            Self::Preparation(
                LoadPreparationError::Format(_)
                | LoadPreparationError::BodySyntax(_)
                | LoadPreparationError::Deserialize(_)
                | LoadPreparationError::Schema(_),
            )
            | Self::Preflight(_) => SaveLoadFailureKind::InvalidData,
            Self::Commit(error) => commit_failure_kind(error),
        }
    }
}

pub(super) const fn commit_failure_kind(error: &CommitError) -> SaveLoadFailureKind {
    match error {
        CommitError::Recovered { .. } => SaveLoadFailureKind::ApplyRecovered,
        CommitError::RecoveryFailed { .. } => SaveLoadFailureKind::RecoveryFailed,
    }
}

impl fmt::Display for LoadExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "save file read failed: {error}"),
            Self::Preparation(error) => error.fmt(formatter),
            Self::MissingPrerequisite(resource) => {
                write!(
                    formatter,
                    "load prerequisite resource is unavailable: {resource}"
                )
            }
            Self::Preflight(error) => write!(formatter, "load preflight failed: {error}"),
            Self::RehydratePrerequisite(error) => {
                write!(formatter, "rehydrate prerequisites failed: {error}")
            }
            Self::Commit(error) => error.fmt(formatter),
        }
    }
}

pub(super) fn load_world_system(world: &mut World) -> SaveLoadResult {
    let save_path = world.resource::<SavePath>().as_path().to_path_buf();
    match execute_load(world, &save_path) {
        Ok(format) => {
            let format = match format {
                SaveFormat::LegacyV0 => "legacy v0",
                SaveFormat::V1(_) => "v1",
            };
            info!("World loaded from {} ({format})", save_path.display());
            SaveLoadResult::Succeeded
        }
        Err(error) if error.failure_kind() == SaveLoadFailureKind::ApplyRecovered => {
            warn!(
                "Load failed after live apply for {}; rollback recovery completed: {error}",
                save_path.display()
            );
            SaveLoadResult::Failed(error.failure_kind())
        }
        Err(error) => {
            error!("Load aborted for {}: {error}", save_path.display());
            SaveLoadResult::Failed(error.failure_kind())
        }
    }
}

fn execute_load(world: &mut World, save_path: &Path) -> Result<SaveFormat, LoadExecutionError> {
    let contents = read_save_file(save_path).map_err(LoadExecutionError::Read)?;
    let prepared =
        prepare_load_from_str(world, &contents).map_err(LoadExecutionError::Preparation)?;

    let type_registry = world.get_resource::<AppTypeRegistry>().cloned().ok_or(
        LoadExecutionError::MissingPrerequisite(std::any::type_name::<AppTypeRegistry>()),
    )?;
    let registry = type_registry.read();
    preflight_dynamic_world(&prepared.dynamic_world, &registry)
        .map_err(|error| LoadExecutionError::Preflight(error.to_string()))?;
    drop(registry);

    validate_rehydrate_prerequisites(world)
        .map_err(|error| LoadExecutionError::RehydratePrerequisite(error.to_string()))?;

    let registry = type_registry.read();
    let commit_result = replace_persisted_world(
        world,
        &prepared.dynamic_world,
        &registry,
        finalize_loaded_world,
    );
    drop(registry);
    commit_result.map_err(LoadExecutionError::Commit)?;

    Ok(prepared.format)
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
        discard_legacy_reserved_for_task(&mut dynamic_world);
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
    use std::any::TypeId;

    use bevy::asset::AssetPlugin;

    use super::*;
    use crate::world::map::GeneratedWorldLayoutResource;
    use hw_core::GameTime;
    use hw_core::logistics::ResourceType;
    use hw_core::population::PopulationManager;
    use hw_core::soul::DreamPool;
    use hw_jobs::Building;
    use hw_jobs::mud_mixer::MudMixerStorage;
    use hw_logistics::types::{
        BelongsTo, BucketStorage, PendingBelongsToBlueprint, ReservedForTask, ResourceItem,
    };
    use hw_logistics::{Stockpile, StockpilePolicy};
    use hw_world::GeneratedWorldLayout;
    use hw_world::WorldMap;
    use hw_world::Yard;

    use super::super::format::{SaveHeader, encode_save_file};
    use super::super::rehydrate::rehydrate_stockpile_policies;
    use super::super::schema::{
        build_persisted_world, collect_persisted_entities, register_save_types,
    };

    fn classified(error: LoadExecutionError) -> SaveLoadFailureKind {
        error.failure_kind()
    }

    fn legacy_loader_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default());
        register_save_types(&mut app);

        let world = app.world_mut();
        world.insert_resource(GeneratedWorldLayoutResource {
            master_seed: 42,
            layout: GeneratedWorldLayout::stub(42),
        });
        world.insert_resource(GameTime::default());
        world.insert_resource(DreamPool::default());
        world.insert_resource(PopulationManager::default());
        world.insert_resource(WorldMap::default());
        app
    }

    fn legacy_body_with_reserved_for_task(app: &mut App) -> String {
        let item = app.world_mut().spawn(ResourceItem(ResourceType::Wood)).id();
        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let mut dynamic_world =
            build_persisted_world(app.world(), &registry, std::iter::once(item));
        dynamic_world
            .entities
            .iter_mut()
            .find(|entity| entity.entity == item)
            .expect("resource item root must be persisted")
            .components
            .push(Box::new(ReservedForTask));
        dynamic_world.serialize(&registry).unwrap()
    }

    fn stockpile(capacity: usize) -> Stockpile {
        Stockpile {
            capacity,
            resource_type: None,
        }
    }

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
    fn execution_errors_map_exhaustively_to_display_safe_failure_kinds() {
        assert_eq!(
            classified(LoadExecutionError::Read(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "private path",
            ))),
            SaveLoadFailureKind::LoadNotFound
        );
        assert_eq!(
            classified(LoadExecutionError::Read(std::io::Error::other(
                "private path",
            ))),
            SaveLoadFailureKind::LoadRead
        );
        assert_eq!(
            classified(LoadExecutionError::Preparation(
                LoadPreparationError::Format(SaveFormatError::UnsupportedVersion {
                    found: 2,
                    current: 1,
                })
            )),
            SaveLoadFailureKind::UnsupportedFormat
        );
        assert_eq!(
            classified(LoadExecutionError::Preparation(
                LoadPreparationError::BodySyntax("raw parser details".to_owned())
            )),
            SaveLoadFailureKind::InvalidData
        );
        assert_eq!(
            classified(LoadExecutionError::Preparation(
                LoadPreparationError::Format(SaveFormatError::InvalidHeader(
                    "raw header details".to_owned()
                ))
            )),
            SaveLoadFailureKind::InvalidData
        );
        assert_eq!(
            classified(LoadExecutionError::Preparation(
                LoadPreparationError::Deserialize("raw deserialize details".to_owned())
            )),
            SaveLoadFailureKind::InvalidData
        );
        assert_eq!(
            classified(LoadExecutionError::Preflight(
                "raw preflight details".to_owned()
            )),
            SaveLoadFailureKind::InvalidData
        );
        assert_eq!(
            classified(LoadExecutionError::Preparation(
                LoadPreparationError::SeedMismatch {
                    saved: 1,
                    current: 2,
                }
            )),
            SaveLoadFailureKind::SeedMismatch
        );
        assert_eq!(
            classified(LoadExecutionError::MissingPrerequisite("registry")),
            SaveLoadFailureKind::MissingPrerequisite
        );
        assert_eq!(
            classified(LoadExecutionError::Preparation(
                LoadPreparationError::MissingPrerequisite("asset server")
            )),
            SaveLoadFailureKind::MissingPrerequisite
        );
        assert_eq!(
            classified(LoadExecutionError::RehydratePrerequisite(
                "raw prerequisite details".to_owned()
            )),
            SaveLoadFailureKind::MissingPrerequisite
        );
        assert_eq!(
            classified(LoadExecutionError::Commit(CommitError::Recovered {
                cause: "raw apply details".to_owned(),
            })),
            SaveLoadFailureKind::ApplyRecovered
        );
        assert_eq!(
            classified(LoadExecutionError::Commit(CommitError::RecoveryFailed {
                cause: "raw apply details".to_owned(),
                recovery: "raw recovery details".to_owned(),
            })),
            SaveLoadFailureKind::RecoveryFailed
        );
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

    #[test]
    fn legacy_reserved_marker_is_stripped_before_schema_validation_and_v1_resave() {
        assert_eq!(
            ReservedForTask::type_path(),
            "hw_logistics::types::ReservedForTask",
            "headerless v0 bodies require the historical reflected type path"
        );

        let mut app = legacy_loader_test_app();
        let legacy_body = legacy_body_with_reserved_for_task(&mut app);

        let prepared = prepare_load_from_str(app.world(), &legacy_body)
            .expect("headerless v0 body with the legacy marker must remain loadable");
        assert_eq!(prepared.format, SaveFormat::LegacyV0);
        assert!(prepared.dynamic_world.entities.iter().all(|entity| {
            entity.components.iter().all(|component| {
                component
                    .get_represented_type_info()
                    .is_none_or(|info| info.type_id() != TypeId::of::<ReservedForTask>())
            })
        }));

        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let resaved_v1_body = prepared.dynamic_world.serialize(&registry).unwrap();
        assert!(!resaved_v1_body.contains(ReservedForTask::type_path()));
    }

    #[test]
    fn missing_stockpile_policy_migrates_only_yard_owned_cells_for_v0_and_v1() {
        use bevy::ecs::entity::EntityHashMap;

        let mut source = legacy_loader_test_app();
        let (ordinary, tank, legacy_companion, mixer, pending_companion) = {
            let world = source.world_mut();
            let yard = world
                .spawn(Yard {
                    min: Vec2::ZERO,
                    max: Vec2::splat(10.0),
                })
                .id();
            let ordinary = world.spawn((stockpile(6), BelongsTo(yard))).id();
            let tank = world.spawn((Building::default(), stockpile(4))).id();
            let legacy_companion = world
                .spawn((stockpile(2), BucketStorage, BelongsTo(tank)))
                .id();
            let mixer = world
                .spawn((
                    Building::default(),
                    MudMixerStorage::default(),
                    stockpile(3),
                ))
                .id();
            let pending_companion = world
                .spawn((stockpile(2), PendingBelongsToBlueprint(tank)))
                .id();
            (ordinary, tank, legacy_companion, mixer, pending_companion)
        };
        let type_registry = source.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let roots = collect_persisted_entities(source.world_mut());
        let body = build_persisted_world(source.world(), &registry, roots.into_iter())
            .serialize(&registry)
            .unwrap();
        drop(registry);

        let fixtures = [
            body.clone(),
            encode_save_file(SaveHeader::current(42), &body),
        ];
        for contents in fixtures {
            let loader = legacy_loader_test_app();
            let prepared = prepare_load_from_str(loader.world(), &contents).unwrap();
            let type_registry = loader.world().resource::<AppTypeRegistry>().clone();
            let registry = type_registry.read();
            let mut loaded = World::new();
            let mut entity_map = EntityHashMap::default();
            prepared
                .dynamic_world
                .write_to_world_with(&mut loaded, &mut entity_map, &registry)
                .unwrap();
            drop(registry);

            rehydrate_stockpile_policies(&mut loaded);

            assert_eq!(
                loaded.get::<StockpilePolicy>(entity_map[&ordinary]),
                Some(&StockpilePolicy::for_capacity(6))
            );
            for special in [tank, legacy_companion, mixer, pending_companion] {
                assert!(
                    loaded
                        .get::<StockpilePolicy>(entity_map[&special])
                        .is_none()
                );
            }
            assert!(
                loaded
                    .get::<BucketStorage>(entity_map[&legacy_companion])
                    .is_none()
            );
        }
    }

    #[test]
    fn v1_body_with_legacy_reserved_marker_is_rejected() {
        let mut app = legacy_loader_test_app();
        let legacy_body = legacy_body_with_reserved_for_task(&mut app);
        let v1_contents = encode_save_file(SaveHeader::current(42), &legacy_body);

        assert!(matches!(
            prepare_load_from_str(app.world(), &v1_contents),
            Err(LoadPreparationError::Schema(_))
        ));
    }
}
