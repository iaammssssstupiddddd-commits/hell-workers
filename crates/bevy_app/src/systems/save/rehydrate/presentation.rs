use super::*;

/// Removes only presentation entities created by this module's shell helpers.
/// This is deliberately narrower than M4's general load-reset registry.
pub(crate) fn clear_rehydrate_presentation(world: &mut World) {
    let presentation_entities: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, Or<(
            With<SoulProxy3d>,
            With<SoulMaskProxy3d>,
            With<SoulShadowProxy3d>,
            With<FamiliarProxy3d>,
            With<Building3dVisual>,
            With<crate::entities::familiar::FamiliarRangeIndicator>,
        )>>();
        query.iter(world).collect()
    };
    for entity in presentation_entities {
        world.despawn(entity);
    }
    if world.contains_resource::<SoulProxyOwnerCache>() {
        world.insert_resource(SoulProxyOwnerCache::default());
    }
}
