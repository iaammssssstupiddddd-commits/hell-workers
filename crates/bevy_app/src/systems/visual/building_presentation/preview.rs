use super::{asset_kind, production};
use crate::app_contexts::{BuildContext, CompanionPlacementState, MoveContext, TaskContext};
use crate::assets::GameAssets;
use crate::assets::building_asset_set::{BuildingAssetDescriptor, BuildingAssetPool};
use crate::systems::visual::placement_ghost::{PlacementGhost, PlacementPartnerGhost};
use bevy::prelude::*;
use bevy::sprite::Anchor;
use hw_core::game_state::{PlayMode, TaskMode};
use hw_core::visual_mirror::PoweredVisualState;
use hw_jobs::{Blueprint, Building, BuildingType, MovePlantTask};
use hw_ui::{components::BuildingCatalogPreview, setup::UiAssets};
use hw_visual::blueprint::BlueprintVisual;

/// Capture only geometry/handle fields owned by the descriptor. Pulse,
/// placement validity, construction opacity and bounce keep their own writers.
#[derive(Component, Clone)]
struct SpriteFallback {
    image: Handle<Image>,
    size: Option<Vec2>,
    anchor: Anchor,
}

fn apply(
    world: &mut World,
    entity: Entity,
    set: Option<&BuildingAssetDescriptor>,
    powered: Option<bool>,
) {
    let Some(sprite) = world.get::<Sprite>(entity) else {
        return;
    };
    if set.is_some() && world.get::<SpriteFallback>(entity).is_none() {
        let baseline = SpriteFallback {
            image: sprite.image.clone(),
            size: sprite.custom_size,
            anchor: world
                .get::<Anchor>(entity)
                .copied()
                .unwrap_or(Anchor::CENTER),
        };
        world.entity_mut(entity).insert(baseline);
    }
    if let Some(set) = set {
        let preview = &set.manifest.world_preview;
        let role = if powered == Some(true) {
            "world_on"
        } else {
            &preview.image_role
        };
        let image = set.image(role).expect("validated image role").clone();
        let anchor = Anchor(Vec2::new(
            preview.anchor_px[0] / preview.canvas_px[0] as f32 - 0.5,
            0.5 - preview.anchor_px[1] / preview.canvas_px[1] as f32,
        ));
        let mut sprite = world.get_mut::<Sprite>(entity).unwrap();
        if sprite.image != image {
            sprite.image = image;
        }
        let size = Some(Vec2::from_array(preview.canvas_wu));
        if sprite.custom_size != size {
            sprite.custom_size = size;
        }
        // Off/on images already encode Lamp brightness. Preserve the alpha.
        if powered.is_some() {
            let color = Color::WHITE.with_alpha(sprite.color.alpha());
            if sprite.color != color {
                sprite.color = color;
            }
        }
        if world.get::<Anchor>(entity) != Some(&anchor) {
            world.entity_mut(entity).insert(anchor);
        }
    } else if let Some(baseline) = world.get::<SpriteFallback>(entity).cloned() {
        let mut sprite = world.get_mut::<Sprite>(entity).unwrap();
        sprite.image = baseline.image;
        sprite.custom_size = baseline.size;
        if let Some(powered) = powered {
            sprite.color = if powered {
                Color::WHITE
            } else {
                Color::srgba(0.4, 0.4, 0.4, 1.0)
            };
        }
        world
            .entity_mut(entity)
            .insert(baseline.anchor)
            .remove::<SpriteFallback>();
    }
}

fn fallback_image(assets: &GameAssets, kind: BuildingType) -> Handle<Image> {
    match kind {
        BuildingType::Wall => assets.wall_isolated.clone(),
        BuildingType::Door => assets.door_closed.clone(),
        BuildingType::Floor => assets.mud_floor.clone(),
        BuildingType::Tank => assets.tank_empty.clone(),
        BuildingType::MudMixer => assets.mud_mixer.clone(),
        BuildingType::RestArea | BuildingType::SoulSpa => assets.rest_area.clone(),
        BuildingType::Bridge => assets.bridge.clone(),
        BuildingType::SandPile => assets.sand_pile.clone(),
        BuildingType::BonePile | BuildingType::OutdoorLamp => assets.bone_pile.clone(),
        BuildingType::WheelbarrowParking => assets.wheelbarrow_parking.clone(),
    }
}

fn ghost_kind(world: &World) -> Option<BuildingType> {
    if world.resource::<CompanionPlacementState>().0.is_some() {
        return None;
    }
    if matches!(world.resource::<TaskContext>().0, TaskMode::SoulSpaPlace(_)) {
        return Some(BuildingType::SoulSpa);
    }
    match world.resource::<State<PlayMode>>().get() {
        PlayMode::BuildingMove => world
            .resource::<MoveContext>()
            .0
            .and_then(|owner| world.get::<Building>(owner).map(|building| building.kind)),
        PlayMode::BuildingPlace => world.resource::<BuildContext>().0,
        _ => None,
    }
}

