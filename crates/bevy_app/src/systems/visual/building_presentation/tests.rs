use super::*;
use crate::app_contexts::{BuildContext, CompanionPlacementState, MoveContext, TaskContext};
use crate::assets::building_asset_set::BuildingAssetAuthority;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::world::CommandQueue;
use bevy::sprite::Anchor;
use hw_core::game_state::PlayMode;
use hw_core::visual_mirror::{MudMixerVisualState, PoweredVisualState};
use hw_jobs::{Blueprint, Building, MovePlantTask};
use hw_ui::components::BuildingCatalogPreview;
#[cfg(feature = "profiling")]
use hw_visual::StructuralPresentationState;
use hw_visual::{Building3dVisual, TopDownStructuralMaterial};

mod mixed;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_asset::<Gltf>()
        .init_asset::<WorldAsset>()
        .init_resource::<BuildingAssetPool>()
        .init_resource::<BuildContext>()
        .init_resource::<MoveContext>()
        .init_resource::<TaskContext>()
        .init_resource::<CompanionPlacementState>()
        .insert_resource(State::new(PlayMode::BuildingPlace));
    let server = app.world().resource::<AssetServer>().clone();
    let assets = crate::plugins::startup::create_game_assets(
        &server,
        &mut app.world_mut().resource_mut::<Assets<Image>>(),
    );
    app.insert_resource(assets);
    let mut materials = Assets::<TopDownStructuralMaterial>::default();
    let mut handles = crate::test_support::empty_building_3d_handles();
    handles.equipment_material = materials.add(TopDownStructuralMaterial::default());
    handles.tank_partial_material = materials.add(TopDownStructuralMaterial::default());
    handles.tank_full_material = materials.add(TopDownStructuralMaterial::default());
    handles.mixer_idle_material = materials.add(TopDownStructuralMaterial::default());
    handles.mixer_active_material = materials.add(TopDownStructuralMaterial::default());
    handles.render_layers = RenderLayers::layer(3);
    app.insert_resource(handles)
        .insert_resource(materials)
        .add_systems(
            PostUpdate,
            (
                sync_equipment_structure,
                sync_building_previews,
                ApplyDeferred,
            )
                .chain()
                .before(bevy::transform::TransformSystems::Propagate),
        );
    app
}

fn shell(world: &mut World, kind: BuildingType) -> Entity {
    let owner = world
        .spawn((
            Building {
                kind,
                is_provisional: false,
            },
            Transform::from_xyz(12.0, 24.0, 9.0),
        ))
        .id();
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        crate::systems::jobs::attach_building_shell(
            &mut commands,
            owner,
            kind,
            false,
            Vec2::new(12.0, 24.0),
            world.resource::<crate::assets::GameAssets>(),
            world.resource::<crate::plugins::startup::Building3dHandles>(),
        );
    }
    queue.apply(world);
    owner
}

fn root(world: &mut World, owner: Entity) -> Entity {
    let mut query = world.query::<(Entity, &Building3dVisual)>();
    let roots: Vec<_> = query
        .iter(world)
        .filter(|(_, visual)| visual.owner == owner)
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(roots.len(), 1);
    roots[0]
}

fn children(world: &World, entity: Entity) -> Vec<Entity> {
    world.get::<Children>(entity).unwrap().iter().collect()
}

fn publish(world: &mut World, kind: BuildingAssetKind, generation: u64) -> BuildingAssetDescriptor {
    world
        .resource_mut::<BuildingAssetPool>()
        .install_presentation_fixture(kind, generation, BuildingAssetAuthority::ReleaseApproved)
}

fn assert_authored_parts(world: &World, parts: &[Entity], set: &BuildingAssetDescriptor) {
    assert_eq!(parts.len(), set.manifest.parts.len());
    for (&entity, spec) in parts.iter().zip(&set.manifest.parts) {
        assert_eq!(
            world.get::<Transform>(entity),
            Some(&Transform {
                translation: Vec3::from_array(spec.translation_wu),
                rotation: Quat::from_array(spec.rotation_xyzw),
                scale: Vec3::from_array(spec.scale),
            })
        );
    }
}

#[test]
fn release_motion_uses_its_manifest_contract_and_preserves_part_ownership() {
    use hw_core::relationships::StoredIn;
    use hw_core::visual_mirror::StockpileVisualState;

    for (kind, asset_kind) in [
        (BuildingType::Tank, BuildingAssetKind::Tank),
        (BuildingType::MudMixer, BuildingAssetKind::MudMixer),
    ] {
        let mut app = app();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ));
        let owner = shell(app.world_mut(), kind);
        app.world_mut().entity_mut(owner).insert((
            StockpileVisualState { capacity: 2 },
            MudMixerVisualState { is_active: true },
        ));
        publish(app.world_mut(), asset_kind, 2);
        app.update();
        let visual = root(app.world_mut(), owner);
        let parts = children(app.world(), visual);
        let initial = *app.world().get::<Transform>(parts[1]).unwrap();
        for count in 1..=3 {
            app.world_mut().spawn(StoredIn(owner));
            app.update();
            assert_eq!(children(app.world(), visual), parts);
            let state_part = app.world().get::<Transform>(parts[1]).unwrap();
            if kind == BuildingType::Tank {
                assert_eq!(state_part.translation.y, if count < 2 { 9.0 } else { 19.0 });
            } else {
                assert_ne!(state_part.rotation, initial.rotation);
                // Fixture rotates around Z, whereas the independent clay draft uses Y.
                assert_eq!(state_part.rotation.y, 0.0);
            }
        }
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.update();
        let paused = *app.world().get::<Transform>(parts[1]).unwrap();
        app.update();
        assert_eq!(app.world().get::<Transform>(parts[1]), Some(&paused));
    }
}

