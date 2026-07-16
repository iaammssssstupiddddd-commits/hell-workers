use bevy::prelude::*;
use hw_core::constants::FAMILIAR_TASK_DELEGATION_INTERVAL;
use std::time::Duration;

#[derive(Resource)]
pub struct FamiliarTaskDelegationTimer {
    pub timer: Timer,
    pub first_run_done: bool,
}

impl Default for FamiliarTaskDelegationTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(FAMILIAR_TASK_DELEGATION_INTERVAL, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

impl FamiliarTaskDelegationTimer {
    /// 起動直後だけ即時に、それ以降は定義済みの周期で委譲を許可する。
    ///
    /// Familiar の移動・監視はこの gate の外で毎フレーム進める。Idle command
    /// だけを例外にすると Yard 共有タスクの候補探索まで毎フレーム走るため、
    /// Yard を候補集合へ含めることと委譲の周期はここで分離する。
    pub fn advance(&mut self, delta: Duration) -> bool {
        let is_first_cycle = !self.first_run_done;
        let timer_finished = self.timer.tick(delta).just_finished();
        self.first_run_done = true;
        is_first_cycle || timer_finished
    }
}

/// Expensive recruit/squad state decisions share the same 0.5 s cadence as
/// delegation. Scouting remains a render-frame path because it drives an
/// already-selected target's movement and handoff.
#[derive(Resource)]
pub struct FamiliarStateDecisionTimer {
    timer: Timer,
    first_run_done: bool,
}

impl Default for FamiliarStateDecisionTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(FAMILIAR_TASK_DELEGATION_INTERVAL, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

impl FamiliarStateDecisionTimer {
    pub fn advance(&mut self, delta: Duration) -> bool {
        let is_first_cycle = !self.first_run_done;
        let timer_finished = self.timer.tick(delta).just_finished();
        self.first_run_done = true;
        is_first_cycle || timer_finished
    }
}

/// Familiar task delegation の計測値（PERF-00）。
///
/// 通常ビルドのhot pathへカウンタ更新を残さないため、profiling feature時だけ登録する。
#[cfg(feature = "profiling")]
#[derive(Resource, Debug)]
pub struct FamiliarDelegationPerfMetrics {
    /// 直近フレームの委譲システム実行時間
    pub latest_elapsed_ms: f32,
    /// task delegation の実行 cycle 数（期間集計）
    pub delegation_cycles: u32,
    /// 共有 IncomingDeliverySnapshot の構築回数（期間集計）
    pub incoming_snapshot_builds: u32,
    /// source_selector 呼び出し回数（期間集計）
    pub source_selector_calls: u32,
    /// source_selector のキャッシュ構築で走査したアイテム数（期間集計）
    pub source_selector_cache_build_scanned_items: u32,
    /// source_selector の候補探索で走査したアイテム数（期間集計）
    pub source_selector_candidate_scanned_items: u32,
    /// source_selector が走査したアイテム数（期間集計）
    pub source_selector_scanned_items: u32,
    /// version付き連結成分cacheによる到達判定回数（期間集計）。
    ///
    /// CSV schema v4との互換のためフィールド名は維持する。
    pub reachable_with_cache_calls: u32,
    /// 委譲対象として処理した Familiar 数（期間集計）
    pub familiars_processed: u32,
}

#[cfg(feature = "profiling")]
impl Default for FamiliarDelegationPerfMetrics {
    fn default() -> Self {
        Self {
            latest_elapsed_ms: 0.0,
            delegation_cycles: 0,
            incoming_snapshot_builds: 0,
            source_selector_calls: 0,
            source_selector_cache_build_scanned_items: 0,
            source_selector_candidate_scanned_items: 0,
            source_selector_scanned_items: 0,
            reachable_with_cache_calls: 0,
            familiars_processed: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FamiliarStateDecisionTimer, FamiliarTaskDelegationTimer};
    use std::time::Duration;

    #[test]
    fn delegation_runs_immediately_then_only_after_the_interval() {
        let mut timer = FamiliarTaskDelegationTimer::default();

        assert!(timer.advance(Duration::ZERO));
        assert!(!timer.advance(Duration::from_millis(499)));
        assert!(timer.advance(Duration::from_millis(1)));
    }

    #[test]
    fn state_decision_uses_the_same_half_second_cadence() {
        let mut timer = FamiliarStateDecisionTimer::default();

        assert!(timer.advance(Duration::ZERO));
        assert!(!timer.advance(Duration::from_millis(499)));
        assert!(timer.advance(Duration::from_millis(1)));
    }
}
