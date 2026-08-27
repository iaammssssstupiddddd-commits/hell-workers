use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;

pub use hw_world::SpatialGridOps;

/// 汎用的なグリッドデータ構造
#[derive(Clone)]
pub struct GridData {
    pub cell_size: f32,
    pub grid: HashMap<(i32, i32), HashSet<Entity>>,
    pub positions: HashMap<Entity, Vec2>,
}

impl Default for GridData {
    fn default() -> Self {
        Self::new(32.0 * 20.0) // 640px - マップ全体の数分の一程度
    }
}

impl GridData {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            grid: HashMap::default(),
            positions: HashMap::default(),
        }
    }

    pub fn pos_to_cell(&self, pos: Vec2) -> (i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
        )
    }

    pub fn insert(&mut self, entity: Entity, pos: Vec2) {
        let cell = self.pos_to_cell(pos);
        self.grid.entry(cell).or_default().insert(entity);
        self.positions.insert(entity, pos);
    }

    pub fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        let mut results = Vec::new();
        self.get_nearby_in_radius_into(pos, radius, &mut results);
        results
    }

    pub fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>) {
        self.get_nearby_in_radius_observed(pos, radius, out, &mut ());
    }

    /// Runs the radius query and returns exact work counters for profiling.
    ///
    /// Callers aggregate the returned value in their own `Local` state. The
    /// grid intentionally does not own shared mutable metrics, so independent
    /// spatial readers remain parallelizable.
    pub fn get_nearby_in_radius_with_stats_into(
        &self,
        pos: Vec2,
        radius: f32,
        out: &mut Vec<Entity>,
    ) -> SpatialQueryStats {
        let mut stats = SpatialQueryStats::default();
        self.get_nearby_in_radius_observed(pos, radius, out, &mut stats);
        stats
    }

    #[inline]
    fn get_nearby_in_radius_observed<Observer: RadiusQueryObserver>(
        &self,
        pos: Vec2,
        radius: f32,
        out: &mut Vec<Entity>,
        observer: &mut Observer,
    ) {
        out.clear();
        observer.query_started();
        let Some((min_cell, max_cell)) = self.radius_cell_bounds(pos, radius) else {
            observer.invalid_query();
            return;
        };
        let radius_squared = radius * radius;

        for y in min_cell.1..=max_cell.1 {
            for x in min_cell.0..=max_cell.0 {
                observer.coordinate_probed();
                let cell = (x, y);
                if let Some(entities) = self.grid.get(&cell) {
                    observer.occupied_bucket();
                    for &entity in entities {
                        observer.bucket_member_examined();
                        if let Some(&entity_pos) = self.positions.get(&entity) {
                            if pos.distance_squared(entity_pos) <= radius_squared {
                                observer.exact_hit();
                                out.push(entity);
                            }
                        } else {
                            observer.position_fallback();
                        }
                    }
                }
            }
        }
    }

    fn radius_cell_bounds(&self, pos: Vec2, radius: f32) -> Option<((i32, i32), (i32, i32))> {
        if !self.cell_size.is_finite()
            || self.cell_size <= 0.0
            || !pos.x.is_finite()
            || !pos.y.is_finite()
            || !radius.is_finite()
            || radius < 0.0
        {
            return None;
        }
        let extent = Vec2::splat(radius);
        Some((
            self.pos_to_cell(pos - extent),
            self.pos_to_cell(pos + extent),
        ))
    }

    /// 矩形範囲内のエンティティを返す
    pub fn get_in_area(&self, min: Vec2, max: Vec2) -> Vec<Entity> {
        let mut results = Vec::new();
        let min_cell = self.pos_to_cell(min);
        let max_cell = self.pos_to_cell(max);

        for dy in min_cell.1..=max_cell.1 {
            for dx in min_cell.0..=max_cell.0 {
                let cell = (dx, dy);
                if let Some(entities) = self.grid.get(&cell) {
                    for &entity in entities {
                        if let Some(&pos) = self.positions.get(&entity)
                            && pos.x >= min.x
                            && pos.x <= max.x
                            && pos.y >= min.y
                            && pos.y <= max.y
                        {
                            results.push(entity);
                        }
                    }
                }
            }
        }
        results
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some(pos) = self.positions.remove(&entity) {
            let cell = self.pos_to_cell(pos);
            if let Some(entities) = self.grid.get_mut(&cell) {
                entities.remove(&entity);
                if entities.is_empty() {
                    self.grid.remove(&cell);
                }
            }
        }
    }

    pub fn update(&mut self, entity: Entity, new_pos: Vec2) {
        if let Some(&old_pos) = self.positions.get(&entity) {
            if old_pos == new_pos {
                return;
            }

            let old_cell = self.pos_to_cell(old_pos);
            let new_cell = self.pos_to_cell(new_pos);

            if old_cell == new_cell {
                // セルが変わらない場合は位置情報のみ更新（高速パス）
                self.positions.insert(entity, new_pos);
            } else {
                // セルが変わる場合は移動処理
                if let Some(entities) = self.grid.get_mut(&old_cell) {
                    entities.remove(&entity);
                    if entities.is_empty() {
                        self.grid.remove(&old_cell);
                    }
                }
                self.grid.entry(new_cell).or_default().insert(entity);
                self.positions.insert(entity, new_pos);
            }
        } else {
            // 新規登録
            self.insert(entity, new_pos);
        }
    }

    pub fn clear(&mut self) {
        self.grid.clear();
        self.positions.clear();
    }
}

