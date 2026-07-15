//! セーブ形式の型schema。
//!
//! 型を追加・削除するときは、このファイルの分類だけを更新する。各分類は
//! `register_save_types`、DynamicWorld の allow-list、root entity の収集に展開される。
//! runtime shell、cache、`AssignedTask`、`ObstacleSourceKind`、表示用 `ChildOf` 階層は
//! 意図的にこのschemaの外に置き、ロード後の再構築に委ねる。

use std::collections::HashSet;
use std::fmt;

use bevy::prelude::*;
use bevy::reflect::TypeRegistry;
use bevy_world_serialization::{DynamicWorld, DynamicWorldBuilder};

use crate::entities::damned_soul::{Gender, SoulIdentity};
use crate::world::map::Tile;

use hw_core::GameTime;
use hw_core::area::{AreaBounds, TaskArea};
use hw_core::familiar::{Familiar, FamiliarType};
use hw_core::logistics::ResourceType;
use hw_core::population::PopulationManager;
use hw_core::relationships::{
    CommandedBy, Commanding, DeliveringTo, GatheringParticipants, IncomingDeliveries, LoadedIn,
    LoadedItems, ManagedBy, ManagedTasks, ParkedAt, ParkedWheelbarrows, ParticipatingIn, PushedBy,
    PushingWheelbarrow, RestAreaOccupants, RestAreaReservations, RestAreaReservedFor, RestingIn,
    StoredIn, StoredItems, TaskWorkers, WorkingOn,
};
use hw_core::soul::{
    DamnedSoul, DreamPool, DreamQuality, DreamState, DriftEdge, DriftPhase, DriftingState,
    GatheringBehavior, IdleBehavior, IdleState, RestAreaCooldown, StressBreakdown,
};
use hw_core::world::DoorState;

use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerGenerator,
    PowerGrid, SoulSpaPhase, SoulSpaSite, SoulSpaTile, Unpowered, YardPowerGrid,
};

use hw_jobs::construction::{
    FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
    WallConstructionPhase, WallConstructionSite, WallTileBlueprint, WallTileState,
};
use hw_jobs::mud_mixer::{MudMixerStorage, StoredByMixer, TargetMixer};
use hw_jobs::{
    Blueprint, BonePile, BridgeMarker, Building, BuildingType, Designation,
    FlexibleMaterialRequirement, ObstaclePosition, Priority, ProvisionalWall, RestArea, Rock,
    SandPile, TargetBlueprint, TargetSoulSpaSite, TaskSlots, Tree, TreeVariant, WorkType,
};

use hw_logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportPolicy,
    TransportPriority, TransportRequest, TransportRequestFixedSource, TransportRequestKind,
};
use hw_logistics::types::{ReservedForTask, WheelbarrowParking};
use hw_logistics::zone::Stockpile;
use hw_logistics::{BelongsTo, Inventory, PendingBelongsToBlueprint, ResourceItem, Wheelbarrow};

use hw_world::{TerrainType, WorldMap};

use super::state::SavedWorldgenSeed;

macro_rules! for_each_persisted_resource {
    ($callback:ident) => {
        $callback!(GameTime);
        $callback!(DreamPool);
        $callback!(PopulationManager);
        $callback!(WorldMap);
    };
}

