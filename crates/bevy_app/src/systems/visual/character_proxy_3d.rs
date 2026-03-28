//! キャラクター3Dプロキシ同期・クリーンアップシステム
//!
//! DamnedSoul / Familiar の 2D Transform を対応する 3D プロキシエンティティに毎フレーム同期する。
//! 2D 座標 (x, y) → 3D 座標 (x, height/2, -y) の変換を使用する。

use crate::plugins::startup::{Camera3dRtt, CharacterHandles};
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::gltf::GltfMeshName;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::Mesh3d;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use hw_core::constants::{
    LAYER_3D, LAYER_3D_SOUL_MASK, LAYER_3D_SOUL_SHADOW, SOUL_FACE_SCALE_MULTIPLIER,
    SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES,
};
use hw_core::familiar::Familiar;
use hw_core::soul::DamnedSoul;
use hw_visual::visual3d::{FamiliarProxy3d, SoulMaskProxy3d, SoulProxy3d, SoulShadowProxy3d};
use hw_visual::{
    CharacterMaterial, SoulAnimationPlayer3d, SoulBodyAnimState, SoulFaceMaterial3d,
    SoulMaskMaterial, SoulShadowMaterial,
};

type SoulProxy3dQuery<'w, 's> = Query<
    'w,
    's,
    (&'static SoulProxy3d, &'static mut Transform),
    (Without<DamnedSoul>, Without<Camera3dRtt>),
>;
type SoulMaskProxy3dQuery<'w, 's> = Query<
    'w,
    's,
    (&'static SoulMaskProxy3d, &'static mut Transform),
    (Without<DamnedSoul>, Without<Camera3dRtt>),
>;
type SoulShadowProxy3dQuery<'w, 's> = Query<
    'w,
    's,
    (&'static SoulShadowProxy3d, &'static mut Transform),
    (Without<DamnedSoul>, Without<Camera3dRtt>),
>;
type FamiliarProxy3dQuery<'w, 's> = Query<
    'w,
    's,
    (&'static FamiliarProxy3d, &'static mut Transform),
    (Without<Familiar>, Without<Camera3dRtt>),
>;
type SoulProxyRootsQuery<'w, 's> = Query<'w, 's, &'static SoulProxy3d>;
type SoulMaskProxyRootsQuery<'w, 's> = Query<'w, 's, &'static SoulMaskProxy3d>;
type SoulShadowProxyRootsQuery<'w, 's> = Query<'w, 's, &'static SoulShadowProxy3d>;
type ChildListQuery<'w, 's> = Query<'w, 's, &'static Children>;
type TransformReadQuery<'w, 's> = Query<'w, 's, &'static Transform>;
type MeshNameQuery<'w, 's> = Query<'w, 's, &'static GltfMeshName>;
type NameQuery<'w, 's> = Query<'w, 's, &'static Name>;
type MeshMarkerQuery<'w, 's> = Query<'w, 's, (), With<Mesh3d>>;
type AnimationPlayerMarkerQuery<'w, 's> = Query<'w, 's, (), With<AnimationPlayer>>;

#[derive(SystemParam)]
pub struct SoulGltfApplyParams<'w, 's> {
    q_soul_roots: SoulProxyRootsQuery<'w, 's>,
    q_children: ChildListQuery<'w, 's>,
    q_transforms: TransformReadQuery<'w, 's>,
    q_mesh_names: MeshNameQuery<'w, 's>,
    q_names: NameQuery<'w, 's>,
    q_meshes: MeshMarkerQuery<'w, 's>,
    q_animation_players: AnimationPlayerMarkerQuery<'w, 's>,
}

#[derive(SystemParam)]
pub struct SoulMaskGltfApplyParams<'w, 's> {
    q_soul_mask_roots: SoulMaskProxyRootsQuery<'w, 's>,
    q_children: ChildListQuery<'w, 's>,
    q_meshes: MeshMarkerQuery<'w, 's>,
}

