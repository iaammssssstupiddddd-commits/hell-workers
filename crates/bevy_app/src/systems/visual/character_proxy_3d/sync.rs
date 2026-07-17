use super::*;

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

pub(super) fn soul_proxy_transform(transform: &Transform) -> Vec3 {
    let position = transform.translation.truncate();
    Vec3::new(position.x, 0.0, -position.y)
}

pub(super) fn familiar_proxy_transform(
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
