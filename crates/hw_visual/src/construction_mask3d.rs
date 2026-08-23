//! Floor/wall construction masks rendered inside the 3D RtT scene.

use bevy::camera::visibility::RenderLayers;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use hw_core::constants::{LAYER_3D, TILE_SIZE, Z_BUILDING_FLOOR};
use hw_core::visual_mirror::construction::{
    FloorTileStateMirror, FloorTileVisualMirror, WallTileStateMirror, WallTileVisualMirror,
};
use std::collections::HashMap;

use crate::material::ConstructionMaskMaterial;

const MASK_STATE_SHIFT: u32 = 8;
const MASK_KIND_WALL_BIT: u32 = 1 << 12;
const CONSTRUCTION_MASK_HEIGHT: f32 = Z_BUILDING_FLOOR * 0.5;

#[derive(Resource)]
pub struct ConstructionMask3dHandles {
    mesh: Handle<Mesh>,
    material: Handle<ConstructionMaskMaterial>,
}

/// Runtime-only Scene proxy for one logical construction tile.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstructionMask3dVisual {
    pub owner: Entity,
}

#[derive(Resource, Default)]
pub struct ConstructionMask3dOwnerCache {
    proxies: HashMap<Entity, Entity>,
}

type MaskProxyQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ConstructionMask3dVisual,
        &'static mut Transform,
        &'static mut MeshTag,
    ),
    (
        Without<FloorTileVisualMirror>,
        Without<WallTileVisualMirror>,
    ),
>;

type FloorMaskOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, &'static FloorTileVisualMirror),
    (
        Or<(
            Added<FloorTileVisualMirror>,
            Changed<FloorTileVisualMirror>,
            Changed<Transform>,
        )>,
        Without<ConstructionMask3dVisual>,
    ),
>;

type WallMaskOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, &'static WallTileVisualMirror),
    (
        Or<(
            Added<WallTileVisualMirror>,
            Changed<WallTileVisualMirror>,
            Changed<Transform>,
        )>,
        Without<ConstructionMask3dVisual>,
    ),
>;

pub fn init_construction_mask3d_handles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ConstructionMaskMaterial>>,
) {
    commands.insert_resource(ConstructionMask3dHandles {
        mesh: meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE)),
        material: materials.add(ConstructionMaskMaterial::default()),
    });
}

pub fn sync_floor_construction_masks_system(
    mut commands: Commands,
    handles: Res<ConstructionMask3dHandles>,
    mut cache: ResMut<ConstructionMask3dOwnerCache>,
    q_owners: FloorMaskOwnerQuery,
    mut q_proxies: MaskProxyQuery,
) {
    for (owner, transform, mirror) in &q_owners {
        sync_mask_proxy(
            &mut commands,
            &handles,
            &mut cache,
            &mut q_proxies,
            owner,
            transform,
            floor_mask_tag(mirror.state),
        );
    }
}

pub fn sync_wall_construction_masks_system(
    mut commands: Commands,
    handles: Res<ConstructionMask3dHandles>,
    mut cache: ResMut<ConstructionMask3dOwnerCache>,
    q_owners: WallMaskOwnerQuery,
    mut q_proxies: MaskProxyQuery,
) {
    for (owner, transform, mirror) in &q_owners {
        sync_mask_proxy(
            &mut commands,
            &handles,
            &mut cache,
            &mut q_proxies,
            owner,
            transform,
            wall_mask_tag(mirror.state),
        );
    }
}

fn sync_mask_proxy(
    commands: &mut Commands,
    handles: &ConstructionMask3dHandles,
    cache: &mut ConstructionMask3dOwnerCache,
    q_proxies: &mut MaskProxyQuery,
    owner: Entity,
    owner_transform: &Transform,
    tag: MeshTag,
) {
    // A Sprite would be drawn by the post-composite Camera2d and therefore
    // cover every Soul billboard regardless of its logical Z value.
    commands.entity(owner).remove::<Sprite>();

    let next_transform = mask_transform(owner_transform);
    if let Some(proxy) = cache.proxies.get(&owner).copied()
        && let Ok((visual, mut transform, mut current_tag)) = q_proxies.get_mut(proxy)
        && visual.owner == owner
    {
        if *transform != next_transform {
            *transform = next_transform;
        }
        if *current_tag != tag {
            *current_tag = tag;
        }
        return;
    }

    let proxy = commands
        .spawn((
            Mesh3d(handles.mesh.clone()),
            MeshMaterial3d(handles.material.clone()),
            tag,
            next_transform,
            RenderLayers::layer(LAYER_3D),
            NotShadowCaster,
            NotShadowReceiver,
            ConstructionMask3dVisual { owner },
            Name::new("ConstructionMask3dVisual"),
        ))
        .id();
    cache.proxies.insert(owner, proxy);
}

pub fn cleanup_construction_mask3d_system(
    mut commands: Commands,
    mut cache: ResMut<ConstructionMask3dOwnerCache>,
    mut removed_floor: RemovedComponents<FloorTileVisualMirror>,
    mut removed_wall: RemovedComponents<WallTileVisualMirror>,
) {
    for owner in removed_floor.read().chain(removed_wall.read()) {
        if let Some(proxy) = cache.proxies.remove(&owner) {
            commands.entity(proxy).try_despawn();
        }
    }
}