#[test]
fn unadmitted_art_preview_does_not_apply_clay_transforms() {
    for (kind, asset_kind) in [
        (BuildingType::Tank, BuildingAssetKind::Tank),
        (BuildingType::MudMixer, BuildingAssetKind::MudMixer),
    ] {
        let mut app = app();
        let owner = shell(app.world_mut(), kind);
        // Retain an exact permission for generation 1 only, where available.
        #[cfg(feature = "profiling")]
        publish_clay(app.world_mut(), asset_kind, 1);
        app.world_mut()
            .resource_mut::<BuildingAssetPool>()
            .install_presentation_fixture(asset_kind, 2, BuildingAssetAuthority::ArtPreview);
        app.update();
        let visual = root(app.world_mut(), owner);
        let parts = children(app.world(), visual);
        assert_eq!(parts.len(), 1);
        assert_eq!(
            app.world().get::<Transform>(parts[0]),
            Some(&Transform::IDENTITY)
        );
    }
}

#[cfg(feature = "profiling")]
fn publish_clay(
    world: &mut World,
    kind: BuildingAssetKind,
    generation: u64,
) -> BuildingAssetDescriptor {
    world
        .resource_mut::<BuildingAssetPool>()
        .install_clay_presentation_fixture(kind, generation)
}

#[test]
fn fallback_roots_preserve_center_height_bounce_layers_and_cleanup() {
    let mut app = app();
    for kind in [
        BuildingType::Tank,
        BuildingType::MudMixer,
        BuildingType::RestArea,
        BuildingType::SoulSpa,
    ] {
        let owner = shell(app.world_mut(), kind);
        let root = root(app.world_mut(), owner);
        assert!(app.world().get::<Mesh3d>(root).is_none());
        for leaf in children(app.world(), root) {
            assert_eq!(
                app.world().get::<Visibility>(leaf),
                Some(&Visibility::Inherited),
                "factory must supply visibility before any renderer system runs",
            );
        }
        for scale in [1.0, 1.15, 0.95, 1.0] {
            app.world_mut().get_mut::<Transform>(owner).unwrap().scale = Vec3::splat(scale);
            app.update();
            let expected =
                crate::systems::visual::building3d_cleanup::building_presentation_transform(
                    kind,
                    app.world().get::<Transform>(owner).unwrap(),
                );
            assert_eq!(app.world().get::<Transform>(root), Some(&expected));
            let leaves = children(app.world(), root);
            assert_eq!(leaves.len(), 1);
            let leaf = leaves[0];
            assert!(app.world().get::<Building3dVisual>(leaf).is_none());
            assert_eq!(
                app.world().get::<Transform>(leaf),
                Some(&Transform::IDENTITY)
            );
            assert_eq!(
                app.world().get::<RenderLayers>(leaf),
                Some(&RenderLayers::layer(3))
            );
        }
        let leaves = children(app.world(), root);
        app.world_mut().despawn(owner); // Same owner removal used by cancel/deconstruct.
        app.update();
        assert!(app.world().get_entity(root).is_err());
        assert!(
            leaves
                .iter()
                .all(|leaf| app.world().get_entity(*leaf).is_err())
        );
    }
}

#[test]
fn late_generation_switch_pause_new_consumer_and_reset_preserve_one_root() {
    let mut app = app();
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    let owner = shell(app.world_mut(), BuildingType::Tank);
    let visual = root(app.world_mut(), owner);
    let initial = children(app.world(), visual);
    let a = publish(app.world_mut(), BuildingAssetKind::Tank, 1);
    app.update();
    assert_eq!(root(app.world_mut(), owner), visual);
    assert!(
        initial
            .iter()
            .all(|child| app.world().get_entity(*child).is_err())
    );
    let parts = children(app.world(), visual);
    assert_eq!(parts.len(), 2);
    for &part in &parts {
        assert!(app.world().get::<Visibility>(part).is_some());
    }
    assert_eq!(
        app.world().get::<Transform>(visual).unwrap().translation.y,
        0.0
    );
    assert_eq!(
        app.world().get::<Visibility>(parts[1]),
        Some(&Visibility::Hidden)
    );
    app.update();
    assert_eq!(children(app.world(), visual), parts);
    let newcomer = shell(app.world_mut(), BuildingType::Tank);
    app.update();
    let newcomer_root = root(app.world_mut(), newcomer);
    assert_eq!(children(app.world(), newcomer_root).len(), 2);
    let b = publish(app.world_mut(), BuildingAssetKind::Tank, 2);
    app.update();
    assert_eq!(root(app.world_mut(), owner), visual);
    assert!(
        parts
            .iter()
            .all(|child| app.world().get_entity(*child).is_err())
    );
    assert!(
        !app.world_mut()
            .resource_mut::<BuildingAssetPool>()
            .invalidate_active(&a.manifest.identity)
    );
    assert!(
        app.world_mut()
            .resource_mut::<BuildingAssetPool>()
            .invalidate_active(&b.manifest.identity)
    );
    app.update();
    assert_eq!(children(app.world(), visual).len(), 1);
    assert_eq!(
        app.world().get::<Transform>(visual).unwrap().translation.y,
        hw_core::constants::TILE_SIZE * 0.4
    );
    publish(app.world_mut(), BuildingAssetKind::Tank, 3);
    app.update();
    hw_visual::reset_for_world_replace(app.world_mut());
    assert!(app.world().get_entity(visual).is_err());
    assert!(
        app.world()
            .resource::<BuildingAssetPool>()
            .active(BuildingAssetKind::Tank)
            .is_some()
    );
}

