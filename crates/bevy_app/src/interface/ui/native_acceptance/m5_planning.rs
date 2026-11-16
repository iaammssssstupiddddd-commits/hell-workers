//! Ordinary M5 / separately admitted Mixer planning: camera and pause only.
//! All catalog, ghost, reservation and selection transitions require real input.
use super::*;
use crate::interface::selection::placement_geometry::{
    bucket_storage_geometry, live_building_geometry,
};
use crate::world::map::WorldMapRef;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_jobs::{Blueprint, Building, BuildingType};
use hw_ui::selection::{
    BuildingPlacementContext, PlacementValidation, validate_building_placement,
};
use hw_world::{
    WorldMap,
    zones::{Site, Yard},
};

#[derive(Resource)]
struct Target {
    kind: BuildingType,
    anchor: (i32, i32),
    initial_building_inventory: Vec<Value>,
}

#[derive(Resource)]
struct SupplyMixerTarget {
    anchor: (i32, i32),
}

fn building_inventory(world: &mut World, kind: BuildingType) -> Vec<Value> {
    let mut rows = world
        .query::<(
            Entity,
            &Building,
            Option<&Transform>,
            Option<&GlobalTransform>,
        )>()
        .iter(world)
        .filter(|(_, building, _, _)| building.kind == kind)
        .take(129)
        .map(|(entity, _, local, global)| {
            json!({"entity": entity.to_bits(),
            "pose": local.map(|local| json!({"translation": local.translation.to_array(),
                "rotation": local.rotation.to_array(), "scale": local.scale.to_array()})),
            "global_matrix": global.map(|global| global.to_matrix().to_cols_array())})
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row["entity"].as_u64());
    rows
}

fn case_kind(case: &str) -> Option<BuildingType> {
    if matches!(
        case,
        "refactor-placement-ghosts"
            | "m2-tank-candidate-completion"
            | "m2-tank-candidate-inflow"
            | "m2-tank-candidate-full"
            | "m2-tank-candidate-persistence"
            | "m2-tank-candidate-supply"
    ) {
        return Some(BuildingType::Tank);
    }
    if matches!(
        case,
        "m2-mixer-candidate-planning" | "m2-mixer-candidate-completion"
    ) {
        return Some(BuildingType::MudMixer);
    }
    use crate::assets::building_asset_set::{BuildingAssetKind, m5_kind_for_case};
    match m5_kind_for_case(case)? {
        BuildingAssetKind::SandPile => Some(BuildingType::SandPile),
        BuildingAssetKind::BonePile => Some(BuildingType::BonePile),
        BuildingAssetKind::WheelbarrowParking => Some(BuildingType::WheelbarrowParking),
        BuildingAssetKind::OutdoorLamp => Some(BuildingType::OutdoorLamp),
        _ => None,
    }
}

pub(super) fn enabled() -> bool {
    std::env::var("HW_NATIVE_UI_CASE")
        .ok()
        .as_deref()
        .and_then(case_kind)
        .is_some()
}

fn validate(
    map: &WorldMap,
    sites: &[Site],
    yards: &[Yard],
    kind: BuildingType,
    anchor: (i32, i32),
) -> PlacementValidation {
    let geometry = live_building_geometry(map, kind, anchor);
    let read = WorldMapRef(map);
    let context = BuildingPlacementContext {
        world: &read,
        in_site: sites.iter().any(|site| site.contains(geometry.draw_pos)),
        in_yard: yards.iter().any(|yard| yard.contains(geometry.draw_pos)),
        // This module admits M5/Mixer kinds, never Wall/Door placement.
        is_wall_or_door_at: &|_| false,
        is_replaceable_wall_at: &|_| false,
    };
    validate_building_placement(&context, kind, anchor, &geometry)
}

fn legal_anchor(
    map: &WorldMap,
    sites: &[Site],
    yards: &[Yard],
    kind: BuildingType,
    empty_neighbors: bool,
) -> Option<(i32, i32)> {
    (1..MAP_WIDTH - 2)
        .flat_map(|x| (1..MAP_HEIGHT - 2).map(move |y| (x, y)))
        .filter(|&anchor| validate(map, sites, yards, kind, anchor).can_place)
        .filter(|&(x, y)| kind != BuildingType::Tank || companion_legal(map, (x, y)))
        .filter(|&(x, y)| {
            !empty_neighbors
                || [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                    .into_iter()
                    .all(|grid| map.building_entity(grid).is_none())
        })
        .min_by_key(|&(x, y)| (x - MAP_WIDTH / 2).abs() + (y - MAP_HEIGHT / 2).abs())
}

fn companion_legal(map: &WorldMap, anchor: (i32, i32)) -> bool {
    hw_ui::selection::validate_bucket_storage_placement(
        &WorldMapRef(map),
        &bucket_storage_geometry((anchor.0 + 2, anchor.1)),
        &live_building_geometry(map, BuildingType::Tank, anchor).occupied_grids,
        true,
        hw_ui::selection::TANK_NEARBY_BUCKET_STORAGE_TILES,
    )
    .can_place
}

fn supply_mixer_anchor(
    map: &WorldMap,
    sites: &[Site],
    yards: &[Yard],
    tank: (i32, i32),
) -> Option<(i32, i32)> {
    let mut reserved = live_building_geometry(map, BuildingType::Tank, tank).occupied_grids;
    reserved.extend(bucket_storage_geometry((tank.0 + 2, tank.1)).occupied_grids);
    (1..MAP_WIDTH - 2)
        .flat_map(|x| (1..MAP_HEIGHT - 2).map(move |y| (x, y)))
        .filter(|&p| (p.0 - tank.0).abs() + (p.1 - tank.1).abs() <= 12)
        .filter(|&p| validate(map, sites, yards, BuildingType::MudMixer, p).can_place)
        .filter(|&p| {
            live_building_geometry(map, BuildingType::MudMixer, p)
                .occupied_grids
                .iter()
                .all(|cell| {
                    reserved
                        .iter()
                        .all(|r| (cell.0 - r.0).abs().max((cell.1 - r.1).abs()) >= 2)
                })
        })
        .min_by_key(|&(x, y)| ((x - tank.0).abs() + (y - tank.1).abs(), x, y))
}

pub(super) fn prepare(world: &mut World) {
    let kind = case_kind(&std::env::var("HW_NATIVE_UI_CASE").expect("M5 case required"))
        .expect("dedicated M5 case required");
    let sites = world
        .query::<&Site>()
        .iter(world)
        .cloned()
        .collect::<Vec<_>>();
    let yards = world
        .query::<&Yard>()
        .iter(world)
        .cloned()
        .collect::<Vec<_>>();
    let ghost_case =
        std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("refactor-placement-ghosts");
    let anchor = legal_anchor(
        world.resource::<WorldMap>(),
        &sites,
        &yards,
        kind,
        ghost_case,
    )
    .expect("generated world must contain a legal M5 anchor");
    let center = live_building_geometry(world.resource::<WorldMap>(), kind, anchor).draw_pos;
    for mut transform in world
        .query_filtered::<&mut Transform, With<hw_ui::camera::MainCamera>>()
        .iter_mut(world)
    {
        transform.translation.x = center.x;
        transform.translation.y = center.y;
    }
    let initial_building_inventory = building_inventory(world, kind);
    if std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("m2-tank-candidate-supply") {
        assert!(
            building_inventory(world, BuildingType::MudMixer).is_empty(),
            "supply requires no initial Mixer; never remove it"
        );
        let mixer = supply_mixer_anchor(world.resource::<WorldMap>(), &sites, &yards, anchor)
            .expect("generated world requires a separate legal Mixer anchor with a free corridor");
        super::supply_worldgen::prepare(world, anchor, mixer, &yards);
        super::supply_workload::prepare(world, anchor, mixer, &yards);
        world.insert_resource(SupplyMixerTarget { anchor: mixer });
    }
    assert!(
        !matches!(kind, BuildingType::MudMixer | BuildingType::Tank)
            || initial_building_inventory.is_empty(),
        "M2 planning requires no initial Mixer; never clear existing owners"
    );
    assert!(
        initial_building_inventory.len() <= 16,
        "M5 initial owner cap exceeded"
    );
    world.insert_resource(Target {
        kind,
        anchor,
        initial_building_inventory,
    });
    world.resource_mut::<Time<Virtual>>().pause();
}

pub(super) fn snapshot(world: &mut World) -> Value {
    let Some(target) = world.get_resource::<Target>() else {
        return Value::Null;
    };
    let (kind, anchor) = (target.kind, target.anchor);
    let initial_building_inventory = target.initial_building_inventory.clone();
    let mixer = world
        .get_resource::<SupplyMixerTarget>()
        .map(|target| target.anchor);
    let mut value = snapshot_at(world, kind, anchor, initial_building_inventory);
    if std::env::var("HW_NATIVE_UI_CASE").as_deref() == Ok("refactor-placement-ghosts") {
        value["placement_ghosts"] = placement_ghosts(world);
    }
    if let Some(mixer) = mixer {
        value["supply_mixer"] = snapshot_at(world, BuildingType::MudMixer, mixer, Vec::new());
        value["supply_worldgen"] = super::supply_worldgen::snapshot(world);
        value["supply_workload"] = super::supply_workload::snapshot(world);
    }
    value
}

fn placement_ghosts(world: &mut World) -> Value {
    use crate::assets::building_asset_set::{BuildingAssetKind, BuildingAssetPool};
    use crate::assets::door_asset_set::{DoorAssetReadiness, DoorAssetReadinessState};
    use crate::systems::visual::placement_ghost::{PlacementGhost, PlacementPartnerGhost};
    use bevy::sprite::Anchor;

    let ghosts: Vec<_> = world
        .query_filtered::<Entity, With<PlacementGhost>>()
        .iter(world)
        .take(3)
        .collect();
    let partners: Vec<_> = world
        .query_filtered::<Entity, With<PlacementPartnerGhost>>()
        .iter(world)
        .take(3)
        .collect();
    let sprite = |entity: Entity| {
        let sprite = world.get::<Sprite>(entity)?;
        let anchor = world.get::<Anchor>(entity)?;
        let transform = world.get::<Transform>(entity)?;
        let server = world.resource::<AssetServer>();
        let path = server
            .get_path(sprite.image.id())
            .map(|path| path.path().to_string_lossy().into_owned());
        Some(json!({
            "entity": entity.to_bits(), "image": format!("{:?}", sprite.image.id()), "path": path,
            "resident": world.resource::<Assets<Image>>().get(&sprite.image).is_some(),
            "size": sprite.custom_size.map(|size| size.to_array()), "anchor": anchor.0.to_array(),
            "color": sprite.color.to_srgba().to_f32_array(), "position": transform.translation.to_array(),
            "visible": world.get::<InheritedVisibility>(entity).is_some_and(|visibility| visibility.get()),
            "view_visible": world.get::<ViewVisibility>(entity).is_some_and(|visibility| visibility.get()),
        }))
    };
    let pool = world.resource::<BuildingAssetPool>();
    let active = pool.active(BuildingAssetKind::Tank);
    let pending = pool.pending(BuildingAssetKind::Tank);
    let failure = pool.failure(BuildingAssetKind::Tank);
    let door = match &world.resource::<DoorAssetReadiness>().state {
        DoorAssetReadinessState::Eligible(identity) => Some(json!({
            "generation": identity.asset_set_generation,
            "authority": format!("{:?}", identity.authority),
            "manifest_sha256": identity.manifest_sha256,
        })),
        _ => None,
    };
    json!({
        "active": active, "pending": pending, "failure": failure.map(|failure| format!("{failure:?}")), "door": door,
        "ghosts": ghosts.into_iter().map(sprite).collect::<Vec<_>>(),
        "partners": partners.into_iter().map(sprite).collect::<Vec<_>>(),
    })
}

fn snapshot_at(
    world: &mut World,
    kind: BuildingType,
    anchor: (i32, i32),
    initial_building_inventory: Vec<Value>,
) -> Value {
    let sites = world
        .query::<&Site>()
        .iter(world)
        .cloned()
        .collect::<Vec<_>>();
    let yards = world
        .query::<&Yard>()
        .iter(world)
        .cloned()
        .collect::<Vec<_>>();
    let point = WorldMap::grid_to_world(anchor.0, anchor.1);
    let geometry = live_building_geometry(world.resource::<WorldMap>(), kind, anchor);
    let cameras = world
        .query_filtered::<(Entity, &Camera, &GlobalTransform), With<hw_ui::camera::MainCamera>>()
        .iter(world)
        .map(|(entity, camera, global)| (entity, camera.clone(), *global))
        .collect::<Vec<_>>();
    let [(camera_entity, camera, camera_global)] = cameras.as_slice() else {
        return json!({"failure": "M5 requires one main camera"});
    };
    let windows = world
        .query_filtered::<(Entity, &Window), With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .map(|(entity, window)| (entity, window.clone()))
        .collect::<Vec<_>>();
    let [(window_entity, window)] = windows.as_slice() else {
        return json!({"failure": "M5 requires one primary window"});
    };
    if !camera.is_active
        || camera.viewport.is_some()
        || world
            .get::<bevy::camera::visibility::RenderLayers>(*camera_entity)
            .cloned()
            .unwrap_or_default()
            != bevy::camera::visibility::RenderLayers::layer(0)
        || camera.physical_viewport_size() != Some(window.physical_size())
        || !matches!(
            world.get::<bevy::camera::RenderTarget>(*camera_entity),
            Some(bevy::camera::RenderTarget::Window(
                bevy::window::WindowRef::Primary
            ))
        )
    {
        return json!({"failure": "M5 camera must render the entire primary client"});
    }
    let projected = camera
        .world_to_viewport(camera_global, point.extend(0.0))
        .ok();
    let cursor_grid = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_global, cursor).ok())
        .map(WorldMap::world_to_grid);
    let blueprints = world.query::<(Entity, &Blueprint)>().iter(world)
        .filter(|(_, blueprint)| blueprint.kind == kind).take(129)
        .map(|(entity, blueprint)| json!({"entity": entity.to_bits(), "debug_entity": format!("{entity:?}"), "footprint": blueprint.occupied_grids,
            "progress": blueprint.progress, "materials_complete": blueprint.materials_complete(),
            "delivered_materials": blueprint.delivered_materials.iter().map(|(kind, count)| (format!("{kind:?}"), *count)).collect::<std::collections::BTreeMap<_, _>>() }))
        .collect::<Vec<_>>();
    let buildings = world
        .query::<(Entity, &Building)>()
        .iter(world)
        .filter(|(_, building)| building.kind == kind)
        .take(129)
        .map(|(entity, _)| entity.to_bits())
        .collect::<Vec<_>>();
    let building_inventory = building_inventory(world, kind);
    let consumers = crate::assets::building_asset_set::snapshot_ordinary_art(world)
        .unwrap_or_else(|failure| json!({"failure": failure}));
    let companion_grid = (anchor.0 + 2, anchor.1);
    let companion_point = camera
        .world_to_viewport(
            camera_global,
            WorldMap::grid_to_world(companion_grid.0, companion_grid.1).extend(0.0),
        )
        .ok();
    let companion_pending = world.get_resource::<crate::app_contexts::CompanionPlacementState>()
        .and_then(|state| state.0.as_ref()).map(|pending| json!({
            "parent_anchor": pending.parent_anchor, "parent_kind": format!("{:?}", pending.parent_kind),
            "kind": format!("{:?}", pending.kind), "center": pending.center.to_array(),
            "radius": pending.radius, "required": pending.required}));
    let map = world.resource::<WorldMap>();
    let validation = validate(map, &sites, &yards, kind, anchor);
    let cells = geometry.occupied_grids.iter().map(|&(x, y)| json!({"grid": [x, y],
        "walkable": map.is_walkable(x, y), "owner": map.building_entity((x, y)).map(Entity::to_bits)})).collect::<Vec<_>>();
    json!({"kind": format!("{kind:?}"), "anchor": anchor, "footprint": geometry.occupied_grids,
        "draw_pos": geometry.draw_pos.to_array(), "point": projected.map(|point| point.to_array()),
        "companion_grid": companion_grid, "companion_point": companion_point.map(|point| point.to_array()),
        "companion_pending": companion_pending, "companion_legal": companion_legal(map, anchor),
        "cursor_grid": cursor_grid, "primary_window": window_entity.to_bits(),
        "world_epoch": world.resource::<hw_core::WorldEpoch>().get(),
        "play_mode": format!("{:?}", world.resource::<State<hw_core::game_state::PlayMode>>().get()),
        "task_mode": format!("{:?}", world.resource::<crate::app_contexts::TaskContext>().0),
        "anchor_world": point.to_array(),
        "build_kind": world.resource::<crate::app_contexts::BuildContext>().0.map(|kind| format!("{kind:?}")),
        "camera": {"entity": camera_entity.to_bits(), "primary": true, "viewport": window.physical_size().to_array()},
        "legal": validation.can_place, "reject_reason": validation.reject_reason.map(|reason| format!("{reason:?}")),
        "cells": cells, "blueprints": blueprints, "buildings": buildings,
        "initial_building_inventory": initial_building_inventory, "building_inventory": building_inventory,
        "consumers": consumers,
        "real_seconds": world.resource::<Time<Real>>().elapsed_secs_f64(),
        "virtual_seconds": world.resource::<Time<Virtual>>().elapsed_secs_f64(),
        "inventory_overflow": blueprints.len() > 128 || buildings.len() > 128})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m5_cases_do_not_admit_bridge_or_existing_seeded_cases() {
        assert_eq!(
            case_kind("m2-tank-candidate-completion"),
            Some(BuildingType::Tank)
        );
        assert_eq!(
            case_kind("m2-mixer-candidate-planning"),
            Some(BuildingType::MudMixer)
        );
        assert_eq!(case_kind("m2-tank-candidate-planning"), None);
        assert_eq!(case_kind("m2-ui-mixer-refining"), None);
        for (case, kind) in [
            ("m5-sand-planning", BuildingType::SandPile),
            ("m5-bone-planning", BuildingType::BonePile),
            ("m5-parking-planning", BuildingType::WheelbarrowParking),
            ("m5-lamp-planning", BuildingType::OutdoorLamp),
        ] {
            assert_eq!(case_kind(case), Some(kind));
        }
        for case in [
            "bridge-planning",
            "m5-art-context",
            "rest-area-planning",
            "navigation",
            "",
        ] {
            assert_eq!(case_kind(case), None);
        }
    }

    #[test]
    fn m5_initial_owner_inventory_is_read_only_and_detects_move_or_extra_owner() {
        let mut world = World::new();
        let pose = Transform::from_xyz(1056.0, 96.0, 0.5);
        let owner = world
            .spawn((
                Building {
                    kind: BuildingType::WheelbarrowParking,
                    is_provisional: false,
                },
                pose,
                GlobalTransform::from(pose),
            ))
            .id();
        world.spawn((
            Building {
                kind: BuildingType::SandPile,
                is_provisional: false,
            },
            Transform::default(),
            GlobalTransform::default(),
        ));
        let count = world.entities().len();
        let initial = building_inventory(&mut world, BuildingType::WheelbarrowParking);
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0]["entity"], json!(owner.to_bits()));
        assert_eq!(
            initial[0]["pose"]["translation"],
            json!([1056.0, 96.0, 0.5])
        );
        assert_eq!(world.entities().len(), count);
        assert_eq!(world.get::<Transform>(owner), Some(&pose));
        assert_eq!(
            building_inventory(&mut world, BuildingType::WheelbarrowParking),
            initial
        );
        world.get_mut::<Transform>(owner).unwrap().translation.x += 32.0;
        assert_ne!(
            building_inventory(&mut world, BuildingType::WheelbarrowParking),
            initial
        );
        world.spawn((
            Building {
                kind: BuildingType::WheelbarrowParking,
                is_provisional: false,
            },
            Transform::default(),
            GlobalTransform::default(),
        ));
        assert_eq!(
            building_inventory(&mut world, BuildingType::WheelbarrowParking).len(),
            2
        );
    }

    #[test]
    fn tank_completion_requires_free_companion_cells_without_mutation() {
        let mut map = WorldMap::default();
        assert!(companion_legal(&map, (10, 10)));
        let mut world = World::new();
        let other = world.spawn_empty().id();
        map.register_stockpile_tile((13, 10), other);
        let version = map.obstacle_version;
        assert!(!companion_legal(&map, (10, 10)));
        assert_eq!(map.obstacle_version, version);
    }

    #[test]
    fn supply_secondary_anchor_is_read_only_legal_and_keeps_a_corridor() {
        let map = WorldMap::default();
        let yards = [Yard {
            min: Vec2::splat(-10000.0),
            max: Vec2::splat(10000.0),
        }];
        let tank = (50, 50);
        let version = map.obstacle_version;
        let anchor = supply_mixer_anchor(&map, &[], &yards, tank).unwrap();
        assert_eq!(Some(anchor), supply_mixer_anchor(&map, &[], &yards, tank));
        assert!(validate(&map, &[], &yards, BuildingType::MudMixer, anchor).can_place);
        let mut reserved = live_building_geometry(&map, BuildingType::Tank, tank).occupied_grids;
        reserved.extend(bucket_storage_geometry((52, 50)).occupied_grids);
        assert!(
            live_building_geometry(&map, BuildingType::MudMixer, anchor)
                .occupied_grids
                .iter()
                .all(|p| reserved
                    .iter()
                    .all(|r| (p.0 - r.0).abs().max((p.1 - r.1).abs()) >= 2))
        );
        assert_eq!(version, map.obstacle_version);
        assert!(supply_mixer_anchor(&map, &[], &[], tank).is_none());
        assert_eq!(
            case_kind("m2-tank-candidate-supply"),
            Some(BuildingType::Tank)
        );
    }

    #[test]
    fn m5_snapshot_is_null_without_explicit_preparation() {
        assert_eq!(snapshot(&mut World::new()), Value::Null);
    }
}
