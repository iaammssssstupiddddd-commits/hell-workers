//! Passive target and inventory for ordinary floor demand in the supply campaign.
use super::*;
use crate::interface::selection::placement_geometry::{
    bucket_storage_geometry, live_building_geometry,
};
use crate::world::map::WorldMapRef;
use hw_core::{
    area::TaskArea,
    constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE},
    familiar::{ActiveCommand, Familiar},
};
use hw_jobs::{
    Building, BuildingType,
    construction::{FloorConstructionSite, FloorTileBlueprint},
};
use hw_world::{WorldMap, Yard};
use std::collections::HashSet;

#[derive(Resource)]
struct Target((i32, i32));

pub(crate) fn prepared(world: &World) -> bool {
    world.contains_resource::<Target>()
}

const COLUMNS: i32 = 9;
const ROWS: i32 = 5;

fn bounds(anchor: (i32, i32)) -> TaskArea {
    let half = Vec2::splat(TILE_SIZE * 0.5);
    TaskArea::from_points(
        WorldMap::grid_to_world(anchor.0, anchor.1) - half,
        WorldMap::grid_to_world(anchor.0 + COLUMNS - 1, anchor.1 + ROWS - 1) + half,
    )
}

fn cells(anchor: (i32, i32)) -> Vec<(i32, i32)> {
    (0..ROWS)
        .flat_map(|dy| (0..COLUMNS).map(move |dx| (anchor.0 + dx, anchor.1 + dy)))
        .collect()
}

fn choose(
    map: &WorldMap,
    yards: &[Yard],
    reserved: &[(i32, i32)],
    mixer: (i32, i32),
) -> Option<(i32, i32)> {
    let empty = HashSet::new();
    (1..MAP_WIDTH - COLUMNS)
        .flat_map(|x| (1..MAP_HEIGHT - ROWS).map(move |y| (x, y)))
        .filter(|&(x, y)| (x - mixer.0).abs() + (y - mixer.1).abs() <= 12)
        .filter(|&anchor| {
            let plan = hw_ui::selection::build_area_placement_plan(
                anchor,
                (anchor.0 + COLUMNS - 1, anchor.1 + ROWS - 1),
                hw_ui::selection::validate_area_size(COLUMNS, ROWS),
                |grid| {
                    hw_ui::selection::validate_floor_tile(&WorldMapRef(map), grid, &empty, &empty)
                },
            );
            plan.total_tile_count == 45
                && plan.valid_tiles.len() == 45
                && plan.first_reject.is_none()
                && cells(anchor).iter().all(|&grid| {
                    map.is_walkable(grid.0, grid.1)
                        && yards
                            .iter()
                            .any(|yard| yard.contains(WorldMap::grid_to_world(grid.0, grid.1)))
                        && reserved
                            .iter()
                            .all(|r| (r.0 - grid.0).abs().max((r.1 - grid.1).abs()) >= 2)
                })
        })
        .min_by_key(|&(x, y)| ((x - mixer.0).abs() + (y - mixer.1).abs(), x, y))
}

