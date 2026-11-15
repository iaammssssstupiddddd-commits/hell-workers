//! Passive, bounded ordinary-world rest witnesses. No state or authority is seeded.
use bevy::prelude::*;
use hw_core::relationships::{
    RestAreaOccupants, RestAreaReservations, RestAreaReservedFor, RestingIn,
};
use hw_core::soul::{DamnedSoul, DreamPool, IdleState, RestAreaCooldown};
use hw_jobs::{Building, RestArea};
use hw_world::WorldMap;
use serde_json::{Value, json};

const ROW_LIMIT: usize = 128;
const OWNER_LIMIT: usize = 512;

pub(crate) fn snapshot(world: &mut World) -> Result<Value, String> {
    let mut owners = Vec::new();
    for (entity, rest, building) in world
        .query::<(Entity, &RestArea, Option<&Building>)>()
        .iter(world)
        .take(OWNER_LIMIT + 1)
    {
        let occupants = world.get::<RestAreaOccupants>(entity).map(|sources| {
            sources
                .iter()
                .take(ROW_LIMIT + 1)
                .map(|soul| format!("{soul:?}"))
                .collect::<Vec<_>>()
        });
        let reservations = world.get::<RestAreaReservations>(entity).map(|sources| {
            sources
                .iter()
                .take(ROW_LIMIT + 1)
                .map(|soul| format!("{soul:?}"))
                .collect::<Vec<_>>()
        });
        if occupants
            .as_ref()
            .is_some_and(|rows| rows.len() > ROW_LIMIT)
            || reservations
                .as_ref()
                .is_some_and(|rows| rows.len() > ROW_LIMIT)
        {
            return Err("rest observation relationship cap exceeded".into());
        }
        owners.push(json!({
            "entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
            "capacity": rest.capacity, "occupants": occupants, "reservations": reservations,
            "building_kind": building.map(|building| format!("{:?}", building.kind)),
            "provisional": building.map(|building| building.is_provisional),
        }));
    }
    let mut souls = Vec::new();
    for (entity, soul) in world
        .query::<(Entity, &DamnedSoul)>()
        .iter(world)
        .take(ROW_LIMIT + 1)
    {
        let idle = world.get::<IdleState>(entity);
        let cooldown = world.get::<RestAreaCooldown>(entity);
        let pose = world.get::<Transform>(entity);
        if ![soul.dream, soul.fatigue, soul.stress]
            .into_iter()
            .all(f32::is_finite)
            || idle.is_some_and(|idle| {
                !idle.idle_timer.is_finite() || !idle.total_idle_time.is_finite()
            })
            || cooldown.is_some_and(|cooldown| !cooldown.remaining_secs.is_finite())
            || pose.is_some_and(|pose| !pose.translation.is_finite())
        {
            return Err("rest observation contains non-finite Soul state".into());
        }
        souls.push(json!({
            "entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
            "dream": soul.dream, "fatigue": soul.fatigue, "stress": soul.stress,
            "idle": idle.map(|idle| json!({"behavior": format!("{:?}", idle.behavior),
                "timer": idle.idle_timer, "total_idle_time": idle.total_idle_time})),
            "visibility": world.get::<Visibility>(entity).map(|visibility| format!("{visibility:?}")),
            "position": pose.map(|pose| pose.translation.to_array()),
            "walkable": pose.and_then(|pose| world.get_resource::<WorldMap>().map(|map| {
                let (x, y) = WorldMap::world_to_grid(pose.translation.truncate());
                map.is_walkable(x, y)
            })),
            "resting_in": world.get::<RestingIn>(entity).map(|target| format!("{:?}", target.0)),
            "reserved_for": world.get::<RestAreaReservedFor>(entity).map(|target| format!("{:?}", target.0)),
            "cooldown": cooldown.map(|cooldown| cooldown.remaining_secs),
        }));
    }
    if owners.len() > OWNER_LIMIT || souls.len() > ROW_LIMIT {
        return Err("rest observation entity cap exceeded".into());
    }
    let dream_pool = world.get_resource::<DreamPool>().map(|pool| pool.points);
    if dream_pool.is_some_and(|points| !points.is_finite()) {
        return Err("rest observation contains non-finite DreamPool".into());
    }
    Ok(
        json!({"owners": owners, "souls": souls, "dream_pool": dream_pool,
        "real_seconds": world.get_resource::<Time<Real>>().map(Time::elapsed_secs_f64),
        "virtual_seconds": world.get_resource::<Time<Virtual>>().map(Time::elapsed_secs_f64)}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::soul::IdleBehavior;
    use hw_jobs::BuildingType;

    #[test]
    fn rest_snapshot_reads_actual_relationships_and_vitals_without_mutation() {
        let mut world = World::new();
        world.insert_resource(DreamPool { points: 7.0 });
        let owner = world
            .spawn((
                RestArea { capacity: 4 },
                Building {
                    kind: BuildingType::RestArea,
                    is_provisional: false,
                },
            ))
            .id();
        let soul = world
            .spawn((
                DamnedSoul {
                    dream: 3.0,
                    fatigue: 0.4,
                    stress: 0.2,
                    ..default()
                },
                IdleState {
                    behavior: IdleBehavior::Resting,
                    ..default()
                },
                Visibility::Hidden,
                Transform::from_xyz(1.0, 2.0, 0.0),
                RestingIn(owner),
            ))
            .id();
        let reserved = world
            .spawn((DamnedSoul::default(), RestAreaReservedFor(owner)))
            .id();
        let before = snapshot(&mut world).unwrap();
        assert_eq!(
            before["owners"][0]["occupants"],
            json!([format!("{soul:?}")])
        );
        assert_eq!(
            before["owners"][0]["reservations"],
            json!([format!("{reserved:?}")])
        );
        let row = before["souls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["entity"] == format!("{soul:?}"))
            .unwrap();
        assert_eq!(row["resting_in"], format!("{owner:?}"));
        assert_eq!(row["dream"], 3.0);
        assert_eq!(row["visibility"], "Hidden");
        assert_eq!(before["dream_pool"], 7.0);
        assert_eq!(snapshot(&mut world).unwrap(), before);
        assert_eq!(world.get::<DamnedSoul>(soul).unwrap().dream, 3.0);
        assert_eq!(
            world.get::<IdleState>(soul).unwrap().behavior,
            IdleBehavior::Resting
        );
        assert_eq!(*world.get::<Visibility>(soul).unwrap(), Visibility::Hidden);
        assert_eq!(
            world.get::<Transform>(soul).unwrap().translation,
            Vec3::new(1.0, 2.0, 0.0)
        );
        world.entity_mut(soul).remove::<RestingIn>();
        let after = snapshot(&mut world).unwrap();
        assert!(after["owners"][0]["occupants"].is_null());
        assert_eq!(world.resource::<DreamPool>().points, 7.0);
    }

    #[test]
    fn missing_rest_fields_are_null_not_inferred_outcomes() {
        let mut world = World::new();
        world.spawn(DamnedSoul::default());
        let value = snapshot(&mut world).unwrap();
        assert!(value["dream_pool"].is_null());
        for field in [
            "idle",
            "visibility",
            "position",
            "walkable",
            "resting_in",
            "reserved_for",
            "cooldown",
        ] {
            assert!(value["souls"][0][field].is_null(), "{field}");
        }
    }

    #[test]
    fn rest_snapshot_rejects_overflow_and_non_finite_values() {
        let mut world = World::new();
        for _ in 0..=ROW_LIMIT {
            world.spawn(DamnedSoul::default());
        }
        assert!(snapshot(&mut world).unwrap_err().contains("entity cap"));
        let mut world = World::new();
        for _ in 0..=OWNER_LIMIT {
            world.spawn(RestArea { capacity: 4 });
        }
        assert!(snapshot(&mut world).unwrap_err().contains("entity cap"));
        let mut world = World::new();
        let owner = world.spawn(RestArea { capacity: 4 }).id();
        for _ in 0..=ROW_LIMIT {
            world.spawn(RestAreaReservedFor(owner));
        }
        assert!(
            snapshot(&mut world)
                .unwrap_err()
                .contains("relationship cap")
        );
        let mut world = World::new();
        world.spawn(DamnedSoul {
            dream: f32::NAN,
            ..default()
        });
        assert!(snapshot(&mut world).unwrap_err().contains("non-finite"));
        let mut world = World::new();
        world.insert_resource(DreamPool {
            points: f32::INFINITY,
        });
        assert!(snapshot(&mut world).unwrap_err().contains("DreamPool"));
    }
}
