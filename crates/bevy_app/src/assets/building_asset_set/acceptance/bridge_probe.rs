//! Bounded observations of ordinary Bridge input/lifecycle; never seeds buildings,
//! rewrites terrain, supplies materials, or claims UI/native/performance acceptance.
use super::BuildingArtSession;
use crate::assets::building_asset_set::snapshot_io::{require_fresh_output, write_observation};
use crate::assets::building_asset_set::{BuildingAssetKind, BuildingAssetPool};
use crate::world::map::WorldMapRef;
use bevy::prelude::*;
use hw_core::constants::MAP_WIDTH;
use hw_core::soul::DamnedSoul;
use hw_jobs::{Blueprint, Building, BuildingType};
use hw_ui::selection::resolve_bridge_crossing;
use hw_visual::Building3dVisual;
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Resource)]
struct Trace {
    path: PathBuf,
    leg: String,
    next: f64,
    samples: Vec<Value>,
}

pub(super) fn configure(app: &mut App, session: &BuildingArtSession) -> Result<(), String> {
    let Ok(leg) = std::env::var("HW_BRIDGE_ACCEPTANCE_PROBE") else {
        return Ok(());
    };
    if session.identity.kind != BuildingAssetKind::Bridge
        || !matches!(
            leg.as_str(),
            "placement"
                | "passage"
                | "adjacent"
                | "construction"
                | "instant-build"
                | "cancel"
                | "save-load"
                | "deconstruct"
                | "non-movable"
                | "gallery"
                | "cleanup"
        )
    {
        return Err("Bridge probe requires a Bridge session and an explicit supported leg".into());
    }
    if std::env::var_os("HW_M2_ACCEPTANCE_PROBE").is_some() {
        return Err("Bridge observations cannot share the M2 seeded fixture".into());
    }
    let path = session.status_path.with_extension("bridge-trace.json");
    require_fresh_output(&path).map_err(|e| format!("use a fresh Bridge trace path: {e}"))?;
    app.insert_resource(Trace {
        path,
        leg,
        next: 0.0,
        samples: Vec::new(),
    })
    .add_systems(Last, observe);
    Ok(())
}

fn catalog_visible(world: &World, entity: Entity) -> bool {
    if world
        .get::<ComputedNode>(entity)
        .is_none_or(|node| node.size().min_element() <= 0.0)
    {
        return false;
    }
    let mut ancestor = Some(entity);
    for _ in 0..128 {
        let Some(current) = ancestor else {
            return true;
        };
        if world
            .get::<Node>(current)
            .is_some_and(|node| node.display == Display::None)
        {
            return false;
        }
        ancestor = world.get::<ChildOf>(current).map(ChildOf::parent);
    }
    false
}

