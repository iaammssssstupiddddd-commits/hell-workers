//! 使い魔 AI の状態判断 dispatch core + Bevy System
//!
//! `FamiliarCommand` / `FamiliarAiState` / 分隊人数から「どのサブ処理に進むか」を
//! pure function で決定する。
//!
//! # 設計原則
//! - `determine_decision_path` は hw_core 型のみを使う pure function。
//! - `FamiliarStateDecisionResult` は `familiar_ai_state_system` が MessageWriter に変換するデータ。
//! - lens 構築と MessageWriter 呼び出しは `familiar_ai_state_system` が担う。

use std::collections::HashSet;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::events::{
    FamiliarAiStateChangedEvent, FamiliarIdleVisualRequest, FamiliarStateRequest, ReleaseReason,
    SquadManagementOperation, SquadManagementRequest,
};
use hw_core::familiar::{FamiliarAiState, FamiliarCommand};
use hw_core::relationships::CommandedBy;
use hw_core::soul::{RestAreaCooldown, StressBreakdown};
use hw_spatial::SpatialGrid;

use super::{
    FamiliarDecideOutput,
    query_types::{FamiliarSoulQuery, FamiliarStateQuery},
};
use crate::familiar_ai::decide::{
    helpers::{
        FamiliarSquadContext, SquadManagementOutcome, finalize_state_transitions,
        process_squad_management,
    },
    recruitment::{FamiliarRecruitmentContext, RecruitmentOutcome, process_recruitment},
    scouting::FamiliarScoutingContext,
    state_handlers,
};
use crate::familiar_ai::perceive::state_detection::determine_transition_reason;
use hw_jobs::AssignedTask;

// ──────────────────────────────────────────────────────────────────────────────
// dispatch 判定
// ──────────────────────────────────────────────────────────────────────────────

/// 使い魔の状態判断パス
///
/// `determine_decision_path` が返す enum。
/// 各 variant が「どの lens を構築してどの hw_ai サブ関数を呼ぶか」を 1 対 1 で表す。
#[derive(Debug, Clone)]
pub enum FamiliarDecisionPath {
    /// Idle command + 招募必要 + 既に Scouting 中
    IdleScoutingContinue { target_soul: Entity },
    /// Idle command + 招募必要 + 非 Scouting（即時リクルートまたはスカウト開始）
    IdleRecruitSearch,
    /// Idle command + 分隊満員（招募不要）→ Idle 停止ロジックへ
    IdleSquadFull,
    /// 非 Idle command + 既に Scouting 中
    NonIdleScoutingContinue { target_soul: Entity },
    /// 非 Idle command + その他（SearchingTask / Supervising / etc.）
    NonIdleRecruitOrTransition,
}

