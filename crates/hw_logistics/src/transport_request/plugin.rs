//! TransportRequest サブシステムの Plugin 定義

use bevy::prelude::*;
use hw_core::system_sets::{FamiliarAiSystemSet, GameSystemSet, SoulAiSystemSet};

use super::producer::{
    active_unit_cache::{
        CachedActiveFamiliars, CachedActiveYards, CachedStockpileGroups,
        update_cached_active_familiars_system, update_cached_active_yards_system,
        update_cached_stockpile_groups_system,
    },
    blueprint::blueprint_auto_haul_system,
    bucket::bucket_auto_haul_system,
    consolidation::stockpile_consolidation_producer_system,
    floor_construction::{
        floor_construction_auto_haul_system, floor_material_delivery_sync_system,
        floor_tile_designation_system,
    },
    mixer::mud_mixer_auto_haul_system,
    provisional_wall::{
        provisional_wall_auto_haul_system, provisional_wall_designation_system,
        provisional_wall_material_delivery_sync_system,
    },
    tank_water_request::tank_water_request_system,
    task_area::task_area_auto_haul_system,
    tile_wait_cache::{
        FloorTileWaitingCache, WallTileWaitingCache, update_floor_tile_waiting_cache_system,
        update_wall_tile_waiting_cache_system,
    },
    wall_construction::{
        wall_construction_auto_haul_system, wall_material_delivery_sync_system,
        wall_tile_designation_system,
    },
    wheelbarrow::wheelbarrow_auto_haul_system,
};
use super::state_machine::{
    transport_request_state_sync_system, transport_request_task_workers_reconcile_system,
};
use super::{
    TransportRequestMetrics, transport_request_anchor_cleanup_system,
    transport_request_metrics_system, wheelbarrow_arbitration_system,
};

/// TransportRequest サブシステムの実行フェーズ
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TransportRequestSet {
    /// 需要観測
    Perceive,
    /// upsert/close 決定
    Decide,
    /// 手押し車仲裁（Decide → Execute 間）
    Arbitrate,
    /// Commands 適用
    Execute,
    /// Soul AI 実行後の relationship lifecycle 同期
    Reconcile,
    /// timeout/retry/cleanup
    Maintain,
}

pub struct TransportRequestPlugin;

impl Plugin for TransportRequestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TransportRequestMetrics>();
        app.init_resource::<FloorTileWaitingCache>();
        app.init_resource::<WallTileWaitingCache>();
        app.init_resource::<CachedActiveFamiliars>();
        app.init_resource::<CachedActiveYards>();
        app.init_resource::<CachedStockpileGroups>();

        app.configure_sets(
            Update,
            (
                TransportRequestSet::Perceive,
                TransportRequestSet::Decide,
                TransportRequestSet::Arbitrate,
                TransportRequestSet::Execute,
            )
                .chain()
                .after(FamiliarAiSystemSet::Update)
                .before(FamiliarAiSystemSet::Decide)
                .in_set(GameSystemSet::Logic),
        );

        app.configure_sets(
            Update,
            (
                TransportRequestSet::Reconcile,
                TransportRequestSet::Maintain,
            )
                .chain()
                .after(SoulAiSystemSet::Actor)
                .in_set(GameSystemSet::Actor),
        );

        app.add_systems(
            Update,
            ApplyDeferred
                .after(TransportRequestSet::Execute)
                .before(FamiliarAiSystemSet::Decide),
        );

        // Actor systems can remove WorkingOn after Logic has completed. Removing that source can
        // queue removal of its empty relationship target from the world's internal command queue.
        // Flush both stages before reading TaskWorkers removals.
        app.add_systems(
            Update,
            (ApplyDeferred, ApplyDeferred)
                .chain()
                .after(SoulAiSystemSet::Actor)
                .before(TransportRequestSet::Reconcile)
                .in_set(GameSystemSet::Actor),
        );

        app.add_systems(
            Update,
            (
                transport_request_metrics_system.in_set(TransportRequestSet::Perceive),
                update_floor_tile_waiting_cache_system.in_set(TransportRequestSet::Perceive),
                update_wall_tile_waiting_cache_system.in_set(TransportRequestSet::Perceive),
                update_cached_active_familiars_system.in_set(TransportRequestSet::Perceive),
                update_cached_active_yards_system.in_set(TransportRequestSet::Perceive),
                update_cached_stockpile_groups_system
                    .after(update_cached_active_yards_system)
                    .in_set(TransportRequestSet::Perceive),
                (
                    blueprint_auto_haul_system,
                    bucket_auto_haul_system,
                    floor_construction_auto_haul_system,
                    floor_material_delivery_sync_system.after(floor_construction_auto_haul_system),
                    floor_tile_designation_system.after(floor_material_delivery_sync_system),
                    provisional_wall_auto_haul_system,
                    provisional_wall_material_delivery_sync_system
                        .after(provisional_wall_auto_haul_system),
                    provisional_wall_designation_system
                        .after(provisional_wall_material_delivery_sync_system),
                    mud_mixer_auto_haul_system,
                    tank_water_request_system,
                    task_area_auto_haul_system,
                    wall_construction_auto_haul_system,
                    wall_material_delivery_sync_system.after(wall_construction_auto_haul_system),
                    wall_tile_designation_system.after(wall_material_delivery_sync_system),
                    wheelbarrow_auto_haul_system,
                    stockpile_consolidation_producer_system.after(task_area_auto_haul_system),
                )
                    .in_set(TransportRequestSet::Decide),
                wheelbarrow_arbitration_system.in_set(TransportRequestSet::Arbitrate),
                transport_request_state_sync_system.in_set(TransportRequestSet::Execute),
                transport_request_task_workers_reconcile_system
                    .in_set(TransportRequestSet::Reconcile),
                transport_request_anchor_cleanup_system.in_set(TransportRequestSet::Maintain),
            ),
        );
    }
}
