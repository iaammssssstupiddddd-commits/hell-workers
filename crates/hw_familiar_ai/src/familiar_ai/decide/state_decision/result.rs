use bevy::prelude::*;
use hw_core::events::{
    FamiliarAiStateChangedEvent, FamiliarIdleVisualRequest, FamiliarStateRequest, ReleaseReason,
    SquadManagementOperation, SquadManagementRequest,
};
use hw_core::familiar::FamiliarAiState;

use super::super::FamiliarDecideOutput;
use crate::familiar_ai::perceive::state_detection::determine_transition_reason;

// ──────────────────────────────────────────────────────────────────────────────
// 結果型
// ──────────────────────────────────────────────────────────────────────────────

/// per-familiar state decision の実行結果
///
/// root adapter がこれを受け取り、以下の順序で MessageWriter へ変換する：
///   1. `squad_release` → `SquadManagementRequest::ReleaseMember` (Fatigued)
///   2. `squad_add`     → `SquadManagementRequest::AddMember`
///   3. `emit_idle_visual` → `FamiliarIdleVisualRequest`
///   4. `state_changed` → `FamiliarStateRequest` + `FamiliarAiStateChangedEvent`
pub struct FamiliarStateDecisionResult {
    /// AddMember 対象（Some = root が AddMember request を発行）
    pub squad_add: Option<Entity>,
    /// ReleaseMember 対象一覧（ReleaseReason::Fatigued）
    pub squad_release: Vec<Entity>,
    /// state が変化したか（true = StateRequest + StateChangedEvent を発行）
    pub state_changed: bool,
    /// FamiliarIdleVisualRequest を発行すべきか
    pub emit_idle_visual: bool,
}

impl FamiliarStateDecisionResult {
    /// 変化なしの結果
    pub fn no_change() -> Self {
        Self {
            squad_add: None,
            squad_release: Vec::new(),
            state_changed: false,
            emit_idle_visual: false,
        }
    }

    /// Idle path: Scouting 継続の結果
    ///
    /// `recruited` = Some(Entity) なら AddMember request を追加する。
    /// `transition_applied` = true なら StateRequest を発行する。
    pub fn from_idle_scouting(recruited: Option<Entity>, transition_applied: bool) -> Self {
        Self {
            squad_add: recruited,
            squad_release: Vec::new(),
            state_changed: transition_applied,
            emit_idle_visual: false,
        }
    }

    /// Idle path: recruitment（即時リクルートまたはスカウト開始）の結果
    pub fn from_idle_recruitment(recruited: Option<Entity>, state_changed: bool) -> Self {
        Self {
            squad_add: recruited,
            squad_release: Vec::new(),
            state_changed,
            emit_idle_visual: false,
        }
    }

    /// Idle path: 分隊満員 → Idle 遷移の結果
    pub fn from_idle_squad_full(transition_applied: bool) -> Self {
        Self {
            squad_add: None,
            squad_release: Vec::new(),
            state_changed: transition_applied,
            emit_idle_visual: transition_applied,
        }
    }

    /// 非 Idle path の結果（squad 管理 + scouting / recruitment + finalize を集約）
    pub fn from_non_idle(
        squad_release: Vec<Entity>,
        recruited: Option<Entity>,
        state_changed: bool,
    ) -> Self {
        Self {
            squad_add: recruited,
            squad_release,
            state_changed,
            emit_idle_visual: false,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Message emission helpers
// ──────────────────────────────────────────────────────────────────────────────

fn write_add_member_request(
    writer: &mut MessageWriter<'_, SquadManagementRequest>,
    familiar_entity: Entity,
    soul_entity: Entity,
) {
    writer.write(SquadManagementRequest {
        familiar_entity,
        operation: SquadManagementOperation::AddMember { soul_entity },
    });
}

fn write_release_requests(
    writer: &mut MessageWriter<'_, SquadManagementRequest>,
    familiar_entity: Entity,
    released_entities: &[Entity],
) {
    for &soul_entity in released_entities {
        writer.write(SquadManagementRequest {
            familiar_entity,
            operation: SquadManagementOperation::ReleaseMember {
                soul_entity,
                reason: ReleaseReason::Fatigued,
            },
        });
    }
}

/// `FamiliarStateDecisionResult` を `FamiliarDecideOutput` の各 MessageWriter に変換する
///
/// 発行順序（パス共通）:
///   1. squad_release → ReleaseMember requests
///   2. squad_add     → AddMember request
///   3. emit_idle_visual → FamiliarIdleVisualRequest
///   4. state_changed → FamiliarStateRequest + FamiliarAiStateChangedEvent
pub(super) fn emit_state_decision_messages(
    fam_entity: Entity,
    old_state: &FamiliarAiState,
    next_state: &FamiliarAiState,
    result: &FamiliarStateDecisionResult,
    decide_output: &mut FamiliarDecideOutput,
) {
    write_release_requests(
        &mut decide_output.squad_requests,
        fam_entity,
        &result.squad_release,
    );
    if let Some(soul_entity) = result.squad_add {
        write_add_member_request(&mut decide_output.squad_requests, fam_entity, soul_entity);
    }
    if result.emit_idle_visual {
        decide_output
            .idle_visual_requests
            .write(FamiliarIdleVisualRequest {
                familiar_entity: fam_entity,
            });
    }
    if result.state_changed {
        decide_output.state_requests.write(FamiliarStateRequest {
            familiar_entity: fam_entity,
            new_state: next_state.clone(),
        });
        decide_output
            .state_changed_events
            .write(FamiliarAiStateChangedEvent {
                familiar_entity: fam_entity,
                from: old_state.clone(),
                to: next_state.clone(),
                reason: determine_transition_reason(old_state, next_state),
            });
    }
}
