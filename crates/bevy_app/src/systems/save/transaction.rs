//! DynamicWorld の preflight と live world 置換transaction。
//!
//! staging preflight は reflect registry と `write_to_world_with` の静的契約だけを
//! 検証する。live commit は別であり、write開始後の失敗時には同じsave schemaから
//! 取得したrollback snapshotを復元する。

use std::fmt;

use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;
use bevy::reflect::TypeRegistry;
use bevy_world_serialization::DynamicWorld;

use super::rehydrate::{ResolvedRehydratePlan, clear_rehydrate_presentation};
use super::reset::{advance_world_epoch, discard_old_removed_components, run_load_resets};
use super::schema::{build_persisted_world, collect_persisted_entities, validate_persisted_world};
use super::state::SaveRecoveryMode;

#[derive(Debug)]
pub(super) struct PreflightError(String);

impl fmt::Display for PreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "DynamicWorld cannot be applied to a staging world: {}",
            self.0
        )
    }
}

/// Applies the dynamic world to an isolated staging world. A successful result
/// proves reflected type/component/resource registration only; it makes no
/// claim about live-world reset, rehydrate, or runtime cache prerequisites.
pub(super) fn preflight_dynamic_world(
    dynamic_world: &DynamicWorld,
    type_registry: &TypeRegistry,
) -> Result<World, PreflightError> {
    let mut staging = World::new();
    let mut entity_map = EntityHashMap::default();
    dynamic_world
        .write_to_world_with(&mut staging, &mut entity_map, type_registry)
        .map_err(|error| PreflightError(error.to_string()))?;
    staging.flush();
    Ok(staging)
}

#[derive(Debug)]
pub(super) enum CommitError {
    Rejected {
        candidate: &'static str,
        cause: String,
    },
    RecoveryModeRequired,
    Recovered {
        cause: String,
    },
    RecoveryFailed {
        cause: String,
        recovery: String,
    },
}

impl fmt::Display for CommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { candidate, cause } => write!(
                formatter,
                "{candidate} candidate was rejected before live replacement: {cause}"
            ),
            Self::RecoveryModeRequired => write!(
                formatter,
                "requested world replacement mode is invalid for the current recovery state"
            ),
            Self::Recovered { cause } => write!(
                formatter,
                "live apply failed ({cause}); restored the persisted rollback snapshot"
            ),
            Self::RecoveryFailed { cause, recovery } => write!(
                formatter,
                "world replacement failed ({cause}); recovery did not complete ({recovery})"
            ),
        }
    }
}

/// Replaces the durable world and runs the same finalizer for a successful
/// load and a recovered rollback. The no-op post-write check is kept separate
/// so tests can inject a failure after live apply has started.
pub(super) fn replace_persisted_world(
    world: &mut World,
    incoming: &DynamicWorld,
    type_registry: &TypeRegistry,
    plan: &ResolvedRehydratePlan,
) -> Result<(), CommitError> {
    replace_persisted_world_with_post_write(
        world,
        incoming,
        type_registry,
        plan,
        |_| Ok(()),
        |world| {
            plan.run(world);
            Ok(())
        },
    )
}

fn replace_persisted_world_with_post_write(
    world: &mut World,
    incoming: &DynamicWorld,
    type_registry: &TypeRegistry,
    plan: &ResolvedRehydratePlan,
    mut post_write: impl FnMut(&mut World) -> Result<(), String>,
    mut finalize: impl FnMut(&mut World) -> Result<(), String>,
) -> Result<(), CommitError> {
    if recovery_mode(world) != SaveRecoveryMode::Healthy {
        return Err(CommitError::RecoveryModeRequired);
    }
    validate_dynamic_candidate(incoming, type_registry, plan, "incoming")?;
    let rollback_snapshot = capture_persisted_world(world, type_registry);
    validate_dynamic_candidate(&rollback_snapshot, type_registry, plan, "rollback")?;

    run_load_resets(world);
    clear_rehydrate_presentation(world);
    despawn_persisted_entities(world);
    advance_world_epoch(world);
    discard_old_removed_components(world);

    let mut incoming_entity_map = EntityHashMap::default();
    let apply_error = incoming
        .write_to_world_with(world, &mut incoming_entity_map, type_registry)
        .map_err(|error| error.to_string())
        .and_then(|()| post_write(world))
        .and_then(|()| finalize(world));

    if let Err(cause) = apply_error {
        let recovery = recover_persisted_world(
            world,
            &rollback_snapshot,
            type_registry,
            incoming_entity_map,
            cause,
            &mut finalize,
        );
        if matches!(recovery, Err(CommitError::RecoveryFailed { .. })) {
            enter_recovery_failed(world);
        }
        return recovery;
    }

    world.flush();
    Ok(())
}

