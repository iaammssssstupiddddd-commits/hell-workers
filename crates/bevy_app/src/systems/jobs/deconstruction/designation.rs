//! Headless owner transaction for creating durable deconstruction orders.

use bevy::prelude::*;
use hw_core::WorldEpoch;
use hw_energy::{PowerConsumer, PowerGenerator, SoulSpaPhase, SoulSpaSite};
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{
    AssignedTask, BonePile, BridgeMarker, DeconstructionDesignationOutcome,
    DeconstructionDesignationRejectReason, DeconstructionDesignationRequest,
    DeconstructionDesignationResult, DeconstructionEligibilityFacts, DeconstructionOrder,
    DeconstructionPending, DeconstructionTargetMarkers, Designation, Door, MovePlantTask,
    PendingBuildingMove, PlayerIssuedDesignation, Priority, ResolvedDeconstructionTarget, RestArea,
    SandPile, TargetDeconstructionRoot, TaskSlots, WorkType, deconstruction_marker_matches,
    evaluate_deconstruction_target, resolve_deconstruction_target, supports_deconstruction_cleanup,
};
use hw_logistics::ResourceType;
use hw_logistics::types::WheelbarrowParking;
use hw_logistics::zone::Stockpile;

#[cfg(test)]
use hw_jobs::DeconstructionTargetClass;

/// Applies every logical designation request in stable request order.
///
/// This is intentionally an exclusive root transaction.  The winning request
/// makes `TargetDeconstructionRoot` and `DeconstructionPending` visible before
/// the next request is evaluated, so same-batch duplicate clicks cannot create
/// two durable orders.
pub fn deconstruction_designation_system(world: &mut World) {
    let mut requests = world
        .resource_mut::<Messages<DeconstructionDesignationRequest>>()
        .drain()
        .collect::<Vec<_>>();
    requests.sort_unstable_by_key(|request| {
        (
            request.world_epoch,
            request.request_id,
            request.hit.map_or(0, Entity::to_bits),
        )
    });

    let current_epoch = world
        .get_resource::<WorldEpoch>()
        .copied()
        .unwrap_or_default()
        .get();
    for request in requests {
        let result = apply_designation_request(world, current_epoch, request);
        world
            .resource_mut::<Messages<DeconstructionDesignationOutcome>>()
            .write(DeconstructionDesignationOutcome {
                request_id: request.request_id,
                hit: request.hit,
                result,
            });
    }
    world.flush();
}

fn apply_designation_request(
    world: &mut World,
    current_epoch: u64,
    request: DeconstructionDesignationRequest,
) -> DeconstructionDesignationResult {
    if request.world_epoch != current_epoch {
        return rejected(DeconstructionDesignationRejectReason::StaleWorld);
    }
    let Some(hit) = request.hit else {
        return rejected(DeconstructionDesignationRejectReason::NoTarget);
    };

    let resolved = match resolve_deconstruction_target(world, hit) {
        Ok(resolved) => resolved,
        Err(reason) => {
            return rejected(DeconstructionDesignationRejectReason::Target(reason));
        }
    };
    if !designation_target_shape_is_supported(world, resolved) {
        return rejected(DeconstructionDesignationRejectReason::CleanupUnavailable);
    }

    let facts = deconstruction_eligibility_facts(world, resolved.root);
    let resolved = match evaluate_deconstruction_target(world, hit, facts) {
        Ok(resolved) => resolved,
        Err(reason) => {
            return rejected(DeconstructionDesignationRejectReason::Target(reason));
        }
    };
    let Some(anchor) = world
        .get::<Transform>(resolved.root)
        .map(|transform| transform.translation)
    else {
        return rejected(DeconstructionDesignationRejectReason::Target(
            hw_jobs::DeconstructionRejectReason::OwnerMismatch,
        ));
    };

    let order = world
        .spawn((
            Name::new("DeconstructionOrder"),
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            PlayerIssuedDesignation,
            Priority(0),
            TaskSlots::new(1),
            TargetDeconstructionRoot(resolved.root),
            Transform::from_translation(anchor),
        ))
        .id();
    world.flush();
    world
        .entity_mut(resolved.root)
        .insert(DeconstructionPending { order });

    DeconstructionDesignationResult::Designated {
        order,
        target: resolved.root,
        class: resolved.class,
    }
}