/// 使い魔の状態判断パスを決定する（pure function）
///
/// # 引数
/// - `command`            : `ActiveCommand.command`
/// - `current_state`      : 現在の `FamiliarAiState`
/// - `max_workers`        : `FamiliarOperation.max_controlled_soul`
/// - `current_squad_count`: `Commanding` コンポーネントが持つメンバー数（なければ 0）
pub fn determine_decision_path(
    command: &FamiliarCommand,
    current_state: &FamiliarAiState,
    max_workers: usize,
    current_squad_count: usize,
) -> FamiliarDecisionPath {
    if matches!(command, FamiliarCommand::Idle) {
        let needs_recruitment = max_workers > 0 && current_squad_count < max_workers;
        if needs_recruitment {
            if let FamiliarAiState::Scouting { target_soul } = *current_state {
                FamiliarDecisionPath::IdleScoutingContinue { target_soul }
            } else {
                FamiliarDecisionPath::IdleRecruitSearch
            }
        } else {
            FamiliarDecisionPath::IdleSquadFull
        }
    } else if let FamiliarAiState::Scouting { target_soul } = *current_state {
        FamiliarDecisionPath::NonIdleScoutingContinue { target_soul }
    } else {
        FamiliarDecisionPath::NonIdleRecruitOrTransition
    }
}

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
// Bevy System
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
fn emit_state_decision_messages(
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

/// 使い魔AIの状態決定に必要なSystemParam
#[derive(SystemParam)]
pub struct FamiliarAiStateDecisionParams<'w, 's> {
    pub spatial_grid: Res<'w, SpatialGrid>,
    pub q_familiars: FamiliarStateQuery<'w, 's>,
    pub q_souls: FamiliarSoulQuery<'w, 's>,
    pub q_breakdown: Query<'w, 's, &'static StressBreakdown>,
    pub q_resting: Query<'w, 's, (), With<hw_core::relationships::RestingIn>>,
    pub q_rest_cooldown: Query<'w, 's, &'static RestAreaCooldown>,
    pub decide_output: FamiliarDecideOutput<'w>,
}

/// 使い魔AIの状態更新システム（Decide Phase）
pub fn familiar_ai_state_system(params: FamiliarAiStateDecisionParams) {
    let FamiliarAiStateDecisionParams {
        spatial_grid,
        mut q_familiars,
        mut q_souls,
        q_breakdown,
        q_resting,
        q_rest_cooldown,
        mut decide_output,
        ..
    } = params;

    // 同フレーム内での重複リクルートを防ぐ予約セット
    let mut recruitment_reservations: HashSet<Entity> = HashSet::new();

    for (
        fam_entity,
        fam_transform,
        familiar,
        familiar_op,
        active_command,
        ai_state,
        mut fam_dest,
        mut fam_path,
        task_area_opt,
        commanding,
    ) in q_familiars.iter_mut()
    {
        debug!(
            "FAM_AI: {:?} Processing. Command: {:?}, State: {:?}, Area: {}",
            fam_entity,
            active_command.command,
            *ai_state,
            task_area_opt.is_some()
        );

        let old_state = ai_state.clone();
        let mut next_state = old_state.clone();
        let max_workers = familiar_op.max_controlled_soul;
        let current_count = commanding.map(|c| c.len()).unwrap_or(0);

        let path = determine_decision_path(
            &active_command.command,
            &old_state,
            max_workers,
            current_count,
        );

        let result = match path {
            FamiliarDecisionPath::IdleScoutingContinue { target_soul } => {
                // スカウト継続: ターゲットを予約登録して他 familiar が横取りしないよう保護
                recruitment_reservations.insert(target_soul);
                let fam_pos = fam_transform.translation.truncate();
                let mut squad_entities: Vec<Entity> = commanding
                    .map(|c| c.iter().copied().collect())
                    .unwrap_or_default();
                let transition_result = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &Transform,
                        &hw_core::soul::DamnedSoul,
                        &AssignedTask,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarScoutingContext {
                        fam_entity,
                        fam_pos,
                        target_soul,
                        fatigue_threshold: familiar_op.fatigue_threshold,
                        max_workers,
                        squad: &mut squad_entities,
                        ai_state: &mut next_state,
                        fam_dest: &mut fam_dest,
                        fam_path: &mut fam_path,
                        q_souls: &q,
                        q_breakdown: &q_breakdown,
                    };
                    state_handlers::scouting::handle_scouting_state(&mut ctx)
                };
                let recruited = transition_result.recruited_entity;
                let transition_applied = transition_result.transition.apply_to(&mut next_state);
                FamiliarStateDecisionResult::from_idle_scouting(recruited, transition_applied)
            }

            FamiliarDecisionPath::IdleRecruitSearch => {
                // 未スカウト: 即時リクルートまたはスカウト開始
                let mut squad_entities: Vec<Entity> = commanding
                    .map(|c| c.iter().copied().collect())
                    .unwrap_or_default();
                let outcome = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &Transform,
                        &hw_core::soul::DamnedSoul,
                        &AssignedTask,
                        &hw_core::soul::IdleState,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarRecruitmentContext {
                        fam_entity,
                        fam_transform,
                        familiar,
                        familiar_op,
                        ai_state: &mut next_state,
                        fam_dest: &mut fam_dest,
                        fam_path: &mut fam_path,
                        squad_entities: &mut squad_entities,
                        max_workers,
                        task_area_opt,
                        spatial_grid: &*spatial_grid,
                        q_souls: &q,
                        q_breakdown: &q_breakdown,
                        q_resting: &q_resting,
                        q_cooldown: &q_rest_cooldown,
                        recruitment_reservations: &mut recruitment_reservations,
                    };
                    process_recruitment(&mut ctx)
                };
                match outcome {
                    RecruitmentOutcome::ImmediateRecruit(soul) => {
                        FamiliarStateDecisionResult::from_idle_recruitment(Some(soul), true)
                    }
                    RecruitmentOutcome::ScoutingStarted => {
                        FamiliarStateDecisionResult::from_idle_recruitment(None, true)
                    }
                    RecruitmentOutcome::NoRecruit => FamiliarStateDecisionResult::no_change(),
                }
            }

            FamiliarDecisionPath::IdleSquadFull => {
                // 分隊十分: Idle 停止ロジック
                let transition = state_handlers::idle::handle_idle_state(
                    active_command,
                    &next_state,
                    fam_transform.translation.truncate(),
                    &mut fam_dest,
                    &mut fam_path,
                );
                let applied = transition.apply_to(&mut next_state);
                FamiliarStateDecisionResult::from_idle_squad_full(applied)
            }

            FamiliarDecisionPath::NonIdleScoutingContinue { target_soul } => {
                let fam_pos = fam_transform.translation.truncate();
                let fatigue_threshold = familiar_op.fatigue_threshold;

                let SquadManagementOutcome {
                    mut squad_entities,
                    released_entities,
                } = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &hw_core::soul::DamnedSoul,
                        &hw_core::soul::IdleState,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarSquadContext {
                        fam_entity,
                        familiar_op,
                        commanding,
                        q_souls: &q,
                    };
                    process_squad_management(&mut ctx)
                };

                // スカウト中のターゲットを予約登録
                recruitment_reservations.insert(target_soul);
                let transition_result = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &Transform,
                        &hw_core::soul::DamnedSoul,
                        &AssignedTask,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarScoutingContext {
                        fam_entity,
                        fam_pos,
                        target_soul,
                        fatigue_threshold,
                        max_workers,
                        squad: &mut squad_entities,
                        ai_state: &mut next_state,
                        fam_dest: &mut fam_dest,
                        fam_path: &mut fam_path,
                        q_souls: &q,
                        q_breakdown: &q_breakdown,
                    };
                    state_handlers::scouting::handle_scouting_state(&mut ctx)
                };
                let recruited = transition_result.recruited_entity;
                let scout_changed = transition_result.transition.apply_to(&mut next_state);
                let finalized = finalize_state_transitions(
                    &mut next_state,
                    &squad_entities,
                    fam_entity,
                    max_workers,
                );
                FamiliarStateDecisionResult::from_non_idle(
                    released_entities,
                    recruited,
                    scout_changed || finalized,
                )
            }

            FamiliarDecisionPath::NonIdleRecruitOrTransition => {
                let fatigue_threshold = familiar_op.fatigue_threshold;
                let _ = fatigue_threshold;

                let SquadManagementOutcome {
                    mut squad_entities,
                    released_entities,
                } = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &hw_core::soul::DamnedSoul,
                        &hw_core::soul::IdleState,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarSquadContext {
                        fam_entity,
                        familiar_op,
                        commanding,
                        q_souls: &q,
                    };
                    process_squad_management(&mut ctx)
                };

                let outcome = {
                    let mut q_lens = q_souls.transmute_lens_filtered::<(
                        Entity,
                        &Transform,
                        &hw_core::soul::DamnedSoul,
                        &AssignedTask,
                        &hw_core::soul::IdleState,
                        Option<&CommandedBy>,
                    ), Without<hw_core::familiar::Familiar>>(
                    );
                    let q = q_lens.query();
                    let mut ctx = FamiliarRecruitmentContext {
                        fam_entity,
                        fam_transform,
                        familiar,
                        familiar_op,
                        ai_state: &mut next_state,
                        fam_dest: &mut fam_dest,
                        fam_path: &mut fam_path,
                        squad_entities: &mut squad_entities,
                        max_workers,
                        task_area_opt,
                        spatial_grid: &*spatial_grid,
                        q_souls: &q,
                        q_breakdown: &q_breakdown,
                        q_resting: &q_resting,
                        q_cooldown: &q_rest_cooldown,
                        recruitment_reservations: &mut recruitment_reservations,
                    };
                    process_recruitment(&mut ctx)
                };
                let (recruited, recruit_changed) = match outcome {
                    RecruitmentOutcome::ImmediateRecruit(e) => (Some(e), true),
                    RecruitmentOutcome::ScoutingStarted => (None, true),
                    RecruitmentOutcome::NoRecruit => (None, false),
                };
                let finalized = finalize_state_transitions(
                    &mut next_state,
                    &squad_entities,
                    fam_entity,
                    max_workers,
                );
                FamiliarStateDecisionResult::from_non_idle(
                    released_entities,
                    recruited,
                    recruit_changed || finalized,
                )
            }
        };

        emit_state_decision_messages(
            fam_entity,
            &old_state,
            &next_state,
            &result,
            &mut decide_output,
        );
    }
}
