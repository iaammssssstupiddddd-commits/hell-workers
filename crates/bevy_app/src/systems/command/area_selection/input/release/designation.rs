use super::super::super::apply::apply_designation_in_area;
use super::super::super::queries::DesignationTargetQuery;
use super::super::transitions::reset_designation_mode;
use crate::app_contexts::TaskContext;
use crate::systems::command::{TaskArea, TaskMode};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub(super) struct DesignationReleaseCtx {
    pub(super) selected_entity: Option<Entity>,
    pub(super) world_pos: Vec2,
    pub(super) start_pos: Vec2,
    pub(super) mode: TaskMode,
}

pub(super) fn handle_release_designation(
    task_context: &mut TaskContext,
    ctx: DesignationReleaseCtx,
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
    let area = TaskArea::from_points(ctx.start_pos, WorldMap::snap_to_grid_edge(ctx.world_pos));
    let issued_by = ctx
        .selected_entity
        .filter(|entity| q_familiars.contains(*entity));
    let summary = apply_designation_in_area(commands, ctx.mode, &area, issued_by, q_targets);
    let label = match ctx.mode {
        TaskMode::DesignateChop(_) => "伐採",
        TaskMode::DesignateMine(_) => "採掘",
        TaskMode::DesignateHaul(_) => "運搬",
        _ => return,
    };
    super::notify_order_result(
        commands,
        issued_by,
        label,
        format!(
            "対象 {}・適用 {}・受入先なし {}・担当なし {}",
            summary.matched, summary.applied, summary.no_destination, summary.no_issuer,
        ),
    );
    task_context.0 = reset_designation_mode(ctx.mode);
}
