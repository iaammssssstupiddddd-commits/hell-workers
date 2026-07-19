//! Blueprint 自動資材収集システムのオーケストレーション。

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::BLUEPRINT_AUTO_GATHER_INTERVAL_SECS;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::relationships::{DeliveringTo, LoadedIn, ManagedBy, StoredIn, TaskWorkers};
use hw_jobs::construction::TargetWallConstructionSite;
use hw_jobs::model::{Blueprint, Designation, Rock, TargetBlueprint, Tree};
use hw_logistics::ResourceItem;
use hw_logistics::transport_request::components::{TransportDemand, TransportRequest};
use hw_world::{WalkabilityConnectivityCache, WorldMapRead, Yard};

use crate::familiar_ai::decide::auto_gather_for_blueprint::AutoGatherDesignation;
use crate::familiar_ai::decide::auto_gather_for_blueprint::actions::{
    assign_needed_auto_designations, cleanup_auto_gather_markers,
};
use crate::familiar_ai::decide::auto_gather_for_blueprint::demand::collect_auto_gather_demand;
use crate::familiar_ai::decide::auto_gather_for_blueprint::helpers::OwnerInfo;
use crate::familiar_ai::decide::auto_gather_for_blueprint::planning::{
    build_auto_gather_targets, resolve_raw_demand_by_owner,
};
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
        Without<StoredIn>,
        Without<LoadedIn>,
        Without<DeliveringTo>,
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
    connectivity_cache: ResMut<'w, WalkabilityConnectivityCache>,
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

    let demand = collect_auto_gather_demand(
        &owner_infos,
        &p.q_bp_requests,
        &p.q_wall_requests,
        &p.q_mixer_solid_requests,
        &p.q_blueprints,
    );
    let owner_resource_interest = demand.owner_resource_interest();

    let mut supply_state = collect_supply_state(
        &owner_infos,
        &owner_resource_interest,
        &p.q_ground_items,
        &p.q_sources,
        p.world_map.as_ref(),
        &mut p.connectivity_cache,
    );
    let raw_demand_by_owner = resolve_raw_demand_by_owner(
        demand,
        &supply_state.supply_by_owner,
        &supply_state.candidate_sources,
        &owner_infos,
        p.world_map.as_ref(),
        &mut p.connectivity_cache,
    );

    let plan = build_auto_gather_targets(&raw_demand_by_owner, &supply_state.supply_by_owner);

    assign_needed_auto_designations(
        &mut commands,
        &plan.needed_new_auto_count,
        &owner_infos,
        &supply_state.candidate_sources,
        p.world_map.as_ref(),
        &mut p.connectivity_cache,
    );

    cleanup_auto_gather_markers(
        &mut commands,
        supply_state.stale_marker_only,
        supply_state.invalid_auto_idle,
        &mut supply_state.supply_by_owner,
        &plan.target_auto_idle_count,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::familiar::FamiliarCommand;
    use hw_core::logistics::ResourceType;
    use hw_core::relationships::{DeliveringTo, ManagedBy, WorkingOn};
    use hw_jobs::{BuildingType, Priority, TaskSlots, WorkType};
    use hw_logistics::transport_request::{TransportPriority, TransportRequestKind};
    use hw_world::WorldMap;

    fn assert_yard_demand_designates_source(
        resource_type: ResourceType,
        expected_work_type: WorkType,
    ) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<WorldMap>()
            .init_resource::<WalkabilityConnectivityCache>()
            .init_resource::<BlueprintAutoGatherTimer>()
            .add_systems(Update, blueprint_auto_gather_system);

        let yard = app
            .world_mut()
            .spawn(Yard {
                min: WorldMap::grid_to_world(10, 10),
                max: WorldMap::grid_to_world(20, 20),
            })
            .id();

        let source_pos = WorldMap::grid_to_world(40, 40);
        let familiar = app
            .world_mut()
            .spawn((
                ActiveCommand {
                    command: FamiliarCommand::Patrol,
                },
                TaskArea::from_points(
                    WorldMap::grid_to_world(35, 35),
                    WorldMap::grid_to_world(45, 45),
                ),
                Transform::from_translation(source_pos.extend(0.0)),
            ))
            .id();

        let mut blueprint = Blueprint::new(BuildingType::Tank, vec![(15, 15)]);
        blueprint.required_materials.clear();
        blueprint.required_materials.insert(resource_type, 1);
        blueprint.flexible_material_requirement = None;
        let blueprint_entity = app.world_mut().spawn(blueprint).id();

        app.world_mut().spawn((
            TransportRequest {
                kind: TransportRequestKind::DeliverToBlueprint,
                anchor: blueprint_entity,
                resource_type,
                issued_by: yard,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TargetBlueprint(blueprint_entity),
        ));

        let source = match resource_type {
            ResourceType::Wood => app
                .world_mut()
                .spawn((Tree, Transform::from_translation(source_pos.extend(0.0))))
                .id(),
            ResourceType::Rock => app
                .world_mut()
                .spawn((Rock, Transform::from_translation(source_pos.extend(0.0))))
                .id(),
            other => panic!("unsupported test resource: {other:?}"),
        };

        app.update();

        let source_ref = app.world().entity(source);
        assert_eq!(
            source_ref.get::<Designation>().map(|d| d.work_type),
            Some(expected_work_type)
        );
        assert_eq!(
            source_ref.get::<ManagedBy>().map(|owner| owner.0),
            Some(yard)
        );
        assert_eq!(
            source_ref
                .get::<AutoGatherDesignation>()
                .map(|marker| (marker.owner, marker.resource_type)),
            Some((yard, resource_type))
        );
        assert!(source_ref.contains::<TaskSlots>());
        assert!(source_ref.contains::<Priority>());

        // The source lies inside the Familiar area, which is the condition that
        // previously split supply away from the Yard-owned demand.
        assert_ne!(familiar, yard);
    }

    #[test]
    fn yard_owned_wood_demand_uses_tree_in_familiar_area() {
        assert_yard_demand_designates_source(ResourceType::Wood, WorkType::Chop);
    }

    #[test]
    fn yard_owned_rock_demand_uses_rock_in_familiar_area() {
        assert_yard_demand_designates_source(ResourceType::Rock, WorkType::Mine);
    }

    #[test]
    fn bridge_flexible_demand_uses_reachable_rock_over_unreachable_trees() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<WorldMap>()
            .init_resource::<WalkabilityConnectivityCache>()
            .init_resource::<BlueprintAutoGatherTimer>()
            .add_systems(Update, blueprint_auto_gather_system);

        let yard = app
            .world_mut()
            .spawn(Yard {
                min: WorldMap::grid_to_world(10, 10),
                max: WorldMap::grid_to_world(20, 20),
            })
            .id();
        let source_pos = WorldMap::grid_to_world(40, 40);
        app.world_mut().spawn((
            ActiveCommand {
                command: FamiliarCommand::Patrol,
            },
            TaskArea::from_points(
                WorldMap::grid_to_world(35, 35),
                WorldMap::grid_to_world(45, 45),
            ),
            Transform::from_translation(source_pos.extend(0.0)),
        ));

        let blueprint_entity = app
            .world_mut()
            .spawn(Blueprint::new(BuildingType::Bridge, vec![(15, 15)]))
            .id();
        for resource_type in [ResourceType::Wood, ResourceType::Rock] {
            app.world_mut().spawn((
                TransportRequest {
                    kind: TransportRequestKind::DeliverToBlueprint,
                    anchor: blueprint_entity,
                    resource_type,
                    issued_by: yard,
                    priority: TransportPriority::Normal,
                    stockpile_group: Vec::new(),
                },
                TargetBlueprint(blueprint_entity),
            ));
        }

        for tree_grid in [(60, 60), (70, 70)] {
            {
                let mut world_map = app.world_mut().resource_mut::<WorldMap>();
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx != 0 || dy != 0 {
                            world_map.add_obstacle(tree_grid.0 + dx, tree_grid.1 + dy);
                        }
                    }
                }
            }
            app.world_mut().spawn((
                Tree,
                Transform::from_translation(
                    WorldMap::grid_to_world(tree_grid.0, tree_grid.1).extend(0.0),
                ),
            ));
        }

        let rock = app
            .world_mut()
            .spawn((Rock, Transform::from_translation(source_pos.extend(0.0))))
            .id();

        app.update();

        let rock_ref = app.world().entity(rock);
        assert_eq!(
            rock_ref.get::<Designation>().map(|d| d.work_type),
            Some(WorkType::Mine)
        );
        assert_eq!(
            rock_ref
                .get::<AutoGatherDesignation>()
                .map(|marker| (marker.owner, marker.resource_type)),
            Some((yard, ResourceType::Rock))
        );
    }

    #[test]
    fn unreachable_ground_wood_does_not_suppress_tree_designation() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<WorldMap>()
            .init_resource::<WalkabilityConnectivityCache>()
            .init_resource::<BlueprintAutoGatherTimer>()
            .add_systems(Update, blueprint_auto_gather_system);

        let yard = app
            .world_mut()
            .spawn(Yard {
                min: WorldMap::grid_to_world(10, 10),
                max: WorldMap::grid_to_world(20, 20),
            })
            .id();
        let tree_pos = WorldMap::grid_to_world(40, 40);
        app.world_mut().spawn((
            ActiveCommand {
                command: FamiliarCommand::Patrol,
            },
            TaskArea::from_points(
                WorldMap::grid_to_world(35, 35),
                WorldMap::grid_to_world(45, 45),
            ),
            Transform::from_translation(tree_pos.extend(0.0)),
        ));

        let mut blueprint = Blueprint::new(BuildingType::Tank, vec![(15, 15)]);
        blueprint.required_materials.clear();
        blueprint.required_materials.insert(ResourceType::Wood, 1);
        let blueprint_entity = app.world_mut().spawn(blueprint).id();
        app.world_mut().spawn((
            TransportRequest {
                kind: TransportRequestKind::DeliverToBlueprint,
                anchor: blueprint_entity,
                resource_type: ResourceType::Wood,
                issued_by: yard,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TargetBlueprint(blueprint_entity),
        ));

        let unreachable_grid = (80, 80);
        {
            let mut world_map = app.world_mut().resource_mut::<WorldMap>();
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx != 0 || dy != 0 {
                        world_map.add_obstacle(unreachable_grid.0 + dx, unreachable_grid.1 + dy);
                    }
                }
            }
        }
        app.world_mut().spawn((
            ResourceItem(ResourceType::Wood),
            Transform::from_translation(
                WorldMap::grid_to_world(unreachable_grid.0, unreachable_grid.1).extend(0.0),
            ),
            Visibility::Visible,
        ));
        let tree = app
            .world_mut()
            .spawn((Tree, Transform::from_translation(tree_pos.extend(0.0))))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .entity(tree)
                .get::<Designation>()
                .map(|designation| designation.work_type),
            Some(WorkType::Chop)
        );
    }

    #[test]
    fn delivering_ground_wood_does_not_double_count_inflight_supply() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<WorldMap>()
            .init_resource::<WalkabilityConnectivityCache>()
            .init_resource::<BlueprintAutoGatherTimer>()
            .add_systems(Update, blueprint_auto_gather_system);

        let yard = app
            .world_mut()
            .spawn(Yard {
                min: WorldMap::grid_to_world(10, 10),
                max: WorldMap::grid_to_world(20, 20),
            })
            .id();
        let tree_pos = WorldMap::grid_to_world(40, 40);
        app.world_mut().spawn((
            ActiveCommand {
                command: FamiliarCommand::Patrol,
            },
            TaskArea::from_points(
                WorldMap::grid_to_world(35, 35),
                WorldMap::grid_to_world(45, 45),
            ),
            Transform::from_translation(tree_pos.extend(0.0)),
        ));

        let mut blueprint = Blueprint::new(BuildingType::Tank, vec![(15, 15)]);
        blueprint.required_materials.clear();
        blueprint.required_materials.insert(ResourceType::Wood, 2);
        let blueprint_entity = app.world_mut().spawn(blueprint).id();
        let request = app
            .world_mut()
            .spawn((
                TransportRequest {
                    kind: TransportRequestKind::DeliverToBlueprint,
                    anchor: blueprint_entity,
                    resource_type: ResourceType::Wood,
                    issued_by: yard,
                    priority: TransportPriority::Normal,
                    stockpile_group: Vec::new(),
                },
                TargetBlueprint(blueprint_entity),
            ))
            .id();
        app.world_mut().spawn(WorkingOn(request));
        app.world_mut().spawn((
            ResourceItem(ResourceType::Wood),
            DeliveringTo(blueprint_entity),
            Transform::from_translation(WorldMap::grid_to_world(15, 15).extend(0.0)),
            Visibility::Visible,
        ));
        let tree = app
            .world_mut()
            .spawn((Tree, Transform::from_translation(tree_pos.extend(0.0))))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .entity(tree)
                .get::<Designation>()
                .map(|designation| designation.work_type),
            Some(WorkType::Chop)
        );
    }

    #[test]
    fn manual_chop_in_another_owner_area_does_not_suppress_yard_demand() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<WorldMap>()
            .init_resource::<WalkabilityConnectivityCache>()
            .init_resource::<BlueprintAutoGatherTimer>()
            .add_systems(Update, blueprint_auto_gather_system);

        let yard = app
            .world_mut()
            .spawn(Yard {
                min: WorldMap::grid_to_world(10, 10),
                max: WorldMap::grid_to_world(20, 20),
            })
            .id();
        let candidate_pos = WorldMap::grid_to_world(40, 40);
        app.world_mut().spawn((
            ActiveCommand {
                command: FamiliarCommand::Patrol,
            },
            TaskArea::from_points(
                WorldMap::grid_to_world(35, 35),
                WorldMap::grid_to_world(45, 45),
            ),
            Transform::from_translation(candidate_pos.extend(0.0)),
        ));
        app.world_mut().spawn((
            ActiveCommand {
                command: FamiliarCommand::Patrol,
            },
            TaskArea::from_points(
                WorldMap::grid_to_world(75, 75),
                WorldMap::grid_to_world(85, 85),
            ),
            Transform::from_translation(WorldMap::grid_to_world(80, 80).extend(0.0)),
        ));

        let mut blueprint = Blueprint::new(BuildingType::Tank, vec![(15, 15)]);
        blueprint.required_materials.clear();
        blueprint.required_materials.insert(ResourceType::Wood, 1);
        let blueprint_entity = app.world_mut().spawn(blueprint).id();
        app.world_mut().spawn((
            TransportRequest {
                kind: TransportRequestKind::DeliverToBlueprint,
                anchor: blueprint_entity,
                resource_type: ResourceType::Wood,
                issued_by: yard,
                priority: TransportPriority::Normal,
                stockpile_group: Vec::new(),
            },
            TargetBlueprint(blueprint_entity),
        ));

        app.world_mut().spawn((
            Tree,
            Designation {
                work_type: WorkType::Chop,
            },
            Transform::from_translation(WorldMap::grid_to_world(80, 80).extend(0.0)),
        ));
        let candidate = app
            .world_mut()
            .spawn((Tree, Transform::from_translation(candidate_pos.extend(0.0))))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .entity(candidate)
                .get::<AutoGatherDesignation>()
                .map(|marker| (marker.owner, marker.resource_type)),
            Some((yard, ResourceType::Wood))
        );
    }
}
