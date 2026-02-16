//! エリア選択で使う Query 型の共通定義

use crate::relationships::{StoredItems, TaskWorkers};
use crate::systems::jobs::{Blueprint, Designation, Rock, Tree};
use crate::systems::logistics::transport_request::{
    ManualTransportRequest, TransportRequest, TransportRequestFixedSource,
};
use crate::systems::logistics::{BelongsTo, BucketStorage, ResourceItem, Stockpile};
use bevy::prelude::*;

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
