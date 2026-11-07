//! Passive, bounded SoulSpa domain witnesses; no seeding, normalization or authority.
use bevy::{ecs::message::MessageCursor, prelude::*};
use hw_core::{
    relationships::{TaskWorkers, WorkingOn},
    soul::DamnedSoul,
};
use hw_energy::{
    GeneratesFor, GridGenerators, PowerGenerator, PowerGrid, SoulSpaConstructionCancelOutcome,
    SoulSpaConstructionCancelResult, SoulSpaSite, SoulSpaSlotsChangeOutcome,
    SoulSpaSlotsChangeStatus, SoulSpaTile,
};
use hw_jobs::{AssignedTask, Building, Designation, TaskSlots};
use hw_world::WorldMap;
use serde_json::{Value, json};

const SITE_LIMIT: usize = 512;
const TILE_LIMIT: usize = SITE_LIMIT * 4;
const SOUL_LIMIT: usize = 128;

fn pose(world: &World, entity: Entity) -> Result<Value, String> {
    let Some(pose) = world.get::<Transform>(entity) else {
        return Ok(Value::Null);
    };
    if !pose.translation.is_finite() || !pose.rotation.is_finite() || !pose.scale.is_finite() {
        return Err("Spa observation contains non-finite pose".into());
    }
    Ok(
        json!({"translation": pose.translation.to_array(), "rotation": pose.rotation.to_array(), "scale": pose.scale.to_array()}),
    )
}

pub(crate) fn snapshot(world: &mut World) -> Result<Value, String> {
    let mut sites = Vec::new();
    for (entity, site) in world
        .query::<(Entity, &SoulSpaSite)>()
        .iter(world)
        .take(SITE_LIMIT + 1)
    {
        let generator = world.get::<PowerGenerator>(entity);
        if generator
            .is_some_and(|g| !g.current_output.is_finite() || !g.output_per_soul.is_finite())
        {
            return Err("Spa observation contains non-finite generator".into());
        }
        let building = world.get::<Building>(entity);
        let layers = world
            .get_resource::<WorldMap>()
            .map(|map| map.snapshot_owner(entity));
        sites.push(json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
            "phase": format!("{:?}", site.phase), "bones_required": site.bones_required,
            "bones_delivered": site.bones_delivered, "active_slots": site.active_slots,
            "building_kind": building.map(|b| format!("{:?}", b.kind)), "provisional": building.map(|b| b.is_provisional),
            "pose": pose(world, entity)?,
            "generator": generator.map(|g| json!({"current_output": g.current_output, "output_per_soul": g.output_per_soul})),
            "grid": world.get::<GeneratesFor>(entity).map(|g| format!("{:?}", g.0)),
            "map_building_grids": layers.map(|layers| layers.building_grids)}));
    }
    let mut tiles = Vec::new();
    for (entity, tile) in world
        .query::<(Entity, &SoulSpaTile)>()
        .iter(world)
        .take(TILE_LIMIT + 1)
    {
        let workers = world.get::<TaskWorkers>(entity).map(|workers| {
            workers
                .iter()
                .take(SOUL_LIMIT + 1)
                .map(|worker| format!("{worker:?}"))
                .collect::<Vec<_>>()
        });
        if workers
            .as_ref()
            .is_some_and(|workers| workers.len() > SOUL_LIMIT)
        {
            return Err("Spa observation worker cap exceeded".into());
        }
        let map = world.get_resource::<WorldMap>();
        tiles.push(json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
            "parent_site": format!("{:?}", tile.parent_site), "parent_exists": world.get::<SoulSpaSite>(tile.parent_site).is_some(),
            "grid": tile.grid_pos, "pose": pose(world, entity)?, "workers": workers,
            "designation": world.get::<Designation>(entity).map(|d| format!("{:?}", d.work_type)),
            "task_slots": world.get::<TaskSlots>(entity).map(|slots| slots.max),
            "map": map.map(|map| json!({"owner": map.building_entity(tile.grid_pos).map(|owner| format!("{owner:?}")),
                "walkable": map.is_walkable(tile.grid_pos.0, tile.grid_pos.1)}))}));
    }
    let mut souls = Vec::new();
    for (entity, soul) in world
        .query::<(Entity, &DamnedSoul)>()
        .iter(world)
        .take(SOUL_LIMIT + 1)
    {
        if ![soul.dream, soul.fatigue, soul.stress]
            .into_iter()
            .all(f32::is_finite)
        {
            return Err("Spa observation contains non-finite Soul state".into());
        }
        let task = world.get::<AssignedTask>(entity);
        let generating = if let Some(AssignedTask::GeneratePower(task)) = task {
            if !task.tile_pos.is_finite() {
                return Err("Spa observation contains non-finite task position".into());
            }
            Some(
                json!({"tile": format!("{:?}", task.tile), "tile_pos": task.tile_pos.to_array(), "phase": format!("{:?}", task.phase)}),
            )
        } else {
            None
        };
        souls.push(json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(),
            "dream": soul.dream, "fatigue": soul.fatigue, "stress": soul.stress, "pose": pose(world, entity)?,
            "working_on": world.get::<WorkingOn>(entity).map(|working| format!("{:?}", working.0)),
            "task": task.map(|task| format!("{task:?}")), "generate_power": generating}));
    }
    // Keep the full source relation inventory, including malformed/non-Soul workers.
    // Consumers must compare it with TaskWorkers; absence never proves cleanup.
    let mut working = Vec::new();
    for (entity, target) in world
        .query::<(Entity, &WorkingOn)>()
        .iter(world)
        .take(SOUL_LIMIT + 1)
    {
        working.push(json!({"entity": format!("{entity:?}"), "target": format!("{:?}", target.0),
            "is_soul": world.get::<DamnedSoul>(entity).is_some(), "target_is_spa_tile": world.get::<SoulSpaTile>(target.0).is_some()}));
    }
    let mut grids = Vec::new();
    for (entity, grid) in world
        .query::<(Entity, &PowerGrid)>()
        .iter(world)
        .take(SITE_LIMIT + 1)
    {
        if !grid.generation.is_finite() || !grid.consumption.is_finite() {
            return Err("Spa observation contains non-finite grid".into());
        }
        let generators = world.get::<GridGenerators>(entity).map(|generators| {
            generators
                .iter()
                .take(SITE_LIMIT + 1)
                .map(|owner| format!("{owner:?}"))
                .collect::<Vec<_>>()
        });
        if generators
            .as_ref()
            .is_some_and(|owners| owners.len() > SITE_LIMIT)
        {
            return Err("Spa observation grid generator cap exceeded".into());
        }
        grids.push(
            json!({"entity": format!("{entity:?}"), "entity_bits": entity.to_bits(), "generation": grid.generation,
            "consumption": grid.consumption, "powered": grid.powered, "generators": generators}),
        );
    }
    if sites.len() > SITE_LIMIT
        || tiles.len() > TILE_LIMIT
        || souls.len() > SOUL_LIMIT
        || working.len() > SOUL_LIMIT
        || grids.len() > SITE_LIMIT
    {
        return Err("Spa observation entity cap exceeded".into());
    }
    for rows in [&mut sites, &mut tiles, &mut souls, &mut working, &mut grids] {
        rows.sort_by(|a, b| a["entity"].as_str().cmp(&b["entity"].as_str()));
    }
    Ok(
        json!({"sites": sites, "tiles": tiles, "souls": souls, "working_relations": working, "grids": grids,
        "outcome_channels": {
            "construction_cancel": world.contains_resource::<Messages<SoulSpaConstructionCancelOutcome>>(),
            "slots_change": world.contains_resource::<Messages<SoulSpaSlotsChangeOutcome>>()}}),
    )
}

