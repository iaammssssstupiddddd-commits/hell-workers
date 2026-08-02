use super::*;
use hw_energy::{SOUL_SPA_MAX_ACTIVE_SLOTS, SoulSpaPhase};

#[test]
fn soul_spa_load_normalization_clamps_only_active_slots() {
    let mut world = World::new();
    let site = world
        .spawn(SoulSpaSite {
            phase: SoulSpaPhase::Operational,
            bones_required: 12,
            bones_delivered: 7,
            active_slots: 99,
        })
        .id();

    rehydrate_soul_spas(&mut world);

    let loaded = world.get::<SoulSpaSite>(site).unwrap();
    assert_eq!(loaded.active_slots, SOUL_SPA_MAX_ACTIVE_SLOTS);
    assert_eq!(loaded.phase, SoulSpaPhase::Operational);
    assert_eq!(loaded.bones_required, 12);
    assert_eq!(loaded.bones_delivered, 7);
}

#[test]
fn soul_spa_load_normalization_preserves_valid_zero_slots() {
    let mut world = World::new();
    let site = world
        .spawn(SoulSpaSite {
            active_slots: 0,
            ..default()
        })
        .id();

    rehydrate_soul_spas(&mut world);

    assert_eq!(world.get::<SoulSpaSite>(site).unwrap().active_slots, 0);
}
