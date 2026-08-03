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

use bevy_world_serialization::DynamicWorld;
use bevy_world_serialization::serde::WorldDeserializer;

use crate::world::map::GeneratedWorldLayoutResource;

use super::format::{SaveFormat, SaveFormatError, decode_save_file};
use super::rehydrate::ResolvedRehydratePlan;
use super::schema::{
    DynamicWorldSchemaError, discard_legacy_reserved_for_task, discard_runtime_derived_components,
    validate_persisted_world,
};
use super::state::{SaveLoadFailureKind, SaveLoadResult, SavePath, SavedWorldgenSeed};
use super::transaction::{CommitError, replace_persisted_world, replace_recovery_only_world};

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
            ) => SaveLoadFailureKind::InvalidData,
            Self::Commit(error) => commit_failure_kind(error),
        }
    }
}

pub(super) const fn commit_failure_kind(error: &CommitError) -> SaveLoadFailureKind {
    match error {
        CommitError::Rejected { .. } => SaveLoadFailureKind::InvalidData,
        CommitError::RecoveryModeRequired => SaveLoadFailureKind::RecoveryFailed,
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
            Self::RehydratePrerequisite(error) => {
                write!(formatter, "rehydrate prerequisites failed: {error}")
            }
            Self::Commit(error) => error.fmt(formatter),
        }
    }
}

pub(super) fn load_world_system(world: &mut World) -> SaveLoadResult {
    load_world_with_mode(world, LoadCommitMode::Normal)
}

pub(super) fn recover_world_system(world: &mut World) -> SaveLoadResult {
    load_world_with_mode(world, LoadCommitMode::RecoveryOnly)
}

#[derive(Clone, Copy)]
enum LoadCommitMode {
    Normal,
    RecoveryOnly,
}

