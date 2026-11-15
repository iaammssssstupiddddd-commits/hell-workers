//! Read-only, explicit natural-world prerequisite for the supply-only campaign.
use crate::world::map::GeneratedWorldLayoutResource;
use bevy::prelude::*;
use hw_infra::lighting::digest_hex;
use hw_world::{GeneratedWorldLayout, WalkabilityConnectivityCache, WorldMap, Yard};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Resource)]
struct Observation(Value);

fn inspect(
    layout: &GeneratedWorldLayout,
    map: &WorldMap,
    yard: &Yard,
    tank: (i32, i32),
    mixer: (i32, i32),
) -> Value {
    assert!(
        layout.initial_tree_positions.len() <= 128 && layout.initial_rock_positions.len() <= 128
    );
    let payload = json!({"seed":layout.master_seed,"attempt":layout.generation_attempt,
        "fallback":layout.used_fallback,"terrain":layout.terrain_tiles,
        "trees":layout.initial_tree_positions,"rocks":layout.initial_rock_positions,
        "site":[layout.anchors.site.min_x,layout.anchors.site.min_y,layout.anchors.site.max_x,layout.anchors.site.max_y],
        "yard":[layout.anchors.yard.min_x,layout.anchors.yard.min_y,layout.anchors.yard.max_x,layout.anchors.yard.max_y],
        "wood":layout.anchors.initial_wood_positions,
        "parking":layout.anchors.wheelbarrow_parking.iter_cells().collect::<Vec<_>>()});
    let digest =
        digest_hex(&Sha256::digest(serde_json::to_vec(&payload).expect("layout JSON")).into());
    // A private walkability projection, NOT a mutation of the actual world.
    let mut projected = WorldMap {
        tiles: map.tiles.clone(),
        obstacles: map.obstacles.clone(),
        buildings: map.buildings.clone(),
        stockpiles: map.stockpiles.clone(),
        doors: map.doors.clone(),
        door_states: map.door_states.clone(),
        bridged_tiles: map.bridged_tiles.clone(),
        ..Default::default()
    };
    let mut cache = WalkabilityConnectivityCache::default();
    let mut stages = Vec::new();
    for phase in 0..3 {
        if phase > 0 {
            let anchor = if phase == 1 { tank } else { mixer };
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                projected
                    .set_building_occupancy((anchor.0 + dx, anchor.1 + dy), Entity::PLACEHOLDER);
            }
            if phase == 1 {
                for dx in [2, 3] {
                    projected.register_stockpile_tile((tank.0 + dx, tank.1), Entity::PLACEHOLDER);
                }
            }
        }
        let start = projected
            .get_nearest_walkable_grid((yard.min + yard.max) * 0.5)
            .expect("walkable Yard start");
        let mut count = |sources: &[(i32, i32)]| {
            sources
                .iter()
                .filter(|&&target| cache.can_reach_target(&projected, start, target, false))
                .count()
        };
        stages.push(json!({"phase":phase,"start":start,
            "trees":count(&layout.initial_tree_positions),"rocks":count(&layout.initial_rock_positions)}));
    }
    json!({"seed":layout.master_seed,"attempt":layout.generation_attempt,"fallback":layout.used_fallback,
        "layout_sha256":digest,"projected_routes":stages})
}

pub(super) fn prepare(world: &mut World, tank: (i32, i32), mixer: (i32, i32), yards: &[Yard]) {
    let generated = world.resource::<GeneratedWorldLayoutResource>();
    assert_eq!(generated.master_seed, generated.layout.master_seed);
    assert_eq!(
        generated.master_seed, 42,
        "dedicated supply natural-world seed must be explicit"
    );
    let center =
        WorldMap::grid_to_world(tank.0, tank.1) + Vec2::splat(hw_core::constants::TILE_SIZE * 0.5);
    let owners: Vec<_> = yards.iter().filter(|yard| yard.contains(center)).collect();
    assert_eq!(owners.len(), 1, "supply requires one actual Yard owner");
    let value = inspect(
        &generated.layout,
        world.resource::<WorldMap>(),
        owners[0],
        tank,
        mixer,
    );
    assert!(
        value["projected_routes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["trees"].as_u64().unwrap() > 0 && row["rocks"].as_u64().unwrap() > 0),
        "natural supply world must have reachable Tree and Rock before and after planned footprints"
    );
    world.insert_resource(Observation(value));
}

pub(super) fn snapshot(world: &World) -> Value {
    world
        .get_resource::<Observation>()
        .map_or(Value::Null, |value| value.0.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supply_seed_is_natural_reachable_and_does_not_change_the_map() {
        for (seed, reachable) in [(20260914, false), (42, true)] {
            let layout = hw_world::generate_world_layout(seed);
            let mut map = WorldMap {
                tiles: layout.terrain_tiles.clone(),
                ..Default::default()
            };
            for &grid in layout
                .initial_tree_positions
                .iter()
                .chain(&layout.initial_rock_positions)
            {
                if map
                    .pos_to_idx(grid.0, grid.1)
                    .and_then(|index| map.terrain_at_idx(index))
                    .is_some_and(|terrain| terrain.is_walkable())
                {
                    map.add_grid_obstacle(grid);
                }
            }
            map.register_completed_building_footprint(
                hw_jobs::BuildingType::WheelbarrowParking,
                Entity::PLACEHOLDER,
                layout.anchors.wheelbarrow_parking.iter_cells(),
            );
            let version = map.obstacle_version;
            let obstacles = map.obstacles.clone();
            let buildings = map.buildings.clone();
            let yard = Yard {
                min: WorldMap::grid_to_world(layout.anchors.yard.min_x, layout.anchors.yard.min_y),
                max: WorldMap::grid_to_world(layout.anchors.yard.max_x, layout.anchors.yard.max_y),
            };
            let value = inspect(&layout, &map, &yard, (70, 50), (70, 47));
            println!("SUPPLY_LAYOUT {value}");
            assert!(
                value["projected_routes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|row| (row["trees"].as_u64().unwrap() > 0) == reachable
                        && row["rocks"].as_u64().unwrap() > 0)
            );
            assert_eq!(map.obstacle_version, version);
            assert_eq!(map.obstacles, obstacles);
            assert_eq!(map.buildings, buildings);
        }
    }
}