macro_rules! for_each_persisted_component {
    ($callback:ident) => {
        $callback!(DamnedSoul);
        $callback!(IdleState);
        $callback!(DreamState);
        $callback!(StressBreakdown);
        $callback!(RestAreaCooldown);
        $callback!(DriftingState);
        $callback!(Familiar);
        $callback!(CommandedBy);
        $callback!(Commanding);
        $callback!(WorkingOn);
        $callback!(TaskWorkers);
        $callback!(ManagedBy);
        $callback!(ManagedTasks);
        $callback!(StoredIn);
        $callback!(StoredItems);
        $callback!(LoadedIn);
        $callback!(LoadedItems);
        $callback!(ParkedAt);
        $callback!(ParkedWheelbarrows);
        $callback!(PushedBy);
        $callback!(PushingWheelbarrow);
        $callback!(DeliveringTo);
        $callback!(IncomingDeliveries);
        $callback!(RestingIn);
        $callback!(RestAreaOccupants);
        $callback!(RestAreaReservedFor);
        $callback!(RestAreaReservations);
        $callback!(Designation);
        $callback!(Priority);
        $callback!(TaskSlots);
        $callback!(TaskArea);
        $callback!(Building);
        $callback!(hw_jobs::Door);
        $callback!(RestArea);
        $callback!(Blueprint);
        $callback!(ProvisionalWall);
        $callback!(BridgeMarker);
        $callback!(SandPile);
        $callback!(BonePile);
        $callback!(TargetBlueprint);
        $callback!(TargetSoulSpaSite);
        $callback!(FloorConstructionSite);
        $callback!(FloorTileBlueprint);
        $callback!(WallConstructionSite);
        $callback!(WallTileBlueprint);
        $callback!(ResourceItem);
        $callback!(BelongsTo);
        $callback!(PendingBelongsToBlueprint);
        $callback!(Inventory);
        $callback!(Wheelbarrow);
        $callback!(WheelbarrowParking);
        $callback!(Stockpile);
        $callback!(TransportRequest);
        $callback!(TransportRequestFixedSource);
        $callback!(ManualTransportRequest);
        $callback!(ManualHaulPinnedSource);
        $callback!(TransportDemand);
        $callback!(TransportPolicy);
        $callback!(MudMixerStorage);
        $callback!(TargetMixer);
        $callback!(StoredByMixer);
        $callback!(PowerGrid);
        $callback!(PowerGenerator);
        $callback!(PowerConsumer);
        $callback!(Unpowered);
        $callback!(YardPowerGrid);
        $callback!(GeneratesFor);
        $callback!(GridGenerators);
        $callback!(ConsumesFrom);
        $callback!(GridConsumers);
        $callback!(SoulSpaSite);
        $callback!(SoulSpaTile);
        $callback!(Tree);
        $callback!(TreeVariant);
        $callback!(Rock);
        $callback!(ObstaclePosition);
        $callback!(Tile);
        $callback!(SoulIdentity);
        $callback!(hw_world::zones::Site);
        $callback!(hw_world::zones::Yard);
        $callback!(hw_world::zones::PairedSite);
        $callback!(hw_world::zones::PairedYard);
    };
}

// Gathering spots and their relationship endpoints are short-lived behavior
// state. Keep the types registered to read older bodies, but never include
// them in a new durable DynamicWorld.
macro_rules! for_each_runtime_derived_component {
    ($callback:ident) => {
        $callback!(ParticipatingIn);
        $callback!(GatheringParticipants);
    };
}

// `ReservedForTask` was persisted by headerless v0 saves. Keep it registered
// only so those bodies can deserialize; it is stripped before v0 schema
// validation and never enters the v1 allow-list.
macro_rules! for_each_legacy_v0_component {
    ($callback:ident) => {
        $callback!(ReservedForTask);
    };
}

// `Transform` is registered by Bevy's `reflect_auto_register` feature in production apps.
// Keep it separate so the dependency is visible and can be tested instead of becoming implicit.
macro_rules! for_each_external_registered_component {
    ($callback:ident) => {
        $callback!(Transform);
    };
}

macro_rules! for_each_reflect_dependency {
    ($callback:ident) => {
        $callback!(IdleBehavior);
        $callback!(GatheringBehavior);
        $callback!(DreamQuality);
        $callback!(DriftPhase);
        $callback!(DriftEdge);
        $callback!(FamiliarType);
        $callback!(WorkType);
        $callback!(AreaBounds);
        $callback!(BuildingType);
        $callback!(DoorState);
        $callback!(FlexibleMaterialRequirement);
        $callback!(FloorConstructionPhase);
        $callback!(FloorTileState);
        $callback!(WallConstructionPhase);
        $callback!(WallTileState);
        $callback!(ResourceType);
        $callback!(TransportRequestKind);
        $callback!(TransportPriority);
        $callback!(SoulSpaPhase);
        $callback!(TerrainType);
        $callback!(Gender);
        $callback!(SavedWorldgenSeed);
    };
}

