//! Profiling-only, bounded observations of the ordinary world. This does not
//! create owners, drive input, grant authority, or claim performance evidence.
use super::snapshot_io::{require_fresh_path, temporary_path, write_observation};
use bevy::{
    ecs::message::MessageCursor,
    input::{keyboard::KeyboardInput, mouse::MouseButtonInput},
    prelude::*,
};
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
use crate::interface::selection::building_place::observed_outcomes::PlacementReceipts;
use crate::systems::jobs::{CancellationReceipts, finalize_cancellation_receipts};
use crate::systems::save::SaveLoadOutcome;
use hw_jobs::delivery_observations::BlueprintDeliveryReceipts;

#[derive(Resource, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    nonce: String,
    output: PathBuf,
    admitted_identities: Vec<BuildingAssetSetIdentity>,
    #[serde(skip)]
    art_feedback: bool,
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
    keyboard: MessageCursor<KeyboardInput>,
    mouse: MessageCursor<MouseButtonInput>,
    failure: Option<String>,
    tracked_owners: std::collections::HashSet<Entity>,
    written_events: usize,
}

fn production_session(raw: &str) -> Result<Session, String> {
    let session: Session = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    if session
        .admitted_identities
        .iter()
        .any(|identity| identity.authority != BuildingAssetAuthority::ReleaseApproved)
    {
        return Err("production observations require release-approved identities".into());
    }
    Ok(session)
}