const fn rejected(
    reason: DeconstructionDesignationRejectReason,
) -> DeconstructionDesignationResult {
    DeconstructionDesignationResult::Rejected(reason)
}

pub(super) fn designation_target_shape_is_supported(
    world: &World,
    resolved: ResolvedDeconstructionTarget,
) -> bool {
    let kind = resolved.class.building_type();
    if !supports_deconstruction_cleanup(kind) {
        return false;
    }

    let root = resolved.root;
    let has_water_storage = world
        .get::<Stockpile>(root)
        .is_some_and(|stockpile| stockpile.resource_type == Some(ResourceType::Water));
    let has_mixer = world.get::<MudMixerStorage>(root).is_some();
    let has_rest_area = world.get::<RestArea>(root).is_some();
    let has_parking = world.get::<WheelbarrowParking>(root).is_some();
    let has_sand_pile = world.get::<SandPile>(root).is_some();
    let has_bone_pile = world.get::<BonePile>(root).is_some();

    deconstruction_marker_matches(
        kind,
        DeconstructionTargetMarkers {
            water_storage: has_water_storage,
            mud_mixer_storage: has_mixer,
            rest_area: has_rest_area,
            wheelbarrow_parking: has_parking,
            sand_pile: has_sand_pile,
            bone_pile: has_bone_pile,
            door: world.get::<Door>(root).is_some(),
            bridge: world.get::<BridgeMarker>(root).is_some(),
            operational_soul_spa: world
                .get::<SoulSpaSite>(root)
                .is_some_and(|site| site.phase == SoulSpaPhase::Operational),
            power_consumer: world.get::<PowerConsumer>(root).is_some(),
            power_generator: world.get::<PowerGenerator>(root).is_some(),
        },
    )
}

