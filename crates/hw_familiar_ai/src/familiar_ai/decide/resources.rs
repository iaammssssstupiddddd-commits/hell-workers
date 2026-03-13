use bevy::prelude::*;
use hw_core::constants::FAMILIAR_TASK_DELEGATION_INTERVAL;
use std::collections::HashMap;

pub type ReachabilityCacheKey = ((i32, i32), (i32, i32));

#[derive(Resource, Default)]
pub struct ReachabilityFrameCache {
    pub cache: HashMap<ReachabilityCacheKey, bool>,
    pub age: u32,
}

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

/// Familiar task delegation の計測値（PERF-00）
#[derive(Resource, Debug)]
pub struct FamiliarDelegationPerfMetrics {
    /// 集計ログ出力までの経過秒
    pub log_interval_secs: f32,
    /// 直近フレームの委譲システム実行時間
    pub latest_elapsed_ms: f32,
    /// source_selector 呼び出し回数（期間集計）
    pub source_selector_calls: u32,
    /// source_selector のキャッシュ構築で走査したアイテム数（期間集計）
    pub source_selector_cache_build_scanned_items: u32,
    /// source_selector の候補探索で走査したアイテム数（期間集計）
    pub source_selector_candidate_scanned_items: u32,
    /// source_selector が走査したアイテム数（期間集計）
    pub source_selector_scanned_items: u32,
    /// reachable_with_cache 呼び出し回数（期間集計）
    pub reachable_with_cache_calls: u32,
    /// 委譲対象として処理した Familiar 数（期間集計）
    pub familiars_processed: u32,
}

impl Default for FamiliarDelegationPerfMetrics {
    fn default() -> Self {
        Self {
            log_interval_secs: 0.0,
            latest_elapsed_ms: 0.0,
            source_selector_calls: 0,
            source_selector_cache_build_scanned_items: 0,
            source_selector_candidate_scanned_items: 0,
            source_selector_scanned_items: 0,
            reachable_with_cache_calls: 0,
            familiars_processed: 0,
        }
    }
}
