//! Actual load-map projection for the explicit Tank persistence observer only.
//! No entity generation, task repair, acceptance or save-schema changes.
use bevy::{ecs::entity::EntityHashMap, prelude::*};
use hw_core::logistics::ResourceType;
use hw_jobs::{Building, BuildingType};
use hw_logistics::{BucketStorage, ResourceItem};
use serde_json::{Value, json};
use std::collections::HashMap;

const LIMIT: usize = 5120;

pub(super) fn mapping(
    world: &mut World,
    actual: &EntityHashMap<Entity>,
) -> Result<Vec<Value>, String> {
    let mut selected = HashMap::new();
    for (entity, building, storage, item) in world
        .query::<(
            Entity,
            Option<&Building>,
            Option<&BucketStorage>,
            Option<&ResourceItem>,
        )>()
        .iter(world)
    {
        let kind = if building.is_some_and(|b| b.kind == BuildingType::Tank) {
            "Tank"
        } else if storage.is_some() {
            "BucketStorage"
        } else {
            match item.map(|r| r.0) {
                Some(ResourceType::Water) => "Water",
                Some(ResourceType::BucketEmpty) => "BucketEmpty",
                Some(ResourceType::BucketWater) => "BucketWater",
                _ => continue,
            }
        };
        if usize::from(building.is_some())
            + usize::from(storage.is_some())
            + usize::from(item.is_some())
            != 1
        {
            return Err("durable M2 mapping has ambiguous components".into());
        }
        selected.insert(entity, kind);
        if selected.len() > LIMIT {
            return Err("durable M2 mapping inventory cap exceeded".into());
        }
    }
    let mut rows = Vec::with_capacity(selected.len());
    let mut targets = std::collections::HashSet::new();
    for (&saved, &loaded) in actual {
        let Some(kind) = selected.get(&loaded) else {
            continue;
        };
        if !targets.insert(loaded) {
            return Err("durable M2 mapping has duplicate loaded target".into());
        }
        rows.push(
            json!({"saved_entity": format!("{saved:?}"), "saved_bits": saved.to_bits(),
            "loaded_entity": format!("{loaded:?}"), "loaded_bits": loaded.to_bits(), "kind": kind}),
        );
    }
    if rows.len() != selected.len() {
        return Err("durable M2 mapping is incomplete".into());
    }
    rows.sort_by_key(|row| row["saved_bits"].as_u64());
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_actual_map_is_complete_typed_bijective_and_nonmutating() {
        let mut world = World::new();
        let tank = world
            .spawn(Building {
                kind: BuildingType::Tank,
                is_provisional: false,
            })
            .id();
        let storage = world.spawn(BucketStorage).id();
        let mut entities = vec![tank, storage];
        for kind in [
            ResourceType::Water,
            ResourceType::BucketEmpty,
            ResourceType::BucketWater,
        ] {
            entities.push(world.spawn(ResourceItem(kind)).id());
        }
        let other = world.spawn_empty().id();
        let mut actual = EntityHashMap::default();
        for entity in &entities {
            actual.insert(world.spawn_empty().id(), *entity);
        }
        actual.insert(other, other);
        let before = world.entities().len();
        let rows = mapping(&mut world, &actual).unwrap();
        assert_eq!(world.entities().len(), before);
        assert_eq!(rows.len(), 5);
        for kind in [
            "Tank",
            "BucketStorage",
            "Water",
            "BucketEmpty",
            "BucketWater",
        ] {
            assert_eq!(rows.iter().filter(|row| row["kind"] == kind).count(), 1);
        }
        assert_eq!(
            world.get::<Building>(tank).unwrap().kind,
            BuildingType::Tank
        );
        let saved_tank = actual
            .iter()
            .find(|(_, loaded)| **loaded == tank)
            .map(|(saved, _)| *saved)
            .unwrap();
        actual.remove(&saved_tank);
        assert!(
            mapping(&mut world, &actual)
                .unwrap_err()
                .contains("incomplete")
        );
    }

    #[test]
    fn duplicate_ambiguous_and_over_cap_maps_are_rejected() {
        for case in 0..3 {
            let mut world = World::new();
            let item = world.spawn(ResourceItem(ResourceType::Water)).id();
            let mut actual = EntityHashMap::from_iter([(item, item)]);
            if case == 0 {
                actual.insert(world.spawn_empty().id(), item);
            } else if case == 1 {
                world.entity_mut(item).insert(BucketStorage);
            } else {
                for _ in 0..LIMIT {
                    let entity = world.spawn(ResourceItem(ResourceType::BucketWater)).id();
                    actual.insert(entity, entity);
                }
            }
            assert!(mapping(&mut world, &actual).is_err(), "case {case}");
        }
    }
}
