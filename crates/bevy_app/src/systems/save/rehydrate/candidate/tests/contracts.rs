use super::*;

#[test]
fn durable_topology_wrapper_preserves_construction_before_generic_target_precedence() {
    let mut world = candidate_world();
    let blueprint = world
        .spawn(Blueprint::new(BuildingType::Tank, Vec::new()))
        .id();
    let wrong_target = world.spawn_empty().id();
    world.spawn(TargetMixer(wrong_target));

    assert_eq!(
        validate_durable_topology_candidate(&world).unwrap_err(),
        format!("Blueprint {blueprint:?} has an empty occupied_grids footprint")
    );
}

#[test]
fn familiar_wrapper_preserves_source_before_target_pass_precedence() {
    let mut world = candidate_world();
    let wrong_target = world.spawn_empty().id();
    world.spawn((DamnedSoul::default(), CommandedBy(wrong_target)));
    world.spawn(Commanding::default());
    world.flush();

    assert_eq!(
        validate_familiar_candidate(&world).unwrap_err(),
        format!("CommandedBy target {wrong_target:?} is not a Familiar")
    );
}

#[test]
fn task_logistics_wrapper_preserves_owner_before_container_precedence() {
    let mut world = candidate_world();
    let missing_owner = world.spawn_empty().id();
    world.despawn(missing_owner);
    let source = world
        .spawn((ResourceItem(ResourceType::Wood), BelongsTo(missing_owner)))
        .id();
    world.spawn(MudMixerStorage {
        mud: 1,
        ..default()
    });

    assert_eq!(
        validate_task_logistics_candidate(&world).unwrap_err(),
        format!("BelongsTo source {source:?} references missing owner {missing_owner:?}")
    );
}
