//! タスクハンドラのトレイト定義

use hw_core::soul::StressBreakdown;
use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use hw_world::WorldMap;
use bevy::prelude::*;

/// タスクタイプごとの実行ロジックを表すトレイト
pub trait TaskHandler<T> {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: T,
        commands: &mut Commands,
        soul_handles: &hw_visual::SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        breakdown_opt: Option<&StressBreakdown>,
    );
}
