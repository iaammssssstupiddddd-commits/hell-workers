use bevy::prelude::*;

use crate::constants::REST_AREA_CAPACITY;
use crate::entities::damned_soul::{IdleBehavior, IdleState};
use crate::relationships::{RestAreaReservedFor, RestingIn, WorkingOn};
use crate::systems::jobs::{Building, BuildingType, RestArea};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;

/// AssignedTask が None なのに WorkingOn が残っている不整合を解消する。
pub fn clear_stale_working_on_system(
    mut commands: Commands,
    q_souls: Query<(Entity, &AssignedTask), With<WorkingOn>>,
) {
    for (entity, task) in q_souls.iter() {
        if matches!(task, AssignedTask::None) {
            commands.entity(entity).remove::<WorkingOn>();
        }
    }
}

/// Building.kind と RestArea コンポーネントの整合性を保つ。
pub fn ensure_rest_area_component_system(
    mut commands: Commands,
    q_buildings: Query<(Entity, &Building, Option<&RestArea>)>,
) {
    for (entity, building, rest_area_opt) in q_buildings.iter() {
        if building.kind == BuildingType::RestArea && rest_area_opt.is_none() {
            commands.entity(entity).insert(RestArea {
                capacity: REST_AREA_CAPACITY,
            });
        }
    }
}

/// 休憩状態と休憩リレーションの整合性を保つ。
pub fn reconcile_rest_state_system(
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &mut IdleState,
        Option<&RestingIn>,
        Option<&RestAreaReservedFor>,
    )>,
    mut q_visibility: Query<&mut Visibility, With<crate::entities::damned_soul::DamnedSoul>>,
) {
    for (entity, mut idle, resting_in, reserved_for) in q_souls.iter_mut() {
        if idle.behavior == IdleBehavior::Resting && resting_in.is_none() {
            idle.behavior = if reserved_for.is_some() {
                IdleBehavior::GoingToRest
            } else {
                IdleBehavior::Wandering
            };
            idle.idle_timer = 0.0;
        }

        if resting_in.is_none()
            && reserved_for.is_some()
            && !matches!(idle.behavior, IdleBehavior::GoingToRest | IdleBehavior::Resting)
        {
            commands.entity(entity).remove::<RestAreaReservedFor>();
        }

        if idle.behavior != IdleBehavior::Resting
            && (resting_in.is_some() || reserved_for.is_some())
            && matches!(
                idle.behavior,
                IdleBehavior::Escaping | IdleBehavior::Drifting | IdleBehavior::ExhaustedGathering
            )
        {
            commands
                .entity(entity)
                .remove::<(RestingIn, RestAreaReservedFor)>();
            if let Ok(mut visibility) = q_visibility.get_mut(entity) {
                *visibility = Visibility::Visible;
            }
        }
    }
}
