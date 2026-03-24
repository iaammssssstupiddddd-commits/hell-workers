use super::super::super::apply::apply_designation_in_area;
use super::super::super::queries::DesignationTargetQuery;
use super::super::transitions::reset_designation_mode;
use crate::app_contexts::TaskContext;
use crate::systems::command::{TaskArea, TaskMode};
use crate::world::map::WorldMap;
use bevy::prelude::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_release_designation(
    task_context: &mut TaskContext,
    selected_entity: Option<Entity>,
    world_pos: Vec2,
    start_pos: Vec2,
    mode: TaskMode,
    q_familiars: &Query<
        (
            &mut crate::entities::familiar::ActiveCommand,
            &mut crate::entities::damned_soul::Destination,
        ),
        With<crate::entities::familiar::Familiar>,
    >,
    q_targets: &DesignationTargetQuery,
    commands: &mut Commands,
) {
    let area = TaskArea::from_points(start_pos, WorldMap::snap_to_grid_edge(world_pos));
    let issued_by = selected_entity.filter(|entity| q_familiars.contains(*entity));
    apply_designation_in_area(commands, mode, &area, issued_by, q_targets);
    task_context.0 = reset_designation_mode(mode);
}