#[derive(SystemParam)]
pub struct SoulShadowGltfApplyParams<'w, 's> {
    q_soul_shadow_roots: SoulShadowProxyRootsQuery<'w, 's>,
    q_children: ChildListQuery<'w, 's>,
    q_mesh_names: MeshNameQuery<'w, 's>,
    q_names: NameQuery<'w, 's>,
    q_meshes: MeshMarkerQuery<'w, 's>,
    q_animation_players: AnimationPlayerMarkerQuery<'w, 's>,
}

/// SoulProxy3d を対応する DamnedSoul の 2D Transform に同期する。
pub fn sync_soul_proxy_3d_system(
    q_souls: Query<(Entity, &Transform), With<DamnedSoul>>,
    q_cam3d: Query<&Transform, With<Camera3dRtt>>,
    mut q_proxies: SoulProxy3dQuery,
) {
    let cam_rotation = q_cam3d.single().ok().map(|cam3d| cam3d.rotation);

    for (proxy, mut proxy_transform) in q_proxies.iter_mut() {
        if let Ok((_, soul_transform)) = q_souls.get(proxy.owner) {
            let pos2d = soul_transform.translation.truncate();
            proxy_transform.translation.x = pos2d.x;
            // y（高度）は固定値のまま変更しない
            proxy_transform.translation.z = -pos2d.y;
            proxy_transform.rotation = if proxy.billboard {
                cam_rotation.unwrap_or(Quat::IDENTITY)
            } else {
                Quat::IDENTITY
            };
        }
    }
}

/// SoulMaskProxy3d を対応する DamnedSoul の 2D Transform に同期する。
pub fn sync_soul_mask_proxy_3d_system(
    q_souls: Query<(Entity, &Transform), With<DamnedSoul>>,
    mut q_proxies: SoulMaskProxy3dQuery,
) {
    for (proxy, mut proxy_transform) in q_proxies.iter_mut() {
        if let Ok((_, soul_transform)) = q_souls.get(proxy.owner) {
            let pos2d = soul_transform.translation.truncate();
            proxy_transform.translation.x = pos2d.x;
            proxy_transform.translation.z = -pos2d.y;
            proxy_transform.rotation = Quat::IDENTITY;
        }
    }
}

/// SoulShadowProxy3d を対応する DamnedSoul の 2D Transform に同期する。
pub fn sync_soul_shadow_proxy_3d_system(
    q_souls: Query<(Entity, &Transform), With<DamnedSoul>>,
    mut q_proxies: SoulShadowProxy3dQuery,
) {
    let pitch_correction =
        Quat::from_rotation_x(SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES.to_radians());

    for (proxy, mut proxy_transform) in q_proxies.iter_mut() {
        if let Ok((_, soul_transform)) = q_souls.get(proxy.owner) {
            let pos2d = soul_transform.translation.truncate();
            proxy_transform.translation.x = pos2d.x;
            proxy_transform.translation.z = -pos2d.y;
            proxy_transform.rotation = pitch_correction;
        }
    }
}

/// FamiliarProxy3d を対応する Familiar の 2D Transform に同期する。
pub fn sync_familiar_proxy_3d_system(
    q_familiars: Query<(Entity, &Transform), With<Familiar>>,
    q_cam3d: Query<&Transform, With<Camera3dRtt>>,
    mut q_proxies: FamiliarProxy3dQuery,
) {
    let Ok(cam3d) = q_cam3d.single() else { return };

    for (proxy, mut proxy_transform) in q_proxies.iter_mut() {
        if let Ok((_, fam_transform)) = q_familiars.get(proxy.owner) {
            let pos2d = fam_transform.translation.truncate();
            proxy_transform.translation.x = pos2d.x;
            proxy_transform.translation.z = -pos2d.y;
            proxy_transform.rotation = cam3d.rotation;
        }
    }
}

