//! Profiling-only, bounded observations of the ordinary world. This does not
//! create owners, drive input, grant authority, or claim performance evidence.
use bevy::{ecs::message::MessageCursor, prelude::*};
use hw_core::{
    WorldEpoch,
    events::{DreamTransferredVisualMessage, TaskCompletedVisualMessage},
};
use hw_jobs::{
    Blueprint, Building, BuildingCompletedEvent, BuildingCompletedVisualMessage, BuildingType,
    DeconstructionCommitOutcome, MovePlantTask,
};
use hw_visual::{Building3dVisual, StructuralPresentationState, TopDownStructuralMaterial};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Component, PathBuf};

use super::{
    BuildingAssetAuthority, BuildingAssetKind, BuildingAssetPool, BuildingAssetSetIdentity,
};
use crate::systems::save::SaveLoadOutcome;

#[derive(Resource, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    nonce: String,
    output: PathBuf,
    admitted_identities: Vec<BuildingAssetSetIdentity>,
}

#[derive(Resource, Default)]
struct Observations {
    next: f64,
    samples: Vec<Value>,
    events: Vec<Value>,
    saves: MessageCursor<SaveLoadOutcome>,
    deconstruction: MessageCursor<DeconstructionCommitOutcome>,
    completed: MessageCursor<BuildingCompletedVisualMessage>,
    tasks: MessageCursor<TaskCompletedVisualMessage>,
    dreams: MessageCursor<DreamTransferredVisualMessage>,
    failure: Option<String>,
    tracked_owners: std::collections::HashSet<Entity>,
    written_events: usize,
}

pub(super) fn configure(app: &mut App) -> Result<(), String> {
    let Ok(raw) = std::env::var("HW_BUILDING_PRODUCTION_OBSERVATIONS") else {
        return Ok(());
    };
    let session: Session = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if session.nonce.len() != 32
        || !session
            .nonce
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        || !session.output.is_absolute()
        || session
            .output
            .components()
            .any(|c| matches!(c, Component::ParentDir))
        || session.output.exists()
        || session.admitted_identities.len() > 18
        || session
            .admitted_identities
            .iter()
            .any(|identity| identity.authority != BuildingAssetAuthority::ReleaseApproved)
    {
        return Err("production observations require a fresh absolute path and nonce".into());
    }
    if std::env::var_os("HW_BUILDING_ART_SESSION").is_some()
        || std::env::var_os("HW_BUILDING_M6_MODE").is_some()
    {
        return Err(
            "normal-world production observations cannot share draft/static fixtures".into(),
        );
    }
    app.insert_resource(session)
        .init_resource::<Observations>()
        .add_observer(building_completed)
        .add_systems(Last, observe);
    Ok(())
}

fn building_completed(
    event: On<BuildingCompletedEvent>,
    time: Res<Time<Real>>,
    mut observations: ResMut<Observations>,
) {
    let event = event.event();
    if !target(event.kind) {
        return;
    }
    if observations.events.len() >= 4096 {
        observations.failure = Some("production event cap exceeded".into());
        return;
    }
    let index = observations.samples.len();
    observations.events.push(
        json!({"sample_index": index, "real_seconds": time.elapsed_secs_f64(),
        "producer": "hw_jobs::publish_building_completed", "operation": "BuildingCompleted",
        "blueprint_owner": format!("{:?}", event.blueprint_entity),
        "owner": format!("{:?}", event.building_entity), "kind": format!("{:?}", event.kind),
        "footprint": event.occupied_grids}),
    );
}

