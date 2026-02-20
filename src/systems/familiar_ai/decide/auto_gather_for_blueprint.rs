use crate::constants::BLUEPRINT_AUTO_GATHER_INTERVAL_SECS;
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::{LoadedIn, ManagedBy, StoredIn, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Blueprint, Designation, Rock, TargetBlueprint, Tree};
use crate::systems::logistics::transport_request::TransportRequest;
use crate::systems::logistics::{ReservedForTask, ResourceItem, ResourceType};
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;
use bevy::prelude::*;
use std::collections::HashMap;

mod actions;
mod demand;
mod helpers;
mod planning;
mod supply;

use self::actions::{assign_needed_auto_designations, cleanup_auto_gather_markers};
use self::demand::collect_raw_demand_by_owner;
use self::helpers::OwnerInfo;
use self::planning::build_auto_gather_targets;
use self::supply::collect_supply_state;

#[derive(Resource)]
pub struct BlueprintAutoGatherTimer {
    pub timer: Timer,
    pub first_run_done: bool,
}

impl Default for BlueprintAutoGatherTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(BLUEPRINT_AUTO_GATHER_INTERVAL_SECS, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AutoGatherForBlueprint {
    pub owner: Entity,
    pub resource_type: ResourceType,
}

pub fn blueprint_auto_gather_system(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<BlueprintAutoGatherTimer>,
    world_map: Res<WorldMap>,
    mut pf_context: Local<PathfindingContext>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea, &Transform)>,
    q_bp_requests: Query<(&TransportRequest, &TargetBlueprint, Option<&TaskWorkers>)>,
    q_blueprints: Query<&Blueprint>,
    q_ground_items: Query<
        (&Transform, &Visibility, &ResourceItem),
        (
            Without<Designation>,
            Without<TaskWorkers>,
            Without<ReservedForTask>,
            Without<StoredIn>,
            Without<LoadedIn>,
        ),
    >,
    q_sources: Query<
        (
            Entity,
            &Transform,
            Option<&Tree>,
            Option<&Rock>,
            Option<&Designation>,
            Option<&TaskWorkers>,
            Option<&ManagedBy>,
            Option<&AutoGatherForBlueprint>,
        ),
        Or<(With<Tree>, With<Rock>, With<AutoGatherForBlueprint>)>,
    >,
) {
    let timer_finished = timer.timer.tick(time.delta()).just_finished();
    if timer.first_run_done && !timer_finished {
        return;
    }
    timer.first_run_done = true;

    let mut owner_infos = HashMap::<Entity, OwnerInfo>::new();
    for (fam_entity, active_command, area, transform) in q_familiars.iter() {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }

        let start_grid = world_map
            .get_nearest_walkable_grid(transform.translation.truncate())
            .or_else(|| world_map.get_nearest_walkable_grid(area.center()));
        let Some(path_start) = start_grid else {
            continue;
        };

        owner_infos.insert(
            fam_entity,
            OwnerInfo {
                area: area.clone(),
                center: area.center(),
                path_start,
            },
        );
    }

    let mut owner_areas: Vec<(Entity, TaskArea)> = owner_infos
        .iter()
        .map(|(entity, info)| (*entity, info.area.clone()))
        .collect();
    owner_areas.sort_by_key(|(entity, _)| entity.to_bits());

    let raw_demand_by_owner =
        collect_raw_demand_by_owner(&owner_infos, &q_bp_requests, &q_blueprints);

    let mut supply_state =
        collect_supply_state(&owner_infos, &owner_areas, &q_ground_items, &q_sources);

    let plan = build_auto_gather_targets(&raw_demand_by_owner, &supply_state.supply_by_owner);

    assign_needed_auto_designations(
        &mut commands,
        &plan.needed_new_auto_count,
        &owner_infos,
        &supply_state.candidate_sources,
        &world_map,
        &mut pf_context,
    );

    cleanup_auto_gather_markers(
        &mut commands,
        supply_state.stale_marker_only,
        supply_state.invalid_auto_idle,
        &mut supply_state.supply_by_owner,
        &plan.target_auto_idle_count,
    );
}
