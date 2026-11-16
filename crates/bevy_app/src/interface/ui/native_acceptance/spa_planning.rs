//! Ordinary generated-world Spa planning; camera/pause preparation only.
use super::*;
use crate::interface::selection::soul_spa_place::validate_soul_spa_placement;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_world::{WorldMap, zones::Yard};

#[derive(Resource)]
struct Target((i32, i32));

#[derive(Resource)]
struct SeededOccupation {
    owner: Entity,
    tile: Entity,
    worker: Entity,
}

pub(super) fn enabled() -> bool {
    matches!(
        std::env::var("HW_NATIVE_UI_CASE").as_deref(),
        Ok("soul-spa-planning"
            | "soul-spa-completion"
            | "soul-spa-usage"
            | "soul-spa-persistence"
            | "soul-spa-occupied-save"
            | "soul-spa-candidate-occupied-save"
            | "soul-spa-candidate-resumed-use"
            | "soul-spa-candidate-loaded-use"
            | "soul-spa-slots"
            | "refactor-spa-occupied-slots")
    )
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
    if std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("refactor-spa-occupied-slots") {
        prepare_occupied_site(world, anchor);
    }
}

fn prepare_occupied_site(world: &mut World, anchor: (i32, i32)) {
    use crate::assets::GameAssets;
    use crate::interface::selection::soul_spa_place::spawn_soul_spa;
    use crate::plugins::startup::Building3dHandles;
    use crate::world::map::WorldMapWrite;
    use bevy::ecs::system::RunSystemOnce;
    use hw_core::relationships::{CommandedBy, WorkingOn};
    use hw_core::soul::{DamnedSoul, Destination, IdleState, Path};
    use hw_energy::{SoulSpaPhase, SoulSpaSite};
    use hw_jobs::{
        ActiveTaskIdentity, AssignedTask, Designation, GeneratePowerData, GeneratePowerPhase,
        TaskSlots, WorkType,
    };

    let geometry = validate_soul_spa_placement(world.resource::<WorldMap>(), anchor, true).0;
    let (owner, tiles) = world
        .run_system_once_with(
            |In((grids, center)): In<(Vec<(i32, i32)>, Vec2)>,
             mut commands: Commands,
             mut map: WorldMapWrite,
             assets: Res<GameAssets>,
             handles: Res<Building3dHandles>| {
                spawn_soul_spa(
                    &mut commands,
                    &mut map,
                    &grids,
                    center,
                    None,
                    &assets,
                    &handles,
                )
            },
            (geometry.occupied_grids, geometry.draw_pos),
        )
        .expect("Spa fixture spawn");
    let site = SoulSpaSite::default();
    world.entity_mut(owner).insert(SoulSpaSite {
        phase: SoulSpaPhase::Operational,
        bones_delivered: site.bones_required,
        ..site
    });
    for &tile in &tiles {
        world.entity_mut(tile).insert((
            Designation {
                work_type: WorkType::GeneratePower,
            },
            TaskSlots::new(1),
        ));
    }
    let actor = world
        .query::<(Entity, &DamnedSoul, &AssignedTask)>()
        .iter(world)
        .find_map(|(entity, _, task)| matches!(task, AssignedTask::None).then_some(entity))
        .expect("Spa fixture requires an idle real Soul");
    for mut policy in world
        .query::<&mut hw_core::familiar::FamiliarPolicy>()
        .iter_mut(world)
    {
        policy.set_all_allowed(false);
    }
    let tile = tiles[0];
    let pos = world
        .get::<Transform>(tile)
        .expect("Spa tile transform")
        .translation
        .truncate();
    let mut soul = world.get_mut::<DamnedSoul>(actor).expect("Soul fixture");
    soul.dream = 100.0;
    soul.fatigue = 0.0;
    soul.stress = 0.0;
    world.entity_mut(actor).remove::<CommandedBy>();
    world.entity_mut(actor).insert((
        Transform::from_translation(pos.extend(0.0)),
        Destination(pos),
        Path::default(),
        IdleState::default(),
        AssignedTask::GeneratePower(GeneratePowerData {
            tile,
            tile_pos: pos,
            phase: GeneratePowerPhase::Generating,
        }),
        ActiveTaskIdentity::new(tile, tile, WorkType::GeneratePower),
        WorkingOn(tile),
    ));
    world.insert_resource(SeededOccupation {
        owner,
        tile,
        worker: actor,
    });
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
    let owner = map.building_entity(geometry.occupied_grids[0]);
    let selection_target = if matches!(
        std::env::var("HW_NATIVE_UI_CASE").as_deref(),
        Ok("soul-spa-slots" | "refactor-spa-occupied-slots")
    ) && world.resource::<Time<Virtual>>().is_paused()
    {
        owner
            .filter(|owner| {
                world
                    .get::<hw_energy::SoulSpaSite>(*owner)
                    .is_some_and(|site| site.phase == hw_energy::SoulSpaPhase::Operational)
            })
            .and_then(|owner| {
                let mut points: Vec<_> = geometry
                    .occupied_grids
                    .iter()
                    .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                    .collect();
                points.push(geometry.draw_pos);
                crate::interface::selection::native_direct_owner_point(world, owner, &points).map(
                    |(point, screen, rank)| {
                        json!({"owner": owner.to_bits(), "world": point.to_array(),
                        "point": screen.to_array(), "hit": "Direct", "class": "Object", "rank": rank})
                    },
                )
            })
    } else {
        None
    };
    let seeded = world.get_resource::<SeededOccupation>().map(|seed| json!({
        "owner": seed.owner.to_bits(), "tile": seed.tile.to_bits(), "worker": seed.worker.to_bits()}));
    json!({"anchor": anchor, "footprint": geometry.occupied_grids, "draw_pos": geometry.draw_pos.to_array(), "seeded": seeded,
        "real_seconds": world.resource::<Time<Real>>().elapsed_secs_f64(),
        "virtual_seconds": world.resource::<Time<Virtual>>().elapsed_secs_f64(),
        "task_mode": format!("{:?}", world.resource::<crate::app_contexts::TaskContext>().0),
        "point": projected.map(|p| p.to_array()), "owner_point": owner_projected.map(|p| p.to_array()),
        "selection_target": selection_target,
        "picker": crate::interface::selection::candidates::native_inventory(world),
        "selection_point": selection_target.as_ref().and_then(|value| value.get("point")),
        "legal": legal(world.resource::<WorldMap>(), &yards, anchor), "cells": cells, "context": context})
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
