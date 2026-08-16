use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_infra::lighting::{FixtureMount, LightGridPos};
use hw_jobs::{Building, BuildingType, ProvisionalWall};
use hw_world::WorldMap;

use super::{
    IndoorLightRuntime, IndoorLightingAllocationProbe, IndoorLightingDirty,
    IndoorLightingLifecycleProbe, LightingFixtureMount, RadialLightEmitter,
};

pub(crate) fn validate_fixture_mount_candidate(candidate: &World) -> Result<(), String> {
    let map = candidate
        .get_resource::<WorldMap>()
        .ok_or_else(|| "persisted WorldMap is missing".to_owned())?;

    for entity_ref in candidate.iter_entities() {
        let Some(adapter) = entity_ref.get::<LightingFixtureMount>() else {
            continue;
        };
        let building = entity_ref.get::<Building>().ok_or_else(|| {
            format!(
                "LightingFixtureMount owner {:?} has no Building component",
                entity_ref.id()
            )
        })?;
        if building.kind != BuildingType::OutdoorLamp || building.is_provisional {
            return Err(format!(
                "LightingFixtureMount owner {:?} is not a completed OutdoorLamp",
                entity_ref.id()
            ));
        }
        let transform = entity_ref.get::<Transform>().ok_or_else(|| {
            format!(
                "LightingFixtureMount owner {:?} has no Transform",
                entity_ref.id()
            )
        })?;
        let transform_grid = WorldMap::world_to_grid(transform.translation.truncate());
        let origin = adapter.mount().origin();
        validate_grid("fixture origin", origin)?;
        if transform_grid != origin.into_core() {
            return Err(format!(
                "LightingFixtureMount owner {:?} transform grid {:?} disagrees with mount origin ({}, {})",
                entity_ref.id(),
                transform_grid,
                origin.x,
                origin.y
            ));
        }
        if map.building_entity(transform_grid) != Some(entity_ref.id()) {
            return Err(format!(
                "LightingFixtureMount owner {:?} is not the WorldMap building owner at {:?}",
                entity_ref.id(),
                transform_grid
            ));
        }

        let FixtureMount::WallMounted { anchor, .. } = adapter.mount() else {
            continue;
        };
        validate_grid("wall anchor", anchor)?;
        let anchor_grid = anchor.into_core();
        let anchor_entity = map.building_entity(anchor_grid).ok_or_else(|| {
            format!(
                "wall-mounted LightingFixtureMount owner {:?} has no anchor at {:?}",
                entity_ref.id(),
                anchor_grid
            )
        })?;
        let anchor_ref = candidate.get_entity(anchor_entity).map_err(|_| {
            format!(
                "wall-mounted LightingFixtureMount owner {:?} references missing anchor {anchor_entity:?}",
                entity_ref.id()
            )
        })?;
        if anchor_ref.get::<Building>().is_none_or(|anchor_building| {
            anchor_building.kind != BuildingType::Wall || anchor_building.is_provisional
        }) || anchor_ref.contains::<ProvisionalWall>()
        {
            return Err(format!(
                "wall-mounted LightingFixtureMount owner {:?} anchor {anchor_entity:?} is not a completed Wall",
                entity_ref.id()
            ));
        }
    }

    Ok(())
}

fn validate_grid(label: &str, grid: LightGridPos) -> Result<(), String> {
    if (0..MAP_WIDTH).contains(&grid.x) && (0..MAP_HEIGHT).contains(&grid.y) {
        Ok(())
    } else {
        Err(format!(
            "{label} ({}, {}) is outside the world map",
            grid.x, grid.y
        ))
    }
}

pub(crate) fn normalize_lighting_mounts(world: &mut World) {
    let missing: Vec<_> = {
        let mut query =
            world.query::<(Entity, &Building, &Transform, Option<&LightingFixtureMount>)>();
        query
            .iter(world)
            .filter(|(_, building, _, mount)| {
                building.kind == BuildingType::OutdoorLamp
                    && !building.is_provisional
                    && mount.is_none()
            })
            .map(|(entity, _, transform, _)| {
                let grid = WorldMap::world_to_grid(transform.translation.truncate());
                (entity, LightingFixtureMount::free_standing(grid))
            })
            .collect()
    };
    for (entity, mount) in missing {
        world.entity_mut(entity).insert(mount);
    }
}

pub(crate) fn rebuild_lighting_emitters(world: &mut World) {
    let emitters: Vec<_> = {
        let mut query = world.query::<(Entity, &Building, &LightingFixtureMount)>();
        query
            .iter(world)
            .filter_map(|(entity, building, mount)| {
                (building.kind == BuildingType::OutdoorLamp && !building.is_provisional).then_some(
                    (
                        entity,
                        RadialLightEmitter::outdoor_lamp_at_mount(mount.mount()),
                    ),
                )
            })
            .collect()
    };
    for (entity, emitter) in emitters {
        world.entity_mut(entity).insert(emitter);
    }
}

pub(crate) fn wake_indoor_lighting(world: &mut World) {
    if let Some(mut dirty) = world.get_resource_mut::<IndoorLightingDirty>() {
        dirty.request_full_rebuild();
    }
    if let Some(mut probe) = world.get_resource_mut::<IndoorLightingLifecycleProbe>() {
        probe.record_wake();
    }
}

pub(crate) fn reset_indoor_lighting_for_world_replace(world: &mut World) {
    if let Some(mut runtime) = world.get_resource_mut::<IndoorLightRuntime>() {
        runtime.reset_for_world_replace();
    }
    if let Some(mut dirty) = world.get_resource_mut::<IndoorLightingDirty>() {
        dirty.reset_for_world_replace();
    }
    if world.contains_resource::<IndoorLightingAllocationProbe>() {
        world.insert_resource(IndoorLightingAllocationProbe::default());
    }
    let fail_dark = world
        .get_resource::<IndoorLightRuntime>()
        .is_none_or(IndoorLightRuntime::is_fail_dark);
    if let Some(mut probe) = world.get_resource_mut::<IndoorLightingLifecycleProbe>() {
        probe.record_reset(fail_dark);
    }
}