fn mask_transform(owner: &Transform) -> Transform {
    Transform::from_xyz(
        owner.translation.x,
        CONSTRUCTION_MASK_HEIGHT,
        -owner.translation.y,
    )
}

fn encode_mask_tag(state: u32, progress: u8, is_wall: bool) -> MeshTag {
    MeshTag(
        u32::from(progress.min(100))
            | (state << MASK_STATE_SHIFT)
            | if is_wall { MASK_KIND_WALL_BIT } else { 0 },
    )
}

fn floor_mask_tag(state: FloorTileStateMirror) -> MeshTag {
    let (state, progress) = match state {
        FloorTileStateMirror::WaitingBones => (0, 0),
        FloorTileStateMirror::ReinforcingReady => (1, 0),
        FloorTileStateMirror::Reinforcing { progress } => (2, progress),
        FloorTileStateMirror::ReinforcedComplete => (3, 100),
        FloorTileStateMirror::WaitingMud => (4, 0),
        FloorTileStateMirror::PouringReady => (5, 0),
        FloorTileStateMirror::Pouring { progress } => (6, progress),
        FloorTileStateMirror::Complete => (7, 100),
    };
    encode_mask_tag(state, progress, false)
}

fn wall_mask_tag(state: WallTileStateMirror) -> MeshTag {
    let (state, progress) = match state {
        WallTileStateMirror::WaitingWood => (0, 0),
        WallTileStateMirror::FramingReady => (1, 0),
        WallTileStateMirror::Framing { progress } => (2, progress),
        WallTileStateMirror::FramedProvisional => (3, 100),
        WallTileStateMirror::WaitingMud => (4, 0),
        WallTileStateMirror::CoatingReady => (5, 0),
        WallTileStateMirror::Coating { progress } => (6, progress),
        WallTileStateMirror::Complete => (7, 100),
    };
    encode_mask_tag(state, progress, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask_test_app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<ConstructionMaskMaterial>>()
            .init_resource::<ConstructionMask3dOwnerCache>();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE));
        let material = app
            .world_mut()
            .resource_mut::<Assets<ConstructionMaskMaterial>>()
            .add(ConstructionMaskMaterial::default());
        app.insert_resource(ConstructionMask3dHandles { mesh, material })
            .add_systems(
                Update,
                (
                    sync_floor_construction_masks_system,
                    sync_wall_construction_masks_system,
                    cleanup_construction_mask3d_system,
                )
                    .chain(),
            );
        app
    }

    #[test]
    fn mask_tags_keep_kind_state_and_bounded_progress_per_instance() {
        let floor = floor_mask_tag(FloorTileStateMirror::Pouring { progress: 240 }).0;
        assert_eq!(floor & 0xff, 100);
        assert_eq!((floor >> MASK_STATE_SHIFT) & 0x0f, 6);
        assert_eq!(floor & MASK_KIND_WALL_BIT, 0);

        let wall = wall_mask_tag(WallTileStateMirror::Framing { progress: 37 }).0;
        assert_eq!(wall & 0xff, 37);
        assert_eq!((wall >> MASK_STATE_SHIFT) & 0x0f, 2);
        assert_ne!(wall & MASK_KIND_WALL_BIT, 0);
    }

    #[test]
    fn mask_transform_maps_2d_world_to_ground_plane_below_finished_floor() {
        let owner = Transform::from_xyz(48.0, -64.0, 9.0);
        let proxy = mask_transform(&owner);

        assert_eq!(
            proxy.translation,
            Vec3::new(48.0, CONSTRUCTION_MASK_HEIGHT, 64.0)
        );
        assert!(proxy.translation.y > 0.0);
        assert!(proxy.translation.y < Z_BUILDING_FLOOR);
    }

    #[test]
    fn tile_sprite_is_replaced_by_owner_linked_3d_proxy_and_cleaned_up() {
        let mut app = mask_test_app();
        let owner = app
            .world_mut()
            .spawn((
                FloorTileVisualMirror::default(),
                Sprite::default(),
                Transform::from_xyz(32.0, -96.0, 0.02),
            ))
            .id();

        app.update();

        assert!(app.world().get::<Sprite>(owner).is_none());
        let (proxy, visual, layers, transform) = {
            let mut query =
                app.world_mut()
                    .query::<(Entity, &ConstructionMask3dVisual, &RenderLayers, &Transform)>();
            let rows: Vec<_> = query
                .iter(app.world())
                .map(|(entity, visual, layers, transform)| {
                    (entity, *visual, layers.clone(), *transform)
                })
                .collect();
            assert_eq!(rows.len(), 1);
            rows[0].clone()
        };
        assert_eq!(visual.owner, owner);
        assert_eq!(layers, RenderLayers::layer(LAYER_3D));
        assert_eq!(
            transform.translation,
            Vec3::new(32.0, CONSTRUCTION_MASK_HEIGHT, 96.0)
        );

        app.world_mut().despawn(owner);
        app.update();

        assert!(app.world().get_entity(proxy).is_err());
    }
}
