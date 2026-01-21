use crate::entities::damned_soul::{Destination, Path};
use crate::systems::command::TaskArea;
use bevy::prelude::*;

/// 探索（SearchingTask）状態のロジック
/// 
/// 注意: この関数は `state_handlers::searching::handle_searching_task_state` に移行されました。
/// Phase 1 のリファクタリング完了後、このファイルは削除予定です。
#[allow(dead_code)]
pub fn searching_logic(
    fam_entity: Entity,
    fam_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
) {
    if let Some(area) = task_area_opt {
        let center = area.center();
        crate::systems::familiar_ai::supervising::move_to_center(
            fam_entity, fam_pos, center, fam_dest, fam_path,
        );
    }
}
