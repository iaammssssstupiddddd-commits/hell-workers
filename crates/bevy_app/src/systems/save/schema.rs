//! セーブ形式の型schema。
//!
//! 型を追加・削除するときは、このファイルの分類だけを更新する。各分類は
//! `register_save_types`、DynamicWorld の allow-list、root entity の収集に展開される。
//! runtime shell、cache、`AssignedTask`、`ObstacleSourceKind`、表示用 `ChildOf` 階層は
//! 意図的にこのschemaの外に置き、ロード後の再構築に委ねる。

use bevy::prelude::*;
use bevy::reflect::TypeRegistry;
use bevy_world_serialization::{DynamicWorld, DynamicWorldBuilder};
use std::collections::HashSet;

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
    FlexibleMaterialRequirement, ObstaclePosition, PlayerIssuedDesignation, Priority,
    ProvisionalWall, RestArea, Rock, SandPile, TargetBlueprint, TargetSoulSpaSite, TaskSlots, Tree,
    TreeVariant, WorkType,
};

use hw_logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportPolicy,
    TransportPriority, TransportRequest, TransportRequestFixedSource, TransportRequestKind,
};
use hw_logistics::types::{ReservedForTask, WheelbarrowParking};
use hw_logistics::zone::{Stockpile, StockpileAcceptance, StockpilePolicy};
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
        $callback!(PlayerIssuedDesignation);
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
        $callback!(StockpilePolicy);
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
        $callback!(StockpileAcceptance);
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

mod validation;

pub(super) use validation::{DynamicWorldSchemaError, validate_persisted_world};

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
#[path = "schema/tests.rs"]
mod tests;
