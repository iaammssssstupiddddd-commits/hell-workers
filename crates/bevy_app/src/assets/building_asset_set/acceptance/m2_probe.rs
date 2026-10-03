//! Opt-in observations for coordinator-driven M2 lifecycle acceptance.
//! Does not simulate input, change gameplay, approve art, or measure performance.

use super::BuildingArtSession;
use crate::assets::building_asset_set::{
    BuildingAssetAuthority, BuildingAssetKind, BuildingAssetPool,
};
use crate::systems::visual::building_presentation::EquipmentRoot;
use crate::systems::visual::placement_ghost::{PlacementGhost, PlacementPartnerGhost};
use bevy::prelude::*;
use hw_core::relationships::StoredIn;
use hw_jobs::{Blueprint, Building, BuildingType, MovePlantTask};
use hw_ui::components::BuildingCatalogPreview;
use hw_visual::{Building3dVisual, StructuralPresentationState};
use serde_json::{Value, json};
use std::path::PathBuf;

const SAMPLE_CAP: usize = 1800;
const ENTITY_CAP: usize = 128;
const NUMERIC_SHA256: &str = "1b913ea0784bf6815a6c46ed0396dafc6ebcdcfc107370bc569aa875775d949d";

#[derive(Resource)]
struct Trace {
    path: PathBuf,
    next_sample: f64,
    next_write: f64,
    samples: Vec<Value>,
}

pub(super) fn configure(app: &mut App, session: &BuildingArtSession) -> Result<(), String> {
    let Ok(value) = std::env::var("HW_M2_ACCEPTANCE_PROBE") else {
        return Ok(());
    };
    if value != "1"
        || session.mode != "feedback"
        || session.identity.authority != BuildingAssetAuthority::ArtPreview
        || !matches!(
            session.identity.kind,
            BuildingAssetKind::Tank | BuildingAssetKind::MudMixer
        )
    {
        return Err(
            "M2 probe requires an explicit Tank/MudMixer feedback ArtPreview session".into(),
        );
    }
    let path = session.status_path.with_extension("m2-trace.json");
    if path.exists() {
        return Err("M2 trace path already exists; use a fresh acceptance session".into());
    }
    super::m2_fixture::configure(app, session.identity.kind)?;
    app.insert_resource(Trace {
        path,
        next_sample: 0.0,
        next_write: 0.0,
        samples: Vec::new(),
    })
    .add_systems(Last, observe);
    Ok(())
}