/// DamnedSoul 削除時に対応する SoulProxy3d を despawn する。
pub fn cleanup_soul_proxy_3d_system(
    mut commands: Commands,
    mut removed: RemovedComponents<DamnedSoul>,
    q_proxies: Query<(Entity, &SoulProxy3d)>,
) {
    for removed_entity in removed.read() {
        for (proxy_entity, proxy) in q_proxies.iter() {
            if proxy.owner == removed_entity {
                commands.entity(proxy_entity).despawn();
            }
        }
    }
}

/// DamnedSoul 削除時に対応する SoulMaskProxy3d を despawn する。
pub fn cleanup_soul_mask_proxy_3d_system(
    mut commands: Commands,
    mut removed: RemovedComponents<DamnedSoul>,
    q_proxies: Query<(Entity, &SoulMaskProxy3d)>,
) {
    for removed_entity in removed.read() {
        for (proxy_entity, proxy) in q_proxies.iter() {
            if proxy.owner == removed_entity {
                commands.entity(proxy_entity).despawn();
            }
        }
    }
}

/// DamnedSoul 削除時に対応する SoulShadowProxy3d を despawn する。
pub fn cleanup_soul_shadow_proxy_3d_system(
    mut commands: Commands,
    mut removed: RemovedComponents<DamnedSoul>,
    q_proxies: Query<(Entity, &SoulShadowProxy3d)>,
) {
    for removed_entity in removed.read() {
        for (proxy_entity, proxy) in q_proxies.iter() {
            if proxy.owner == removed_entity {
                commands.entity(proxy_entity).despawn();
            }
        }
    }
}

/// Familiar 削除時に対応する FamiliarProxy3d を despawn する。
pub fn cleanup_familiar_proxy_3d_system(
    mut commands: Commands,
    mut removed: RemovedComponents<Familiar>,
    q_proxies: Query<(Entity, &FamiliarProxy3d)>,
) {
    for removed_entity in removed.read() {
        for (proxy_entity, proxy) in q_proxies.iter() {
            if proxy.owner == removed_entity {
                commands.entity(proxy_entity).despawn();
            }
        }
    }
}

/// Soul GLB の SceneRoot 子孫へ RenderLayers を付与し、RtT Camera3d に乗せる。
pub fn apply_soul_gltf_render_layers_on_ready(
    scene_ready: On<SceneInstanceReady>,
    mut commands: Commands,
    character_handles: Res<CharacterHandles>,
    mut character_materials: ResMut<Assets<CharacterMaterial>>,
    params: SoulGltfApplyParams,
) {
    let Ok(proxy) = params.q_soul_roots.get(scene_ready.entity) else {
        return;
    };
    if proxy.billboard {
        return;
    }

    let render_layers = RenderLayers::layer(LAYER_3D);
    for child in params.q_children.iter_descendants(scene_ready.entity) {
        let mut entity_commands = commands.entity(child);
        entity_commands.insert(render_layers.clone());

        if params.q_animation_players.get(child).is_ok() {
            entity_commands.insert(SoulAnimationPlayer3d {
                owner: proxy.owner,
                current_body: SoulBodyAnimState::Idle,
                walk_facing_right: None,
                last_owner_pos: None,
                walk_variant_lock_secs: 0.0,
            });
        }

        let is_mesh_entity = params.q_meshes.get(child).is_ok();
        if !is_mesh_entity {
            continue;
        }
        entity_commands.insert(NotShadowCaster);
        let mesh_name = params
            .q_mesh_names
            .get(child)
            .ok()
            .map(|name| name.0.as_str());
        let name = params.q_names.get(child).ok().map(Name::as_str);
        let is_face_mesh =
            matches!(mesh_name, Some("Soul_Face_Mesh")) || matches!(name, Some("Soul_Face_Mesh"));
        if is_face_mesh {
            if let Ok(face_transform) = params.q_transforms.get(child) {
                let mut scaled_face = *face_transform;
                scaled_face.scale *=
                    Vec3::new(SOUL_FACE_SCALE_MULTIPLIER, SOUL_FACE_SCALE_MULTIPLIER, 1.0);
                entity_commands.insert(scaled_face);
            }
            let face_material = character_materials
                .get(&character_handles.soul_face_material)
                .cloned()
                .map(|template| character_materials.add(template))
                .unwrap_or_else(|| character_handles.soul_face_material.clone());
            entity_commands
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(face_material.clone()))
                .insert(SoulFaceMaterial3d {
                    owner: proxy.owner,
                    material: face_material,
                });
            continue;
        }

        let is_body_mesh = matches!(mesh_name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010.SoulMat"));
        if is_body_mesh {
            entity_commands
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d::<CharacterMaterial>(
                    character_handles.soul_body_material.clone(),
                ));
        }
    }
}

