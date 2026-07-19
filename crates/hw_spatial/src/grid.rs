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
        out.clear();
        let cell_radius = (radius / self.cell_size).ceil() as i32;
        let center_cell = self.pos_to_cell(pos);

        for dy in -cell_radius..=cell_radius {
            for dx in -cell_radius..=cell_radius {
                let cell = (center_cell.0 + dx, center_cell.1 + dy);
                if let Some(entities) = self.grid.get(&cell) {
                    for &entity in entities {
                        if let Some(&entity_pos) = self.positions.get(&entity)
                            && pos.distance(entity_pos) <= radius
                        {
                            out.push(entity);
                        }
                    }
                }
            }
        }
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