/// Replaces an untrusted live world without attempting to snapshot it. This is
/// deliberately available only after transactional rollback has failed.
pub(super) fn replace_recovery_only_world(
    world: &mut World,
    incoming: &DynamicWorld,
    type_registry: &TypeRegistry,
    plan: &ResolvedRehydratePlan,
) -> Result<(), CommitError> {
    replace_recovery_only_world_with_post_write(world, incoming, type_registry, plan, |_| Ok(()))
}

fn replace_recovery_only_world_with_post_write(
    world: &mut World,
    incoming: &DynamicWorld,
    type_registry: &TypeRegistry,
    plan: &ResolvedRehydratePlan,
    mut post_write: impl FnMut(&mut World) -> Result<(), String>,
) -> Result<(), CommitError> {
    if recovery_mode(world) != SaveRecoveryMode::RecoveryFailed {
        return Err(CommitError::RecoveryModeRequired);
    }
    validate_dynamic_candidate(incoming, type_registry, plan, "recovery")?;

    run_load_resets(world);
    clear_rehydrate_presentation(world);
    despawn_persisted_entities(world);
    advance_world_epoch(world);
    discard_old_removed_components(world);

    let mut entity_map = EntityHashMap::default();
    let apply_result = incoming
        .write_to_world_with(world, &mut entity_map, type_registry)
        .map_err(|error| error.to_string())
        .and_then(|()| post_write(world));
    if let Err(error) = apply_result {
        despawn_mapped_entities(world, entity_map.values().copied());
        enter_recovery_failed(world);
        return Err(CommitError::RecoveryFailed {
            cause: "recovery-only apply failed".to_owned(),
            recovery: error,
        });
    }

    plan.run(world);
    world.flush();
    set_recovery_mode(world, SaveRecoveryMode::Healthy);
    Ok(())
}

fn validate_dynamic_candidate(
    dynamic_world: &DynamicWorld,
    type_registry: &TypeRegistry,
    plan: &ResolvedRehydratePlan,
    candidate: &'static str,
) -> Result<(), CommitError> {
    validate_persisted_world(dynamic_world).map_err(|error| CommitError::Rejected {
        candidate,
        cause: error.to_string(),
    })?;
    let staging = preflight_dynamic_world(dynamic_world, type_registry).map_err(|error| {
        CommitError::Rejected {
            candidate,
            cause: error.to_string(),
        }
    })?;
    plan.validate_candidate(&staging)
        .map_err(|cause| CommitError::Rejected { candidate, cause })
}

fn capture_persisted_world(world: &mut World, type_registry: &TypeRegistry) -> DynamicWorld {
    let entities = collect_persisted_entities(world);
    build_persisted_world(world, type_registry, entities.into_iter())
}

fn despawn_persisted_entities(world: &mut World) {
    let entities = collect_persisted_entities(world);
    for entity in entities {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }
    world.flush();
}

