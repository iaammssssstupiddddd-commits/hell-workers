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

use super::rehydrate::clear_rehydrate_presentation;
use super::reset::{advance_world_epoch, discard_old_removed_components, run_load_resets};
use super::schema::{build_persisted_world, collect_persisted_entities};

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
) -> Result<(), PreflightError> {
    let mut staging = World::new();
    let mut entity_map = EntityHashMap::default();
    dynamic_world
        .write_to_world_with(&mut staging, &mut entity_map, type_registry)
        .map_err(|error| PreflightError(error.to_string()))
}

#[derive(Debug)]
pub(super) enum CommitError {
    Recovered { cause: String },
    RecoveryFailed { cause: String, recovery: String },
}

impl fmt::Display for CommitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Recovered { cause } => write!(
                formatter,
                "live apply failed ({cause}); restored the persisted rollback snapshot"
            ),
            Self::RecoveryFailed { cause, recovery } => write!(
                formatter,
                "live apply failed ({cause}) and rollback recovery also failed ({recovery})"
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
    finalize: impl FnMut(&mut World) -> Result<(), String>,
) -> Result<(), CommitError> {
    replace_persisted_world_with_post_write(world, incoming, type_registry, |_| Ok(()), finalize)
}

fn replace_persisted_world_with_post_write(
    world: &mut World,
    incoming: &DynamicWorld,
    type_registry: &TypeRegistry,
    mut post_write: impl FnMut(&mut World) -> Result<(), String>,
    mut finalize: impl FnMut(&mut World) -> Result<(), String>,
) -> Result<(), CommitError> {
    let rollback_snapshot = capture_persisted_world(world, type_registry);
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
        return recover_persisted_world(
            world,
            &rollback_snapshot,
            type_registry,
            incoming_entity_map,
            cause,
            &mut finalize,
        );
    }

    world.flush();
    Ok(())
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

#[cfg(test)]
mod tests {
    use bevy::ecs::reflect::AppTypeRegistry;
    use bevy::reflect::Reflect;

    use hw_core::GameTime;
    use hw_core::familiar::Familiar;
    use hw_core::population::PopulationManager;
    use hw_core::relationships::{CommandedBy, Commanding};
    use hw_core::soul::{DamnedSoul, DreamPool};
    use hw_jobs::Building;
    use hw_world::WorldMap;

    use super::*;
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
        let result = replace_persisted_world_with_post_write(
            live.world_mut(),
            &incoming,
            &registry,
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
            replace_persisted_world(live.world_mut(), &incoming, &registry, |_| Ok(())).unwrap();
        }

        live.update();

        let receipt = live.world().resource::<LifecycleReceipt>();
        assert!(receipt.removed.is_empty());
        assert_eq!(receipt.added, 1);
        assert_eq!(receipt.changed, 1);
    }
}