pub(super) fn configure(app: &mut App) -> Result<(), String> {
    let session = if let Some(art) = app.world().get_resource::<super::ordinary_art::Session>() {
        // Only the validated dedicated session selects this scope. The public
        // production JSON cannot set art_feedback or admit draft identities.
        Session {
            nonce: art.nonce.clone(),
            output: art.output.clone(),
            admitted_identities: vec![art.identity.clone()],
            art_feedback: true,
        }
    } else {
        let Ok(raw) = std::env::var("HW_BUILDING_PRODUCTION_OBSERVATIONS") else {
            return Ok(());
        };
        production_session(&raw)?
    };
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
        || require_fresh_path(&session.output).is_err()
        || require_fresh_path(&temporary_path(&session.output)).is_err()
        || session.admitted_identities.len() > 18
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
        .init_resource::<PlacementReceipts>()
        .init_resource::<CancellationReceipts>()
        .init_resource::<BlueprintDeliveryReceipts>()
        .init_resource::<crate::systems::save::observations::DurableReceipts>()
        .add_observer(building_completed)
        .add_systems(
            Last,
            (finalize_cancellation_receipts, observe)
                .chain()
                .after(crate::systems::save::SaveLoadApplySet),
        );
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

fn placement_context(world: &mut World) -> Result<Value, String> {
    let yards: Vec<_> = world
        .query::<(Entity, &hw_world::Yard)>()
        .iter(world)
        .take(33)
        .map(|(entity, yard)| (entity, yard.clone()))
        .collect();
    if yards.len() > 32 {
        return Err("production Yard observation cap exceeded".into());
    }
    if yards.iter().any(|(_, yard)| {
        !yard.min.is_finite()
            || !yard.max.is_finite()
            || yard.min.x > yard.max.x
            || yard.min.y > yard.max.y
    }) {
        return Err("production Yard bounds are nonfinite or inverted".into());
    }
    let primary_window = world
        .query_filtered::<Entity, (With<bevy::window::PrimaryWindow>, With<Window>)>()
        .single(world)
        .ok();
    let mut cameras = world.query_filtered::<(
        Entity,
        &Camera,
        &GlobalTransform,
        &bevy::camera::RenderTarget,
    ), With<hw_ui::camera::MainCamera>>();
    let camera = match cameras.single(world) {
        Ok((entity, camera, transform, target)) if camera.is_active => {
            Some((entity, camera.clone(), *transform, target.clone()))
        }
        _ => None,
    };
    if camera
        .as_ref()
        .is_some_and(|(_, _, transform, _)| !transform.translation().is_finite())
    {
        return Err("production placement camera position is nonfinite".into());
    }
    let rows: Vec<_> = yards
        .iter()
        .map(|(entity, yard)| {
            let corners = [
                yard.min,
                Vec2::new(yard.max.x, yard.min.y),
                yard.max,
                Vec2::new(yard.min.x, yard.max.y),
            ];
            let projected = camera.as_ref().and_then(|(_, camera, transform, _)| {
                corners
                    .iter()
                    .map(|point| {
                        camera
                            .world_to_viewport(transform, point.extend(0.0))
                            .ok()
                            .filter(|point| point.is_finite())
                            .map(|point| point.to_array())
                    })
                    .collect::<Option<Vec<_>>>()
            });
            let grid_min = hw_world::WorldMap::world_to_grid(yard.min);
            let grid_max = hw_world::WorldMap::world_to_grid(yard.max);
            json!({"entity": format!("{entity:?}"), "min": yard.min.to_array(),
                "max": yard.max.to_array(), "grid_min": grid_min, "grid_max": grid_max,
                "grid_world_min": hw_world::WorldMap::grid_to_world(grid_min.0, grid_min.1).to_array(),
                "grid_world_max": hw_world::WorldMap::grid_to_world(grid_max.0, grid_max.1).to_array(),
                "logical_viewport_corners": projected})
        })
        .collect();
    Ok(
        json!({"camera": camera.as_ref().map(|(entity, _, _, _)| format!("{entity:?}")),
        "camera_position": camera.as_ref().map(|(_, _, transform, _)| transform.translation().to_array()),
        "target_window": camera.as_ref().and_then(|(_, _, _, target)| match target {
            bevy::camera::RenderTarget::Window(reference) => match reference {
                bevy::window::WindowRef::Primary => primary_window,
                bevy::window::WindowRef::Entity(entity) => Some(*entity),
            }.map(|entity| format!("{entity:?}")),
            _ => None,
        }),
        "logical_viewport_size": camera.as_ref().and_then(|(_, camera, _, _)| camera.logical_viewport_size().map(|size| size.to_array())),
        "yards": rows}),
    )
}

fn companion_context(world: &mut World) -> Result<Value, String> {
    use crate::app_contexts::CompanionPlacementState;
    use hw_logistics::{BelongsTo, BucketStorage, PendingBelongsToBlueprint};
    let state = world.get_resource::<CompanionPlacementState>();
    let available = state.is_some();
    let pending = state.and_then(|state| state.0.as_ref()).map(|state| {
        json!({"parent_kind": format!("{:?}", state.parent_kind),
            "parent_anchor": state.parent_anchor, "kind": format!("{:?}", state.kind),
            "center": state.center.to_array(), "radius": state.radius, "required": state.required})
    });
    if pending.as_ref().is_some_and(|state| {
        state["center"].as_array().is_none_or(|values| {
            values
                .iter()
                .any(|value| value.as_f64().is_none_or(|v| !v.is_finite()))
        }) || state["radius"]
            .as_f64()
            .is_none_or(|v| !v.is_finite() || v <= 0.0)
    }) {
        return Err("production companion context is nonfinite".into());
    }
    let mut query = world.query_filtered::<(
        Entity,
        Option<&Transform>,
        Option<&PendingBelongsToBlueprint>,
        Option<&BelongsTo>,
    ), With<BucketStorage>>();
    let roots: Vec<_> = query.iter(world).take(513).collect();
    if roots.len() > 512 {
        return Err("production companion observation cap exceeded".into());
    }
    let mut storages = Vec::new();
    for (entity, pose, pending_owner, owner) in roots {
        let pose = pose.ok_or("production companion Transform missing")?;
        if !pose.translation.is_finite() {
            return Err("production companion position is nonfinite".into());
        }
        let layers = world
            .resource::<hw_world::WorldMap>()
            .snapshot_owner(entity);
        storages.push(json!({"entity": format!("{entity:?}"),
            "position": pose.translation.to_array(),
            "grid": hw_world::WorldMap::world_to_grid(pose.translation.truncate()),
            "pending_owner": pending_owner.map(|owner| format!("{:?}", owner.0)),
            "owner": owner.map(|owner| format!("{:?}", owner.0)),
            "stockpile_grids": layers.stockpile_grids}));
    }
    storages.sort_by(|a, b| a["entity"].as_str().cmp(&b["entity"].as_str()));
    Ok(json!({"available": available, "pending": pending, "storages": storages}))
}

fn snapshot(world: &mut World) -> Result<Value, String> {
    let art_feedback = super::ordinary_art::snapshot(world)?;
    let rest_area_context = super::observations_rest::snapshot(world)?;
    let companion_context = companion_context(world)?;
    let save_catalog_context = super::observations_ui::save_catalog_context(world)?;
    let architect_panel_ready = architect_panel_ready(world);
    let architect_cards = super::observations_ui::architect_cards(world)?;
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
            "entity_bits": entity.to_bits(),
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
        json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(), "kind": format!("{:?}", bp.kind),
            "footprint": bp.occupied_grids, "progress": bp.progress,
            "materials_complete": bp.materials_complete(),
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
    let mut query = world.query_filtered::<(Entity, &Window), With<bevy::window::PrimaryWindow>>();
    let primary_window = query
        .iter(world)
        .next()
        .map(|(entity, _)| format!("{entity:?}"));
    let dpi = query
        .iter(world)
        .next()
        .map(|(_, window)| window.scale_factor());
    let cursor_position = query
        .iter(world)
        .next()
        .and_then(|(_, window)| window.cursor_position())
        .map(|point| point.to_array());
    let mut query = world.query_filtered::<&Transform, With<hw_ui::camera::MainCamera>>();
    let camera_scale = query.iter(world).next().map(|pose| pose.scale.x);
    let placement_context = placement_context(world)?;
    let bridge = super::acceptance::bridge_probe::snapshot(world)?;
    let mut actors = world.query_filtered::<(
        Entity,
        &Transform,
        Option<&hw_jobs::ActiveTaskIdentity>,
    ), With<hw_core::soul::DamnedSoul>>();
    let actors: Vec<_> = actors
        .iter(world)
        .take(129)
        .map(|(entity, pose, identity)| {
            json!({
        "entity": format!("{entity:?}"), "position": pose.translation.to_array(),
        "grid": hw_world::WorldMap::world_to_grid(pose.translation.truncate()),
        "task_identity": identity.map(|identity| json!({
            "assignment": format!("{:?}", identity.assignment_entity),
            "target": format!("{:?}", identity.current_target_entity),
            "work_type": format!("{:?}", identity.current_work_type)}))})
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
    let modes = ui_modes(world);
    let mut sample = json!({"real_seconds": world.resource::<Time<Real>>().elapsed_secs_f64(),
        "virtual_seconds": world.resource::<Time<Virtual>>().elapsed_secs_f64(),
        "paused": world.resource::<Time<Virtual>>().is_paused(),
        "menu_state": modes["menu_state"],
        "play_mode": modes["play_mode"],
        "task_mode": modes["task_mode"],
        "selected_building": world.get_resource::<crate::app_contexts::BuildContext>()
            .and_then(|context| context.0).map(|kind| format!("{kind:?}")),
        "architect_panel_ready": architect_panel_ready,
        "architect_cards": architect_cards,
        "world_epoch": world.get_resource::<WorldEpoch>().map(|epoch| epoch.get()),
        "owners": owners, "blueprints": blueprints, "roots": roots, "bridge": bridge,
        "owner_layers": owner_layers, "actors": actors,
        "items": items, "move_tasks": move_tasks, "generations": generations,
        "quality": world.get_resource::<hw_core::quality::QualitySettings>().map(|settings| format!("{:?}", settings.rtt)),
        "dpi": dpi, "primary_window": primary_window,
        "camera_scale": camera_scale, "cursor_position": cursor_position,
        "placement_context": placement_context,
        "identities": pool.active_identities(),
        "pool_active": active, "pool_pending": pending,
        "pool_owned_handle_count": pool.owned_handle_count(),
        "resident_mesh_handles": meshes.iter().map(|(id, _)| format!("{id:?}")).collect::<Vec<_>>(),
        "mesh_count": meshes.len(), "image_count": images.len(), "material_count": materials.len(),
        "image_cpu_bytes": images.iter().map(|(_, image)| image.data.as_ref().map_or(0, Vec::len)).sum::<usize>(),
        "material_shallow_bytes": materials.len() * std::mem::size_of::<TopDownStructuralMaterial>(),
        "mesh_cpu_bytes": null, "gpu_measured_bytes": null, "gpu_estimated_bytes": null,
        "native_peak_live_bytes": null, "rss_bytes": null,
        "application_handle_count": null});
    sample["rest_area_context"] = rest_area_context;
    if !art_feedback.is_null() {
        sample["art_feedback"] = art_feedback;
    }
    sample["companion_context"] = companion_context;
    sample["save_catalog_context"] = save_catalog_context;
    Ok(sample)
}

fn ui_modes(world: &World) -> Value {
    json!({
        "menu_state": world.get_resource::<hw_ui::components::MenuState>().map(|state| format!("{state:?}")),
        "play_mode": world.get_resource::<State<hw_core::game_state::PlayMode>>().map(|state| format!("{:?}", state.get())),
        "task_mode": world.get_resource::<crate::app_contexts::TaskContext>().map(|state| format!("{:?}", state.0)),
    })
}

fn architect_panel_ready(world: &mut World) -> bool {
    let mut query =
        world.query_filtered::<(&Node, &ComputedNode), With<hw_ui::components::ArchitectSubMenu>>();
    let Ok((node, computed)) = query.single(world) else {
        return false;
    };
    node.display != Display::None
        && computed.size().is_finite()
        && computed.size().x > 0.0
        && computed.size().y > 0.0
}

fn observe(world: &mut World) {
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    world.resource_scope(|world, mut observations: Mut<Observations>| {
        let index = observations.samples.len();
        if let Some(mut receipts) = world.get_resource_mut::<crate::systems::save::observations::DurableReceipts>() {
            if let Some(failure) = &receipts.failure {
                observations.failure = Some(failure.clone());
            }
            for mut receipt in receipts.rows.drain(..) {
                receipt["sample_index"] = json!(index);
                observations.events.push(receipt);
            }
        }
        if let Some(mut receipts) = world.get_resource_mut::<BlueprintDeliveryReceipts>() {
            if let Some(failure) = receipts.failure {
                observations.failure = Some(failure.into());
            }
            for receipt in receipts.rows.drain(..) {
                if !target(receipt.kind) { continue; }
                observations.events.push(json!({"sample_index": index, "real_seconds": now,
                    "producer": "ordinary-blueprint-delivery-branch-v1", "operation": "MaterialDelivery",
                    "result": "Committed", "worker": format!("{:?}", receipt.worker),
                    "assignment": format!("{:?}", receipt.assignment), "owner": format!("{:?}", receipt.owner),
                    "item": format!("{:?}", receipt.item), "world_epoch": receipt.world_epoch,
                    "kind": format!("{:?}", receipt.kind), "resource": format!("{:?}", receipt.resource),
                    "before_count": receipt.before_count, "after_count": receipt.after_count,
                    "footprint": receipt.footprint, "wheelbarrow": receipt.wheelbarrow}));
            }
        }
        if let Some(mut receipts) = world.get_resource_mut::<CancellationReceipts>() {
            if let Some(failure) = &receipts.failure {
                observations.failure = Some(failure.clone());
            }
            for mut receipt in receipts.rows.drain(..) {
                receipt["sample_index"] = json!(index);
                receipt["real_seconds"] = json!(now);
                observations.events.push(receipt);
            }
        }
        if let Some(mut receipts) = world.get_resource_mut::<PlacementReceipts>() {
            if receipts.failed {
                observations.failure = Some("ordinary placement receipt cap/context failed".into());
            }
            for mut receipt in receipts.rows.drain(..) {
                receipt["sample_index"] = json!(index);
                observations.events.push(receipt);
            }
        }
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
        if let Some(messages) = world.get_resource::<Messages<KeyboardInput>>() {
            record_keyboard_messages(&mut observations, messages, index, now);
        }
        if let Some(messages) = world.get_resource::<Messages<MouseButtonInput>>() {
            record_mouse_messages(&mut observations, messages, index, now);
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
    let interval = sample_interval(world);
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
        state.next = now + interval;
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
    let scope = if session.art_feedback {
        super::ordinary_art::SCOPE
    } else {
        "production-raw-observations-v1"
    };
    let record = json!({"schema_version": 2, "scope": scope,
        "transition_contract": "indexed-state-delta-v1",
        "nonce": session.nonce, "process_id": std::process::id(), "accepted": false,
        "performance_evidence": false, "promotion_authority": false,
        "unsupported_observations": [
            "Placement receipts are command-branch results, not full input/domain lifecycle acceptance",
            "Move: hw_jobs::MovePlantTask is state, not a move commit/reject/cancel receipt",
            "Navigation: actor Transform/grid samples do not prove a navigation request arrived",
            "CancelConstruction: BlueprintCancellationApplied binds owner cleanup/refund IDs, not full reservation/inventory lifecycle; SoulSpa needs its separate producer",
            "MaterialDelivery: typed receipts cover ordinary blueprint hand/wheelbarrow branches only, not every construction-site delivery",
            "Salvage: DeconstructionCommitOutcome does not contain emitted salvage item IDs/counts",
            "Durable state: commit byte hashes and selected Entity remaps do not prove canonical world-state equality or complete inventory lifecycle",
            "Complete lifecycle witnesses/cycles are not inferred from input or state deltas",
            "GPU/native/RSS/total application handles require independent host instrumentation"],
        "failure": failure, "samples": state.samples, "events": state.events});
    let result = serde_json::to_vec(&record)
        .map_err(|e| e.to_string())
        .and_then(|bytes| {
            if bytes.len() > 64 * 1024 * 1024 {
                return Err("production observation byte cap exceeded".into());
            }
            write_observation(&session.output, &bytes).map_err(|e| e.to_string())
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

// Only the explicit profiling observer uses this cadence. Ordinary camera
// input remains owned by the existing input systems; no key or outcome is set.
fn sample_interval(world: &World) -> f64 {
    if world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keys| {
            keys.any_pressed([KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD])
        })
    {
        0.05
    } else {
        0.5
    }
}

fn record_mouse_messages(
    observations: &mut Observations,
    messages: &Messages<MouseButtonInput>,
    sample_index: usize,
    now: f64,
) {
    for event in observations.mouse.read(messages).take(4097) {
        observations
            .events
            .push(json!({"sample_index": sample_index, "real_seconds": now,
            "producer": "MouseButtonInput", "operation": "MouseButtonInput",
            "button": format!("{:?}", event.button), "state": format!("{:?}", event.state),
            "window": format!("{:?}", event.window)}));
    }
}

fn record_keyboard_messages(
    observations: &mut Observations,
    messages: &Messages<KeyboardInput>,
    sample_index: usize,
    now: f64,
) {
    let events: Vec<_> = observations
        .keyboard
        .read(messages)
        .take(4097)
        .cloned()
        .collect();
    for event in events {
        observations
            .events
            .push(json!({"sample_index": sample_index, "real_seconds": now,
            "producer": "KeyboardInput", "operation": "KeyboardInput",
            "key": format!("{:?}", event.key_code),
            "state": format!("{:?}", event.state),
            "repeat": event.repeat, "window": format!("{:?}", event.window)}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::{ButtonState, keyboard::Key};

    #[test]
    fn public_production_json_cannot_admit_preview_or_select_feedback_scope() {
        let mut value = json!({"nonce": "a".repeat(32), "output": "/private/observation.json", "admitted_identities": []});
        assert!(!production_session(&value.to_string()).unwrap().art_feedback);
        value["art_feedback"] = json!(true);
        assert!(production_session(&value.to_string()).is_err());
        value.as_object_mut().unwrap().remove("art_feedback");
        for authority in ["art_preview", "isolated_candidate"] {
            value["admitted_identities"] = json!([{"kind": "RestArea", "generation": 1,
                "authority": authority, "manifest_sha256": "a".repeat(64)}]);
            assert!(production_session(&value.to_string()).is_err());
        }
    }

    #[test]
    fn companion_context_reads_real_pending_state_and_stockpile_ownership() {
        use crate::app_contexts::{
            CompanionParentKind, CompanionPlacement, CompanionPlacementKind,
            CompanionPlacementState,
        };
        let mut world = World::new();
        world.init_resource::<hw_world::WorldMap>();
        assert_eq!(
            companion_context(&mut world).unwrap(),
            json!({"available": false, "pending": null, "storages": []})
        );
        let parent = world.spawn_empty().id();
        let grid = (75, 60);
        let pose = hw_world::WorldMap::grid_to_world(grid.0, grid.1);
        let child = world
            .spawn((
                hw_logistics::BucketStorage,
                hw_logistics::PendingBelongsToBlueprint(parent),
                Transform::from_translation(pose.extend(0.0)),
            ))
            .id();
        world
            .resource_mut::<hw_world::WorldMap>()
            .register_stockpile_tile(grid, child);
        world.insert_resource(CompanionPlacementState(Some(CompanionPlacement {
            parent_kind: CompanionParentKind::Tank,
            parent_anchor: (73, 60),
            kind: CompanionPlacementKind::BucketStorage,
            center: Vec2::new(768.0, 352.0),
            radius: 160.0,
            required: true,
        })));
        let before = world.entities().len();
        let value = companion_context(&mut world).unwrap();
        assert_eq!(value["pending"]["parent_anchor"], json!([73, 60]));
        assert_eq!(value["storages"][0]["entity"], format!("{child:?}"));
        assert_eq!(value["storages"][0]["pending_owner"], format!("{parent:?}"));
        assert!(value["storages"][0]["owner"].is_null());
        assert_eq!(value["storages"][0]["stockpile_grids"], json!([[75, 60]]));
        assert_eq!(world.entities().len(), before);
    }

    #[test]
    fn companion_context_rejects_nonfinite_or_overflow() {
        let mut world = World::new();
        world.init_resource::<hw_world::WorldMap>();
        let child = world
            .spawn((
                hw_logistics::BucketStorage,
                Transform::from_xyz(f32::NAN, 0.0, 0.0),
            ))
            .id();
        assert!(companion_context(&mut world).is_err());
        world.despawn(child);
        for _ in 0..513 {
            world.spawn((hw_logistics::BucketStorage, Transform::default()));
        }
        assert!(companion_context(&mut world).is_err());
    }

    #[test]
    fn companion_context_never_hides_missing_transform_roots() {
        let mut world = World::new();
        world.init_resource::<hw_world::WorldMap>();
        world.spawn(hw_logistics::BucketStorage);
        assert_eq!(
            companion_context(&mut world).unwrap_err(),
            "production companion Transform missing"
        );
        for _ in 0..512 {
            world.spawn(hw_logistics::BucketStorage);
        }
        assert_eq!(
            companion_context(&mut world).unwrap_err(),
            "production companion observation cap exceeded"
        );
    }

    #[test]
    fn placement_context_keeps_real_yards_and_missing_projection_explicit() {
        let mut world = World::new();
        let yard = world
            .spawn(hw_world::Yard {
                min: Vec2::new(10.0, 20.0),
                max: Vec2::new(30.0, 40.0),
            })
            .id();
        let value = placement_context(&mut world).unwrap();
        assert!(value["camera"].is_null());
        assert_eq!(value["yards"][0]["entity"], format!("{yard:?}"));
        assert_eq!(value["yards"][0]["min"], json!([10.0, 20.0]));
        assert_eq!(value["yards"][0]["max"], json!([30.0, 40.0]));
        assert!(value["yards"][0]["logical_viewport_corners"].is_null());
        assert_eq!(world.query::<&hw_world::Yard>().iter(&world).count(), 1);
        assert!(value.get("accepted").is_none());
    }

    #[test]
    fn placement_context_rejects_bad_bounds_and_overflow() {
        for (min, max) in [
            (Vec2::splat(f32::NAN), Vec2::ONE),
            (Vec2::ZERO, Vec2::splat(f32::INFINITY)),
            (Vec2::ONE, Vec2::ZERO),
        ] {
            let mut world = World::new();
            world.spawn(hw_world::Yard { min, max });
            assert!(placement_context(&mut world).is_err());
        }
        let mut world = World::new();
        for _ in 0..33 {
            world.spawn(hw_world::Yard {
                min: Vec2::ZERO,
                max: Vec2::ONE,
            });
        }
        assert!(placement_context(&mut world).is_err());
    }

    #[test]
    fn placement_context_never_selects_first_of_ambiguous_cameras() {
        let mut world = World::new();
        for _ in 0..2 {
            world.spawn((
                hw_ui::camera::MainCamera,
                Camera::default(),
                GlobalTransform::IDENTITY,
            ));
        }
        let value = placement_context(&mut world).unwrap();
        assert!(value["camera"].is_null());
        assert!(value["camera_position"].is_null());
    }

    #[test]
    fn placement_context_records_actual_target_without_rebinding_foreign_window() {
        let mut world = World::new();
        let primary = world
            .spawn((bevy::window::PrimaryWindow, Window::default()))
            .id();
        let foreign = world.spawn(Window::default()).id();
        let camera = world
            .spawn((
                hw_ui::camera::MainCamera,
                Camera::default(),
                GlobalTransform::IDENTITY,
            ))
            .id();
        let value = placement_context(&mut world).unwrap();
        assert_eq!(value["target_window"], format!("{primary:?}"));
        assert!(value["logical_viewport_size"].is_null());
        world
            .entity_mut(camera)
            .insert(bevy::camera::RenderTarget::Window(
                bevy::window::WindowRef::Entity(foreign),
            ));
        let value = placement_context(&mut world).unwrap();
        assert_eq!(value["target_window"], format!("{foreign:?}"));
        assert_ne!(value["target_window"], format!("{primary:?}"));
    }

    #[test]
    fn ui_mode_observations_are_read_only_and_do_not_infer_missing_resources() {
        use hw_core::game_state::{PlayMode, TaskMode};
        use hw_ui::components::MenuState;

        let mut world = World::new();
        assert_eq!(
            ui_modes(&world),
            json!({"menu_state": null, "play_mode": null, "task_mode": null})
        );
        world.insert_resource(MenuState::Architect);
        world.insert_resource(State::new(PlayMode::Normal));
        world.insert_resource(crate::app_contexts::TaskContext(TaskMode::None));
        assert_eq!(
            ui_modes(&world),
            json!({"menu_state": "Architect", "play_mode": "Normal", "task_mode": "None"})
        );
        assert_eq!(*world.resource::<MenuState>(), MenuState::Architect);
        assert_eq!(world.resource::<State<PlayMode>>().get(), &PlayMode::Normal);
        assert_eq!(
            world.resource::<crate::app_contexts::TaskContext>().0,
            TaskMode::None
        );
    }

    #[test]
    fn architect_readiness_requires_one_displayed_laid_out_panel() {
        use hw_ui::components::ArchitectSubMenu;
        let mut world = World::new();
        assert!(!architect_panel_ready(&mut world));
        let entity = world
            .spawn((ArchitectSubMenu, Node::default(), ComputedNode::default()))
            .id();
        assert!(!architect_panel_ready(&mut world));
        world.entity_mut(entity).insert(ComputedNode {
            size: Vec2::new(100.0, 50.0),
            ..default()
        });
        assert!(architect_panel_ready(&mut world));
        world.entity_mut(entity).get_mut::<Node>().unwrap().display = Display::None;
        assert!(!architect_panel_ready(&mut world));
        world.entity_mut(entity).get_mut::<Node>().unwrap().display = Display::Flex;
        world.spawn((
            ArchitectSubMenu,
            Node::default(),
            ComputedNode {
                size: Vec2::ONE,
                ..default()
            },
        ));
        assert!(!architect_panel_ready(&mut world));
    }

    #[test]
    fn keyboard_messages_remain_distinct_from_button_state_witnesses() {
        let mut messages = Messages::<KeyboardInput>::default();
        for state in [ButtonState::Pressed, ButtonState::Released] {
            messages.write(KeyboardInput {
                key_code: KeyCode::F12,
                logical_key: Key::F12,
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        let mut observations = Observations::default();
        record_keyboard_messages(&mut observations, &messages, 3, 1.5);
        assert_eq!(observations.events.len(), 2);
        assert_eq!(observations.events[0]["producer"], "KeyboardInput");
        assert_eq!(observations.events[0]["state"], "Pressed");
        assert_eq!(observations.events[1]["state"], "Released");
        assert_eq!(observations.events[1]["sample_index"], 3);
        record_keyboard_messages(&mut observations, &messages, 4, 2.0);
        assert_eq!(observations.events.len(), 2);
    }

    #[test]
    fn mouse_messages_preserve_window_and_do_not_infer_click_outcomes() {
        let mut messages = Messages::<MouseButtonInput>::default();
        for state in [ButtonState::Pressed, ButtonState::Released] {
            messages.write(MouseButtonInput {
                button: MouseButton::Left,
                state,
                window: Entity::PLACEHOLDER,
            });
        }
        let mut observations = Observations::default();
        record_mouse_messages(&mut observations, &messages, 4, 2.0);
        assert_eq!(observations.events.len(), 2);
        assert_eq!(observations.events[0]["producer"], "MouseButtonInput");
        assert_eq!(observations.events[0]["state"], "Pressed");
        assert_eq!(observations.events[1]["state"], "Released");
        assert_eq!(observations.events[1]["sample_index"], 4);
        assert_eq!(
            observations.events[0]["window"],
            format!("{:?}", Entity::PLACEHOLDER)
        );
        assert!(
            observations
                .events
                .iter()
                .all(|event| event.get("accepted").is_none())
        );
        record_mouse_messages(&mut observations, &messages, 5, 2.5);
        assert_eq!(observations.events.len(), 2);
    }

    struct OutputDirectory(PathBuf);

    impl OutputDirectory {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "hw-production-observation-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for OutputDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn observation_snapshots_replace_only_after_exclusive_temporary_creation() {
        let directory = OutputDirectory::new();
        let output = directory.0.join("raw.json");
        assert!(require_fresh_path(&output).is_ok());
        write_observation(&output, b"first").unwrap();
        write_observation(&output, b"second").unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"second");
        let temporary = temporary_path(&output);
        std::fs::write(&temporary, b"retained failure").unwrap();
        assert!(require_fresh_path(&temporary).is_err());
        assert!(write_observation(&output, b"must not replace").is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"second");
        assert_eq!(std::fs::read(&temporary).unwrap(), b"retained failure");
    }

    #[cfg(unix)]
    #[test]
    fn observation_paths_reject_existing_and_dangling_symlinks() {
        let directory = OutputDirectory::new();
        let output = directory.0.join("raw.json");
        let victim = directory.0.join("retained.json");
        std::fs::write(&victim, b"retained evidence").unwrap();
        std::os::unix::fs::symlink(&victim, temporary_path(&output)).unwrap();
        assert!(write_observation(&output, b"must not follow").is_err());
        assert_eq!(std::fs::read(&victim).unwrap(), b"retained evidence");
        std::os::unix::fs::symlink(directory.0.join("missing"), &output).unwrap();
        assert!(require_fresh_path(&output).is_err());
    }

    fn sample() -> Value {
        json!({"real_seconds": 1.0, "owners": [], "blueprints": [], "roots": [],
            "items": [], "move_tasks": [], "generations": [], "owner_layers": []})
    }

    #[test]
    fn held_camera_input_uses_bounded_read_only_sampling_cadence() {
        let mut world = World::new();
        assert_eq!(sample_interval(&world), 0.5);
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::F5);
        world.insert_resource(keys);
        assert_eq!(sample_interval(&world), 0.5);
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        assert_eq!(sample_interval(&world), 0.05);
        assert!(
            world
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyW)
        );
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyW);
        assert_eq!(sample_interval(&world), 0.5);
        assert!(
            world
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::F5)
        );
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
