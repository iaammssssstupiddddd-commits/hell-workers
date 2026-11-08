//! One-time seeded M2 UI art feedback. No timed driver, UI writer or repair.
use super::{BuildingArtSession, m2_fixture};
use crate::assets::building_asset_set::{BuildingAssetKind, BuildingAssetPool};
use bevy::{camera_controller::pan_camera::PanCamera, prelude::*};
use hw_core::{
    relationships::{StoredIn, StoredItems, TaskWorkers, WorkingOn},
    soul::DamnedSoul,
};
use hw_jobs::{ActiveTaskIdentity, AssignedTask, Building, RefineData, RefinePhase, WorkType};
use hw_logistics::{ResourceItem, ResourceType, Stockpile};
use serde_json::{Value, json};

pub(super) fn case_kind(case: &str) -> Option<BuildingAssetKind> {
    match case {
        "m2-ui-tank-empty" | "m2-ui-tank-partial" | "m2-ui-tank-full" => {
            Some(BuildingAssetKind::Tank)
        }
        "m2-ui-mixer-refining" => Some(BuildingAssetKind::MudMixer),
        _ => None,
    }
}

#[derive(Resource)]
struct UiFixture {
    case: String,
    owner: Option<Entity>,
    worker: Option<Entity>,
}

pub(super) fn configure(
    app: &mut App,
    session: &BuildingArtSession,
    case: &str,
) -> Result<(), String> {
    validate_admission(
        session,
        case,
        &std::env::var("HW_M2_ACCEPTANCE_LEG").unwrap_or_default(),
        &std::env::var("HW_NATIVE_UI_NONCE").unwrap_or_default(),
        |key| std::env::var_os(key).is_some(),
    )?;
    app.insert_resource(UiFixture {
        case: case.into(),
        owner: None,
        worker: None,
    });
    m2_fixture::configure_ui(app);
    Ok(())
}

fn validate_admission(
    session: &BuildingArtSession,
    case: &str,
    leg: &str,
    nonce: &str,
    present: impl Fn(&str) -> bool,
) -> Result<(), String> {
    if case_kind(case) != Some(session.identity.kind)
        || session.mode != "feedback"
        || session.identity.authority
            != crate::assets::building_asset_set::BuildingAssetAuthority::ArtPreview
        || session.variant != super::BuildingArtVariant::Gallery
        || session.view != super::BuildingArtView::Normal
        || session.lighting != super::BuildingArtLighting::Standard
        || session.spa_mask_page.is_some()
        || session.lamp_supply_target.is_some()
        || leg != "ui-art"
        || !present("HW_NATIVE_UI_ROOT")
        || nonce != session.nonce
        || [
            "HW_PERF_SCENARIO",
            "HW_PERF_ENABLED",
            "HW_PERF_WORKLOAD",
            "HW_BUILDING_ORDINARY_ART_SESSION",
            "HW_BUILDING_PRODUCTION_OBSERVATIONS",
            "HW_BUILDING_ASSET_RELEASES",
            "HW_BUILDING_M6_MODE",
            "HW_BRIDGE_ACCEPTANCE_PROBE",
            "HW_NATIVE_UI_LAYOUT_ONLY",
        ]
        .into_iter()
        .any(present)
    {
        return Err(
            "M2 UI requires its exact separate case/kind/nonce and non-static feedback probe"
                .into(),
        );
    }
    Ok(())
}