// These are observations of state differences, never inferred domain commits.
// Exact before/after rows remain available to the offline verifier at both indices.
fn state_transitions(before: &Value, after: &Value, index: usize) -> Result<Vec<Value>, String> {
    let mut events = Vec::new();
    for (category, id) in [
        ("owners", "entity"),
        ("blueprints", "entity"),
        ("roots", "entity"),
        ("items", "entity"),
        ("move_tasks", "entity"),
        ("generations", "kind"),
        ("owner_layers", "entity"),
    ] {
        let indexed =
            |sample: &Value| -> Result<std::collections::BTreeMap<String, Value>, String> {
                let mut rows = std::collections::BTreeMap::new();
                for row in sample[category]
                    .as_array()
                    .ok_or("missing state transition category")?
                {
                    let key = row[id]
                        .as_str()
                        .ok_or("missing state transition identity")?;
                    if rows.insert(key.to_owned(), row.clone()).is_some() {
                        return Err("duplicate state transition identity".into());
                    }
                }
                Ok(rows)
            };
        let old = indexed(before)?;
        let new = indexed(after)?;
        let keys: std::collections::BTreeSet<_> = old.keys().chain(new.keys()).collect();
        for key in keys {
            if old.get(key) != new.get(key) {
                events.push(
                    json!({"producer": "production_snapshot_delta", "operation": "StateTransition",
                    "sample_index": index, "before_sample_index": index - 1,
                    "real_seconds": after["real_seconds"], "category": category, "entity": key,
                    "before": old.get(key), "after": new.get(key)}),
                );
                if events.len() >= 4096 {
                    return Err("production transition cap exceeded".into());
                }
            }
        }
    }
    Ok(events)
}

fn target(kind: BuildingType) -> bool {
    matches!(
        kind,
        BuildingType::Tank
            | BuildingType::MudMixer
            | BuildingType::RestArea
            | BuildingType::SoulSpa
            | BuildingType::Bridge
            | BuildingType::Door
            | BuildingType::WheelbarrowParking
            | BuildingType::SandPile
            | BuildingType::BonePile
            | BuildingType::OutdoorLamp
    )
}

