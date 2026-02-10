use super::{
    transport_request_anchor_cleanup_system, TransportRequestMetrics,
    transport_request_metrics_system,
};
use super::producer::{
    blueprint::blueprint_auto_haul_system, bucket::bucket_auto_haul_system,
    mixer::mud_mixer_auto_haul_system, tank_water_request::tank_water_request_system,
    task_area::task_area_auto_haul_system,
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
    /// Commands 適用
    Execute,
    /// timeout/retry/cleanup
    Maintain,
}

pub struct TransportRequestPlugin;

impl Plugin for TransportRequestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TransportRequestMetrics>()
            .init_resource::<super::ItemReservations>();

        // Perceive → Decide → Execute: FamiliarAi::Update の後、FamiliarAi::Decide の前
        app.configure_sets(
            Update,
            (
                TransportRequestSet::Perceive,
                TransportRequestSet::Decide,
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
                    mud_mixer_auto_haul_system,
                    tank_water_request_system,
                    task_area_auto_haul_system,
                )
                    .in_set(TransportRequestSet::Decide),
                transport_request_execute_placeholder.in_set(TransportRequestSet::Execute),
                transport_request_anchor_cleanup_system.in_set(TransportRequestSet::Maintain),
            ),
        );
    }
}


fn transport_request_execute_placeholder() {}
