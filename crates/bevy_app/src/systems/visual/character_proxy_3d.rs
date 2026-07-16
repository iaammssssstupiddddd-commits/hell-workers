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
use bevy::world_serialization::WorldInstanceReady;
use hw_core::constants::{
    LAYER_3D, LAYER_3D_SOUL_MASK, LAYER_3D_SOUL_SHADOW, SOUL_FACE_SCALE_MULTIPLIER,
    SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES,
};
use hw_core::familiar::Familiar;
use hw_core::soul::DamnedSoul;
use hw_visual::familiar::FamiliarVisualOffset;
use hw_visual::visual3d::{FamiliarProxy3d, SoulMaskProxy3d, SoulProxy3d, SoulShadowProxy3d};
use hw_visual::{
    CharacterMaterial, SoulAnimationPlayer3d, SoulBodyAnimState, SoulFaceMaterial3d,
    SoulMaskMaterial, SoulProxyOwnerCache, SoulShadowMaterial,
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
type SoulTransformQuery<'w, 's> = Query<'w, 's, &'static Transform, With<DamnedSoul>>;
type ChangedSoulTransformQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Transform), (With<DamnedSoul>, Changed<Transform>)>;
type FamiliarTransformQuery<'w, 's> =
    Query<'w, 's, (&'static Transform, Option<&'static FamiliarVisualOffset>), With<Familiar>>;
type ChangedFamiliarTransformQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static FamiliarVisualOffset>,
    ),
    (With<Familiar>, Changed<Transform>),
>;
type ChangedFamiliarVisualOffsetQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static FamiliarVisualOffset),
    (With<Familiar>, Changed<FamiliarVisualOffset>),
>;
type Camera3dTransformQuery<'w, 's> = Query<'w, 's, Ref<'static, Transform>, With<Camera3dRtt>>;
type NewSoulProxyQuery<'w, 's> = Query<'w, 's, (Entity, &'static SoulProxy3d), Added<SoulProxy3d>>;
type NewSoulMaskProxyQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static SoulMaskProxy3d), Added<SoulMaskProxy3d>>;
type NewSoulShadowProxyQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static SoulShadowProxy3d), Added<SoulShadowProxy3d>>;
type NewFamiliarProxyQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static FamiliarProxy3d), Added<FamiliarProxy3d>>;

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

/// 値が実際に変わる場合だけproxy Transformへ書き込む。
///
/// `Mut`を`&mut Transform`へ即座にcoerceすると、比較だけでもChangedになる。
/// そのため読み取り後、差分があるfieldだけを更新する。
fn apply_proxy_transform(
    proxy_transform: &mut Mut<'_, Transform>,
    translation: Vec3,
    rotation: Quat,
) {
    if proxy_transform.translation != translation {
        proxy_transform.translation = translation;
    }
    if proxy_transform.rotation != rotation {
        proxy_transform.rotation = rotation;
    }
}

fn soul_proxy_transform(transform: &Transform) -> Vec3 {
    let position = transform.translation.truncate();
    Vec3::new(position.x, 0.0, -position.y)
}

fn familiar_proxy_transform(
    transform: &Transform,
    visual_offset: Option<&FamiliarVisualOffset>,
) -> Vec3 {
    let position = transform.translation.truncate();
    // Before M2 the 2D root's hover offset was part of `translation.y`, and
    // the proxy mapped it to Z. Preserve that established projection without
    // making the logical root itself visual-state dependent again.
    let hover_offset = visual_offset.map_or(0.0, |offset| offset.hover_offset);
    Vec3::new(position.x, 0.0, -(position.y + hover_offset))
}

