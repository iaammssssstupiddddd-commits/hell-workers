use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::events::{EscapeOperation, EscapeRequest};
use hw_core::familiar::Familiar;
use hw_core::relationships::{CommandedBy, ParticipatingIn};
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState};
use hw_core::{EpochLocal, WorldEpoch};
use hw_spatial::FamiliarSpatialGrid;
use hw_world::{PathSearchResult, PathfindingContext, RuntimePathSearchBudget, WorldMap};

use crate::soul_ai::decide::SoulDecideOutput;
use crate::soul_ai::decide::idle_behavior::GATHERING_ARRIVAL_RADIUS;
use crate::soul_ai::helpers::gathering::GatheringSpot;
use crate::soul_ai::pathfinding::ESCAPE_PATHFINDS_PER_FRAME;
use crate::soul_ai::perceive::escaping::{
    EscapeBehaviorTimer, EscapeDetectionTimer, EscapePathSearchInputs, EscapePathSearchProgress,
    calculate_escape_destination, detect_nearest_familiar,
    detect_reachable_familiar_within_safe_distance, find_safe_gathering_spot,
};

type EscapeDetectQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static DamnedSoul,
        Option<&'static CommandedBy>,
        Option<&'static ParticipatingIn>,
        &'static IdleState,
    ),
>;

type EscapeBehaviorQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static IdleState,
        Option<&'static CommandedBy>,
        Option<&'static ParticipatingIn>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct EscapeTimers<'w> {
    detection_timer: ResMut<'w, EscapeDetectionTimer>,
    behavior_timer: ResMut<'w, EscapeBehaviorTimer>,
}

#[derive(SystemParam)]
pub(crate) struct EscapeLocalState<'w, 's> {
    pf_context: Local<'s, PathfindingContext>,
    nearby_buf: Local<'s, Vec<Entity>>,
    path_budget: ResMut<'w, RuntimePathSearchBudget>,
    world_epoch: Option<Res<'w, WorldEpoch>>,
    runtime: Local<'s, EpochLocal<EscapeRuntimeState>>,
}

#[derive(Default)]
struct EscapeRuntimeState {
    progress: EscapePathSearchProgress,
    last_core_search_claimant: Option<Entity>,
    entities: Vec<Entity>,
}

