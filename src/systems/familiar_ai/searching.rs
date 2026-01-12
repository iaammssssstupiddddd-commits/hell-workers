use crate::constants::*;
use crate::entities::damned_soul::{Destination, Path};
use crate::systems::command::TaskArea;
use bevy::prelude::*;

/// 探索（SearchingTask）状態のロジック
pub fn searching_logic(
    fam_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
) {
    if let Some(area) = task_area_opt {
        let center = (area.min + area.max) * 0.5;
        // 5タイル以上離れているなら中心へ
        if fam_pos.distance_squared(center) > (TILE_SIZE * 5.0).powi(2) {
            // すでに中心に向かっている、または到着しているなら更新しない
            let is_moving_to_center =
                fam_dest.0.distance_squared(center) < (TILE_SIZE * 0.5).powi(2);
            let is_path_finished = fam_path.current_index >= fam_path.waypoints.len();

            if is_path_finished || !is_moving_to_center {
                info!("FAM_AI: Moving to task area center at {:?}", center);
                fam_dest.0 = center;
                fam_path.waypoints = vec![center];
                fam_path.current_index = 0;
            }
        }
    }
}