/// SoulProxy3d を対応する DamnedSoul の 2D Transform に同期する。
pub fn sync_soul_proxy_3d_system(
    q_changed_souls: ChangedSoulTransformQuery,
    q_souls: SoulTransformQuery,
    q_new_proxies: NewSoulProxyQuery,
    q_cam3d: Camera3dTransformQuery,
    cache: Res<SoulProxyOwnerCache>,
    mut q_proxies: SoulProxy3dQuery,
) {
    let camera = q_cam3d.single().ok();
    let camera_changed = camera.as_ref().is_some_and(|camera| camera.is_changed());
    let cam_rotation = camera.map_or(Quat::IDENTITY, |camera| camera.rotation);

    if camera_changed {
        for (proxy, mut proxy_transform) in q_proxies.iter_mut() {
            if let Ok(soul_transform) = q_souls.get(proxy.owner) {
                let rotation = if proxy.billboard {
                    cam_rotation
                } else {
                    Quat::IDENTITY
                };
                apply_proxy_transform(
                    &mut proxy_transform,
                    soul_proxy_transform(soul_transform),
                    rotation,
                );
            }
        }
        return;
    }

    for (owner, soul_transform) in q_changed_souls.iter() {
        let Some(&proxy_entity) = cache.soul_proxy.get(&owner) else {
            continue;
        };
        let Ok((proxy, mut proxy_transform)) = q_proxies.get_mut(proxy_entity) else {
            continue;
        };
        let rotation = if proxy.billboard {
            cam_rotation
        } else {
            Quat::IDENTITY
        };
        apply_proxy_transform(
            &mut proxy_transform,
            soul_proxy_transform(soul_transform),
            rotation,
        );
    }

    for (proxy_entity, _) in q_new_proxies.iter() {
        let Ok((proxy, mut proxy_transform)) = q_proxies.get_mut(proxy_entity) else {
            continue;
        };
        let Ok(soul_transform) = q_souls.get(proxy.owner) else {
            continue;
        };
        let rotation = if proxy.billboard {
            cam_rotation
        } else {
            Quat::IDENTITY
        };
        apply_proxy_transform(
            &mut proxy_transform,
            soul_proxy_transform(soul_transform),
            rotation,
        );
    }
}

/// SoulMaskProxy3d を対応する DamnedSoul の 2D Transform に同期する。
pub fn sync_soul_mask_proxy_3d_system(
    q_changed_souls: ChangedSoulTransformQuery,
    q_souls: SoulTransformQuery,
    q_new_proxies: NewSoulMaskProxyQuery,
    cache: Res<SoulProxyOwnerCache>,
    mut q_proxies: SoulMaskProxy3dQuery,
) {
    for (owner, soul_transform) in q_changed_souls.iter() {
        let Some(&proxy_entity) = cache.soul_mask_proxy.get(&owner) else {
            continue;
        };
        let Ok((_proxy, mut proxy_transform)) = q_proxies.get_mut(proxy_entity) else {
            continue;
        };
        apply_proxy_transform(
            &mut proxy_transform,
            soul_proxy_transform(soul_transform),
            Quat::IDENTITY,
        );
    }

    for (proxy_entity, proxy) in q_new_proxies.iter() {
        let Ok((_proxy, mut proxy_transform)) = q_proxies.get_mut(proxy_entity) else {
            continue;
        };
        let Ok(soul_transform) = q_souls.get(proxy.owner) else {
            continue;
        };
        apply_proxy_transform(
            &mut proxy_transform,
            soul_proxy_transform(soul_transform),
            Quat::IDENTITY,
        );
    }
}

