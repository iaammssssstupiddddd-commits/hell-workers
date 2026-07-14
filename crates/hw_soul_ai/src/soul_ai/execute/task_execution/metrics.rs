//! task execution の計測用カウンタ。
//!
//! このモジュールは `profiling` feature 時だけコンパイルされる。通常ビルドの
//! task execution hot path には Resource 取得・カウンタ更新を残さない。

use bevy::prelude::*;

/// frame-time capture の計測区間で集計する task execution の作業量。
#[derive(Resource, Debug, Default)]
pub struct TaskExecutionPerfMetrics {
    /// `TaskExecutionSoulQuery` が返した Soul 数。
    pub souls_queried: u32,
    /// `AssignedTask::None` と判定され、context 構築前に除外した Soul 数。
    pub idle_skips: u32,
    /// task handler まで到達した Soul 数。
    pub handler_runs: u32,
}