/// Exact work performed by one or more spatial radius queries.
///
/// The counters distinguish empty coordinate probes from occupied buckets and
/// exact hits. This is the evidence needed to compare cell layouts without
/// treating the final result count as the amount of index work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpatialQueryStats {
    pub queries: u64,
    pub invalid_queries: u64,
    pub coordinate_probes: u64,
    pub occupied_buckets: u64,
    pub bucket_members_examined: u64,
    pub exact_hits: u64,
    pub position_fallbacks: u64,
}

impl SpatialQueryStats {
    pub fn merge(&mut self, other: Self) {
        self.queries = self.queries.saturating_add(other.queries);
        self.invalid_queries = self.invalid_queries.saturating_add(other.invalid_queries);
        self.coordinate_probes = self
            .coordinate_probes
            .saturating_add(other.coordinate_probes);
        self.occupied_buckets = self.occupied_buckets.saturating_add(other.occupied_buckets);
        self.bucket_members_examined = self
            .bucket_members_examined
            .saturating_add(other.bucket_members_examined);
        self.exact_hits = self.exact_hits.saturating_add(other.exact_hits);
        self.position_fallbacks = self
            .position_fallbacks
            .saturating_add(other.position_fallbacks);
    }
}

trait RadiusQueryObserver {
    #[inline]
    fn query_started(&mut self) {}
    #[inline]
    fn invalid_query(&mut self) {}
    #[inline]
    fn coordinate_probed(&mut self) {}
    #[inline]
    fn occupied_bucket(&mut self) {}
    #[inline]
    fn bucket_member_examined(&mut self) {}
    #[inline]
    fn exact_hit(&mut self) {}
    #[inline]
    fn position_fallback(&mut self) {}
}

impl RadiusQueryObserver for () {}

impl RadiusQueryObserver for SpatialQueryStats {
    #[inline]
    fn query_started(&mut self) {
        self.queries = self.queries.saturating_add(1);
    }

    #[inline]
    fn invalid_query(&mut self) {
        self.invalid_queries = self.invalid_queries.saturating_add(1);
    }

    #[inline]
    fn coordinate_probed(&mut self) {
        self.coordinate_probes = self.coordinate_probes.saturating_add(1);
    }

    #[inline]
    fn occupied_bucket(&mut self) {
        self.occupied_buckets = self.occupied_buckets.saturating_add(1);
    }

    #[inline]
    fn bucket_member_examined(&mut self) {
        self.bucket_members_examined = self.bucket_members_examined.saturating_add(1);
    }

    #[inline]
    fn exact_hit(&mut self) {
        self.exact_hits = self.exact_hits.saturating_add(1);
    }

    #[inline]
    fn position_fallback(&mut self) {
        self.position_fallbacks = self.position_fallbacks.saturating_add(1);
    }
}

