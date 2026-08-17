use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use hw_core::WorldEpoch;
use hw_core::world::DoorState;
use hw_energy::{PowerShedReason, PowerSupplyState};
use hw_infra::lighting::{CardinalDirection, FixtureMount, LightGridPos};
use hw_jobs::{Building, BuildingType, Door};
use hw_soul_ai::soul_ai::update::slow_simulation::{
    SlowSimulationClock, advance_slow_simulation_clock_system,
};
use hw_world::{
    DoorLockToggleRequest, Room, RoomBounds, RoomTileLookup, RoomTileSignature, WorldMap,
};

use super::*;
use crate::plugins::lighting::IndoorLightingPlugin;
use crate::systems::GameSystemSet;

fn lighting_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<DoorLockToggleRequest>()
        .init_resource::<WorldMap>()
        .init_resource::<RoomTileLookup>()
        .init_resource::<SlowSimulationClock>()
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

fn consumer_app(step: Duration) -> App {
    let mut app = lighting_app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(step))
        .add_systems(
            Update,
            advance_slow_simulation_clock_system.in_set(GameSystemSet::Logic),
        );
    // TimePlugin's first update establishes its baseline; the requested manual
    // duration is observed from the following update onward.
    app.update();
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .set_max_delta(Duration::from_secs(1));
    app
}

fn spawn_room(app: &mut App, tiles: Vec<(i32, i32)>) -> Entity {
    let tile_signature = RoomTileSignature::from_tiles(&tiles);
    let bounds = tiles
        .iter()
        .copied()
        .fold(None, |bounds: Option<RoomBounds>, tile| {
            let mut bounds = bounds.unwrap_or_else(|| RoomBounds::from_tile(tile));
            bounds.include(tile);
            Some(bounds)
        })
        .expect("test Room must contain at least one tile");
    app.world_mut()
        .spawn(Room {
            tile_count: tiles.len(),
            tiles,
            tile_signature,
            wall_tiles: Vec::new(),
            door_tiles: Vec::new(),
            bounds,
        })
        .id()
}

fn publish_room(app: &mut App, room: Entity, tiles: &[(i32, i32)]) {
    let lookup = tiles.iter().copied().map(|tile| (tile, room)).collect();
    app.world_mut()
        .resource_mut::<RoomTileLookup>()
        .replace(lookup);
}

fn publish_indoor_tile(app: &mut App, grid: (i32, i32)) {
    let mut tiles = HashMap::new();
    tiles.insert(grid, Entity::PLACEHOLDER);
    app.world_mut()
        .resource_mut::<RoomTileLookup>()
        .replace(tiles);
}

#[derive(Component)]
struct MoveToLitCell;

