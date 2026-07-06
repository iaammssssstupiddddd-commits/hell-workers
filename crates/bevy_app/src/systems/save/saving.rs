//! ワールドのセーブ（exclusive system）。
//!
//! `DynamicWorldBuilder` を allow-list（`register_save_types` で登録した型のみ許可）
//! で構築し、RON にシリアライズしてファイルへ書き込む。
//!
//! # 設計上の逸脱（plan からの変更点）
//! plan の Phase A は「セーブ前にライブワールドを正規化する」（例: `AssignedTask` を
//! `None` にリセットする）ことを想定していたが、本実装ではその代わりに
//! **allow-list に含めない**（`AssignedTask` 等のタスク実行中状態を deny）方式を
//! 採用した。ライブゲームの状態を一切変更せずに済み、`unassign_task` の呼び出しに
//! 伴う予約解放処理を経由する必要もない。詳細は `docs/save_load.md` を参照。

use std::time::Instant;

use bevy::ecs::reflect::AppTypeRegistry;
use bevy::prelude::*;
use std::io::Write;

use crate::world::map::Tile;

use hw_core::area::TaskArea;
use hw_core::familiar::Familiar;
use hw_core::population::PopulationManager;
use hw_core::relationships::{
    CommandedBy, Commanding, DeliveringTo, GatheringParticipants, IncomingDeliveries, LoadedIn,
    LoadedItems, ManagedBy, ManagedTasks, ParkedAt, ParkedWheelbarrows, ParticipatingIn, PushedBy,
    PushingWheelbarrow, RestAreaOccupants, RestAreaReservations, RestAreaReservedFor, RestingIn,
    StoredIn, StoredItems, TaskWorkers, WorkingOn,
};
use hw_core::soul::{
    DamnedSoul, DreamPool, DreamState, DriftingState, RestAreaCooldown, StressBreakdown,
};
use hw_core::GameTime;

use hw_jobs::construction::{
    FloorConstructionSite, FloorTileBlueprint, WallConstructionSite, WallTileBlueprint,
};
use hw_jobs::mud_mixer::{MudMixerStorage, StoredByMixer, TargetMixer};
use hw_jobs::{
    Blueprint, BonePile, BridgeMarker, Building, Designation, Door, ProvisionalWall, RestArea,
    Rock, SandPile, TargetBlueprint, TargetSoulSpaSite, TaskSlots, Tree, TreeVariant,
};

use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerGenerator,
    PowerGrid, SoulSpaSite, SoulSpaTile, Unpowered, YardPowerGrid,
};

use hw_logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportPolicy,
    TransportRequest, TransportRequestFixedSource,
};
use hw_logistics::types::WheelbarrowParking;
use hw_logistics::zone::Stockpile;
use hw_logistics::{
    BelongsTo, Inventory, PendingBelongsToBlueprint, ReservedForTask, ResourceItem, Wheelbarrow,
};

use hw_world::WorldMap;

use bevy_world_serialization::DynamicWorldBuilder;

use super::entities::collect_persisted_entities;
use super::state::SAVE_FILE_PATH;

