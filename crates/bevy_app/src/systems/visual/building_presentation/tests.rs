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
use hw_visual::{Building3dVisual, StructuralPresentationState, TopDownStructuralMaterial};

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
fn mixer_active_state_and_fallback_material_survive_fixed_production_rotor() {
    let mut app = app();
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
    publish(app.world_mut(), BuildingAssetKind::MudMixer, 1);
    app.update();
    let parts = children(app.world(), visual);
    assert_eq!(parts.len(), 2);
    let rotor = *app.world().get::<Transform>(parts[1]).unwrap();
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.update();
    assert_eq!(children(app.world(), visual), parts);
    // M2 supplies the art-approved axis/speed; M1-b must not guess them.
    assert_eq!(app.world().get::<Transform>(parts[1]), Some(&rotor));
}

#[test]
fn world_blueprint_pulse_destination_and_open_catalog_use_same_generation() {
    let mut app = app();
    let world_owner = shell(app.world_mut(), BuildingType::SandPile);
    let sprite = children(app.world(), world_owner)[0];
    let fallback = app.world().get::<Sprite>(sprite).unwrap().image.clone();
    let blueprint = app
        .world_mut()
        .spawn((
            Blueprint::new(BuildingType::SandPile, vec![]),
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
        .spawn((
            BuildingCatalogPreview(BuildingType::SandPile),
            ImageNode::default(),
        ))
        .id();
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    for generation in [1, 2] {
        let set = publish(app.world_mut(), BuildingAssetKind::SandPile, generation);
        app.update();
        for entity in [sprite, blueprint, overlay, destination] {
            assert_eq!(
                &app.world().get::<Sprite>(entity).unwrap().image,
                set.image("world").unwrap()
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
        .active(BuildingAssetKind::SandPile)
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
    app.world_mut()
        .resource_mut::<BuildingAssetPool>()
        .invalidate_active(&set.manifest.identity);
    app.update();
    assert_eq!(
        app.world().get::<Sprite>(leaf).unwrap().color,
        Color::srgba(0.4, 0.4, 0.4, 1.0)
    );
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
    let tiles: Vec<_> = shape
        .ordered_relative_tiles
        .iter()
        .map(|offset| {
            app.world_mut()
                .spawn(SoulSpaTile {
                    parent_site: owner,
                    grid_pos: (anchor.0 + offset.0, anchor.1 + offset.1),
                })
                .id()
        })
        .collect();
    publish(app.world_mut(), BuildingAssetKind::SoulSpa, 1);
    app.update();
    let visual = root(app.world_mut(), owner);
    let parts = children(app.world(), visual);
    assert_eq!(parts.len(), 5);
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
        }
    }
    app.world_mut().get_mut::<SoulSpaSite>(owner).unwrap().phase = SoulSpaPhase::Constructing;
    app.update();
    assert_eq!(
        app.world().get::<EquipmentRoot>(visual).unwrap().spa_mask,
        0
    );
    assert_eq!(children(app.world(), visual), parts);
}

#[test]
fn tank_state_changes_reuse_water_leaf_and_keep_fallback_materials() {
    use hw_core::relationships::StoredIn;
    use hw_core::visual_mirror::StockpileVisualState;
    let mut app = app();
    let owner = shell(app.world_mut(), BuildingType::Tank);
    app.world_mut()
        .entity_mut(owner)
        .insert(StockpileVisualState { capacity: 2 });
    let set = publish(app.world_mut(), BuildingAssetKind::Tank, 1);
    app.update();
    let visual = root(app.world_mut(), owner);
    let parts = children(app.world(), visual);
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
    }
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