#[test]
#[cfg(feature = "profiling")]
fn mixer_refining_stop_pause_resume_only_rotates_the_existing_rotor() {
    let mut app = app();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(100),
    ));
    let owner = shell(app.world_mut(), BuildingType::MudMixer);
    app.world_mut()
        .entity_mut(owner)
        .insert(MudMixerVisualState { is_active: true });
    app.update();
    let visual = root(app.world_mut(), owner);
    assert_eq!(
        app.world().get::<StructuralPresentationState>(visual),
        Some(&StructuralPresentationState::MixerActive)
    );
    let fallback = children(app.world(), visual)[0];
    assert_eq!(
        app.world()
            .get::<MeshMaterial3d<TopDownStructuralMaterial>>(fallback)
            .unwrap()
            .0,
        app.world()
            .resource::<crate::plugins::startup::Building3dHandles>()
            .mixer_active_material
    );
    publish_clay(app.world_mut(), BuildingAssetKind::MudMixer, 1);
    app.update();
    let parts = children(app.world(), visual);
    assert_eq!(parts.len(), 2);
    let body = *app.world().get::<Transform>(parts[0]).unwrap();
    let rotor = *app.world().get::<Transform>(parts[1]).unwrap();
    app.update();
    let active = *app.world().get::<Transform>(parts[1]).unwrap();
    assert_ne!(active.rotation, rotor.rotation);
    assert_eq!(active.translation, rotor.translation);
    assert_eq!(active.scale, rotor.scale);
    app.world_mut()
        .get_mut::<MudMixerVisualState>(owner)
        .unwrap()
        .is_active = false;
    app.update();
    assert_eq!(app.world().get::<Transform>(parts[1]), Some(&active));
    app.world_mut()
        .get_mut::<MudMixerVisualState>(owner)
        .unwrap()
        .is_active = true;
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.update();
    assert_eq!(children(app.world(), visual), parts);
    assert_eq!(app.world().get::<Transform>(parts[1]), Some(&active));
    app.world_mut().resource_mut::<Time<Virtual>>().unpause();
    app.update();
    assert_ne!(
        app.world().get::<Transform>(parts[1]).unwrap().rotation,
        active.rotation
    );
    assert_eq!(app.world().get::<Transform>(parts[0]), Some(&body));
    assert_eq!(children(app.world(), visual), parts);

    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.update();
    let held = app.world().get::<Transform>(parts[1]).unwrap().rotation;
    publish_clay(app.world_mut(), BuildingAssetKind::MudMixer, 2);
    app.update();
    let replacement = children(app.world(), visual);
    assert_eq!(replacement.len(), 2);
    assert!(
        parts
            .iter()
            .all(|part| app.world().get_entity(*part).is_err())
    );
    assert_eq!(
        app.world()
            .get::<Transform>(replacement[1])
            .unwrap()
            .rotation,
        held
    );

    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(visual).is_err());
    assert!(
        replacement
            .iter()
            .all(|part| app.world().get_entity(*part).is_err())
    );
    // A rehydrated owner starts with no runtime task mirror and angle zero,
    // even while the same asset generation remains resident through a pause.
    let reloaded = shell(app.world_mut(), BuildingType::MudMixer);
    app.update();
    let reloaded_root = root(app.world_mut(), reloaded);
    let reloaded_parts = children(app.world(), reloaded_root);
    assert_eq!(
        app.world()
            .get::<Transform>(reloaded_parts[1])
            .unwrap()
            .rotation,
        Quat::IDENTITY
    );
    assert_eq!(
        app.world()
            .get::<StructuralPresentationState>(reloaded_root),
        Some(&StructuralPresentationState::MixerIdle)
    );
}

#[test]
fn world_blueprint_pulse_destination_and_open_catalog_use_same_generation() {
    for (kind, asset_kind) in [
        (
            BuildingType::WheelbarrowParking,
            BuildingAssetKind::WheelbarrowParking,
        ),
        (BuildingType::SandPile, BuildingAssetKind::SandPile),
        (BuildingType::BonePile, BuildingAssetKind::BonePile),
        (BuildingType::OutdoorLamp, BuildingAssetKind::OutdoorLamp),
    ] {
        m5_consumers_follow_generation(kind, asset_kind);
    }
}

// Move consumers are synthetic coverage only: M5 kinds are not player-movable.
fn m5_consumers_follow_generation(kind: BuildingType, asset_kind: BuildingAssetKind) {
    let mut app = app();
    let world_owner = shell(app.world_mut(), kind);
    let sprite = children(app.world(), world_owner)[0];
    let fallback = app.world().get::<Sprite>(sprite).unwrap().image.clone();
    let blueprint = app
        .world_mut()
        .spawn((
            Blueprint::new(kind, vec![]),
            Sprite {
                image: fallback.clone(),
                custom_size: Some(Vec2::splat(32.0)),
                color: Color::srgba(0.2, 0.4, 0.6, 0.5),
                ..default()
            },
        ))
        .id();
    let overlay = app
        .world_mut()
        .spawn((
            ChildOf(blueprint),
            Sprite::from_image(fallback.clone()),
            hw_visual::blueprint::BlueprintPulseOverlayChild,
        ))
        .id();
    app.world_mut()
        .entity_mut(blueprint)
        .insert(hw_visual::blueprint::BlueprintVisual {
            pulse_overlay: Some(hw_visual::blueprint::BlueprintPulseOverlay {
                entity: overlay,
                base_color: Color::WHITE,
            }),
            ..default()
        });
    let destination = app
        .world_mut()
        .spawn((
            MovePlantTask {
                building: world_owner,
                destination_grid: (1, 1),
                destination_pos: Vec2::ZERO,
                companion_anchor: None,
            },
            Sprite::from_image(fallback.clone()),
        ))
        .id();
    let card = app
        .world_mut()
        .spawn((BuildingCatalogPreview(kind), ImageNode::default()))
        .id();
    let ghost = app
        .world_mut()
        .spawn((
            crate::systems::visual::placement_ghost::PlacementGhost,
            Sprite::from_image(fallback.clone()),
        ))
        .id();
    app.world_mut().resource_mut::<BuildContext>().0 = Some(kind);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.update();
    assert_eq!(app.world().get::<Sprite>(sprite).unwrap().image, fallback);
    for generation in [1, 2] {
        let set = publish(app.world_mut(), asset_kind, generation);
        app.update();
        for entity in [sprite, blueprint, overlay, destination, ghost] {
            assert_eq!(
                &app.world().get::<Sprite>(entity).unwrap().image,
                set.image(&set.manifest.world_preview.image_role).unwrap()
            );
            assert_eq!(
                app.world().get::<Anchor>(entity),
                Some(&Anchor(Vec2::new(0.0, -0.25)))
            );
        }
        assert_eq!(
            &app.world().get::<ImageNode>(card).unwrap().image,
            set.image("catalog").unwrap()
        );
    }
    let identity = app
        .world()
        .resource::<BuildingAssetPool>()
        .active(asset_kind)
        .unwrap()
        .clone();
    app.world_mut()
        .resource_mut::<BuildingAssetPool>()
        .invalidate_active(&identity);
    app.update();
    assert_eq!(app.world().get::<Sprite>(sprite).unwrap().image, fallback);
    assert_eq!(
        app.world().get::<Sprite>(blueprint).unwrap().color,
        Color::srgba(0.2, 0.4, 0.6, 0.5)
    );
    assert_eq!(app.world().get::<Anchor>(blueprint), Some(&Anchor::CENTER));
    assert_eq!(
        &app.world().get::<ImageNode>(card).unwrap().image,
        hw_ui::setup::UiAssets::building_preview(
            app.world().resource::<crate::assets::GameAssets>(),
            kind,
        ),
    );
    app.world_mut().despawn(world_owner);
    app.update();
    assert!(app.world().get_entity(sprite).is_err());
}