pub fn save_world_system(world: &mut World) {
    let started = Instant::now();

    // Reset the trigger immediately so a failure below doesn't leave the
    // state stuck (which would block all future save/load requests).
    *world.resource_mut::<super::state::SaveLoadState>() = super::state::SaveLoadState::Idle;

    // セーブ時点の worldgen seed をリソースとして焼き込む（ロード時の照合用）。
    let master_seed = world
        .resource::<crate::world::map::GeneratedWorldLayoutResource>()
        .master_seed;
    world.insert_resource(super::state::SavedWorldgenSeed(master_seed));

    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();

    let target_entities = collect_persisted_entities(world);

    let dynamic_world = DynamicWorldBuilder::from_world(world, &registry)
        .deny_all_components()
        .deny_all_resources()
        // ---- Resources ----
        .allow_resource::<GameTime>()
        .allow_resource::<DreamPool>()
        .allow_resource::<PopulationManager>()
        .allow_resource::<WorldMap>()
        .allow_resource::<super::state::SavedWorldgenSeed>()
        // ---- Components ----
        .allow_component::<Transform>()
        .allow_component::<DamnedSoul>()
        .allow_component::<hw_core::soul::IdleState>()
        .allow_component::<DreamState>()
        .allow_component::<StressBreakdown>()
        .allow_component::<RestAreaCooldown>()
        .allow_component::<DriftingState>()
        .allow_component::<Familiar>()
        .allow_component::<CommandedBy>()
        .allow_component::<Commanding>()
        .allow_component::<WorkingOn>()
        .allow_component::<TaskWorkers>()
        .allow_component::<ManagedBy>()
        .allow_component::<ManagedTasks>()
        .allow_component::<StoredIn>()
        .allow_component::<StoredItems>()
        .allow_component::<LoadedIn>()
        .allow_component::<LoadedItems>()
        .allow_component::<ParkedAt>()
        .allow_component::<ParkedWheelbarrows>()
        .allow_component::<PushedBy>()
        .allow_component::<PushingWheelbarrow>()
        .allow_component::<DeliveringTo>()
        .allow_component::<IncomingDeliveries>()
        .allow_component::<ParticipatingIn>()
        .allow_component::<GatheringParticipants>()
        .allow_component::<RestingIn>()
        .allow_component::<RestAreaOccupants>()
        .allow_component::<RestAreaReservedFor>()
        .allow_component::<RestAreaReservations>()
        .allow_component::<Designation>()
        .allow_component::<hw_jobs::Priority>()
        .allow_component::<TaskSlots>()
        .allow_component::<TaskArea>()
        .allow_component::<Building>()
        .allow_component::<Door>()
        .allow_component::<RestArea>()
        .allow_component::<Blueprint>()
        .allow_component::<ProvisionalWall>()
        .allow_component::<BridgeMarker>()
        .allow_component::<SandPile>()
        .allow_component::<BonePile>()
        .allow_component::<TargetBlueprint>()
        .allow_component::<TargetSoulSpaSite>()
        .allow_component::<FloorConstructionSite>()
        .allow_component::<FloorTileBlueprint>()
        .allow_component::<WallConstructionSite>()
        .allow_component::<WallTileBlueprint>()
        .allow_component::<ResourceItem>()
        .allow_component::<BelongsTo>()
        .allow_component::<PendingBelongsToBlueprint>()
        .allow_component::<ReservedForTask>()
        .allow_component::<Inventory>()
        .allow_component::<Wheelbarrow>()
        .allow_component::<WheelbarrowParking>()
        .allow_component::<Stockpile>()
        .allow_component::<TransportRequest>()
        .allow_component::<TransportRequestFixedSource>()
        .allow_component::<ManualTransportRequest>()
        .allow_component::<ManualHaulPinnedSource>()
        .allow_component::<TransportDemand>()
        .allow_component::<TransportPolicy>()
        .allow_component::<MudMixerStorage>()
        .allow_component::<TargetMixer>()
        .allow_component::<StoredByMixer>()
        .allow_component::<PowerGrid>()
        .allow_component::<PowerGenerator>()
        .allow_component::<PowerConsumer>()
        .allow_component::<Unpowered>()
        .allow_component::<YardPowerGrid>()
        .allow_component::<GeneratesFor>()
        .allow_component::<GridGenerators>()
        .allow_component::<ConsumesFrom>()
        .allow_component::<GridConsumers>()
        .allow_component::<SoulSpaSite>()
        .allow_component::<SoulSpaTile>()
        .allow_component::<Tree>()
        .allow_component::<TreeVariant>()
        .allow_component::<Rock>()
        .allow_component::<hw_jobs::ObstaclePosition>()
        .allow_component::<Tile>()
        .allow_component::<crate::entities::damned_soul::SoulIdentity>()
        .allow_component::<hw_world::zones::Site>()
        .allow_component::<hw_world::zones::Yard>()
        .allow_component::<hw_world::zones::PairedSite>()
        .allow_component::<hw_world::zones::PairedYard>()
        .extract_entities(target_entities.into_iter())
        .remove_empty_entities()
        .extract_resources()
        .build();

    let ron_string = match dynamic_world.serialize(&registry) {
        Ok(s) => s,
        Err(err) => {
            error!("Failed to serialize world for save: {err}");
            return;
        }
    };

    if let Err(err) = write_save_file(&ron_string) {
        error!("Failed to write save file {SAVE_FILE_PATH}: {err}");
        return;
    }

    let elapsed = started.elapsed();
    if elapsed.as_millis() > 100 {
        warn!("Save took {elapsed:?} (>100ms)");
    } else {
        info!("World saved to {SAVE_FILE_PATH} in {elapsed:?}");
    }
}

/// アトミック書き込み: `.tmp` に書き出してから rename する。
/// 途中でクラッシュしても既存のセーブファイルは破損しない。
fn write_save_file(contents: &str) -> std::io::Result<()> {
    let path = std::path::Path::new(SAVE_FILE_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_file_name(format!(
        "{}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("world.scn.ron")
    ));
    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}