fn recover_persisted_world(
    world: &mut World,
    rollback_snapshot: &DynamicWorld,
    type_registry: &TypeRegistry,
    incoming_entity_map: EntityHashMap<Entity>,
    cause: String,
    finalize: &mut impl FnMut(&mut World) -> Result<(), String>,
) -> Result<(), CommitError> {
    // `write_to_world_with` allocates all target entities before it starts
    // applying components. Remove those ids directly so a partially applied
    // entity without its root marker cannot survive the recovery path.
    for entity in incoming_entity_map.values().copied() {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }
    world.flush();
    // Recovery is a second world-replacement phase for the partially applied
    // payload. Re-run registered resets before the rollback DynamicWorld is
    // written so a fallible finalizer cannot leak owner caches or requests.
    run_load_resets(world);
    // A fallible finalizer may have spawned presentation shells before it
    // returned. They are not in the DynamicWorld entity map, so clean this
    // narrowly-owned set before the rollback finalizer recreates it.
    clear_rehydrate_presentation(world);
    world.flush();
    discard_old_removed_components(world);

    let mut rollback_entity_map = EntityHashMap::default();
    if let Err(error) =
        rollback_snapshot.write_to_world_with(world, &mut rollback_entity_map, type_registry)
    {
        despawn_mapped_entities(world, rollback_entity_map.values().copied());
        return Err(CommitError::RecoveryFailed {
            cause,
            recovery: error.to_string(),
        });
    }

    if let Err(error) = finalize(world) {
        return Err(CommitError::RecoveryFailed {
            cause,
            recovery: error,
        });
    }

    world.flush();

    Err(CommitError::Recovered { cause })
}

fn despawn_mapped_entities(world: &mut World, entities: impl Iterator<Item = Entity>) {
    for entity in entities {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }
    world.flush();
}

fn recovery_mode(world: &World) -> SaveRecoveryMode {
    world
        .get_resource::<SaveRecoveryMode>()
        .copied()
        .unwrap_or_default()
}

fn set_recovery_mode(world: &mut World, mode: SaveRecoveryMode) {
    if world.contains_resource::<SaveRecoveryMode>() {
        *world.resource_mut::<SaveRecoveryMode>() = mode;
    } else {
        world.insert_resource(mode);
    }
}

fn enter_recovery_failed(world: &mut World) {
    set_recovery_mode(world, SaveRecoveryMode::RecoveryFailed);
    if !world.contains_resource::<Time<Virtual>>() {
        world.insert_resource(Time::<Virtual>::default());
    }
    world.resource_mut::<Time<Virtual>>().pause();
}

#[cfg(test)]
mod tests {
    use bevy::ecs::reflect::AppTypeRegistry;
    use bevy::reflect::Reflect;

    use hw_core::GameTime;
    use hw_core::familiar::{
        Familiar, FamiliarOperation, FamiliarPolicy, FamiliarWorkPriority, FamiliarWorkRule,
    };
    use hw_core::jobs::WorkType;
    use hw_core::population::PopulationManager;
    use hw_core::relationships::{CommandedBy, Commanding};
    use hw_core::selection::SelectedEntity;
    use hw_core::soul::{DamnedSoul, DreamPool};
    use hw_jobs::Building;
    use hw_world::{Room, RoomBounds, RoomOverlayTile, WorldMap};

    use super::*;
    use crate::systems::save::rehydrate::validate_familiar_candidate;
    use crate::systems::save::schema::{
        build_persisted_world, collect_persisted_entities, register_save_types,
    };
    use crate::test_support::minimal_app;

    #[derive(Reflect)]
    struct ReflectedButNotAComponent;

    #[derive(Resource, Default)]
    struct LifecycleReceipt {
        removed: Vec<Entity>,
        added: usize,
        changed: usize,
    }

    #[derive(Resource, Default)]
    struct ResetCount(usize);