pub(crate) fn snapshot(world: &mut World) -> Result<Value, String> {
    let mut owners = Vec::new();
    let mut query = world.query::<(Entity, &Building, &Transform)>();
    for (entity, building, pose) in query.iter(world) {
        if building.kind == BuildingType::Bridge {
            owners.push(
                json!({"entity": format!("{entity:?}"), "position": pose.translation.to_array(),
                "scale": pose.scale.to_array(), "movable": building.kind.is_player_movable()}),
            );
        }
    }
    let mut blueprints = Vec::new();
    let mut query = world.query::<(Entity, &Blueprint, Option<&Sprite>)>();
    for (entity, bp, sprite) in query.iter(world) {
        if bp.kind == BuildingType::Bridge {
            blueprints.push(
                json!({"entity": format!("{entity:?}"), "footprint": bp.occupied_grids,
                "progress": bp.progress, "image": sprite.map(|s| format!("{:?}", s.image.id()))}),
            );
        }
    }
    let mut roots = Vec::new();
    let mut query = world.query::<(Entity, &Building3dVisual, &Transform, Option<&Children>)>();
    for (entity, visual, pose, children) in query.iter(world) {
        if world
            .get::<Building>(visual.owner)
            .is_some_and(|b| b.kind == BuildingType::Bridge)
        {
            let parts: Vec<_> = children
                .into_iter()
                .flat_map(|c| c.iter())
                .map(|child| {
                    json!({"entity": format!("{child:?}"),
                    "mesh": world.get::<Mesh3d>(child).map(|m| format!("{:?}", m.0.id()))})
                })
                .collect();
            roots.push(
                json!({"entity": format!("{entity:?}"), "owner": format!("{:?}", visual.owner),
                "position": pose.translation.to_array(), "parts": parts}),
            );
        }
    }
    let mut actors = Vec::new();
    let mut query = world.query_filtered::<(Entity, &Transform), With<DamnedSoul>>();
    for (entity, pose) in query.iter(world) {
        actors.push(
            json!({"entity": format!("{entity:?}"), "position": pose.translation.to_array()}),
        );
    }
    if [owners.len(), blueprints.len(), roots.len(), actors.len()]
        .into_iter()
        .any(|n| n > 128)
    {
        return Err("Bridge observation entity cap exceeded".into());
    }
    let mut consumers = Vec::new();
    let mut query = world.query::<(Entity, &Sprite)>();
    for (entity, sprite) in query.iter(world) {
        let blueprint_owner = if world
            .get::<Blueprint>(entity)
            .is_some_and(|bp| bp.kind == BuildingType::Bridge)
        {
            Some(entity)
        } else if world
            .get::<hw_visual::blueprint::BlueprintPulseOverlayChild>(entity)
            .is_some()
        {
            world
                .get::<ChildOf>(entity)
                .map(ChildOf::parent)
                .filter(|owner| {
                    world
                        .get::<Blueprint>(*owner)
                        .is_some_and(|bp| bp.kind == BuildingType::Bridge)
                })
        } else {
            None
        };
        let ghost = world
            .get::<crate::systems::visual::placement_ghost::PlacementGhost>(entity)
            .is_some()
            && world
                .get_resource::<crate::app_contexts::BuildContext>()
                .is_some_and(|context| context.0 == Some(BuildingType::Bridge));
        if ghost || blueprint_owner.is_some() {
            consumers.push(json!({"entity": format!("{entity:?}"),
                "role": if ghost { "ghost" } else if blueprint_owner == Some(entity) { "blueprint" } else { "pulse" },
                "owner": blueprint_owner.map(|owner| format!("{owner:?}")),
                "image": format!("{:?}", sprite.image.id()),
                "size": sprite.custom_size.map(|size| size.to_array()),
                "visible": world.get::<ViewVisibility>(entity).is_some_and(|visibility| visibility.get())}));
        }
    }
    let mut query = world.query::<(
        Entity,
        &hw_ui::components::BuildingCatalogPreview,
        &ImageNode,
    )>();
    for (entity, preview, node) in query.iter(world) {
        if preview.0 == BuildingType::Bridge {
            consumers.push(json!({"entity": format!("{entity:?}"), "role": "catalog",
                "image": format!("{:?}", node.image.id()),
                "visible": catalog_visible(world, entity)}));
        }
    }
    if consumers.len() > 128 {
        return Err("Bridge consumer observation cap exceeded".into());
    }
    let map = world.resource::<hw_world::WorldMap>();
    let read = WorldMapRef(map);
    let candidates: Vec<_> = (0..MAP_WIDTH - 1)
        .filter_map(|x| {
            resolve_bridge_crossing(&read, (x, 0)).ok().map(|c| {
                json!({"anchor": c.anchor,
            "footprint": c.occupied_grids, "banks": c.banks})
            })
        })
        .collect();
    let mut bridged: Vec<_> = map.bridged_tiles.iter().copied().collect();
    bridged.sort_unstable();
    let bridge_cells: Vec<_> = bridged
        .iter()
        .map(|&(x, y)| {
            json!({"grid": [x,y],
        "owner": map.building_entity((x,y)).map(|e| format!("{e:?}")),
        "river": map.is_river_tile(x,y), "walkable": map.is_walkable(x,y)})
        })
        .collect();
    let river_cells: Vec<_> = (0..MAP_WIDTH)
        .flat_map(|x| (0..hw_core::constants::MAP_HEIGHT).map(move |y| (x, y)))
        .filter(|&(x, y)| map.is_river_tile(x, y))
        .map(|(x, y)| json!({"grid": [x,y], "walkable": map.is_walkable(x,y)}))
        .collect();
    let pool = world.resource::<BuildingAssetPool>();
    let expected = pool.descriptor(BuildingAssetKind::Bridge).map(|set| {
        json!({
        "body": set.mesh("body").map(|handle| format!("{:?}", handle.id())),
        "world": set.image("world_preview").map(|handle| format!("{:?}", handle.id())),
        "catalog": set.image("catalog").map(|handle| format!("{:?}", handle.id()))})
    });

    Ok(
        json!({"real_seconds": world.resource::<Time<Real>>().elapsed_secs_f64(),
        "virtual_seconds": world.resource::<Time<Virtual>>().elapsed_secs_f64(),
        "paused": world.resource::<Time<Virtual>>().is_paused(),
        "identity": pool.active(BuildingAssetKind::Bridge),
        "owners": owners, "blueprints": blueprints, "roots": roots, "actors": actors,
        "legal_crossings": candidates, "bridge_cells": bridge_cells,
        "river_cells": river_cells, "consumers": consumers, "expected": expected}),
    )
}

fn observe(world: &mut World) {
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    if now < world.resource::<Trace>().next {
        return;
    }
    let sample = if world.resource::<Trace>().samples.len() >= 600 {
        Err("Bridge observation cap reached; use separate bounded legs".into())
    } else {
        snapshot(world)
    };
    let failure = sample.as_ref().err().cloned();
    let path = {
        let mut trace = world.resource_mut::<Trace>();
        trace.next = now + 0.5;
        if let Ok(sample) = sample {
            trace.samples.push(sample);
        }
        trace.path.clone()
    };
    let session = world.resource::<BuildingArtSession>();
    let trace = world.resource::<Trace>();
    let record = json!({"schema_version": 1, "scope": "bridge-normal-world-observations-only",
        "leg": trace.leg, "identity": session.identity, "nonce": session.nonce,
        "process_id": std::process::id(), "accepted": false, "performance_evidence": false,
        "promotion_authority": false,
        "fixture_seeded_completion": false, "failure": failure, "samples": trace.samples});
    let result = serde_json::to_vec(&record)
        .map_err(|e| e.to_string())
        .and_then(|data| write_observation(&path, &data).map_err(|e| e.to_string()));
    if failure.is_some() || result.is_err() {
        error!(
            "Bridge observation failed: {:?} {:?}",
            failure,
            result.err()
        );
        world.write_message(AppExit::error());
    }
}