#[cfg(feature = "profiling")]
#[test]
fn m2_exact_preview_generation_reaches_ghost_companion_blueprint_pulse_move_and_catalog() {
    use crate::systems::visual::placement_ghost::{PlacementGhost, PlacementPartnerGhost};

    for (kind, asset_kind) in [
        (BuildingType::Tank, BuildingAssetKind::Tank),
        (BuildingType::MudMixer, BuildingAssetKind::MudMixer),
    ] {
        let mut app = app();
        let owner = shell(app.world_mut(), kind);
        let ghost = app
            .world_mut()
            .spawn((PlacementGhost, Sprite::default()))
            .id();
        let partner = app
            .world_mut()
            .spawn((PlacementPartnerGhost, Sprite::default()))
            .id();
        let blueprint = app
            .world_mut()
            .spawn((Blueprint::new(kind, vec![]), Sprite::default()))
            .id();
        let pulse = app
            .world_mut()
            .spawn((
                ChildOf(blueprint),
                Sprite::default(),
                hw_visual::blueprint::BlueprintPulseOverlayChild,
            ))
            .id();
        app.world_mut()
            .entity_mut(blueprint)
            .insert(hw_visual::blueprint::BlueprintVisual {
                pulse_overlay: Some(hw_visual::blueprint::BlueprintPulseOverlay {
                    entity: pulse,
                    base_color: Color::WHITE,
                }),
                ..default()
            });
        let destination = app
            .world_mut()
            .spawn((
                MovePlantTask {
                    building: owner,
                    destination_grid: (1, 1),
                    destination_pos: Vec2::new(64.0, 64.0),
                    companion_anchor: None,
                },
                Sprite::default(),
            ))
            .id();
        let card = app
            .world_mut()
            .spawn((BuildingCatalogPreview(kind), ImageNode::default()))
            .id();
        app.world_mut().resource_mut::<BuildContext>().0 = Some(kind);
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let mut retired = Vec::new();
        for generation in [1, 2] {
            let set = publish_clay(app.world_mut(), asset_kind, generation);
            app.update();
            let visual = root(app.world_mut(), owner);
            assert_eq!(children(app.world(), visual).len(), 2);
            assert!(
                retired
                    .iter()
                    .all(|entity| app.world().get_entity(*entity).is_err())
            );
            retired = children(app.world(), visual);
            for entity in [ghost, blueprint, pulse, destination] {
                assert_eq!(
                    &app.world().get::<Sprite>(entity).unwrap().image,
                    set.image("world_preview").unwrap()
                );
            }
            if kind == BuildingType::Tank {
                assert_eq!(
                    &app.world().get::<Sprite>(partner).unwrap().image,
                    set.image("world_preview").unwrap()
                );
            }
            assert_eq!(
                &app.world().get::<ImageNode>(card).unwrap().image,
                set.image("catalog").unwrap()
            );
            app.insert_resource(State::new(PlayMode::BuildingMove));
            app.world_mut().resource_mut::<MoveContext>().0 = Some(owner);
            app.update();
            assert_eq!(
                &app.world().get::<Sprite>(ghost).unwrap().image,
                set.image("world_preview").unwrap()
            );
            // Presentation reads placement/move state but cannot move the owner.
            assert_eq!(
                app.world().get::<Transform>(owner).unwrap().translation,
                Vec3::new(12.0, 24.0, 9.0)
            );
        }
        let identity = app
            .world()
            .resource::<BuildingAssetPool>()
            .active(asset_kind)
            .unwrap()
            .clone();
        app.world_mut()
            .resource_mut::<BuildingAssetPool>()
            .invalidate_active(&identity);
        app.update();
        let visual = root(app.world_mut(), owner);
        assert_eq!(children(app.world(), visual).len(), 1);
        assert!(
            retired
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
        app.world_mut().despawn(owner);
        app.update();
        assert!(app.world().get_entity(visual).is_err());
    }
}

#[test]
fn reused_door_equipment_wall_ghost_restores_geometry_and_candidate_stays_private() {
    let mut app = app();
    let ghost = app
        .world_mut()
        .spawn((
            crate::systems::visual::placement_ghost::PlacementGhost,
            Sprite::default(),
            Anchor(Vec2::new(0.0, -0.25)),
        ))
        .id();
    publish(app.world_mut(), BuildingAssetKind::Tank, 1);
    app.world_mut().resource_mut::<BuildContext>().0 = Some(BuildingType::Tank);
    app.update();
    assert_eq!(
        app.world().get::<Anchor>(ghost),
        Some(&Anchor(Vec2::new(0.0, -0.25)))
    );
    app.world_mut().resource_mut::<BuildContext>().0 = Some(BuildingType::Wall);
    app.update();
    assert_eq!(app.world().get::<Anchor>(ghost), Some(&Anchor::CENTER));
    assert_eq!(
        app.world().get::<Sprite>(ghost).unwrap().custom_size,
        Some(Vec2::splat(hw_core::constants::TILE_SIZE))
    );
    app.world_mut()
        .resource_mut::<BuildingAssetPool>()
        .install_presentation_fixture(
            BuildingAssetKind::Tank,
            2,
            BuildingAssetAuthority::IsolatedCandidate,
        );
    app.world_mut().resource_mut::<BuildContext>().0 = Some(BuildingType::Tank);
    app.update();
    assert_eq!(app.world().get::<Anchor>(ghost), Some(&Anchor::CENTER));
    assert_eq!(
        app.world().get::<Sprite>(ghost).unwrap().image,
        app.world()
            .resource::<crate::assets::GameAssets>()
            .tank_empty
    );
}

#[test]
fn lamp_images_replace_tint_and_invalidation_restores_current_power_state() {
    let mut app = app();
    app.add_systems(Update, hw_visual::power::sync_powered_visual_system);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    let owner = shell(app.world_mut(), BuildingType::OutdoorLamp);
    app.world_mut()
        .entity_mut(owner)
        .insert(PoweredVisualState { is_powered: false });
    let leaf = children(app.world(), owner)[0];
    let set = publish(app.world_mut(), BuildingAssetKind::OutdoorLamp, 1);
    for powered in [false, true, false] {
        app.world_mut()
            .get_mut::<PoweredVisualState>(owner)
            .unwrap()
            .is_powered = powered;
        app.update();
        let sprite = app.world().get::<Sprite>(leaf).unwrap();
        assert_eq!(
            &sprite.image,
            set.image(if powered { "world_on" } else { "world_off" })
                .unwrap()
        );
        assert_eq!(sprite.color, Color::WHITE);
    }
    app.world_mut().get_mut::<Sprite>(leaf).unwrap().color = Color::WHITE.with_alpha(0.5);
    app.update();
    assert_eq!(app.world().get::<Sprite>(leaf).unwrap().color.alpha(), 0.5);
    app.world_mut()
        .resource_mut::<BuildingAssetPool>()
        .invalidate_active(&set.manifest.identity);
    app.update();
    assert_eq!(
        app.world().get::<Sprite>(leaf).unwrap().color,
        Color::srgba(0.4, 0.4, 0.4, 0.5)
    );
}

#[test]
fn rebuilt_lamp_shell_uses_retained_generation_and_current_mirror_while_paused() {
    let mut app = app();
    app.add_systems(Update, hw_visual::power::sync_powered_visual_system);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    let set = publish(app.world_mut(), BuildingAssetKind::OutdoorLamp, 1);
    let old = shell(app.world_mut(), BuildingType::OutdoorLamp);
    app.world_mut()
        .entity_mut(old)
        .insert(PoweredVisualState { is_powered: true });
    app.update();
    let old_leaf = children(app.world(), old)[0];
    app.world_mut().despawn(old);
    // Same completion factory as rehydration; no saved sprite or power mirror.
    // This is shell reconstruction coverage, not a real save/load transaction.
    let rebuilt = shell(app.world_mut(), BuildingType::OutdoorLamp);
    let leaf = children(app.world(), rebuilt)[0];
    app.update();
    assert!(app.world().get_entity(old_leaf).is_err());
    assert_eq!(
        &app.world().get::<Sprite>(leaf).unwrap().image,
        set.image("world_off").unwrap(),
    );
    app.world_mut().resource_mut::<Time<Virtual>>().unpause();
    app.world_mut()
        .entity_mut(rebuilt)
        .insert(PoweredVisualState { is_powered: true });
    app.update();
    assert_eq!(
        &app.world().get::<Sprite>(leaf).unwrap().image,
        set.image("world_on").unwrap(),
    );
    assert_eq!(app.world().get::<Sprite>(leaf).unwrap().color, Color::WHITE);
}

#[test]
fn spa_fixed_slots_follow_all_worker_masks_and_construction_phase() {
    use hw_core::relationships::WorkingOn;
    use hw_energy::{SoulSpaPhase, SoulSpaSite, SoulSpaTile};
    let mut app = app();
    let owner = shell(app.world_mut(), BuildingType::SoulSpa);
    let anchor = (4, 5);
    let position = hw_world::WorldMap::grid_to_world(anchor.0, anchor.1)
        + Vec2::new(0.5, -0.5) * hw_core::constants::TILE_SIZE;
    app.world_mut().entity_mut(owner).insert((
        Transform::from_translation(position.extend(0.0)),
        SoulSpaSite {
            phase: SoulSpaPhase::Operational,
            active_slots: 0,
            ..default()
        },
    ));
    let shape = hw_jobs::placement_geometry::building_shape(BuildingType::SoulSpa);
    let mut tiles: Vec<_> = shape
        .ordered_relative_tiles
        .iter()
        .enumerate()
        .rev()
        .map(|(index, offset)| {
            let tile = app
                .world_mut()
                .spawn((
                    SoulSpaTile {
                        parent_site: owner,
                        grid_pos: (anchor.0 + offset.0, anchor.1 + offset.1),
                    },
                    hw_core::relationships::TaskWorkers::default(),
                    ChildOf(owner),
                ))
                .id();
            (index, tile)
        })
        .collect();
    tiles.sort_by_key(|(index, _)| *index);
    let tiles: Vec<_> = tiles.into_iter().map(|(_, tile)| tile).collect();
    publish(app.world_mut(), BuildingAssetKind::SoulSpa, 1);
    app.update();
    let visual = root(app.world_mut(), owner);
    let parts = children(app.world(), visual);
    assert_eq!(parts.len(), 5);
    assert!(app.world().get::<ChildOf>(visual).is_none());
    assert_eq!(children(app.world(), owner).len(), 4);
    assert!(
        parts
            .iter()
            .all(|part| app.world().get::<SoulSpaTile>(*part).is_none())
    );
    let mut workers = Vec::new();
    for mask in 0_u8..16 {
        for worker in workers.drain(..) {
            app.world_mut().despawn(worker);
        }
        for (index, tile) in tiles.iter().enumerate() {
            if mask & (1 << index) != 0 {
                workers.push(app.world_mut().spawn(WorkingOn(*tile)).id());
            }
        }
        // Capacity is deliberately unrelated to the observed worker mask.
        app.world_mut()
            .get_mut::<SoulSpaSite>(owner)
            .unwrap()
            .active_slots = if mask.is_multiple_of(2) { 4 } else { 0 };
        app.update();
        assert_eq!(
            app.world().get::<EquipmentRoot>(visual).unwrap().spa_mask,
            mask
        );
        assert_eq!(children(app.world(), visual), parts);
        let body = &app
            .world()
            .get::<MeshMaterial3d<TopDownStructuralMaterial>>(parts[0])
            .unwrap()
            .0;
        for index in 0..4 {
            let slot = &app
                .world()
                .get::<MeshMaterial3d<TopDownStructuralMaterial>>(parts[index + 1])
                .unwrap()
                .0;
            assert_eq!(slot != body, mask & (1 << index) != 0);
            let materials = app.world().resource::<Assets<TopDownStructuralMaterial>>();
            assert_eq!(
                materials.get(slot).unwrap().base.emissive,
                if mask & (1 << index) != 0 {
                    LinearRgba::WHITE
                } else {
                    LinearRgba::BLACK
                }
            );
        }
    }
    app.world_mut().get_mut::<SoulSpaSite>(owner).unwrap().phase = SoulSpaPhase::Constructing;
    app.update();
    assert_eq!(
        app.world().get::<EquipmentRoot>(visual).unwrap().spa_mask,
        0
    );
    assert_eq!(children(app.world(), visual), parts);
    // A paused rehydrated shell has durable phase, but no runtime workers.
    for worker in workers.drain(..) {
        app.world_mut().despawn(worker);
    }
    app.world_mut().get_mut::<SoulSpaSite>(owner).unwrap().phase = SoulSpaPhase::Operational;
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.update();
    assert_eq!(
        app.world().get::<EquipmentRoot>(visual).unwrap().spa_mask,
        0
    );
    let worker = app.world_mut().spawn(WorkingOn(tiles[2])).id();
    app.update();
    assert_eq!(
        app.world().get::<EquipmentRoot>(visual).unwrap().spa_mask,
        4
    );
    assert_eq!(children(app.world(), visual), parts);
    app.world_mut().despawn(worker);
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(visual).is_err());
    assert!(
        parts
            .iter()
            .chain(tiles.iter())
            .all(|entity| app.world().get_entity(*entity).is_err())
    );
}