fn move_to_lit_cell(mut souls: Query<&mut Transform, With<MoveToLitCell>>) {
    for mut transform in &mut souls {
        transform.translation = WorldMap::grid_to_world(12, 12).extend(0.0);
    }
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
fn recovery_samples_the_rebuilt_field_in_the_same_update() {
    let mut app = consumer_app(Duration::from_millis(100));
    let grid = (10, 10);
    publish_indoor_tile(&mut app, grid);
    app.world_mut().spawn((
        Building {
            kind: BuildingType::OutdoorLamp,
            is_provisional: false,
        },
        Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
        RadialLightEmitter::outdoor_lamp(grid),
        PowerSupplyState::Supplied,
    ));
    let soul = app
        .world_mut()
        .spawn((
            Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            hw_core::soul::DamnedSoul {
                fatigue: 1.0,
                stress: 1.0,
                ..default()
            },
        ))
        .id();

    app.update();

    let soul_state = app.world().get::<hw_core::soul::DamnedSoul>(soul).unwrap();
    assert!(soul_state.fatigue < 1.0);
    assert!(soul_state.stress < 1.0);
    let metrics = app.world().resource::<IndoorLightConsumerMetrics>();
    assert_eq!(metrics.recovery_steps, 1);
    assert_eq!(metrics.soul_samples, 1);
    assert_eq!(metrics.recovery_effects, 1);
}

#[test]
fn recovery_samples_the_post_actor_transform() {
    let mut app = consumer_app(Duration::from_millis(100));
    let lit = (12, 12);
    publish_indoor_tile(&mut app, lit);
    app.add_systems(Update, move_to_lit_cell.in_set(GameSystemSet::Actor));
    app.world_mut().spawn((
        Building {
            kind: BuildingType::OutdoorLamp,
            is_provisional: false,
        },
        Transform::from_translation(WorldMap::grid_to_world(lit.0, lit.1).extend(0.0)),
        RadialLightEmitter::outdoor_lamp(lit),
        PowerSupplyState::Supplied,
    ));
    let soul = app
        .world_mut()
        .spawn((
            MoveToLitCell,
            Transform::from_translation(WorldMap::grid_to_world(30, 30).extend(0.0)),
            hw_core::soul::DamnedSoul {
                fatigue: 1.0,
                stress: 1.0,
                ..default()
            },
        ))
        .id();

    app.update();

    assert!(
        app.world()
            .get::<hw_core::soul::DamnedSoul>(soul)
            .unwrap()
            .fatigue
            < 1.0
    );
    assert_eq!(
        app.world()
            .resource::<IndoorLightConsumerMetrics>()
            .recovery_effects,
        1
    );
}

#[test]
fn recovery_processes_each_catch_up_step_once() {
    for steps in 1..=5 {
        let mut app = consumer_app(Duration::from_millis(100 * steps));
        let grid = (10, 10);
        publish_indoor_tile(&mut app, grid);
        app.world_mut().spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            RadialLightEmitter::outdoor_lamp(grid),
            PowerSupplyState::Supplied,
        ));
        app.world_mut().spawn((
            Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            hw_core::soul::DamnedSoul {
                fatigue: 1.0,
                stress: 1.0,
                ..default()
            },
        ));

        app.update();

        let metrics = app.world().resource::<IndoorLightConsumerMetrics>();
        assert_eq!(metrics.recovery_steps, steps);
        assert_eq!(metrics.soul_samples, steps);
        assert_eq!(metrics.recovery_effects, steps);
        assert_eq!(metrics.old_epoch_recovery_effects, 0);
    }
}

#[test]
fn recovery_is_non_stacking_and_fails_dark_outside_the_field() {
    let mut lit_app = consumer_app(Duration::from_millis(100));
    let lit = (10, 10);
    let adjacent = (11, 10);
    let mut mask = HashMap::new();
    mask.insert(lit, Entity::PLACEHOLDER);
    mask.insert(adjacent, Entity::PLACEHOLDER);
    lit_app
        .world_mut()
        .resource_mut::<RoomTileLookup>()
        .replace(mask);
    for grid in [lit, adjacent] {
        lit_app.world_mut().spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            RadialLightEmitter::outdoor_lamp(grid),
            PowerSupplyState::Supplied,
        ));
    }
    lit_app.world_mut().spawn((
        Transform::from_translation(WorldMap::grid_to_world(lit.0, lit.1).extend(0.0)),
        hw_core::soul::DamnedSoul::default(),
    ));
    lit_app.update();
    let lit_metrics = lit_app.world().resource::<IndoorLightConsumerMetrics>();
    assert_eq!(lit_metrics.soul_samples, 1);
    assert_eq!(lit_metrics.recovery_effects, 1);

    let mut dark_app = consumer_app(Duration::from_millis(100));
    let dark = (20, 20);
    publish_indoor_tile(&mut dark_app, dark);
    for grid in [dark, (-1_000, -1_000)] {
        dark_app.world_mut().spawn((
            Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            hw_core::soul::DamnedSoul::default(),
        ));
    }
    dark_app.update();
    let dark_metrics = dark_app.world().resource::<IndoorLightConsumerMetrics>();
    assert_eq!(dark_metrics.soul_samples, 2);
    assert_eq!(dark_metrics.recovery_effects, 0);
    assert_eq!(dark_metrics.dark_samples, 1);
    assert_eq!(dark_metrics.out_of_bounds_samples, 1);
}

