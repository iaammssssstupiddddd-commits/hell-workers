//! 使い魔 AI の状態判断 dispatch core
//!
//! `FamiliarCommand` / `FamiliarAiState` / 分隊人数から「どのサブ処理に進むか」を
//! pure function で決定する。Bevy Resource / MessageWriter に依存しない。
//!
//! # 設計原則
//! - `determine_decision_path` は hw_core 型のみを使う pure function。
//! - `FamiliarStateDecisionResult` は root adapter が MessageWriter に変換するデータ。
//! - lens 構築と MessageWriter 呼び出しは root `state_decision.rs` が担う。

use bevy::prelude::*;
use hw_core::familiar::{FamiliarAiState, FamiliarCommand};

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
