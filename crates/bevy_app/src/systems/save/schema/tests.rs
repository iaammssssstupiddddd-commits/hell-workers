use std::any::TypeId;
use std::collections::HashSet;

use bevy::asset::{AssetPath, LoadFromPath, UntypedHandle};
use bevy::ecs::entity::EntityHashMap;
use bevy::ecs::reflect::{AppTypeRegistry, ReflectComponent, ReflectResource};
use bevy::reflect::{ReflectDeserialize, ReflectSerialize, TypePath, TypeRegistry};
use bevy_world_serialization::serde::WorldDeserializer;
use serde::de::DeserializeSeed;

use super::*;

struct NoAssetLoad;

#[derive(Reflect)]
struct RuntimeOnlyType;

impl LoadFromPath for NoAssetLoad {
    fn load_from_path_erased(
        &mut self,
        _type_id: TypeId,
        _path: AssetPath<'static>,
    ) -> UntypedHandle {
        panic!("schema fixture contains no asset handles")
    }
}

fn registration<T: 'static>(registry: &TypeRegistry) -> &bevy::reflect::TypeRegistration {
    registry.get(TypeId::of::<T>()).unwrap_or_else(|| {
        panic!(
            "{} is missing from the save type registry",
            std::any::type_name::<T>()
        )
    })
}

fn assert_registered<T: 'static>(registry: &TypeRegistry) {
    registration::<T>(registry);
}

fn assert_component_registration<T: 'static>(registry: &TypeRegistry) {
    assert!(
        registration::<T>(registry)
            .data::<ReflectComponent>()
            .is_some(),
        "{} is missing ReflectComponent data",
        std::any::type_name::<T>()
    );
}

fn assert_resource_registration<T: 'static>(registry: &TypeRegistry) {
    let registration = registration::<T>(registry);
    assert!(
        registration.data::<ReflectResource>().is_some(),
        "{} is missing ReflectResource data",
        std::any::type_name::<T>()
    );
    assert!(
        registration.data::<ReflectComponent>().is_some(),
        "{} is missing ReflectComponent data",
        std::any::type_name::<T>()
    );
}

#[test]
fn schema_registers_owned_types_with_their_required_reflect_data() {
    let mut app = App::empty();
    app.init_resource::<AppTypeRegistry>();
    register_save_types(&mut app);

    let registry = app.world().resource::<AppTypeRegistry>().read();

    macro_rules! assert_resource {
        ($type:ty) => {
            assert_resource_registration::<$type>(&registry);
        };
    }
    macro_rules! assert_component {
        ($type:ty) => {
            assert_component_registration::<$type>(&registry);
        };
    }
    macro_rules! assert_dependency {
        ($type:ty) => {
            assert_registered::<$type>(&registry);
        };
    }

    for_each_persisted_resource!(assert_resource);
    for_each_persisted_component!(assert_component);
    for_each_runtime_derived_component!(assert_component);
    for_each_legacy_v0_component!(assert_component);
    for_each_reflect_dependency!(assert_dependency);
    assert!(
        registration::<WorldMap>(&registry)
            .data::<ReflectSerialize>()
            .is_some()
    );
    assert!(
        registration::<WorldMap>(&registry)
            .data::<ReflectDeserialize>()
            .is_some()
    );
    assert!(
        registry.get(TypeId::of::<Transform>()).is_none(),
        "Transform must remain an explicit external-registration dependency"
    );
}

#[test]
fn external_components_exist_in_the_production_registry() {
    let mut app = App::new();
    register_save_types(&mut app);
    let registry = app.world().resource::<AppTypeRegistry>().read();

    macro_rules! assert_external_component {
        ($type:ty) => {
            assert_component_registration::<$type>(&registry);
        };
    }

    for_each_external_registered_component!(assert_external_component);
}