pub(super) fn prepare(world: &mut World, tank: (i32, i32), mixer: (i32, i32), yards: &[Yard]) {
    assert!(
        world
            .query::<&FloorTileBlueprint>()
            .iter(world)
            .next()
            .is_none()
    );
    assert!(
        !world
            .query::<&Building>()
            .iter(world)
            .any(|b| b.kind == BuildingType::Floor)
    );
    let map = world.resource::<WorldMap>();
    let mut reserved = live_building_geometry(map, BuildingType::Tank, tank).occupied_grids;
    reserved.extend(live_building_geometry(map, BuildingType::MudMixer, mixer).occupied_grids);
    reserved.extend(bucket_storage_geometry((tank.0 + 2, tank.1)).occupied_grids);
    let owners = yards
        .iter()
        .filter(|yard| {
            yard.contains(WorldMap::grid_to_world(tank.0, tank.1))
                && yard.contains(WorldMap::grid_to_world(mixer.0, mixer.1))
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        owners.len(),
        1,
        "supply workload requires the same unique Yard"
    );
    let anchor = choose(map, &owners, &reserved, mixer)
        .expect("supply needs a legal forty-five-tile ordinary floor target");
    world.insert_resource(Target(anchor));
}

pub(crate) fn raw(world: &mut World) -> Value {
    let Some(anchor) = world.get_resource::<Target>().map(|t| t.0) else {
        return Value::Null;
    };
    let area = bounds(anchor);
    let mut sites = world
        .query::<(Entity, &FloorConstructionSite)>()
        .iter(world)
        .take(129)
        .map(|(e, s)| {
            json!({"entity":format!("{e:?}"),"phase":format!("{:?}",s.phase),
            "min":s.area_bounds.min().to_array(),"max":s.area_bounds.max().to_array(),
            "material_center":s.material_center.to_array(),"total":s.tiles_total,
            "reinforced":s.tiles_reinforced,"poured":s.tiles_poured})
        })
        .collect::<Vec<_>>();
    let mut tiles = world.query::<(Entity,&FloorTileBlueprint)>().iter(world).take(129)
        .map(|(e,t)|json!({"entity":format!("{e:?}"),"site":format!("{:?}",t.parent_site),
            "grid":t.grid_pos,"state":format!("{:?}",t.state),"bones":t.bones_delivered,"mud":t.mud_delivered})).collect::<Vec<_>>();
    let mut familiars = world
        .query::<(Entity, &Familiar, &ActiveCommand, Option<&TaskArea>)>()
        .iter(world)
        .take(129)
        .map(|(e, _, c, a)| {
            json!({"entity":format!("{e:?}"),"bits":e.to_bits(),"command":format!("{:?}",c.command),
            "area":a.map(|a|json!({"min":a.min().to_array(),"max":a.max().to_array()}))})
        })
        .collect::<Vec<_>>();
    for rows in [&mut sites, &mut tiles, &mut familiars] {
        rows.sort_by(|a, b| a["entity"].as_str().cmp(&b["entity"].as_str()));
    }
    let completed = cells(anchor)
        .into_iter()
        .filter_map(|grid| {
            world
                .resource::<WorldMap>()
                .floor_entity(grid)
                .map(|e| json!({"grid":grid,"entity":format!("{e:?}"),
                    "valid":world.get::<Building>(e).is_some_and(|b| b.kind == BuildingType::Floor && !b.is_provisional)}))
        })
        .collect::<Vec<_>>();
    json!({"anchor":anchor,"cells":cells(anchor),"min":area.min().to_array(),"max":area.max().to_array(),
        "sites":sites,"tiles":tiles,"familiars":familiars,"completed":completed,
        "overflow":sites.len()>128 || tiles.len()>128 || familiars.len()>128})
}

pub(super) fn snapshot(world: &mut World) -> Value {
    let mut value = raw(world);
    let Some(anchor) = world.get_resource::<Target>().map(|t| t.0) else {
        return value;
    };
    let area = bounds(anchor);
    let points = [area.min() + Vec2::splat(2.), area.max() - Vec2::splat(2.)];
    let cameras = world
        .query_filtered::<(&Camera, &GlobalTransform), With<hw_ui::camera::MainCamera>>()
        .iter(world)
        .map(|(c, g)| (c.clone(), *g))
        .collect::<Vec<_>>();
    value["points"] = if let [(camera, global)] = cameras.as_slice() {
        json!(points.map(|p| {
            camera
                .world_to_viewport(global, p.extend(0.))
                .ok()
                .map(|p| p.to_array())
        }))
    } else {
        Value::Null
    };
    value["task_mode"] = json!(format!(
        "{:?}",
        world.resource::<crate::app_contexts::TaskContext>().0
    ));
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supply_floor_target_is_forty_five_legal_tiles_without_mutation_or_reserved_overlap() {
        let map = WorldMap::default();
        let area = bounds((40, 40));
        let yards = [Yard {
            min: area.min() - Vec2::splat(TILE_SIZE * 6.),
            max: area.max() + Vec2::splat(TILE_SIZE * 6.),
        }];
        let reserved = [(40, 40), (41, 40), (40, 41), (41, 41)];
        let anchor = choose(&map, &yards, &reserved, (40, 40)).unwrap();
        let grids = cells(anchor);
        assert_eq!(grids.len(), 45);
        assert!(grids.iter().all(|p| !reserved.contains(p)));
        assert!(map.buildings.is_empty());
        assert!(choose(&map, &[], &reserved, (40, 40)).is_none());
        let area = bounds(anchor);
        assert_eq!(
            WorldMap::snap_to_grid_edge(area.min() + Vec2::splat(2.)),
            area.min()
        );
        assert_eq!(
            WorldMap::snap_to_grid_edge(area.max() - Vec2::splat(2.)),
            area.max()
        );
    }

    #[test]
    fn supply_seed_has_a_canonical_unfiltered_forty_five_tile_floor() {
        let layout = hw_world::generate_world_layout(42);
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
                .and_then(|i| map.terrain_at_idx(i))
                .is_some_and(|t| t.is_walkable())
            {
                map.add_grid_obstacle(grid);
            }
        }
        map.register_completed_building_footprint(
            BuildingType::WheelbarrowParking,
            Entity::PLACEHOLDER,
            layout.anchors.wheelbarrow_parking.iter_cells(),
        );
        let yard = Yard {
            min: WorldMap::grid_to_world(layout.anchors.yard.min_x, layout.anchors.yard.min_y),
            max: WorldMap::grid_to_world(layout.anchors.yard.max_x, layout.anchors.yard.max_y),
        };
        let mut reserved =
            live_building_geometry(&map, BuildingType::Tank, (70, 50)).occupied_grids;
        reserved
            .extend(live_building_geometry(&map, BuildingType::MudMixer, (71, 48)).occupied_grids);
        reserved.extend(bucket_storage_geometry((72, 50)).occupied_grids);
        let anchor =
            choose(&map, &[yard], &reserved, (71, 48)).expect("actual seed must admit all45 tiles");
        assert_eq!(cells(anchor).len(), 45);
    }
}
