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
    /// Idle command + 所属なし + 招募無効 → Idle 停止ロジックへ
    IdleWithoutSquad,
    /// Idle command + 既存分隊あり（追加募集後、既存メンバーを監視）
    IdleSquadRecruitOrTransition,
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
        if current_squad_count > 0 {
            if let FamiliarAiState::Scouting { target_soul } = *current_state {
                FamiliarDecisionPath::IdleScoutingContinue { target_soul }
            } else {
                FamiliarDecisionPath::IdleSquadRecruitOrTransition
            }
        } else if needs_recruitment {
            if let FamiliarAiState::Scouting { target_soul } = *current_state {
                FamiliarDecisionPath::IdleScoutingContinue { target_soul }
            } else {
                FamiliarDecisionPath::IdleRecruitSearch
            }
        } else {
            FamiliarDecisionPath::IdleWithoutSquad
        }
    } else if let FamiliarAiState::Scouting { target_soul } = *current_state {
        FamiliarDecisionPath::NonIdleScoutingContinue { target_soul }
    } else {
        FamiliarDecisionPath::NonIdleRecruitOrTransition
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_supervising_squad_still_uses_recruitment_path() {
        let path = determine_decision_path(
            &FamiliarCommand::GatherResources,
            &FamiliarAiState::Supervising {
                target: None,
                timer: 0.0,
            },
            2,
            1,
        );

        assert!(matches!(
            path,
            FamiliarDecisionPath::NonIdleRecruitOrTransition
        ));
    }

    #[test]
    fn idle_command_with_existing_squad_uses_recruitment_and_supervision_path() {
        let path = determine_decision_path(&FamiliarCommand::Idle, &FamiliarAiState::Idle, 2, 1);

        assert!(matches!(
            path,
            FamiliarDecisionPath::IdleSquadRecruitOrTransition
        ));
    }
}