#[test]
fn root_marker_matrix_collects_extracts_and_round_trips_durable_entities() {
    let mut app = App::new();
    register_save_types(&mut app);
    app.world_mut().insert_resource(GameTime {
        seconds: 42.0,
        ..default()
    });

    let (expected_roots, familiar, soul, unmarked_transform, non_root_component) = {
        let world = app.world_mut();
        let familiar = world
            .spawn((Familiar::default(), Transform::default()))
            .id();
        let soul = world
            .spawn((
                DamnedSoul::default(),
                Transform::from_xyz(1.0, 2.0, 0.0),
                CommandedBy(familiar),
            ))
            .id();
        let building_door = world
            .spawn((Building::default(), hw_jobs::Door::default()))
            .id();
        let area = TaskArea::from_points(Vec2::ZERO, Vec2::ONE);
        let mut roots = HashSet::from([
            familiar,
            soul,
            building_door,
            world.spawn(RestArea { capacity: 1 }).id(),
            world.spawn(area.clone()).id(),
            world
                .spawn(Blueprint::new(BuildingType::Wall, Vec::new()))
                .id(),
            world
                .spawn(FloorConstructionSite::new(area.clone(), Vec2::ZERO, 1))
                .id(),
            world
                .spawn(FloorTileBlueprint::new(Entity::PLACEHOLDER, (0, 0)))
                .id(),
            world
                .spawn(WallConstructionSite::new(area, Vec2::ZERO, 1))
                .id(),
            world
                .spawn(WallTileBlueprint::new(Entity::PLACEHOLDER, (0, 0)))
                .id(),
            world.spawn(ResourceItem(ResourceType::Wood)).id(),
            world.spawn(Wheelbarrow { capacity: 1 }).id(),
            world.spawn(WheelbarrowParking { capacity: 1 }).id(),
            world
                .spawn(Stockpile {
                    capacity: 1,
                    resource_type: None,
                })
                .id(),
            world
                .spawn(TransportRequest {
                    kind: TransportRequestKind::DepositToStockpile,
                    anchor: Entity::PLACEHOLDER,
                    resource_type: ResourceType::Wood,
                    issued_by: Entity::PLACEHOLDER,
                    priority: TransportPriority::Normal,
                    stockpile_group: Vec::new(),
                })
                .id(),
            world.spawn(PowerGrid::default()).id(),
            world.spawn(PowerGenerator::default()).id(),
            world.spawn(PowerConsumer { demand: 1.0 }).id(),
            world.spawn(SoulSpaSite::default()).id(),
            world
                .spawn(SoulSpaTile {
                    parent_site: Entity::PLACEHOLDER,
                    grid_pos: (0, 0),
                })
                .id(),
            world.spawn(Tree).id(),
            world.spawn(Rock).id(),
            world.spawn(Tile).id(),
            world
                .spawn(hw_world::zones::Site {
                    min: Vec2::ZERO,
                    max: Vec2::ONE,
                })
                .id(),
            world
                .spawn(hw_world::zones::Yard {
                    min: Vec2::ZERO,
                    max: Vec2::ONE,
                })
                .id(),
        ]);

        let designation = world
            .spawn(Designation {
                work_type: WorkType::default(),
            })
            .id();
        roots.insert(designation);

        let unmarked_transform = world.spawn(Transform::default()).id();
        let non_root_component = world.spawn(IdleState::default()).id();
        world.flush();

        (
            roots,
            familiar,
            soul,
            unmarked_transform,
            non_root_component,
        )
    };

    let target_entities = collect_persisted_entities(app.world_mut());
    let collected: HashSet<_> = target_entities.iter().copied().collect();
    assert_eq!(collected, expected_roots);
    assert!(!collected.contains(&unmarked_transform));
    assert!(!collected.contains(&non_root_component));

    let type_registry = app.world().resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();
    let dynamic_world =
        build_persisted_world(app.world(), &registry, target_entities.iter().copied());
    let extracted: HashSet<_> = dynamic_world
        .entities
        .iter()
        .map(|entity| entity.entity)
        .collect();
    assert_eq!(extracted, expected_roots);

    let has_component = |entity: Entity, type_id: TypeId| {
        dynamic_world
            .entities
            .iter()
            .find(|dynamic_entity| dynamic_entity.entity == entity)
            .is_some_and(|dynamic_entity| {
                dynamic_entity.components.iter().any(|component| {
                    component
                        .get_represented_type_info()
                        .is_some_and(|info| info.type_id() == type_id)
                })
            })
    };
    assert!(has_component(soul, TypeId::of::<Transform>()));
    assert!(has_component(soul, TypeId::of::<CommandedBy>()));
    assert!(has_component(familiar, TypeId::of::<Commanding>()));

    let body = dynamic_world.serialize(&registry).unwrap();
    let mut ron_deserializer = ron::de::Deserializer::from_str(&body).unwrap();
    let round_tripped = WorldDeserializer {
        type_registry: &registry,
        load_from_path: &mut NoAssetLoad,
    }
    .deserialize(&mut ron_deserializer)
    .unwrap();
    drop(registry);

    let mut destination = World::new();
    let mut entity_map = EntityHashMap::default();
    let registry = type_registry.read();
    round_tripped
        .write_to_world_with(&mut destination, &mut entity_map, &registry)
        .unwrap();

    let mapped_familiar = entity_map[&familiar];
    let mapped_soul = entity_map[&soul];
    assert_eq!(
        destination.get::<CommandedBy>(mapped_soul).unwrap().0,
        mapped_familiar
    );
    assert!(
        destination
            .get::<Commanding>(mapped_familiar)
            .unwrap()
            .iter()
            .any(|entity| *entity == mapped_soul)
    );
    assert_eq!(destination.resource::<GameTime>().seconds, 42.0);
}

