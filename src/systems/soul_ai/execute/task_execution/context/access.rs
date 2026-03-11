use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::events::ResourceReservationRequest;
use crate::relationships::{ManagedBy, TaskWorkers};
use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache;
use crate::systems::jobs::{Blueprint, Designation, Priority, TaskSlots};
use crate::systems::logistics::Stockpile;

/// リソース予約・管理に必要な共通アクセス
#[derive(SystemParam)]
pub struct ReservationAccess<'w, 's> {
    pub resources: Query<'w, 's, &'static crate::systems::logistics::ResourceItem>,
    pub resource_cache: Res<'w, SharedResourceCache>,
    pub reservation_writer: MessageWriter<'w, ResourceReservationRequest>,
    pub incoming_deliveries_query:
        Query<'w, 's, (Entity, &'static crate::relationships::IncomingDeliveries)>,
}

/// 指定・場所・属性確認に必要な共通アクセス
#[derive(SystemParam)]
pub struct DesignationAccess<'w, 's> {
    pub targets: Query<
        'w,
        's,
        (
            &'static Transform,
            Option<&'static crate::systems::jobs::Tree>,
            Option<&'static crate::systems::jobs::TreeVariant>,
            Option<&'static crate::systems::jobs::Rock>,
            Option<&'static crate::systems::logistics::ResourceItem>,
            Option<&'static Designation>,
            Option<&'static crate::relationships::StoredIn>,
        ),
    >,
    pub designations: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Designation,
            Option<&'static ManagedBy>,
            Option<&'static TaskSlots>,
            Option<&'static TaskWorkers>,
            Option<&'static crate::relationships::StoredIn>,
            Option<&'static Priority>,
        ),
    >,
    pub belongs: Query<'w, 's, &'static crate::systems::logistics::BelongsTo>,
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
            Option<&'static crate::relationships::StoredItems>,
        ),
    >,
    pub loaded_in: Query<'w, 's, &'static crate::relationships::LoadedIn>,
    pub loaded_items: Query<'w, 's, &'static crate::relationships::LoadedItems>,
    pub bucket_storages: Query<'w, 's, (), With<crate::systems::logistics::BucketStorage>>,
    pub blueprints: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static Blueprint,
            Option<&'static Designation>,
        ),
    >,
    pub target_blueprints: Query<'w, 's, &'static crate::systems::jobs::TargetBlueprint>,
    pub mixers: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::systems::jobs::MudMixerStorage,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub target_mixers: Query<'w, 's, &'static crate::systems::jobs::TargetMixer>,
    pub floor_tiles:
        Query<'w, 's, &'static crate::systems::jobs::floor_construction::FloorTileBlueprint>,
    pub wall_tiles:
        Query<'w, 's, &'static crate::systems::jobs::wall_construction::WallTileBlueprint>,
    pub buildings: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::systems::jobs::Building,
            Option<&'static crate::systems::jobs::ProvisionalWall>,
        ),
    >,
}

/// 建設サイトへの読み取り専用アクセス（root bridge 専用）
#[derive(SystemParam)]
pub struct ConstructionSiteAccess<'w, 's> {
    pub floor_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::systems::jobs::floor_construction::FloorConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub wall_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::systems::jobs::wall_construction::WallConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
}

impl hw_ai::familiar_ai::decide::task_management::ConstructionSitePositions
    for ConstructionSiteAccess<'_, '_>
{
    fn floor_site_pos(&self, site: Entity) -> Option<Vec2> {
        self.floor_sites
            .get(site)
            .ok()
            .map(|(t, _, _)| t.translation.truncate())
    }

    fn wall_site_pos(&self, site: Entity) -> Option<Vec2> {
        self.wall_sites
            .get(site)
            .ok()
            .map(|(t, _, _)| t.translation.truncate())
    }
}

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
            Option<&'static crate::relationships::StoredItems>,
        ),
    >,
    pub loaded_in: Query<'w, 's, &'static crate::relationships::LoadedIn>,
    pub loaded_items: Query<'w, 's, &'static crate::relationships::LoadedItems>,
    pub bucket_storages: Query<'w, 's, (), With<crate::systems::logistics::BucketStorage>>,
    pub blueprints: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static Blueprint,
            Option<&'static Designation>,
        ),
    >,
    pub target_blueprints: Query<'w, 's, &'static crate::systems::jobs::TargetBlueprint>,
    pub mixers: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::systems::jobs::MudMixerStorage,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub target_mixers: Query<'w, 's, &'static crate::systems::jobs::TargetMixer>,
    pub floor_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::systems::jobs::floor_construction::FloorConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub floor_tiles:
        Query<'w, 's, &'static crate::systems::jobs::floor_construction::FloorTileBlueprint>,
    pub wall_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::systems::jobs::wall_construction::WallConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub wall_tiles:
        Query<'w, 's, &'static crate::systems::jobs::wall_construction::WallTileBlueprint>,
    pub buildings: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static crate::systems::jobs::Building,
            Option<&'static crate::systems::jobs::ProvisionalWall>,
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
            Option<&'static crate::relationships::StoredItems>,
        ),
    >,
    pub loaded_in: Query<'w, 's, &'static crate::relationships::LoadedIn>,
    pub loaded_items: Query<'w, 's, &'static crate::relationships::LoadedItems>,
    pub bucket_storages: Query<'w, 's, (), With<crate::systems::logistics::BucketStorage>>,
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
            &'static mut crate::systems::jobs::MudMixerStorage,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub target_mixers: Query<'w, 's, &'static crate::systems::jobs::TargetMixer>,
    pub floor_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut crate::systems::jobs::floor_construction::FloorConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub floor_tiles:
        Query<'w, 's, &'static mut crate::systems::jobs::floor_construction::FloorTileBlueprint>,
    pub wall_sites: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut crate::systems::jobs::wall_construction::WallConstructionSite,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub wall_tiles:
        Query<'w, 's, &'static mut crate::systems::jobs::wall_construction::WallTileBlueprint>,
    pub buildings: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static mut crate::systems::jobs::Building,
            Option<&'static mut crate::systems::jobs::ProvisionalWall>,
        ),
    >,
}