macro_rules! for_each_root_marker {
    ($callback:ident) => {
        $callback!(DamnedSoul);
        $callback!(Familiar);
        $callback!(Designation);
        $callback!(Building);
        $callback!(hw_jobs::Door);
        $callback!(RestArea);
        $callback!(TaskArea);
        $callback!(Blueprint);
        $callback!(FloorConstructionSite);
        $callback!(FloorTileBlueprint);
        $callback!(WallConstructionSite);
        $callback!(WallTileBlueprint);
        $callback!(ResourceItem);
        $callback!(Wheelbarrow);
        $callback!(WheelbarrowParking);
        $callback!(Stockpile);
        $callback!(TransportRequest);
        $callback!(PowerGrid);
        $callback!(PowerGenerator);
        $callback!(PowerConsumer);
        $callback!(SoulSpaSite);
        $callback!(SoulSpaTile);
        $callback!(Tree);
        $callback!(Rock);
        $callback!(Tile);
        $callback!(hw_world::zones::Site);
        $callback!(hw_world::zones::Yard);
    };
}

/// Registers the schema-owned reflected types used to deserialize saved data.
///
/// `Transform` stays in the external-registration classification above: Bevy registers it
/// for the production app, and the corresponding test verifies its `ReflectComponent` data.
pub(super) fn register_save_types(app: &mut App) {
    macro_rules! register_type {
        ($type:ty) => {
            app.register_type::<$type>();
        };
    }

    for_each_persisted_resource!(register_type);
    for_each_persisted_component!(register_type);
    for_each_runtime_derived_component!(register_type);
    for_each_legacy_v0_component!(register_type);
    for_each_reflect_dependency!(register_type);
}

/// Builds the DynamicWorld that represents the durable simulation state.
pub(super) fn build_persisted_world(
    world: &World,
    type_registry: &TypeRegistry,
    target_entities: impl Iterator<Item = Entity>,
) -> DynamicWorld {
    let mut builder = DynamicWorldBuilder::from_world(world, type_registry)
        .deny_all_components()
        .deny_all_resources();

    macro_rules! allow_resource {
        ($type:ty) => {
            builder = builder.allow_resource::<$type>();
        };
    }
    macro_rules! allow_component {
        ($type:ty) => {
            builder = builder.allow_component::<$type>();
        };
    }

    for_each_persisted_resource!(allow_resource);
    for_each_persisted_component!(allow_component);
    for_each_external_registered_component!(allow_component);

    builder
        .extract_entities(target_entities)
        .remove_empty_entities()
        .extract_resources()
        .build()
}

/// Removes state that older save bodies may carry but which belongs to the
/// discarded runtime world. This runs before schema validation, allowing a
/// legacy body to load while ensuring the live apply never receives dangling
/// gathering relationship references.
pub(super) fn discard_runtime_derived_components(dynamic_world: &mut DynamicWorld) {
    macro_rules! is_runtime_derived_component {
        ($type_id:expr) => {{
            let mut runtime_derived = false;
            macro_rules! matches_type {
                ($type:ty) => {
                    runtime_derived |= $type_id == std::any::TypeId::of::<$type>();
                };
            }
            for_each_runtime_derived_component!(matches_type);
            runtime_derived
        }};
    }

    for entity in &mut dynamic_world.entities {
        entity.components.retain(|component| {
            !component
                .get_represented_type_info()
                .is_some_and(|info| is_runtime_derived_component!(info.type_id()))
        });
    }
}

/// Removes the loader-only v0 marker before the durable schema rejects it.
///
/// This must only be called for headerless v0 files. A v1 body containing the
/// removed marker is malformed and must remain visible to schema validation.
pub(super) fn discard_legacy_reserved_for_task(dynamic_world: &mut DynamicWorld) {
    let legacy_type = std::any::TypeId::of::<ReservedForTask>();
    for entity in &mut dynamic_world.entities {
        entity.components.retain(|component| {
            component
                .get_represented_type_info()
                .is_none_or(|info| info.type_id() != legacy_type)
        });
    }
}

