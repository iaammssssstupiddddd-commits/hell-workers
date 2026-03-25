//! エリア選択で使う Query 型の共通定義

use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::systems::jobs::wall_construction::WallTileBlueprint;
use crate::systems::jobs::{Blueprint, Designation, Rock, Tree};
use crate::systems::logistics::transport_request::{
    ManualTransportRequest, TransportRequest, TransportRequestFixedSource,
};
use crate::systems::logistics::{BelongsTo, BucketStorage, ResourceItem, Stockpile};
use bevy::prelude::*;
use hw_core::relationships::{StoredItems, TaskWorkers};

/// apply.rs と input.rs で共有する designation target 用 Query の型
pub type DesignationTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static Tree>,
        Option<&'static Rock>,
        Option<&'static ResourceItem>,
        Option<&'static Designation>,
        Option<&'static TaskWorkers>,
        Option<&'static Blueprint>,
        Option<&'static BelongsTo>,
        Option<&'static TransportRequest>,
        Option<&'static TransportRequestFixedSource>,
        Option<&'static Stockpile>,
        Option<&'static StoredItems>,
        Option<&'static BucketStorage>,
        Option<&'static ManualTransportRequest>,
    ),
>;

pub type FloorTileBlueprintQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform, &'static FloorTileBlueprint)>;

pub type WallTileBlueprintQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform, &'static WallTileBlueprint)>;

pub type UnassignedDesignationQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, &'static Designation),
    Without<hw_core::relationships::ManagedBy>,
>;
