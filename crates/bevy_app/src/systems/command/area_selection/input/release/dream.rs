use crate::app_contexts::TaskContext;
use crate::systems::command::TaskMode;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_ui::area_edit::AreaEditSession;

pub(super) fn handle_release_dream_planting(
    task_context: &mut TaskContext,
    world_pos: Vec2,
    start_pos: Vec2,
    area_edit_session: &mut AreaEditSession,
) {
    let end_pos = WorldMap::snap_to_grid_center(world_pos);
    let seed = area_edit_session
        .dream_planting_preview_seed
        .take()
        .unwrap_or_else(rand::random::<u64>);
    area_edit_session.pending_dream_planting = Some((start_pos, end_pos, seed));
    task_context.0 = TaskMode::DreamPlanting(None);
}
