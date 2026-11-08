//! Passive typed Mixer/resource/task inventory, never a lifecycle driver.
use bevy::prelude::*;
use hw_core::relationships::{
    DeliveringTo, LoadedIn, StoredIn, StoredItems, TaskWorkers, WorkingOn,
};
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource, Building, BuildingType, RefinePhase, StoredByMixer,
    mud_mixer::MudMixerStorage,
};
use hw_logistics::{
    BelongsTo, BucketStorage, Inventory, PendingBelongsToBlueprint, ResourceItem, Stockpile,
};
use serde_json::{Value, json};

fn pose(world: &World, entity: Entity) -> Result<Value, String> {
    let Some(pose) = world.get::<Transform>(entity) else {
        return Ok(Value::Null);
    };
    if !pose.translation.is_finite() || !pose.rotation.is_finite() || !pose.scale.is_finite() {
        return Err("M2 inventory contains non-finite pose".into());
    }
    Ok(
        json!({"translation": pose.translation.to_array(), "rotation": pose.rotation.to_array(), "scale": pose.scale.to_array()}),
    )
}

pub(super) fn snapshot(world: &mut World) -> Result<Value, String> {
    let mut tanks = Vec::new();
    for (entity, building) in world.query::<(Entity, &Building)>().iter(world) {
        if building.kind != BuildingType::Tank {
            continue;
        }
        tanks.push(
            json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
            "provisional": building.is_provisional, "pose": pose(world, entity)?,
            "stockpile": stockpile(world, entity), "stored_items": stored_items(world, entity)?}),
        );
        if tanks.len() > 512 {
            return Err("M2 Tank inventory cap exceeded".into());
        }
    }
    let mut bucket_storages = Vec::new();
    for entity in world
        .query_filtered::<Entity, With<BucketStorage>>()
        .iter(world)
        .take(513)
    {
        bucket_storages.push(json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
            "pose": pose(world, entity)?, "belongs_to": belongs_to(world, entity),
            "pending_blueprint": world.get::<PendingBelongsToBlueprint>(entity).map(|pending| format!("{:?}", pending.0)),
            "stockpile": stockpile(world, entity), "stored_items": stored_items(world, entity)?}));
    }
    if bucket_storages.len() > 512 {
        return Err("M2 bucket storage inventory cap exceeded".into());
    }
    let mut mixers = Vec::new();
    for (entity, storage) in world
        .query::<(Entity, &MudMixerStorage)>()
        .iter(world)
        .take(513)
    {
        let stored: Vec<_> = world
            .get::<StoredItems>(entity)
            .into_iter()
            .flat_map(|items| items.iter())
            .take(4097)
            .map(|item| format!("{item:?}"))
            .collect();
        let workers: Vec<_> = world
            .get::<TaskWorkers>(entity)
            .into_iter()
            .flat_map(|workers| workers.iter())
            .take(129)
            .map(|worker| format!("{worker:?}"))
            .collect();
        if stored.len() > 4096 || workers.len() > 128 {
            return Err("M2 owner relation cap exceeded".into());
        }
        mixers.push(json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
            "pose": pose(world, entity)?, "sand": storage.sand, "rock": storage.rock, "mud": storage.mud,
            "building": world.get::<Building>(entity).map(|building| json!({"kind": format!("{:?}", building.kind), "provisional": building.is_provisional})),
            "stockpile": world.get::<Stockpile>(entity).map(|stockpile| json!({"resource": stockpile.resource_type.map(|kind| format!("{kind:?}")), "capacity": stockpile.capacity})),
            "stored_items": stored, "stored_items_present": world.get::<StoredItems>(entity).is_some(),
            "workers": workers, "task_workers_present": world.get::<TaskWorkers>(entity).is_some()}));
    }
    // Include unstored/carried items too: an absent source relation is explicit,
    // not permission to infer ownership from the nearest building or Sprite.
    let mut items = Vec::new();
    for (entity, resource) in world
        .query::<(Entity, &ResourceItem)>()
        .iter(world)
        .take(4097)
    {
        items.push(json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
            "resource": format!("{:?}", resource.0), "pose": pose(world, entity)?,
            "belongs_to": belongs_to(world, entity),
            "delivering_to": world.get::<DeliveringTo>(entity).map(|target| format!("{:?}", target.0)),
            "stored_in": world.get::<StoredIn>(entity).map(|stored| format!("{:?}", stored.0)),
            "stored_by_mixer": world.get::<StoredByMixer>(entity).map(|stored| format!("{:?}", stored.0)),
            "loaded_in": world.get::<LoadedIn>(entity).map(|loaded| format!("{:?}", loaded.0))}));
    }
    let mut bucket_actors = Vec::new();
    let mut actors = Vec::new();
    for (entity, task) in world.query::<(Entity, &AssignedTask)>().iter(world) {
        if let AssignedTask::BucketTransport(data) = task {
            let (phase, progress) = match data.phase {
                BucketTransportPhase::GoingToBucket => ("GoingToBucket", None),
                BucketTransportPhase::GoingToSource => ("GoingToSource", None),
                BucketTransportPhase::Filling { progress } => ("Filling", Some(progress)),
                BucketTransportPhase::GoingToDestination => ("GoingToDestination", None),
                BucketTransportPhase::Pouring { progress } => ("Pouring", Some(progress)),
                BucketTransportPhase::ReturningBucket => ("ReturningBucket", None),
            };
            if progress.is_some_and(|value| !value.is_finite()) {
                return Err("M2 bucket transport progress is non-finite".into());
            }
            let source = match data.source {
                BucketTransportSource::River => json!({"kind": "River"}),
                BucketTransportSource::Tank { tank, needs_fill } => json!({"kind": "Tank",
                    "entity": format!("{tank:?}"), "exists": world.get_entity(tank).is_ok(), "needs_fill": needs_fill}),
            };
            let (destination_kind, destination) = match data.destination {
                BucketTransportDestination::Tank(tank) => ("Tank", tank),
                BucketTransportDestination::Mixer(mixer) => ("Mixer", mixer),
            };
            bucket_actors.push(json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
                "is_soul": world.get::<hw_core::soul::DamnedSoul>(entity).is_some(),
                "pose": pose(world, entity)?, "phase": phase, "progress": progress, "amount": data.amount,
                "bucket": format!("{:?}", data.bucket), "bucket_exists": world.get_entity(data.bucket).is_ok(),
                "source": source, "destination": {"kind": destination_kind, "entity": format!("{destination:?}"),
                    "exists": world.get_entity(destination).is_ok()},
                "working_on": world.get::<WorkingOn>(entity).map(|working| format!("{:?}", working.0)),
                "inventory_present": world.get::<Inventory>(entity).is_some(),
                "carried_item": world.get::<Inventory>(entity).and_then(|inventory| inventory.0).map(|item| format!("{item:?}")),
                "identity": world.get::<ActiveTaskIdentity>(entity).map(|identity| json!({
                    "assignment": format!("{:?}", identity.assignment_entity), "target": format!("{:?}", identity.current_target_entity),
                    "work_type": format!("{:?}", identity.current_work_type),
                    "matches_working_on": identity.matches_working_on(world.get::<WorkingOn>(entity).map(|working| working.0))}))}));
            if bucket_actors.len() > 128 {
                return Err("M2 bucket actor cap exceeded".into());
            }
        }
        let AssignedTask::Refine(data) = task else {
            continue;
        };
        let (phase, progress) = match data.phase {
            RefinePhase::GoingToMixer => ("GoingToMixer", None),
            RefinePhase::Refining { progress } => {
                if !progress.is_finite() {
                    return Err("M2 Refine progress is non-finite".into());
                }
                ("Refining", Some(progress))
            }
            RefinePhase::Done => ("Done", None),
        };
        actors.push(json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
            "is_soul": world.get::<hw_core::soul::DamnedSoul>(entity).is_some(),
            "mixer": format!("{:?}", data.mixer), "owner_exists": world.get_entity(data.mixer).is_ok(),
            "phase": phase, "progress": progress, "pose": pose(world, entity)?,
            "working_on": world.get::<WorkingOn>(entity).map(|working| format!("{:?}", working.0)),
            "inventory_present": world.get::<Inventory>(entity).is_some(),
            "carried_item": world.get::<Inventory>(entity).and_then(|inventory| inventory.0).map(|item| format!("{item:?}")),
            "identity": world.get::<ActiveTaskIdentity>(entity).map(|identity| json!({
                "assignment": format!("{:?}", identity.assignment_entity), "target": format!("{:?}", identity.current_target_entity),
                "work_type": format!("{:?}", identity.current_work_type),
                "matches_working_on": identity.matches_working_on(world.get::<WorkingOn>(entity).map(|working| working.0))}))}));
        if actors.len() > 128 {
            return Err("M2 Refine actor cap exceeded".into());
        }
    }
    let mut working = Vec::new();
    for (entity, relation) in world.query::<(Entity, &WorkingOn)>().iter(world) {
        if world.get::<MudMixerStorage>(relation.0).is_none() {
            continue;
        }
        working.push(
            json!({"entity": format!("{entity:?}"), "owner": format!("{:?}", relation.0),
            "is_soul": world.get::<hw_core::soul::DamnedSoul>(entity).is_some(),
            "task": world.get::<AssignedTask>(entity).map(|task| format!("{task:?}"))}),
        );
        if working.len() > 128 {
            return Err("M2 working relation cap exceeded".into());
        }
    }
    if mixers.len() > 512 || items.len() > 4096 {
        return Err("M2 typed inventory cap exceeded".into());
    }
    for rows in [
        &mut mixers,
        &mut items,
        &mut actors,
        &mut working,
        &mut tanks,
        &mut bucket_storages,
        &mut bucket_actors,
    ] {
        rows.sort_by(|a, b| a["entity"].as_str().cmp(&b["entity"].as_str()));
    }
    Ok(
        json!({"schema": 1, "mixers": mixers, "items": items, "actors": actors, "working_relations": working,
        "water_inventory_schema": 1, "tanks": tanks, "bucket_storages": bucket_storages, "bucket_actors": bucket_actors,
        "water_transfer_receipts": false,
        "refine_commit_channel": world.contains_resource::<hw_soul_ai::refine_observations::RefineCommitReceipts>(),
        "accepted": false, "performance_evidence": false, "promotion_authority": false}),
    )
}