pub(super) fn prepare(world: &mut World) {
    if world.resource::<UiFixture>().owner.is_some() {
        return;
    }
    let Some(owner) = world.resource::<m2_fixture::Fixture>().owner else {
        return;
    };
    if world.get::<Building>(owner).is_none() {
        return;
    }
    let case = world.resource::<UiFixture>().case.clone();
    // Freeze the explicitly seeded owner after its one-shot construction bounce,
    // not at an arbitrary paused bounce scale. Never repair this after readiness.
    world
        .entity_mut(owner)
        .remove::<hw_visual::blueprint::BuildingBounceEffect>();
    world
        .get_mut::<Transform>(owner)
        .expect("constructed owner pose")
        .scale = Vec3::ONE;
    let worker = if case == "m2-ui-mixer-refining" {
        let actor = world
            .query_filtered::<(Entity, &AssignedTask), With<DamnedSoul>>()
            .iter(world)
            .filter(|(entity, task)| {
                matches!(task, AssignedTask::None)
                    && world.get::<WorkingOn>(*entity).is_none()
                    && world.get::<ActiveTaskIdentity>(*entity).is_none()
            })
            .map(|(entity, _)| entity)
            .min_by_key(|entity| entity.to_bits())
            .expect("M2 UI needs an idle, unassigned real Soul");
        let point = world
            .get::<Transform>(owner)
            .expect("constructed owner pose")
            .translation;
        world
            .get_mut::<Transform>(actor)
            .expect("Soul pose")
            .translation = point + Vec3::new(0., -48., 0.);
        world.entity_mut(actor).insert((
            AssignedTask::Refine(RefineData {
                mixer: owner,
                phase: RefinePhase::Refining { progress: 0.0 },
            }),
            WorkingOn(owner),
            ActiveTaskIdentity::new(owner, owner, WorkType::Refine),
        ));
        // One setup projection through the existing production producers.
        world
            .run_system_cached(hw_jobs::sync_refine_activity_index_system)
            .expect("M2 UI refine resources");
        world
            .run_system_cached(hw_jobs::visual_sync::sync_mud_mixer_active_system)
            .expect("M2 UI visual resources");
        Some(actor)
    } else {
        let count = match case.as_str() {
            "m2-ui-tank-empty" => 0,
            "m2-ui-tank-partial" => 25,
            "m2-ui-tank-full" => 50,
            _ => unreachable!("admitted M2 UI case"),
        };
        assert!(
            world
                .get::<StoredItems>(owner)
                .is_none_or(|items| items.is_empty()),
            "fresh M2 UI owner already has items"
        );
        for _ in 0..count {
            world.spawn((
                ResourceItem(ResourceType::Water),
                StoredIn(owner),
                Transform::default(),
                Visibility::Hidden,
            ));
        }
        None
    };
    for (mut pose, mut controller) in world
        .query_filtered::<(&mut Transform, &mut PanCamera), With<hw_ui::camera::MainCamera>>()
        .iter_mut(world)
    {
        pose.scale = Vec3::splat(5.);
        controller.zoom_factor = 5.;
    }
    // Preparation only, never repeated. Do not inject text, hover or selection.
    world.resource_mut::<Time<Virtual>>().pause();
    let mut fixture = world.resource_mut::<UiFixture>();
    fixture.owner = Some(owner);
    fixture.worker = worker;
}

fn project(world: &mut World, entity: Entity) -> Option<[f32; 2]> {
    let point = world.get::<Transform>(entity)?.translation;
    let mut cameras =
        world.query_filtered::<(&Camera, &GlobalTransform), With<hw_ui::camera::MainCamera>>();
    let (camera, global) = cameras.single(world).ok()?;
    camera
        .world_to_viewport(global, point)
        .ok()
        .map(|point| point.to_array())
}

