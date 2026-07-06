//! セーブ/ロード対象の型を `AppTypeRegistry` に一括登録する。
//!
//! 既存のプラグイン各所に散らばる `register_type` とは別に、セーブ/ロードで
//! 必要な型はここに集約する（登録漏れ防止。plan §5.5 を参照）。
//! 既に他の場所で登録済みの型（`DamnedSoul` 等）を再度 `register_type` しても
//! 副作用はないため、重複登録を気にせずここでも並べている。

use bevy::prelude::*;

use crate::world::map::Tile;

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
use hw_core::GameTime;

use hw_jobs::construction::{
    FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
    WallConstructionPhase, WallConstructionSite, WallTileBlueprint, WallTileState,
};
use hw_jobs::mud_mixer::{MudMixerStorage, StoredByMixer, TargetMixer};
use hw_jobs::{
    Blueprint, BonePile, BridgeMarker, Building, BuildingType, Designation, Door,
    FlexibleMaterialRequirement, ObstaclePosition, Priority, ProvisionalWall, RestArea, Rock,
    SandPile, TargetBlueprint, TargetSoulSpaSite, TaskSlots, Tree, TreeVariant, WorkType,
};

use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerGenerator,
    PowerGrid, SoulSpaPhase, SoulSpaSite, SoulSpaTile, Unpowered, YardPowerGrid,
};

use hw_logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportPolicy,
    TransportPriority, TransportRequest, TransportRequestFixedSource, TransportRequestKind,
};
use hw_logistics::types::WheelbarrowParking;
use hw_logistics::zone::Stockpile;
use hw_logistics::{
    BelongsTo, Inventory, PendingBelongsToBlueprint, ReservedForTask, ResourceItem, Wheelbarrow,
};

use hw_world::{TerrainType, WorldMap};

/// セーブ/ロード対象になる全ての `Reflect` 型を型レジストリに登録する。
pub fn register_save_types(app: &mut App) {
    app
        // ---- Resources ----
        .register_type::<GameTime>()
        .register_type::<DreamPool>()
        .register_type::<PopulationManager>()
        .register_type::<WorldMap>()
        // ---- Soul ----
        .register_type::<DamnedSoul>()
        .register_type::<IdleState>()
        .register_type::<IdleBehavior>()
        .register_type::<GatheringBehavior>()
        .register_type::<DreamState>()
        .register_type::<DreamQuality>()
        .register_type::<StressBreakdown>()
        .register_type::<RestAreaCooldown>()
        .register_type::<DriftingState>()
        .register_type::<DriftPhase>()
        .register_type::<DriftEdge>()
        // ---- Familiar ----
        .register_type::<Familiar>()
        .register_type::<FamiliarType>()
        // ---- Relationships ----
        .register_type::<CommandedBy>()
        .register_type::<Commanding>()
        .register_type::<WorkingOn>()
        .register_type::<TaskWorkers>()
        .register_type::<ManagedBy>()
        .register_type::<ManagedTasks>()
        .register_type::<StoredIn>()
        .register_type::<StoredItems>()
        .register_type::<LoadedIn>()
        .register_type::<LoadedItems>()
        .register_type::<ParkedAt>()
        .register_type::<ParkedWheelbarrows>()
        .register_type::<PushedBy>()
        .register_type::<PushingWheelbarrow>()
        .register_type::<DeliveringTo>()
        .register_type::<IncomingDeliveries>()
        .register_type::<ParticipatingIn>()
        .register_type::<GatheringParticipants>()
        .register_type::<RestingIn>()
        .register_type::<RestAreaOccupants>()
        .register_type::<RestAreaReservedFor>()
        .register_type::<RestAreaReservations>()
        // ---- Task / Building / Area ----
        .register_type::<Designation>()
        .register_type::<WorkType>()
        .register_type::<Priority>()
        .register_type::<TaskSlots>()
        .register_type::<TaskArea>()
        .register_type::<AreaBounds>()
        .register_type::<Building>()
        .register_type::<BuildingType>()
        .register_type::<Door>()
        .register_type::<DoorState>()
        .register_type::<RestArea>()
        .register_type::<Blueprint>()
        .register_type::<FlexibleMaterialRequirement>()
        .register_type::<ProvisionalWall>()
        .register_type::<BridgeMarker>()
        .register_type::<SandPile>()
        .register_type::<BonePile>()
        .register_type::<TargetBlueprint>()
        .register_type::<TargetSoulSpaSite>()
        // ---- Construction ----
        .register_type::<FloorConstructionSite>()
        .register_type::<FloorConstructionPhase>()
        .register_type::<FloorTileBlueprint>()
        .register_type::<FloorTileState>()
        .register_type::<WallConstructionSite>()
        .register_type::<WallConstructionPhase>()
        .register_type::<WallTileBlueprint>()
        .register_type::<WallTileState>()
        // ---- Logistics ----
        .register_type::<ResourceType>()
        .register_type::<ResourceItem>()
        .register_type::<BelongsTo>()
        .register_type::<PendingBelongsToBlueprint>()
        .register_type::<ReservedForTask>()
        .register_type::<Inventory>()
        .register_type::<Wheelbarrow>()
        .register_type::<WheelbarrowParking>()
        .register_type::<Stockpile>()
        .register_type::<TransportRequest>()
        .register_type::<TransportRequestKind>()
        .register_type::<TransportPriority>()
        .register_type::<TransportRequestFixedSource>()
        .register_type::<ManualTransportRequest>()
        .register_type::<ManualHaulPinnedSource>()
        .register_type::<TransportDemand>()
        .register_type::<TransportPolicy>()
        .register_type::<MudMixerStorage>()
        .register_type::<TargetMixer>()
        .register_type::<StoredByMixer>()
        // ---- Energy ----
        .register_type::<PowerGrid>()
        .register_type::<PowerGenerator>()
        .register_type::<PowerConsumer>()
        .register_type::<Unpowered>()
        .register_type::<YardPowerGrid>()
        .register_type::<GeneratesFor>()
        .register_type::<GridGenerators>()
        .register_type::<ConsumesFrom>()
        .register_type::<GridConsumers>()
        .register_type::<SoulSpaSite>()
        .register_type::<SoulSpaPhase>()
        .register_type::<SoulSpaTile>()
        // ---- World ----
        .register_type::<TerrainType>()
        .register_type::<Tree>()
        .register_type::<TreeVariant>()
        .register_type::<Rock>()
        .register_type::<ObstaclePosition>()
        .register_type::<Tile>()
        .register_type::<hw_world::zones::Site>()
        .register_type::<hw_world::zones::Yard>()
        .register_type::<hw_world::zones::PairedSite>()
        .register_type::<hw_world::zones::PairedYard>()
        // ---- Identity / Meta ----
        .register_type::<crate::entities::damned_soul::SoulIdentity>()
        .register_type::<crate::entities::damned_soul::Gender>()
        .register_type::<super::state::SavedWorldgenSeed>();
}
