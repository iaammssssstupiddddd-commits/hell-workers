//! タスクハンドラのトレイト定義

use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use bevy::prelude::*;
use hw_core::soul::StressBreakdown;
use hw_core::visual::SoulTaskHandles;
use hw_world::WorldMap;

/// タスクタイプごとの実行ロジックを表すトレイト
pub trait TaskHandler<T> {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: T,
        commands: &mut Commands,
        soul_handles: &SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        breakdown_opt: Option<&StressBreakdown>,
    );
}