/// Error returned when a deserialized DynamicWorld cannot satisfy the durable
/// resource contract required by the live simulation and rehydrate steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DynamicWorldSchemaError {
    missing_resources: Vec<&'static str>,
    unsupported_resources: Vec<String>,
    unsupported_components: Vec<String>,
    rootless_entities: Vec<Entity>,
}

impl fmt::Display for DynamicWorldSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut reasons = Vec::new();
        if !self.missing_resources.is_empty() {
            reasons.push(format!(
                "missing required persisted resource(s): {}",
                self.missing_resources.join(", ")
            ));
        }
        if !self.unsupported_resources.is_empty() {
            reasons.push(format!(
                "unsupported resource(s): {}",
                self.unsupported_resources.join(", ")
            ));
        }
        if !self.unsupported_components.is_empty() {
            reasons.push(format!(
                "unsupported component(s): {}",
                self.unsupported_components.join(", ")
            ));
        }
        if !self.rootless_entities.is_empty() {
            reasons.push(format!(
                "entity or entities without a persisted root marker: {}",
                self.rootless_entities.len()
            ));
        }
        write!(
            formatter,
            "save body violates the persisted schema: {}",
            reasons.join("; ")
        )
    }
}

/// Ensures a deserialized save carries every durable resource required to
/// replace the current simulation world and does not contain allow-list-external
/// types. This runs before any live despawn.
pub(super) fn validate_persisted_world(
    dynamic_world: &DynamicWorld,
) -> Result<(), DynamicWorldSchemaError> {
    let mut missing_resources = Vec::new();
    let mut unsupported_resources = Vec::new();
    let mut unsupported_components = Vec::new();
    let mut rootless_entities = Vec::new();

    macro_rules! require_resource {
        ($type:ty) => {
            if !dynamic_world.resources.iter().any(|resource| {
                resource
                    .get_represented_type_info()
                    .is_some_and(|info| info.type_id() == std::any::TypeId::of::<$type>())
            }) {
                missing_resources.push(std::any::type_name::<$type>());
            }
        };
    }

    for_each_persisted_resource!(require_resource);

    macro_rules! is_persisted_resource {
        ($type_id:expr) => {{
            let mut allowed = false;
            macro_rules! matches_type {
                ($type:ty) => {
                    allowed |= $type_id == std::any::TypeId::of::<$type>();
                };
            }
            for_each_persisted_resource!(matches_type);
            allowed
        }};
    }
    macro_rules! is_persisted_component {
        ($type_id:expr) => {{
            let mut allowed = false;
            macro_rules! matches_type {
                ($type:ty) => {
                    allowed |= $type_id == std::any::TypeId::of::<$type>();
                };
            }
            for_each_persisted_component!(matches_type);
            for_each_external_registered_component!(matches_type);
            allowed
        }};
    }
    macro_rules! is_root_marker {
        ($type_id:expr) => {{
            let mut root_marker = false;
            macro_rules! matches_type {
                ($type:ty) => {
                    root_marker |= $type_id == std::any::TypeId::of::<$type>();
                };
            }
            for_each_root_marker!(matches_type);
            root_marker
        }};
    }

    for resource in &dynamic_world.resources {
        let allowed = resource
            .get_represented_type_info()
            .is_some_and(|info| is_persisted_resource!(info.type_id()));
        if !allowed {
            unsupported_resources.push(resource.reflect_type_path().to_string());
        }
    }
    for entity in &dynamic_world.entities {
        let has_root_marker = entity.components.iter().any(|component| {
            component
                .get_represented_type_info()
                .is_some_and(|info| is_root_marker!(info.type_id()))
        });
        if !has_root_marker {
            rootless_entities.push(entity.entity);
        }
        for component in &entity.components {
            let allowed = component
                .get_represented_type_info()
                .is_some_and(|info| is_persisted_component!(info.type_id()));
            if !allowed {
                unsupported_components.push(component.reflect_type_path().to_string());
            }
        }
    }

    unsupported_resources.sort();
    unsupported_resources.dedup();
    unsupported_components.sort();
    unsupported_components.dedup();

    if missing_resources.is_empty()
        && unsupported_resources.is_empty()
        && unsupported_components.is_empty()
        && rootless_entities.is_empty()
    {
        Ok(())
    } else {
        Err(DynamicWorldSchemaError {
            missing_resources,
            unsupported_resources,
            unsupported_components,
            rootless_entities,
        })
    }
}