/// SoulShadowProxy3d を対応する DamnedSoul の 2D Transform に同期する。
pub fn sync_soul_shadow_proxy_3d_system(
    q_changed_souls: ChangedSoulTransformQuery,
    q_souls: SoulTransformQuery,
    q_new_proxies: NewSoulShadowProxyQuery,
    cache: Res<SoulProxyOwnerCache>,
    mut q_proxies: SoulShadowProxy3dQuery,
) {
    let pitch_correction =
        Quat::from_rotation_x((-SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES).to_radians());

    for (owner, soul_transform) in q_changed_souls.iter() {
        let Some(&proxy_entity) = cache.soul_shadow_proxy.get(&owner) else {
            continue;
        };
        let Ok((_proxy, mut proxy_transform)) = q_proxies.get_mut(proxy_entity) else {
            continue;
        };
        apply_proxy_transform(
            &mut proxy_transform,
            soul_proxy_transform(soul_transform),
            pitch_correction,
        );
    }

    for (proxy_entity, proxy) in q_new_proxies.iter() {
        let Ok((_proxy, mut proxy_transform)) = q_proxies.get_mut(proxy_entity) else {
            continue;
        };
        let Ok(soul_transform) = q_souls.get(proxy.owner) else {
            continue;
        };
        apply_proxy_transform(
            &mut proxy_transform,
            soul_proxy_transform(soul_transform),
            pitch_correction,
        );
    }
}

/// FamiliarProxy3d を対応する Familiar の論理rootとvisual hoverに同期する。
pub fn sync_familiar_proxy_3d_system(
    q_changed_familiars: ChangedFamiliarTransformQuery,
    q_changed_visual_offsets: ChangedFamiliarVisualOffsetQuery,
    q_familiars: FamiliarTransformQuery,
    q_new_proxies: NewFamiliarProxyQuery,
    q_cam3d: Camera3dTransformQuery,
    cache: Res<SoulProxyOwnerCache>,
    mut q_proxies: FamiliarProxy3dQuery,
) {
    let Ok(camera) = q_cam3d.single() else {
        return;
    };
    let camera_rotation = camera.rotation;
    if camera.is_changed() {
        for (proxy, mut proxy_transform) in q_proxies.iter_mut() {
            if let Ok((familiar_transform, visual_offset)) = q_familiars.get(proxy.owner) {
                apply_proxy_transform(
                    &mut proxy_transform,
                    familiar_proxy_transform(familiar_transform, visual_offset),
                    camera_rotation,
                );
            }
        }
        return;
    }

    for (owner, familiar_transform, visual_offset) in q_changed_familiars.iter() {
        sync_familiar_proxy_for_owner(
            owner,
            familiar_transform,
            visual_offset,
            camera_rotation,
            &cache,
            &mut q_proxies,
        );
    }
    for (owner, visual_offset) in q_changed_visual_offsets.iter() {
        let Ok((familiar_transform, _)) = q_familiars.get(owner) else {
            continue;
        };
        sync_familiar_proxy_for_owner(
            owner,
            familiar_transform,
            Some(visual_offset),
            camera_rotation,
            &cache,
            &mut q_proxies,
        );
    }
    for (proxy_entity, proxy) in q_new_proxies.iter() {
        let Ok((familiar_transform, visual_offset)) = q_familiars.get(proxy.owner) else {
            continue;
        };
        let Ok((_proxy, mut proxy_transform)) = q_proxies.get_mut(proxy_entity) else {
            continue;
        };
        apply_proxy_transform(
            &mut proxy_transform,
            familiar_proxy_transform(familiar_transform, visual_offset),
            camera_rotation,
        );
    }
}

fn sync_familiar_proxy_for_owner(
    owner: Entity,
    familiar_transform: &Transform,
    visual_offset: Option<&FamiliarVisualOffset>,
    camera_rotation: Quat,
    cache: &SoulProxyOwnerCache,
    q_proxies: &mut FamiliarProxy3dQuery,
) {
    let Some(&proxy_entity) = cache.familiar_proxy.get(&owner) else {
        return;
    };
    let Ok((_proxy, mut proxy_transform)) = q_proxies.get_mut(proxy_entity) else {
        return;
    };
    apply_proxy_transform(
        &mut proxy_transform,
        familiar_proxy_transform(familiar_transform, visual_offset),
        camera_rotation,
    );
}

/// 新規スポーンされた SoulProxy3d を SoulProxyOwnerCache に登録する。
pub fn register_soul_proxy_3d_system(
    q_new: Query<(Entity, &SoulProxy3d), Added<SoulProxy3d>>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for (proxy_entity, proxy) in q_new.iter() {
        cache.soul_proxy.insert(proxy.owner, proxy_entity);
    }
}

