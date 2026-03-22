mod area;
mod cancel;
mod designation;
mod dream;

use area::{collect_indicator_entities, handle_release_area_selection};
use cancel::handle_release_cancel_designation;
use designation::handle_release_designation;
use dream::handle_release_dream_planting;

use super::super::queries::DesignationTargetQuery;
use super::super::{AreaEditHistory, AreaEditSession};
use crate::app_contexts::TaskContext;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar};
use crate::systems::command::{AreaSelectionIndicator, TaskArea, TaskMode};
use crate::systems::jobs::Designation;
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::systems::jobs::wall_construction::WallTileBlueprint;
use hw_world::zones::Site;
use bevy::prelude::*;
use hw_core::game_state::PlayMode;

pub(super) fn handle_left_just_released_input(
    task_context: &mut TaskContext,
    selected_entity: Option<Entity>,
    world_pos: Vec2,
    q_familiar_areas: &Query<&TaskArea, With<Familiar>>,
    q_sites: &Query<&Site>,
    q_familiars: &mut Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    q_target_sets: &mut bevy::ecs::system::ParamSet<(
        DesignationTargetQuery<'_, '_>,
        Query<(Entity, &Transform, &FloorTileBlueprint)>,
        Query<(Entity, &Transform, &WallTileBlueprint)>,
    )>,
    q_aux: &mut bevy::ecs::system::ParamSet<(
        Query<(Entity, &Transform, &Designation), Without<hw_core::relationships::ManagedBy>>,
        Query<Entity, With<AreaSelectionIndicator>>,
    )>,
    keyboard: &ButtonInput<KeyCode>,
    next_play_mode: &mut NextState<PlayMode>,
    commands: &mut Commands,
    area_edit_session: &mut AreaEditSession,
    area_edit_history: &mut AreaEditHistory,
) {
    match task_context.0 {
        TaskMode::AreaSelection(Some(start_pos)) => {
            let indicator_entities: Vec<Entity> = {
                collect_indicator_entities(&q_aux.p1())
            };
            let q_unassigned = q_aux.p0();
            handle_release_area_selection(
                task_context,
                selected_entity,
                world_pos,
                start_pos,
                q_familiar_areas,
                q_sites,
                q_familiars,
                &indicator_entities,
                &q_unassigned,
                keyboard,
                next_play_mode,
                commands,
                area_edit_history,
            );
        }
        TaskMode::DesignateChop(Some(start_pos))
        | TaskMode::DesignateMine(Some(start_pos))
        | TaskMode::DesignateHaul(Some(start_pos)) => {
            let mode = task_context.0;
            let q_targets = q_target_sets.p0();
            handle_release_designation(
                task_context,
                selected_entity,
                world_pos,
                start_pos,
                mode,
                q_familiars,
                &q_targets,
                commands,
            );
        }
        TaskMode::CancelDesignation(Some(start_pos)) => {
            handle_release_cancel_designation(
                task_context,
                selected_entity,
                world_pos,
                start_pos,
                q_target_sets,
                commands,
            );
        }
        TaskMode::DreamPlanting(Some(start_pos)) => {
            handle_release_dream_planting(task_context, world_pos, start_pos, area_edit_session);
        }
        _ => {}
    }
}