#[derive(SystemParam)]
pub(crate) struct EscapeSpatialInputs<'w, 's> {
    world_map: Res<'w, WorldMap>,
    familiar_grid: Res<'w, FamiliarSpatialGrid>,
    q_familiars: Query<'w, 's, (&'static Transform, &'static Familiar)>,
    q_gathering_spots: Query<'w, 's, (Entity, &'static GatheringSpot)>,
}

/// 逃走の判定と要求生成を行う（Decide Phase）
pub(crate) fn escaping_decision_system(
    time: Res<Time>,
    mut timers: EscapeTimers,
    mut local: EscapeLocalState,
    spatial: EscapeSpatialInputs,
    q_detect: EscapeDetectQuery,
    q_behavior: EscapeBehaviorQuery,
    mut decide_output: SoulDecideOutput,
) {
    let detect_tick = timers
        .detection_timer
        .timer
        .tick(time.delta())
        .just_finished();

    let behavior_tick = {
        let finished = timers
            .behavior_timer
            .timer
            .tick(time.delta())
            .just_finished();
        if timers.behavior_timer.first_run_done && !finished {
            false
        } else {
            timers.behavior_timer.first_run_done = true;
            true
        }
    };

    if !detect_tick && !behavior_tick {
        return;
    }

    if detect_tick {
        for (entity, transform, soul, under_command, participating_in, idle_state) in
            q_detect.iter()
        {
            if matches!(
                idle_state.behavior,
                IdleBehavior::Escaping
                    | IdleBehavior::Drifting
                    | IdleBehavior::Resting
                    | IdleBehavior::GoingToRest
            ) {
                continue;
            }
            if under_command.is_some() {
                continue;
            }
            if idle_state.behavior == IdleBehavior::ExhaustedGathering {
                continue;
            }
            if soul.stress <= ESCAPE_STRESS_THRESHOLD {
                continue;
            }

            let soul_pos = transform.translation.truncate();
            if let Some(threat) = detect_nearest_familiar(
                soul_pos,
                &spatial.familiar_grid,
                &spatial.q_familiars,
                &mut local.nearby_buf,
            ) {
                debug!(
                    "ESCAPE_DECIDE: {:?} detected threat {:?} dist {:.1}",
                    entity, threat.entity, threat.distance
                );
                decide_output.escape_requests.write(EscapeRequest {
                    entity,
                    operation: EscapeOperation::StartEscaping {
                        leave_gathering: participating_in.map(|p| p.0),
                    },
                });
            }
        }
    }

    if behavior_tick {
        // Decide runs before Actor. Keep the escape slice small so task and
        // idle pathfinding can raise the cumulative ceiling afterwards.
        local.path_budget.begin_phase(ESCAPE_PATHFINDS_PER_FRAME);

        let world_epoch = local
            .world_epoch
            .map_or_else(WorldEpoch::default, |epoch| *epoch);
        let runtime = local.runtime.get_mut(world_epoch);
        runtime.entities.clear();
        runtime
            .entities
            .extend(q_behavior.iter().map(|(entity, ..)| entity));
        let entity_count = runtime.entities.len();
        let start = runtime
            .last_core_search_claimant
            .and_then(|last| runtime.entities.iter().position(|entity| *entity == last))
            .map_or(0, |index| (index + 1) % entity_count.max(1));

        for offset in 0..entity_count {
            let entity = runtime.entities[(start + offset) % entity_count];
            let Ok((entity, transform, idle_state, under_command, _participating)) =
                q_behavior.get(entity)
            else {
                runtime.progress.clear_entity(entity);
                continue;
            };
            if idle_state.behavior != IdleBehavior::Escaping {
                runtime.progress.clear_entity(entity);
                continue;
            }

            if under_command.is_some() {
                runtime.progress.clear_entity(entity);
                decide_output.escape_requests.write(EscapeRequest {
                    entity,
                    operation: EscapeOperation::ReachSafety,
                });
                continue;
            }

            let soul_pos = transform.translation.truncate();
            let budget_used_before = local.path_budget.used();
            match detect_reachable_familiar_within_safe_distance(EscapePathSearchInputs {
                escaping_soul: entity,
                progress: &mut runtime.progress,
                soul_pos,
                familiar_grid: &spatial.familiar_grid,
                q_familiars: &spatial.q_familiars,
                world_map: spatial.world_map.as_ref(),
                pf_context: &mut local.pf_context,
                budget: &mut local.path_budget,
                scratch: &mut local.nearby_buf,
            }) {
                PathSearchResult::Found(threat) => {
                    let safe_spot = find_safe_gathering_spot(
                        soul_pos,
                        &spatial.q_gathering_spots,
                        &spatial.familiar_grid,
                        &spatial.q_familiars,
                        &mut local.nearby_buf,
                    );

                    if let Some(spot_pos) = safe_spot
                        && soul_pos.distance(spot_pos) <= GATHERING_ARRIVAL_RADIUS
                    {
                        decide_output.escape_requests.write(EscapeRequest {
                            entity,
                            operation: EscapeOperation::JoinSafeGathering,
                        });
                        continue;
                    }

                    let destination = calculate_escape_destination(
                        soul_pos,
                        &threat,
                        safe_spot,
                        spatial.world_map.as_ref(),
                    );

                    decide_output.escape_requests.write(EscapeRequest {
                        entity,
                        operation: EscapeOperation::UpdateDestination { destination },
                    });
                }
                PathSearchResult::Unreachable => {
                    decide_output.escape_requests.write(EscapeRequest {
                        entity,
                        operation: EscapeOperation::ReachSafety,
                    });
                }
                // Budget exhaustion is not a safety decision. Leaving the
                // request stream untouched preserves Escaping, Destination,
                // and the existing Path until the next behavior tick.
                PathSearchResult::Deferred => {}
            }
            if local.path_budget.used() > budget_used_before {
                runtime.last_core_search_claimant = Some(entity);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hw_core::constants::MAP_HEIGHT;
    use hw_core::events::GatheringManagementRequest;
    use hw_core::soul::{Destination, Path};
    use hw_world::SpatialGridOps;

    use crate::soul_ai::execute::escaping_apply::escaping_apply_system;

    #[derive(Resource, Default)]
    struct EscapeRequestLog(Vec<EscapeOperation>);

    fn collect_escape_requests(
        mut reader: MessageReader<EscapeRequest>,
        mut log: ResMut<EscapeRequestLog>,
    ) {
        log.0
            .extend(reader.read().map(|request| request.operation.clone()));
    }

    fn escape_decision_test_app(
        world_map: WorldMap,
        hard_limit: usize,
    ) -> (App, Entity, Vec2, Vec2) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(world_map)
            .init_resource::<FamiliarSpatialGrid>()
            .insert_resource(RuntimePathSearchBudget::new(hard_limit))
            .init_resource::<EscapeDetectionTimer>()
            .init_resource::<EscapeBehaviorTimer>()
            .init_resource::<EscapeRequestLog>()
            .add_message::<EscapeRequest>()
            .add_message::<GatheringManagementRequest>()
            .add_systems(
                Update,
                (
                    escaping_decision_system,
                    escaping_apply_system,
                    collect_escape_requests,
                )
                    .chain(),
            );

        let soul_pos = WorldMap::grid_to_world(10, 10);
        // 320px: within the 448px safe distance but outside the 313.6px
        // Euclidean fast path, so this candidate requires one core A*.
        let familiar_pos = WorldMap::grid_to_world(20, 10);
        let familiar = app
            .world_mut()
            .spawn((
                Transform::from_translation(familiar_pos.extend(0.0)),
                Familiar::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<FamiliarSpatialGrid>()
            .insert(familiar, familiar_pos);

        let initial_destination = WorldMap::grid_to_world(5, 10);
        let initial_waypoint = WorldMap::grid_to_world(6, 10);
        let soul = app
            .world_mut()
            .spawn((
                Transform::from_translation(soul_pos.extend(0.0)),
                DamnedSoul::default(),
                IdleState {
                    behavior: IdleBehavior::Escaping,
                    ..default()
                },
                Destination(initial_destination),
                Path {
                    waypoints: vec![initial_waypoint],
                    ..default()
                },
            ))
            .id();

        (app, soul, initial_destination, initial_waypoint)
    }

    #[test]
    fn deferred_escape_search_preserves_existing_escape_state() {
        let (mut app, soul, initial_destination, initial_waypoint) =
            escape_decision_test_app(WorldMap::default(), 0);

        app.update();

        assert!(app.world().resource::<EscapeRequestLog>().0.is_empty());
        assert_eq!(
            app.world().get::<IdleState>(soul).map(|idle| idle.behavior),
            Some(IdleBehavior::Escaping)
        );
        assert_eq!(
            app.world()
                .get::<Destination>(soul)
                .map(|destination| destination.0),
            Some(initial_destination)
        );
        assert_eq!(
            app.world()
                .get::<Path>(soul)
                .map(|path| path.waypoints.as_slice()),
            Some([initial_waypoint].as_slice())
        );
        assert_eq!(app.world().resource::<RuntimePathSearchBudget>().used(), 0);
    }

    #[test]
    fn reachable_escape_threat_claims_one_core_search() {
        let (mut app, _soul, _initial_destination, _initial_waypoint) =
            escape_decision_test_app(WorldMap::default(), 1);

        app.update();

        assert!(matches!(
            app.world().resource::<EscapeRequestLog>().0.as_slice(),
            [EscapeOperation::UpdateDestination { .. }]
        ));
        assert_eq!(app.world().resource::<RuntimePathSearchBudget>().used(), 1);
    }

    #[test]
    fn unreachable_escape_threat_still_reaches_safety() {
        let mut world_map = WorldMap::default();
        for y in 0..MAP_HEIGHT {
            world_map.add_grid_obstacle((15, y));
        }
        let (mut app, soul, _initial_destination, _initial_waypoint) =
            escape_decision_test_app(world_map, 1);

        app.update();

        assert!(matches!(
            app.world().resource::<EscapeRequestLog>().0.as_slice(),
            [EscapeOperation::ReachSafety]
        ));
        assert_eq!(
            app.world().get::<IdleState>(soul).map(|idle| idle.behavior),
            Some(IdleBehavior::Wandering)
        );
        assert_eq!(app.world().resource::<RuntimePathSearchBudget>().used(), 1);
    }
}