fn deconstruction_eligibility_facts(
    world: &mut World,
    target: Entity,
) -> DeconstructionEligibilityFacts {
    let move_task_targets_root = {
        let mut query = world.query::<&MovePlantTask>();
        query.iter(world).any(|task| task.building == target)
    };
    let assigned_move_targets_root = {
        let mut query = world.query::<&AssignedTask>();
        query
            .iter(world)
            .any(|task| matches!(task, AssignedTask::MovePlant(data) if data.building == target))
    };
    DeconstructionEligibilityFacts {
        move_task_targets_root,
        assigned_move_targets_root,
        pending_building_move: world.get::<PendingBuildingMove>(target).is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::{Building, BuildingType, DeconstructionOrders, DeconstructionRejectReason};
    use std::collections::HashSet;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DeconstructionDesignationRequest>()
            .add_message::<DeconstructionDesignationOutcome>()
            .insert_resource(WorldEpoch::default())
            .add_systems(Update, deconstruction_designation_system);
        app
    }

    fn spawn_pile(world: &mut World, kind: BuildingType, pos: Vec2) -> Entity {
        let mut root = world.spawn((
            Building {
                kind,
                is_provisional: false,
            },
            Transform::from_translation(pos.extend(0.0)),
        ));
        match kind {
            BuildingType::SandPile => {
                root.insert(SandPile);
            }
            BuildingType::BonePile => {
                root.insert(BonePile);
            }
            _ => unreachable!(),
        }
        root.id()
    }

    fn outcomes(app: &App) -> Vec<DeconstructionDesignationOutcome> {
        app.world()
            .resource::<Messages<DeconstructionDesignationOutcome>>()
            .iter_current_update_messages()
            .copied()
            .collect()
    }

    #[test]
    fn one_click_creates_a_dedicated_order_without_overwriting_target_designation() {
        let mut app = app();
        let target = spawn_pile(app.world_mut(), BuildingType::BonePile, Vec2::splat(32.0));
        app.world_mut().entity_mut(target).insert(Designation {
            work_type: WorkType::CollectBone,
        });
        app.world_mut()
            .write_message(DeconstructionDesignationRequest {
                request_id: 7,
                world_epoch: 0,
                hit: Some(target),
            });

        app.update();

        assert_eq!(
            app.world().get::<Designation>(target).unwrap().work_type,
            WorkType::CollectBone
        );
        let pending = *app.world().get::<DeconstructionPending>(target).unwrap();
        assert_ne!(pending.order, target);
        assert_eq!(
            app.world()
                .get::<Designation>(pending.order)
                .unwrap()
                .work_type,
            WorkType::Deconstruct
        );
        assert_eq!(
            app.world()
                .get::<TargetDeconstructionRoot>(pending.order)
                .map(|relation| relation.0),
            Some(target)
        );
        assert_eq!(
            app.world()
                .get::<DeconstructionOrders>(target)
                .unwrap()
                .iter()
                .copied()
                .collect::<HashSet<_>>(),
            HashSet::from([pending.order])
        );
        assert!(matches!(
            outcomes(&app).as_slice(),
            [DeconstructionDesignationOutcome {
                request_id: 7,
                result: DeconstructionDesignationResult::Designated {
                    order,
                    target: result_target,
                    class: DeconstructionTargetClass::Building(BuildingType::BonePile),
                },
                ..
            }] if *order == pending.order && *result_target == target
        ));
    }

    #[test]
    fn same_batch_duplicate_clicks_return_one_success_and_one_typed_rejection() {
        let mut app = app();
        let target = spawn_pile(app.world_mut(), BuildingType::SandPile, Vec2::ZERO);
        for request_id in [20, 10] {
            app.world_mut()
                .write_message(DeconstructionDesignationRequest {
                    request_id,
                    world_epoch: 0,
                    hit: Some(target),
                });
        }

        app.update();

        let outcomes = outcomes(&app);
        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].request_id, 10);
        assert!(matches!(
            outcomes[0].result,
            DeconstructionDesignationResult::Designated { .. }
        ));
        assert_eq!(outcomes[1].request_id, 20);
        assert_eq!(
            outcomes[1].result,
            DeconstructionDesignationResult::Rejected(
                DeconstructionDesignationRejectReason::Target(
                    DeconstructionRejectReason::AlreadyDesignated
                )
            )
        );
        assert_eq!(
            app.world()
                .get::<DeconstructionOrders>(target)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn stale_world_and_unsupported_cleanup_are_non_mutating() {
        let mut app = app();
        let target = app
            .world_mut()
            .spawn((
                Building {
                    kind: BuildingType::Door,
                    is_provisional: false,
                },
                Transform::default(),
            ))
            .id();
        app.world_mut()
            .write_message(DeconstructionDesignationRequest {
                request_id: 1,
                world_epoch: 9,
                hit: Some(target),
            });
        app.world_mut()
            .write_message(DeconstructionDesignationRequest {
                request_id: 2,
                world_epoch: 0,
                hit: Some(target),
            });

        app.update();

        assert!(app.world().get::<DeconstructionPending>(target).is_none());
        assert_eq!(
            outcomes(&app)
                .iter()
                .map(|outcome| outcome.result)
                .collect::<Vec<_>>(),
            vec![
                DeconstructionDesignationResult::Rejected(
                    DeconstructionDesignationRejectReason::CleanupUnavailable
                ),
                DeconstructionDesignationResult::Rejected(
                    DeconstructionDesignationRejectReason::StaleWorld
                ),
            ]
        );
    }
}
