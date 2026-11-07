use super::*;
use hw_energy::{SoulSpaConstructionCancelResult, SoulSpaPhase, SoulSpaSlotsChangeStatus};
use hw_jobs::{BuildingType, GeneratePowerData, GeneratePowerPhase, WorkType};

fn spa(world: &mut World) -> (Entity, Vec<Entity>) {
    let owner = world
        .spawn((
            SoulSpaSite {
                phase: SoulSpaPhase::Operational,
                ..default()
            },
            Building {
                kind: BuildingType::SoulSpa,
                is_provisional: false,
            },
            Transform::from_xyz(16., -16., 0.),
        ))
        .id();
    let mut tiles = Vec::new();
    for grid in [(0, 0), (1, 0), (0, -1), (1, -1)] {
        let tile = world
            .spawn((
                SoulSpaTile {
                    parent_site: owner,
                    grid_pos: grid,
                },
                Designation {
                    work_type: WorkType::GeneratePower,
                },
                TaskSlots::new(1),
            ))
            .id();
        tiles.push(tile);
    }
    (owner, tiles)
}

#[test]
fn spa_snapshot_reads_all_worker_masks_without_hierarchy_or_gameplay_mutation() {
    let mut world = World::new();
    let (owner, tiles) = spa(&mut world);
    assert!(world.get::<Children>(owner).is_none());
    let grid = world.spawn(PowerGrid::default()).id();
    world.entity_mut(owner).insert(GeneratesFor(grid));
    let mut workers = Vec::new();
    for mask in 0..16 {
        for worker in workers.drain(..) {
            world.despawn(worker);
        }
        for (index, tile) in tiles.iter().enumerate() {
            if mask & (1 << index) != 0 {
                workers.push(
                    world
                        .spawn((
                            DamnedSoul {
                                dream: 37.,
                                fatigue: 0.25,
                                stress: 0.125,
                                ..default()
                            },
                            Transform::from_xyz(index as f32, 0., 0.),
                            WorkingOn(*tile),
                            AssignedTask::GeneratePower(GeneratePowerData {
                                tile: *tile,
                                tile_pos: Vec2::ZERO,
                                phase: GeneratePowerPhase::Generating,
                            }),
                        ))
                        .id(),
                );
            }
        }
        // Existing workers are intentionally above the configured assignment gate.
        // The observer must not normalize or evict them.
        world.get_mut::<SoulSpaSite>(owner).unwrap().active_slots = 0;
        let first = snapshot(&mut world).unwrap();
        assert_eq!(first["sites"][0]["active_slots"], 0);
        assert_eq!(first["sites"][0]["grid"], format!("{grid:?}"));
        assert_eq!(first["grids"][0]["entity_bits"], grid.to_bits());
        assert_eq!(
            first["grids"][0]["generators"],
            json!([format!("{owner:?}")])
        );
        assert_eq!(first["souls"].as_array().unwrap().len(), workers.len());
        for (index, tile) in tiles.iter().enumerate() {
            let row = first["tiles"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["entity"] == format!("{tile:?}"))
                .unwrap();
            assert_eq!(row["parent_site"], format!("{owner:?}"));
            assert!(row["map"].is_null());
            assert_eq!(
                row["workers"].as_array().map_or(0, Vec::len),
                usize::from(mask & (1 << index) != 0)
            );
        }
        for row in first["souls"].as_array().unwrap() {
            assert_eq!(row["dream"], 37.);
            assert_eq!(row["generate_power"]["phase"], "Generating");
            assert_eq!(row["working_on"], row["generate_power"]["tile"]);
        }
        assert_eq!(snapshot(&mut world).unwrap(), first);
        assert_eq!(world.get::<SoulSpaSite>(owner).unwrap().active_slots, 0);
        assert!(world.get::<Children>(owner).is_none());
    }
}

