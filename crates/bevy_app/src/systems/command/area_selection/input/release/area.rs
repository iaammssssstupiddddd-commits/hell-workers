use super::super::super::AreaEditHistory;
use super::super::super::apply::apply_area_and_record_history;
use super::super::super::apply::assign_unassigned_tasks_in_area;
use super::super::transitions::should_exit_after_apply;
use crate::app_contexts::TaskContext;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar};
use crate::systems::command::{AreaSelectionIndicator, TaskArea, TaskMode};
use crate::systems::jobs::Designation;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::game_state::PlayMode;
use hw_core::relationships::ManagedBy;
use hw_world::zones::Site;

pub(super) fn handle_release_area_selection(
    task_context: &mut TaskContext,
    selected_entity: Option<Entity>,
    world_pos: Vec2,
    start_pos: Vec2,
    q_familiar_areas: &Query<&TaskArea, With<Familiar>>,
    q_sites: &Query<&Site>,
    q_familiars: &mut Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    indicator_entities: &[Entity],
    q_unassigned: &Query<(Entity, &Transform, &Designation), Without<ManagedBy>>,
    keyboard: &ButtonInput<KeyCode>,
    next_play_mode: &mut NextState<PlayMode>,
    commands: &mut Commands,
    area_edit_history: &mut AreaEditHistory,
) {
    let end_pos = WorldMap::snap_to_grid_edge(world_pos);

    if start_pos.distance(end_pos) < 0.1 {
        task_context.0 = TaskMode::None;
        next_play_mode.set(PlayMode::Normal);
        despawn_indicators(indicator_entities, commands);
        return;
    }

    let new_area = TaskArea::from_points(start_pos, end_pos);
    if let Some(fam_entity) = selected_entity {
        let before_area = q_familiar_areas.get(fam_entity).ok().cloned();
        apply_area_and_record_history(
            fam_entity,
            &new_area,
            before_area,
            commands,
            q_familiars,
            area_edit_history,
            q_sites,
        );
        assign_unassigned_tasks_in_area(commands, fam_entity, &new_area, q_unassigned);
    }

    despawn_indicators(indicator_entities, commands);

    if should_exit_after_apply(keyboard) {
        task_context.0 = TaskMode::None;
        next_play_mode.set(PlayMode::Normal);
    } else {
        task_context.0 = TaskMode::AreaSelection(None);
    }
}

fn despawn_indicators(indicator_entities: &[Entity], commands: &mut Commands) {
    for &e in indicator_entities {
        commands.entity(e).try_despawn();
    }
}

pub(super) fn collect_indicator_entities(
    q_indicators: &Query<Entity, With<AreaSelectionIndicator>>,
) -> Vec<Entity> {
    q_indicators.iter().collect()
}