/// Runs after the legacy preview writers, including Door. A reused ghost always
/// gets the current mode's baseline before applying a descriptor, so an old
/// production anchor cannot leak into a bucket, Wall, or other non-target.
pub fn sync_building_previews(world: &mut World) {
    world.resource_scope(|world, pool: Mut<BuildingAssetPool>| {
        world.resource_scope(|world, assets: Mut<GameAssets>| {
            sync_previews(world, &pool, &assets);
        });
    });
}

fn sync_previews(world: &mut World, pool: &BuildingAssetPool, assets: &GameAssets) {
    let mut consumers = Vec::new();
    let mut overlays = Vec::new();
    let mut buildings = world.query::<(
        Entity,
        &Building,
        Option<&PoweredVisualState>,
        Option<&Children>,
    )>();
    for (entity, building, power, children) in buildings.iter(world) {
        if asset_kind(building.kind).is_some_and(|kind| kind.mesh_roles().is_empty()) {
            let powered = (building.kind == BuildingType::OutdoorLamp)
                .then_some(power.is_some_and(|power| power.is_powered));
            if world.get::<Sprite>(entity).is_some() {
                consumers.push((entity, building.kind, powered));
            }
            for child in children.into_iter().flat_map(|children| children.iter()) {
                if world
                    .get::<hw_visual::layer::VisualLayerKind>(child)
                    .is_some()
                {
                    consumers.push((child, building.kind, powered));
                }
            }
        }
    }
    let mut blueprints = world.query::<(Entity, &Blueprint, Option<&BlueprintVisual>)>();
    for (entity, blueprint, visual) in blueprints.iter(world) {
        if asset_kind(blueprint.kind).is_none() {
            continue;
        }
        consumers.push((entity, blueprint.kind, None));
        if let Some(overlay) = visual.and_then(|visual| visual.pulse_overlay) {
            overlays.push((overlay.entity, entity, blueprint.kind));
        }
    }
    let mut destinations = world.query::<(Entity, &MovePlantTask)>();
    for (entity, task) in destinations.iter(world) {
        if let Some(building) = world.get::<Building>(task.building) {
            consumers.push((entity, building.kind, None));
        }
    }
    for (entity, kind, powered) in consumers {
        apply(world, entity, production(pool, kind), powered);
    }
    for (overlay, parent, kind) in overlays {
        let set = production(pool, kind);
        // Pulse children can be created after the parent already uses a set.
        // Their copied sprite is not a fallback; inherit the parent's baseline.
        if set.is_some()
            && let Some(baseline) = world.get::<SpriteFallback>(parent).cloned()
            && let Ok(mut child) = world.get_entity_mut(overlay)
        {
            child.insert(baseline);
        }
        apply(world, overlay, set, None);
    }
    let current_kind = ghost_kind(world);
    let mut ghosts = world.query_filtered::<Entity, With<PlacementGhost>>();
    let ghosts: Vec<_> = ghosts.iter(world).collect();
    for entity in ghosts {
        if current_kind == Some(BuildingType::Door) {
            // Door owns its final sprite. Release our old fallback handles only.
            world.entity_mut(entity).remove::<SpriteFallback>();
            continue;
        }
        world
            .entity_mut(entity)
            .remove::<SpriteFallback>()
            .insert(Anchor::CENTER);
        if let Some(kind) = current_kind {
            if let Some(mut sprite) = world.get_mut::<Sprite>(entity) {
                sprite.image = fallback_image(assets, kind);
                sprite.custom_size =
                    Some(crate::interface::selection::placement_geometry::building_size(kind));
            }
            apply(world, entity, production(pool, kind), None);
        } else if world.resource::<CompanionPlacementState>().0.is_some()
            && let Some(mut sprite) = world.get_mut::<Sprite>(entity)
        {
            sprite.image = assets.bucket_empty.clone();
            sprite.custom_size = Some(Vec2::new(2.0, 1.0) * hw_core::constants::TILE_SIZE);
        }
    }
    let mut partners = world.query_filtered::<Entity, With<PlacementPartnerGhost>>();
    let partners: Vec<_> = partners.iter(world).collect();
    for entity in partners {
        // Both placement and move partner ghosts represent the Tank parent.
        apply(world, entity, production(pool, BuildingType::Tank), None);
    }
    let mut cards = world.query::<(&BuildingCatalogPreview, &mut ImageNode)>();
    for (preview, mut node) in cards.iter_mut(world) {
        if asset_kind(preview.0).is_none() {
            continue;
        }
        let image = production(pool, preview.0)
            .and_then(|set| set.image(&set.manifest.catalog_preview.image_role))
            .unwrap_or_else(|| assets.building_preview(preview.0));
        if node.image != *image {
            node.image = image.clone();
        }
    }
}
