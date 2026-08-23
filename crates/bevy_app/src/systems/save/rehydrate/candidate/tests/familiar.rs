use super::*;

#[test]
fn familiar_roster_validation_is_immutable_and_rejects_saved_overflow() {
    let mut world = candidate_world();
    let familiar = world
        .spawn((
            Familiar::default(),
            FamiliarOperation {
                max_controlled_soul: 1,
                ..default()
            },
        ))
        .id();
    world.spawn((DamnedSoul::default(), CommandedBy(familiar)));
    world.spawn((DamnedSoul::default(), CommandedBy(familiar)));
    world.flush();

    let error = validate_familiar_candidate(&world).unwrap_err();

    assert!(error.contains("roster contains 2"));
    assert_eq!(world.get::<Commanding>(familiar).unwrap().iter().count(), 2);
}

#[test]
fn familiar_validation_rejects_commanded_by_source_and_target_roles() {
    let mut wrong_source = candidate_world();
    let familiar = wrong_source.spawn(Familiar::default()).id();
    let source = wrong_source.spawn(CommandedBy(familiar)).id();
    wrong_source.flush();
    assert_eq!(
        validate_familiar_candidate(&wrong_source).unwrap_err(),
        format!("CommandedBy source {source:?} is not a DamnedSoul")
    );

    let mut wrong_target = candidate_world();
    let target = wrong_target.spawn_empty().id();
    wrong_target.spawn((DamnedSoul::default(), CommandedBy(target)));
    wrong_target.flush();
    assert_eq!(
        validate_familiar_candidate(&wrong_target).unwrap_err(),
        format!("CommandedBy target {target:?} is not a Familiar")
    );
}

#[test]
fn familiar_validation_rejects_commanding_role_duplicate_and_asymmetry() {
    let mut wrong_role = candidate_world();
    let owner = wrong_role.spawn(Commanding::default()).id();
    assert_eq!(
        validate_familiar_candidate(&wrong_role).unwrap_err(),
        format!("Commanding target {owner:?} is not a Familiar")
    );

    let mut duplicate = candidate_world();
    let familiar = duplicate.spawn(Familiar::default()).id();
    let soul = duplicate
        .spawn((DamnedSoul::default(), CommandedBy(familiar)))
        .id();
    duplicate.flush();
    duplicate
        .get_mut::<Commanding>(familiar)
        .unwrap()
        .collection_mut_risky()
        .push(soul);
    assert_eq!(
        validate_familiar_candidate(&duplicate).unwrap_err(),
        format!("Commanding for Familiar {familiar:?} contains duplicate Souls")
    );

    let mut asymmetric = candidate_world();
    let familiar = asymmetric.spawn(Familiar::default()).id();
    asymmetric.spawn((DamnedSoul::default(), CommandedBy(familiar)));
    asymmetric.flush();
    asymmetric
        .get_mut::<Commanding>(familiar)
        .unwrap()
        .collection_mut_risky()
        .clear();
    assert_eq!(
        validate_familiar_candidate(&asymmetric).unwrap_err(),
        format!("CommandedBy/Commanding are not symmetric for Familiar {familiar:?}")
    );
}
