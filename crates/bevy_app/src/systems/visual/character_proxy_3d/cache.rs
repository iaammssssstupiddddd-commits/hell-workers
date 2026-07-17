use super::*;

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