/// Soul shadow proxy 用 SceneRoot 子孫へ RenderLayers と shadow proxy 設定を付与する。
pub fn apply_soul_shadow_gltf_render_layers_on_ready(
    scene_ready: On<SceneInstanceReady>,
    mut commands: Commands,
    character_handles: Res<CharacterHandles>,
    params: SoulShadowGltfApplyParams,
) {
    let Ok(proxy) = params.q_soul_shadow_roots.get(scene_ready.entity) else {
        return;
    };

    let render_layers = RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SOUL_SHADOW]);
    for child in params.q_children.iter_descendants(scene_ready.entity) {
        let mut entity_commands = commands.entity(child);
        entity_commands.insert((render_layers.clone(), NotShadowReceiver));
        entity_commands.remove::<NotShadowCaster>();

        if params.q_animation_players.get(child).is_ok() {
            entity_commands.insert(SoulAnimationPlayer3d {
                owner: proxy.owner,
                current_body: SoulBodyAnimState::Idle,
                walk_facing_right: None,
                last_owner_pos: None,
                walk_variant_lock_secs: 0.0,
            });
        }

        if params.q_meshes.get(child).is_err() {
            continue;
        }

        let mesh_name = params
            .q_mesh_names
            .get(child)
            .ok()
            .map(|name| name.0.as_str());
        let name = params.q_names.get(child).ok().map(Name::as_str);
        let is_face_mesh =
            matches!(mesh_name, Some("Soul_Face_Mesh")) || matches!(name, Some("Soul_Face_Mesh"));
        if is_face_mesh {
            entity_commands
                .remove::<MeshMaterial3d<CharacterMaterial>>()
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .remove::<MeshMaterial3d<SoulShadowMaterial>>()
                .insert(MeshMaterial3d(
                    character_handles.soul_shadow_proxy_material.clone(),
                ));
            continue;
        }

        let is_body_mesh = matches!(mesh_name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010.SoulMat"));
        if is_body_mesh {
            entity_commands
                .remove::<MeshMaterial3d<CharacterMaterial>>()
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .remove::<MeshMaterial3d<SoulShadowMaterial>>()
                .insert(MeshMaterial3d(
                    character_handles.soul_shadow_proxy_material.clone(),
                ));
        }
    }
}

/// Soul mask 用 SceneRoot 子孫へ RenderLayers と単色 mask material を付与する。
pub fn apply_soul_mask_gltf_render_layers_on_ready(
    scene_ready: On<SceneInstanceReady>,
    mut commands: Commands,
    character_handles: Res<CharacterHandles>,
    params: SoulMaskGltfApplyParams,
) {
    let Ok(_proxy) = params.q_soul_mask_roots.get(scene_ready.entity) else {
        return;
    };

    let render_layers = RenderLayers::layer(LAYER_3D_SOUL_MASK);
    for child in params.q_children.iter_descendants(scene_ready.entity) {
        let mut entity_commands = commands.entity(child);
        entity_commands.insert(render_layers.clone());

        if params.q_meshes.get(child).is_err() {
            continue;
        }

        entity_commands
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .remove::<MeshMaterial3d<CharacterMaterial>>()
            .insert(MeshMaterial3d::<SoulMaskMaterial>(
                character_handles.soul_mask_material.clone(),
            ));
    }
}
