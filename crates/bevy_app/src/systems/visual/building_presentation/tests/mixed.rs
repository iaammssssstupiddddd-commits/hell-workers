//! Synthetic consumer regression, never a release or native acceptance record.
use super::*;

const KINDS: [BuildingType; 8] = [
    BuildingType::Tank,
    BuildingType::MudMixer,
    BuildingType::RestArea,
    BuildingType::SoulSpa,
    BuildingType::WheelbarrowParking,
    BuildingType::SandPile,
    BuildingType::BonePile,
    BuildingType::OutdoorLamp,
];

struct Consumers {
    kind: BuildingType,
    owner: Entity,
    blueprint: Entity,
    pulse: Entity,
    card: Entity,
}

fn consumers(world: &mut World, kind: BuildingType) -> Consumers {
    let owner = shell(world, kind);
    let blueprint = world
        .spawn((Blueprint::new(kind, vec![]), Sprite::default()))
        .id();
    let pulse = world
        .spawn((
            ChildOf(blueprint),
            Sprite::default(),
            hw_visual::blueprint::BlueprintPulseOverlayChild,
        ))
        .id();
    world
        .entity_mut(blueprint)
        .insert(hw_visual::blueprint::BlueprintVisual {
            pulse_overlay: Some(hw_visual::blueprint::BlueprintPulseOverlay {
                entity: pulse,
                base_color: Color::WHITE,
            }),
            ..default()
        });
    let card = world
        .spawn((BuildingCatalogPreview(kind), ImageNode::default()))
        .id();
    Consumers {
        kind,
        owner,
        blueprint,
        pulse,
        card,
    }
}

fn check_consumers(world: &mut World, row: &Consumers) -> Vec<Entity> {
    let kind = asset_kind(row.kind).unwrap();
    let set = world
        .resource::<BuildingAssetPool>()
        .descriptor(kind)
        .unwrap()
        .clone();
    for entity in [row.blueprint, row.pulse] {
        assert_eq!(
            &world.get::<Sprite>(entity).unwrap().image,
            set.image(&set.manifest.world_preview.image_role).unwrap()
        );
    }
    assert_eq!(
        &world.get::<ImageNode>(row.card).unwrap().image,
        set.image("catalog").unwrap()
    );
    if kind.mesh_roles().is_empty() {
        let sprite = children(world, row.owner)[0];
        assert_eq!(
            &world.get::<Sprite>(sprite).unwrap().image,
            set.image(&set.manifest.world_preview.image_role).unwrap()
        );
        vec![sprite]
    } else {
        let visual = root(world, row.owner);
        let parts = children(world, visual);
        assert_eq!(parts.len(), set.manifest.parts.len());
        for (&part, spec) in parts.iter().zip(&set.manifest.parts) {
            assert_eq!(
                &world.get::<Mesh3d>(part).unwrap().0,
                set.mesh(&spec.mesh_role).unwrap()
            );
        }
        parts
    }
}

#[test]
fn m6_mixed_generations_invalidation_and_world_reset_are_kind_local() {
    let mut app = app();
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    let mut rows: Vec<_> = KINDS
        .into_iter()
        .map(|kind| consumers(app.world_mut(), kind))
        .collect();
    // Door remains a distinct consumer. Equipment synchronization must not
    // overwrite its final descriptor; actual Door readiness has dedicated tests.
    let door_image = Handle::<Image>::default();
    let door_card = app
        .world_mut()
        .spawn((
            BuildingCatalogPreview(BuildingType::Door),
            ImageNode::new(door_image.clone()),
        ))
        .id();
    for row in &rows {
        publish(app.world_mut(), asset_kind(row.kind).unwrap(), 1);
    }
    app.update();
    for round in 0..10 {
        for (changed, changed_row) in rows.iter().enumerate() {
            let before: Vec<_> = rows
                .iter()
                .map(|row| check_consumers(app.world_mut(), row))
                .collect();
            let kind = asset_kind(changed_row.kind).unwrap();
            let old = app
                .world()
                .resource::<BuildingAssetPool>()
                .active(kind)
                .unwrap()
                .clone();
            let replacement = publish(app.world_mut(), kind, 2 + round * 2);
            app.update();
            for (index, row) in rows.iter().enumerate() {
                let after = check_consumers(app.world_mut(), row);
                if index != changed || kind.mesh_roles().is_empty() {
                    assert_eq!(
                        after, before[index],
                        "unrelated owner's parts were replaced"
                    );
                } else {
                    assert!(
                        before[index]
                            .iter()
                            .all(|part| app.world().get_entity(*part).is_err())
                    );
                }
            }
            assert!(
                !app.world_mut()
                    .resource_mut::<BuildingAssetPool>()
                    .invalidate_active(&old)
            );
            assert!(
                app.world_mut()
                    .resource_mut::<BuildingAssetPool>()
                    .invalidate_active(&replacement.manifest.identity)
            );
            app.update();
            for (index, row) in rows.iter().enumerate() {
                if index != changed {
                    check_consumers(app.world_mut(), row);
                }
            }
            assert_ne!(
                app.world()
                    .get::<ImageNode>(changed_row.card)
                    .unwrap()
                    .image,
                *replacement.image("catalog").unwrap()
            );
            publish(app.world_mut(), kind, 3 + round * 2);
            app.update();
        }
        assert_eq!(
            app.world().get::<ImageNode>(door_card).unwrap().image,
            door_image
        );
        let old_parts: Vec<_> = rows
            .iter()
            .flat_map(|row| check_consumers(app.world_mut(), row))
            .collect();
        hw_visual::reset_for_world_replace(app.world_mut());
        for row in &rows {
            app.world_mut().despawn(row.owner);
            app.world_mut().despawn(row.blueprint);
            app.world_mut().despawn(row.card);
        }
        rows = KINDS
            .into_iter()
            .map(|kind| consumers(app.world_mut(), kind))
            .collect();
        app.update();
        assert!(
            old_parts
                .iter()
                .all(|part| app.world().get_entity(*part).is_err())
        );
        for row in &rows {
            check_consumers(app.world_mut(), row);
        }
    }
}
