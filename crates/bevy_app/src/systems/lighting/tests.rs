use std::collections::HashMap;

use bevy::prelude::*;
use hw_core::world::DoorState;
use hw_energy::{PowerShedReason, PowerSupplyState};
use hw_jobs::{Building, BuildingType, Door};
use hw_world::{DoorLockToggleRequest, RoomTileLookup, WorldMap};

use super::*;
use crate::plugins::lighting::IndoorLightingPlugin;
use crate::systems::GameSystemSet;

fn lighting_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<DoorLockToggleRequest>()
        .init_resource::<WorldMap>()
        .init_resource::<RoomTileLookup>()
        .configure_sets(
            Update,
            (
                GameSystemSet::Input,
                GameSystemSet::Spatial,
                GameSystemSet::Logic,
                GameSystemSet::PreActor,
                GameSystemSet::Actor,
                GameSystemSet::PostActor,
                GameSystemSet::Visual,
                GameSystemSet::Interface,
            )
                .chain(),
        )
        .add_plugins(IndoorLightingPlugin);
    app
}

fn publish_indoor_tile(app: &mut App, grid: (i32, i32)) {
    let mut tiles = HashMap::new();
    tiles.insert(grid, Entity::PLACEHOLDER);
    app.world_mut()
        .resource_mut::<RoomTileLookup>()
        .replace(tiles);
}

#[test]
fn supplied_outdoor_lamp_builds_field_and_steady_updates_do_no_work() {
    let mut app = lighting_app();
    app.world_mut()
        .resource_mut::<IndoorLightingAllocationProbe>()
        .enable();
    let grid = (10, 10);
    publish_indoor_tile(&mut app, grid);
    let world = WorldMap::grid_to_world(grid.0, grid.1);
    let lamp = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            Transform::from_translation(world.extend(0.0)),
            RadialLightEmitter::outdoor_lamp(grid),
            PowerSupplyState::Supplied,
        ))
        .id();

    app.update();

    let runtime = app.world().resource::<IndoorLightRuntime>();
    assert_eq!(runtime.availability(), IndoorLightAvailability::Available);
    assert_eq!(runtime.typed_emitter_components(), 1);
    assert_eq!(runtime.eligible_supplied_emitters(), 1);
    let index = usize::try_from(grid.1 * 100 + grid.0).unwrap();
    assert!(runtime.snapshot().unwrap().cells()[index].luminance > 0);
    let probe = app.world().resource::<IndoorLightingAllocationProbe>();
    assert!(probe.emitter_collect_allocation_events().unwrap() >= 1);
    assert!(probe.emitter_collect_allocation_bytes().unwrap() >= 10_000);
    let initial_allocation = (
        probe.emitter_collect_allocation_events(),
        probe.emitter_collect_allocation_bytes(),
    );
    let before = runtime.metrics().clone();

    for _ in 0..600 {
        app.update();
    }

    let runtime = app.world().resource::<IndoorLightRuntime>();
    assert_eq!(
        runtime.metrics().full_snapshot_scan_count,
        before.full_snapshot_scan_count
    );
    assert_eq!(
        runtime.metrics().field_rebuild_count,
        before.field_rebuild_count
    );
    assert_eq!(
        runtime.metrics().output_revision_increment_count,
        before.output_revision_increment_count
    );
    let probe = app.world().resource::<IndoorLightingAllocationProbe>();
    assert_eq!(
        (
            probe.emitter_collect_allocation_events(),
            probe.emitter_collect_allocation_bytes(),
        ),
        initial_allocation,
    );

    for unavailable in [
        PowerSupplyState::Shed {
            reason: PowerShedReason::InsufficientGeneration,
        },
        PowerSupplyState::Disconnected,
        PowerSupplyState::InvalidDemand,
    ] {
        app.world_mut().entity_mut(lamp).insert(unavailable);
        app.update();

        let runtime = app.world().resource::<IndoorLightRuntime>();
        assert_eq!(runtime.availability(), IndoorLightAvailability::Available);
        assert_eq!(runtime.eligible_supplied_emitters(), 0);
        assert_eq!(runtime.snapshot().unwrap().cells()[index].luminance, 0);

        app.world_mut()
            .entity_mut(lamp)
            .insert(PowerSupplyState::Supplied);
        app.update();
        let runtime = app.world().resource::<IndoorLightRuntime>();
        assert_eq!(runtime.eligible_supplied_emitters(), 1);
        assert!(runtime.snapshot().unwrap().cells()[index].luminance > 0);
    }
}

#[test]
fn completed_loaded_outdoor_lamp_reconstructs_runtime_emitter() {
    let mut app = lighting_app();
    let grid = (12, 14);
    publish_indoor_tile(&mut app, grid);
    let world = WorldMap::grid_to_world(grid.0, grid.1);
    let lamp = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            Transform::from_translation(world.extend(0.0)),
            PowerSupplyState::Supplied,
        ))
        .id();

    app.update();

    assert!(app.world().entity(lamp).contains::<RadialLightEmitter>());
    let runtime = app.world().resource::<IndoorLightRuntime>();
    assert_eq!(runtime.availability(), IndoorLightAvailability::Available);
    assert_eq!(runtime.typed_emitter_components(), 1);
    assert_eq!(runtime.eligible_supplied_emitters(), 1);
}

#[test]
fn manual_door_request_is_consumed_while_virtual_time_is_paused() {
    let mut app = lighting_app();
    let grid = (5, 5);
    let world = WorldMap::grid_to_world(grid.0, grid.1);
    let door = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::Door,
                is_provisional: false,
            },
            Door {
                state: DoorState::Closed,
            },
            Transform::from_translation(world.extend(0.0)),
        ))
        .id();
    app.world_mut()
        .resource_mut::<WorldMap>()
        .register_door(grid, door, DoorState::Closed);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app.world_mut()
        .write_message(DoorLockToggleRequest { owner: door });

    app.update();

    assert_eq!(
        app.world().get::<Door>(door).unwrap().state,
        DoorState::Locked
    );
    assert_eq!(app.world().resource::<DoorLockToggleMetrics>().applied, 1);
}

#[test]
fn invalid_manual_door_requests_are_consumed_once_with_reason_metrics() {
    let mut app = lighting_app();
    let invalid = app.world_mut().spawn_empty().id();
    let stale = app.world_mut().spawn_empty().id();
    app.world_mut().despawn(stale);
    app.world_mut()
        .write_message(DoorLockToggleRequest { owner: invalid });
    app.world_mut()
        .write_message(DoorLockToggleRequest { owner: stale });

    app.update();
    let metrics = app.world().resource::<DoorLockToggleMetrics>();
    assert_eq!(metrics.invalid_target, 1);
    assert_eq!(metrics.stale_target, 1);

    app.update();
    let metrics = app.world().resource::<DoorLockToggleMetrics>();
    assert_eq!(metrics.invalid_target, 1);
    assert_eq!(metrics.stale_target, 1);
}

#[test]
fn duplicate_occlusion_owner_fails_unavailable_without_exposing_stale_field() {
    let mut app = lighting_app();
    let grid = (12, 12);
    let transform =
        Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0));
    for _ in 0..2 {
        app.world_mut().spawn((
            Building {
                kind: BuildingType::Wall,
                is_provisional: false,
            },
            transform,
        ));
    }

    app.update();

    let runtime = app.world().resource::<IndoorLightRuntime>();
    assert_eq!(runtime.availability(), IndoorLightAvailability::Unavailable);
    assert!(runtime.snapshot().is_none());
    assert!(
        runtime
            .last_error()
            .unwrap()
            .contains("multiple occlusion owners")
    );
}
