use super::*;

#[test]
fn durable_topology_validation_rejects_mistyped_energy_and_zone_links() {
    let mut energy_world = candidate_world();
    let wrong_grid = energy_world.spawn_empty().id();
    energy_world.spawn((PowerGenerator::default(), GeneratesFor(wrong_grid)));
    energy_world.flush();
    assert!(
        validate_durable_topology_candidate(&energy_world)
            .unwrap_err()
            .contains("is not a PowerGrid")
    );

    let mut zone_world = candidate_world();
    let yard = zone_world
        .spawn(Yard {
            min: Vec2::ZERO,
            max: Vec2::ONE,
        })
        .id();
    zone_world.spawn((
        Site {
            min: Vec2::ZERO,
            max: Vec2::ONE,
        },
        PairedYard(yard),
    ));
    assert!(
        validate_durable_topology_candidate(&zone_world)
            .unwrap_err()
            .contains("are not symmetric")
    );
}

#[test]
fn energy_validation_rejects_soul_spa_map_footprint_mismatch() {
    let mut world = candidate_world();
    let site = world
        .spawn((
            SoulSpaSite::default(),
            Building {
                kind: BuildingType::SoulSpa,
                is_provisional: false,
            },
            PowerGenerator::default(),
            Transform::default(),
        ))
        .id();
    for grid in [(8, 8), (9, 8), (8, 9), (9, 9)] {
        world.spawn((
            SoulSpaTile {
                parent_site: site,
                grid_pos: grid,
            },
            Transform::default(),
        ));
    }
    for grid in [(8, 8), (9, 8), (8, 9)] {
        world.resource_mut::<WorldMap>().set_building(grid, site);
    }

    assert!(
        validate_energy_links(&world)
            .unwrap_err()
            .contains("is not owned by site")
    );
}

#[test]
fn durable_topology_validation_rejects_generic_target_with_wrong_role() {
    let mut world = candidate_world();
    let target = world.spawn_empty().id();
    let source = world.spawn(TargetMixer(target)).id();

    assert_eq!(
        validate_durable_topology_candidate(&world).unwrap_err(),
        format!("TargetMixer source {source:?} references a target with the wrong role")
    );
}
