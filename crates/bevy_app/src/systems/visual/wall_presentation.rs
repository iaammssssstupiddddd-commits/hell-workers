//! Atomic production/fallback presentation for owner-linked 3D Wall visuals.

use std::collections::HashMap;

use bevy::ecs::system::SystemParam;
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use hw_jobs::{Building, BuildingType};
use hw_visual::wall_connection::{WallMeshFamily, WallTopologyState};
use hw_visual::{
    Building3dVisual, TopDownStructuralMaterial, Wall3dPresentationMode, Wall3dPresentationState,
};

use crate::assets::wall_asset_set::{
    ProductionWallAssetPool, ProductionWallMaterialPool, WallProductionActivation,
    WallProductionActivationState, WallProductionFallbackReason,
};
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::structural_light_anchor_mesh_tag;

use super::building3d_cleanup::building_presentation_transform;

#[derive(Resource, Debug, Default)]
pub struct Wall3dVisualOwnerIndex {
    by_owner: HashMap<Entity, Entity>,
    by_visual: HashMap<Entity, Entity>,
    last_decision_revision: Option<u64>,
    pub full_refreshes: u64,
    pub visual_writes: u64,
}

/// Drops root-owned visual links and prevents a newly rehydrated world from
/// inheriting the previous world's production activation decision.
pub fn reset_wall_presentation_for_world_replace(world: &mut World) {
    if world.contains_resource::<Wall3dVisualOwnerIndex>() {
        world.insert_resource(Wall3dVisualOwnerIndex::default());
    }
    if let Some(mut activation) = world.get_resource_mut::<WallProductionActivation>() {
        activation.topology_ready = false;
        activation.state = WallProductionActivationState::Fallback(
            WallProductionFallbackReason::TopologyUnavailable,
        );
        activation.decision_revision = activation.decision_revision.wrapping_add(1);
    }
}

type WallOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building,
        &'static Transform,
        &'static WallTopologyState,
    ),
    Without<Building3dVisual>,
>;

type ChangedWallOwnerQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        Without<Building3dVisual>,
        With<WallTopologyState>,
        Or<(
            Changed<Building>,
            Changed<Transform>,
            Changed<WallTopologyState>,
        )>,
    ),
>;

type AddedWallVisualQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Building3dVisual), Added<Wall3dPresentationState>>;

type WallVisualMutQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building3dVisual,
        &'static mut Wall3dPresentationState,
        &'static mut Mesh3d,
        &'static mut MeshMaterial3d<TopDownStructuralMaterial>,
        &'static mut Transform,
        &'static mut MeshTag,
    ),
>;

#[derive(SystemParam)]
pub struct WallPresentationQueries<'w, 's> {
    owners: WallOwnerQuery<'w, 's>,
    changed_owners: ChangedWallOwnerQuery<'w, 's>,
    visuals: ParamSet<'w, 's, (AddedWallVisualQuery<'w, 's>, WallVisualMutQuery<'w, 's>)>,
    removed_visuals: RemovedComponents<'w, 's, Wall3dPresentationState>,
}

fn production_mesh_index(family: WallMeshFamily) -> usize {
    match family {
        WallMeshFamily::Isolated => 0,
        WallMeshFamily::End => 1,
        WallMeshFamily::Straight => 2,
        WallMeshFamily::Corner => 3,
        WallMeshFamily::TJunction => 4,
        WallMeshFamily::Cross => 5,
    }
}

fn production_is_bound(
    activation: &WallProductionActivation,
    assets: &ProductionWallAssetPool,
    materials: &ProductionWallMaterialPool,
) -> bool {
    let WallProductionActivationState::ReadyToApply {
        asset_set_generation,
        authority,
        manifest_sha256,
        ..
    } = &activation.state
    else {
        return false;
    };
    let Some(resolved) = assets.resolved.as_ref() else {
        return false;
    };
    resolved.identity.asset_set_generation == *asset_set_generation
        && resolved.identity.authority == *authority
        && resolved.identity.manifest_sha256 == *manifest_sha256
        && materials.identity.as_ref() == Some(&resolved.identity)
        && materials.complete.is_some()
        && materials.provisional.is_some()
}