fn pose(transform: &Transform) -> Value {
    json!({"translation": transform.translation.to_array(),
           "rotation": transform.rotation.to_array(), "scale": transform.scale.to_array()})
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

fn push_capped<T>(values: &mut Vec<T>, value: T) -> Result<(), String> {
    if values.len() == ENTITY_CAP {
        return Err("M2 entity observation cap exceeded".into());
    }
    values.push(value);
    Ok(())
}

fn snapshot(world: &mut World) -> Result<Value, String> {
    let fixture_owner = world
        .get_resource::<super::m2_fixture::Fixture>()
        .and_then(|fixture| fixture.owner);
    let session = world.resource::<BuildingArtSession>();
    let kind = match session.identity.kind {
        BuildingAssetKind::Tank => BuildingType::Tank,
        BuildingAssetKind::MudMixer => BuildingType::MudMixer,
        _ => return Err("unexpected M2 probe kind".into()),
    };
    let pool = world.resource::<BuildingAssetPool>();
    let identity = pool.active(session.identity.kind).cloned();
    let descriptor = pool.descriptor(session.identity.kind);
    if descriptor.is_some_and(|set| set.manifest.geometry_contract_sha256 != NUMERIC_SHA256) {
        return Err("M2 probe descriptor does not bind the exact numeric candidate".into());
    }
    let expected_images = descriptor.map(|set| {
        json!({"world": format!("{:?}", set.image(&set.manifest.world_preview.image_role)),
               "catalog": format!("{:?}", set.image(&set.manifest.catalog_preview.image_role))})
    });
    let mut owners = Vec::new();
    let mut query = world.query::<(Entity, &Building, &Transform)>();
    for (entity, building, transform) in query.iter(world) {
        if building.kind == kind {
            push_capped(&mut owners, (entity, pose(transform)))?;
        }
    }
    let mut roots = Vec::new();
    let mut query = world.query_filtered::<(
        Entity,
        &Building3dVisual,
        &Transform,
        Option<&StructuralPresentationState>,
    ), With<EquipmentRoot>>();
    for (entity, visual, transform, state) in query.iter(world) {
        if owners.iter().any(|(owner, _)| *owner == visual.owner)
            || world.get_entity(visual.owner).is_err()
        {
            let parts: Vec<_> = world
                .get::<Children>(entity)
                .into_iter()
                .flat_map(|c| c.iter())
                .take(17)
                .map(|part| {
                    json!({
                        "entity": format!("{part:?}"),
                        "name": world.get::<Name>(part).map(Name::as_str),
                        "pose": world.get::<Transform>(part).map(pose),
                        "visibility": format!("{:?}", world.get::<Visibility>(part)),
                        "mesh": format!("{:?}", world.get::<Mesh3d>(part)),
                    })
                })
                .collect();
            if parts.len() > 16 {
                return Err("M2 part observation cap exceeded".into());
            }
            push_capped(
                &mut roots,
                json!({"entity": format!("{entity:?}"),
                "owner": format!("{:?}", visual.owner), "owner_exists": world.get_entity(visual.owner).is_ok(),
                "pose": pose(transform), "state": format!("{state:?}"), "parts": parts}),
            )?;
        }
    }
    let mut items = Vec::new();
    let mut query = world.query::<(Entity, &StoredIn, Option<&Transform>)>();
    for (entity, stored, transform) in query.iter(world) {
        if owners.iter().any(|(owner, _)| *owner == stored.0) || fixture_owner == Some(stored.0) {
            push_capped(
                &mut items,
                json!({"entity": format!("{entity:?}"), "owner": format!("{:?}", stored.0),
                              "pose": transform.map(pose)}),
            )?;
        }
    }
    let mut consumers = Vec::new();
    let mut query = world.query::<(Entity, &Sprite, Option<&Transform>)>();
    for (entity, sprite, transform) in query.iter(world) {
        let role = if world.get::<PlacementGhost>(entity).is_some() {
            Some("ghost")
        } else if world.get::<PlacementPartnerGhost>(entity).is_some() {
            Some("companion")
        } else if world
            .get::<Blueprint>(entity)
            .is_some_and(|b| b.kind == kind)
        {
            Some("blueprint")
        } else if world
            .get::<hw_visual::blueprint::BlueprintPulseOverlayChild>(entity)
            .is_some()
        {
            Some("pulse")
        } else if world.get::<MovePlantTask>(entity).is_some_and(|task| {
            world
                .get::<Building>(task.building)
                .is_some_and(|building| building.kind == kind)
        }) {
            Some("destination")
        } else {
            None
        };
        if let Some(role) = role {
            let blueprint_owner = match role {
                "blueprint" => Some(entity),
                "pulse" => world.get::<ChildOf>(entity).map(ChildOf::parent),
                _ => None,
            };
            let blueprint = blueprint_owner.and_then(|owner| world.get::<Blueprint>(owner));
            push_capped(
                &mut consumers,
                json!({"entity": format!("{entity:?}"), "role": role,
                "blueprint_owner": blueprint_owner.map(|owner| format!("{owner:?}")),
                "blueprint_kind": blueprint.map(|blueprint| format!("{:?}", blueprint.kind)),
                "blueprint_progress": blueprint.map(|blueprint| blueprint.progress),
                "blueprint_state": blueprint_owner.and_then(|owner| world.get::<hw_visual::blueprint::BlueprintVisual>(owner)).map(|visual| format!("{:?}", visual.state)),
                "visible": world.get::<ViewVisibility>(entity).is_some_and(|visibility| visibility.get()),
                "image": format!("{:?}", Some(&sprite.image)), "pose": transform.map(pose),
                "size": sprite.custom_size.map(|v| v.to_array()),
                "anchor": format!("{:?}", world.get::<bevy::sprite::Anchor>(entity)),
                "color": format!("{:?}", sprite.color)}),
            )?;
        }
    }
    let mut query = world.query::<(Entity, &BuildingCatalogPreview, &ImageNode)>();
    for (entity, preview, node) in query.iter(world) {
        if preview.0 == kind {
            push_capped(
                &mut consumers,
                json!({"entity": format!("{entity:?}"), "role": "catalog",
                "visible": catalog_visible(world, entity),
                "image": format!("{:?}", Some(&node.image))}),
            )?;
        }
    }
    let mut camera_query = world.query_filtered::<&Transform, With<hw_ui::camera::MainCamera>>();
    let camera_scale = camera_query.iter(world).next().map(|camera| camera.scale.x);
    let destination = world
        .get_resource::<super::m2_lifecycle::Lifecycle>()
        .and_then(|driver| driver.destination);
    let destination_owner = destination.and_then(|grid| {
        world
            .get_resource::<hw_world::WorldMap>()
            .and_then(|map| map.building_entity(grid))
    });
    let mut companion_query = world.query_filtered::<(Entity, &hw_logistics::BelongsTo, &Transform), With<crate::systems::logistics::BucketStorage>>();
    let companions: Vec<_> = companion_query.iter(world).filter(|(_, parent, _)| Some(parent.0) == fixture_owner)
        .map(|(entity, _, transform)| json!({"entity": format!("{entity:?}"), "pose": pose(transform)})).take(129).collect();
    if companions.len() > 128 {
        return Err("M2 companion cap exceeded".into());
    }
    let feedback = world
        .get_resource::<hw_ui::selection::PlacementFeedbackState>()
        .and_then(|feedback| feedback.visible(world.resource::<Time<Real>>().elapsed()));
    Ok(
        json!({"real_seconds": world.resource::<Time<Real>>().elapsed_secs_f64(),
        "virtual_seconds": world.resource::<Time<Virtual>>().elapsed_secs_f64(),
        "paused": world.resource::<Time<Virtual>>().is_paused(),
        "identity": identity, "expected_images": expected_images,
        "fixture_owner": fixture_owner.map(|owner| format!("{owner:?}")),
        "fixture_owner_exists": fixture_owner.is_some_and(|owner| world.get_entity(owner).is_ok()),
        "world_epoch": world.get_resource::<hw_core::WorldEpoch>().map(|epoch| epoch.get()),
        "camera_scale": camera_scale,
        "play_mode": world.get_resource::<State<hw_core::game_state::PlayMode>>().map(|mode| format!("{:?}", mode.get())),
        "placement_rejection": feedback.map(|feedback| json!({"header": feedback.header(), "body": feedback.body()})),
        "destination_owner": destination_owner.map(|entity| format!("{entity:?}")),
        "destination_anchor": destination,
        "companions": companions,
        "lifecycle": world.get_resource::<super::m2_lifecycle::Lifecycle>().map(|lifecycle| json!({"leg": lifecycle.leg, "events": lifecycle.events,
            "placement_blueprint": lifecycle.placement_blueprint.map(|owner| format!("{owner:?}")),
            "pulse_observation_complete": lifecycle.pulse_observation_complete})),
        "owners": owners.into_iter().map(|(entity, pose)| json!({"entity": format!("{entity:?}"),
            "pose": pose})).collect::<Vec<_>>(),
        "roots": roots, "items": items, "consumers": consumers}),
    )
}

fn placement_pulse_observed(sample: &Value, owner: Entity, kind: &str) -> bool {
    let owner = format!("{owner:?}");
    let Some(image) = sample["expected_images"]["world"]
        .as_str()
        .filter(|image| *image != "None")
    else {
        return false;
    };
    let Some(consumers) = sample["consumers"].as_array() else {
        return false;
    };
    sample["paused"] == false
        && sample["destination_owner"].as_str() == Some(owner.as_str())
        && ["blueprint", "pulse"].into_iter().all(|role| {
            consumers.iter().any(|consumer| {
                consumer["role"] == role
                    && consumer["visible"] == true
                    && consumer["image"] == image
                    && consumer["blueprint_owner"] == owner
                    && consumer["blueprint_kind"] == kind
                    && consumer["blueprint_state"] == "Building"
                    && consumer["blueprint_progress"]
                        .as_f64()
                        .is_some_and(|progress| progress > 0.0 && progress < 1.0)
                    && consumer["entity"].as_str().is_some_and(|entity| {
                        if role == "blueprint" {
                            entity == owner
                        } else {
                            entity != owner
                        }
                    })
            })
        })
}

fn observe(world: &mut World) {
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    if now < world.resource::<Trace>().next_sample {
        return;
    }
    let result = if world.resource::<Trace>().samples.len() >= SAMPLE_CAP {
        Err("M2 sample cap reached; use separate bounded lifecycle legs".to_string())
    } else {
        snapshot(world)
    };
    let failure = result.as_ref().err().cloned();
    let session = world.resource::<BuildingArtSession>();
    let identity = json!(session.identity);
    let kind = format!("{:?}", session.identity.kind);
    if let Some(mut driver) = world.get_resource_mut::<super::m2_lifecycle::Lifecycle>() {
        if driver.leg == "placement" && !driver.pulse_observation_complete {
            let observed = result.as_ref().is_ok_and(|sample| {
                sample["identity"] == identity
                    && driver
                        .placement_blueprint
                        .is_some_and(|owner| placement_pulse_observed(sample, owner, &kind))
            });
            if observed {
                driver.pulse_observed_since.get_or_insert(now);
            } else {
                driver.pulse_observed_since = None;
            }
        }
    }
    let path = {
        let mut trace = world.resource_mut::<Trace>();
        trace.next_sample = now + 0.1;
        if let Ok(sample) = result {
            trace.samples.push(sample);
        }
        if now < trace.next_write && failure.is_none() {
            return;
        }
        trace.next_write = now + 1.0;
        trace.path.clone()
    };
    let session = world.resource::<BuildingArtSession>();
    let record = json!({"schema_version": 1, "scope": "m2-feedback-observations-only",
        "leg": world.resource::<super::m2_lifecycle::Lifecycle>().leg,
        "process_id": std::process::id(),
        "identity": session.identity, "nonce": session.nonce, "failure": failure,
        "accepted": false, "performance_evidence": false,
        "samples": world.resource::<Trace>().samples});
    let write = serde_json::to_vec(&record)
        .map_err(|e| e.to_string())
        .and_then(|data| {
            let temporary = path.with_extension("tmp");
            std::fs::write(&temporary, data)
                .and_then(|()| std::fs::rename(temporary, &path))
                .map_err(|e| e.to_string())
        });
    if failure.is_some() || write.is_err() {
        error!("M2 observation failed: {:?} {:?}", failure, write.err());
        world.write_message(AppExit::error());
    } else if let Some(mut driver) = world.get_resource_mut::<super::m2_lifecycle::Lifecycle>() {
        // ACK only after the qualifying sample interval is durably in the trace.
        if driver.leg == "placement"
            && driver
                .pulse_observed_since
                .is_some_and(|since| now - since >= 3.0)
        {
            driver.pulse_observation_complete = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::building_asset_set::BuildingAssetSetIdentity;

    #[test]
    fn pulse_ack_requires_both_visible_candidate_consumers_on_the_placement_owner() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();
        let name = format!("{owner:?}");
        let consumers = ["blueprint", "pulse"].map(|role| {
            json!({"role": role,
                "entity": if role == "blueprint" { name.as_str() } else { "overlay" },
                "blueprint_owner": name, "blueprint_kind": "Tank", "blueprint_state": "Building",
                "blueprint_progress": 0.25, "visible": true, "image": "Some(candidate)"})
        });
        let row = json!({"paused": false, "destination_owner": name, "expected_images": {"world": "Some(candidate)"},
            "consumers": consumers});
        assert!(placement_pulse_observed(&row, owner, "Tank"));
        for index in 0..2 {
            for (key, value) in [
                ("visible", json!(false)),
                ("image", json!("Some(fallback)")),
                ("blueprint_owner", json!("other")),
                ("blueprint_kind", json!("MudMixer")),
                ("blueprint_state", json!("ReadyToBuild")),
                ("blueprint_progress", json!(1.0)),
            ] {
                let mut changed = row.clone();
                changed["consumers"][index][key] = value;
                assert!(!placement_pulse_observed(&changed, owner, "Tank"));
            }
        }
    }

    fn world() -> World {
        let mut world = World::new();
        world.insert_resource(BuildingArtSession {
            mode: "feedback".into(),
            identity: BuildingAssetSetIdentity {
                kind: BuildingAssetKind::Tank,
                generation: 1,
                authority: BuildingAssetAuthority::ArtPreview,
                manifest_sha256: "a".repeat(64),
            },
            locator: "manifests/building-tank-v1.buildingset".into(),
            status_path: PathBuf::from("/unused/m2-probe-test.json"),
            nonce: "a".repeat(32),
        });
        world.init_resource::<BuildingAssetPool>();
        world.init_resource::<Time<Real>>();
        world.init_resource::<Time<Virtual>>();
        world
    }

    #[test]
    fn observation_does_not_mutate_owner_and_exposes_orphan_roots() {
        let mut world = world();
        let transform = Transform::from_xyz(12.0, 24.0, 9.0);
        let owner = world
            .spawn((
                Building {
                    kind: BuildingType::Tank,
                    is_provisional: false,
                },
                transform,
            ))
            .id();
        let root = world
            .spawn((
                EquipmentRoot::default(),
                Building3dVisual { owner },
                Transform::from_xyz(12.0, 0.0, -24.0),
            ))
            .id();
        let before = snapshot(&mut world).unwrap();
        assert_eq!(before["owners"].as_array().unwrap().len(), 1);
        assert_eq!(before["roots"][0]["owner_exists"], true);
        assert_eq!(world.get::<Transform>(owner), Some(&transform));
        world.despawn(owner);
        let orphan = snapshot(&mut world).unwrap();
        assert_eq!(orphan["roots"][0]["owner_exists"], false);
        world.despawn(root);
        assert!(
            snapshot(&mut world).unwrap()["roots"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn observation_refuses_unbounded_owner_inventory() {
        let mut world = world();
        for _ in 0..=ENTITY_CAP {
            world.spawn((
                Building {
                    kind: BuildingType::Tank,
                    is_provisional: false,
                },
                Transform::default(),
            ));
        }
        assert!(snapshot(&mut world).unwrap_err().contains("cap exceeded"));
    }
}
