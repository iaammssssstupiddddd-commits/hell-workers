use crate::handles::WallVisualHandles;
use crate::layer::VisualLayerKind;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::visual_mirror::building::{BuildingTypeVisual, BuildingVisualState};
use hw_core::visual_mirror::construction::{BlueprintVisualState, WallTileVisualMirror};
use hw_world::WorldMap;
use std::collections::{HashMap, HashSet};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WallTopologyResolveSet;

/// Four-neighbor Wall connector mask in the canonical `(N, S, W, E)` order.
/// N is the high bit so `bits()` matches the M0 fixture's four-character masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WallConnectionMask(u8);

impl WallConnectionMask {
    pub const fn from_neighbors(north: bool, south: bool, west: bool, east: bool) -> Self {
        Self(((north as u8) << 3) | ((south as u8) << 2) | ((west as u8) << 1) | east as u8)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WallMeshFamily {
    Isolated,
    End,
    Straight,
    Corner,
    TJunction,
    Cross,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuarterTurns(u8);

impl QuarterTurns {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1);
    pub const TWO: Self = Self(2);
    pub const THREE: Self = Self(3);

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResolvedWallTopology {
    pub family: WallMeshFamily,
    pub quarter_turns_y: QuarterTurns,
}

/// Maps every canonical connector mask to one of the six production meshes.
/// Positive quarter turns rotate around +Y and appear counterclockwise from above.
pub const fn resolve_wall_topology(mask: WallConnectionMask) -> ResolvedWallTopology {
    use QuarterTurns as Q;
    use WallMeshFamily as F;

    let (family, quarter_turns_y) = match mask.bits() {
        0b0000 => (F::Isolated, Q::ZERO),
        0b1000 => (F::End, Q::ZERO),
        0b0100 => (F::End, Q::TWO),
        0b0010 => (F::End, Q::ONE),
        0b0001 => (F::End, Q::THREE),
        0b1100 => (F::Straight, Q::ZERO),
        0b0011 => (F::Straight, Q::ONE),
        0b1010 => (F::Corner, Q::ZERO),
        0b1001 => (F::Corner, Q::THREE),
        0b0110 => (F::Corner, Q::ONE),
        0b0101 => (F::Corner, Q::TWO),
        0b1110 => (F::TJunction, Q::ZERO),
        0b1101 => (F::TJunction, Q::TWO),
        0b1011 => (F::TJunction, Q::THREE),
        0b0111 => (F::TJunction, Q::ONE),
        0b1111 => (F::Cross, Q::ZERO),
        _ => unreachable!(),
    };
    ResolvedWallTopology {
        family,
        quarter_turns_y,
    }
}

type Grid = (i32, i32);

#[derive(Debug, Default)]
struct ContributorGrids {
    building: HashSet<Grid>,
    blueprint: HashSet<Grid>,
    wall_tile: HashSet<Grid>,
}

impl ContributorGrids {
    fn combined(&self) -> HashSet<Grid> {
        let mut combined = self.building.clone();
        combined.extend(self.blueprint.iter().copied());
        combined.extend(self.wall_tile.iter().copied());
        combined
    }
}

/// Incremental connector ownership shared by the 2D and 3D Wall consumers.
#[derive(Resource, Debug)]
pub struct WallTopologyIndex {
    by_entity: HashMap<Entity, ContributorGrids>,
    by_grid: HashMap<Grid, HashSet<Entity>>,
    resolved: HashMap<Grid, ResolvedWallTopology>,
    dirty_targets: HashSet<Grid>,
    last_update_targets: HashSet<Grid>,
    full_rebuild_requested: bool,
    pub resolution_revision: u64,
}

impl Default for WallTopologyIndex {
    fn default() -> Self {
        Self {
            by_entity: HashMap::new(),
            by_grid: HashMap::new(),
            resolved: HashMap::new(),
            dirty_targets: HashSet::new(),
            last_update_targets: HashSet::new(),
            full_rebuild_requested: true,
            resolution_revision: 0,
        }
    }
}

impl WallTopologyIndex {
    /// True after the current world has completed its mandatory full rebuild.
    pub fn is_ready(&self) -> bool {
        !self.full_rebuild_requested
    }

    /// Returns the current four-neighbor connector mask without consuming the
    /// Wall resolver's dirty set. Door presentation uses this read-only view.
    pub fn connection_mask(&self, grid: Grid) -> Option<WallConnectionMask> {
        self.has_connector(grid).then(|| {
            WallConnectionMask::from_neighbors(
                self.has_connector((grid.0, grid.1 + 1)),
                self.has_connector((grid.0, grid.1 - 1)),
                self.has_connector((grid.0 - 1, grid.1)),
                self.has_connector((grid.0 + 1, grid.1)),
            )
        })
    }

    fn replace_source(
        &mut self,
        entity: Entity,
        source: fn(&mut ContributorGrids) -> &mut HashSet<Grid>,
        grids: impl IntoIterator<Item = Grid>,
    ) {
        let old_combined = self
            .by_entity
            .get(&entity)
            .map_or_else(HashSet::new, ContributorGrids::combined);
        let entry = self.by_entity.entry(entity).or_default();
        *source(entry) = grids.into_iter().collect();
        let new_combined = entry.combined();
        if old_combined == new_combined {
            return;
        }
        for grid in old_combined.difference(&new_combined) {
            if let Some(contributors) = self.by_grid.get_mut(grid) {
                contributors.remove(&entity);
                if contributors.is_empty() {
                    self.by_grid.remove(grid);
                }
            }
        }
        for grid in new_combined.difference(&old_combined) {
            self.by_grid.entry(*grid).or_default().insert(entity);
        }
        for grid in old_combined.union(&new_combined) {
            add_neighbors_to_update(grid.0, grid.1, &mut self.dirty_targets);
        }
        if new_combined.is_empty() {
            self.by_entity.remove(&entity);
        }
    }

    fn set_building(&mut self, entity: Entity, grid: Option<Grid>) {
        self.replace_source(entity, |entry| &mut entry.building, grid);
    }

    fn set_blueprint(&mut self, entity: Entity, grids: impl IntoIterator<Item = Grid>) {
        self.replace_source(entity, |entry| &mut entry.blueprint, grids);
    }

    fn set_wall_tile(&mut self, entity: Entity, grid: Option<Grid>) {
        self.replace_source(entity, |entry| &mut entry.wall_tile, grid);
    }

    fn mark_dirty_around(&mut self, grids: impl IntoIterator<Item = Grid>) {
        for (x, y) in grids {
            add_neighbors_to_update(x, y, &mut self.dirty_targets);
        }
    }

    fn has_connector(&self, grid: Grid) -> bool {
        self.by_grid.get(&grid).is_some_and(|set| !set.is_empty())
    }

    fn contributors(&self, grid: Grid) -> Vec<Entity> {
        self.by_grid
            .get(&grid)
            .map_or_else(Vec::new, |set| set.iter().copied().collect())
    }

    fn take_dirty(&mut self) -> HashSet<Grid> {
        std::mem::take(&mut self.dirty_targets)
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallTopologyState {
    pub grid: Grid,
    pub mask: WallConnectionMask,
    pub resolved: ResolvedWallTopology,
    pub revision: u64,
}

/// Runtime wake-up for wall/door removals whose visual mirror disappears
/// before the regular `Changed<BuildingVisualState>` query can observe it.
#[derive(Resource, Debug, Default)]
pub struct WallConnectionDirty {
    removed_grids: HashSet<(i32, i32)>,
}

impl WallConnectionDirty {
    pub fn mark_removed<I>(&mut self, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        self.removed_grids.extend(grids);
    }

    fn take_removed(&mut self) -> HashSet<(i32, i32)> {
        std::mem::take(&mut self.removed_grids)
    }
}

type ChangedBuildingQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, &'static BuildingVisualState),
    Or<(
        Added<BuildingVisualState>,
        Changed<BuildingVisualState>,
        Changed<Transform>,
    )>,
>;

type WallCheckQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static BuildingVisualState>,
        Option<&'static BlueprintVisualState>,
    ),
    Or<(With<BuildingVisualState>, With<BlueprintVisualState>)>,
>;

type ChangedBlueprintQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, &'static BlueprintVisualState),
    Changed<BlueprintVisualState>,
>;

type ChangedWallTileQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform),
    (
        With<WallTileVisualMirror>,
        Or<(
            Added<WallTileVisualMirror>,
            Changed<WallTileVisualMirror>,
            Changed<Transform>,
        )>,
    ),
>;

#[derive(SystemParam)]
pub struct WallTopologyQueries<'w, 's> {
    q_all_buildings: Query<'w, 's, (Entity, &'static Transform, &'static BuildingVisualState)>,
    q_changed_buildings: ChangedBuildingQuery<'w, 's>,
    q_all_blueprints: Query<'w, 's, (Entity, &'static BlueprintVisualState)>,
    q_changed_blueprints: ChangedBlueprintQuery<'w, 's>,
    q_all_wall_tiles: Query<'w, 's, (Entity, &'static Transform), With<WallTileVisualMirror>>,
    q_changed_wall_tiles: ChangedWallTileQuery<'w, 's>,
    q_walls_check: WallCheckQuery<'w, 's>,
    removed_buildings: RemovedComponents<'w, 's, BuildingVisualState>,
    removed_blueprints: RemovedComponents<'w, 's, BlueprintVisualState>,
    removed_wall_tiles: RemovedComponents<'w, 's, WallTileVisualMirror>,
}

#[derive(SystemParam)]
pub struct WallConnectionQueries<'w, 's> {
    q_children: Query<'w, 's, &'static Children>,
    q_visual_layers: Query<'w, 's, (&'static VisualLayerKind, &'static mut Sprite)>,
    q_blueprint_sprites: Query<'w, 's, &'static mut Sprite, Without<VisualLayerKind>>,
}

/// 壁の接続更新を行うシステム
pub fn wall_connections_system(
    mut commands: Commands,
    wall_handles: Res<WallVisualHandles>,
    mut topology: WallTopologyQueries,
    mut dirty: ResMut<WallConnectionDirty>,
    mut index: ResMut<WallTopologyIndex>,
    mut queries: WallConnectionQueries,
) {
    for entity in topology.removed_buildings.read() {
        index.set_building(entity, None);
        commands.entity(entity).try_remove::<WallTopologyState>();
    }
    for entity in topology.removed_blueprints.read() {
        index.set_blueprint(entity, []);
        commands.entity(entity).try_remove::<WallTopologyState>();
    }
    for entity in topology.removed_wall_tiles.read() {
        index.set_wall_tile(entity, None);
    }
    if index.full_rebuild_requested {
        for (entity, transform, building_visual) in topology.q_all_buildings.iter() {
            let grid = matches!(
                building_visual.kind,
                BuildingTypeVisual::Wall | BuildingTypeVisual::Door
            )
            .then(|| WorldMap::world_to_grid(transform.translation.truncate()));
            index.set_building(entity, grid);
            if building_visual.kind != BuildingTypeVisual::Wall {
                commands.entity(entity).try_remove::<WallTopologyState>();
            }
        }
        for (entity, state) in topology.q_all_blueprints.iter() {
            let grids = if state.is_wall_or_door {
                state.occupied_grids.clone()
            } else {
                Vec::new()
            };
            index.set_blueprint(entity, grids);
            if !state.is_plain_wall {
                commands.entity(entity).try_remove::<WallTopologyState>();
            }
        }
        for (entity, transform) in topology.q_all_wall_tiles.iter() {
            index.set_wall_tile(
                entity,
                Some(WorldMap::world_to_grid(transform.translation.truncate())),
            );
        }
        index.full_rebuild_requested = false;
    } else {
        for (entity, transform, building_visual) in topology.q_changed_buildings.iter() {
            let grid = matches!(
                building_visual.kind,
                BuildingTypeVisual::Wall | BuildingTypeVisual::Door
            )
            .then(|| WorldMap::world_to_grid(transform.translation.truncate()));
            index.set_building(entity, grid);
            if building_visual.kind != BuildingTypeVisual::Wall {
                commands.entity(entity).try_remove::<WallTopologyState>();
            }
        }
        for (entity, _transform, state) in topology.q_changed_blueprints.iter() {
            let grids = if state.is_wall_or_door {
                state.occupied_grids.clone()
            } else {
                Vec::new()
            };
            index.set_blueprint(entity, grids);
            if !state.is_plain_wall {
                commands.entity(entity).try_remove::<WallTopologyState>();
            }
        }
        for (entity, transform) in topology.q_changed_wall_tiles.iter() {
            index.set_wall_tile(
                entity,
                Some(WorldMap::world_to_grid(transform.translation.truncate())),
            );
        }
    }
    index.mark_dirty_around(dirty.take_removed());

    let update_targets = index.take_dirty();
    index.last_update_targets = update_targets.clone();
    if update_targets.is_empty() {
        return;
    }
    index.resolution_revision = index.resolution_revision.wrapping_add(1);
    let revision = index.resolution_revision;

    for (gx, gy) in update_targets {
        let grid = (gx, gy);
        if !index.has_connector(grid) {
            index.resolved.remove(&grid);
            continue;
        }
        let mask = WallConnectionMask::from_neighbors(
            index.has_connector((gx, gy + 1)),
            index.has_connector((gx, gy - 1)),
            index.has_connector((gx - 1, gy)),
            index.has_connector((gx + 1, gy)),
        );
        let resolved = resolve_wall_topology(mask);
        index.resolved.insert(grid, resolved);
        for entity in index.contributors(grid) {
            let is_plain_wall = topology.q_walls_check.get(entity).ok().is_some_and(
                |(building_visual_opt, blueprint_opt)| {
                    building_visual_opt.is_some_and(|v| v.kind == BuildingTypeVisual::Wall)
                        || blueprint_opt.is_some_and(|s| s.is_plain_wall)
                },
            );
            if !is_plain_wall {
                continue;
            }
            commands.entity(entity).try_insert(WallTopologyState {
                grid,
                mask,
                resolved,
                revision,
            });

            // 完成した Building は Sprite を VisualLayerKind::Struct 子エンティティに持つ
            let mut updated = false;
            if let Ok(children) = queries.q_children.get(entity) {
                for child in children.iter() {
                    if let Ok((kind, mut sprite)) = queries.q_visual_layers.get_mut(child)
                        && *kind == VisualLayerKind::Struct
                    {
                        update_wall_sprite(
                            entity,
                            mask,
                            &mut sprite,
                            &topology.q_walls_check,
                            &wall_handles,
                        );
                        updated = true;
                        break;
                    }
                }
            }
            // Blueprint エンティティは Sprite を直接持つ
            if !updated && let Ok(mut sprite) = queries.q_blueprint_sprites.get_mut(entity) {
                update_wall_sprite(
                    entity,
                    mask,
                    &mut sprite,
                    &topology.q_walls_check,
                    &wall_handles,
                );
            }
        }
    }
}

fn add_neighbors_to_update(x: i32, y: i32, targets: &mut HashSet<(i32, i32)>) {
    targets.insert((x, y));
    targets.insert((x, y + 1));
    targets.insert((x, y - 1));
    targets.insert((x + 1, y));
    targets.insert((x - 1, y));
}

fn update_wall_sprite(
    wall_entity: Entity,
    mask: WallConnectionMask,
    sprite: &mut Sprite,
    q_walls_check: &WallCheckQuery<'_, '_>,
    wall_handles: &WallVisualHandles,
) {
    let mask = mask.bits();

    let is_provisional = is_provisional_wall(wall_entity, q_walls_check);

    let (texture, flip_x, flip_y) = if is_provisional {
        match mask {
            0b0000 => (wall_handles.stone_isolated.clone(), false, false),
            0b0010 => (wall_handles.stone_horizontal_left.clone(), false, false),
            0b0001 => (wall_handles.stone_horizontal_right.clone(), false, false),
            0b0011 => (wall_handles.stone_horizontal_both.clone(), false, false),
            0b1000 => (wall_handles.stone_vertical_top.clone(), false, false),
            0b0100 => (wall_handles.stone_vertical_bottom.clone(), false, false),
            0b1100 => (wall_handles.stone_vertical_both.clone(), false, false),
            0b1010 => (wall_handles.stone_corner_tl.clone(), false, false),
            0b1001 => (wall_handles.stone_corner_tr.clone(), false, false),
            0b0110 => (wall_handles.stone_corner_bl.clone(), false, false),
            0b0101 => (wall_handles.stone_corner_br.clone(), false, false),
            0b1110 => (wall_handles.stone_t_left.clone(), false, false),
            0b1101 => (wall_handles.stone_t_right.clone(), false, false),
            0b1011 => (wall_handles.stone_t_up.clone(), false, false),
            0b0111 => (wall_handles.stone_t_down.clone(), false, false),
            0b1111 => (wall_handles.stone_cross.clone(), false, false),
            _ => unreachable!(),
        }
    } else {
        match mask {
            0b0000 => (wall_handles.mud_isolated.clone(), false, false),
            0b0010 => (wall_handles.mud_end_right.clone(), false, false),
            0b0001 => (wall_handles.mud_end_left.clone(), false, false),
            0b0011 => (wall_handles.mud_horizontal.clone(), false, false),
            0b1000 => (wall_handles.mud_end_bottom.clone(), false, false),
            0b0100 => (wall_handles.mud_end_top.clone(), false, false),
            0b1100 => (wall_handles.mud_vertical.clone(), false, false),
            0b1010 => (wall_handles.mud_corner_tl.clone(), false, false),
            0b1001 => (wall_handles.mud_corner_tr.clone(), false, false),
            0b0110 => (wall_handles.mud_corner_bl.clone(), false, false),
            0b0101 => (wall_handles.mud_corner_br.clone(), false, false),
            0b1110 => (wall_handles.mud_t_left.clone(), false, false),
            0b1101 => (wall_handles.mud_t_right.clone(), false, false),
            0b1011 => (wall_handles.mud_t_up.clone(), false, false),
            0b0111 => (wall_handles.mud_t_down.clone(), false, false),
            0b1111 => (wall_handles.mud_cross.clone(), false, false),
            _ => unreachable!(),
        }
    };

    sprite.image = texture;
    sprite.flip_x = flip_x;
    sprite.flip_y = flip_y;
    sprite.color = if is_provisional {
        Color::srgba(1.0, 0.75, 0.4, 0.85)
    } else {
        Color::WHITE
    };
}

fn is_provisional_wall(entity: Entity, q_walls_check: &WallCheckQuery<'_, '_>) -> bool {
    q_walls_check
        .get(entity)
        .ok()
        .and_then(|(visual_opt, _)| visual_opt)
        .is_some_and(|v| v.kind == BuildingTypeVisual::Wall && v.is_provisional)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_mask_order_is_north_south_west_east() {
        assert_eq!(
            WallConnectionMask::from_neighbors(true, false, false, false).bits(),
            0b1000
        );
        assert_eq!(
            WallConnectionMask::from_neighbors(false, true, false, false).bits(),
            0b0100
        );
        assert_eq!(
            WallConnectionMask::from_neighbors(false, false, true, false).bits(),
            0b0010
        );
        assert_eq!(
            WallConnectionMask::from_neighbors(false, false, false, true).bits(),
            0b0001
        );
    }

    #[test]
    fn all_sixteen_masks_match_the_sealed_geometry_fixture() {
        use QuarterTurns as Q;
        use WallMeshFamily as F;

        let expected = [
            (0b0000, F::Isolated, Q::ZERO),
            (0b1000, F::End, Q::ZERO),
            (0b0100, F::End, Q::TWO),
            (0b0010, F::End, Q::ONE),
            (0b0001, F::End, Q::THREE),
            (0b1100, F::Straight, Q::ZERO),
            (0b0011, F::Straight, Q::ONE),
            (0b1010, F::Corner, Q::ZERO),
            (0b1001, F::Corner, Q::THREE),
            (0b0110, F::Corner, Q::ONE),
            (0b0101, F::Corner, Q::TWO),
            (0b1110, F::TJunction, Q::ZERO),
            (0b1101, F::TJunction, Q::TWO),
            (0b1011, F::TJunction, Q::THREE),
            (0b0111, F::TJunction, Q::ONE),
            (0b1111, F::Cross, Q::ZERO),
        ];

        for (bits, family, quarter_turns_y) in expected {
            let mask = WallConnectionMask(bits);
            assert_eq!(
                resolve_wall_topology(mask),
                ResolvedWallTopology {
                    family,
                    quarter_turns_y,
                },
                "mask {bits:04b}"
            );
        }
        assert_eq!(
            expected
                .iter()
                .map(|(bits, _, _)| *bits)
                .collect::<HashSet<_>>()
                .len(),
            16
        );
    }

    fn test_handles(
        mud_isolated: Handle<Image>,
        mud_end_right: Handle<Image>,
    ) -> WallVisualHandles {
        let unused = Handle::default();
        let mud_end_bottom = mud_end_right.clone();
        WallVisualHandles {
            stone_isolated: unused.clone(),
            stone_horizontal_left: unused.clone(),
            stone_horizontal_right: unused.clone(),
            stone_horizontal_both: unused.clone(),
            stone_vertical_top: unused.clone(),
            stone_vertical_bottom: unused.clone(),
            stone_vertical_both: unused.clone(),
            stone_corner_tl: unused.clone(),
            stone_corner_tr: unused.clone(),
            stone_corner_bl: unused.clone(),
            stone_corner_br: unused.clone(),
            stone_t_up: unused.clone(),
            stone_t_down: unused.clone(),
            stone_t_left: unused.clone(),
            stone_t_right: unused.clone(),
            stone_cross: unused.clone(),
            door_closed: unused.clone(),
            door_open: unused.clone(),
            mud_isolated,
            mud_horizontal: unused.clone(),
            mud_vertical: unused.clone(),
            mud_corner_tl: unused.clone(),
            mud_corner_tr: unused.clone(),
            mud_corner_bl: unused.clone(),
            mud_corner_br: unused.clone(),
            mud_t_up: unused.clone(),
            mud_t_down: unused.clone(),
            mud_t_left: unused.clone(),
            mud_t_right: unused.clone(),
            mud_cross: unused.clone(),
            mud_end_top: unused.clone(),
            mud_end_bottom,
            mud_end_left: unused.clone(),
            mud_end_right,
            mud_floor: unused,
        }
    }

    fn spawn_completed_wall(app: &mut App, grid: (i32, i32)) -> (Entity, Entity) {
        let wall = app
            .world_mut()
            .spawn((
                Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
                BuildingVisualState {
                    kind: BuildingTypeVisual::Wall,
                    is_provisional: false,
                },
            ))
            .id();
        let sprite = app
            .world_mut()
            .spawn((VisualLayerKind::Struct, Sprite::default(), ChildOf(wall)))
            .id();
        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_building(grid, wall);
        (wall, sprite)
    }

    fn spawn_completed_door(app: &mut App, grid: Grid) -> Entity {
        app.world_mut()
            .spawn((
                Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
                BuildingVisualState {
                    kind: BuildingTypeVisual::Door,
                    is_provisional: false,
                },
            ))
            .id()
    }

    fn spawn_wall_tile(app: &mut App, grid: Grid) -> Entity {
        app.world_mut()
            .spawn((
                Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
                WallTileVisualMirror::default(),
            ))
            .id()
    }

    fn expected_dirty_around(grids: impl IntoIterator<Item = Grid>) -> HashSet<Grid> {
        let mut dirty = HashSet::new();
        for (x, y) in grids {
            add_neighbors_to_update(x, y, &mut dirty);
        }
        dirty
    }

    #[test]
    fn removed_wall_dirty_refreshes_surviving_neighbor_sprite() {
        let mut images = Assets::<Image>::default();
        let isolated = images.add(Image::default());
        let connected = images.add(Image::default());

        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<WallConnectionDirty>()
            .init_resource::<WallTopologyIndex>()
            .insert_resource(test_handles(isolated.clone(), connected.clone()))
            .add_systems(Update, wall_connections_system);

        let removed_grid = (10, 10);
        let survivor_grid = (11, 10);
        let (removed_wall, _) = spawn_completed_wall(&mut app, removed_grid);
        let (_, survivor_sprite) = spawn_completed_wall(&mut app, survivor_grid);

        app.update();
        assert_eq!(
            app.world().get::<Sprite>(survivor_sprite).unwrap().image,
            connected
        );

        assert!(
            app.world_mut()
                .resource_mut::<WorldMap>()
                .clear_building_if_owned(removed_grid, removed_wall)
        );
        app.world_mut().despawn(removed_wall);
        app.world_mut()
            .resource_mut::<WallConnectionDirty>()
            .mark_removed([removed_grid]);

        app.update();
        assert_eq!(
            app.world().get::<Sprite>(survivor_sprite).unwrap().image,
            isolated
        );
    }

    #[test]
    fn completed_door_contributes_to_the_canonical_north_mask() {
        let mut images = Assets::<Image>::default();
        let isolated = images.add(Image::default());
        let north_connected = images.add(Image::default());
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<WallConnectionDirty>()
            .init_resource::<WallTopologyIndex>()
            .insert_resource(test_handles(isolated, north_connected.clone()))
            .add_systems(Update, wall_connections_system);
        let target_grid = (12, 12);
        let north_grid = (12, 13);
        let (_, target_sprite) = spawn_completed_wall(&mut app, target_grid);
        let door = app
            .world_mut()
            .spawn((
                Transform::from_translation(
                    WorldMap::grid_to_world(north_grid.0, north_grid.1).extend(0.0),
                ),
                BuildingVisualState {
                    kind: BuildingTypeVisual::Door,
                    is_provisional: false,
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_building(north_grid, door);

        app.update();

        assert_eq!(
            app.world().get::<Sprite>(target_sprite).unwrap().image,
            north_connected
        );
        assert_eq!(
            resolve_wall_topology(WallConnectionMask::from_neighbors(
                true, false, false, false
            )),
            ResolvedWallTopology {
                family: WallMeshFamily::End,
                quarter_turns_y: QuarterTurns::ZERO,
            }
        );
    }

    #[test]
    fn moving_connector_updates_old_and_new_neighborhood_once() {
        let mut images = Assets::<Image>::default();
        let isolated = images.add(Image::default());
        let connected = images.add(Image::default());
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<WallConnectionDirty>()
            .init_resource::<WallTopologyIndex>()
            .insert_resource(test_handles(isolated, connected))
            .add_systems(Update, wall_connections_system);
        let target_grid = (20, 20);
        let old_grid = (20, 21);
        let new_grid = (21, 20);
        let (target, _) = spawn_completed_wall(&mut app, target_grid);
        let door = spawn_completed_door(&mut app, old_grid);
        app.update();
        assert_eq!(
            app.world().get::<WallTopologyState>(target).unwrap().mask,
            WallConnectionMask::from_neighbors(true, false, false, false)
        );

        app.world_mut()
            .get_mut::<Transform>(door)
            .unwrap()
            .translation = WorldMap::grid_to_world(new_grid.0, new_grid.1).extend(0.0);
        app.update();

        let state = *app.world().get::<WallTopologyState>(target).unwrap();
        assert_eq!(
            state.mask,
            WallConnectionMask::from_neighbors(false, false, false, true)
        );
        assert_eq!(state.resolved.family, WallMeshFamily::End);
        assert_eq!(state.resolved.quarter_turns_y, QuarterTurns::THREE);
        let index = app.world().resource::<WallTopologyIndex>();
        assert_eq!(
            index.last_update_targets,
            expected_dirty_around([old_grid, new_grid])
        );
        let revision = index.resolution_revision;

        app.update();
        let index = app.world().resource::<WallTopologyIndex>();
        assert!(index.last_update_targets.is_empty());
        assert_eq!(index.resolution_revision, revision);
        assert_eq!(
            app.world()
                .get::<WallTopologyState>(target)
                .unwrap()
                .revision,
            revision
        );
    }

    #[test]
    fn building_and_blueprint_on_one_grid_coalesce_to_one_connector() {
        let mut images = Assets::<Image>::default();
        let isolated = images.add(Image::default());
        let connected = images.add(Image::default());
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<WallConnectionDirty>()
            .init_resource::<WallTopologyIndex>()
            .insert_resource(test_handles(isolated, connected))
            .add_systems(Update, wall_connections_system);
        let target_grid = (30, 30);
        let connector_grid = (30, 31);
        let (target, _) = spawn_completed_wall(&mut app, target_grid);
        let door = spawn_completed_door(&mut app, connector_grid);
        let blueprint = app
            .world_mut()
            .spawn((
                Transform::default(),
                BlueprintVisualState {
                    is_wall_or_door: true,
                    is_plain_wall: false,
                    occupied_grids: vec![connector_grid],
                    ..default()
                },
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<WallTopologyState>(target).unwrap().mask,
            WallConnectionMask::from_neighbors(true, false, false, false)
        );
        assert_eq!(
            app.world()
                .resource::<WallTopologyIndex>()
                .by_grid
                .get(&connector_grid)
                .unwrap()
                .len(),
            2
        );

        app.world_mut().despawn(door);
        app.update();
        assert_eq!(
            app.world()
                .resource::<WallTopologyIndex>()
                .by_grid
                .get(&connector_grid)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            app.world().get::<WallTopologyState>(target).unwrap().mask,
            WallConnectionMask::from_neighbors(true, false, false, false)
        );

        app.world_mut().despawn(blueprint);
        app.update();
        assert!(
            !app.world()
                .resource::<WallTopologyIndex>()
                .by_grid
                .contains_key(&connector_grid)
        );
        assert_eq!(
            app.world().get::<WallTopologyState>(target).unwrap().mask,
            WallConnectionMask::from_neighbors(false, false, false, false)
        );
    }

    #[test]
    fn construction_tile_and_spawned_wall_coalesce_to_one_connector() {
        let mut images = Assets::<Image>::default();
        let isolated = images.add(Image::default());
        let connected = images.add(Image::default());
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<WallConnectionDirty>()
            .init_resource::<WallTopologyIndex>()
            .insert_resource(test_handles(isolated, connected))
            .add_systems(Update, wall_connections_system);
        let target_grid = (35, 35);
        let connector_grid = (35, 36);
        let (target, _) = spawn_completed_wall(&mut app, target_grid);
        let tile = spawn_wall_tile(&mut app, connector_grid);

        app.update();
        assert_eq!(
            app.world().get::<WallTopologyState>(target).unwrap().mask,
            WallConnectionMask::from_neighbors(true, false, false, false)
        );
        assert_eq!(
            app.world()
                .resource::<WallTopologyIndex>()
                .by_grid
                .get(&connector_grid)
                .unwrap()
                .len(),
            1
        );

        let (spawned_wall, _) = spawn_completed_wall(&mut app, connector_grid);
        app.update();
        assert_eq!(
            app.world()
                .resource::<WallTopologyIndex>()
                .by_grid
                .get(&connector_grid)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            app.world().get::<WallTopologyState>(target).unwrap().mask,
            WallConnectionMask::from_neighbors(true, false, false, false)
        );

        app.world_mut().despawn(tile);
        app.update();
        assert_eq!(
            app.world()
                .resource::<WallTopologyIndex>()
                .by_grid
                .get(&connector_grid)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            app.world().get::<WallTopologyState>(target).unwrap().mask,
            WallConnectionMask::from_neighbors(true, false, false, false)
        );

        assert!(app.world_mut().despawn(spawned_wall));
        assert!(app.world().get_entity(spawned_wall).is_err());
        app.update();
        let remaining = app
            .world()
            .resource::<WallTopologyIndex>()
            .by_grid
            .get(&connector_grid)
            .cloned()
            .unwrap_or_default();
        assert!(
            remaining.is_empty(),
            "connector contributors remain after tile {tile:?} and wall {spawned_wall:?} removal: {remaining:?}"
        );
        assert!(
            app.world()
                .resource::<WallTopologyIndex>()
                .last_update_targets
                .contains(&target_grid)
        );
        assert_eq!(
            app.world().get::<WallTopologyState>(target).unwrap().mask,
            WallConnectionMask::from_neighbors(false, false, false, false)
        );
    }

    #[test]
    fn consecutive_world_resets_request_one_full_topology_rebuild() {
        let mut images = Assets::<Image>::default();
        let isolated = images.add(Image::default());
        let connected = images.add(Image::default());
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<WallConnectionDirty>()
            .init_resource::<WallTopologyIndex>()
            .insert_resource(test_handles(isolated, connected))
            .add_systems(Update, wall_connections_system);
        let target_grid = (40, 40);
        let (target, _) = spawn_completed_wall(&mut app, target_grid);
        spawn_completed_door(&mut app, (40, 41));
        app.update();
        assert_eq!(
            app.world().get::<WallTopologyState>(target).unwrap().mask,
            WallConnectionMask::from_neighbors(true, false, false, false)
        );

        crate::reset_for_world_replace(app.world_mut());
        crate::reset_for_world_replace(app.world_mut());
        assert_eq!(
            app.world()
                .resource::<WallTopologyIndex>()
                .resolution_revision,
            0
        );
        app.update();
        let rebuilt = app.world().resource::<WallTopologyIndex>();
        assert_eq!(rebuilt.resolution_revision, 1);
        assert!(!rebuilt.full_rebuild_requested);
        assert_eq!(
            app.world().get::<WallTopologyState>(target).unwrap().mask,
            WallConnectionMask::from_neighbors(true, false, false, false)
        );

        app.update();
        assert_eq!(
            app.world()
                .resource::<WallTopologyIndex>()
                .resolution_revision,
            1
        );
    }
}
