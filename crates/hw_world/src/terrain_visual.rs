//! テレイン系ビジュアルアセットハンドルと source-aware 障害物同期。

use crate::map::{WorldMap, WorldMapWrite};
use crate::terrain::TerrainType;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_jobs::construction::WallConstructionSite;
use hw_jobs::{Blueprint, Building, BuildingType, ObstaclePosition, ObstacleSourceKind};
use std::collections::{HashMap, HashSet};

/// 障害物除去によってテレインが変化したことを通知するメッセージ。
/// `bevy_app` 側の `terrain_material_sync_system` が受信してマテリアルを差し替える。
#[derive(Message, Clone)]
pub struct TerrainChangedEvent {
    pub idx: usize,
}

/// Runtime marker の旧位置と論理 owner を保持する差分 index。
///
/// `WorldMap` の building record と `BuildingFootprint` child marker は同じ
/// owner を指すため、ここでは別 blocker として数えない。`WorldMap` 自体は
/// direct semantic blocker の正本であり、この index は marker の旧値を失わずに
/// removal を解決するためだけに使う。
#[derive(Resource, Default)]
pub struct ObstaclePositionIndex {
    records: HashMap<Entity, ObstacleRecord>,
    grid_owners: HashMap<(i32, i32), HashMap<Entity, usize>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ObstacleRecord {
    grid: (i32, i32),
    owner: Entity,
    source: ObstacleSourceKind,
}

impl ObstaclePositionIndex {
    fn clear(&mut self) {
        self.records.clear();
        self.grid_owners.clear();
    }

    fn upsert(&mut self, marker: Entity, record: ObstacleRecord) -> Option<ObstacleRecord> {
        let previous = self.records.insert(marker, record);
        if let Some(previous) = previous {
            self.remove_owner(previous);
        }
        self.add_owner(record);
        previous
    }

    fn remove(&mut self, marker: Entity) -> Option<ObstacleRecord> {
        let record = self.records.remove(&marker)?;
        self.remove_owner(record);
        Some(record)
    }

    fn has_owners_at(&self, grid: (i32, i32)) -> bool {
        self.grid_owners
            .get(&grid)
            .is_some_and(|owners| !owners.is_empty())
    }

    fn add_owner(&mut self, record: ObstacleRecord) {
        *self
            .grid_owners
            .entry(record.grid)
            .or_default()
            .entry(record.owner)
            .or_default() += 1;
    }

    fn remove_owner(&mut self, record: ObstacleRecord) {
        let Some(owners) = self.grid_owners.get_mut(&record.grid) else {
            return;
        };
        let Some(count) = owners.get_mut(&record.owner) else {
            return;
        };
        *count -= 1;
        if *count == 0 {
            owners.remove(&record.owner);
        }
        if owners.is_empty() {
            self.grid_owners.remove(&record.grid);
        }
    }
}

type ObstacleMarkerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ObstaclePosition,
        &'static ObstacleSourceKind,
        Option<&'static ChildOf>,
    ),
>;

type ChangedObstacleMarkerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ObstaclePosition,
        &'static ObstacleSourceKind,
        Option<&'static ChildOf>,
    ),
    Or<(
        Added<ObstaclePosition>,
        Changed<ObstaclePosition>,
        Added<ObstacleSourceKind>,
        Changed<ObstacleSourceKind>,
        Added<ChildOf>,
        Changed<ChildOf>,
    )>,
>;

#[derive(SystemParam)]
pub struct ObstacleSyncParams<'w, 's> {
    world_map: WorldMapWrite<'w>,
    index: ResMut<'w, ObstaclePositionIndex>,
    removed_positions: RemovedComponents<'w, 's, ObstaclePosition>,
    removed_sources: RemovedComponents<'w, 's, ObstacleSourceKind>,
    removed_parents: RemovedComponents<'w, 's, ChildOf>,
    q_markers: ObstacleMarkerQuery<'w, 's>,
    q_changed_markers: ChangedObstacleMarkerQuery<'w, 's>,
    q_entities: Query<'w, 's, Entity>,
    q_buildings: Query<'w, 's, &'static Building>,
    q_blueprints: Query<'w, 's, &'static Blueprint>,
    q_wall_sites: Query<'w, 's, (), With<WallConstructionSite>>,
    #[cfg(debug_assertions)]
    q_source_less_markers:
        Query<'w, 's, Entity, (With<ObstaclePosition>, Without<ObstacleSourceKind>)>,
    ev_terrain_changed: MessageWriter<'w, TerrainChangedEvent>,
}

/// Seeds [`ObstaclePositionIndex`] after load rehydration has rebuilt runtime markers.
pub fn seed_obstacle_position_index(world: &mut World) {
    world.init_resource::<ObstaclePositionIndex>();

    let markers: Vec<ObstacleRecordWithMarker> = {
        let mut query = world.query::<(
            Entity,
            &ObstaclePosition,
            &ObstacleSourceKind,
            Option<&ChildOf>,
        )>();
        query
            .iter(world)
            .map(
                |(marker, position, source, parent)| ObstacleRecordWithMarker {
                    marker,
                    record: obstacle_record(marker, position, *source, parent),
                },
            )
            .collect()
    };

    let mut index = world.resource_mut::<ObstaclePositionIndex>();
    index.clear();
    for marker in markers {
        index.upsert(marker.marker, marker.record);
    }
}

struct ObstacleRecordWithMarker {
    marker: Entity,
    record: ObstacleRecord,
}