/// A type-separated spatial index backed by the common grid storage.
///
/// Tags are owned by this crate rather than by the domain crates that own the
/// tracked components. This keeps the spatial crate independent from its
/// downstream users while preserving Bevy Resource separation per index.
#[derive(Resource)]
pub struct SpatialIndex<Tag> {
    data: GridData,
    generation: u64,
    marker: PhantomData<fn() -> Tag>,
}

impl<Tag> Default for SpatialIndex<Tag> {
    fn default() -> Self {
        Self {
            data: GridData::default(),
            generation: 0,
            marker: PhantomData,
        }
    }
}

impl<Tag> SpatialIndex<Tag> {
    /// Creates an index with caller-supplied grid storage.
    ///
    /// This keeps custom cell sizing available to callers without exposing the
    /// tag marker that separates Bevy resources.
    #[must_use]
    pub fn new(data: GridData) -> Self {
        Self {
            data,
            generation: 0,
            marker: PhantomData,
        }
    }

    /// Returns the underlying grid data for read-only inspection.
    pub fn data(&self) -> &GridData {
        &self.data
    }

    /// Returns the underlying grid data for explicit grid configuration.
    pub fn data_mut(&mut self) -> &mut GridData {
        &mut self.data
    }

    /// Semantic generation used by readers that cache search results.
    ///
    /// It advances only when membership or the recorded position changes.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Consumes this index and returns its grid data.
    #[must_use]
    pub fn into_data(self) -> GridData {
        self.data
    }

    /// Returns the entities whose recorded positions are inside the rectangle.
    pub fn get_in_area(&self, min: Vec2, max: Vec2) -> Vec<Entity> {
        self.data.get_in_area(min, max)
    }

    /// Runs a radius query and returns exact grid work counters.
    ///
    /// This is an inherent profiling API rather than part of `SpatialGridOps`:
    /// domain-independent consumers keep the minimal trait while concrete
    /// runtime owners can opt into caller-local instrumentation.
    pub fn get_nearby_in_radius_with_stats_into(
        &self,
        pos: Vec2,
        radius: f32,
        out: &mut Vec<Entity>,
    ) -> SpatialQueryStats {
        self.data
            .get_nearby_in_radius_with_stats_into(pos, radius, out)
    }
}

impl<Tag> From<GridData> for SpatialIndex<Tag> {
    fn from(data: GridData) -> Self {
        Self::new(data)
    }
}

