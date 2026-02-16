use super::producer::{
    blueprint::blueprint_auto_haul_system,
    bucket::bucket_auto_haul_system,
    consolidation::stockpile_consolidation_producer_system,
    floor_construction::{
        floor_construction_auto_haul_system, floor_material_delivery_sync_system,
        floor_tile_designation_system,
    },
    mixer::mud_mixer_auto_haul_system,
    tank_water_request::tank_water_request_system,
    task_area::task_area_auto_haul_system,
    wheelbarrow::wheelbarrow_auto_haul_system,
};
use super::state_machine::transport_request_state_sync_system;
use super::{
    TransportRequestMetrics, transport_request_anchor_cleanup_system,
    transport_request_metrics_system, wheelbarrow_arbitration_system,
};
use crate::systems::GameSystemSet;
use crate::systems::soul_ai::scheduling::{FamiliarAiSystemSet, SoulAiSystemSet};
use bevy::prelude::*;

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
    /// timeout/retry/cleanup
    Maintain,
}

pub struct TransportRequestPlugin;

impl Plugin for TransportRequestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TransportRequestMetrics>();

        // Perceive → Decide → Arbitrate → Execute: FamiliarAi::Update の後、FamiliarAi::Decide の前
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

        // Maintain: SoulAi::Execute の後
        app.configure_sets(
            Update,
            TransportRequestSet::Maintain
                .after(SoulAiSystemSet::Execute)
                .in_set(GameSystemSet::Logic),
        );

        // ApplyDeferred: Execute → FamiliarAi::Decide
        app.add_systems(
            Update,
            ApplyDeferred
                .after(TransportRequestSet::Execute)
                .before(FamiliarAiSystemSet::Decide),
        );

        app.add_systems(
            Update,
            (
                transport_request_metrics_system.in_set(TransportRequestSet::Perceive),
                (
                    blueprint_auto_haul_system,
                    bucket_auto_haul_system,
                    floor_construction_auto_haul_system,
                    floor_material_delivery_sync_system.after(floor_construction_auto_haul_system),
                    floor_tile_designation_system.after(floor_material_delivery_sync_system),
                    mud_mixer_auto_haul_system,
                    tank_water_request_system,
                    task_area_auto_haul_system,
                    wheelbarrow_auto_haul_system,
                    stockpile_consolidation_producer_system.after(task_area_auto_haul_system),
                )
                    .in_set(TransportRequestSet::Decide),
                wheelbarrow_arbitration_system.in_set(TransportRequestSet::Arbitrate),
                transport_request_state_sync_system.in_set(TransportRequestSet::Execute),
                transport_request_anchor_cleanup_system.in_set(TransportRequestSet::Maintain),
            ),
        );
    }
}
