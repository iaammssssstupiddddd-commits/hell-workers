//! Passive target and inventory for ordinary floor demand in the supply campaign.
use super::*;
use crate::interface::selection::placement_geometry::{
    bucket_storage_geometry, live_building_geometry,
};
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

#[derive(Resource)]
struct Target((i32, i32));

fn bounds(anchor: (i32, i32)) -> TaskArea {
    let half = Vec2::splat(TILE_SIZE * 0.5);
    TaskArea::from_points(
        WorldMap::grid_to_world(anchor.0, anchor.1) - half,
        WorldMap::grid_to_world(anchor.0 + 4, anchor.1 + 3) + half,
    )
}

fn cells(anchor: (i32, i32)) -> Vec<(i32, i32)> {
    (0..4)
        .flat_map(|dy| (0..5).map(move |dx| (anchor.0 + dx, anchor.1 + dy)))
        .collect()
}

fn choose(
    map: &WorldMap,
    yards: &[Yard],
    reserved: &[(i32, i32)],
    mixer: (i32, i32),
) -> Option<(i32, i32)> {
    (1..MAP_WIDTH - 5)
        .flat_map(|x| (1..MAP_HEIGHT - 4).map(move |y| (x, y)))
        .filter(|&(x, y)| (x - mixer.0).abs() + (y - mixer.1).abs() <= 12)
        .filter(|&anchor| {
            cells(anchor).iter().all(|&grid| {
                map.is_walkable(grid.0, grid.1)
                    && !map.has_building(grid)
                    && !map.has_stockpile(grid)
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
        .expect("supply needs a legal twenty-tile ordinary floor target");
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
    fn supply_floor_target_is_twenty_legal_tiles_without_mutation_or_reserved_overlap() {
        let map = WorldMap::default();
        let area = bounds((40, 40));
        let yards = [Yard {
            min: area.min() - Vec2::splat(TILE_SIZE * 6.),
            max: area.max() + Vec2::splat(TILE_SIZE * 6.),
        }];
        let reserved = [(40, 40), (41, 40), (40, 41), (41, 41)];
        let anchor = choose(&map, &yards, &reserved, (40, 40)).unwrap();
        let grids = cells(anchor);
        assert_eq!(grids.len(), 20);
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
}