fn snapshot(world: &mut World) -> Result<Value, String> {
    let mut query = world.query::<(Entity, &Building, &Transform)>();
    let mut current: Vec<Entity> = query
        .iter(world)
        .filter(|(_, building, _)| target(building.kind))
        .take(513)
        .map(|(entity, _, _)| entity)
        .collect();
    let owners: Vec<_> = query
        .iter(world)
        .filter(|(_, building, _)| target(building.kind))
        .map(|(entity, building, pose)| {
            json!({"entity": format!("{entity:?}"),
            "kind": format!("{:?}", building.kind), "position": pose.translation.to_array(),
            "rotation": pose.rotation.to_array(), "scale": pose.scale.to_array(),
            "movable": building.kind.is_player_movable(), "provisional": building.is_provisional,
            "refining": world.get::<hw_core::visual_mirror::MudMixerVisualState>(entity).map(|state| state.is_active)})
        })
        .take(513)
        .collect();
    let mut query = world.query::<(Entity, &Blueprint)>();
    current.extend(
        query
            .iter(world)
            .filter(|(_, blueprint)| target(blueprint.kind))
            .take(513)
            .map(|(entity, _)| entity),
    );
    let blueprints: Vec<_> = query
        .iter(world)
        .filter(|(_, bp)| target(bp.kind))
        .map(|(entity, bp)| {
            json!({"entity": format!("{entity:?}"), "kind": format!("{:?}", bp.kind),
            "footprint": bp.occupied_grids, "progress": bp.progress,
            "delivered_materials": bp.delivered_materials.iter().map(|(kind, count)| (format!("{kind:?}"), *count)).collect::<std::collections::BTreeMap<_, _>>(),
            "flexible_material_requirement": bp.flexible_material_requirement.as_ref().map(|r| json!({"required_total": r.required_total, "delivered_total": r.delivered_total}))})
        })
        .take(513)
        .collect();
    let mut query = world.query::<(Entity, &Building3dVisual, Option<&Children>)>();
    let roots: Vec<_> = query.iter(world).filter(|(_, root, _)| {
        world.get::<Building>(root.owner).is_none_or(|building| target(building.kind))
    }).map(|(entity, root, children)| {
        let parts: Vec<_> = std::iter::once(entity).chain(children.into_iter().flat_map(|c| c.iter()))
            .filter_map(|part| world.get::<Mesh3d>(part).map(|mesh| json!({
                "entity": format!("{part:?}"), "mesh": format!("{:?}", mesh.id()),
                "position": world.get::<Transform>(part).map(|t| t.translation.to_array()),
                "rotation": world.get::<Transform>(part).map(|t| t.rotation.to_array()),
                "name": world.get::<Name>(part).map(Name::as_str),
                "visible": world.get::<ViewVisibility>(part).map(|visibility| visibility.get())}))).take(17).collect();
        json!({"entity": format!("{entity:?}"), "owner": format!("{:?}", root.owner),
            "owner_exists": world.get_entity(root.owner).is_ok(),
            "state": world.get::<StructuralPresentationState>(entity).map(|state| format!("{state:?}")), "parts": parts})
    }).take(513).collect();
    if owners.len() > 512 || blueprints.len() > 512 || roots.len() > 512 {
        return Err("production observation entity cap exceeded".into());
    }
    let tracked = {
        let mut observations = world.resource_mut::<Observations>();
        observations.tracked_owners.extend(current);
        if observations.tracked_owners.len() > 2048 {
            return Err("production tracked-owner cap exceeded".into());
        }
        let mut tracked: Vec<_> = observations.tracked_owners.iter().copied().collect();
        tracked.sort_unstable();
        tracked
    };
    let map = world.resource::<hw_world::WorldMap>();
    let owner_layers: Vec<_> = tracked
        .into_iter()
        .map(|owner| {
            let layers = map.snapshot_owner(owner);
            json!({"entity": format!("{owner:?}"), "exists": world.get_entity(owner).is_ok(),
            "building_grids": layers.building_grids, "floor_grids": layers.floor_grids,
            "door_grids": layers.door_grids, "bridge_grids": layers.bridge_grids,
            "stockpile_grids": layers.stockpile_grids})
        })
        .collect();
    if roots.iter().any(|root| {
        root["parts"]
            .as_array()
            .is_some_and(|parts| parts.len() > 16)
    }) {
        return Err("production observation part cap exceeded".into());
    }
    let mut query = world.query::<(Entity, &hw_core::relationships::StoredIn, Option<&Sprite>)>();
    let items: Vec<_> = query
        .iter(world)
        .take(4097)
        .map(|(entity, stored, sprite)| {
            json!({"entity": format!("{entity:?}"), "owner": format!("{:?}", stored.0),
            "image": sprite.map(|sprite| format!("{:?}", sprite.image.id()))})
        })
        .collect();
    let mut query = world.query::<(Entity, &MovePlantTask)>();
    let move_tasks: Vec<_> = query
        .iter(world)
        .take(513)
        .map(|(entity, task)| {
            json!({"entity": format!("{entity:?}"), "owner": format!("{:?}", task.building),
            "destination_grid": task.destination_grid, "companion_anchor": task.companion_anchor})
        })
        .collect();
    if items.len() > 4096 || move_tasks.len() > 512 {
        return Err("production item/task observation cap exceeded".into());
    }
    let mut query = world.query_filtered::<&Window, With<bevy::window::PrimaryWindow>>();
    let dpi = query.iter(world).next().map(|window| window.scale_factor());
    let cursor_position = query
        .iter(world)
        .next()
        .and_then(|window| window.cursor_position())
        .map(|point| point.to_array());
    let mut query = world.query_filtered::<&Transform, With<hw_ui::camera::MainCamera>>();
    let camera_scale = query.iter(world).next().map(|pose| pose.scale.x);
    let bridge = super::acceptance::bridge_probe::snapshot(world)?;
    let mut actors =
        world.query_filtered::<(Entity, &Transform), With<hw_core::soul::DamnedSoul>>();
    let actors: Vec<_> = actors
        .iter(world)
        .take(129)
        .map(|(entity, pose)| {
            json!({
        "entity": format!("{entity:?}"), "position": pose.translation.to_array(),
        "grid": hw_world::WorldMap::world_to_grid(pose.translation.truncate())})
        })
        .collect();
    if actors.len() > 128 {
        return Err("production actor observation cap exceeded".into());
    }
    let meshes = world.resource::<Assets<Mesh>>();
    let images = world.resource::<Assets<Image>>();
    let materials = world.resource::<Assets<TopDownStructuralMaterial>>();
    if meshes.len() > 4096 || images.len() > 4096 || materials.len() > 4096 {
        return Err("production resident asset observation cap exceeded".into());
    }
    let pool = world.resource::<BuildingAssetPool>();
    if pool.active_identities().iter().any(|identity| {
        !world
            .resource::<Session>()
            .admitted_identities
            .contains(identity)
    }) {
        return Err("production observation identity is not in the admitted inventory".into());
    }
    let (active, pending) = pool.generation_counts();
    let generations: Vec<_> = [BuildingAssetKind::Tank, BuildingAssetKind::MudMixer,
        BuildingAssetKind::RestArea, BuildingAssetKind::SoulSpa, BuildingAssetKind::Bridge,
        BuildingAssetKind::WheelbarrowParking, BuildingAssetKind::SandPile,
        BuildingAssetKind::BonePile, BuildingAssetKind::OutdoorLamp].into_iter().map(|kind| {
            json!({"kind": kind, "active": pool.active(kind), "pending": pool.pending(kind),
                "failure": pool.failure(kind).map(|failure| json!({"identity": failure.identity, "reason": failure.reason})),
                "production_state": pool.descriptor(kind).and_then(|set| set.manifest.production_state.as_ref())})
        }).collect();
    Ok(
        json!({"real_seconds": world.resource::<Time<Real>>().elapsed_secs_f64(),
        "virtual_seconds": world.resource::<Time<Virtual>>().elapsed_secs_f64(),
        "paused": world.resource::<Time<Virtual>>().is_paused(),
        "world_epoch": world.get_resource::<WorldEpoch>().map(|epoch| epoch.get()),
        "owners": owners, "blueprints": blueprints, "roots": roots, "bridge": bridge,
        "owner_layers": owner_layers, "actors": actors,
        "items": items, "move_tasks": move_tasks, "generations": generations,
        "quality": world.get_resource::<hw_core::quality::QualitySettings>().map(|settings| format!("{:?}", settings.rtt)),
        "dpi": dpi, "camera_scale": camera_scale, "cursor_position": cursor_position,
        "identities": pool.active_identities(),
        "pool_active": active, "pool_pending": pending,
        "pool_owned_handle_count": pool.owned_handle_count(),
        "resident_mesh_handles": meshes.iter().map(|(id, _)| format!("{id:?}")).collect::<Vec<_>>(),
        "mesh_count": meshes.len(), "image_count": images.len(), "material_count": materials.len(),
        "image_cpu_bytes": images.iter().map(|(_, image)| image.data.as_ref().map_or(0, Vec::len)).sum::<usize>(),
        "material_shallow_bytes": materials.len() * std::mem::size_of::<TopDownStructuralMaterial>(),
        "mesh_cpu_bytes": null, "gpu_measured_bytes": null, "gpu_estimated_bytes": null,
        "native_peak_live_bytes": null, "rss_bytes": null,
        "application_handle_count": null}),
    )
}