#[test]
fn paused_update_refreshes_room_summary_without_recovering_souls() {
    let mut app = consumer_app(Duration::from_millis(100));
    let grid = (15, 15);
    let room = spawn_room(&mut app, vec![grid]);
    publish_room(&mut app, room, &[grid]);
    app.world_mut().spawn((
        Building {
            kind: BuildingType::OutdoorLamp,
            is_provisional: false,
        },
        Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
        RadialLightEmitter::outdoor_lamp(grid),
        PowerSupplyState::Supplied,
    ));
    let soul = app
        .world_mut()
        .spawn((
            Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            hw_core::soul::DamnedSoul {
                fatigue: 1.0,
                stress: 1.0,
                ..default()
            },
        ))
        .id();
    app.world_mut().resource_mut::<Time<Virtual>>().pause();

    app.update();

    let soul_state = app.world().get::<hw_core::soul::DamnedSoul>(soul).unwrap();
    assert_eq!(soul_state.fatigue, 1.0);
    assert_eq!(soul_state.stress, 1.0);
    let summary = app.world().get::<RoomIlluminationState>(room).unwrap();
    assert_eq!(summary.summary().sample_count, 1);
    assert!(summary.summary().mean_luminance > 0);
}

#[test]
fn recreated_room_reuses_entity_independent_summary_cache() {
    let mut app = lighting_app();
    let grid = (18, 18);
    let first_room = spawn_room(&mut app, vec![grid]);
    publish_room(&mut app, first_room, &[grid]);
    let lamp = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
            RadialLightEmitter::outdoor_lamp(grid),
            PowerSupplyState::Supplied,
        ))
        .id();
    app.update();
    assert!(
        app.world()
            .get::<RoomIlluminationState>(first_room)
            .is_some()
    );
    let before = app
        .world()
        .resource::<IndoorLightConsumerMetrics>()
        .room_summary_recomputes;

    app.world_mut().despawn(first_room);
    let replacement = spawn_room(&mut app, vec![grid]);
    publish_room(&mut app, replacement, &[grid]);
    app.update();

    assert!(
        app.world()
            .get::<RoomIlluminationState>(replacement)
            .is_some()
    );
    let metrics = app.world().resource::<IndoorLightConsumerMetrics>();
    assert_eq!(metrics.room_summary_recomputes, before);
    assert_eq!(metrics.room_summary_cache_hits, 1);
    assert_eq!(app.world().resource::<RoomIlluminationCache>().len(), 1);

    app.world_mut()
        .entity_mut(lamp)
        .insert(PowerSupplyState::Shed {
            reason: PowerShedReason::InsufficientGeneration,
        });
    app.update();
    let metrics = app.world().resource::<IndoorLightConsumerMetrics>();
    assert_eq!(metrics.room_summary_recomputes, before + 1);
    assert_eq!(app.world().resource::<RoomIlluminationCache>().len(), 1);
    assert_eq!(
        app.world()
            .get::<RoomIlluminationState>(replacement)
            .unwrap()
            .summary()
            .mean_luminance,
        0
    );
}

#[test]
fn malformed_room_tile_does_not_publish_a_partial_summary() {
    let mut app = lighting_app();
    let valid = (24, 24);
    let invalid = (25, 24);
    let room = spawn_room(&mut app, vec![valid, invalid]);
    publish_indoor_tile(&mut app, valid);
    app.update();

    assert!(app.world().get::<RoomIlluminationState>(room).is_none());
    assert_eq!(
        app.world()
            .resource::<IndoorLightConsumerMetrics>()
            .invalid_room_summaries,
        1
    );
    assert!(app.world().resource::<RoomIlluminationCache>().is_empty());
}

