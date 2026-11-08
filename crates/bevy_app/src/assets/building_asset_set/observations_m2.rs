//! Passive typed Mixer/resource/task inventory, never a lifecycle driver.
use bevy::prelude::*;
use hw_core::relationships::{LoadedIn, StoredIn, StoredItems, TaskWorkers, WorkingOn};
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, Building, RefinePhase, StoredByMixer,
    mud_mixer::MudMixerStorage,
};
use hw_logistics::{Inventory, ResourceItem, Stockpile};
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
            "stored_in": world.get::<StoredIn>(entity).map(|stored| format!("{:?}", stored.0)),
            "stored_by_mixer": world.get::<StoredByMixer>(entity).map(|stored| format!("{:?}", stored.0)),
            "loaded_in": world.get::<LoadedIn>(entity).map(|loaded| format!("{:?}", loaded.0))}));
    }
    let mut actors = Vec::new();
    for (entity, task) in world.query::<(Entity, &AssignedTask)>().iter(world) {
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
    for rows in [&mut mixers, &mut items, &mut actors, &mut working] {
        rows.sort_by(|a, b| a["entity"].as_str().cmp(&b["entity"].as_str()));
    }
    Ok(
        json!({"schema": 1, "mixers": mixers, "items": items, "actors": actors, "working_relations": working,
        "refine_commit_channel": world.contains_resource::<hw_soul_ai::refine_observations::RefineCommitReceipts>(),
        "accepted": false, "performance_evidence": false, "promotion_authority": false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::{RefineData, WorkType};
    use hw_logistics::ResourceType;

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