impl<Tag: Send + Sync + 'static> SpatialGridOps for SpatialIndex<Tag> {
    fn insert(&mut self, entity: Entity, pos: Vec2) {
        let changed = self.data.positions.get(&entity).copied() != Some(pos);
        self.data.insert(entity, pos);
        if changed {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    fn remove(&mut self, entity: Entity) {
        let changed = self.data.positions.contains_key(&entity);
        self.data.remove(entity);
        if changed {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    fn update(&mut self, entity: Entity, pos: Vec2) {
        let changed = self.data.positions.get(&entity).copied() != Some(pos);
        self.data.update(entity, pos);
        if changed {
            self.generation = self.generation.wrapping_add(1);
        }
    }

    fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        self.data.get_nearby_in_radius(pos, radius)
    }

    fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>) {
        self.data.get_nearby_in_radius_into(pos, radius, out);
    }
}

/// Query shape for index families whose position source is `Transform`.
pub type TransformSpatialUpdateQuery<'w, 's, Tracked> = Query<
    'w,
    's,
    (Entity, &'static Transform),
    (With<Tracked>, Or<(Added<Tracked>, Changed<Transform>)>),
>;

/// Synchronizes a standard Transform-backed index from component changes.
///
/// Resource and Gathering indexes intentionally do not use this system because
/// their visibility and center-coordinate policies differ from the standard
/// Transform contract.
pub fn update_transform_spatial_index_system<Tag, Tracked>(
    mut index: ResMut<SpatialIndex<Tag>>,
    query: TransformSpatialUpdateQuery<Tracked>,
    mut removed: RemovedComponents<Tracked>,
) where
    Tag: Send + Sync + 'static,
    Tracked: Component,
{
    for (entity, transform) in query.iter() {
        index.update(entity, transform.translation.truncate());
    }

    for entity in removed.read() {
        index.remove(entity);
    }
}

/// Tag for DamnedSoul positions.
pub struct SoulIndexTag;
/// Tag for Familiar positions.
pub struct FamiliarIndexTag;
/// Tag for Designation positions.
pub struct DesignationIndexTag;
/// Tag for Blueprint positions.
pub struct BlueprintIndexTag;
/// Tag for FloorConstructionSite positions.
pub struct FloorConstructionIndexTag;
/// Tag for Stockpile positions.
pub struct StockpileIndexTag;
/// Tag for TransportRequest positions.
pub struct TransportRequestIndexTag;
/// Tag for ResourceItem positions.
pub struct ResourceIndexTag;
/// Tag for GatheringSpot centers.
pub struct GatheringSpotIndexTag;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct Tracked;

    struct FirstTag;
    struct SecondTag;

    #[test]
    fn tags_keep_spatial_resources_separate() {
        let mut app = App::new();
        app.init_resource::<SpatialIndex<FirstTag>>()
            .init_resource::<SpatialIndex<SecondTag>>()
            .add_systems(
                Update,
                update_transform_spatial_index_system::<FirstTag, Tracked>,
            );

        let entity = app
            .world_mut()
            .spawn((Tracked, Transform::from_xyz(16.0, 0.0, 0.0)))
            .id();
        app.update();

        assert_eq!(
            app.world()
                .resource::<SpatialIndex<FirstTag>>()
                .get_nearby_in_radius(Vec2::new(16.0, 0.0), 1.0),
            vec![entity]
        );
        assert!(
            app.world()
                .resource::<SpatialIndex<SecondTag>>()
                .get_nearby_in_radius(Vec2::new(16.0, 0.0), 1.0)
                .is_empty()
        );
    }

    #[test]
    fn index_keeps_custom_grid_data_available() {
        let mut index = SpatialIndex::<FirstTag>::new(GridData::new(48.0));
        assert_eq!(index.data().cell_size, 48.0);

        index.data_mut().clear();
        assert_eq!(index.into_data().cell_size, 48.0);
    }

    #[test]
    fn radius_bounds_visit_only_cells_intersecting_the_circle_aabb() {
        let grid = GridData::new(640.0);

        assert_eq!(
            grid.radius_cell_bounds(Vec2::new(320.0, 320.0), 48.0),
            Some(((0, 0), (0, 0)))
        );
        assert_eq!(
            grid.radius_cell_bounds(Vec2::new(620.0, 620.0), 48.0),
            Some(((0, 0), (1, 1)))
        );
        assert_eq!(
            grid.radius_cell_bounds(Vec2::new(-320.0, -320.0), 48.0),
            Some(((-1, -1), (-1, -1)))
        );
        assert_eq!(
            grid.radius_cell_bounds(Vec2::new(-20.0, -20.0), 48.0),
            Some(((-1, -1), (0, 0)))
        );
    }

    #[test]
    fn radius_query_includes_distance_boundary_and_rejects_invalid_shapes() {
        let mut grid = GridData::new(640.0);
        let boundary = Entity::from_bits(1);
        let outside = Entity::from_bits(2);
        grid.insert(boundary, Vec2::new(48.0, 0.0));
        grid.insert(outside, Vec2::new(48.01, 0.0));

        assert_eq!(grid.get_nearby_in_radius(Vec2::ZERO, 48.0), vec![boundary]);
        assert!(grid.get_nearby_in_radius(Vec2::ZERO, -1.0).is_empty());
        assert!(grid.get_nearby_in_radius(Vec2::ZERO, f32::NAN).is_empty());
        assert!(
            grid.get_nearby_in_radius(Vec2::ZERO, f32::INFINITY)
                .is_empty()
        );
        assert!(
            GridData::new(0.0)
                .get_nearby_in_radius(Vec2::ZERO, 48.0)
                .is_empty()
        );
    }

    #[test]
    fn radius_query_stats_separate_probes_members_hits_and_missing_positions() {
        let mut grid = GridData::new(64.0);
        let hit = Entity::from_bits(1);
        let miss = Entity::from_bits(2);
        let outside = Entity::from_bits(3);
        let missing_position = Entity::from_bits(4);
        grid.insert(hit, Vec2::new(8.0, 8.0));
        grid.insert(miss, Vec2::new(56.0, 56.0));
        grid.insert(outside, Vec2::new(-24.0, -24.0));
        grid.grid
            .entry((0, 0))
            .or_default()
            .insert(missing_position);

        let mut results = Vec::new();
        let stats =
            grid.get_nearby_in_radius_with_stats_into(Vec2::new(8.0, 8.0), 32.0, &mut results);

        assert_eq!(results, vec![hit]);
        assert_eq!(
            stats,
            SpatialQueryStats {
                queries: 1,
                invalid_queries: 0,
                coordinate_probes: 4,
                occupied_buckets: 2,
                bucket_members_examined: 4,
                exact_hits: 1,
                position_fallbacks: 1,
            }
        );
    }

    #[test]
    fn radius_query_stats_merge_and_count_invalid_queries() {
        let grid = GridData::new(64.0);
        let mut results = vec![Entity::from_bits(99)];
        let invalid = grid.get_nearby_in_radius_with_stats_into(Vec2::ZERO, f32::NAN, &mut results);
        let valid = grid.get_nearby_in_radius_with_stats_into(Vec2::ZERO, 0.0, &mut results);
        let mut total = invalid;
        total.merge(valid);

        assert!(results.is_empty());
        assert_eq!(total.queries, 2);
        assert_eq!(total.invalid_queries, 1);
        assert_eq!(total.coordinate_probes, 1);
        assert_eq!(total.occupied_buckets, 0);
    }

    #[test]
    fn cell_widths_and_hybrid_path_return_the_same_radius_membership() {
        let fixtures = [
            Vec2::new(-600.0, -40.0),
            Vec2::new(-128.0, 0.0),
            Vec2::new(-48.0, 0.0),
            Vec2::ZERO,
            Vec2::new(48.0, 0.0),
            Vec2::new(127.0, 1.0),
            Vec2::new(900.0, 900.0),
        ];
        let queries = [
            (Vec2::ZERO, 48.0),
            (Vec2::new(32.0, 16.0), 240.0),
            (Vec2::ZERO, 5_120.0),
        ];
        let mut expected = None;

        for cell_size in [64.0, 128.0, 256.0, 640.0] {
            let mut grid = GridData::new(cell_size);
            for (index, position) in fixtures.into_iter().enumerate() {
                grid.insert(Entity::from_bits(index as u64 + 1), position);
            }
            let mut observed = Vec::new();
            for (center, radius) in queries {
                let mut entities = grid.get_nearby_in_radius(center, radius);
                entities.sort_unstable_by_key(|entity| entity.to_bits());
                observed.push(entities);
            }

            if let Some(expected) = &expected {
                assert_eq!(&observed, expected, "cell_size={cell_size}");
            } else {
                expected = Some(observed);
            }
        }
    }

    #[test]
    fn transform_updater_tracks_add_move_and_component_removal() {
        let mut app = App::new();
        app.init_resource::<SpatialIndex<FirstTag>>().add_systems(
            Update,
            update_transform_spatial_index_system::<FirstTag, Tracked>,
        );

        let entity = app
            .world_mut()
            .spawn((Tracked, Transform::from_xyz(16.0, 0.0, 0.0)))
            .id();
        app.update();
        assert_eq!(
            app.world()
                .resource::<SpatialIndex<FirstTag>>()
                .get_nearby_in_radius(Vec2::new(16.0, 0.0), 1.0),
            vec![entity]
        );

        app.world_mut()
            .entity_mut(entity)
            .get_mut::<Transform>()
            .expect("tracked entity has a Transform")
            .translation = Vec3::new(96.0, 0.0, 0.0);
        app.update();
        let index = app.world().resource::<SpatialIndex<FirstTag>>();
        assert!(
            index
                .get_nearby_in_radius(Vec2::new(16.0, 0.0), 1.0)
                .is_empty()
        );
        assert_eq!(
            index.get_nearby_in_radius(Vec2::new(96.0, 0.0), 1.0),
            vec![entity]
        );

        app.world_mut().entity_mut(entity).remove::<Tracked>();
        app.update();
        assert!(
            app.world()
                .resource::<SpatialIndex<FirstTag>>()
                .get_nearby_in_radius(Vec2::new(96.0, 0.0), 1.0)
                .is_empty()
        );
    }
}
