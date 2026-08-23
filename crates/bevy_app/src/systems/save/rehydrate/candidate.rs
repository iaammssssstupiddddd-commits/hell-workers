//! Immutable domain checks for an isolated, entity-remapped load candidate.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, MUD_MIXER_CAPACITY, MUD_MIXER_MUD_CAPACITY};
use hw_core::familiar::{Familiar, FamiliarOperation};
use hw_core::jobs::WorkType;
use hw_core::relationships::{
    CommandedBy, Commanding, LoadedIn, LoadedItems, ManagedBy, ManagedTasks, ParkedAt,
    ParkedWheelbarrows, RestAreaOccupants, RestAreaReservations, RestAreaReservedFor, RestingIn,
    StoredIn, StoredItems,
};
use hw_core::soul::{DamnedSoul, DreamState, IdleBehavior, IdleState};
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerGenerator,
    PowerGrid, SoulSpaSite, SoulSpaTile, YardPowerGrid,
};
use hw_jobs::construction::{
    FloorConstructionPhase, FloorTileBlueprint, FloorTileState, TargetFloorConstructionSite,
    TargetWallConstructionSite, WallConstructionPhase, WallTileBlueprint, WallTileState,
};
use hw_jobs::mud_mixer::{MudMixerStorage, StoredByMixer, TargetMixer};
use hw_jobs::{
    Blueprint, Building, BuildingType, Designation, Door, FloorConstructionSite, ObstaclePosition,
    Priority, ProvisionalWall, RestArea, Rock, TargetBlueprint, TargetSoulSpaSite, TaskSlots, Tree,
    TreeVariant, WallConstructionSite,
};
use hw_logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportPolicy,
    TransportRequest, TransportRequestFixedSource, TransportRequestKind,
};
use hw_logistics::types::WheelbarrowParking;
use hw_logistics::{
    BelongsTo, BucketStorage, Inventory, PendingBelongsToBlueprint, ResourceItem, ResourceType,
    Stockpile, Wheelbarrow,
};
use hw_world::{PairedSite, PairedYard, Site, WorldMap, Yard};

use crate::world::map::Tile;

use super::obstacles::DurableNavigationView;

mod familiar;
mod logistics;
mod shell;
mod topology;

/// Validates every persisted Entity-bearing topology that can be decided in
/// the isolated, remapped candidate. Derived caches may be rebuilt later, but
/// no mutation phase is allowed to discover a missing or mistyped durable
/// endpoint after the live world has already been replaced.
pub(in crate::systems::save) fn validate_durable_topology_candidate(
    candidate: &World,
) -> Result<(), String> {
    topology::validate(candidate)
}

pub(in crate::systems::save) fn validate_familiar_candidate(
    candidate: &World,
) -> Result<(), String> {
    familiar::validate(candidate)
}

pub(in crate::systems::save) fn validate_task_logistics_candidate(
    candidate: &World,
) -> Result<(), String> {
    logistics::validate(candidate)
}

pub(super) fn validate_shell_candidate(candidate: &World) -> Result<(), String> {
    shell::validate(candidate)
}

#[cfg(test)]
mod tests;
