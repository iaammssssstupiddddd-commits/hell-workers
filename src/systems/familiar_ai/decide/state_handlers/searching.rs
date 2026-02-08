//! SearchingTask 状態のハンドラー
//!
//! タスクを探索している状態の処理を行います。

use super::StateTransitionResult;
use crate::entities::damned_soul::{Destination, Path};
use crate::systems::command::TaskArea;
use bevy::prelude::*;

/// SearchingTask 状態のハンドラー
///
/// # 引数
/// - `fam_entity`: 使い魔のエンティティ
/// - `fam_pos`: 使い魔の位置
/// - `task_area_opt`: タスクエリア（オプション）
/// - `fam_dest`: 目的地（変更可能）
/// - `fam_path`: パス（変更可能）
pub fn handle_searching_task_state(
    fam_entity: Entity,
    fam_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
) -> StateTransitionResult {
    if let Some(area) = task_area_opt {
        let center = area.center();
        crate::systems::familiar_ai::decide::supervising::move_to_center(
            fam_entity, fam_pos, center, fam_dest, fam_path,
        );
    }

    StateTransitionResult::Stay
}