fn belongs_to(world: &World, entity: Entity) -> Option<String> {
    world
        .get::<BelongsTo>(entity)
        .map(|belongs| format!("{:?}", belongs.0))
}

fn stockpile(world: &World, entity: Entity) -> Value {
    world.get::<Stockpile>(entity).map(|stockpile| json!({
        "resource": stockpile.resource_type.map(|kind| format!("{kind:?}")), "capacity": stockpile.capacity
    })).unwrap_or(Value::Null)
}

fn stored_items(world: &World, entity: Entity) -> Result<Value, String> {
    let Some(items) = world.get::<StoredItems>(entity) else {
        return Ok(Value::Null);
    };
    let mut rows = items
        .iter()
        .take(4097)
        .map(|item| format!("{item:?}"))
        .collect::<Vec<_>>();
    if rows.len() > 4096 {
        return Err("M2 water owner relation cap exceeded".into());
    }
    rows.sort();
    Ok(json!(rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::{RefineData, WorkType};
    use hw_logistics::ResourceType;

    #[test]
    fn water_inventory_preserves_actual_tank_companions_and_bucket_payloads() {
        let mut world = World::new();
        let tank = world
            .spawn((
                Building {
                    kind: BuildingType::Tank,
                    is_provisional: false,
                },
                Stockpile {
                    resource_type: Some(ResourceType::Water),
                    capacity: 50,
                },
                Transform::default(),
            ))
            .id();
        let companion = world
            .spawn((BucketStorage, BelongsTo(tank), Transform::default()))
            .id();
        let pending = world
            .spawn((BucketStorage, PendingBelongsToBlueprint(tank)))
            .id();
        let water = world
            .spawn((ResourceItem(ResourceType::Water), StoredIn(tank)))
            .id();
        let bucket = world
            .spawn((
                ResourceItem(ResourceType::BucketWater),
                BelongsTo(tank),
                StoredIn(companion),
                DeliveringTo(tank),
            ))
            .id();
        let dead = world.spawn_empty().id();
        world.despawn(dead);
        let actor = world
            .spawn((
                AssignedTask::BucketTransport(hw_jobs::BucketTransportData {
                    bucket,
                    source: BucketTransportSource::Tank {
                        tank,
                        needs_fill: true,
                    },
                    destination: BucketTransportDestination::Mixer(dead),
                    amount: 5,
                    phase: BucketTransportPhase::Pouring { progress: 0.25 },
                }),
                Inventory(Some(bucket)),
                WorkingOn(tank),
                ActiveTaskIdentity::new(tank, tank, hw_jobs::WorkType::HaulWaterToMixer),
            ))
            .id();
        let value = snapshot(&mut world).unwrap();
        assert_eq!(value["water_inventory_schema"], 1);
        assert_eq!(value["water_transfer_receipts"], false);
        assert_eq!(
            value["tanks"][0]["stockpile"],
            json!({"resource": "Water", "capacity": 50})
        );
        assert_eq!(
            value["tanks"][0]["stored_items"],
            json!([format!("{water:?}")])
        );
        let find = |key: &str, entity: Entity| {
            value[key]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["entity"] == format!("{entity:?}"))
                .unwrap()
        };
        assert_eq!(
            find("bucket_storages", companion)["belongs_to"],
            format!("{tank:?}")
        );
        assert!(find("bucket_storages", companion)["stockpile"].is_null());
        assert_eq!(
            find("bucket_storages", companion)["stored_items"],
            json!([format!("{bucket:?}")])
        );
        assert!(find("bucket_storages", pending)["belongs_to"].is_null());
        assert!(find("bucket_storages", pending)["stored_items"].is_null());
        assert_eq!(
            find("bucket_storages", pending)["pending_blueprint"],
            format!("{tank:?}")
        );
        assert_eq!(find("items", bucket)["belongs_to"], format!("{tank:?}"));
        assert_eq!(find("items", bucket)["stored_in"], format!("{companion:?}"));
        assert_eq!(find("items", bucket)["delivering_to"], format!("{tank:?}"));
        let task = find("bucket_actors", actor);
        assert_eq!(
            task["source"],
            json!({"kind": "Tank", "entity": format!("{tank:?}"), "exists": true, "needs_fill": true})
        );
        assert_eq!(
            task["destination"],
            json!({"kind": "Mixer", "entity": format!("{dead:?}"), "exists": false})
        );
        assert_eq!(task["is_soul"], false);
        assert_eq!(task["progress"], 0.25);
        assert_eq!(task["carried_item"], format!("{bucket:?}"));
        // Observing contradictory/foreign data must not fix it or grant authority.
        assert_eq!(world.get::<StoredIn>(bucket).unwrap().0, companion);
        assert_eq!(world.get::<Inventory>(actor).unwrap().0, Some(bucket));
        assert_eq!(value["accepted"], false);
    }

    #[test]
    fn every_bucket_phase_is_typed_and_missing_relations_stay_missing() {
        let mut world = World::new();
        let absent = world.spawn_empty().id();
        world.despawn(absent);
        let actor = world.spawn(AssignedTask::None).id();
        for (phase, name, progress) in [
            (BucketTransportPhase::GoingToBucket, "GoingToBucket", None),
            (BucketTransportPhase::GoingToSource, "GoingToSource", None),
            (
                BucketTransportPhase::Filling { progress: 0.5 },
                "Filling",
                Some(0.5),
            ),
            (
                BucketTransportPhase::GoingToDestination,
                "GoingToDestination",
                None,
            ),
            (
                BucketTransportPhase::Pouring { progress: 0.75 },
                "Pouring",
                Some(0.75),
            ),
            (
                BucketTransportPhase::ReturningBucket,
                "ReturningBucket",
                None,
            ),
        ] {
            world
                .entity_mut(actor)
                .insert(AssignedTask::BucketTransport(
                    hw_jobs::BucketTransportData {
                        bucket: absent,
                        source: BucketTransportSource::River,
                        destination: BucketTransportDestination::Tank(absent),
                        amount: 0,
                        phase,
                    },
                ));
            let value = snapshot(&mut world).unwrap();
            let task = &value["bucket_actors"][0];
            assert_eq!(task["phase"], name);
            assert_eq!(task["progress"], json!(progress));
            assert_eq!(task["source"], json!({"kind": "River"}));
            assert_eq!(task["bucket_exists"], false);
            assert_eq!(task["inventory_present"], false);
            assert!(task["identity"].is_null());
            assert!(task["working_on"].is_null());
        }
        for progress in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for phase in [
                BucketTransportPhase::Filling { progress },
                BucketTransportPhase::Pouring { progress },
            ] {
                world
                    .entity_mut(actor)
                    .insert(AssignedTask::BucketTransport(
                        hw_jobs::BucketTransportData {
                            bucket: absent,
                            source: BucketTransportSource::River,
                            destination: BucketTransportDestination::Tank(absent),
                            amount: 0,
                            phase,
                        },
                    ));
                assert!(snapshot(&mut world).unwrap_err().contains("non-finite"));
            }
        }
    }

    #[test]
    fn water_inventories_fail_closed_at_each_bound() {
        for category in ["tanks", "bucket_storages", "bucket_actors"] {
            let mut world = World::new();
            let target = world.spawn_empty().id();
            let cap = if category == "bucket_actors" {
                128
            } else {
                512
            };
            for index in 0..=cap {
                match category {
                    "tanks" => {
                        world.spawn(Building {
                            kind: BuildingType::Tank,
                            is_provisional: false,
                        });
                    }
                    "bucket_storages" => {
                        world.spawn(BucketStorage);
                    }
                    _ => {
                        world.spawn(AssignedTask::BucketTransport(
                            hw_jobs::BucketTransportData {
                                bucket: target,
                                source: BucketTransportSource::River,
                                destination: BucketTransportDestination::Tank(target),
                                amount: 0,
                                phase: BucketTransportPhase::GoingToBucket,
                            },
                        ));
                    }
                }
                if index + 1 == cap {
                    assert_eq!(
                        snapshot(&mut world).unwrap()[category]
                            .as_array()
                            .unwrap()
                            .len(),
                        cap
                    );
                }
            }
            assert!(snapshot(&mut world).unwrap_err().contains("cap exceeded"));
        }
    }

    #[test]
    fn m2_inventory_keeps_water_and_mud_relations_distinct_without_seeding_or_repair() {
        let mut world = World::new();
        let owner = world
            .spawn((
                MudMixerStorage {
                    sand: 2,
                    rock: 3,
                    mud: 5,
                },
                Transform::default(),
            ))
            .id();
        let water = world
            .spawn((
                ResourceItem(ResourceType::Water),
                StoredIn(owner),
                Transform::default(),
            ))
            .id();
        let mud = world
            .spawn((ResourceItem(ResourceType::StasisMud), StoredByMixer(owner)))
            .id();
        let actor = world
            .spawn((
                AssignedTask::Refine(RefineData {
                    mixer: owner,
                    phase: RefinePhase::Refining { progress: 0.4 },
                }),
                ActiveTaskIdentity::new(owner, owner, WorkType::Refine),
                WorkingOn(owner),
            ))
            .id();
        let value = snapshot(&mut world).unwrap();
        let find = |entity: Entity| {
            value["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["entity"] == format!("{entity:?}"))
                .unwrap()
        };
        assert_eq!(find(water)["stored_in"], format!("{owner:?}"));
        assert!(find(water)["stored_by_mixer"].is_null());
        assert!(find(mud)["stored_in"].is_null());
        assert_eq!(find(mud)["stored_by_mixer"], format!("{owner:?}"));
        assert_eq!(
            value["mixers"][0]["stored_items"],
            json!([format!("{water:?}")])
        );
        assert_eq!(value["mixers"][0]["workers"], json!([format!("{actor:?}")]));
        assert_eq!(value["actors"][0]["is_soul"], false);
        assert_eq!(value["refine_commit_channel"], false);
        assert_eq!(world.get::<MudMixerStorage>(owner).unwrap().sand, 2);
        assert!(world.get::<Building>(owner).is_none());
    }

    #[test]
    fn m2_inventory_rejects_nonfinite_and_overflow_without_normalization() {
        let mut world = World::new();
        let owner = world
            .spawn((
                MudMixerStorage::default(),
                Transform::from_xyz(f32::NAN, 0., 0.),
            ))
            .id();
        assert!(snapshot(&mut world).is_err());
        world.entity_mut(owner).insert(Transform::default());
        let actor = world
            .spawn(AssignedTask::Refine(RefineData {
                mixer: owner,
                phase: RefinePhase::Refining {
                    progress: f32::INFINITY,
                },
            }))
            .id();
        assert!(snapshot(&mut world).is_err());
        world.despawn(actor);
        for _ in 0..512 {
            world.spawn(MudMixerStorage::default());
        }
        assert!(snapshot(&mut world).is_err());
    }
}
