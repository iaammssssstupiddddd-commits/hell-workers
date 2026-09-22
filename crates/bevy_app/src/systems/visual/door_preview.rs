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
use hw_ui::components::BuildingCatalogPreview;
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

/// Catalog cards use the Closed EW preview, including cards created after
/// readiness settles. Do not copy the world sprite's size or ground anchor.
pub fn sync_door_catalog_preview_system(
    game_assets: Res<GameAssets>,
    production: Res<ProductionDoorAssetPool>,
    readiness: Res<DoorAssetReadiness>,
    mut cards: Query<(&BuildingCatalogPreview, &mut ImageNode)>,
) {
    let image = eligible_previews(&readiness, &production)
        .map_or(&game_assets.door_closed, |previews| previews[0]);
    for (preview, mut node) in &mut cards {
        if preview.0 == BuildingType::Door && node.image != *image {
            node.image = image.clone();
        }
    }
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
    use crate::assets::door_asset_set::{
        DoorAssetAuthority, DoorAssetFallbackReason, DoorAssetSetIdentity,
        ResolvedProductionDoorAssets,
    };
    use bevy::asset::uuid::Uuid;

    fn image_handle(value: u128) -> Handle<Image> {
        Uuid::from_u128(value).into()
    }

    #[test]
    fn catalog_tracks_late_readiness_generation_fallback_and_new_cards_while_paused() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<Font>()
            .init_asset::<Gltf>()
            .init_asset::<WorldAsset>();
        let server = app.world().resource::<AssetServer>().clone();
        let assets = crate::plugins::startup::create_game_assets(
            &server,
            &mut app.world_mut().resource_mut::<Assets<Image>>(),
        );
        let fallback = assets.door_closed.clone();
        app.insert_resource(assets)
            .init_resource::<DoorAssetReadiness>()
            .insert_resource(ProductionDoorAssetPool {
                manifest: default(),
                resolved: None,
            })
            .add_systems(PostUpdate, sync_door_catalog_preview_system);
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        let tint = Color::srgb(0.5, 0.6, 0.7);
        let door = app
            .world_mut()
            .spawn((
                BuildingCatalogPreview(BuildingType::Door),
                ImageNode {
                    color: tint,
                    ..ImageNode::new(fallback.clone())
                },
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(32.0),
                    ..default()
                },
            ))
            .id();
        let tank = app
            .world_mut()
            .spawn((
                BuildingCatalogPreview(BuildingType::Tank),
                ImageNode::new(image_handle(90)),
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<ImageNode>(door).unwrap().image, fallback);

        let mut resolved = ResolvedProductionDoorAssets {
            identity: DoorAssetSetIdentity {
                asset_set_generation: 7,
                authority: DoorAssetAuthority::ReleaseApproved,
                manifest_sha256: "generation-seven".into(),
            },
            meshes: default(),
            albedo: default(),
            preview_ew: image_handle(7),
            preview_ns: image_handle(17),
        };
        for generation in [7, 8] {
            resolved.identity.asset_set_generation = generation;
            resolved.preview_ew = image_handle(u128::from(generation));
            app.world_mut()
                .resource_mut::<ProductionDoorAssetPool>()
                .resolved = Some(resolved.clone());
            app.world_mut().resource_mut::<DoorAssetReadiness>().state =
                DoorAssetReadinessState::Eligible(resolved.identity.clone());
            app.update();
            assert_eq!(
                app.world().get::<ImageNode>(door).unwrap().image,
                resolved.preview_ew
            );
        }
        let new_card = app
            .world_mut()
            .spawn((
                BuildingCatalogPreview(BuildingType::Door),
                ImageNode::new(fallback.clone()),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<ImageNode>(new_card).unwrap().image,
            resolved.preview_ew
        );
        assert_eq!(app.world().get::<ImageNode>(door).unwrap().color, tint);
        assert_eq!(app.world().get::<Node>(door).unwrap().width, Val::Px(32.0));
        assert_eq!(
            app.world().get::<ImageNode>(tank).unwrap().image,
            image_handle(90)
        );

        let mut mismatched = resolved.identity.clone();
        mismatched.manifest_sha256 = "different-manifest".into();
        for state in [
            DoorAssetReadinessState::Eligible(mismatched),
            DoorAssetReadinessState::Loading,
            DoorAssetReadinessState::Fallback(DoorAssetFallbackReason::LoadFailed),
            DoorAssetReadinessState::Fallback(DoorAssetFallbackReason::CandidateDisabled),
        ] {
            app.world_mut().resource_mut::<DoorAssetReadiness>().state = state;
            app.update();
            for entity in [door, new_card] {
                assert_eq!(
                    app.world().get::<ImageNode>(entity).unwrap().image,
                    fallback
                );
            }
        }
        app.world_mut().resource_mut::<DoorAssetReadiness>().state =
            DoorAssetReadinessState::Eligible(resolved.identity);
        app.world_mut()
            .resource_mut::<ProductionDoorAssetPool>()
            .resolved = None;
        app.update();
        assert_eq!(app.world().get::<ImageNode>(door).unwrap().image, fallback);
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