fn observe(world: &mut World) {
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    world.resource_scope(|world, mut observations: Mut<Observations>| {
        let index = observations.samples.len();
        if let Some(messages) = world.get_resource::<Messages<SaveLoadOutcome>>() {
            let events: Vec<_> = observations.saves.read(messages).take(4097).cloned().collect();
            for event in events {
                observations.events.push(json!({"sample_index": index, "real_seconds": now,
                    "producer": "SaveLoadOutcome", "operation": format!("{:?}", event.operation),
                    "result": format!("{:?}", event.result), "target": event.target,
                    "source": format!("{:?}", event.source)}));
            }
        }
        if let Some(messages) = world.get_resource::<Messages<DeconstructionCommitOutcome>>() {
            let events: Vec<_> = observations.deconstruction.read(messages).take(4097).cloned().collect();
            for event in events {
                observations.events.push(json!({"sample_index": index, "real_seconds": now,
                    "producer": "DeconstructionCommitOutcome", "operation": "Deconstruct", "result": format!("{:?}", event.result),
                    "worker": format!("{:?}", event.worker), "order": format!("{:?}", event.order),
                    "owner": format!("{:?}", event.target)}));
            }
        }
        if let Some(messages) = world.get_resource::<Messages<BuildingCompletedVisualMessage>>() {
            let events: Vec<_> = observations.completed.read(messages).take(4097).cloned().collect();
            for event in events {
                observations.events.push(json!({"sample_index": index, "real_seconds": now,
                    "producer": "BuildingCompletedVisualMessage", "operation": "BuildingCompletedVisualMessage",
                    "blueprint_owner": format!("{:?}", event.blueprint_entity)}));
            }
        }
        if let Some(messages) = world.get_resource::<Messages<TaskCompletedVisualMessage>>() {
            let events: Vec<_> = observations.tasks.read(messages).take(4097).cloned().collect();
            for event in events {
                observations.events.push(json!({"sample_index": index, "real_seconds": now,
                    "producer": "TaskCompletedVisualMessage", "operation": "TaskCompleted",
                    "worker": format!("{:?}", event.entity), "assignment": format!("{:?}", event.assignment_entity),
                    "target": format!("{:?}", event.current_target_entity), "work_type": format!("{:?}", event.current_work_type)}));
            }
        }
        if let Some(messages) = world.get_resource::<Messages<DreamTransferredVisualMessage>>() {
            let events: Vec<_> = observations.dreams.read(messages).take(4097).cloned().collect();
            for event in events {
                let source = match event.source {
                    hw_core::events::DreamTransferVisualSource::Sleeping { origin } => json!({"kind": "Sleeping", "origin": origin.to_array()}),
                    hw_core::events::DreamTransferVisualSource::RestArea { rest_area, origin } => json!({"kind": "RestArea", "owner": format!("{rest_area:?}"), "origin": origin.to_array()}),
                };
                observations.events.push(json!({"sample_index": index, "real_seconds": now,
                    "producer": "DreamTransferredVisualMessage", "operation": "DreamTransferred",
                    "worker": format!("{:?}", event.soul), "amount": event.amount,
                    "source": source, "is_final": event.is_final}));
            }
        }
        if let Some(input) = world.get_resource::<ButtonInput<MouseButton>>() {
            for (pressed, operation) in [(input.just_pressed(MouseButton::Left), "PointerPress"),
                (input.just_released(MouseButton::Left), "PointerRelease")] {
                if pressed {
                    observations.events.push(json!({"sample_index": index, "real_seconds": now,
                        "operation": operation}));
                }
            }
        }
        if let Some(input) = world.get_resource::<ButtonInput<KeyCode>>() {
            for (keys, operation) in [(input.get_just_pressed().collect::<Vec<_>>(), "KeyPress"),
                (input.get_just_released().collect::<Vec<_>>(), "KeyRelease")] {
                for key in keys {
                    observations.events.push(json!({"sample_index": index, "real_seconds": now,
                        "operation": operation, "key": format!("{key:?}")}));
                }
            }
        }
    });
    let state = world.resource::<Observations>();
    if now < state.next
        && state.events.len() == state.written_events
        && state.events.len() < 4096
        && state.failure.is_none()
    {
        return;
    }
    let sample = if let Some(failure) = &state.failure {
        Err(failure.clone())
    } else if state.samples.len() >= 1200 || state.events.len() >= 4096 {
        Err("production observation cap reached".into())
    } else {
        snapshot(world)
    };
    let mut failure = sample.as_ref().err().cloned();
    if let Ok(mut sample) = sample {
        let mut state = world.resource_mut::<Observations>();
        let index = state.samples.len();
        sample["sample_index"] = json!(index);
        if let Some(before) = state.samples.last() {
            match state_transitions(before, &sample, index) {
                Ok(events) if state.events.len() + events.len() < 4096 => {
                    state.events.extend(events)
                }
                Ok(_) => failure = Some("production transition/event cap exceeded".into()),
                Err(error) => failure = Some(error),
            }
        }
        state.samples.push(sample);
        state.next = now + 0.5;
        state.written_events = state.events.len();
    }
    {
        let mut state = world.resource_mut::<Observations>();
        state.failure = failure.clone();
        for (index, event) in state.events.iter_mut().enumerate() {
            event["event_index"] = json!(index);
        }
    }
    let state = world.resource::<Observations>();
    let session = world.resource::<Session>();
    let record = json!({"schema_version": 2, "scope": "production-raw-observations-v1",
        "transition_contract": "indexed-state-delta-v1",
        "nonce": session.nonce, "process_id": std::process::id(), "accepted": false,
        "performance_evidence": false, "promotion_authority": false,
        "unsupported_observations": [
            "Placement/CompanionPlacement: interface/selection/building_place has no terminal operation receipt",
            "Move: hw_jobs::MovePlantTask is state, not a move commit/reject/cancel receipt",
            "Navigation: actor Transform/grid samples do not prove a navigation request arrived",
            "CancelConstruction/refund: BlueprintCancelRequested is a request, not a terminal refund receipt",
            "MaterialDelivery: delivered_materials deltas do not identify the delivering transaction",
            "Salvage: DeconstructionCommitOutcome does not contain emitted salvage item IDs/counts",
            "Durable state: SaveLoadOutcome does not contain canonical before/after save-state bytes",
            "Complete lifecycle witnesses/cycles are not inferred from input or state deltas",
            "GPU/native/RSS/total application handles require independent host instrumentation"],
        "failure": failure, "samples": state.samples, "events": state.events});
    let result = serde_json::to_vec(&record)
        .map_err(|e| e.to_string())
        .and_then(|bytes| {
            if bytes.len() > 64 * 1024 * 1024 {
                return Err("production observation byte cap exceeded".into());
            }
            let temporary = session.output.with_extension("tmp");
            std::fs::write(&temporary, bytes)
                .and_then(|()| std::fs::rename(temporary, &session.output))
                .map_err(|e| e.to_string())
        });
    if failure.is_some() || result.is_err() {
        error!(
            "Production observations failed: {:?} {:?}",
            failure,
            result.err()
        );
        world.write_message(AppExit::error());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        json!({"real_seconds": 1.0, "owners": [], "blueprints": [], "roots": [],
            "items": [], "move_tasks": [], "generations": [], "owner_layers": []})
    }

    #[test]
    fn transitions_preserve_exact_owner_rows_and_sample_indices_without_verdicts() {
        let mut before = sample();
        before["owners"] = json!([{"entity": "owner-a", "position": [1, 2, 0]}]);
        let mut after = before.clone();
        after["real_seconds"] = json!(2.0);
        after["owners"] = json!([{"entity": "owner-a", "position": [3, 2, 0]}]);
        let events = state_transitions(&before, &after, 7).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["before_sample_index"], 6);
        assert_eq!(events[0]["sample_index"], 7);
        assert_eq!(events[0]["entity"], "owner-a");
        assert_eq!(events[0]["before"], before["owners"][0]);
        assert_eq!(events[0]["after"], after["owners"][0]);
        assert!(events[0].get("result").is_none());
        assert!(events[0].get("accepted").is_none());
    }

    #[test]
    fn removed_owner_and_remaining_map_layer_are_both_observed() {
        let mut before = sample();
        before["owners"] = json!([{"entity": "owner-a"}]);
        before["owner_layers"] =
            json!([{"entity": "owner-a", "exists": true, "building_grids": [[1, 2]]}]);
        let mut after = before.clone();
        after["owners"] = json!([]);
        after["owner_layers"][0]["exists"] = json!(false);
        let events = state_transitions(&before, &after, 1).unwrap();
        assert_eq!(events.len(), 2);
        assert!(events[0]["after"].is_null());
        assert_eq!(events[1]["after"]["building_grids"], json!([[1, 2]]));
    }

    #[test]
    fn duplicate_identity_and_transition_overflow_fail_closed() {
        let before = sample();
        let mut after = sample();
        after["owners"] = json!([{"entity": "duplicate"}, {"entity": "duplicate"}]);
        assert!(state_transitions(&before, &after, 1).is_err());
        after["owners"] = Value::Array(
            (0..4096)
                .map(|index| json!({"entity": format!("owner-{index}")}))
                .collect(),
        );
        assert!(state_transitions(&before, &after, 1).is_err());
    }
}