pub fn apply_wall_presentation_system(
    activation: Res<WallProductionActivation>,
    assets: Res<ProductionWallAssetPool>,
    production_materials: Res<ProductionWallMaterialPool>,
    fallback: Res<Building3dHandles>,
    mut index: ResMut<Wall3dVisualOwnerIndex>,
    mut queries: WallPresentationQueries,
) {
    for visual in queries.removed_visuals.read() {
        if let Some(owner) = index.by_visual.remove(&visual)
            && index.by_owner.get(&owner) == Some(&visual)
        {
            index.by_owner.remove(&owner);
        }
    }

    let added: Vec<_> = queries
        .visuals
        .p0()
        .iter()
        .map(|(visual, link)| (visual, link.owner))
        .collect();
    for (visual, owner) in &added {
        if queries.owners.get(*owner).is_ok() {
            if let Some(old_visual) = index.by_owner.insert(*owner, *visual) {
                index.by_visual.remove(&old_visual);
            }
            index.by_visual.insert(*visual, *owner);
        }
    }

    let full_refresh = index.last_decision_revision != Some(activation.decision_revision);
    let mut owners = if full_refresh {
        index.full_refreshes = index.full_refreshes.saturating_add(1);
        index.by_owner.keys().copied().collect::<Vec<_>>()
    } else {
        queries.changed_owners.iter().collect::<Vec<_>>()
    };
    owners.extend(added.into_iter().map(|(_, owner)| owner));
    owners.sort_unstable();
    owners.dedup();
    index.last_decision_revision = Some(activation.decision_revision);

    let production = production_is_bound(&activation, &assets, &production_materials);
    let mut visual_query = queries.visuals.p1();
    for owner_entity in owners {
        let Some(&visual_entity) = index.by_owner.get(&owner_entity) else {
            continue;
        };
        let Ok((building, owner_transform, topology)) = queries.owners.get(owner_entity) else {
            continue;
        };
        if building.kind != BuildingType::Wall {
            continue;
        }
        let Ok((_, mut state, mut mesh, mut material, mut transform, mut mesh_tag)) =
            visual_query.get_mut(visual_entity)
        else {
            continue;
        };

        let mut next_transform =
            building_presentation_transform(BuildingType::Wall, owner_transform);
        let (next_mesh, next_material, mode) = if production {
            let resolved = assets
                .resolved
                .as_ref()
                .expect("bound production Wall assets disappeared");
            next_transform.rotation *= Quat::from_rotation_y(
                f32::from(topology.resolved.quarter_turns_y.get()) * std::f32::consts::FRAC_PI_2,
            );
            let next_material = if building.is_provisional {
                production_materials
                    .provisional
                    .as_ref()
                    .expect("bound provisional Wall material")
            } else {
                production_materials
                    .complete
                    .as_ref()
                    .expect("bound complete Wall material")
            };
            let mesh_index = production_mesh_index(topology.resolved.family);
            let next_mesh = if building.is_provisional {
                resolved
                    .formwork_meshes
                    .as_ref()
                    .map_or(&resolved.meshes[mesh_index], |meshes| &meshes[mesh_index])
            } else {
                &resolved.meshes[mesh_index]
            };
            (next_mesh, next_material, Wall3dPresentationMode::Production)
        } else {
            let next_material = if building.is_provisional {
                &fallback.wall_provisional_material
            } else {
                &fallback.wall_material
            };
            (
                &fallback.wall_mesh,
                next_material,
                Wall3dPresentationMode::Fallback,
            )
        };
        let next_state = Wall3dPresentationState {
            topology: topology.resolved,
            mode,
            activation_revision: activation.decision_revision,
        };
        let next_tag = structural_light_anchor_mesh_tag(BuildingType::Wall, owner_transform)
            .expect("Wall grid anchor fits MeshTag");
        let changed = *state != next_state
            || mesh.0 != *next_mesh
            || material.0 != *next_material
            || *transform != next_transform
            || *mesh_tag != next_tag;
        if !changed {
            continue;
        }
        *state = next_state;
        mesh.0 = next_mesh.clone();
        material.0 = next_material.clone();
        *transform = next_transform;
        *mesh_tag = next_tag;
        index.visual_writes = index.visual_writes.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::transform::{TransformPlugin, TransformSystems};
    use hw_core::visual_mirror::building::{BuildingTypeVisual, BuildingVisualState};
    use hw_visual::blueprint::{BuildingBounceEffect, building_bounce_animation_system};
    use hw_visual::wall_connection::{
        QuarterTurns, ResolvedWallTopology, WallConnectionDirty, WallConnectionMask,
        WallTopologyIndex, WallTopologyResolveSet, wall_connections_system,
    };
    use hw_world::WorldMap;

    use super::*;
    use crate::assets::wall_asset_set::{
        ResolvedProductionWallAssets, WallAssetAuthority, WallAssetReadiness,
        WallAssetReadinessState, WallAssetSetIdentity, finalize_wall_production_activation_system,
    };
    use crate::plugins::visual::{WallAssetReadinessSet, WallPresentationApplySet};

    const GENERATION: u64 = 17;
    const MANIFEST_SHA256: &str = "presentation-test-manifest";

    struct PresentationFixture {
        app: App,
        owner: Entity,
        visual: Entity,
        fallback_mesh: Handle<Mesh>,
        production_meshes: [Handle<Mesh>; 6],
        formwork_meshes: [Handle<Mesh>; 6],
        complete_material: Handle<TopDownStructuralMaterial>,
        provisional_material: Handle<TopDownStructuralMaterial>,
    }

    fn identity() -> WallAssetSetIdentity {
        WallAssetSetIdentity {
            asset_set_generation: GENERATION,
            authority: WallAssetAuthority::IsolatedCandidate,
            manifest_sha256: MANIFEST_SHA256.to_string(),
        }
    }

    fn ready_activation() -> WallProductionActivation {
        WallProductionActivation {
            topology_ready: true,
            decision_revision: 4,
            state: WallProductionActivationState::ReadyToApply {
                asset_set_generation: GENERATION,
                authority: WallAssetAuthority::IsolatedCandidate,
                manifest_sha256: MANIFEST_SHA256.to_string(),
                asset_activation_revision: 3,
            },
        }
    }

    fn make_fixture(add_system: impl FnOnce(&mut App)) -> PresentationFixture {
        let mut meshes = Assets::<Mesh>::default();
        let fallback_mesh = meshes.add(Cuboid::new(32.0, 32.0, 32.0));
        let production_meshes =
            std::array::from_fn(|index| meshes.add(Cuboid::new(9.6 + index as f32, 32.0, 32.0)));
        let formwork_meshes =
            std::array::from_fn(|index| meshes.add(Cuboid::new(4.8 + index as f32, 32.0, 32.0)));
        let mut materials = Assets::<TopDownStructuralMaterial>::default();
        let fallback_material = materials.add(TopDownStructuralMaterial::default());
        let fallback_provisional_material = materials.add(TopDownStructuralMaterial::default());
        let complete_material = materials.add(TopDownStructuralMaterial::default());
        let provisional_material = materials.add(TopDownStructuralMaterial::default());

        let mut fallback = crate::test_support::empty_building_3d_handles();
        fallback.wall_mesh = fallback_mesh.clone();
        fallback.wall_material = fallback_material.clone();
        fallback.wall_provisional_material = fallback_provisional_material;

        let resolved = ResolvedProductionWallAssets {
            identity: identity(),
            meshes: production_meshes.clone(),
            formwork_meshes: Some(formwork_meshes.clone()),
            albedo: Handle::default(),
            formwork_albedo: Some(Handle::default()),
            emissive: Handle::default(),
            normal: None,
        };
        let production_assets = ProductionWallAssetPool {
            manifest: Handle::default(),
            resolved: Some(resolved),
        };
        let production_materials = ProductionWallMaterialPool {
            identity: Some(identity()),
            complete: Some(complete_material.clone()),
            provisional: Some(provisional_material.clone()),
        };

        let mut app = App::new();
        app.add_plugins(TransformPlugin)
            .insert_resource(ready_activation())
            .insert_resource(production_assets)
            .insert_resource(production_materials)
            .insert_resource(fallback)
            .init_resource::<Wall3dVisualOwnerIndex>();
        add_system(&mut app);

        let owner_transform = Transform::from_xyz(32.0, 64.0, 0.0);
        let topology = WallTopologyState {
            grid: (1, 2),
            mask: WallConnectionMask::from_neighbors(true, false, true, false),
            resolved: ResolvedWallTopology {
                family: WallMeshFamily::Corner,
                quarter_turns_y: QuarterTurns::ZERO,
            },
            revision: 1,
        };
        let owner = app
            .world_mut()
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: false,
                },
                owner_transform,
                topology,
            ))
            .id();
        let visual = app
            .world_mut()
            .spawn((
                Building3dVisual { owner },
                Wall3dPresentationState::default(),
                Mesh3d(fallback_mesh.clone()),
                MeshMaterial3d(fallback_material),
                Transform::default(),
                MeshTag(0),
            ))
            .id();

        PresentationFixture {
            app,
            owner,
            visual,
            fallback_mesh,
            production_meshes,
            formwork_meshes,
            complete_material,
            provisional_material,
        }
    }

    fn add_apply_to_update(app: &mut App) {
        app.add_systems(Update, apply_wall_presentation_system);
    }

    #[test]
    fn atomically_applies_family_material_transform_and_fallback_without_steady_writes() {
        let mut fixture = make_fixture(add_apply_to_update);

        fixture.app.update();
        assert_eq!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            fixture.production_meshes[3]
        );
        assert_eq!(
            fixture
                .app
                .world()
                .get::<MeshMaterial3d<TopDownStructuralMaterial>>(fixture.visual)
                .unwrap()
                .0,
            fixture.complete_material
        );
        assert_eq!(
            fixture
                .app
                .world()
                .get::<Wall3dPresentationState>(fixture.visual)
                .unwrap()
                .mode,
            Wall3dPresentationMode::Production
        );
        let writes = fixture
            .app
            .world()
            .resource::<Wall3dVisualOwnerIndex>()
            .visual_writes;

        fixture.app.update();
        assert_eq!(
            fixture
                .app
                .world()
                .resource::<Wall3dVisualOwnerIndex>()
                .visual_writes,
            writes,
            "a stable frame must not rewrite Wall presentation components"
        );

        {
            let mut topology = fixture
                .app
                .world_mut()
                .get_mut::<WallTopologyState>(fixture.owner)
                .unwrap();
            topology.resolved = ResolvedWallTopology {
                family: WallMeshFamily::Straight,
                quarter_turns_y: QuarterTurns::ONE,
            };
            topology.revision += 1;
        }
        fixture
            .app
            .world_mut()
            .get_mut::<Building>(fixture.owner)
            .unwrap()
            .is_provisional = true;
        fixture.app.update();
        assert_eq!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            fixture.formwork_meshes[2]
        );
        assert_eq!(
            fixture
                .app
                .world()
                .get::<MeshMaterial3d<TopDownStructuralMaterial>>(fixture.visual)
                .unwrap()
                .0,
            fixture.provisional_material
        );
        let rotation = fixture
            .app
            .world()
            .get::<Transform>(fixture.visual)
            .unwrap()
            .rotation;
        assert!(rotation.abs_diff_eq(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), 1e-5));

        {
            let mut activation = fixture
                .app
                .world_mut()
                .resource_mut::<WallProductionActivation>();
            activation.state =
                WallProductionActivationState::Fallback(WallProductionFallbackReason::LoadFailed);
            activation.decision_revision += 1;
        }
        fixture.app.update();
        assert_eq!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            fixture.fallback_mesh
        );
        assert_eq!(
            fixture
                .app
                .world()
                .get::<Wall3dPresentationState>(fixture.visual)
                .unwrap()
                .mode,
            Wall3dPresentationMode::Fallback
        );
    }

    #[test]
    fn activation_transition_refreshes_every_indexed_wall_once() {
        let mut fixture = make_fixture(add_apply_to_update);
        let second_owner = fixture
            .app
            .world_mut()
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: false,
                },
                Transform::from_xyz(64.0, 64.0, 0.0),
                WallTopologyState {
                    grid: (2, 2),
                    mask: WallConnectionMask::from_neighbors(false, false, false, true),
                    resolved: ResolvedWallTopology {
                        family: WallMeshFamily::End,
                        quarter_turns_y: QuarterTurns::THREE,
                    },
                    revision: 1,
                },
            ))
            .id();
        let second_visual = fixture
            .app
            .world_mut()
            .spawn((
                Building3dVisual {
                    owner: second_owner,
                },
                Wall3dPresentationState::default(),
                Mesh3d(fixture.fallback_mesh.clone()),
                MeshMaterial3d(fixture.complete_material.clone()),
                Transform::default(),
                MeshTag(0),
            ))
            .id();

        fixture.app.update();
        assert_eq!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            fixture.production_meshes[3]
        );
        assert_eq!(
            fixture.app.world().get::<Mesh3d>(second_visual).unwrap().0,
            fixture.production_meshes[1]
        );
        let writes_before_fallback = fixture
            .app
            .world()
            .resource::<Wall3dVisualOwnerIndex>()
            .visual_writes;

        {
            let mut activation = fixture
                .app
                .world_mut()
                .resource_mut::<WallProductionActivation>();
            activation.state =
                WallProductionActivationState::Fallback(WallProductionFallbackReason::LoadFailed);
            activation.decision_revision += 1;
        }
        fixture.app.update();

        for visual in [fixture.visual, second_visual] {
            assert_eq!(
                fixture.app.world().get::<Mesh3d>(visual).unwrap().0,
                fixture.fallback_mesh
            );
            assert_eq!(
                fixture
                    .app
                    .world()
                    .get::<Wall3dPresentationState>(visual)
                    .unwrap()
                    .mode,
                Wall3dPresentationMode::Fallback
            );
        }
        assert_eq!(
            fixture
                .app
                .world()
                .resource::<Wall3dVisualOwnerIndex>()
                .visual_writes,
            writes_before_fallback + 2
        );
    }

    #[test]
    fn completion_transition_changes_mesh_and_material_without_replacing_owner_or_visual() {
        let mut fixture = make_fixture(add_apply_to_update);
        fixture.app.update();

        let mesh = fixture
            .app
            .world()
            .get::<Mesh3d>(fixture.visual)
            .unwrap()
            .0
            .clone();
        let state = *fixture
            .app
            .world()
            .get::<Wall3dPresentationState>(fixture.visual)
            .unwrap();
        let transform = *fixture
            .app
            .world()
            .get::<Transform>(fixture.visual)
            .unwrap();
        let tag = fixture
            .app
            .world()
            .get::<MeshTag>(fixture.visual)
            .unwrap()
            .clone();

        fixture
            .app
            .world_mut()
            .get_mut::<Building>(fixture.owner)
            .unwrap()
            .is_provisional = true;
        fixture.app.update();

        assert_ne!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            mesh
        );
        assert_eq!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            fixture.formwork_meshes[3]
        );
        assert_eq!(
            *fixture
                .app
                .world()
                .get::<Wall3dPresentationState>(fixture.visual)
                .unwrap(),
            state
        );
        assert_eq!(
            *fixture
                .app
                .world()
                .get::<Transform>(fixture.visual)
                .unwrap(),
            transform
        );
        assert_eq!(
            fixture.app.world().get::<MeshTag>(fixture.visual).unwrap(),
            &tag
        );
        assert_eq!(
            fixture
                .app
                .world()
                .get::<MeshMaterial3d<TopDownStructuralMaterial>>(fixture.visual)
                .unwrap()
                .0,
            fixture.provisional_material
        );
    }

    #[test]
    fn owner_transform_and_topology_rotation_compose_without_corrupting_mesh_tag() {
        let mut fixture = make_fixture(add_apply_to_update);
        fixture.app.update();

        {
            let mut owner = fixture
                .app
                .world_mut()
                .get_mut::<Transform>(fixture.owner)
                .unwrap();
            owner.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
            owner.scale = Vec3::splat(1.15);
        }
        fixture.app.update();

        let owner = *fixture.app.world().get::<Transform>(fixture.owner).unwrap();
        let expected = building_presentation_transform(BuildingType::Wall, &owner);
        let actual = *fixture
            .app
            .world()
            .get::<Transform>(fixture.visual)
            .unwrap();
        assert_eq!(actual, expected);
        let owner_tag = fixture
            .app
            .world()
            .get::<MeshTag>(fixture.visual)
            .unwrap()
            .clone();

        fixture
            .app
            .world_mut()
            .get_mut::<WallTopologyState>(fixture.owner)
            .unwrap()
            .resolved
            .quarter_turns_y = QuarterTurns::ONE;
        fixture.app.update();

        let actual = *fixture
            .app
            .world()
            .get::<Transform>(fixture.visual)
            .unwrap();
        assert!(actual.translation.abs_diff_eq(expected.translation, 1e-5));
        assert!(actual.scale.abs_diff_eq(expected.scale, 1e-5));
        assert!(actual.rotation.abs_diff_eq(
            expected.rotation * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            1e-5
        ));
        assert_eq!(
            fixture.app.world().get::<MeshTag>(fixture.visual).unwrap(),
            &owner_tag,
            "topology-only quarter turns must not change the logical light anchor"
        );
    }

    #[test]
    fn completion_bounce_preserves_topology_rotation_through_global_propagation() {
        let mut fixture = make_fixture(add_production_topology_chain);
        fixture
            .app
            .insert_resource(Time::<()>::default())
            .add_systems(Update, building_bounce_animation_system);
        fixture.app.update();

        fixture
            .app
            .world_mut()
            .get_mut::<WallTopologyState>(fixture.owner)
            .unwrap()
            .resolved
            .quarter_turns_y = QuarterTurns::ONE;
        fixture
            .app
            .world_mut()
            .entity_mut(fixture.owner)
            .insert(BuildingBounceEffect::completion());
        fixture
            .app
            .world_mut()
            .resource_mut::<Time<()>>()
            .advance_by(Duration::from_millis(100));

        fixture.app.update();

        let owner = *fixture.app.world().get::<Transform>(fixture.owner).unwrap();
        assert!(owner.scale.x > 1.0);
        let mut expected = building_presentation_transform(BuildingType::Wall, &owner);
        expected.rotation *= Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let local = *fixture
            .app
            .world()
            .get::<Transform>(fixture.visual)
            .unwrap();
        assert_eq!(local, expected);
        let global = fixture
            .app
            .world()
            .get::<GlobalTransform>(fixture.visual)
            .unwrap()
            .compute_transform();
        assert!(
            global
                .translation
                .abs_diff_eq(expected.translation, f32::EPSILON)
        );
        assert!(global.rotation.abs_diff_eq(expected.rotation, 1e-5));
        assert!(global.scale.abs_diff_eq(expected.scale, 1e-5));
        assert_eq!(
            fixture
                .app
                .world()
                .get::<Wall3dPresentationState>(fixture.visual)
                .unwrap()
                .topology
                .quarter_turns_y,
            QuarterTurns::ONE
        );
    }

    fn insert_resolved_topology_for_test(
        mut commands: Commands,
        owners: Query<Entity, (With<Building>, Without<WallTopologyState>)>,
    ) {
        for owner in owners.iter() {
            commands.entity(owner).insert(WallTopologyState {
                grid: (1, 2),
                mask: WallConnectionMask::from_neighbors(false, false, false, false),
                resolved: ResolvedWallTopology {
                    family: WallMeshFamily::Isolated,
                    quarter_turns_y: QuarterTurns::ZERO,
                },
                revision: 1,
            });
        }
    }

    fn add_post_update_chain(app: &mut App) {
        app.configure_sets(
            PostUpdate,
            (WallTopologyResolveSet, WallPresentationApplySet)
                .chain()
                .before(TransformSystems::Propagate),
        )
        .add_systems(
            PostUpdate,
            insert_resolved_topology_for_test.in_set(WallTopologyResolveSet),
        )
        .add_systems(
            PostUpdate,
            ApplyDeferred
                .after(WallTopologyResolveSet)
                .before(WallPresentationApplySet)
                .before(TransformSystems::Propagate),
        )
        .add_systems(
            PostUpdate,
            apply_wall_presentation_system.in_set(WallPresentationApplySet),
        );
    }

    fn add_production_topology_chain(app: &mut App) {
        app.insert_resource(WallProductionActivation::default())
            .insert_resource(WallAssetReadiness {
                activation_revision: 3,
                state: WallAssetReadinessState::Eligible {
                    asset_set_generation: GENERATION,
                    authority: WallAssetAuthority::IsolatedCandidate,
                    manifest_sha256: MANIFEST_SHA256.to_string(),
                },
                ..Default::default()
            })
            .insert_resource(crate::test_support::empty_wall_visual_handles())
            .init_resource::<WallConnectionDirty>()
            .init_resource::<WallTopologyIndex>()
            .configure_sets(
                PostUpdate,
                (
                    WallAssetReadinessSet,
                    WallTopologyResolveSet,
                    WallPresentationApplySet,
                )
                    .chain()
                    .before(TransformSystems::Propagate),
            )
            .add_systems(
                PostUpdate,
                wall_connections_system.in_set(WallTopologyResolveSet),
            )
            .add_systems(
                PostUpdate,
                ApplyDeferred
                    .after(WallTopologyResolveSet)
                    .before(WallPresentationApplySet)
                    .before(TransformSystems::Propagate),
            )
            .add_systems(
                PostUpdate,
                (
                    finalize_wall_production_activation_system,
                    apply_wall_presentation_system,
                )
                    .chain()
                    .in_set(WallPresentationApplySet),
            );
    }

    #[test]
    fn deferred_topology_reaches_visual_and_global_transform_in_the_same_frame() {
        let mut fixture = make_fixture(add_post_update_chain);
        fixture
            .app
            .world_mut()
            .entity_mut(fixture.owner)
            .remove::<WallTopologyState>();

        fixture.app.update();

        assert_eq!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            fixture.production_meshes[0]
        );
        let local = *fixture
            .app
            .world()
            .get::<Transform>(fixture.visual)
            .unwrap();
        let global = fixture
            .app
            .world()
            .get::<GlobalTransform>(fixture.visual)
            .unwrap()
            .compute_transform();
        assert!(global.translation.abs_diff_eq(local.translation, 1e-5));
        assert!(global.rotation.abs_diff_eq(local.rotation, 1e-5));
        assert!(global.scale.abs_diff_eq(local.scale, 1e-5));
    }

    #[test]
    fn real_topology_producer_updates_door_add_and_remove_in_the_same_frame() {
        let mut fixture = make_fixture(add_production_topology_chain);
        fixture
            .app
            .world_mut()
            .entity_mut(fixture.owner)
            .remove::<WallTopologyState>()
            .insert(BuildingVisualState {
                kind: BuildingTypeVisual::Wall,
                is_provisional: false,
            });
        fixture
            .app
            .world_mut()
            .get_mut::<Transform>(fixture.owner)
            .unwrap()
            .translation = WorldMap::grid_to_world(1, 2).extend(0.0);

        fixture.app.update();
        assert_eq!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            fixture.production_meshes[0]
        );

        let north_grid = (1, 3);
        let door = fixture
            .app
            .world_mut()
            .spawn((
                Transform::from_translation(
                    WorldMap::grid_to_world(north_grid.0, north_grid.1).extend(0.0),
                ),
                BuildingVisualState {
                    kind: BuildingTypeVisual::Door,
                    is_provisional: false,
                },
            ))
            .id();
        fixture.app.update();
        assert_eq!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            fixture.production_meshes[1]
        );
        let topology = *fixture
            .app
            .world()
            .get::<WallTopologyState>(fixture.owner)
            .unwrap();
        assert_eq!(
            topology.mask,
            WallConnectionMask::from_neighbors(true, false, false, false)
        );
        assert_eq!(topology.resolved.family, WallMeshFamily::End);
        assert_eq!(topology.resolved.quarter_turns_y, QuarterTurns::ZERO);

        fixture.app.world_mut().despawn(door);
        fixture.app.update();
        assert_eq!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            fixture.production_meshes[0]
        );
        assert_eq!(
            fixture
                .app
                .world()
                .get::<WallTopologyState>(fixture.owner)
                .unwrap()
                .mask,
            WallConnectionMask::from_neighbors(false, false, false, false)
        );
    }

    #[test]
    fn identity_mismatch_fails_closed_and_world_reset_is_idempotent() {
        let mut fixture = make_fixture(add_apply_to_update);
        fixture
            .app
            .world_mut()
            .resource_mut::<ProductionWallMaterialPool>()
            .identity = Some(WallAssetSetIdentity {
            manifest_sha256: "stale-materials".to_string(),
            ..identity()
        });

        fixture.app.update();
        assert_eq!(
            fixture.app.world().get::<Mesh3d>(fixture.visual).unwrap().0,
            fixture.fallback_mesh
        );

        let revision = fixture
            .app
            .world()
            .resource::<WallProductionActivation>()
            .decision_revision;
        reset_wall_presentation_for_world_replace(fixture.app.world_mut());
        reset_wall_presentation_for_world_replace(fixture.app.world_mut());
        let activation = fixture.app.world().resource::<WallProductionActivation>();
        assert!(!activation.topology_ready);
        assert_eq!(activation.decision_revision, revision + 2);
        assert_eq!(
            activation.state,
            WallProductionActivationState::Fallback(
                WallProductionFallbackReason::TopologyUnavailable
            )
        );
        let index = fixture.app.world().resource::<Wall3dVisualOwnerIndex>();
        assert_eq!(index.full_refreshes, 0);
        assert_eq!(index.visual_writes, 0);
    }
}