#[derive(Default)]
pub(super) struct Events {
    cancellations: MessageCursor<SoulSpaConstructionCancelOutcome>,
    slots: MessageCursor<SoulSpaSlotsChangeOutcome>,
}

impl Events {
    // Timestamp/index identify this observer read, not domain apply time or input causality.
    pub(super) fn read(
        &mut self,
        world: &World,
        index: usize,
        now: f64,
    ) -> Result<Vec<Value>, String> {
        let mut rows = Vec::new();
        if let Some(messages) = world.get_resource::<Messages<SoulSpaConstructionCancelOutcome>>() {
            for event in self.cancellations.read(messages).take(4097) {
                let refunded = match event.result {
                    SoulSpaConstructionCancelResult::Canceled { refunded_bones } => {
                        Some(refunded_bones)
                    }
                    _ => None,
                };
                rows.push(json!({"producer": "SoulSpaConstructionCancelOutcome", "operation": "SoulSpaConstructionCancel",
                    "owner": format!("{:?}", event.target), "result": format!("{:?}", event.result),
                    "refunded_bones": refunded, "sample_index": index, "real_seconds": now}));
            }
        }
        if let Some(messages) = world.get_resource::<Messages<SoulSpaSlotsChangeOutcome>>() {
            for event in self.slots.read(messages).take(4097) {
                let applied = match event.status {
                    SoulSpaSlotsChangeStatus::Applied {
                        requested,
                        applied,
                        clamped,
                    } => Some(
                        json!({"requested": requested, "applied": applied, "clamped": clamped}),
                    ),
                    _ => None,
                };
                rows.push(json!({"producer": "SoulSpaSlotsChangeOutcome", "operation": "SoulSpaSlotsChange",
                    "owner": format!("{:?}", event.target), "result": format!("{:?}", event.status),
                    "applied": applied, "sample_index": index, "real_seconds": now}));
            }
        }
        if rows.len() > 4096 {
            return Err("Spa observation event cap exceeded".into());
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests;