#[test]
fn room_summary_reader_rejects_old_epoch_and_reset_is_idempotent() {
    let mut app = lighting_app();
    let grid = (22, 22);
    let room = spawn_room(&mut app, vec![grid]);
    publish_room(&mut app, room, &[grid]);
    app.world_mut().spawn((
        Building {
            kind: BuildingType::OutdoorLamp,
            is_provisional: false,
        },
        Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
        RadialLightEmitter::outdoor_lamp(grid),
        PowerSupplyState::Supplied,
    ));
    app.update();

    let state = app
        .world()
        .get::<RoomIlluminationState>(room)
        .unwrap()
        .clone();
    let old_epoch = *app.world().resource::<WorldEpoch>();
    app.world_mut().resource_mut::<WorldEpoch>().advance();
    let current_epoch = *app.world().resource::<WorldEpoch>();
    let field_revision = state.field_revision();
    let topology_revision = state.room_topology_revision();
    let signature = state.room_tile_signature().clone();
    app.world_mut()
        .resource_scope(|_world, mut metrics: Mut<IndoorLightConsumerMetrics>| {
            assert!(
                read_room_illumination_state(
                    Some(&state),
                    old_epoch,
                    current_epoch,
                    field_revision,
                    topology_revision,
                    &signature,
                    &mut metrics,
                )
                .is_none()
            );
        });
    let metrics = app.world().resource::<IndoorLightConsumerMetrics>();
    assert_eq!(metrics.old_epoch_room_read_attempts, 1);
    assert_eq!(metrics.old_epoch_room_reads, 0);

    reset_indoor_light_consumers_for_world_replace(app.world_mut());
    reset_indoor_light_consumers_for_world_replace(app.world_mut());
    assert!(app.world().get::<RoomIlluminationState>(room).is_none());
    assert!(app.world().resource::<RoomIlluminationCache>().is_empty());
    assert_eq!(
        app.world().resource::<IndoorLightConsumerMetrics>(),
        &IndoorLightConsumerMetrics::default()
    );
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
    assert_eq!(
        app.world().get::<LightingFixtureMount>(lamp),
        Some(&LightingFixtureMount::free_standing(grid))
    );
    let runtime = app.world().resource::<IndoorLightRuntime>();
    assert_eq!(runtime.availability(), IndoorLightAvailability::Available);
    assert_eq!(runtime.typed_emitter_components(), 1);
    assert_eq!(runtime.eligible_supplied_emitters(), 1);
}

#[test]
fn moving_a_free_standing_lamp_updates_durable_mount_and_runtime_emitter() {
    let mut app = lighting_app();
    let original = (12, 14);
    let moved = (16, 18);
    let lamp = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            Transform::from_translation(
                WorldMap::grid_to_world(original.0, original.1).extend(0.0),
            ),
            LightingFixtureMount::free_standing(original),
            RadialLightEmitter::outdoor_lamp(original),
        ))
        .id();
    app.update();

    app.world_mut()
        .get_mut::<Transform>(lamp)
        .unwrap()
        .translation = WorldMap::grid_to_world(moved.0, moved.1).extend(0.0);
    app.update();

    let expected = LightingFixtureMount::free_standing(moved);
    assert_eq!(
        app.world().get::<LightingFixtureMount>(lamp),
        Some(&expected)
    );
    assert_eq!(
        app.world().get::<RadialLightEmitter>(lamp).unwrap().mount,
        expected.mount()
    );
}

