//! Blueprint 自動資材収集システムのオーケストレーション。

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::BLUEPRINT_AUTO_GATHER_INTERVAL_SECS;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::relationships::{LoadedIn, ManagedBy, StoredIn, TaskWorkers};
use hw_jobs::construction::TargetWallConstructionSite;
use hw_jobs::model::{Blueprint, Designation, Rock, TargetBlueprint, Tree};
use hw_logistics::transport_request::components::{TransportDemand, TransportRequest};
use hw_logistics::{ReservedForTask, ResourceItem};
use hw_world::pathfinding::PathfindingContext;
use hw_world::{WorldMapRead, Yard};

use crate::familiar_ai::decide::auto_gather_for_blueprint::AutoGatherDesignation;
use crate::familiar_ai::decide::auto_gather_for_blueprint::actions::{
    assign_needed_auto_designations, cleanup_auto_gather_markers,
};
use crate::familiar_ai::decide::auto_gather_for_blueprint::demand::collect_raw_demand_by_owner;
use crate::familiar_ai::decide::auto_gather_for_blueprint::helpers::OwnerInfo;
use crate::familiar_ai::decide::auto_gather_for_blueprint::planning::build_auto_gather_targets;
use crate::familiar_ai::decide::auto_gather_for_blueprint::supply::collect_supply_state;

type BpGroundItemsQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static Visibility,
        &'static ResourceItem,
    ),
    (
        Without<Designation>,
        Without<TaskWorkers>,
        Without<ReservedForTask>,
        Without<StoredIn>,
        Without<LoadedIn>,
    ),
>;

type BpSourcesQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static Tree>,
        Option<&'static Rock>,
        Option<&'static Designation>,
        Option<&'static TaskWorkers>,
        Option<&'static ManagedBy>,
        Option<&'static AutoGatherDesignation>,
    ),
    Or<(With<Tree>, With<Rock>, With<AutoGatherDesignation>)>,
>;

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

#[derive(SystemParam)]
pub struct BlueprintAutoGatherParams<'w, 's> {
    world_map: WorldMapRead<'w>,
    pf_context: Local<'s, PathfindingContext>,
    q_familiars: Query<
        'w,
        's,
        (
            Entity,
            &'static ActiveCommand,
            &'static TaskArea,
            &'static Transform,
        ),
    >,
    q_yards: Query<'w, 's, (Entity, &'static Yard)>,
    q_bp_requests: Query<
        'w,
        's,
        (
            &'static TransportRequest,
            &'static TargetBlueprint,
            Option<&'static TaskWorkers>,
        ),
    >,
    q_wall_requests: Query<
        'w,
        's,
        (
            &'static TransportRequest,
            &'static TargetWallConstructionSite,
            Option<&'static TaskWorkers>,
            Option<&'static TransportDemand>,
        ),
    >,
    q_mixer_solid_requests: Query<
        'w,
        's,
        (
            &'static TransportRequest,
            Option<&'static TaskWorkers>,
            Option<&'static TransportDemand>,
        ),
    >,
    q_blueprints: Query<'w, 's, &'static Blueprint>,
    q_ground_items: BpGroundItemsQuery<'w, 's>,
    q_sources: BpSourcesQuery<'w, 's>,
}

pub fn blueprint_auto_gather_system(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<BlueprintAutoGatherTimer>,
    mut p: BlueprintAutoGatherParams,
) {
    let timer_finished = timer.timer.tick(time.delta()).just_finished();
    if timer.first_run_done && !timer_finished {
        return;
    }
    timer.first_run_done = true;

    let mut owner_infos = HashMap::<Entity, OwnerInfo>::new();
    let yards: Vec<(Entity, Yard)> = p
        .q_yards
        .iter()
        .map(|(entity, yard)| (entity, yard.clone()))
        .collect();

    for (fam_entity, active_command, area, transform) in p.q_familiars.iter() {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }

        let start_grid = p
            .world_map
            .get_nearest_walkable_grid(transform.translation.truncate())
            .or_else(|| p.world_map.get_nearest_walkable_grid(area.center()));
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
        let Some(path_start) = p.world_map.get_nearest_walkable_grid(yard_center) else {
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
        &p.q_bp_requests,
        &p.q_wall_requests,
        &p.q_mixer_solid_requests,
        &p.q_blueprints,
    );

    let mut supply_state = collect_supply_state(&owner_infos, &p.q_ground_items, &p.q_sources);

    let plan = build_auto_gather_targets(&raw_demand_by_owner, &supply_state.supply_by_owner);

    assign_needed_auto_designations(
        &mut commands,
        &plan.needed_new_auto_count,
        &owner_infos,
        &supply_state.candidate_sources,
        p.world_map.as_ref(),
        &mut p.pf_context,
    );

    cleanup_auto_gather_markers(
        &mut commands,
        supply_state.stale_marker_only,
        supply_state.invalid_auto_idle,
        &mut supply_state.supply_by_owner,
        &plan.target_auto_idle_count,
    );
}
