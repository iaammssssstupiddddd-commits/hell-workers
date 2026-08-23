use super::*;

#[test]
fn shell_validation_rejects_missing_transform_soul_prerequisite_and_tree_variant() {
    let mut missing_transform = candidate_world();
    let soul = missing_transform.spawn(DamnedSoul::default()).id();
    assert_eq!(
        validate_shell_candidate(&missing_transform).unwrap_err(),
        format!("DamnedSoul {soul:?} has no Transform")
    );

    let mut missing_prerequisite = candidate_world();
    let soul = missing_prerequisite
        .spawn((DamnedSoul::default(), Transform::default()))
        .id();
    assert_eq!(
        validate_shell_candidate(&missing_prerequisite).unwrap_err(),
        format!("DamnedSoul {soul:?} is missing IdleState, DreamState, or Inventory")
    );

    let mut missing_variant = candidate_world();
    let tree = missing_variant.spawn((Tree, Transform::default())).id();
    assert_eq!(
        validate_shell_candidate(&missing_variant).unwrap_err(),
        format!("Tree {tree:?} has no TreeVariant")
    );
}
