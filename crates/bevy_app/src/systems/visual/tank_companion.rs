//! A completed Tank's companion keeps its logical footprint, not its plan tint.
use bevy::prelude::*;
use hw_jobs::{Building, BuildingType};
use hw_logistics::{BelongsTo, BucketStorage, PendingBelongsToBlueprint, Stockpile};

type StorageSprites<'w, 's> = Query<
    'w,
    's,
    (
        &'static BelongsTo,
        Option<&'static PendingBelongsToBlueprint>,
        &'static mut Sprite,
    ),
    (With<BucketStorage>, With<Stockpile>),
>;

/// Run after Update's construction/load/zone-preview commands, even while paused.
/// Retain the Sprite for hit geometry and preview colours. Only the yellow
/// baseline of a positively identified completed Tank companion is suppressed;
/// active red/orange removal feedback, pending plans and ordinary stores remain.
pub(crate) fn clear_completed_tank_planning_tint(
    mut storage: StorageSprites,
    buildings: Query<&Building>,
) {
    let planning_tint = Color::srgba(1.0, 1.0, 0.0, 0.2);
    for (owner, pending, mut sprite) in &mut storage {
        if pending.is_none()
            && sprite.color == planning_tint
            && buildings.get(owner.0).is_ok_and(|building| {
                building.kind == BuildingType::Tank && !building.is_provisional
            })
        {
            sprite.color = Color::NONE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(world: &mut World, owner: Entity) -> Entity {
        world
            .spawn((
                BucketStorage,
                Stockpile {
                    capacity: 10,
                    resource_type: None,
                },
                BelongsTo(owner),
                Sprite {
                    color: Color::srgba(1., 1., 0., 0.2),
                    custom_size: Some(Vec2::splat(32.)),
                    ..default()
                },
                Transform::from_xyz(16., 48., 0.01),
            ))
            .id()
    }

    fn setup() -> (World, Schedule, Entity) {
        let mut world = World::new();
        let owner = world
            .spawn(Building {
                kind: BuildingType::Tank,
                is_provisional: false,
            })
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(clear_completed_tank_planning_tint);
        (world, schedule, owner)
    }

    #[test]
    fn completion_and_existing_loaded_sprite_clear_only_tint_not_storage_or_hit_geometry() {
        let (mut world, mut schedule, owner) = setup();
        let storage = store(&mut world, owner);
        schedule.run(&mut world);
        let sprite = world.get::<Sprite>(storage).unwrap();
        assert_eq!(sprite.color, Color::NONE);
        assert_eq!(sprite.custom_size, Some(Vec2::splat(32.)));
        assert_eq!(
            world.get::<Transform>(storage).unwrap().translation,
            Vec3::new(16., 48., 0.01)
        );
        assert_eq!(world.get::<BelongsTo>(storage).unwrap().0, owner);
        assert_eq!(world.get::<Stockpile>(storage).unwrap().capacity, 10);
        assert!(world.get::<BucketStorage>(storage).is_some());
        // Rehydrate of an old saved store inserts yellow again; no task/time
        // progress is required for the next visual frame to normalize it.
        world.get_mut::<Sprite>(storage).unwrap().color = Color::srgba(1., 1., 0., 0.2);
        schedule.run(&mut world);
        assert_eq!(world.get::<Sprite>(storage).unwrap().color, Color::NONE);
    }

    #[test]
    fn active_removal_colours_remain_and_preview_clear_does_not_restore_yellow() {
        let (mut world, mut schedule, owner) = setup();
        let storage = store(&mut world, owner);
        for color in [
            Color::srgba(1., 0.2, 0.2, 0.4),
            Color::srgba(1., 0.6, 0., 0.4),
        ] {
            world.get_mut::<Sprite>(storage).unwrap().color = color;
            schedule.run(&mut world);
            assert_eq!(world.get::<Sprite>(storage).unwrap().color, color);
        }
        world.get_mut::<Sprite>(storage).unwrap().color = Color::srgba(1., 1., 0., 0.2);
        schedule.run(&mut world);
        assert_eq!(world.get::<Sprite>(storage).unwrap().color, Color::NONE);
    }

    #[test]
    fn pending_ordinary_foreign_and_provisional_stores_keep_their_baseline() {
        let (mut world, mut schedule, owner) = setup();
        let pending = store(&mut world, owner);
        world
            .entity_mut(pending)
            .insert(PendingBelongsToBlueprint(owner));
        let ordinary = store(&mut world, owner);
        world.entity_mut(ordinary).remove::<BucketStorage>();
        let other_owner = world
            .spawn(Building {
                kind: BuildingType::RestArea,
                is_provisional: false,
            })
            .id();
        let foreign = store(&mut world, other_owner);
        let provisional_owner = world
            .spawn(Building {
                kind: BuildingType::Tank,
                is_provisional: true,
            })
            .id();
        let provisional = store(&mut world, provisional_owner);
        let missing_owner = world.spawn_empty().id();
        let missing = store(&mut world, missing_owner);
        schedule.run(&mut world);
        for entity in [pending, ordinary, foreign, provisional, missing] {
            assert_eq!(
                world.get::<Sprite>(entity).unwrap().color,
                Color::srgba(1., 1., 0., 0.2)
            );
        }
    }

    #[test]
    fn completion_or_marker_reconstruction_without_sprite_changes_is_not_missed() {
        let (mut world, mut schedule, owner) = setup();
        let pending = store(&mut world, owner);
        world
            .entity_mut(pending)
            .insert(PendingBelongsToBlueprint(owner));
        let reconstructed_marker = store(&mut world, owner);
        world
            .entity_mut(reconstructed_marker)
            .remove::<BucketStorage>();
        let provisional_owner = world
            .spawn(Building {
                kind: BuildingType::Tank,
                is_provisional: true,
            })
            .id();
        let provisional = store(&mut world, provisional_owner);
        schedule.run(&mut world);
        // None of these transitions changes Sprite or BelongsTo.
        world
            .entity_mut(pending)
            .remove::<PendingBelongsToBlueprint>();
        world.entity_mut(reconstructed_marker).insert(BucketStorage);
        world
            .get_mut::<Building>(provisional_owner)
            .unwrap()
            .is_provisional = false;
        schedule.run(&mut world);
        for entity in [pending, reconstructed_marker, provisional] {
            assert_eq!(world.get::<Sprite>(entity).unwrap().color, Color::NONE);
        }
    }
}
