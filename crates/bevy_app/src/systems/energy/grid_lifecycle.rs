use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerGenerator,
    PowerGrid, YardPowerGrid,
};
use hw_world::zones::Yard;

use super::grid_recalc::EnergyUpdateDirty;

/// Observer は world topology を直接編集せず、次の ordered transaction を起こす。
pub fn on_yard_added(_on: On<Add, Yard>, mut dirty: ResMut<EnergyUpdateDirty>) {
    dirty.request_topology_reconcile();
}

pub fn on_yard_removed(_on: On<Remove, Yard>, mut dirty: ResMut<EnergyUpdateDirty>) {
    dirty.request_topology_reconcile();
}

pub fn on_power_consumer_added(_on: On<Add, PowerConsumer>, mut dirty: ResMut<EnergyUpdateDirty>) {
    dirty.request_topology_reconcile();
}

pub fn on_power_generator_added(
    _on: On<Add, PowerGenerator>,
    mut dirty: ResMut<EnergyUpdateDirty>,
) {
    dirty.request_topology_reconcile();
}

#[derive(Clone, Copy)]
struct GridCandidate {
    entity: Entity,
    relationship_count: usize,
}

type ConsumerTopologyQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static Transform>,
        Option<&'static ConsumesFrom>,
    ),
    With<PowerConsumer>,
>;
type GeneratorTopologyQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static Transform>,
        Option<&'static GeneratesFor>,
    ),
    With<PowerGenerator>,
>;
type PowerGridTopologyQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static YardPowerGrid>,
        Option<&'static GridGenerators>,
        Option<&'static GridConsumers>,
    ),
    With<PowerGrid>,
>;

