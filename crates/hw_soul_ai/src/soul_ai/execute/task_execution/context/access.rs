use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use hw_core::events::ResourceReservationRequest;
use hw_core::relationships::{ManagedBy, TaskWorkers};
use hw_energy::SoulSpaSite;
use hw_jobs::{Blueprint, Designation, Priority, TaskSlots};
use hw_logistics::SharedResourceCache;
use hw_logistics::zone::Stockpile;

/// リソース予約・管理に必要な共通アクセス
#[derive(SystemParam)]
pub struct ReservationAccess<'w, 's> {
    pub resources: Query<'w, 's, &'static hw_logistics::types::ResourceItem>,
    pub resource_cache: Res<'w, SharedResourceCache>,
    pub reservation_writer: MessageWriter<'w, ResourceReservationRequest>,
    pub incoming_deliveries_query:
        Query<'w, 's, (Entity, &'static hw_core::relationships::IncomingDeliveries)>,
}

type DesignationTargetsQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        Option<&'static hw_jobs::Tree>,
        Option<&'static hw_jobs::TreeVariant>,
        Option<&'static hw_jobs::Rock>,
        Option<&'static hw_logistics::types::ResourceItem>,
        Option<&'static Designation>,
        Option<&'static hw_core::relationships::StoredIn>,
    ),
>;

type DesignationsAccessQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Designation,
        Option<&'static ManagedBy>,
        Option<&'static TaskSlots>,
        Option<&'static TaskWorkers>,
        Option<&'static hw_core::relationships::StoredIn>,
        Option<&'static Priority>,
    ),
>;

/// 指定・場所・属性確認に必要な共通アクセス
#[derive(SystemParam)]
pub struct DesignationAccess<'w, 's> {
    pub targets: DesignationTargetsQuery<'w, 's>,
    pub designations: DesignationsAccessQuery<'w, 's>,
    pub belongs: Query<'w, 's, &'static hw_logistics::types::BelongsTo>,
}

/// 倉庫・設備・ブループリントへの読み取り専用アクセス（Familiar AI向け・建設サイト除く）
#[derive(SystemParam)]
pub struct FamiliarStorageAccess<'w, 's> {
    pub stockpiles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Stockpile,
            Option<&'static hw_core::relationships::StoredItems>,
        ),
    >,
    pub loaded_in: Query<'w, 's, &'static hw_core::relationships::LoadedIn>,
    pub loaded_items: Query<'w, 's, &'static hw_core::relationships::LoadedItems>,
    pub bucket_storages: Query<'w, 's, (), With<hw_logistics::types::BucketStorage>>,
    pub blueprints: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static Blueprint,
            Option<&'static Designation>,
        ),
    >,
    pub target_blueprints: Query<'w, 's, &'static hw_jobs::TargetBlueprint>,
    pub mixers: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static hw_jobs::mud_mixer::MudMixerStorage,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub target_mixers: Query<'w, 's, &'static hw_jobs::mud_mixer::TargetMixer>,
    pub floor_tiles: Query<'w, 's, &'static hw_jobs::construction::FloorTileBlueprint>,
    pub wall_tiles: Query<'w, 's, &'static hw_jobs::construction::WallTileBlueprint>,
    pub buildings: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static hw_jobs::Building,
            Option<&'static hw_jobs::ProvisionalWall>,
        ),
    >,
}

/// 建設サイトへの読み取り専用アクセス（実装は `hw_jobs::ConstructionSiteAccess`）
pub use hw_jobs::ConstructionSiteAccess;

/// 倉庫・設備・ブループリントへの読み取り専用アクセス
#[derive(SystemParam)]
pub struct StorageAccess<'w, 's> {
    pub stockpiles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Stockpile,
            Option<&'static hw_core::relationships::StoredItems>,
        ),
    >,
    pub loaded_in: Query<'w, 's, &'static hw_core::relationships::LoadedIn>,
    pub loaded_items: Query<'w, 's, &'static hw_core::relationships::LoadedItems>,
    pub bucket_storages: Query<'w, 's, (), With<hw_logistics::types::BucketStorage>>,
    pub blueprints: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static Blueprint,
            Option<&'static Designation>,
        ),
    >,
    pub target_blueprints: Query<'w, 's, &'static hw_jobs::TargetBlueprint>,
    pub mixers: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static hw_jobs::mud_mixer::MudMixerStorage,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub target_mixers: Query<'w, 's, &'static hw_jobs::mud_mixer::TargetMixer>,
    pub floor_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static hw_jobs::construction::FloorConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub floor_tiles: Query<'w, 's, &'static hw_jobs::construction::FloorTileBlueprint>,
    pub wall_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static hw_jobs::construction::WallConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub wall_tiles: Query<'w, 's, &'static hw_jobs::construction::WallTileBlueprint>,
    pub buildings: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static hw_jobs::Building,
            Option<&'static hw_jobs::ProvisionalWall>,
        ),
    >,
}

/// 倉庫・設備・ブループリントへの変更可能アクセス
#[derive(SystemParam)]
pub struct MutStorageAccess<'w, 's> {
    pub stockpiles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static mut Stockpile,
            Option<&'static hw_core::relationships::StoredItems>,
        ),
    >,
    pub loaded_in: Query<'w, 's, &'static hw_core::relationships::LoadedIn>,
    pub loaded_items: Query<'w, 's, &'static hw_core::relationships::LoadedItems>,
    pub bucket_storages: Query<'w, 's, (), With<hw_logistics::types::BucketStorage>>,
    pub blueprints: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut Blueprint,
            Option<&'static Designation>,
        ),
    >,
    pub mixers: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut hw_jobs::mud_mixer::MudMixerStorage,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub target_mixers: Query<'w, 's, &'static hw_jobs::mud_mixer::TargetMixer>,
    pub floor_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut hw_jobs::construction::FloorConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub floor_tiles: Query<
        'w,
        's,
        (
            Entity,
            &'static mut hw_jobs::construction::FloorTileBlueprint,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub wall_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut hw_jobs::construction::WallConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub wall_tiles: Query<
        'w,
        's,
        (
            Entity,
            &'static mut hw_jobs::construction::WallTileBlueprint,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub buildings: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut hw_jobs::Building,
            Option<&'static mut hw_jobs::ProvisionalWall>,
        ),
    >,
    pub soul_spa_sites: Query<'w, 's, &'static Transform, With<SoulSpaSite>>,
}
