use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::{LoadedIn, ManagedBy, StoredIn, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::wall_construction::TargetWallConstructionSite;
use crate::systems::jobs::{Blueprint, Designation, Rock, TargetBlueprint, Tree};
use crate::systems::logistics::transport_request::{TransportDemand, TransportRequest};
use crate::systems::logistics::{ReservedForTask, ResourceItem};
use crate::systems::world::zones::Yard;
use crate::world::map::WorldMapRead;
use crate::world::pathfinding::PathfindingContext;
use bevy::prelude::*;
use hw_core::constants::BLUEPRINT_AUTO_GATHER_INTERVAL_SECS;
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

pub use hw_ai::familiar_ai::decide::auto_gather_for_blueprint::AutoGatherDesignation;

pub fn blueprint_auto_gather_system(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<BlueprintAutoGatherTimer>,
    world_map: WorldMapRead,
    mut pf_context: Local<PathfindingContext>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea, &Transform)>,
    q_yards: Query<(Entity, &Yard)>,
    q_bp_requests: Query<(&TransportRequest, &TargetBlueprint, Option<&TaskWorkers>)>,
    q_wall_requests: Query<(
        &TransportRequest,
        &TargetWallConstructionSite,
        Option<&TaskWorkers>,
        Option<&TransportDemand>,
    )>,
    q_mixer_solid_requests: Query<(
        &TransportRequest,
        Option<&TaskWorkers>,
        Option<&TransportDemand>,
    )>,
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
            Option<&AutoGatherDesignation>,
        ),
        Or<(With<Tree>, With<Rock>, With<AutoGatherDesignation>)>,
    >,
) {
    let timer_finished = timer.timer.tick(time.delta()).just_finished();
    if timer.first_run_done && !timer_finished {
        return;
    }
    timer.first_run_done = true;

    let mut owner_infos = HashMap::<Entity, OwnerInfo>::new();
    let yards: Vec<(Entity, Yard)> = q_yards
        .iter()
        .map(|(entity, yard)| (entity, yard.clone()))
        .collect();
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

        let owner_pos = area.center();
        let owner_yard = yards
            .iter()
            .find(|(_, yard)| yard.contains(owner_pos))
            .map(|(_, yard)| yard.clone());
        owner_infos.insert(
            fam_entity,
            OwnerInfo {
                area: area.bounds(),
                center: area.center(),
                path_start,
                yard: owner_yard,
            },
        );
    }

    for (yard_entity, yard) in &yards {
        let yard_center = (yard.min + yard.max) / 2.0;
        let Some(path_start) = world_map.get_nearest_walkable_grid(yard_center) else {
            continue;
        };
        owner_infos.insert(
            *yard_entity,
            OwnerInfo {
                area: yard.bounds(),
                center: yard_center,
                path_start,
                yard: Some(yard.clone()),
            },
        );
    }

    let raw_demand_by_owner = collect_raw_demand_by_owner(
        &owner_infos,
        &q_bp_requests,
        &q_wall_requests,
        &q_mixer_solid_requests,
        &q_blueprints,
    );

    let mut supply_state = collect_supply_state(&owner_infos, &q_ground_items, &q_sources);

    let plan = build_auto_gather_targets(&raw_demand_by_owner, &supply_state.supply_by_owner);

    assign_needed_auto_designations(
        &mut commands,
        &plan.needed_new_auto_count,
        &owner_infos,
        &supply_state.candidate_sources,
        world_map.as_ref(),
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
