//! Passive M5 2D consumer inventory in an explicitly admitted ordinary UI case.
use super::{BuildingAssetPool, BuildingAssetSetIdentity};
use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};
use hw_jobs::{Blueprint, Building, BuildingType};
use serde_json::{Value, json};

fn pose(value: &Transform) -> Value {
    json!({"translation": value.translation.to_array(), "rotation": value.rotation.to_array(), "scale": value.scale.to_array()})
}

fn sprite(world: &World, entity: Entity, owner: Entity, role: &str) -> Result<Value, String> {
    let sprite = world
        .get::<Sprite>(entity)
        .ok_or("M5 consumer Sprite missing")?;
    let transform = world
        .get::<Transform>(entity)
        .ok_or("M5 consumer pose missing")?;
    let global = world
        .get::<GlobalTransform>(entity)
        .ok_or("M5 consumer global pose missing")?;
    Ok(
        json!({"entity": entity.to_bits(), "owner": owner.to_bits(), "consumer": role,
        "parent": world.get::<ChildOf>(entity).map(|parent| parent.parent().to_bits()),
        "image": format!("{:?}", sprite.image.id()), "size": sprite.custom_size.map(|size| size.to_array()),
        "whole_image": sprite.texture_atlas.is_none() && sprite.rect.is_none() && !sprite.flip_x && !sprite.flip_y,
        "color": sprite.color.to_srgba().to_f32_array(), "anchor": world.get::<Anchor>(entity).map_or(Vec2::ZERO, |anchor| anchor.0).to_array(),
        "pose": pose(transform), "global_matrix": global.to_matrix().to_cols_array(),
        "blueprint_state": world.get::<hw_visual::blueprint::BlueprintVisual>(owner).map(|visual| format!("{:?}", visual.state)),
        "blueprint_progress": world.get::<hw_core::visual_mirror::construction::BlueprintVisualState>(owner).map(|state| state.progress),
        "layers": world.get::<RenderLayers>(entity).cloned().unwrap_or_default().iter().collect::<Vec<_>>(),
        "visible": world.get::<ViewVisibility>(entity).is_some_and(|visible| visible.get())}),
    )
}