pub(super) fn snapshot(world: &mut World) -> Value {
    let Some(fixture) = world.get_resource::<UiFixture>() else {
        return Value::Null;
    };
    let (case, owner, worker) = (fixture.case.clone(), fixture.owner, fixture.worker);
    let Some(owner) = owner else {
        return json!({"ready": false});
    };
    let session = world.resource::<BuildingArtSession>();
    let identity = session.identity.clone();
    let active = world
        .resource::<BuildingAssetPool>()
        .active(identity.kind)
        .cloned();
    let stockpile = world.get::<Stockpile>(owner).map(|stockpile| {
        json!({"capacity": stockpile.capacity,
        "resource": format!("{:?}", stockpile.resource_type)})
    });
    let mut inverse = world
        .get::<StoredItems>(owner)
        .map(|items| items.iter().map(Entity::to_bits).collect::<Vec<_>>())
        .unwrap_or_default();
    inverse.sort_unstable();
    let mut items = world
        .query::<(Entity, &StoredIn, Option<&ResourceItem>)>()
        .iter(world)
        .filter(|(_, stored, _)| stored.0 == owner)
        .map(|(entity, stored, item)| {
            json!({"entity": entity.to_bits(), "owner": stored.0.to_bits(),
            "resource": item.map(|item| format!("{:?}", item.0))})
        })
        .collect::<Vec<_>>();
    items.sort_by_key(|item| item["entity"].as_u64());
    let mut workers = world
        .get::<TaskWorkers>(owner)
        .map(|workers| {
            workers
                .iter()
                .map(|entity| entity.to_bits())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    workers.sort_unstable();
    let mut working_on = world
        .query::<(Entity, &WorkingOn)>()
        .iter(world)
        .filter(|(_, target)| target.0 == owner)
        .map(|(entity, _)| entity.to_bits())
        .collect::<Vec<_>>();
    working_on.sort_unstable();
    let mut assigned = world
        .query::<(Entity, &AssignedTask)>()
        .iter(world)
        .filter_map(|(entity, task)| {
            matches!(task, AssignedTask::Refine(data) if data.mixer == owner)
                .then_some(entity.to_bits())
        })
        .collect::<Vec<_>>();
    assigned.sort_unstable();
    let actor = worker.map(|actor| {
        let task = world.get::<AssignedTask>(actor);
        let binding = world.get::<ActiveTaskIdentity>(actor);
        json!({"entity": actor.to_bits(), "soul": world.get::<DamnedSoul>(actor).is_some(),
            "mixer": task.and_then(|task| match task { AssignedTask::Refine(data) => Some(data.mixer.to_bits()), _ => None }),
            "phase": task.and_then(|task| match task { AssignedTask::Refine(data) => Some(format!("{:?}", data.phase)), _ => None }),
            "progress": task.and_then(|task| match task { AssignedTask::Refine(RefineData { phase: RefinePhase::Refining { progress }, .. }) => Some(*progress), _ => None }),
            "working_on": world.get::<WorkingOn>(actor).map(|target| target.0.to_bits()),
            "assignment": binding.map(|binding| binding.assignment_entity.to_bits()),
            "target": binding.map(|binding| binding.current_target_entity.to_bits()),
            "work_type": binding.map(|binding| format!("{:?}", binding.current_work_type))})
    });
    let target = worker.unwrap_or(owner);
    json!({"scope": "seeded-m2-ui-art-feedback-v1", "case": case, "ready": true,
        "identity": identity, "active": active, "owner": owner.to_bits(), "target": target.to_bits(),
        "kind": world.get::<Building>(owner).map(|building| format!("{:?}", building.kind)),
        "owner_point": project(world, owner), "target_point": project(world, target),
        "inventory_overflow": items.len() > 128 || workers.len() > 128 || working_on.len() > 128 || assigned.len() > 128,
        "stockpile": stockpile, "items": items, "stored_items": inverse, "workers": workers, "working_on": working_on, "assigned": assigned, "worker": actor,
        "virtual_seconds": world.resource::<Time<Virtual>>().elapsed_secs_f64(),
        "natural_lifecycle_evidence": false, "performance_evidence": false, "promotion_authority": false})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_admission_rejects_foreign_context_without_registering_old_drivers() {
        let value = json!({"mode": "feedback", "identity": {"kind": "Tank", "generation": 1,
            "authority": "art_preview", "manifest_sha256": "a".repeat(64)},
            "locator": "manifests/building-tank-v1.buildingset", "nonce": "b".repeat(32),
            "status_path": "/unused/probe.json", "view": "normal", "variant": "gallery", "lighting": "standard"});
        let session: BuildingArtSession = serde_json::from_value(value.clone()).unwrap();
        let present = |key: &str| key == "HW_NATIVE_UI_ROOT";
        assert!(
            validate_admission(
                &session,
                "m2-ui-tank-full",
                "ui-art",
                &session.nonce,
                present
            )
            .is_ok()
        );
        for (case, leg, nonce) in [
            ("m2-ui-mixer-refining", "ui-art", session.nonce.as_str()),
            ("m2-ui-tank-full", "water", session.nonce.as_str()),
            ("m2-ui-tank-full", "ui-art", "foreign"),
        ] {
            assert!(validate_admission(&session, case, leg, nonce, present).is_err());
        }
        assert!(
            validate_admission(
                &session,
                "m2-ui-tank-full",
                "ui-art",
                &session.nonce,
                |_| false
            )
            .is_err()
        );
        for conflict in [
            "HW_PERF_SCENARIO",
            "HW_PERF_ENABLED",
            "HW_BUILDING_M6_MODE",
            "HW_BUILDING_PRODUCTION_OBSERVATIONS",
            "HW_NATIVE_UI_LAYOUT_ONLY",
        ] {
            assert!(
                validate_admission(
                    &session,
                    "m2-ui-tank-full",
                    "ui-art",
                    &session.nonce,
                    |key| present(key) || key == conflict
                )
                .is_err()
            );
        }
        for (key, changed) in [
            ("view", "overview"),
            ("variant", "m2-art-context"),
            ("lighting", "dim"),
            ("mode", "candidate"),
        ] {
            let mut wrong = value.clone();
            wrong[key] = json!(changed);
            let wrong: BuildingArtSession = serde_json::from_value(wrong).unwrap();
            assert!(
                validate_admission(&wrong, "m2-ui-tank-full", "ui-art", &wrong.nonce, present)
                    .is_err()
            );
        }
    }

    #[test]
    fn ui_cases_are_exact_and_never_admit_a_legacy_leg() {
        for case in ["m2-ui-tank-empty", "m2-ui-tank-partial", "m2-ui-tank-full"] {
            assert_eq!(case_kind(case), Some(BuildingAssetKind::Tank));
        }
        assert_eq!(
            case_kind("m2-ui-mixer-refining"),
            Some(BuildingAssetKind::MudMixer)
        );
        for case in [
            "refining",
            "water",
            "navigation",
            "m2-ui-mixer-idle",
            "m2-art-context",
            "m2-ui-tank-partial-extra",
        ] {
            assert!(case_kind(case).is_none());
        }
    }

    #[test]
    fn ui_water_setup_is_once_and_never_reinjects_or_repairs_time() {
        for (case, count) in [
            ("m2-ui-tank-empty", 0),
            ("m2-ui-tank-partial", 25),
            ("m2-ui-tank-full", 50),
        ] {
            let mut world = World::new();
            world.init_resource::<Time<Virtual>>();
            let owner = world
                .spawn((
                    Building {
                        kind: hw_jobs::BuildingType::Tank,
                        is_provisional: false,
                    },
                    Transform::default(),
                ))
                .id();
            let mut fixture = m2_fixture::Fixture::default();
            fixture.owner = Some(owner);
            world.insert_resource(fixture);
            world.insert_resource(UiFixture {
                case: case.into(),
                owner: None,
                worker: None,
            });
            prepare(&mut world);
            assert!(world.resource::<Time<Virtual>>().is_paused());
            assert_eq!(world.query::<&StoredIn>().iter(&world).count(), count);
            world.resource_mut::<Time<Virtual>>().unpause();
            world
                .resource_mut::<Time<Virtual>>()
                .advance_by(std::time::Duration::from_secs(20));
            prepare(&mut world);
            assert!(!world.resource::<Time<Virtual>>().is_paused());
            assert_eq!(world.query::<&StoredIn>().iter(&world).count(), count);
        }
    }
}