fn load_world_with_mode(world: &mut World, mode: LoadCommitMode) -> SaveLoadResult {
    let save_path = world.resource::<SavePath>().as_path().to_path_buf();
    match execute_load(world, &save_path, mode) {
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

fn execute_load(
    world: &mut World,
    save_path: &Path,
    mode: LoadCommitMode,
) -> Result<SaveFormat, LoadExecutionError> {
    let contents = read_save_file(save_path).map_err(LoadExecutionError::Read)?;
    let prepared =
        prepare_load_from_str(world, &contents).map_err(LoadExecutionError::Preparation)?;

    let type_registry = world.get_resource::<AppTypeRegistry>().cloned().ok_or(
        LoadExecutionError::MissingPrerequisite(std::any::type_name::<AppTypeRegistry>()),
    )?;
    let plan = world
        .get_resource::<ResolvedRehydratePlan>()
        .cloned()
        .ok_or(LoadExecutionError::MissingPrerequisite(
            std::any::type_name::<ResolvedRehydratePlan>(),
        ))?;

    plan.validate_live(world)
        .map_err(LoadExecutionError::RehydratePrerequisite)?;

    let registry = type_registry.read();
    let commit_result = match mode {
        LoadCommitMode::Normal => {
            replace_persisted_world(world, &prepared.dynamic_world, &registry, &plan)
        }
        LoadCommitMode::RecoveryOnly => {
            replace_recovery_only_world(world, &prepared.dynamic_world, &registry, &plan)
        }
    };
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

#[cfg(test)]
mod tests {
    use std::any::TypeId;

    use bevy::asset::AssetPlugin;

    use super::*;
    use crate::world::map::GeneratedWorldLayoutResource;
    use hw_core::GameTime;
    use hw_core::familiar::{Familiar, FamiliarOperation, FamiliarPolicy};
    use hw_core::logistics::ResourceType;
    use hw_core::population::PopulationManager;
    use hw_core::relationships::{
        CommandedBy, Commanding, DeliveringTo, IncomingDeliveries, LoadedIn, LoadedItems, PushedBy,
        PushingWheelbarrow, TaskWorkers, WorkingOn,
    };
    use hw_core::soul::{DamnedSoul, DreamPool};
    use hw_energy::{PowerConsumer, PowerShedReason, PowerSupplyState, Unpowered};
    use hw_jobs::construction::FloorTileBlueprint;
    use hw_jobs::mud_mixer::MudMixerStorage;
    use hw_jobs::{Building, BuildingType};
    use hw_logistics::item_lifetime::ItemDespawnTimer;
    use hw_logistics::transport_request::{
        TransportRequestState, WheelbarrowDestination, WheelbarrowLease, WheelbarrowPendingSince,
    };
    use hw_logistics::types::{
        BelongsTo, BucketStorage, PendingBelongsToBlueprint, ReservedForTask, ResourceItem,
    };
    use hw_logistics::{Stockpile, StockpilePolicy, Wheelbarrow};
    use hw_world::GeneratedWorldLayout;
    use hw_world::WorldMap;
    use hw_world::Yard;

    use super::super::format::{SaveHeader, encode_save_file};
    use super::super::rehydrate::{
        rehydrate_familiar_settings, rehydrate_stockpile_policies,
        validate_durable_topology_candidate, validate_task_logistics_candidate,
    };
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

    fn legacy_body_with_power_runtime_state(app: &mut App) -> String {
        let consumer = app.world_mut().spawn(PowerConsumer { demand: 1.0 }).id();
        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let mut dynamic_world =
            build_persisted_world(app.world(), &registry, std::iter::once(consumer));
        let dynamic_consumer = dynamic_world
            .entities
            .iter_mut()
            .find(|entity| entity.entity == consumer)
            .expect("power consumer root must be persisted");
        dynamic_consumer.components.push(Box::new(Unpowered));
        dynamic_consumer
            .components
            .push(Box::new(PowerSupplyState::Shed {
                reason: PowerShedReason::RestoreMargin,
            }));
        dynamic_world.serialize(&registry).unwrap()
    }

    fn legacy_body_with_task_runtime_state(app: &mut App) -> String {
        fn dynamic_entity_mut(
            dynamic_world: &mut DynamicWorld,
            entity: Entity,
        ) -> &mut bevy_world_serialization::DynamicEntity {
            dynamic_world
                .entities
                .iter_mut()
                .find(|dynamic| dynamic.entity == entity)
                .expect("runtime fixture entity must be persisted")
        }

        let (task, soul, wheelbarrow, item) = {
            let world = app.world_mut();
            let task = world.spawn(Building::default()).id();
            let soul = world.spawn(DamnedSoul::default()).id();
            let wheelbarrow = world
                .spawn((
                    ResourceItem(ResourceType::Wheelbarrow),
                    Wheelbarrow { capacity: 4 },
                    Transform::default(),
                ))
                .id();
            let item = world
                .spawn((ResourceItem(ResourceType::Wood), Transform::default()))
                .id();
            world.entity_mut(item).insert(LoadedIn(wheelbarrow));
            world.flush();
            (task, soul, wheelbarrow, item)
        };

        let roots = collect_persisted_entities(app.world_mut());
        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let mut dynamic_world = build_persisted_world(app.world(), &registry, roots.into_iter());
        dynamic_entity_mut(&mut dynamic_world, soul)
            .components
            .push(Box::new(WorkingOn(task)));
        dynamic_entity_mut(&mut dynamic_world, soul)
            .components
            .push(Box::new(PushingWheelbarrow::default()));
        dynamic_entity_mut(&mut dynamic_world, task)
            .components
            .push(Box::new(TaskWorkers::default()));
        dynamic_entity_mut(&mut dynamic_world, task)
            .components
            .push(Box::new(IncomingDeliveries::default()));
        dynamic_entity_mut(&mut dynamic_world, task)
            .components
            .push(Box::new(TransportRequestState::Claimed));
        dynamic_entity_mut(&mut dynamic_world, task)
            .components
            .push(Box::new(WheelbarrowPendingSince(3.0)));
        dynamic_entity_mut(&mut dynamic_world, task)
            .components
            .push(Box::new(WheelbarrowLease {
                wheelbarrow,
                items: vec![item],
                source_pos: Vec2::ZERO,
                destination: WheelbarrowDestination::Stockpile(task),
                lease_until: 8.0,
            }));
        dynamic_entity_mut(&mut dynamic_world, wheelbarrow)
            .components
            .push(Box::new(PushedBy(soul)));
        dynamic_entity_mut(&mut dynamic_world, item)
            .components
            .push(Box::new(DeliveringTo(task)));
        dynamic_entity_mut(&mut dynamic_world, item)
            .components
            .push(Box::new(ItemDespawnTimer::new(5.0)));
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
            classified(LoadExecutionError::Commit(CommitError::Rejected {
                candidate: "incoming",
                cause: "raw preflight details".to_owned(),
            })),
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
            classified(LoadExecutionError::Commit(
                CommitError::RecoveryModeRequired
            )),
            SaveLoadFailureKind::RecoveryFailed
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
    fn legacy_power_runtime_state_is_stripped_for_v0_and_v1_before_resave() {
        let mut app = legacy_loader_test_app();
        let body = legacy_body_with_power_runtime_state(&mut app);
        let fixtures = [
            body.clone(),
            encode_save_file(SaveHeader::current(42), &body),
        ];

        for fixture in fixtures {
            let prepared = prepare_load_from_str(app.world(), &fixture)
                .expect("legacy runtime power state must remain loadable");
            assert!(prepared.dynamic_world.entities.iter().all(|entity| {
                entity.components.iter().all(|component| {
                    component.get_represented_type_info().is_none_or(|info| {
                        info.type_id() != TypeId::of::<Unpowered>()
                            && info.type_id() != TypeId::of::<PowerSupplyState>()
                    })
                })
            }));

            let type_registry = app.world().resource::<AppTypeRegistry>().clone();
            let registry = type_registry.read();
            let resaved_v1_body = prepared.dynamic_world.serialize(&registry).unwrap();
            assert!(!resaved_v1_body.contains(Unpowered::type_path()));
            assert!(!resaved_v1_body.contains(PowerSupplyState::type_path()));
        }
    }

    #[test]
    fn serialized_task_runtime_state_is_stripped_but_loaded_cargo_remaps_for_v0_and_v1() {
        use bevy::ecs::entity::EntityHashMap;

        let mut app = legacy_loader_test_app();
        let body = legacy_body_with_task_runtime_state(&mut app);
        let fixtures = [
            body.clone(),
            encode_save_file(SaveHeader::current(42), &body),
        ];
        let runtime_types = [
            TypeId::of::<WorkingOn>(),
            TypeId::of::<TaskWorkers>(),
            TypeId::of::<DeliveringTo>(),
            TypeId::of::<IncomingDeliveries>(),
            TypeId::of::<PushedBy>(),
            TypeId::of::<PushingWheelbarrow>(),
            TypeId::of::<TransportRequestState>(),
            TypeId::of::<WheelbarrowPendingSince>(),
            TypeId::of::<WheelbarrowLease>(),
            TypeId::of::<ItemDespawnTimer>(),
        ];

        for fixture in fixtures {
            let prepared = prepare_load_from_str(app.world(), &fixture)
                .expect("historical task runtime payload must remain loadable");
            assert!(prepared.dynamic_world.entities.iter().all(|entity| {
                entity.components.iter().all(|component| {
                    component
                        .get_represented_type_info()
                        .is_none_or(|info| !runtime_types.contains(&info.type_id()))
                })
            }));
            assert!(prepared.dynamic_world.entities.iter().any(|entity| {
                entity.components.iter().any(|component| {
                    component
                        .get_represented_type_info()
                        .is_some_and(|info| info.type_id() == TypeId::of::<LoadedIn>())
                })
            }));
            assert!(prepared.dynamic_world.entities.iter().any(|entity| {
                entity.components.iter().any(|component| {
                    component
                        .get_represented_type_info()
                        .is_some_and(|info| info.type_id() == TypeId::of::<LoadedItems>())
                })
            }));

            let type_registry = app.world().resource::<AppTypeRegistry>().clone();
            let registry = type_registry.read();
            let mut candidate = World::new();
            for _ in 0..32 {
                candidate.spawn_empty();
            }
            let mut entity_map = EntityHashMap::default();
            prepared
                .dynamic_world
                .write_to_world_with(&mut candidate, &mut entity_map, &registry)
                .unwrap();
            drop(registry);

            let item = candidate
                .iter_entities()
                .find(|entity| {
                    entity
                        .get::<ResourceItem>()
                        .is_some_and(|item| item.0 == ResourceType::Wood)
                })
                .map(|entity| entity.id())
                .expect("loaded item must be remapped into the candidate");
            let carrier = candidate.get::<LoadedIn>(item).unwrap().0;
            assert!(candidate.get::<Wheelbarrow>(carrier).is_some());
            assert!(!candidate.get::<LoadedItems>(carrier).unwrap().is_empty());
        }
    }

    #[test]
    fn serialized_orphan_construction_link_is_rejected_after_entity_remap() {
        use bevy::ecs::entity::EntityHashMap;

        let mut source = legacy_loader_test_app();
        let missing_site = source.world_mut().spawn_empty().id();
        source.world_mut().despawn(missing_site);
        source.world_mut().spawn((
            FloorTileBlueprint::new(missing_site, (3, 4)),
            Transform::default(),
        ));
        let type_registry = source.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let roots = collect_persisted_entities(source.world_mut());
        let body = build_persisted_world(source.world(), &registry, roots.into_iter())
            .serialize(&registry)
            .unwrap();
        drop(registry);

        for contents in [
            body.clone(),
            encode_save_file(SaveHeader::current(42), &body),
        ] {
            let loader = legacy_loader_test_app();
            let prepared = prepare_load_from_str(loader.world(), &contents)
                .expect("orphan Entity IDs remain syntactically and schematically valid");
            let type_registry = loader.world().resource::<AppTypeRegistry>().clone();
            let registry = type_registry.read();
            let mut candidate = World::new();
            let mut entity_map = EntityHashMap::default();
            prepared
                .dynamic_world
                .write_to_world_with(&mut candidate, &mut entity_map, &registry)
                .unwrap();
            drop(registry);

            assert!(
                validate_durable_topology_candidate(&candidate)
                    .unwrap_err()
                    .contains("references missing parent site")
            );
        }
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
    fn serialized_bucket_storage_is_validated_and_restored_from_its_tank_owner() {
        use bevy::ecs::entity::EntityHashMap;
        use hw_core::relationships::StoredIn;

        let mut source = legacy_loader_test_app();
        let (storage, bucket) = {
            let world = source.world_mut();
            let tank = world
                .spawn((
                    Building {
                        kind: BuildingType::Tank,
                        ..default()
                    },
                    Transform::default(),
                ))
                .id();
            let storage = world
                .spawn((stockpile(2), BelongsTo(tank), Transform::default()))
                .id();
            let bucket = world
                .spawn((
                    ResourceItem(ResourceType::BucketEmpty),
                    BelongsTo(tank),
                    StoredIn(storage),
                    Transform::default(),
                ))
                .id();
            world.flush();
            (storage, bucket)
        };
        let type_registry = source.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let roots = collect_persisted_entities(source.world_mut());
        let body = build_persisted_world(source.world(), &registry, roots.into_iter())
            .serialize(&registry)
            .unwrap();
        drop(registry);

        let loader = legacy_loader_test_app();
        let prepared = prepare_load_from_str(
            loader.world(),
            &encode_save_file(SaveHeader::current(42), &body),
        )
        .expect("current bucket storage fixture must prepare");
        let type_registry = loader.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let mut loaded = World::new();
        let mut entity_map = EntityHashMap::default();
        prepared
            .dynamic_world
            .write_to_world_with(&mut loaded, &mut entity_map, &registry)
            .unwrap();
        drop(registry);

        validate_task_logistics_candidate(&loaded).unwrap();
        assert!(loaded.get::<BucketStorage>(entity_map[&storage]).is_none());
        assert_eq!(
            loaded
                .get::<ResourceItem>(entity_map[&bucket])
                .map(|item| item.0),
            Some(ResourceType::BucketEmpty)
        );

        rehydrate_stockpile_policies(&mut loaded);

        assert!(loaded.get::<BucketStorage>(entity_map[&storage]).is_some());
    }

    #[test]
    fn missing_familiar_settings_migrate_from_serialized_v0_and_v1_rosters() {
        use bevy::ecs::entity::EntityHashMap;

        let mut source = legacy_loader_test_app();
        let familiar = source.world_mut().spawn(Familiar::default()).id();
        let souls = [
            source.world_mut().spawn(CommandedBy(familiar)).id(),
            source.world_mut().spawn(CommandedBy(familiar)).id(),
            source.world_mut().spawn(CommandedBy(familiar)).id(),
        ];
        source.world_mut().flush();

        let type_registry = source.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let roots = collect_persisted_entities(source.world_mut());
        let body = build_persisted_world(source.world(), &registry, roots.into_iter())
            .serialize(&registry)
            .unwrap();
        drop(registry);
        assert!(!body.contains(FamiliarOperation::type_path()));
        assert!(!body.contains(FamiliarPolicy::type_path()));

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

            rehydrate_familiar_settings(&mut loaded).unwrap();

            let loaded_familiar = entity_map[&familiar];
            assert_eq!(
                loaded
                    .get::<FamiliarOperation>(loaded_familiar)
                    .unwrap()
                    .max_controlled_soul,
                souls.len()
            );
            assert_eq!(
                loaded.get::<FamiliarPolicy>(loaded_familiar),
                Some(&FamiliarPolicy::default())
            );
            assert_eq!(
                loaded
                    .get::<Commanding>(loaded_familiar)
                    .unwrap()
                    .iter()
                    .count(),
                souls.len()
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