#[test]
fn spa_snapshot_preserves_missing_malformed_and_orphan_state_instead_of_accepting_it() {
    let mut world = World::new();
    let (owner, tiles) = spa(&mut world);
    world.get_mut::<SoulSpaSite>(owner).unwrap().active_slots = 99;
    world.get_mut::<SoulSpaSite>(owner).unwrap().bones_delivered = 777;
    world.entity_mut(owner).remove::<PowerGenerator>();
    world.entity_mut(tiles[0]).insert(SoulSpaTile {
        parent_site: Entity::PLACEHOLDER,
        grid_pos: (-123, 999),
    });
    let worker = world.spawn(WorkingOn(tiles[0])).id();
    let value = snapshot(&mut world).unwrap();
    assert_eq!(value["sites"][0]["active_slots"], 99);
    assert_eq!(value["sites"][0]["bones_delivered"], 777);
    assert!(value["sites"][0]["generator"].is_null());
    assert!(value["sites"][0]["map_building_grids"].is_null());
    assert_eq!(
        value["working_relations"][0]["entity"],
        format!("{worker:?}")
    );
    assert_eq!(value["working_relations"][0]["is_soul"], false);
    assert!(
        value["tiles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["parent_exists"] == false)
    );
    assert!(value.get("accepted").is_none());
    assert_eq!(
        value["outcome_channels"],
        json!({"construction_cancel":false,"slots_change":false})
    );
    assert_eq!(world.get::<SoulSpaSite>(owner).unwrap().active_slots, 99);
}

#[test]
fn spa_snapshot_tracks_parent_site_remapping_and_actual_map_ownership() {
    let mut world = World::new();
    world.init_resource::<WorldMap>();
    let (owner, tiles) = spa(&mut world);
    world.resource_mut::<WorldMap>().set_building((0, 0), owner);
    let before = snapshot(&mut world).unwrap();
    let old_pose = *world.get::<Transform>(owner).unwrap();
    let old_site = world.get::<SoulSpaSite>(owner).unwrap().clone();
    world.despawn(owner);
    let loaded = world.spawn((old_site, old_pose)).id();
    assert_ne!(owner, loaded);
    for tile in &tiles {
        world.get_mut::<SoulSpaTile>(*tile).unwrap().parent_site = loaded;
    }
    world
        .resource_mut::<WorldMap>()
        .set_building((0, 0), loaded);
    let after = snapshot(&mut world).unwrap();
    assert_ne!(before["sites"][0]["entity"], after["sites"][0]["entity"]);
    assert_eq!(before["sites"][0]["pose"], after["sites"][0]["pose"]);
    assert!(world.get::<Children>(loaded).is_none());
    let first = after["tiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["entity"] == format!("{:?}", tiles[0]))
        .unwrap();
    assert_eq!(first["parent_site"], format!("{loaded:?}"));
    assert_eq!(first["map"]["owner"], format!("{loaded:?}"));
    assert_eq!(after["sites"][0]["map_building_grids"], json!([[0, 0]]));
}

#[test]
fn spa_snapshot_rejects_nonfinite_state_and_entity_overflow() {
    for field in ["output", "efficiency", "pose", "soul", "task", "grid"] {
        let mut world = World::new();
        let (owner, tiles) = spa(&mut world);
        match field {
            "output" => {
                world
                    .get_mut::<PowerGenerator>(owner)
                    .unwrap()
                    .current_output = f32::NAN
            }
            "efficiency" => {
                world
                    .get_mut::<PowerGenerator>(owner)
                    .unwrap()
                    .output_per_soul = f32::INFINITY
            }
            "pose" => world.get_mut::<Transform>(owner).unwrap().scale.x = f32::NAN,
            "soul" => {
                world.spawn(DamnedSoul {
                    dream: f32::NAN,
                    ..default()
                });
            }
            "task" => {
                world.spawn((
                    DamnedSoul::default(),
                    AssignedTask::GeneratePower(GeneratePowerData {
                        tile: tiles[0],
                        tile_pos: Vec2::splat(f32::NAN),
                        phase: GeneratePowerPhase::GoingToTile,
                    }),
                ));
            }
            "grid" => {
                world.spawn(PowerGrid {
                    generation: f32::NAN,
                    ..default()
                });
            }
            _ => unreachable!(),
        }
        assert!(
            snapshot(&mut world).unwrap_err().contains("non-finite"),
            "{field}"
        );
    }
    for (kind, limit) in [
        ("site", SITE_LIMIT),
        ("tile", TILE_LIMIT),
        ("soul", SOUL_LIMIT),
        ("worker", SOUL_LIMIT),
        ("grid", SITE_LIMIT),
    ] {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        for _ in 0..=limit {
            match kind {
                "site" => {
                    world.spawn(SoulSpaSite::default());
                }
                "tile" => {
                    world.spawn(SoulSpaTile {
                        parent_site: Entity::PLACEHOLDER,
                        grid_pos: (0, 0),
                    });
                }
                "soul" => {
                    world.spawn(DamnedSoul::default());
                }
                "worker" => {
                    world.spawn(WorkingOn(target));
                }
                "grid" => {
                    world.spawn(PowerGrid::default());
                }
                _ => unreachable!(),
            }
        }
        assert!(snapshot(&mut world).is_err(), "{kind}");
    }
}

#[test]
fn spa_events_read_real_terminal_outcomes_once_without_draining_other_consumers() {
    let mut world = World::new();
    world.init_resource::<Messages<SoulSpaConstructionCancelOutcome>>();
    world.init_resource::<Messages<SoulSpaSlotsChangeOutcome>>();
    let owner = world.spawn_empty().id();
    for result in [
        SoulSpaConstructionCancelResult::Canceled { refunded_bones: 7 },
        SoulSpaConstructionCancelResult::Paused,
        SoulSpaConstructionCancelResult::OwnerMismatch,
    ] {
        world
            .resource_mut::<Messages<SoulSpaConstructionCancelOutcome>>()
            .write(SoulSpaConstructionCancelOutcome {
                target: owner,
                result,
            });
    }
    world
        .resource_mut::<Messages<SoulSpaSlotsChangeOutcome>>()
        .write(SoulSpaSlotsChangeOutcome {
            target: owner,
            status: SoulSpaSlotsChangeStatus::Applied {
                requested: 99,
                applied: 4,
                clamped: true,
            },
        });
    let mut cursor = Events::default();
    assert_eq!(
        snapshot(&mut world).unwrap()["outcome_channels"],
        json!({"construction_cancel":true,"slots_change":true})
    );
    let first = cursor.read(&world, 5, 1.).unwrap();
    assert_eq!(first.len(), 4);
    assert_eq!(first[0]["refunded_bones"], 7);
    assert!(first[1]["refunded_bones"].is_null());
    assert!(first[2]["refunded_bones"].is_null());
    assert_eq!(
        first[3]["applied"],
        json!({"requested":99,"applied":4,"clamped":true})
    );
    assert!(cursor.read(&world, 6, 2.).unwrap().is_empty());
    assert_eq!(Events::default().read(&world, 5, 1.).unwrap(), first);
    for row in first {
        assert_eq!(row["owner"], format!("{owner:?}"));
        assert_eq!(row["sample_index"], 5);
    }
}

#[test]
fn spa_events_fail_closed_above_bounded_inventory() {
    let mut world = World::new();
    world.init_resource::<Messages<SoulSpaConstructionCancelOutcome>>();
    for _ in 0..4097 {
        world
            .resource_mut::<Messages<SoulSpaConstructionCancelOutcome>>()
            .write(SoulSpaConstructionCancelOutcome {
                target: Entity::PLACEHOLDER,
                result: SoulSpaConstructionCancelResult::StaleTarget,
            });
    }
    assert!(Events::default().read(&world, 0, 0.).is_err());
}