/// Applies added/changed markers before removed batches, preserving the old record
/// for source-specific removal policy. The resulting raw bitmap is ready before
/// the Actor/pathfinding phase; terrain visual mutation is emitted as a message.
pub fn obstacle_sync_system(params: ObstacleSyncParams) {
    let ObstacleSyncParams {
        mut world_map,
        mut index,
        mut removed_positions,
        mut removed_sources,
        mut removed_parents,
        q_markers,
        q_changed_markers,
        q_entities,
        q_buildings,
        q_blueprints,
        q_wall_sites,
        #[cfg(debug_assertions)]
        q_source_less_markers,
        mut ev_terrain_changed,
    } = params;
    #[cfg(debug_assertions)]
    debug_assert!(
        q_source_less_markers.iter().next().is_none(),
        "OBSTACLE: every ObstaclePosition must declare an ObstacleSourceKind"
    );
    let mut affected_grids = HashSet::new();
    let mut retired_records = Vec::new();

    for (marker, position, source, parent) in q_changed_markers.iter() {
        apply_marker_record(
            &mut index,
            marker,
            position,
            *source,
            parent,
            &mut affected_grids,
            &mut retired_records,
        );
    }

    // Removing ChildOf does not make the marker query Changed. Refresh the
    // owner in that case before handling actual marker removals.
    let parent_removed_markers: Vec<Entity> = removed_parents.read().collect();
    for marker in parent_removed_markers {
        let Ok((marker, position, source, parent)) = q_markers.get(marker) else {
            continue;
        };
        apply_marker_record(
            &mut index,
            marker,
            position,
            *source,
            parent,
            &mut affected_grids,
            &mut retired_records,
        );
    }

    let mut removed_markers = HashSet::new();
    removed_markers.extend(removed_positions.read());
    removed_markers.extend(removed_sources.read());
    for marker in removed_markers {
        // A component can be removed and reinserted within one deferred batch.
        // The current marker record wins in that case.
        if q_markers.get(marker).is_ok() {
            continue;
        }
        if let Some(record) = index.remove(marker) {
            affected_grids.insert(record.grid);
            retired_records.push(record);
        }
    }

    for grid in affected_grids.iter().copied() {
        let marker_blocks = index.has_owners_at(grid);
        let direct_blocker = has_live_direct_blocker(
            &world_map,
            grid,
            &q_entities,
            &q_buildings,
            &q_blueprints,
            &q_wall_sites,
        );

        if marker_blocks {
            // Completed building markers are intentional mirrors of the direct
            // WorldMap owner. Let the door/building API retain its raw-bit policy.
            if !direct_blocker {
                world_map.add_grid_obstacle(grid);
            }
        } else if !direct_blocker {
            world_map.remove_grid_obstacle(grid);
        }
    }

    let natural_removal_grids: HashSet<(i32, i32)> = retired_records
        .into_iter()
        .filter(|record| record.source.clears_terrain_on_removal())
        .map(|record| record.grid)
        .collect();
    for grid in natural_removal_grids {
        if index.has_owners_at(grid)
            || has_live_direct_blocker(
                &world_map,
                grid,
                &q_entities,
                &q_buildings,
                &q_blueprints,
                &q_wall_sites,
            )
        {
            continue;
        }

        if let Some(idx) = world_map.pos_to_idx(grid.0, grid.1)
            && world_map.terrain_at_idx(idx) != Some(TerrainType::Dirt)
        {
            world_map.set_terrain_at_idx(idx, TerrainType::Dirt);
            ev_terrain_changed.write(TerrainChangedEvent { idx });
        }
    }
}

fn apply_marker_record(
    index: &mut ObstaclePositionIndex,
    marker: Entity,
    position: &ObstaclePosition,
    source: ObstacleSourceKind,
    parent: Option<&ChildOf>,
    affected_grids: &mut HashSet<(i32, i32)>,
    retired_records: &mut Vec<ObstacleRecord>,
) {
    let record = obstacle_record(marker, position, source, parent);
    affected_grids.insert(record.grid);
    if let Some(previous) = index.upsert(marker, record) {
        affected_grids.insert(previous.grid);
        retired_records.push(previous);
    }
}

fn obstacle_record(
    marker: Entity,
    position: &ObstaclePosition,
    source: ObstacleSourceKind,
    parent: Option<&ChildOf>,
) -> ObstacleRecord {
    let owner = match source {
        ObstacleSourceKind::BuildingFootprint | ObstacleSourceKind::PlacementReservation => {
            parent.map(ChildOf::parent).unwrap_or_else(|| {
                warn!(
                    "OBSTACLE: {source:?} marker {marker:?} has no ChildOf; using marker as owner"
                );
                marker
            })
        }
        ObstacleSourceKind::NaturalTerrainClearing | ObstacleSourceKind::ConstructionProtection => {
            marker
        }
    };
    ObstacleRecord {
        grid: (position.0, position.1),
        owner,
        source,
    }
}

fn has_live_direct_blocker(
    world_map: &WorldMap,
    grid: (i32, i32),
    q_entities: &Query<Entity>,
    q_buildings: &Query<&Building>,
    q_blueprints: &Query<&Blueprint>,
    q_wall_sites: &Query<(), With<WallConstructionSite>>,
) -> bool {
    let Some(owner) = world_map.building_entity(grid) else {
        return false;
    };
    if q_entities.get(owner).is_err() {
        return false;
    }

    q_buildings
        .get(owner)
        .is_ok_and(|building| building.kind.blocks_movement())
        || q_blueprints
            .get(owner)
            .is_ok_and(|blueprint| blueprint.kind != BuildingType::Bridge)
        || q_wall_sites.get(owner).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{
        ObstaclePositionIndex, TerrainChangedEvent, obstacle_sync_system,
        seed_obstacle_position_index,
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
}
