//! Keeps Door placement and construction sprites on the same production axis
//! and asset-set identity as the active 3D presentation.

use crate::app_contexts::BuildContext;
use crate::assets::GameAssets;
use crate::assets::door_asset_set::{
    DoorAssetReadiness, DoorAssetReadinessState, ProductionDoorAssetPool,
};
use crate::systems::jobs::{Blueprint, BuildingType};
use crate::systems::visual::placement_ghost::PlacementGhost;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use hw_core::constants::TILE_SIZE;
use hw_visual::blueprint::{BlueprintPulseOverlayChild, BlueprintVisual};
use hw_visual::visual3d::{DoorPresentationAxis, resolve_door_presentation_axis};
use hw_visual::wall_connection::WallTopologyIndex;
use hw_world::WorldMap;

const PRODUCTION_PREVIEW_SIZE: Vec2 = Vec2::splat(64.0);
const PRODUCTION_PREVIEW_ANCHOR: Anchor = Anchor(Vec2::new(0.0, -0.25));

fn eligible_previews<'a>(
    readiness: &DoorAssetReadiness,
    pool: &'a ProductionDoorAssetPool,
) -> Option<[&'a Handle<Image>; 2]> {
    let DoorAssetReadinessState::Eligible(identity) = &readiness.state else {
        return None;
    };
    let resolved = pool.resolved.as_ref()?;
    (&resolved.identity == identity).then_some([&resolved.preview_ew, &resolved.preview_ns])
}

fn preview_axis(topology: &WallTopologyIndex, transform: &Transform) -> DoorPresentationAxis {
    topology
        .connection_mask(WorldMap::world_to_grid(transform.translation.truncate()))
        .map(resolve_door_presentation_axis)
        .unwrap_or_default()
}

fn apply_preview(
    sprite: &mut Sprite,
    anchor: &mut Anchor,
    production: Option<&Handle<Image>>,
    fallback: &Handle<Image>,
) {
    let (image, size, next_anchor) = production.map_or(
        (fallback, Vec2::splat(TILE_SIZE), Anchor::CENTER),
        |image| (image, PRODUCTION_PREVIEW_SIZE, PRODUCTION_PREVIEW_ANCHOR),
    );
    if sprite.image != *image {
        sprite.image = image.clone();
    }
    if sprite.custom_size != Some(size) {
        sprite.custom_size = Some(size);
    }
    if *anchor != next_anchor {
        *anchor = next_anchor;
    }
}

type DoorBlueprintQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Blueprint,
        &'static Transform,
        &'static mut Sprite,
        &'static mut Anchor,
        Option<&'static BlueprintVisual>,
    ),
    (Without<PlacementGhost>, Without<BlueprintPulseOverlayChild>),
>;

type DoorOverlayQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Sprite, &'static mut Anchor),
    (
        With<BlueprintPulseOverlayChild>,
        Without<Blueprint>,
        Without<PlacementGhost>,
    ),
>;

type DoorGhostQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Transform, &'static mut Sprite, &'static mut Anchor),
    (
        With<PlacementGhost>,
        Without<Blueprint>,
        Without<BlueprintPulseOverlayChild>,
    ),
>;

#[derive(bevy::ecs::system::SystemParam)]
pub struct DoorPreviewParams<'w, 's> {
    game_assets: Res<'w, GameAssets>,
    production: Res<'w, ProductionDoorAssetPool>,
    readiness: Res<'w, DoorAssetReadiness>,
    topology: Res<'w, WallTopologyIndex>,
    build_context: Res<'w, BuildContext>,
    blueprints: DoorBlueprintQuery<'w, 's>,
    overlays: DoorOverlayQuery<'w, 's>,
    ghosts: DoorGhostQuery<'w, 's>,
}

/// Applies one atomically eligible Door asset-set generation to root, pulse
/// child, and placement sprites. A missing or rejected candidate always
/// restores the existing visible fallback PNG.
pub fn sync_door_preview_system(mut params: DoorPreviewParams) {
    let previews = eligible_previews(&params.readiness, &params.production);
    for (blueprint, transform, mut sprite, mut anchor, visual) in &mut params.blueprints {
        if blueprint.kind != BuildingType::Door {
            continue;
        }
        let axis = preview_axis(&params.topology, transform);
        let production = previews.map(|images| match axis {
            DoorPresentationAxis::EastWest => images[0],
            DoorPresentationAxis::NorthSouth => images[1],
        });
        apply_preview(
            &mut sprite,
            &mut anchor,
            production,
            &params.game_assets.door_closed,
        );
        if let Some(overlay) = visual.and_then(|visual| visual.pulse_overlay)
            && let Ok((mut overlay_sprite, mut overlay_anchor)) =
                params.overlays.get_mut(overlay.entity)
        {
            apply_preview(
                &mut overlay_sprite,
                &mut overlay_anchor,
                production,
                &params.game_assets.door_closed,
            );
        }
    }

    if params.build_context.0 != Some(BuildingType::Door) {
        return;
    }
    for (transform, mut sprite, mut anchor) in &mut params.ghosts {
        let axis = preview_axis(&params.topology, transform);
        let production = previews.map(|images| match axis {
            DoorPresentationAxis::EastWest => images[0],
            DoorPresentationAxis::NorthSouth => images[1],
        });
        apply_preview(
            &mut sprite,
            &mut anchor,
            production,
            &params.game_assets.door_closed,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::uuid::Uuid;

    fn image_handle(value: u128) -> Handle<Image> {
        Uuid::from_u128(value).into()
    }

    #[test]
    fn production_preview_sets_canvas_anchor_without_changing_tint() {
        let fallback = image_handle(1);
        let production = image_handle(2);
        let color = Color::srgba(0.5, 1.0, 0.5, 0.5);
        let mut sprite = Sprite {
            image: fallback.clone(),
            color,
            custom_size: Some(Vec2::splat(TILE_SIZE)),
            ..default()
        };
        let mut anchor = Anchor::CENTER;

        apply_preview(&mut sprite, &mut anchor, Some(&production), &fallback);

        assert_eq!(sprite.image, production);
        assert_eq!(sprite.custom_size, Some(PRODUCTION_PREVIEW_SIZE));
        assert_eq!(anchor, PRODUCTION_PREVIEW_ANCHOR);
        assert_eq!(sprite.color, color);
    }

    #[test]
    fn rejected_preview_restores_visible_fallback_contract() {
        let fallback = image_handle(1);
        let mut sprite = Sprite {
            image: image_handle(2),
            custom_size: Some(PRODUCTION_PREVIEW_SIZE),
            ..default()
        };
        let mut anchor = PRODUCTION_PREVIEW_ANCHOR;

        apply_preview(&mut sprite, &mut anchor, None, &fallback);

        assert_eq!(sprite.image, fallback);
        assert_eq!(sprite.custom_size, Some(Vec2::splat(TILE_SIZE)));
        assert_eq!(anchor, Anchor::CENTER);
    }
}
