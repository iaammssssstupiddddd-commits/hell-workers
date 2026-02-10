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

        // プレースホルダーシステム（各セットに1つずつ）
        app.add_systems(
            Update,
            (
                transport_request_perceive_placeholder.in_set(TransportRequestSet::Perceive),
                transport_request_decide_placeholder.in_set(TransportRequestSet::Decide),
                transport_request_execute_placeholder.in_set(TransportRequestSet::Execute),
                transport_request_maintain_placeholder.in_set(TransportRequestSet::Maintain),
            ),
        );
    }
}

fn transport_request_perceive_placeholder() {}
fn transport_request_decide_placeholder() {}
fn transport_request_execute_placeholder() {}
fn transport_request_maintain_placeholder() {}