#[test]
fn gathering_relationships_are_excluded_from_new_saves_and_stripped_from_legacy_bodies() {
    let mut app = App::new();
    register_save_types(&mut app);

    let (soul, gathering_spot) = {
        let world = app.world_mut();
        let gathering_spot = world.spawn(GatheringParticipants::default()).id();
        let soul = world
            .spawn((DamnedSoul::default(), ParticipatingIn(gathering_spot)))
            .id();
        (soul, gathering_spot)
    };

    let type_registry = app.world().resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();
    let dynamic_world = build_persisted_world(app.world(), &registry, std::iter::once(soul));
    let saved_soul = dynamic_world
        .entities
        .iter()
        .find(|entity| entity.entity == soul)
        .expect("soul root must be extracted");
    assert!(saved_soul.components.iter().all(|component| {
        component
            .get_represented_type_info()
            .is_none_or(|info| info.type_id() != TypeId::of::<ParticipatingIn>())
    }));

    let mut legacy_body = DynamicWorld {
        resources: Vec::new(),
        entities: vec![bevy_world_serialization::DynamicEntity {
            entity: soul,
            components: vec![
                Box::new(DamnedSoul::default()),
                Box::new(ParticipatingIn(gathering_spot)),
                Box::new(GatheringParticipants::default()),
            ],
        }],
    };
    discard_runtime_derived_components(&mut legacy_body);

    assert_eq!(legacy_body.entities[0].components.len(), 1);
    assert_eq!(
        legacy_body.entities[0].components[0]
            .get_represented_type_info()
            .unwrap()
            .type_id(),
        TypeId::of::<DamnedSoul>()
    );
}

#[test]
fn reserved_for_task_is_loader_registered_but_excluded_from_v1_schema() {
    let mut app = App::new();
    register_save_types(&mut app);
    let item = app
        .world_mut()
        .spawn((ResourceItem(ResourceType::Wood), ReservedForTask))
        .id();

    let type_registry = app.world().resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();
    assert_component_registration::<ReservedForTask>(&registry);

    let persisted = build_persisted_world(app.world(), &registry, std::iter::once(item));
    let saved_item = persisted
        .entities
        .iter()
        .find(|entity| entity.entity == item)
        .expect("resource item root must be extracted");
    assert!(saved_item.components.iter().all(|component| {
        component
            .get_represented_type_info()
            .is_none_or(|info| info.type_id() != TypeId::of::<ReservedForTask>())
    }));
    let body = persisted.serialize(&registry).unwrap();
    assert!(!body.contains(ReservedForTask::type_path()));
    drop(registry);

    let mut legacy_body = DynamicWorld {
        resources: Vec::new(),
        entities: vec![bevy_world_serialization::DynamicEntity {
            entity: item,
            components: vec![
                Box::new(ResourceItem(ResourceType::Wood)),
                Box::new(ReservedForTask),
            ],
        }],
    };
    let error = validate_persisted_world(&legacy_body).unwrap_err();
    assert!(
        error
            .unsupported_components
            .contains(&ReservedForTask::type_path().to_string())
    );

    discard_legacy_reserved_for_task(&mut legacy_body);
    let error = validate_persisted_world(&legacy_body).unwrap_err();
    assert!(error.unsupported_components.is_empty());
}

#[test]
fn persisted_world_requires_every_schema_resource() {
    let mut app = App::new();
    register_save_types(&mut app);
    app.world_mut().insert_resource(GameTime::default());
    app.world_mut().insert_resource(DreamPool::default());
    app.world_mut()
        .insert_resource(PopulationManager::default());
    app.world_mut().insert_resource(WorldMap::default());

    let type_registry = app.world().resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();
    let dynamic_world = build_persisted_world(app.world(), &registry, std::iter::empty());

    assert!(validate_persisted_world(&dynamic_world).is_ok());
    assert_eq!(
        validate_persisted_world(&DynamicWorld::default())
            .unwrap_err()
            .missing_resources,
        vec![
            std::any::type_name::<GameTime>(),
            std::any::type_name::<DreamPool>(),
            std::any::type_name::<PopulationManager>(),
            std::any::type_name::<WorldMap>(),
        ]
    );
}

#[test]
fn persisted_world_rejects_types_outside_the_schema_allow_lists() {
    let dynamic_world = DynamicWorld {
        resources: vec![Box::new(RuntimeOnlyType)],
        entities: vec![bevy_world_serialization::DynamicEntity {
            entity: Entity::PLACEHOLDER,
            components: vec![Box::new(RuntimeOnlyType)],
        }],
    };

    let error = validate_persisted_world(&dynamic_world).unwrap_err();
    assert_eq!(error.unsupported_resources.len(), 1);
    assert_eq!(error.unsupported_components.len(), 1);
}

#[test]
fn persisted_world_rejects_allowed_components_without_a_root_marker() {
    let dynamic_world = DynamicWorld {
        resources: Vec::new(),
        entities: vec![bevy_world_serialization::DynamicEntity {
            entity: Entity::PLACEHOLDER,
            components: vec![Box::new(Transform::default())],
        }],
    };

    let error = validate_persisted_world(&dynamic_world).unwrap_err();
    assert!(error.unsupported_components.is_empty());
    assert_eq!(error.rootless_entities, vec![Entity::PLACEHOLDER]);
}
