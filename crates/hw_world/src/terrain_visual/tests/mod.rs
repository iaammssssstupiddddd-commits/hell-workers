use super::{
    ObstaclePositionIndex, TerrainChangedEvent, obstacle_sync_system, seed_obstacle_position_index,
};
use crate::map::WorldMap;
use crate::terrain::TerrainType;
use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::system_sets::ObstacleSyncSet;
use hw_jobs::construction::WallConstructionSite;
use hw_jobs::{Blueprint, Building, BuildingType, ObstaclePosition, ObstacleSourceKind};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct MarkerMutationSet;

#[derive(Resource)]
struct PendingMarkerRemoval(Option<Entity>);

#[derive(Resource, Default)]
struct PathfindingProbe(Option<bool>);

fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(WorldMap::default())
        .init_resource::<ObstaclePositionIndex>()
        .add_message::<TerrainChangedEvent>()
        .add_systems(Update, obstacle_sync_system);
    app
}

fn queue_marker_removal(mut commands: Commands, mut pending: ResMut<PendingMarkerRemoval>) {
    if let Some(marker) = pending.0.take() {
        commands.entity(marker).remove::<ObstaclePosition>();
    }
}

fn record_pathfinding_input(world_map: Res<WorldMap>, mut probe: ResMut<PathfindingProbe>) {
    probe.0 = Some(world_map.is_walkable(13, 14));
}

#[test]
fn natural_removal_clears_blocker_and_turns_terrain_to_dirt() {
    let mut app = test_app();
    let marker = app
        .world_mut()
        .spawn((
            ObstaclePosition(3, 4),
            ObstacleSourceKind::NaturalTerrainClearing,
        ))
        .id();

    app.update();
    assert!(!app.world().resource::<WorldMap>().is_walkable(3, 4));

    app.world_mut()
        .entity_mut(marker)
        .remove::<ObstaclePosition>();
    app.update();

    let map = app.world().resource::<WorldMap>();
    assert!(map.is_walkable(3, 4));
    let idx = map.pos_to_idx(3, 4).unwrap();
    assert_eq!(map.terrain_at_idx(idx), Some(TerrainType::Dirt));
}

#[test]
fn non_natural_removal_keeps_terrain_unchanged() {
    let mut app = test_app();
    let marker = app
        .world_mut()
        .spawn((
            ObstaclePosition(5, 6),
            ObstacleSourceKind::ConstructionProtection,
        ))
        .id();

    app.update();
    app.world_mut()
        .entity_mut(marker)
        .remove::<ObstaclePosition>();
    app.update();

    let map = app.world().resource::<WorldMap>();
    assert!(map.is_walkable(5, 6));
    let idx = map.pos_to_idx(5, 6).unwrap();
    assert_eq!(map.terrain_at_idx(idx), Some(TerrainType::Grass));
}

#[test]
fn placement_reservation_removal_keeps_terrain_unchanged() {
    let mut app = test_app();
    let reservation = app.world_mut().spawn_empty().id();
    let marker = app
        .world_mut()
        .spawn((
            ChildOf(reservation),
            ObstaclePosition(6, 7),
            ObstacleSourceKind::PlacementReservation,
        ))
        .id();

    app.update();
    assert!(!app.world().resource::<WorldMap>().is_walkable(6, 7));

    app.world_mut()
        .entity_mut(marker)
        .remove::<ObstaclePosition>();
    app.update();

    let map = app.world().resource::<WorldMap>();
    assert!(map.is_walkable(6, 7));
    let idx = map.pos_to_idx(6, 7).unwrap();
    assert_eq!(map.terrain_at_idx(idx), Some(TerrainType::Grass));
}

#[test]
fn last_marker_removal_controls_the_grid_blocker() {
    let mut app = test_app();
    let first = app
        .world_mut()
        .spawn((
            ObstaclePosition(7, 8),
            ObstacleSourceKind::NaturalTerrainClearing,
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            ObstaclePosition(7, 8),
            ObstacleSourceKind::ConstructionProtection,
        ))
        .id();

    app.update();
    app.world_mut()
        .entity_mut(first)
        .remove::<ObstaclePosition>();
    app.update();
    assert!(!app.world().resource::<WorldMap>().is_walkable(7, 8));

    app.world_mut()
        .entity_mut(second)
        .remove::<ObstaclePosition>();
    app.update();
    assert!(app.world().resource::<WorldMap>().is_walkable(7, 8));
}

#[test]
fn same_owner_markers_require_the_last_removal_to_unblock() {
    let mut app = test_app();
    let owner = app.world_mut().spawn_empty().id();
    let first = app
        .world_mut()
        .spawn((
            ChildOf(owner),
            ObstaclePosition(8, 9),
            ObstacleSourceKind::BuildingFootprint,
        ))
        .id();
    let second = app
        .world_mut()
        .spawn((
            ChildOf(owner),
            ObstaclePosition(8, 9),
            ObstacleSourceKind::BuildingFootprint,
        ))
        .id();

    app.update();
    app.world_mut()
        .entity_mut(first)
        .remove::<ObstaclePosition>();
    app.update();
    assert!(!app.world().resource::<WorldMap>().is_walkable(8, 9));

    app.world_mut()
        .entity_mut(second)
        .remove::<ObstaclePosition>();
    app.update();

    let map = app.world().resource::<WorldMap>();
    assert!(map.is_walkable(8, 9));
    let idx = map.pos_to_idx(8, 9).unwrap();
    assert_eq!(map.terrain_at_idx(idx), Some(TerrainType::Grass));
}

