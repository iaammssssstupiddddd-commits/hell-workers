//! タスクハンドラのトレイト定義

use crate::assets::GameAssets;
use crate::entities::damned_soul::StressBreakdown;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// タスクタイプごとの実行ロジックを表すトレイト
pub trait TaskHandler<T> {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: T,
        commands: &mut Commands,
        game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        breakdown_opt: Option<&StressBreakdown>,
    );
}