pub(super) fn snapshot(
    world: &mut World,
    identity: &BuildingAssetSetIdentity,
) -> Result<Value, String> {
    use crate::systems::visual::placement_ghost::PlacementGhost;
    use hw_ui::components::BuildingCatalogPreview;
    use hw_visual::blueprint::BlueprintVisual;
    let kind = match identity.kind {
        super::BuildingAssetKind::SandPile => BuildingType::SandPile,
        super::BuildingAssetKind::BonePile => BuildingType::BonePile,
        super::BuildingAssetKind::WheelbarrowParking => BuildingType::WheelbarrowParking,
        super::BuildingAssetKind::OutdoorLamp => BuildingType::OutdoorLamp,
        _ => return Err("ordinary M5 cannot observe another kind".into()),
    };
    let pool = world.resource::<BuildingAssetPool>();
    let active = pool.active(identity.kind).cloned();
    let pending = pool.pending(identity.kind).cloned();
    let descriptor = pool.descriptor(identity.kind).cloned();
    let descriptor = descriptor.map(|descriptor| json!({"identity": descriptor.manifest.identity,
        "world": descriptor.manifest.world_preview, "catalog": descriptor.manifest.catalog_preview,
        "images": descriptor.manifest.identity.kind.image_roles().iter().map(|role| (*role, descriptor.image(role).map(|image| format!("{:?}", image.id())))).collect::<std::collections::BTreeMap<_, _>>() }));
    let buildings = world
        .query::<(Entity, &Building, Option<&Children>)>()
        .iter(world)
        .filter(|(_, building, _)| building.kind == kind)
        .map(|(entity, _, children)| {
            (
                entity,
                children
                    .map(|children| children.iter().collect::<Vec<_>>())
                    .unwrap_or_default(),
            )
        })
        .take(129)
        .collect::<Vec<_>>();
    let blueprints = world
        .query::<(Entity, &Blueprint, Option<&BlueprintVisual>)>()
        .iter(world)
        .filter(|(_, blueprint, _)| blueprint.kind == kind)
        .map(|(entity, _, visual)| {
            (
                entity,
                visual
                    .and_then(|visual| visual.pulse_overlay)
                    .map(|overlay| overlay.entity),
            )
        })
        .take(129)
        .collect::<Vec<_>>();
    if buildings.len() + blueprints.len() > 128 {
        return Err("ordinary M5 owner cap exceeded".into());
    }
    let mut sprites = Vec::new();
    for (owner, children) in buildings {
        if world.get::<Sprite>(owner).is_some() {
            sprites.push(sprite(world, owner, owner, "world")?);
        }
        for entity in children {
            if world
                .get::<hw_visual::layer::VisualLayerKind>(entity)
                .is_some()
            {
                sprites.push(sprite(world, entity, owner, "world")?);
            }
        }
    }
    for (owner, pulse) in blueprints {
        sprites.push(sprite(world, owner, owner, "blueprint")?);
        if let Some(pulse) = pulse {
            sprites.push(sprite(world, pulse, owner, "pulse")?);
        }
    }
    if world.resource::<crate::app_contexts::BuildContext>().0 == Some(kind)
        && *world
            .resource::<State<hw_core::game_state::PlayMode>>()
            .get()
            == hw_core::game_state::PlayMode::BuildingPlace
    {
        let ghosts = world
            .query_filtered::<Entity, With<PlacementGhost>>()
            .iter(world)
            .take(3)
            .collect::<Vec<_>>();
        if ghosts.len() > 1 {
            return Err("ordinary M5 ghost inventory differs".into());
        }
        if let Some(&ghost) = ghosts.first() {
            sprites.push(sprite(world, ghost, ghost, "ghost")?);
        }
    }
    if sprites.len() > 256 {
        return Err("ordinary M5 Sprite cap exceeded".into());
    }
    sprites.sort_by_key(|row| row["entity"].as_u64());
    let viewport = world
        .query_filtered::<&Window, With<bevy::window::PrimaryWindow>>()
        .single(world)
        .map_err(|_| "ordinary M5 primary window differs")?
        .physical_size()
        .as_vec2();
    let cards = world
        .query::<(Entity, &BuildingCatalogPreview, &ImageNode)>()
        .iter(world)
        .filter(|(_, preview, _)| preview.0 == kind)
        .map(|(entity, _, node)| (entity, node.clone()))
        .take(3)
        .collect::<Vec<_>>();
    // Missing/hidden UI during ordinary transitions is unready, not evidence.
    // Duplicate cards remain a hard failure. The driver must require one visible
    // image and the requested resident descriptor at its catalog checkpoint.
    if cards.len() > 1 {
        return Err("ordinary M5 catalog inventory differs".into());
    }
    let catalog = cards
        .into_iter()
        .map(|(entity, node)| {
            let rect = super::observations_ui::visible_rect(world, entity, viewport);
            let unclipped = world.get::<ComputedNode>(entity).zip(world.get::<UiGlobalTransform>(entity))
                .map(|(computed, transform)| {
                    let min = transform.translation - computed.size() * 0.5;
                    let max = transform.translation + computed.size() * 0.5;
                    [min.x, min.y, max.x, max.y]
                });
            let fully_visible = rect.zip(unclipped).is_some_and(|(clipped, original)|
                clipped.into_iter().zip(original).all(|(a, b)| (a - b).abs() <= 1.0));
            json!({"entity": entity.to_bits(), "image": format!("{:?}", node.image.id()),
                "color": node.color.to_srgba().to_f32_array(), "fully_visible": fully_visible,
                "whole_image": node.texture_atlas.is_none() && node.rect.is_none() && !node.flip_x && !node.flip_y,
                "unclipped_rect": unclipped, "physical_rect": rect})
        })
        .collect::<Vec<_>>();
    Ok(
        json!({"schema": 1, "requested": identity, "active": active, "pending": pending, "descriptor": descriptor,
        "sprites": sprites, "catalog": catalog, "accepted": false, "performance_evidence": false, "promotion_authority": false}),
    )
}