fn ids_with<T: Component>(world: &mut World, out: &mut HashSet<Entity>) {
    let mut query = world.query_filtered::<Entity, With<T>>();
    out.extend(query.iter(world));
}

/// Collects all durable simulation entities from the root-marker classification.
pub(super) fn collect_persisted_entities(world: &mut World) -> Vec<Entity> {
    let mut entities = HashSet::new();

    macro_rules! collect_root_marker {
        ($type:ty) => {
            ids_with::<$type>(world, &mut entities);
        };
    }

    for_each_root_marker!(collect_root_marker);
    entities.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::collections::HashSet;

    use bevy::asset::{AssetPath, LoadFromPath, UntypedHandle};
    use bevy::ecs::entity::EntityHashMap;
    use bevy::ecs::reflect::{AppTypeRegistry, ReflectComponent, ReflectResource};
    use bevy::reflect::{ReflectDeserialize, ReflectSerialize, TypePath, TypeRegistry};
    use bevy_world_serialization::serde::WorldDeserializer;
    use serde::de::DeserializeSeed;

    use super::*;

    struct NoAssetLoad;

    #[derive(Reflect)]
    struct RuntimeOnlyType;

    impl LoadFromPath for NoAssetLoad {
        fn load_from_path_erased(
            &mut self,
            _type_id: TypeId,
            _path: AssetPath<'static>,
        ) -> UntypedHandle {
            panic!("schema fixture contains no asset handles")
        }
    }

    fn registration<T: 'static>(registry: &TypeRegistry) -> &bevy::reflect::TypeRegistration {
        registry.get(TypeId::of::<T>()).unwrap_or_else(|| {
            panic!(
                "{} is missing from the save type registry",
                std::any::type_name::<T>()
            )
        })
    }

    fn assert_registered<T: 'static>(registry: &TypeRegistry) {
        registration::<T>(registry);
    }

    fn assert_component_registration<T: 'static>(registry: &TypeRegistry) {
        assert!(
            registration::<T>(registry)
                .data::<ReflectComponent>()
                .is_some(),
            "{} is missing ReflectComponent data",
            std::any::type_name::<T>()
        );
    }

    fn assert_resource_registration<T: 'static>(registry: &TypeRegistry) {
        let registration = registration::<T>(registry);
        assert!(
            registration.data::<ReflectResource>().is_some(),
            "{} is missing ReflectResource data",
            std::any::type_name::<T>()
        );
        assert!(
            registration.data::<ReflectComponent>().is_some(),
            "{} is missing ReflectComponent data",
            std::any::type_name::<T>()
        );
    }

    #[test]
    fn schema_registers_owned_types_with_their_required_reflect_data() {
        let mut app = App::empty();
        app.init_resource::<AppTypeRegistry>();
        register_save_types(&mut app);

        let registry = app.world().resource::<AppTypeRegistry>().read();

        macro_rules! assert_resource {
            ($type:ty) => {
                assert_resource_registration::<$type>(&registry);
            };
        }
        macro_rules! assert_component {
            ($type:ty) => {
                assert_component_registration::<$type>(&registry);
            };
        }
        macro_rules! assert_dependency {
            ($type:ty) => {
                assert_registered::<$type>(&registry);
            };
        }

        for_each_persisted_resource!(assert_resource);
        for_each_persisted_component!(assert_component);
        for_each_runtime_derived_component!(assert_component);
        for_each_legacy_v0_component!(assert_component);
        for_each_reflect_dependency!(assert_dependency);
        assert!(
            registration::<WorldMap>(&registry)
                .data::<ReflectSerialize>()
                .is_some()
        );
        assert!(
            registration::<WorldMap>(&registry)
                .data::<ReflectDeserialize>()
                .is_some()
        );
        assert!(
            registry.get(TypeId::of::<Transform>()).is_none(),
            "Transform must remain an explicit external-registration dependency"
        );
    }

    #[test]
    fn external_components_exist_in_the_production_registry() {
        let mut app = App::new();
        register_save_types(&mut app);
        let registry = app.world().resource::<AppTypeRegistry>().read();

        macro_rules! assert_external_component {
            ($type:ty) => {
                assert_component_registration::<$type>(&registry);
            };
        }

        for_each_external_registered_component!(assert_external_component);
    }

    #[test]
    fn root_marker_matrix_collects_extracts_and_round_trips_durable_entities() {
        let mut app = App::new();
        register_save_types(&mut app);
        app.world_mut().insert_resource(GameTime {
            seconds: 42.0,
            ..default()
        });

        let (expected_roots, familiar, soul, unmarked_transform, non_root_component) = {
            let world = app.world_mut();
            let familiar = world
                .spawn((Familiar::default(), Transform::default()))
                .id();
            let soul = world
                .spawn((
                    DamnedSoul::default(),
                    Transform::from_xyz(1.0, 2.0, 0.0),
                    CommandedBy(familiar),
                ))
                .id();
            let building_door = world
                .spawn((Building::default(), hw_jobs::Door::default()))
                .id();
            let area = TaskArea::from_points(Vec2::ZERO, Vec2::ONE);
            let mut roots = HashSet::from([
                familiar,
                soul,
                building_door,
                world.spawn(RestArea { capacity: 1 }).id(),
                world.spawn(area.clone()).id(),
                world
                    .spawn(Blueprint::new(BuildingType::Wall, Vec::new()))
                    .id(),
                world
                    .spawn(FloorConstructionSite::new(area.clone(), Vec2::ZERO, 1))
                    .id(),
                world
                    .spawn(FloorTileBlueprint::new(Entity::PLACEHOLDER, (0, 0)))
                    .id(),
                world
                    .spawn(WallConstructionSite::new(area, Vec2::ZERO, 1))
                    .id(),
                world
                    .spawn(WallTileBlueprint::new(Entity::PLACEHOLDER, (0, 0)))
                    .id(),
                world.spawn(ResourceItem(ResourceType::Wood)).id(),
                world.spawn(Wheelbarrow { capacity: 1 }).id(),
                world.spawn(WheelbarrowParking { capacity: 1 }).id(),
                world
                    .spawn(Stockpile {
                        capacity: 1,
                        resource_type: None,
                    })
                    .id(),
                world
                    .spawn(TransportRequest {
                        kind: TransportRequestKind::DepositToStockpile,
                        anchor: Entity::PLACEHOLDER,
                        resource_type: ResourceType::Wood,
                        issued_by: Entity::PLACEHOLDER,
                        priority: TransportPriority::Normal,
                        stockpile_group: Vec::new(),
                    })
                    .id(),
                world.spawn(PowerGrid::default()).id(),
                world.spawn(PowerGenerator::default()).id(),
                world.spawn(PowerConsumer { demand: 1.0 }).id(),
                world.spawn(SoulSpaSite::default()).id(),
                world
                    .spawn(SoulSpaTile {
                        parent_site: Entity::PLACEHOLDER,
                        grid_pos: (0, 0),
                    })
                    .id(),
                world.spawn(Tree).id(),
                world.spawn(Rock).id(),
                world.spawn(Tile).id(),
                world
                    .spawn(hw_world::zones::Site {
                        min: Vec2::ZERO,
                        max: Vec2::ONE,
                    })
                    .id(),
                world
                    .spawn(hw_world::zones::Yard {
                        min: Vec2::ZERO,
                        max: Vec2::ONE,
                    })
                    .id(),
            ]);

            let designation = world
                .spawn(Designation {
                    work_type: WorkType::default(),
                })
                .id();
            roots.insert(designation);

            let unmarked_transform = world.spawn(Transform::default()).id();
            let non_root_component = world.spawn(IdleState::default()).id();
            world.flush();

            (
                roots,
                familiar,
                soul,
                unmarked_transform,
                non_root_component,
            )
        };

        let target_entities = collect_persisted_entities(app.world_mut());
        let collected: HashSet<_> = target_entities.iter().copied().collect();
        assert_eq!(collected, expected_roots);
        assert!(!collected.contains(&unmarked_transform));
        assert!(!collected.contains(&non_root_component));

        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let dynamic_world =
            build_persisted_world(app.world(), &registry, target_entities.iter().copied());
        let extracted: HashSet<_> = dynamic_world
            .entities
            .iter()
            .map(|entity| entity.entity)
            .collect();
        assert_eq!(extracted, expected_roots);

        let has_component = |entity: Entity, type_id: TypeId| {
            dynamic_world
                .entities
                .iter()
                .find(|dynamic_entity| dynamic_entity.entity == entity)
                .is_some_and(|dynamic_entity| {
                    dynamic_entity.components.iter().any(|component| {
                        component
                            .get_represented_type_info()
                            .is_some_and(|info| info.type_id() == type_id)
                    })
                })
        };
        assert!(has_component(soul, TypeId::of::<Transform>()));
        assert!(has_component(soul, TypeId::of::<CommandedBy>()));
        assert!(has_component(familiar, TypeId::of::<Commanding>()));

        let body = dynamic_world.serialize(&registry).unwrap();
        let mut ron_deserializer = ron::de::Deserializer::from_str(&body).unwrap();
        let round_tripped = WorldDeserializer {
            type_registry: &registry,
            load_from_path: &mut NoAssetLoad,
        }
        .deserialize(&mut ron_deserializer)
        .unwrap();
        drop(registry);

        let mut destination = World::new();
        let mut entity_map = EntityHashMap::default();
        let registry = type_registry.read();
        round_tripped
            .write_to_world_with(&mut destination, &mut entity_map, &registry)
            .unwrap();

        let mapped_familiar = entity_map[&familiar];
        let mapped_soul = entity_map[&soul];
        assert_eq!(
            destination.get::<CommandedBy>(mapped_soul).unwrap().0,
            mapped_familiar
        );
        assert!(
            destination
                .get::<Commanding>(mapped_familiar)
                .unwrap()
                .iter()
                .any(|entity| *entity == mapped_soul)
        );
        assert_eq!(destination.resource::<GameTime>().seconds, 42.0);
    }

    #[test]
    fn gathering_relationships_are_excluded_from_new_saves_and_stripped_from_legacy_bodies() {
        let mut app = App::new();
        register_save_types(&mut app);

        let (soul, gathering_spot) = {
            let world = app.world_mut();
            let gathering_spot = world.spawn(GatheringParticipants::default()).id();
            let soul = world
                .spawn((DamnedSoul::default(), ParticipatingIn(gathering_spot)))
                .id();
            (soul, gathering_spot)
        };

        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let dynamic_world = build_persisted_world(app.world(), &registry, std::iter::once(soul));
        let saved_soul = dynamic_world
            .entities
            .iter()
            .find(|entity| entity.entity == soul)
            .expect("soul root must be extracted");
        assert!(saved_soul.components.iter().all(|component| {
            component
                .get_represented_type_info()
                .is_none_or(|info| info.type_id() != TypeId::of::<ParticipatingIn>())
        }));

        let mut legacy_body = DynamicWorld {
            resources: Vec::new(),
            entities: vec![bevy_world_serialization::DynamicEntity {
                entity: soul,
                components: vec![
                    Box::new(DamnedSoul::default()),
                    Box::new(ParticipatingIn(gathering_spot)),
                    Box::new(GatheringParticipants::default()),
                ],
            }],
        };
        discard_runtime_derived_components(&mut legacy_body);

        assert_eq!(legacy_body.entities[0].components.len(), 1);
        assert_eq!(
            legacy_body.entities[0].components[0]
                .get_represented_type_info()
                .unwrap()
                .type_id(),
            TypeId::of::<DamnedSoul>()
        );
    }

    #[test]
    fn reserved_for_task_is_loader_registered_but_excluded_from_v1_schema() {
        let mut app = App::new();
        register_save_types(&mut app);
        let item = app
            .world_mut()
            .spawn((ResourceItem(ResourceType::Wood), ReservedForTask))
            .id();

        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        assert_component_registration::<ReservedForTask>(&registry);

        let persisted = build_persisted_world(app.world(), &registry, std::iter::once(item));
        let saved_item = persisted
            .entities
            .iter()
            .find(|entity| entity.entity == item)
            .expect("resource item root must be extracted");
        assert!(saved_item.components.iter().all(|component| {
            component
                .get_represented_type_info()
                .is_none_or(|info| info.type_id() != TypeId::of::<ReservedForTask>())
        }));
        let body = persisted.serialize(&registry).unwrap();
        assert!(!body.contains(ReservedForTask::type_path()));
        drop(registry);

        let mut legacy_body = DynamicWorld {
            resources: Vec::new(),
            entities: vec![bevy_world_serialization::DynamicEntity {
                entity: item,
                components: vec![
                    Box::new(ResourceItem(ResourceType::Wood)),
                    Box::new(ReservedForTask),
                ],
            }],
        };
        let error = validate_persisted_world(&legacy_body).unwrap_err();
        assert!(
            error
                .unsupported_components
                .contains(&ReservedForTask::type_path().to_string())
        );

        discard_legacy_reserved_for_task(&mut legacy_body);
        let error = validate_persisted_world(&legacy_body).unwrap_err();
        assert!(error.unsupported_components.is_empty());
    }

    #[test]
    fn persisted_world_requires_every_schema_resource() {
        let mut app = App::new();
        register_save_types(&mut app);
        app.world_mut().insert_resource(GameTime::default());
        app.world_mut().insert_resource(DreamPool::default());
        app.world_mut()
            .insert_resource(PopulationManager::default());
        app.world_mut().insert_resource(WorldMap::default());

        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let dynamic_world = build_persisted_world(app.world(), &registry, std::iter::empty());

        assert!(validate_persisted_world(&dynamic_world).is_ok());
        assert_eq!(
            validate_persisted_world(&DynamicWorld::default())
                .unwrap_err()
                .missing_resources,
            vec![
                std::any::type_name::<GameTime>(),
                std::any::type_name::<DreamPool>(),
                std::any::type_name::<PopulationManager>(),
                std::any::type_name::<WorldMap>(),
            ]
        );
    }

    #[test]
    fn persisted_world_rejects_types_outside_the_schema_allow_lists() {
        let dynamic_world = DynamicWorld {
            resources: vec![Box::new(RuntimeOnlyType)],
            entities: vec![bevy_world_serialization::DynamicEntity {
                entity: Entity::PLACEHOLDER,
                components: vec![Box::new(RuntimeOnlyType)],
            }],
        };

        let error = validate_persisted_world(&dynamic_world).unwrap_err();
        assert_eq!(error.unsupported_resources.len(), 1);
        assert_eq!(error.unsupported_components.len(), 1);
    }

    #[test]
    fn persisted_world_rejects_allowed_components_without_a_root_marker() {
        let dynamic_world = DynamicWorld {
            resources: Vec::new(),
            entities: vec![bevy_world_serialization::DynamicEntity {
                entity: Entity::PLACEHOLDER,
                components: vec![Box::new(Transform::default())],
            }],
        };

        let error = validate_persisted_world(&dynamic_world).unwrap_err();
        assert!(error.unsupported_components.is_empty());
        assert_eq!(error.rootless_entities, vec![Entity::PLACEHOLDER]);
    }
}