#[test]
fn direct_building_owner_keeps_its_mirror_grid_blocked() {
    let mut app = test_app();
    let building = app
        .world_mut()
        .spawn(Building {
            kind: BuildingType::Tank,
            is_provisional: false,
        })
        .id();
    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_building_occupancy((9, 10), building);
    let marker = app
        .world_mut()
        .spawn((
            ChildOf(building),
            ObstaclePosition(9, 10),
            ObstacleSourceKind::BuildingFootprint,
        ))
        .id();

    app.update();
    app.world_mut()
        .entity_mut(marker)
        .remove::<ObstaclePosition>();
    app.update();

    let map = app.world().resource::<WorldMap>();
    assert!(!map.is_walkable(9, 10));
    let idx = map.pos_to_idx(9, 10).unwrap();
    assert_eq!(map.terrain_at_idx(idx), Some(TerrainType::Grass));
}

#[test]
fn direct_blueprint_owner_keeps_overlapping_marker_grid_blocked() {
    let mut app = test_app();
    let blueprint = app
        .world_mut()
        .spawn(Blueprint::new(BuildingType::Tank, vec![(10, 11)]))
        .id();
    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_building_occupancy((10, 11), blueprint);
    let marker = app
        .world_mut()
        .spawn((
            ObstaclePosition(10, 11),
            ObstacleSourceKind::NaturalTerrainClearing,
        ))
        .id();

    app.update();
    app.world_mut()
        .entity_mut(marker)
        .remove::<ObstaclePosition>();
    app.update();

    let map = app.world().resource::<WorldMap>();
    assert!(!map.is_walkable(10, 11));
    let idx = map.pos_to_idx(10, 11).unwrap();
    assert_eq!(map.terrain_at_idx(idx), Some(TerrainType::Grass));
}

#[test]
fn direct_wall_construction_owner_keeps_overlapping_marker_grid_blocked() {
    let mut app = test_app();
    let wall_site = app
        .world_mut()
        .spawn(WallConstructionSite::new(
            TaskArea::from_points(Vec2::ZERO, Vec2::splat(16.0)),
            Vec2::ZERO,
            1,
        ))
        .id();
    app.world_mut()
        .resource_mut::<WorldMap>()
        .set_building_occupancy((12, 13), wall_site);
    let marker = app
        .world_mut()
        .spawn((
            ObstaclePosition(12, 13),
            ObstacleSourceKind::NaturalTerrainClearing,
        ))
        .id();

    app.update();
    app.world_mut()
        .entity_mut(marker)
        .remove::<ObstaclePosition>();
    app.update();

    let map = app.world().resource::<WorldMap>();
    assert!(!map.is_walkable(12, 13));
    let idx = map.pos_to_idx(12, 13).unwrap();
    assert_eq!(map.terrain_at_idx(idx), Some(TerrainType::Grass));
}

#[test]
fn seeded_index_tracks_existing_marker_removal() {
    let mut app = test_app();
    let marker = app
        .world_mut()
        .spawn((
            ObstaclePosition(11, 12),
            ObstacleSourceKind::NaturalTerrainClearing,
        ))
        .id();
    seed_obstacle_position_index(app.world_mut());
    app.world_mut()
        .resource_mut::<WorldMap>()
        .add_grid_obstacle((11, 12));

    app.world_mut()
        .entity_mut(marker)
        .remove::<ObstaclePosition>();
    app.update();

    assert!(app.world().resource::<WorldMap>().is_walkable(11, 12));
}

#[test]
fn deferred_removal_reaches_pathfinding_in_the_same_update() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(WorldMap::default())
        .init_resource::<ObstaclePositionIndex>()
        .add_message::<TerrainChangedEvent>()
        .init_resource::<PathfindingProbe>()
        .configure_sets(Update, (MarkerMutationSet, ObstacleSyncSet).chain())
        .add_systems(Update, queue_marker_removal.in_set(MarkerMutationSet))
        .add_systems(
            Update,
            ApplyDeferred
                .after(MarkerMutationSet)
                .before(ObstacleSyncSet),
        )
        .add_systems(Update, obstacle_sync_system.in_set(ObstacleSyncSet))
        .add_systems(Update, record_pathfinding_input.after(ObstacleSyncSet));

    let marker = app
        .world_mut()
        .spawn((
            ObstaclePosition(13, 14),
            ObstacleSourceKind::NaturalTerrainClearing,
        ))
        .id();
    seed_obstacle_position_index(app.world_mut());
    app.world_mut()
        .resource_mut::<WorldMap>()
        .add_grid_obstacle((13, 14));
    app.insert_resource(PendingMarkerRemoval(Some(marker)));

    app.update();

    assert_eq!(app.world().resource::<PathfindingProbe>().0, Some(true));
    assert_eq!(
        app.world().resource::<WorldMap>().terrain_at_idx(
            app.world()
                .resource::<WorldMap>()
                .pos_to_idx(13, 14)
                .unwrap()
        ),
        Some(TerrainType::Dirt)
    );
}