#[test]
#[cfg(feature = "profiling")]
fn tank_state_changes_reuse_water_leaf_and_keep_fallback_materials() {
    use hw_core::relationships::StoredIn;
    use hw_core::visual_mirror::StockpileVisualState;
    let mut app = app();
    let owner = shell(app.world_mut(), BuildingType::Tank);
    app.world_mut()
        .entity_mut(owner)
        .insert(StockpileVisualState { capacity: 2 });
    let set = publish_clay(app.world_mut(), BuildingAssetKind::Tank, 1);
    app.update();
    let visual = root(app.world_mut(), owner);
    let parts = children(app.world(), visual);
    assert_eq!(
        app.world().get::<Visibility>(parts[1]),
        Some(&Visibility::Hidden)
    );
    let geometry: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/blender_ai_workflow/fixtures/building-tank-v1.geometry.json"
    )))
    .unwrap();
    for expected in [
        StructuralPresentationState::TankPartial,
        StructuralPresentationState::TankFull,
    ] {
        app.world_mut().spawn(StoredIn(owner));
        app.update();
        assert_eq!(
            app.world().get::<StructuralPresentationState>(visual),
            Some(&expected)
        );
        assert_eq!(children(app.world(), visual), parts);
        assert_eq!(
            app.world().get::<Visibility>(parts[1]),
            Some(&Visibility::Inherited)
        );
        let name = if expected == StructuralPresentationState::TankFull {
            "Full"
        } else {
            "Partial"
        };
        assert_eq!(
            app.world()
                .get::<Transform>(parts[1])
                .unwrap()
                .translation
                .y,
            geometry["states"][name]["water_y_wu"].as_f64().unwrap() as f32
        );
    }
    app.world_mut()
        .get_mut::<StockpileVisualState>(owner)
        .unwrap()
        .capacity = 0;
    app.update();
    assert_eq!(children(app.world(), visual), parts);
    assert_eq!(
        app.world().get::<StructuralPresentationState>(visual),
        Some(&StructuralPresentationState::TankPartial)
    );
    assert_eq!(
        app.world()
            .get::<Transform>(parts[1])
            .unwrap()
            .translation
            .y,
        12.0
    );
    app.world_mut()
        .get_mut::<StockpileVisualState>(owner)
        .unwrap()
        .capacity = 2;
    app.update();
    app.world_mut()
        .resource_mut::<BuildingAssetPool>()
        .invalidate_active(&set.manifest.identity);
    app.update();
    let leaf = children(app.world(), visual)[0];
    assert_eq!(
        app.world()
            .get::<MeshMaterial3d<TopDownStructuralMaterial>>(leaf)
            .unwrap()
            .0,
        app.world()
            .resource::<crate::plugins::startup::Building3dHandles>()
            .tank_full_material
    );
}

