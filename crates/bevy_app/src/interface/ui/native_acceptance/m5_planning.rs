//! Ordinary M5 planning feedback: prepare only the camera and virtual pause.
//! All catalog, ghost, reservation and selection transitions require real input.
use super::*;
use crate::interface::selection::placement_geometry::live_building_geometry;
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
        // This module admits only M5 kinds, never Wall/Door placement.
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
) -> Option<(i32, i32)> {
    (1..MAP_WIDTH - 2)
        .flat_map(|x| (1..MAP_HEIGHT - 2).map(move |y| (x, y)))
        .filter(|&anchor| validate(map, sites, yards, kind, anchor).can_place)
        .min_by_key(|&(x, y)| (x - MAP_WIDTH / 2).abs() + (y - MAP_HEIGHT / 2).abs())
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
    let anchor = legal_anchor(world.resource::<WorldMap>(), &sites, &yards, kind)
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
    let map = world.resource::<WorldMap>();
    let validation = validate(map, &sites, &yards, kind, anchor);
    let cells = geometry.occupied_grids.iter().map(|&(x, y)| json!({"grid": [x, y],
        "walkable": map.is_walkable(x, y), "owner": map.building_entity((x, y)).map(Entity::to_bits)})).collect::<Vec<_>>();
    json!({"kind": format!("{kind:?}"), "anchor": anchor, "footprint": geometry.occupied_grids,
        "draw_pos": geometry.draw_pos.to_array(), "point": projected.map(|point| point.to_array()),
        "cursor_grid": cursor_grid, "primary_window": window_entity.to_bits(),
        "world_epoch": world.resource::<hw_core::WorldEpoch>().get(),
        "play_mode": format!("{:?}", world.resource::<State<hw_core::game_state::PlayMode>>().get()),
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
    fn m5_snapshot_is_null_without_explicit_preparation() {
        assert_eq!(snapshot(&mut World::new()), Value::Null);
    }
}