#[test]
fn fixture_candidate_accepts_valid_wall_mount_and_rejects_invalid_anchor() {
    let mut candidate = World::new();
    candidate.insert_resource(WorldMap::default());
    let anchor_grid = (10, 10);
    let origin_grid = (11, 10);
    let anchor = candidate
        .spawn((
            Building {
                kind: BuildingType::Wall,
                is_provisional: false,
            },
            Transform::from_translation(
                WorldMap::grid_to_world(anchor_grid.0, anchor_grid.1).extend(0.0),
            ),
        ))
        .id();
    let mount = LightingFixtureMount(FixtureMount::WallMounted {
        anchor: LightGridPos::new(anchor_grid.0, anchor_grid.1),
        inward: CardinalDirection::East,
    });
    let lamp = candidate
        .spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            Transform::from_translation(
                WorldMap::grid_to_world(origin_grid.0, origin_grid.1).extend(0.0),
            ),
            mount,
        ))
        .id();
    candidate
        .resource_mut::<WorldMap>()
        .set_building(anchor_grid, anchor);
    candidate
        .resource_mut::<WorldMap>()
        .set_building(origin_grid, lamp);

    assert!(validate_fixture_mount_candidate(&candidate).is_ok());

    candidate.entity_mut(anchor).insert(Building {
        kind: BuildingType::Floor,
        is_provisional: false,
    });
    assert!(
        validate_fixture_mount_candidate(&candidate)
            .unwrap_err()
            .contains("not a completed Wall")
    );
}

#[test]
fn fixture_candidate_rejects_transform_mount_mismatch() {
    let mut candidate = World::new();
    candidate.insert_resource(WorldMap::default());
    let transform_grid = (8, 8);
    let lamp = candidate
        .spawn((
            Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            },
            Transform::from_translation(
                WorldMap::grid_to_world(transform_grid.0, transform_grid.1).extend(0.0),
            ),
            LightingFixtureMount::free_standing((9, 8)),
        ))
        .id();
    candidate
        .resource_mut::<WorldMap>()
        .set_building(transform_grid, lamp);

    assert!(
        validate_fixture_mount_candidate(&candidate)
            .unwrap_err()
            .contains("disagrees with mount origin")
    );
}

#[test]
fn reset_blocks_old_epoch_reads_until_one_successful_wake() {
    let mut app = lighting_app();
    app.world_mut()
        .resource_mut::<IndoorLightingLifecycleProbe>()
        .enable();
    let grid = (20, 20);
    publish_indoor_tile(&mut app, grid);
    app.world_mut().spawn((
        Building {
            kind: BuildingType::OutdoorLamp,
            is_provisional: false,
        },
        Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
        LightingFixtureMount::free_standing(grid),
        RadialLightEmitter::outdoor_lamp(grid),
        PowerSupplyState::Supplied,
    ));
    app.update();

    let old_epoch = *app.world().resource::<WorldEpoch>();
    assert!(
        app.world()
            .resource::<IndoorLightRuntime>()
            .snapshot_for_epoch(old_epoch)
            .is_some()
    );

    reset_indoor_lighting_for_world_replace(app.world_mut());
    app.world_mut().resource_mut::<WorldEpoch>().advance();
    let current_epoch = *app.world().resource::<WorldEpoch>();
    app.world_mut()
        .resource_scope(|world, mut probe: Mut<IndoorLightingLifecycleProbe>| {
            let runtime = world.resource::<IndoorLightRuntime>();
            assert!(
                read_indoor_light_snapshot(runtime, old_epoch, current_epoch, &mut probe).is_none()
            );
            assert!(runtime.is_fail_dark());
        });

    wake_indoor_lighting(app.world_mut());
    app.update();

    let runtime = app.world().resource::<IndoorLightRuntime>();
    assert!(runtime.snapshot_for_epoch(old_epoch).is_none());
    assert!(runtime.snapshot_for_epoch(current_epoch).is_some());
    let probe = app.world().resource::<IndoorLightingLifecycleProbe>();
    assert_eq!(probe.reset_count(), 1);
    assert_eq!(probe.wake_count(), 1);
    assert_eq!(probe.old_epoch_field_read_attempts(), 1);
    assert_eq!(probe.old_epoch_field_reads(), 0);
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
