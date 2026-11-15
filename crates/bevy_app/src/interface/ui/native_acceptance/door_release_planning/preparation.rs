//! Bounded, one-time support fixture. No terrain/item/actor or outcome repair.
use super::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_world::zones::Site;

#[derive(Resource, Clone)]
pub(super) struct Target {
    pub case: Case,
    pub anchor: (i32, i32),
    pub supports: Vec<((i32, i32), Entity)>,
    pub initial_buildings: Vec<Value>,
    pub initial_cells: Vec<Value>,
}

pub(super) fn inventory(world: &mut World) -> Result<Vec<Value>, String> {
    let mut rows = world
        .query::<(Entity, &Building, Option<&Transform>)>()
        .iter(world)
        .take(257)
        .map(|(entity, building, pose)| {
            let pose = pose.ok_or("existing Building has no Transform".to_string())?;
            Ok(
                json!({"entity":entity.to_bits(),"kind":format!("{:?}",building.kind),
            "is_provisional":building.is_provisional,"position":pose.translation.to_array()}),
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    if rows.len() > 256 {
        return Err("Door building inventory cap exceeded".into());
    }
    rows.sort_by_key(|row| row["entity"].as_u64());
    Ok(rows)
}

fn empty_cells(map: &WorldMap, anchor: (i32, i32), sites: &[Site], actors: &[Vec2]) -> bool {
    let center = WorldMap::grid_to_world(anchor.0, anchor.1);
    sites.iter().any(|site| site.contains(center))
        && actors
            .iter()
            .all(|point| point.distance_squared(center) > 128. * 128.)
        && (-1..=1).all(|dx| {
            (-1..=1).all(|dy| {
                let p = (anchor.0 + dx, anchor.1 + dy);
                map.is_walkable(p.0, p.1)
                    && !map.has_building(p)
                    && !map.has_stockpile(p)
                    && map.floor_entity(p).is_none()
                    && sites
                        .iter()
                        .any(|site| site.contains(WorldMap::grid_to_world(p.0, p.1)))
            })
        })
}

fn cells(map: &WorldMap, anchor: (i32, i32)) -> Vec<Value> {
    (-1..=1)
        .flat_map(|dx| (-1..=1).map(move |dy| (anchor.0 + dx, anchor.1 + dy)))
        .map(|p| {
            json!({"grid":p,"walkable":map.is_walkable(p.0,p.1),
            "owner":map.building_entity(p).map(Entity::to_bits),
            "floor":map.floor_entity(p).map(Entity::to_bits),"stockpile":map.has_stockpile(p)})
        })
        .collect()
}

pub(super) fn prepare(world: &mut World) -> Result<(), String> {
    if world.contains_resource::<Target>() {
        return Err("Door context preparation is one-time".into());
    }
    let case = current_case().ok_or("missing dedicated Door case")?;
    if world
        .query::<&Building>()
        .iter(world)
        .any(|b| b.kind == BuildingType::Door)
        || world
            .query::<&Blueprint>()
            .iter(world)
            .any(|b| b.kind == BuildingType::Door)
    {
        return Err("Door audit requires no initial Door/Blueprint owners".into());
    }
    let sites = world
        .query::<&Site>()
        .iter(world)
        .cloned()
        .collect::<Vec<_>>();
    let actors = world
        .query_filtered::<&Transform, Or<(
            With<hw_core::soul::DamnedSoul>,
            With<hw_core::familiar::Familiar>,
        )>>()
        .iter(world)
        .map(|pose| pose.translation.truncate())
        .collect::<Vec<_>>();
    let map = world.resource::<WorldMap>();
    let anchor = (2..MAP_WIDTH - 2)
        .flat_map(|x| (2..MAP_HEIGHT - 2).map(move |y| (x, y)))
        .filter(|&grid| empty_cells(map, grid, &sites, &actors))
        .min_by_key(|&(x, y)| ((x - MAP_WIDTH / 2).abs() + (y - MAP_HEIGHT / 2).abs(), x, y))
        .ok_or("no empty walkable Door support cell in the existing Site")?;
    let initial_cells = cells(map, anchor);
    let initial_buildings = inventory(world)?;
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let supports = world.resource_scope(
        |world, handles: Mut<crate::plugins::startup::Building3dHandles>| {
            case.supports(anchor)
                .into_iter()
                .map(|grid| {
                    let owner = crate::systems::jobs::wall_construction::spawn_wall_shell(
                        &mut Commands::new(&mut queue, world),
                        &handles,
                        grid,
                        false,
                    );
                    (grid, owner)
                })
                .collect::<Vec<_>>()
        },
    );
    queue.apply(world);
    for &(grid, owner) in &supports {
        world
            .resource_mut::<WorldMap>()
            .register_completed_building_footprint(BuildingType::Wall, owner, [grid]);
    }
    let point = WorldMap::grid_to_world(anchor.0, anchor.1);
    let cameras = world
        .query_filtered::<Entity, With<hw_ui::camera::MainCamera>>()
        .iter(world)
        .collect::<Vec<_>>();
    let [camera] = cameras.as_slice() else {
        return Err("Door fixture requires one existing main camera".into());
    };
    let mut pose = world
        .get_mut::<Transform>(*camera)
        .ok_or("Door camera pose missing")?;
    pose.translation.x = point.x;
    pose.translation.y = point.y;
    world.resource_mut::<Time<Virtual>>().pause();
    world.init_resource::<crate::interface::selection::building_place::observed_outcomes::PlacementReceipts>();
    world.insert_resource(Target {
        case,
        anchor,
        supports,
        initial_buildings,
        initial_cells,
    });
    Ok(())
}

pub(super) fn initial_snapshot(target: &Target) -> Value {
    json!({"scope":"seeded-cardinal-supports-only","anchor":target.anchor,
        "expected_axis":target.case.expected_axis(),
        "supports":target.supports.iter().map(|(grid,owner)|json!({"grid":grid,"owner":owner.to_bits()})).collect::<Vec<_>>(),
        "initial_buildings":target.initial_buildings,"initial_cells":target.initial_cells,
        "natural_construction_evidence":false})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_support_search_is_read_only_and_respects_existing_world_and_actors() {
        let mut map = WorldMap::default();
        let site = Site {
            min: Vec2::splat(-10000.),
            max: Vec2::splat(10000.),
        };
        let anchor = (30, 30);
        let point = WorldMap::grid_to_world(30, 30);
        let before = map.obstacle_version;
        assert!(empty_cells(&map, anchor, std::slice::from_ref(&site), &[]));
        assert!(!empty_cells(&map, anchor, &[], &[]));
        assert!(!empty_cells(
            &map,
            anchor,
            std::slice::from_ref(&site),
            &[point]
        ));
        assert_eq!(map.obstacle_version, before);
        let owner = World::new().spawn_empty().id();
        map.register_stockpile_tile((29, 30), owner);
        let changed = map.obstacle_version;
        assert!(!empty_cells(&map, anchor, std::slice::from_ref(&site), &[]));
        assert_eq!(map.obstacle_version, changed);
    }
    #[test]
    fn independent_floor_layer_is_not_an_empty_support_cell() {
        let mut map = WorldMap::default();
        let site = Site {
            min: Vec2::splat(-10000.),
            max: Vec2::splat(10000.),
        };
        let owner = World::new().spawn_empty().id();
        map.set_floor((29, 30), owner);
        assert_eq!(map.floor_entity((29, 30)), Some(owner));
        let version = map.obstacle_version;
        assert!(!empty_cells(&map, (30, 30), &[site], &[]));
        assert_eq!(map.floor_entity((29, 30)), Some(owner));
        assert_eq!(map.obstacle_version, version);
    }

    #[test]
    fn complete_building_inventory_rejects_missing_pose_and_overflow_without_mutation() {
        let mut world = World::new();
        let owner = world
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: false,
                },
                Transform::default(),
            ))
            .id();
        assert_eq!(inventory(&mut world).unwrap().len(), 1);
        world.entity_mut(owner).remove::<Transform>();
        assert!(inventory(&mut world).is_err());
        assert!(world.get::<Building>(owner).is_some());
        world.entity_mut(owner).insert(Transform::default());
        for _ in 0..256 {
            world.spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: false,
                },
                Transform::default(),
            ));
        }
        let count = world.entities().len();
        assert!(inventory(&mut world).is_err());
        assert_eq!(world.entities().len(), count);
    }
}