/// Yard/Grid の 1:1 対応と、空間上の generator/consumer 接続を一括修復する。
///
/// load、Yard の編集、建物追加、重複・欠落した関係を同じ経路で正規化する。
pub fn reconcile_power_grid_topology_system(
    mut commands: Commands,
    q_yards: Query<(Entity, &Yard)>,
    q_grids: PowerGridTopologyQuery,
    q_owner_markers_without_grid: Query<Entity, (With<YardPowerGrid>, Without<PowerGrid>)>,
    q_consumers: ConsumerTopologyQuery,
    q_generators: GeneratorTopologyQuery,
    mut dirty: ResMut<EnergyUpdateDirty>,
    #[cfg(feature = "profiling")] mut metrics: ResMut<super::grid_recalc::EnergyPerfMetrics>,
) {
    #[cfg(feature = "profiling")]
    {
        metrics.topology_reconcile_runs = metrics.topology_reconcile_runs.saturating_add(1);
    }
    let mut yards: Vec<(Entity, Yard)> = q_yards
        .iter()
        .map(|(entity, yard)| (entity, yard.clone()))
        .collect();
    yards.sort_by_key(|(entity, _)| entity.to_bits());
    let yard_entities: HashSet<Entity> = yards.iter().map(|(entity, _)| *entity).collect();

    let mut grids_by_yard: HashMap<Entity, Vec<GridCandidate>> = HashMap::new();
    let mut all_grids = Vec::new();
    let mut generator_memberships = HashSet::new();
    let mut consumer_memberships = HashSet::new();
    let mut changed = false;
    for entity in &q_owner_markers_without_grid {
        changed = true;
        commands.entity(entity).despawn();
    }
    for (entity, yard, generators, consumers) in &q_grids {
        let Some(yard) = yard else {
            changed = true;
            commands.entity(entity).despawn();
            continue;
        };
        all_grids.push((entity, yard.0));
        grids_by_yard
            .entry(yard.0)
            .or_default()
            .push(GridCandidate {
                entity,
                relationship_count: generators.map_or(0, GridGenerators::len)
                    + consumers.map_or(0, GridConsumers::len),
            });
        if let Some(generators) = generators {
            generator_memberships.extend(generators.iter().map(|source| (*source, entity)));
        }
        if let Some(consumers) = consumers {
            consumer_memberships.extend(consumers.iter().map(|source| (*source, entity)));
        }
    }

    let mut canonical_grids = HashMap::new();
    for (yard_entity, _) in &yards {
        let candidates = grids_by_yard.entry(*yard_entity).or_default();
        candidates.sort_by(|left, right| {
            right
                .relationship_count
                .cmp(&left.relationship_count)
                .then_with(|| left.entity.to_bits().cmp(&right.entity.to_bits()))
        });

        let canonical = if let Some(candidate) = candidates.first() {
            candidate.entity
        } else {
            changed = true;
            commands
                .spawn((
                    Name::new("PowerGrid"),
                    PowerGrid::default(),
                    YardPowerGrid(*yard_entity),
                ))
                .id()
        };
        canonical_grids.insert(*yard_entity, canonical);

        for duplicate in candidates.iter().skip(1) {
            changed = true;
            commands.entity(duplicate.entity).despawn();
        }
    }

    for (grid_entity, yard_entity) in all_grids {
        if !yard_entities.contains(&yard_entity) {
            changed = true;
            commands.entity(grid_entity).despawn();
        }
    }

    let desired_grid_at = |transform: Option<&Transform>| {
        transform.and_then(|transform| {
            let position = transform.translation.xy();
            yards
                .iter()
                .find(|(_, yard)| yard.contains(position))
                .and_then(|(yard_entity, _)| canonical_grids.get(yard_entity).copied())
        })
    };

    for (entity, transform, current) in &q_consumers {
        let desired = desired_grid_at(transform);
        let current_target = current.map(|relation| relation.0);
        let relation_is_consistent = match (current_target, desired) {
            (None, None) => true,
            (Some(current), Some(desired)) => {
                current == desired && consumer_memberships.contains(&(entity, desired))
            }
            _ => false,
        };
        if relation_is_consistent {
            continue;
        }
        changed = true;
        if let Some(grid) = desired {
            commands.entity(entity).insert(ConsumesFrom(grid));
        } else {
            commands.entity(entity).remove::<ConsumesFrom>();
        }
    }

    for (entity, transform, current) in &q_generators {
        let desired = desired_grid_at(transform);
        let current_target = current.map(|relation| relation.0);
        let relation_is_consistent = match (current_target, desired) {
            (None, None) => true,
            (Some(current), Some(desired)) => {
                current == desired && generator_memberships.contains(&(entity, desired))
            }
            _ => false,
        };
        if relation_is_consistent {
            continue;
        }
        changed = true;
        if let Some(grid) = desired {
            commands.entity(entity).insert(GeneratesFor(grid));
        } else {
            commands.entity(entity).remove::<GeneratesFor>();
        }
    }

    dirty.topology_reconcile_due = false;
    dirty.grid_recalc_due |= changed;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::energy::grid_recalc::{
        EnergyUpdateDirty, detect_energy_update_dirty_system, energy_grid_recalc_should_run,
        energy_topology_should_run, grid_recalc_system,
    };
    use hw_energy::{
        PowerAllocationMode, PowerConsumerPolicy, PowerPriority, PowerSupplyState, Unpowered,
    };

    fn yard(min_x: f32, max_x: f32) -> Yard {
        Yard {
            min: Vec2::new(min_x, -10.0),
            max: Vec2::new(max_x, 10.0),
        }
    }

    #[test]
    fn reconciliation_repairs_missing_duplicate_and_stale_connections() {
        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>().add_systems(
            Update,
            (reconcile_power_grid_topology_system, ApplyDeferred).chain(),
        );

        let yard_a = app.world_mut().spawn(yard(-10.0, -1.0)).id();
        let yard_b = app.world_mut().spawn(yard(1.0, 10.0)).id();
        let stale_yard = app.world_mut().spawn_empty().id();
        let grid_a = app
            .world_mut()
            .spawn((PowerGrid::default(), YardPowerGrid(yard_a)))
            .id();
        let duplicate_a = app
            .world_mut()
            .spawn((PowerGrid::default(), YardPowerGrid(yard_a)))
            .id();
        let orphan = app
            .world_mut()
            .spawn((PowerGrid::default(), YardPowerGrid(stale_yard)))
            .id();
        let bare_grid = app.world_mut().spawn(PowerGrid::default()).id();
        let marker_only = app.world_mut().spawn(YardPowerGrid(yard_a)).id();
        let consumer = app
            .world_mut()
            .spawn((
                PowerConsumer { demand: 1.0 },
                PowerConsumerPolicy {
                    priority: PowerPriority::High,
                },
                Transform::from_xyz(5.0, 0.0, 0.0),
                ConsumesFrom(grid_a),
            ))
            .id();

        app.update();

        let grids: Vec<(Entity, Entity)> = app
            .world_mut()
            .query::<(Entity, &YardPowerGrid)>()
            .iter(app.world())
            .map(|(entity, yard)| (entity, yard.0))
            .collect();
        assert_eq!(grids.iter().filter(|(_, yard)| *yard == yard_a).count(), 1);
        assert_eq!(grids.iter().filter(|(_, yard)| *yard == yard_b).count(), 1);
        assert!(!app.world().entities().contains(orphan));
        assert!(!app.world().entities().contains(bare_grid));
        assert!(!app.world().entities().contains(marker_only));
        assert!(
            !app.world().entities().contains(grid_a)
                || !app.world().entities().contains(duplicate_a)
        );
        let yard_b_grid = grids
            .iter()
            .find_map(|(grid, yard)| (*yard == yard_b).then_some(*grid))
            .unwrap();
        assert_eq!(
            app.world().get::<ConsumesFrom>(consumer).unwrap().0,
            yard_b_grid
        );
    }

    #[test]
    fn ordered_transaction_disconnects_and_reconnects_after_grid_replacement() {
        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .init_resource::<PowerAllocationMode>()
            .add_systems(
                Update,
                (
                    detect_energy_update_dirty_system,
                    reconcile_power_grid_topology_system.run_if(energy_topology_should_run),
                    ApplyDeferred,
                    grid_recalc_system.run_if(energy_grid_recalc_should_run),
                    ApplyDeferred,
                )
                    .chain(),
            );
        let yard = app.world_mut().spawn(yard(-10.0, 10.0)).id();
        let grid = app
            .world_mut()
            .spawn((PowerGrid::default(), YardPowerGrid(yard)))
            .id();
        let consumer = app
            .world_mut()
            .spawn((
                PowerConsumer { demand: 0.0 },
                Transform::from_xyz(0.0, 0.0, 0.0),
                ConsumesFrom(grid),
            ))
            .id();
        app.world_mut()
            .resource_mut::<EnergyUpdateDirty>()
            .request_full_rebuild();

        app.update();
        assert_eq!(
            app.world().get::<PowerSupplyState>(consumer),
            Some(&PowerSupplyState::Supplied)
        );
        assert!(app.world().get::<Unpowered>(consumer).is_none());

        app.world_mut()
            .get_mut::<Transform>(consumer)
            .unwrap()
            .translation
            .x = 20.0;
        app.update();
        assert!(app.world().get::<ConsumesFrom>(consumer).is_none());
        assert_eq!(
            app.world().get::<PowerSupplyState>(consumer),
            Some(&PowerSupplyState::Disconnected)
        );
        assert!(app.world().get::<Unpowered>(consumer).is_some());

        app.world_mut()
            .get_mut::<Transform>(consumer)
            .unwrap()
            .translation
            .x = 0.0;
        app.world_mut().despawn(grid);
        app.update();

        let grids: Vec<Entity> = app
            .world_mut()
            .query::<(Entity, &YardPowerGrid)>()
            .iter(app.world())
            .filter_map(|(entity, owner)| (owner.0 == yard).then_some(entity))
            .collect();
        assert_eq!(grids.len(), 1);
        assert_eq!(
            app.world().get::<ConsumesFrom>(consumer).unwrap().0,
            grids[0]
        );
        assert_eq!(
            app.world().get::<PowerSupplyState>(consumer),
            Some(&PowerSupplyState::Supplied)
        );
        assert!(app.world().get::<Unpowered>(consumer).is_none());
    }

    #[test]
    fn changed_connections_are_repaired_to_the_spatially_canonical_grid_same_update() {
        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .init_resource::<PowerAllocationMode>()
            .add_systems(
                Update,
                (
                    detect_energy_update_dirty_system,
                    reconcile_power_grid_topology_system.run_if(energy_topology_should_run),
                    ApplyDeferred,
                    grid_recalc_system.run_if(energy_grid_recalc_should_run),
                    ApplyDeferred,
                )
                    .chain(),
            );
        let yard_a = app.world_mut().spawn(yard(-10.0, -1.0)).id();
        let yard_b = app.world_mut().spawn(yard(1.0, 10.0)).id();
        let grid_a = app
            .world_mut()
            .spawn((PowerGrid::default(), YardPowerGrid(yard_a)))
            .id();
        let grid_b = app
            .world_mut()
            .spawn((PowerGrid::default(), YardPowerGrid(yard_b)))
            .id();
        let consumer = app
            .world_mut()
            .spawn((
                PowerConsumer { demand: 0.0 },
                Transform::from_xyz(-5.0, 0.0, 0.0),
                ConsumesFrom(grid_a),
            ))
            .id();
        let generator = app
            .world_mut()
            .spawn((
                PowerGenerator {
                    current_output: 1.0,
                    ..default()
                },
                Transform::from_xyz(-5.0, 0.0, 0.0),
                GeneratesFor(grid_a),
            ))
            .id();

        app.update();
        assert_eq!(app.world().get::<ConsumesFrom>(consumer).unwrap().0, grid_a);
        assert_eq!(
            app.world().get::<GeneratesFor>(generator).unwrap().0,
            grid_a
        );

        app.world_mut()
            .entity_mut(consumer)
            .insert(ConsumesFrom(grid_b));
        app.world_mut()
            .entity_mut(generator)
            .insert(GeneratesFor(grid_b));
        app.update();

        assert_eq!(app.world().get::<ConsumesFrom>(consumer).unwrap().0, grid_a);
        assert_eq!(
            app.world().get::<GeneratesFor>(generator).unwrap().0,
            grid_a
        );
        assert!(
            app.world()
                .get::<GridConsumers>(grid_a)
                .is_some_and(|members| members.iter().any(|member| *member == consumer))
        );
        assert!(
            app.world()
                .get::<GridGenerators>(grid_a)
                .is_some_and(|members| members.iter().any(|member| *member == generator))
        );
        assert!(
            app.world()
                .get::<GridConsumers>(grid_b)
                .is_none_or(|members| members.iter().all(|member| *member != consumer))
        );
        assert!(
            app.world()
                .get::<GridGenerators>(grid_b)
                .is_none_or(|members| members.iter().all(|member| *member != generator))
        );
        assert_eq!(
            app.world().get::<PowerSupplyState>(consumer),
            Some(&PowerSupplyState::Supplied)
        );
        assert!(app.world().get::<Unpowered>(consumer).is_none());
    }
}