    #[derive(Resource, Default)]
    struct MutationTrace(Vec<&'static str>);

    fn count_reset(world: &mut World) {
        world.resource_mut::<ResetCount>().0 += 1;
    }

    fn record_rehydrate_step(world: &mut World) {
        world.resource_mut::<MutationTrace>().0.push("rehydrate");
    }

    fn spawn_runtime_room(world: &mut World) -> (Entity, Entity) {
        let room = world
            .spawn(Room {
                tiles: vec![(1, 1)],
                wall_tiles: Vec::new(),
                door_tiles: Vec::new(),
                bounds: RoomBounds {
                    min_x: 1,
                    max_x: 1,
                    min_y: 1,
                    max_y: 1,
                },
                tile_count: 1,
            })
            .id();
        let overlay = world.spawn(RoomOverlayTile { grid_pos: (1, 1) }).id();
        (room, overlay)
    }

    fn observe_replacement_lifecycle(
        mut removed: RemovedComponents<DamnedSoul>,
        added: Query<Entity, Added<DamnedSoul>>,
        changed: Query<Entity, Changed<DamnedSoul>>,
        mut receipt: ResMut<LifecycleReceipt>,
    ) {
        receipt.removed.extend(removed.read());
        receipt.added += added.iter().count();
        receipt.changed += changed.iter().count();
    }

    fn app_with_save_schema() -> App {
        let mut app = App::new();
        register_save_types(&mut app);
        app
    }

    fn insert_persisted_resources(world: &mut World, seconds: f32) {
        world.insert_resource(GameTime {
            seconds,
            ..default()
        });
        world.insert_resource(DreamPool::default());
        world.insert_resource(PopulationManager::default());
        world.insert_resource(WorldMap::default());
    }

    fn capture_from_app(app: &mut App) -> DynamicWorld {
        let entities = collect_persisted_entities(app.world_mut());
        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        build_persisted_world(app.world(), &registry, entities.into_iter())
    }

    #[test]
    fn preflight_failure_leaves_the_live_world_unchanged() {
        let mut app = App::empty();
        app.init_resource::<AppTypeRegistry>();
        app.register_type::<ReflectedButNotAComponent>();
        let existing = app.world_mut().spawn(DamnedSoul::default()).id();

        let dynamic_world = DynamicWorld {
            resources: Vec::new(),
            entities: vec![bevy_world_serialization::DynamicEntity {
                entity: Entity::PLACEHOLDER,
                components: vec![Box::new(ReflectedButNotAComponent)],
            }],
        };
        let type_registry = app.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();

        assert!(preflight_dynamic_world(&dynamic_world, &registry).is_err());
        assert!(app.world().get_entity(existing).is_ok());
        assert!(app.world().get::<DamnedSoul>(existing).is_some());
    }

    #[test]
    fn injected_post_write_failure_restores_the_persisted_snapshot() {
        let mut live = app_with_save_schema();
        insert_persisted_resources(live.world_mut(), 1.0);
        let familiar = live.world_mut().spawn(Familiar::default()).id();
        live.world_mut()
            .spawn((DamnedSoul::default(), CommandedBy(familiar)));
        let building = live.world_mut().spawn(Building::default()).id();
        live.world_mut()
            .resource_mut::<WorldMap>()
            .set_building((3, 4), building);
        live.world_mut().flush();

        let mut incoming_source = app_with_save_schema();
        insert_persisted_resources(incoming_source.world_mut(), 99.0);
        incoming_source.world_mut().spawn(DamnedSoul {
            laziness: 0.25,
            ..default()
        });
        let incoming = capture_from_app(&mut incoming_source);

        let type_registry = live.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        preflight_dynamic_world(&incoming, &registry).unwrap();

        let mut finalize_count = 0;
        let plan = ResolvedRehydratePlan::default();
        let result = replace_persisted_world_with_post_write(
            live.world_mut(),
            &incoming,
            &registry,
            &plan,
            |world| {
                world.spawn(hw_visual::visual3d::SoulProxy3d {
                    owner: Entity::PLACEHOLDER,
                    billboard: false,
                });
                Err("injected failure after DynamicWorld write".to_string())
            },
            |_| {
                finalize_count += 1;
                Ok(())
            },
        );

        assert!(matches!(result, Err(CommitError::Recovered { .. })));
        assert_eq!(finalize_count, 1);
        assert_eq!(live.world().resource::<GameTime>().seconds, 1.0);

        let souls: Vec<_> = live
            .world_mut()
            .query_filtered::<Entity, With<DamnedSoul>>()
            .iter(live.world())
            .collect();
        assert_eq!(souls.len(), 1);
        let restored_soul = souls[0];
        assert_eq!(
            live.world()
                .get::<DamnedSoul>(restored_soul)
                .unwrap()
                .laziness,
            DamnedSoul::default().laziness
        );

        let familiars: Vec<_> = live
            .world_mut()
            .query_filtered::<Entity, With<Familiar>>()
            .iter(live.world())
            .collect();
        assert_eq!(familiars.len(), 1);
        let restored_familiar = familiars[0];
        assert_eq!(
            live.world().get::<CommandedBy>(restored_soul).unwrap().0,
            restored_familiar
        );
        assert!(
            live.world()
                .get::<Commanding>(restored_familiar)
                .unwrap()
                .iter()
                .any(|entity| *entity == restored_soul)
        );

        let restored_building = live
            .world()
            .resource::<WorldMap>()
            .building_entity((3, 4))
            .unwrap();
        assert!(live.world().get::<Building>(restored_building).is_some());
        assert_eq!(
            live.world_mut()
                .query_filtered::<Entity, With<hw_visual::visual3d::SoulProxy3d>>()
                .iter(live.world())
                .count(),
            0
        );
    }

    #[test]
    fn normal_apply_and_rollback_run_the_same_immutable_plan_once() {
        let mut live = app_with_save_schema();
        insert_persisted_resources(live.world_mut(), 1.0);
        live.world_mut().spawn(DamnedSoul::default());
        live.init_resource::<MutationTrace>();

        let mut incoming_source = app_with_save_schema();
        insert_persisted_resources(incoming_source.world_mut(), 2.0);
        incoming_source.world_mut().spawn(DamnedSoul {
            laziness: 0.25,
            ..default()
        });
        let incoming = capture_from_app(&mut incoming_source);
        let type_registry = live.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let plan =
            ResolvedRehydratePlan::with_step_for_test("test.rehydrate", record_rehydrate_step);

        replace_persisted_world(live.world_mut(), &incoming, &registry, &plan).unwrap();
        assert_eq!(
            live.world().resource::<MutationTrace>().0,
            vec!["rehydrate"]
        );
        live.world_mut().resource_mut::<MutationTrace>().0.clear();

        let rollback_plan = plan.clone();
        let result = replace_persisted_world_with_post_write(
            live.world_mut(),
            &incoming,
            &registry,
            &plan,
            |_| Err("injected post-write failure".to_owned()),
            move |world| {
                rollback_plan.run(world);
                Ok(())
            },
        );

        assert!(matches!(result, Err(CommitError::Recovered { .. })));
        assert_eq!(
            live.world().resource::<MutationTrace>().0,
            vec!["rehydrate"]
        );
    }

    #[test]
    fn room_runtime_reset_covers_normal_rollback_and_recovery_only_replacement() {
        let mut live = app_with_save_schema();
        insert_persisted_resources(live.world_mut(), 1.0);
        live.world_mut().spawn(DamnedSoul::default());
        super::super::register_load_reset_hook(
            &mut live,
            "hw-world-rooms",
            hw_world::reset_for_world_replace,
        );

        let mut incoming_source = app_with_save_schema();
        insert_persisted_resources(incoming_source.world_mut(), 2.0);
        incoming_source.world_mut().spawn(DamnedSoul::default());
        let incoming = capture_from_app(&mut incoming_source);
        let type_registry = live.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let plan = ResolvedRehydratePlan::default();

        let normal_room = spawn_runtime_room(live.world_mut());
        replace_persisted_world(live.world_mut(), &incoming, &registry, &plan).unwrap();
        for entity in [normal_room.0, normal_room.1] {
            assert!(live.world().get_entity(entity).is_err());
        }

        let mut partial_room = None;
        let rollback_result = replace_persisted_world_with_post_write(
            live.world_mut(),
            &incoming,
            &registry,
            &plan,
            |world| {
                partial_room = Some(spawn_runtime_room(world));
                Err("injected post-write failure after room spawn".to_owned())
            },
            |_| Ok(()),
        );
        assert!(matches!(
            rollback_result,
            Err(CommitError::Recovered { .. })
        ));
        for entity in [partial_room.unwrap().0, partial_room.unwrap().1] {
            assert!(live.world().get_entity(entity).is_err());
        }

        let recovery_room = spawn_runtime_room(live.world_mut());
        live.insert_resource(SaveRecoveryMode::RecoveryFailed);
        replace_recovery_only_world(live.world_mut(), &incoming, &registry, &plan).unwrap();
        for entity in [recovery_room.0, recovery_room.1] {
            assert!(live.world().get_entity(entity).is_err());
        }
    }

    #[test]
    fn invalid_saved_familiar_roster_is_rejected_before_live_replacement() {
        let mut live = app_with_save_schema();
        insert_persisted_resources(live.world_mut(), 1.0);
        let mut expected_policy = FamiliarPolicy::default();
        expected_policy.set_rule(
            WorkType::Haul,
            FamiliarWorkRule {
                allowed: false,
                priority: FamiliarWorkPriority::High,
            },
        );
        let expected_operation = FamiliarOperation {
            fatigue_threshold: 0.7,
            max_controlled_soul: 3,
        };
        let live_familiar = live
            .world_mut()
            .spawn((
                Familiar::default(),
                expected_operation.clone(),
                expected_policy.clone(),
            ))
            .id();
        live.world_mut()
            .spawn((DamnedSoul::default(), CommandedBy(live_familiar)));
        live.world_mut().flush();
        live.init_resource::<hw_core::WorldEpoch>();
        live.insert_resource(SelectedEntity(Some(live_familiar)));
        let info_panel = live
            .world_mut()
            .spawn((
                Node {
                    display: Display::Flex,
                    ..default()
                },
                hw_ui::components::InfoPanel,
            ))
            .id();

        let mut incoming_source = app_with_save_schema();
        insert_persisted_resources(incoming_source.world_mut(), 99.0);
        let incoming_familiar = incoming_source
            .world_mut()
            .spawn((
                Familiar::default(),
                FamiliarOperation {
                    fatigue_threshold: 0.2,
                    max_controlled_soul: 1,
                },
                FamiliarPolicy::default(),
            ))
            .id();
        for _ in 0..2 {
            incoming_source
                .world_mut()
                .spawn((DamnedSoul::default(), CommandedBy(incoming_familiar)));
        }
        incoming_source.world_mut().flush();
        let incoming = capture_from_app(&mut incoming_source);

        let type_registry = live.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let plan = ResolvedRehydratePlan::with_validator_for_test(
            "familiar.roster",
            validate_familiar_candidate,
        );
        let result = replace_persisted_world(live.world_mut(), &incoming, &registry, &plan);

        assert!(matches!(
            result,
            Err(CommitError::Rejected {
                candidate: "incoming",
                ref cause,
            }) if cause.contains("roster contains 2")
        ));
        assert_eq!(live.world().resource::<GameTime>().seconds, 1.0);
        assert_eq!(live.world().resource::<hw_core::WorldEpoch>().get(), 0);
        assert_eq!(
            live.world().resource::<SelectedEntity>().0,
            Some(live_familiar)
        );
        assert_eq!(
            live.world().get::<Node>(info_panel).unwrap().display,
            Display::Flex
        );

        let restored_familiars: Vec<_> = live
            .world_mut()
            .query_filtered::<Entity, With<Familiar>>()
            .iter(live.world())
            .collect();
        assert_eq!(restored_familiars.len(), 1);
        let restored_familiar = restored_familiars[0];
        assert_eq!(
            live.world().get::<FamiliarOperation>(restored_familiar),
            Some(&expected_operation)
        );
        assert_eq!(
            live.world().get::<FamiliarPolicy>(restored_familiar),
            Some(&expected_policy)
        );
        assert_eq!(
            live.world()
                .get::<Commanding>(restored_familiar)
                .unwrap()
                .iter()
                .count(),
            1
        );
        let restored_soul = live
            .world_mut()
            .query_filtered::<Entity, With<DamnedSoul>>()
            .single(live.world())
            .unwrap();
        assert_eq!(
            live.world().get::<CommandedBy>(restored_soul).unwrap().0,
            restored_familiar
        );
    }

    #[test]
    fn invalid_rollback_candidate_is_rejected_before_any_reset_or_epoch_change() {
        let mut live = app_with_save_schema();
        insert_persisted_resources(live.world_mut(), 1.0);
        live.init_resource::<ResetCount>();
        live.init_resource::<hw_core::WorldEpoch>();
        super::super::register_load_reset_hook(&mut live, "test-count", count_reset);
        let familiar = live
            .world_mut()
            .spawn((
                Familiar::default(),
                FamiliarOperation {
                    max_controlled_soul: 1,
                    ..default()
                },
            ))
            .id();
        live.world_mut()
            .spawn((DamnedSoul::default(), CommandedBy(familiar)));
        live.world_mut()
            .spawn((DamnedSoul::default(), CommandedBy(familiar)));
        live.world_mut().flush();

        let mut incoming_source = app_with_save_schema();
        insert_persisted_resources(incoming_source.world_mut(), 2.0);
        incoming_source.world_mut().spawn(DamnedSoul::default());
        let incoming = capture_from_app(&mut incoming_source);
        let plan = ResolvedRehydratePlan::with_validator_for_test(
            "familiar.roster",
            validate_familiar_candidate,
        );
        let type_registry = live.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();

        let result = replace_persisted_world(live.world_mut(), &incoming, &registry, &plan);

        assert!(matches!(
            result,
            Err(CommitError::Rejected {
                candidate: "rollback",
                ..
            })
        ));
        assert_eq!(live.world().resource::<ResetCount>().0, 0);
        assert_eq!(live.world().resource::<hw_core::WorldEpoch>().get(), 0);
        assert_eq!(live.world().resource::<GameTime>().seconds, 1.0);
        assert_eq!(
            live.world()
                .get::<Commanding>(familiar)
                .unwrap()
                .iter()
                .count(),
            2
        );
    }

    #[test]
    fn recovered_and_failed_recovery_outcomes_survive_both_transaction_resets() {
        use crate::systems::save::{
            SaveLoadFailureKind, SaveLoadOperation, SaveLoadOutcome, SaveLoadResult, SaveLoadState,
            SavePath,
        };

        let cases = [
            (false, SaveLoadFailureKind::ApplyRecovered),
            (true, SaveLoadFailureKind::RecoveryFailed),
        ];

        for (recovery_fails, expected_failure) in cases {
            let mut live = app_with_save_schema();
            insert_persisted_resources(live.world_mut(), 1.0);
            live.world_mut().spawn(DamnedSoul::default());
            live.add_message::<SaveLoadOutcome>();
            live.insert_resource(SaveLoadState::LoadRequested);
            live.insert_resource(SavePath::new("slot-a.ron"));
            live.insert_resource(SaveRecoveryMode::Healthy);
            live.insert_resource(Time::<Virtual>::default());
            live.init_resource::<ResetCount>();
            super::super::register_load_reset_hook(&mut live, "test-count", count_reset);
            super::super::register_load_reset_hook(
                &mut live,
                "save-load-outcomes",
                super::super::clear_save_load_outcomes,
            );
            live.world_mut().write_message(SaveLoadOutcome {
                operation: SaveLoadOperation::Save,
                target: "old.ron".to_owned(),
                result: SaveLoadResult::Succeeded,
            });

            let mut incoming_source = app_with_save_schema();
            insert_persisted_resources(incoming_source.world_mut(), 99.0);
            incoming_source.world_mut().spawn(DamnedSoul {
                laziness: 0.25,
                ..default()
            });
            let incoming = capture_from_app(&mut incoming_source);
            let type_registry = live.world().resource::<AppTypeRegistry>().clone();
            let plan = ResolvedRehydratePlan::default();

            super::super::save_load_apply_with(
                live.world_mut(),
                |_| panic!("save executor must not run"),
                |world| {
                    let registry = type_registry.read();
                    let result = replace_persisted_world_with_post_write(
                        world,
                        &incoming,
                        &registry,
                        &plan,
                        |_| Err("injected live apply failure".to_owned()),
                        |_| {
                            if recovery_fails {
                                Err("injected recovery finalizer failure".to_owned())
                            } else {
                                Ok(())
                            }
                        },
                    );
                    match result {
                        Ok(()) => SaveLoadResult::Succeeded,
                        Err(error) => {
                            SaveLoadResult::Failed(super::super::load::commit_failure_kind(&error))
                        }
                    }
                },
                |_| panic!("recovery executor must not run for a normal request"),
            );

            assert_eq!(live.world().resource::<ResetCount>().0, 2);
            if recovery_fails {
                assert_eq!(
                    *live.world().resource::<SaveRecoveryMode>(),
                    SaveRecoveryMode::RecoveryFailed
                );
                assert!(live.world().resource::<Time<Virtual>>().is_paused());
            } else {
                assert_eq!(
                    *live.world().resource::<SaveRecoveryMode>(),
                    SaveRecoveryMode::Healthy
                );
            }
            assert_eq!(
                live.world_mut()
                    .resource_mut::<Messages<SaveLoadOutcome>>()
                    .drain()
                    .collect::<Vec<_>>(),
                vec![SaveLoadOutcome {
                    operation: SaveLoadOperation::Load,
                    target: "slot-a.ron".to_owned(),
                    result: SaveLoadResult::Failed(expected_failure),
                }]
            );
        }
    }

    #[test]
    fn recovery_only_retry_is_fail_closed_then_succeeds_without_unpausing() {
        let mut live = app_with_save_schema();
        insert_persisted_resources(live.world_mut(), 1.0);
        live.world_mut().spawn(DamnedSoul::default());
        live.insert_resource(SaveRecoveryMode::RecoveryFailed);
        live.insert_resource(Time::<Virtual>::default());
        live.world_mut().resource_mut::<Time<Virtual>>().pause();

        let mut incoming_source = app_with_save_schema();
        insert_persisted_resources(incoming_source.world_mut(), 7.0);
        incoming_source.world_mut().spawn(DamnedSoul {
            laziness: 0.25,
            ..default()
        });
        let incoming = capture_from_app(&mut incoming_source);
        let type_registry = live.world().resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let plan = ResolvedRehydratePlan::default();

        let first = replace_recovery_only_world_with_post_write(
            live.world_mut(),
            &incoming,
            &registry,
            &plan,
            |_| Err("injected recovery-only post-write failure".to_owned()),
        );
        assert!(matches!(first, Err(CommitError::RecoveryFailed { .. })));
        assert_eq!(
            *live.world().resource::<SaveRecoveryMode>(),
            SaveRecoveryMode::RecoveryFailed
        );
        assert!(live.world().resource::<Time<Virtual>>().is_paused());

        replace_recovery_only_world(live.world_mut(), &incoming, &registry, &plan).unwrap();

        assert_eq!(
            *live.world().resource::<SaveRecoveryMode>(),
            SaveRecoveryMode::Healthy
        );
        assert!(live.world().resource::<Time<Virtual>>().is_paused());
        assert_eq!(live.world().resource::<GameTime>().seconds, 7.0);
        assert_eq!(
            live.world_mut()
                .query_filtered::<Entity, With<DamnedSoul>>()
                .iter(live.world())
                .count(),
            1
        );

        assert!(matches!(
            replace_recovery_only_world(live.world_mut(), &incoming, &registry, &plan),
            Err(CommitError::RecoveryModeRequired)
        ));
    }

    #[test]
    fn replacement_drops_old_removals_and_preserves_new_change_detection() {
        let mut live = minimal_app();
        register_save_types(&mut live);
        insert_persisted_resources(live.world_mut(), 1.0);
        live.world_mut().spawn(DamnedSoul::default());
        live.init_resource::<LifecycleReceipt>();
        live.add_systems(Update, observe_replacement_lifecycle);

        // Initialize the system-local change and removal readers before the
        // replacement, then ignore observations from the original world.
        live.update();
        *live.world_mut().resource_mut::<LifecycleReceipt>() = LifecycleReceipt::default();

        let mut incoming_source = app_with_save_schema();
        insert_persisted_resources(incoming_source.world_mut(), 2.0);
        incoming_source.world_mut().spawn(DamnedSoul {
            laziness: 0.25,
            ..default()
        });
        let incoming = capture_from_app(&mut incoming_source);

        let type_registry = live.world().resource::<AppTypeRegistry>().clone();
        {
            let registry = type_registry.read();
            replace_persisted_world(
                live.world_mut(),
                &incoming,
                &registry,
                &ResolvedRehydratePlan::default(),
            )
            .unwrap();
        }

        live.update();

        let receipt = live.world().resource::<LifecycleReceipt>();
        assert!(receipt.removed.is_empty());
        assert_eq!(receipt.added, 1);
        assert_eq!(receipt.changed, 1);
    }
}