#[test]
fn m2_release_generations_retire_parts_and_owner_cleanup_without_preview_admission() {
    for (kind, asset_kind) in [
        (BuildingType::Tank, BuildingAssetKind::Tank),
        (BuildingType::MudMixer, BuildingAssetKind::MudMixer),
    ] {
        let mut app = app();
        let owner = shell(app.world_mut(), kind);
        let visual = root(app.world_mut(), owner);
        let mut retired = children(app.world(), visual);
        for generation in 1..=3 {
            let set = publish(app.world_mut(), asset_kind, generation);
            assert_eq!(
                set.manifest.identity.authority,
                BuildingAssetAuthority::ReleaseApproved
            );
            app.update();
            assert_eq!(root(app.world_mut(), owner), visual);
            assert_eq!(children(app.world(), visual).len(), 2);
            assert!(
                retired
                    .iter()
                    .all(|entity| app.world().get_entity(*entity).is_err())
            );
            retired = children(app.world(), visual);
        }
        app.world_mut().despawn(owner);
        app.update();
        assert!(app.world().get_entity(visual).is_err());
        assert!(
            retired
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
    }
}

#[test]
fn m3_consumers_generations_and_owner_cleanup_preserve_logical_state() {
    use crate::systems::visual::placement_ghost::PlacementGhost;
    use hw_core::relationships::{RestAreaOccupants, RestAreaReservedFor, RestingIn};

    for (kind, asset_kind, part_count) in [
        (BuildingType::RestArea, BuildingAssetKind::RestArea, 1),
        (BuildingType::SoulSpa, BuildingAssetKind::SoulSpa, 5),
    ] {
        let mut app = app();
        let owner = shell(app.world_mut(), kind);
        let reservation = app.world_mut().spawn(RestAreaReservedFor(owner)).id();
        let rest = app.world_mut().spawn(RestingIn(owner)).id();
        let ghost = app
            .world_mut()
            .spawn((PlacementGhost, Sprite::default()))
            .id();
        let blueprint = app
            .world_mut()
            .spawn((Blueprint::new(kind, vec![]), Sprite::default()))
            .id();
        let pulse = app
            .world_mut()
            .spawn((
                ChildOf(blueprint),
                Sprite::default(),
                hw_visual::blueprint::BlueprintPulseOverlayChild,
            ))
            .id();
        app.world_mut()
            .entity_mut(blueprint)
            .insert(hw_visual::blueprint::BlueprintVisual {
                pulse_overlay: Some(hw_visual::blueprint::BlueprintPulseOverlay {
                    entity: pulse,
                    base_color: Color::WHITE,
                }),
                ..default()
            });
        let card = app
            .world_mut()
            .spawn((BuildingCatalogPreview(kind), ImageNode::default()))
            .id();
        app.world_mut().resource_mut::<BuildContext>().0 = Some(kind);
        if kind == BuildingType::SoulSpa {
            app.world_mut().resource_mut::<BuildContext>().0 = None;
            app.world_mut().resource_mut::<TaskContext>().0 =
                hw_core::game_state::TaskMode::SoulSpaPlace(None);
        }
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let visual = root(app.world_mut(), owner);
        let owner_pose = *app.world().get::<Transform>(owner).unwrap();
        let mut old_parts = children(app.world(), visual);
        for generation in 1..=3 {
            let set = publish(app.world_mut(), asset_kind, generation);
            app.update();
            assert!(
                old_parts
                    .iter()
                    .all(|part| app.world().get_entity(*part).is_err())
            );
            let current = children(app.world(), visual);
            assert_eq!(current.len(), part_count);
            for entity in [ghost, blueprint, pulse] {
                assert_eq!(
                    &app.world().get::<Sprite>(entity).unwrap().image,
                    set.image("world_preview").unwrap()
                );
            }
            assert_eq!(
                &app.world().get::<ImageNode>(card).unwrap().image,
                set.image("catalog").unwrap()
            );
            assert_eq!(app.world().get::<Transform>(owner), Some(&owner_pose));
            assert_eq!(
                app.world().get::<RestAreaOccupants>(owner).unwrap().len(),
                1
            );
            assert!(
                app.world()
                    .get::<RestAreaReservedFor>(reservation)
                    .is_some()
            );
            assert!(app.world().get::<RestingIn>(rest).is_some());
            app.update();
            assert_eq!(children(app.world(), visual), current);
            old_parts = current;
        }
        let identity = app
            .world()
            .resource::<BuildingAssetPool>()
            .active(asset_kind)
            .unwrap()
            .clone();
        app.world_mut()
            .resource_mut::<BuildingAssetPool>()
            .invalidate_active(&identity);
        app.update();
        assert!(
            old_parts
                .iter()
                .all(|part| app.world().get_entity(*part).is_err())
        );
        assert_eq!(children(app.world(), visual).len(), 1);
        assert_eq!(
            app.world().get::<Sprite>(ghost).unwrap().image,
            app.world()
                .resource::<crate::assets::GameAssets>()
                .rest_area
        );
        app.world_mut().despawn(owner);
        app.update();
        assert!(app.world().get_entity(visual).is_err());
    }
}

#[test]
fn m3_fixture_slot_positions_follow_the_logical_shape_and_surface_emission() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/blender_ai_workflow/fixtures/building-m3-v1.geometry.json"
    )))
    .unwrap();
    let shape = hw_jobs::placement_geometry::building_shape(BuildingType::SoulSpa);
    for (offset, expected) in shape
        .ordered_relative_tiles
        .iter()
        .zip(fixture["ordered_relative_tiles"].as_array().unwrap())
    {
        assert_eq!(serde_json::json!([offset.0, offset.1]), *expected);
    }
    assert_eq!(shape.ordered_relative_tiles.len(), 4);
    assert_eq!(
        serde_json::json!(shape.center_offset_tiles),
        fixture["center_offset_tiles"]
    );
    assert_eq!(
        fixture["geometry_scale"],
        serde_json::json!(hw_core::constants::TILE_SIZE as u32)
    );
    assert_eq!(
        fixture["slot_emissive_multiplier_linear"],
        serde_json::json!([1, 1, 1])
    );
}