/// 新規スポーンされた SoulMaskProxy3d を SoulProxyOwnerCache に登録する。
pub fn register_soul_mask_proxy_3d_system(
    q_new: Query<(Entity, &SoulMaskProxy3d), Added<SoulMaskProxy3d>>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for (proxy_entity, proxy) in q_new.iter() {
        cache.soul_mask_proxy.insert(proxy.owner, proxy_entity);
    }
}

/// 新規スポーンされた SoulShadowProxy3d を SoulProxyOwnerCache に登録する。
pub fn register_soul_shadow_proxy_3d_system(
    q_new: Query<(Entity, &SoulShadowProxy3d), Added<SoulShadowProxy3d>>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for (proxy_entity, proxy) in q_new.iter() {
        cache.soul_shadow_proxy.insert(proxy.owner, proxy_entity);
    }
}

/// 新規スポーンされた FamiliarProxy3d を SoulProxyOwnerCache に登録する。
pub fn register_familiar_proxy_3d_system(
    q_new: Query<(Entity, &FamiliarProxy3d), Added<FamiliarProxy3d>>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for (proxy_entity, proxy) in q_new.iter() {
        cache.familiar_proxy.insert(proxy.owner, proxy_entity);
    }
}

/// DamnedSoul 削除時に対応する SoulProxy3d を despawn する。
pub fn cleanup_soul_proxy_3d_system(
    mut commands: Commands,
    mut removed: RemovedComponents<DamnedSoul>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for removed_entity in removed.read() {
        if let Some(proxy_entity) = cache.soul_proxy.remove(&removed_entity) {
            commands.entity(proxy_entity).despawn();
        }
    }
}

/// DamnedSoul 削除時に対応する SoulMaskProxy3d を despawn する。
pub fn cleanup_soul_mask_proxy_3d_system(
    mut commands: Commands,
    mut removed: RemovedComponents<DamnedSoul>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for removed_entity in removed.read() {
        if let Some(proxy_entity) = cache.soul_mask_proxy.remove(&removed_entity) {
            commands.entity(proxy_entity).despawn();
        }
    }
}

/// DamnedSoul 削除時に対応する SoulShadowProxy3d を despawn する。
pub fn cleanup_soul_shadow_proxy_3d_system(
    mut commands: Commands,
    mut removed: RemovedComponents<DamnedSoul>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for removed_entity in removed.read() {
        if let Some(proxy_entity) = cache.soul_shadow_proxy.remove(&removed_entity) {
            commands.entity(proxy_entity).despawn();
        }
    }
}

/// Familiar 削除時に対応する FamiliarProxy3d を despawn する。
pub fn cleanup_familiar_proxy_3d_system(
    mut commands: Commands,
    mut removed: RemovedComponents<Familiar>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for removed_entity in removed.read() {
        if let Some(proxy_entity) = cache.familiar_proxy.remove(&removed_entity) {
            commands.entity(proxy_entity).despawn();
        }
    }
}

/// Soul GLB の SceneRoot 子孫へ RenderLayers を付与し、RtT Camera3d に乗せる。
pub fn apply_soul_gltf_render_layers_on_ready(
    scene_ready: On<WorldInstanceReady>,
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
                directional_variant_lock_secs: 0.0,
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
                    last_applied_face: None,
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
    scene_ready: On<WorldInstanceReady>,
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
                directional_variant_lock_secs: 0.0,
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
                ))
                .insert(NotShadowCaster);
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
                ))
                .insert(NotShadowCaster);
        }
    }
}

/// Soul mask 用 SceneRoot 子孫へ RenderLayers と単色 mask material を付与する。
pub fn apply_soul_mask_gltf_render_layers_on_ready(
    scene_ready: On<WorldInstanceReady>,
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
