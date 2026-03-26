//! キャラクター3Dプロキシ同期・クリーンアップシステム
//!
//! DamnedSoul / Familiar の 2D Transform を対応する 3D プロキシエンティティに毎フレーム同期する。
//! 2D 座標 (x, y) → 3D 座標 (x, height/2, -y) の変換を使用する。

use crate::plugins::startup::Camera3dRtt;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use hw_core::constants::LAYER_3D;
use hw_core::familiar::Familiar;
use hw_core::soul::DamnedSoul;
use hw_visual::visual3d::{FamiliarProxy3d, SoulProxy3d};

type SoulProxy3dQuery<'w, 's> = Query<
    'w,
    's,
    (&'static SoulProxy3d, &'static mut Transform),
    (Without<DamnedSoul>, Without<Camera3dRtt>),
>;
type FamiliarProxy3dQuery<'w, 's> = Query<
    'w,
    's,
    (&'static FamiliarProxy3d, &'static mut Transform),
    (Without<Familiar>, Without<Camera3dRtt>),
>;

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
    q_soul_roots: Query<&SoulProxy3d>,
    q_children: Query<&Children>,
) {
    let Ok(proxy) = q_soul_roots.get(scene_ready.entity) else {
        return;
    };
    if proxy.billboard {
        return;
    }

    let render_layers = RenderLayers::layer(LAYER_3D);
    for child in q_children.iter_descendants(scene_ready.entity) {
        commands.entity(child).insert(render_layers.clone());
    }
}