#[test]
fn bridge_consumers_share_generation_fallback_bounce_and_owner_cleanup() {
    let mut app = app();
    let kind = BuildingType::Bridge;
    let owner = shell(app.world_mut(), kind);
    let visual = root(app.world_mut(), owner);
    let fallback = app
        .world()
        .resource::<crate::assets::GameAssets>()
        .bridge
        .clone();
    let ghost = app
        .world_mut()
        .spawn((
            crate::systems::visual::placement_ghost::PlacementGhost,
            Sprite::from_image(fallback.clone()),
        ))
        .id();
    let blueprint = app
        .world_mut()
        .spawn((
            Blueprint::new(kind, vec![]),
            Sprite {
                image: fallback.clone(),
                custom_size: Some(Vec2::new(64.0, 160.0)),
                color: Color::srgba(0.2, 0.4, 0.6, 0.5),
                ..default()
            },
        ))
        .id();
    let pulse = app
        .world_mut()
        .spawn((
            ChildOf(blueprint),
            Sprite::from_image(fallback.clone()),
            hw_visual::blueprint::BlueprintPulseOverlayChild,
        ))
        .id();
    app.world_mut()
        .entity_mut(blueprint)
        .insert(hw_visual::blueprint::BlueprintVisual {
            pulse_overlay: Some(hw_visual::blueprint::BlueprintPulseOverlay {
                entity: pulse,
                base_color: Color::WHITE,
            }),
            ..default()
        });
    let card = app
        .world_mut()
        .spawn((BuildingCatalogPreview(kind), ImageNode::default()))
        .id();
    app.world_mut().resource_mut::<BuildContext>().0 = Some(kind);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.update();
    assert_eq!(
        app.world().get::<Transform>(visual).unwrap().translation.y,
        32.0 * 0.09
    );
    let mut retired = children(app.world(), visual);
    for generation in 1..=3 {
        let set = publish(app.world_mut(), BuildingAssetKind::Bridge, generation);
        app.world_mut().get_mut::<Transform>(owner).unwrap().scale = Vec3::splat(1.15);
        app.update();
        assert_eq!(root(app.world_mut(), owner), visual);
        assert_eq!(children(app.world(), visual).len(), 1);
        assert!(
            retired
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
        retired = children(app.world(), visual);
        assert_authored_parts(app.world(), &retired, &set);
        assert_eq!(
            &app.world().get::<Mesh3d>(retired[0]).unwrap().0,
            set.mesh("body").unwrap()
        );
        let pose = app.world().get::<Transform>(visual).unwrap();
        assert_eq!(pose.translation.y, 0.0);
        assert_eq!(pose.scale, Vec3::splat(1.15));
        for entity in [ghost, blueprint, pulse] {
            assert_eq!(
                &app.world().get::<Sprite>(entity).unwrap().image,
                set.image("world_preview").unwrap()
            );
        }
        assert_eq!(
            &app.world().get::<ImageNode>(card).unwrap().image,
            set.image("catalog").unwrap()
        );
        let restored = shell(app.world_mut(), kind);
        app.update();
        let restored_root = root(app.world_mut(), restored);
        let restored_part = children(app.world(), restored_root)[0];
        assert_eq!(
            &app.world().get::<Mesh3d>(restored_part).unwrap().0,
            set.mesh("body").unwrap()
        );
        app.world_mut().despawn(restored);
        app.update();
        assert!(app.world().get_entity(restored_root).is_err());
        assert!(app.world().get_entity(restored_part).is_err());
        assert_eq!(children(app.world(), visual), retired);
    }
    let identity = app
        .world()
        .resource::<BuildingAssetPool>()
        .active(BuildingAssetKind::Bridge)
        .unwrap()
        .clone();
    app.world_mut()
        .resource_mut::<BuildingAssetPool>()
        .invalidate_active(&identity);
    app.update();
    let fallback_part = children(app.world(), visual)[0];
    let handles = app
        .world()
        .resource::<crate::plugins::startup::Building3dHandles>();
    assert_eq!(
        app.world().get::<Mesh3d>(fallback_part).unwrap().0,
        handles.bridge_mesh
    );
    assert_eq!(
        app.world()
            .get::<MeshMaterial3d<TopDownStructuralMaterial>>(fallback_part)
            .unwrap()
            .0,
        handles.bridge_material
    );
    assert_eq!(
        app.world().get::<Transform>(visual).unwrap().translation.y,
        32.0 * 0.09
    );
    for entity in [ghost, blueprint, pulse] {
        assert_eq!(app.world().get::<Sprite>(entity).unwrap().image, fallback);
    }
    assert_eq!(
        app.world().get::<Sprite>(blueprint).unwrap().color,
        Color::srgba(0.2, 0.4, 0.6, 0.5)
    );
    assert_eq!(
        app.world().get::<Sprite>(blueprint).unwrap().custom_size,
        Some(Vec2::new(64.0, 160.0))
    );
    app.world_mut().despawn(owner);
    app.world_mut().despawn(blueprint);
    app.update();
    for entity in [visual, fallback_part, pulse] {
        assert!(app.world().get_entity(entity).is_err());
    }
}
