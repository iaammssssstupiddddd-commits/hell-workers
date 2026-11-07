//! Ordinary generated-world Spa planning; camera/pause preparation only.
use super::*;
use crate::interface::selection::soul_spa_place::validate_soul_spa_placement;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_world::{WorldMap, zones::Yard};

#[derive(Resource)]
struct Target((i32, i32));

pub(super) fn enabled() -> bool {
    std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("soul-spa-planning")
}

fn legal(map: &WorldMap, yards: &[Yard], anchor: (i32, i32)) -> bool {
    let (geometry, _) = validate_soul_spa_placement(map, anchor, false);
    let in_yard = yards.iter().any(|yard| {
        geometry
            .occupied_grids
            .iter()
            .all(|&(x, y)| yard.contains(WorldMap::grid_to_world(x, y)))
    });
    validate_soul_spa_placement(map, anchor, in_yard)
        .1
        .can_place
}

fn legal_anchor(map: &WorldMap, yards: &[Yard]) -> Option<(i32, i32)> {
    (1..MAP_WIDTH - 2)
        .flat_map(|x| (2..MAP_HEIGHT - 1).map(move |y| (x, y)))
        .filter(|&anchor| legal(map, yards, anchor))
        .min_by_key(|&(x, y)| (x - MAP_WIDTH / 2).abs() + (y - MAP_HEIGHT / 2).abs())
}

pub(super) fn prepare(world: &mut World) {
    let yards: Vec<_> = world.query::<&Yard>().iter(world).cloned().collect();
    let anchor = legal_anchor(world.resource::<WorldMap>(), &yards)
        .expect("generated world must have an ordinary legal Spa position");
    let center = validate_soul_spa_placement(world.resource::<WorldMap>(), anchor, true)
        .0
        .draw_pos;
    for mut transform in world
        .query_filtered::<&mut Transform, With<hw_ui::camera::MainCamera>>()
        .iter_mut(world)
    {
        transform.translation.x = center.x;
        transform.translation.y = center.y;
    }
    world.insert_resource(Target(anchor));
    world.resource_mut::<Time<Virtual>>().pause();
}

pub(super) fn snapshot(world: &mut World) -> Value {
    let Some(target) = world.get_resource::<Target>() else {
        return Value::Null;
    };
    let anchor = target.0;
    let context = crate::assets::building_asset_set::snapshot_spa_observations(world)
        .unwrap_or_else(|failure| json!({"failure": failure}));
    let yards: Vec<_> = world.query::<&Yard>().iter(world).cloned().collect();
    let geometry = validate_soul_spa_placement(world.resource::<WorldMap>(), anchor, true).0;
    let point = WorldMap::grid_to_world(anchor.0, anchor.1);
    let project = |point: Vec2, world: &mut World| {
        world
            .query_filtered::<(&Camera, &GlobalTransform), With<hw_ui::camera::MainCamera>>()
            .iter(world)
            .find_map(|(camera, transform)| {
                camera.world_to_viewport(transform, point.extend(0.0)).ok()
            })
    };
    let projected = project(point, world);
    let owner_projected = project(geometry.draw_pos, world);
    let map = world.resource::<WorldMap>();
    let cells: Vec<_> = geometry
        .occupied_grids
        .iter()
        .map(|&(x, y)| {
            json!({"grid": [x, y], "owner": map.building_entity((x, y)).map(Entity::to_bits),
            "walkable": map.is_walkable(x, y)})
        })
        .collect();
    json!({"anchor": anchor, "footprint": geometry.occupied_grids, "draw_pos": geometry.draw_pos.to_array(),
        "real_seconds": world.resource::<Time<Real>>().elapsed_secs_f64(),
        "virtual_seconds": world.resource::<Time<Virtual>>().elapsed_secs_f64(),
        "task_mode": format!("{:?}", world.resource::<crate::app_contexts::TaskContext>().0),
        "point": projected.map(|p| p.to_array()), "owner_point": owner_projected.map(|p| p.to_array()),
        "legal": legal(map, &yards, anchor), "cells": cells, "context": context})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spa_anchor_uses_whole_southward_footprint_and_real_owner_rejection() {
        let mut map = WorldMap::default();
        let anchor = (10, 10);
        let geometry = validate_soul_spa_placement(&map, anchor, true).0;
        assert_eq!(
            geometry.occupied_grids,
            vec![(10, 10), (11, 10), (10, 9), (11, 9)]
        );
        assert!(!legal(&map, &[], anchor));
        // Yard construction and actual geometry match the production caller.
        let min = WorldMap::grid_to_world(9, 8);
        let max = WorldMap::grid_to_world(12, 11);
        let yard = Yard { min, max };
        assert!(legal(&map, std::slice::from_ref(&yard), anchor));
        map.set_building((11, 9), Entity::from_bits(1));
        assert!(!legal(&map, &[yard], anchor));
    }
}
