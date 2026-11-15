use std::collections::HashSet;

use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::Familiar;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Blueprint, Building, BuildingType, Rock, Tree};
use crate::systems::logistics::ResourceItem;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_core::relationships::{LoadedIn, StoredIn};
use hw_core::selection::{
    SelectionCandidate, SelectionHitKind, SelectionTargetClass, classify_selection_distance,
    point_to_rect_distance, sort_selection_candidates,
};
use hw_spatial::{
    FamiliarSpatialGrid, ResourceSpatialGrid, SelectableObstacleSpatialGrid, SpatialGrid,
    SpatialGridOps, StockpileSpatialGrid,
};
use hw_ui::camera::MainCamera;
use hw_world::WorldMap;

const TASK_AREA_BORDER_HIT_THICKNESS: f32 = 6.0;
const FAMILIAR_SNAP_PX: f32 = 12.0;
const SOUL_SNAP_PX: f32 = 10.0;
const OBJECT_SNAP_PX: f32 = 8.0;
const RESOURCE_SNAP_PX: f32 = 8.0;
const MAX_SNAP_PX: f32 = 12.0;

/// Passive profiling probe of the ordinary resolver, never a selection override.
#[cfg(feature = "profiling")]
pub(crate) fn native_direct_owner_point(
    world: &mut World,
    owner: Entity,
    points: &[Vec2],
) -> Option<(Vec2, Vec2, usize)> {
    use bevy::ecs::system::SystemState;
    if points.is_empty() || points.len() > 5 || points.iter().any(|point| !point.is_finite()) {
        return None;
    }
    let (camera, transform) = {
        let mut cameras = world.query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
        let (camera, transform) = cameras.single(world).ok()?;
        (camera.clone(), *transform)
    };
    if !camera.is_active {
        return None;
    }
    let selected = world
        .get_resource::<hw_core::selection::SelectedEntity>()
        .and_then(|selected| selected.0);
    let mut state = SystemState::<SelectionResolver>::new(world);
    let resolver = state.get(world).ok()?;
    let mut overlapping = None;
    for &point in points {
        let screen = camera
            .world_to_viewport(&transform, point.extend(0.0))
            .ok()?;
        let candidates = resolver.resolve(screen, point, &camera, &transform, selected);
        let rank = candidates.iter().position(|candidate| {
            candidate.entity == owner
                && candidate.class == SelectionTargetClass::Object
                && candidate.hit == SelectionHitKind::Direct
        });
        if let Some(rank) = rank {
            if rank == 0 {
                return Some((point, screen, rank));
            }
            overlapping.get_or_insert((point, screen, rank));
        }
    }
    overlapping
}

type ActorQuery<'w, 's, Marker> = Query<
    'w,
    's,
    (&'static Transform, Option<&'static Visibility>),
    (With<Marker>, Without<MainCamera>),
>;

type ObjectQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static Transform>,
        Option<&'static Visibility>,
        Option<&'static Building>,
        Option<&'static Blueprint>,
    ),
    (
        Or<(
            With<Building>,
            With<Blueprint>,
            With<hw_logistics::StockpilePolicy>,
        )>,
        Without<MainCamera>,
    ),
>;

type ResourceQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        Option<&'static Visibility>,
        Option<&'static StoredIn>,
        Option<&'static LoadedIn>,
    ),
    (With<ResourceItem>, Without<MainCamera>),
>;

type ObstacleQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Transform, Option<&'static Visibility>),
    (Or<(With<Tree>, With<Rock>)>, Without<MainCamera>),
>;

/// Root-side data shared by hover, primary selection and secondary actions.
#[derive(SystemParam)]
pub(crate) struct SelectionResolver<'w, 's> {
    world_map: Res<'w, WorldMap>,
    familiar_grid: Res<'w, FamiliarSpatialGrid>,
    soul_grid: Res<'w, SpatialGrid>,
    resource_grid: Res<'w, ResourceSpatialGrid>,
    stockpile_grid: Res<'w, StockpileSpatialGrid>,
    obstacle_grid: Res<'w, SelectableObstacleSpatialGrid>,
    q_familiars: ActorQuery<'w, 's, Familiar>,
    q_souls: ActorQuery<'w, 's, DamnedSoul>,
    q_resources: ResourceQuery<'w, 's>,
    q_obstacles: ObstacleQuery<'w, 's>,
    q_objects: ObjectQuery<'w, 's>,
    q_task_areas: Query<'w, 's, (Entity, &'static TaskArea), With<Familiar>>,
}

impl SelectionResolver<'_, '_> {
    pub(super) fn resolve(
        &self,
        screen_pos: Vec2,
        world_pos: Vec2,
        camera: &Camera,
        camera_transform: &GlobalTransform,
        selected_entity: Option<Entity>,
    ) -> Vec<SelectionCandidate> {
        let mut candidates = Vec::with_capacity(16);
        let broad_radius = pointer_world_radius(
            screen_pos,
            world_pos,
            camera,
            camera_transform,
            TILE_SIZE + MAX_SNAP_PX,
        );

        self.collect_actor_candidates(
            &mut candidates,
            screen_pos,
            world_pos,
            camera,
            camera_transform,
            broad_radius,
        );
        self.collect_grid_owner_candidates(
            &mut candidates,
            screen_pos,
            world_pos,
            camera,
            camera_transform,
        );

        if let Some(familiar) =
            hovered_task_area_border_entity(world_pos, selected_entity, &self.q_task_areas)
        {
            candidates.push(SelectionCandidate {
                entity: familiar,
                class: SelectionTargetClass::TaskArea,
                hit: SelectionHitKind::Direct,
                distance_px: 0.0,
                depth: 0.0,
            });
        }

        sort_selection_candidates(&mut candidates);
        candidates
    }

    fn collect_actor_candidates(
        &self,
        out: &mut Vec<SelectionCandidate>,
        screen_pos: Vec2,
        world_pos: Vec2,
        camera: &Camera,
        camera_transform: &GlobalTransform,
        broad_radius: f32,
    ) {
        let mut nearby = Vec::new();
        let projection = ScreenProjection {
            screen_pos,
            camera,
            camera_transform,
        };
        self.familiar_grid
            .get_nearby_in_radius_into(world_pos, broad_radius, &mut nearby);
        for entity in nearby.drain(..) {
            let Ok((transform, visibility)) = self.q_familiars.get(entity) else {
                continue;
            };
            if !explicitly_hidden(visibility) {
                push_projected_square(
                    out,
                    entity,
                    transform,
                    SelectionTargetClass::Familiar,
                    FAMILIAR_SNAP_PX,
                    &projection,
                );
            }
        }

        self.soul_grid
            .get_nearby_in_radius_into(world_pos, broad_radius, &mut nearby);
        for entity in nearby.drain(..) {
            let Ok((transform, visibility)) = self.q_souls.get(entity) else {
                continue;
            };
            if !explicitly_hidden(visibility) {
                push_projected_square(
                    out,
                    entity,
                    transform,
                    SelectionTargetClass::Soul,
                    SOUL_SNAP_PX,
                    &projection,
                );
            }
        }

        self.resource_grid
            .get_nearby_in_radius_into(world_pos, broad_radius, &mut nearby);
        for entity in nearby.drain(..) {
            let Ok((transform, visibility, stored, loaded)) = self.q_resources.get(entity) else {
                continue;
            };
            if !explicitly_hidden(visibility) && stored.is_none() && loaded.is_none() {
                push_projected_square(
                    out,
                    entity,
                    transform,
                    SelectionTargetClass::Resource,
                    RESOURCE_SNAP_PX,
                    &projection,
                );
            }
        }

        self.obstacle_grid
            .get_nearby_in_radius_into(world_pos, broad_radius, &mut nearby);
        for entity in nearby {
            let Ok((transform, visibility)) = self.q_obstacles.get(entity) else {
                continue;
            };
            if !explicitly_hidden(visibility) {
                push_projected_square(
                    out,
                    entity,
                    transform,
                    SelectionTargetClass::Object,
                    OBJECT_SNAP_PX,
                    &projection,
                );
            }
        }
    }

    fn collect_grid_owner_candidates(
        &self,
        out: &mut Vec<SelectionCandidate>,
        screen_pos: Vec2,
        world_pos: Vec2,
        camera: &Camera,
        camera_transform: &GlobalTransform,
    ) {
        let projection = ScreenProjection {
            screen_pos,
            camera,
            camera_transform,
        };
        let (min_grid, max_grid) =
            pointer_grid_bounds(screen_pos, world_pos, camera, camera_transform, MAX_SNAP_PX);
        let mut owners = HashSet::new();
        let mut floor_owners = HashSet::new();
        for y in min_grid.1..=max_grid.1 {
            for x in min_grid.0..=max_grid.0 {
                let grid = (x, y);
                owners.extend(self.world_map.building_entity(grid));
                owners.extend(self.world_map.stockpile_entity(grid));
                floor_owners.extend(self.world_map.floor_entity(grid));
            }
        }

        // The spatial index narrows managed stockpiles, while WorldMap remains the exact owner.
        let mut indexed_stockpiles = Vec::new();
        self.stockpile_grid.get_nearby_in_radius_into(
            world_pos,
            pointer_world_radius(
                screen_pos,
                world_pos,
                camera,
                camera_transform,
                TILE_SIZE + OBJECT_SNAP_PX,
            ),
            &mut indexed_stockpiles,
        );
        owners.extend(indexed_stockpiles.into_iter().filter(|entity| {
            !self
                .world_map
                .snapshot_owner(*entity)
                .stockpile_grids
                .is_empty()
        }));

        for entity in owners {
            let Ok((transform, visibility, building, blueprint)) = self.q_objects.get(entity)
            else {
                continue;
            };
            if blueprint.is_some() && explicitly_hidden(visibility) {
                continue;
            }
            let snapshot = self.world_map.snapshot_owner(entity);
            let grids = if !snapshot.building_grids.is_empty() {
                &snapshot.building_grids
            } else {
                &snapshot.stockpile_grids
            };
            let snap_margin = if building.is_some_and(|value| value.kind == BuildingType::Door) {
                12.0
            } else {
                OBJECT_SNAP_PX
            };
            push_grid_footprint(
                out,
                entity,
                grids,
                SelectionTargetClass::Object,
                snap_margin,
                transform.map_or(0.0, |value| value.translation.z),
                &projection,
            );
        }

        for entity in floor_owners {
            let Ok((transform, _, building, _)) = self.q_objects.get(entity) else {
                continue;
            };
            if !building.is_some_and(|value| value.kind == BuildingType::Floor) {
                continue;
            }
            let snapshot = self.world_map.snapshot_owner(entity);
            push_grid_footprint(
                out,
                entity,
                &snapshot.floor_grids,
                SelectionTargetClass::Floor,
                OBJECT_SNAP_PX,
                transform.map_or(0.0, |value| value.translation.z),
                &projection,
            );
        }
    }
}

pub(super) fn hovered_task_area_border_entity(
    world_pos: Vec2,
    selected_entity: Option<Entity>,
    q_task_areas: &Query<(Entity, &TaskArea), With<Familiar>>,
) -> Option<Entity> {
    if let Some(selected) = selected_entity
        && let Ok((_, area)) = q_task_areas.get(selected)
        && area.contains_border(world_pos, TASK_AREA_BORDER_HIT_THICKNESS)
    {
        return Some(selected);
    }

    q_task_areas
        .iter()
        .filter(|(_, area)| area.contains_border(world_pos, TASK_AREA_BORDER_HIT_THICKNESS))
        .min_by(|(left_entity, left), (right_entity, right)| {
            left.center()
                .distance_squared(world_pos)
                .total_cmp(&right.center().distance_squared(world_pos))
                .then_with(|| {
                    stable_entity_key(*left_entity).cmp(&stable_entity_key(*right_entity))
                })
        })
        .map(|(entity, _)| entity)
}

struct ScreenProjection<'a> {
    screen_pos: Vec2,
    camera: &'a Camera,
    camera_transform: &'a GlobalTransform,
}

fn push_projected_square(
    out: &mut Vec<SelectionCandidate>,
    entity: Entity,
    transform: &Transform,
    class: SelectionTargetClass,
    snap_margin_px: f32,
    projection: &ScreenProjection,
) {
    let center = transform.translation;
    let half = TILE_SIZE / 2.0;
    let Some((min, max)) = projected_world_rect(
        center.truncate() - Vec2::splat(half),
        center.truncate() + Vec2::splat(half),
        center.z,
        projection.camera,
        projection.camera_transform,
    ) else {
        return;
    };
    let distance_px = point_to_rect_distance(projection.screen_pos, min, max);
    let Some(hit) = classify_selection_distance(distance_px, snap_margin_px) else {
        return;
    };
    out.push(SelectionCandidate {
        entity,
        class,
        hit,
        distance_px,
        depth: center.z,
    });
}

fn push_grid_footprint(
    out: &mut Vec<SelectionCandidate>,
    entity: Entity,
    grids: &[(i32, i32)],
    class: SelectionTargetClass,
    snap_margin_px: f32,
    depth: f32,
    projection: &ScreenProjection,
) {
    let Some((min_world, max_world)) = grid_footprint_world_bounds(grids) else {
        return;
    };
    let Some((min, max)) = projected_world_rect(
        min_world,
        max_world,
        depth,
        projection.camera,
        projection.camera_transform,
    ) else {
        return;
    };
    let distance_px = point_to_rect_distance(projection.screen_pos, min, max);
    let Some(hit) = classify_selection_distance(distance_px, snap_margin_px) else {
        return;
    };
    out.push(SelectionCandidate {
        entity,
        class,
        hit,
        distance_px,
        depth,
    });
}

fn grid_footprint_world_bounds(grids: &[(i32, i32)]) -> Option<(Vec2, Vec2)> {
    let (&first, rest) = grids.split_first()?;
    let (mut min_grid, mut max_grid) = (first, first);
    for &(x, y) in rest {
        min_grid.0 = min_grid.0.min(x);
        min_grid.1 = min_grid.1.min(y);
        max_grid.0 = max_grid.0.max(x);
        max_grid.1 = max_grid.1.max(y);
    }
    let min_world = WorldMap::grid_to_world(min_grid.0, min_grid.1) - Vec2::splat(TILE_SIZE / 2.0);
    let max_world = WorldMap::grid_to_world(max_grid.0, max_grid.1) + Vec2::splat(TILE_SIZE / 2.0);
    Some((min_world, max_world))
}

fn projected_world_rect(
    min_world: Vec2,
    max_world: Vec2,
    depth: f32,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<(Vec2, Vec2)> {
    let corners = [
        Vec2::new(min_world.x, min_world.y),
        Vec2::new(min_world.x, max_world.y),
        Vec2::new(max_world.x, min_world.y),
        Vec2::new(max_world.x, max_world.y),
    ];
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for corner in corners {
        let projected = camera
            .world_to_viewport(camera_transform, corner.extend(depth))
            .ok()?;
        min = min.min(projected);
        max = max.max(projected);
    }
    Some((min, max))
}

fn pointer_grid_bounds(
    screen_pos: Vec2,
    world_pos: Vec2,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    margin_px: f32,
) -> ((i32, i32), (i32, i32)) {
    let first = camera
        .viewport_to_world_2d(camera_transform, screen_pos - Vec2::splat(margin_px))
        .unwrap_or(world_pos);
    let second = camera
        .viewport_to_world_2d(camera_transform, screen_pos + Vec2::splat(margin_px))
        .unwrap_or(world_pos);
    let min = WorldMap::world_to_grid(first.min(second));
    let max = WorldMap::world_to_grid(first.max(second));
    ((min.0 - 1, min.1 - 1), (max.0 + 1, max.1 + 1))
}

fn pointer_world_radius(
    screen_pos: Vec2,
    world_pos: Vec2,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    extent_px: f32,
) -> f32 {
    camera
        .viewport_to_world_2d(camera_transform, screen_pos + Vec2::splat(extent_px))
        .map_or(TILE_SIZE * 2.0, |edge| edge.distance(world_pos) + TILE_SIZE)
}

fn explicitly_hidden(visibility: Option<&Visibility>) -> bool {
    visibility.is_some_and(|value| *value == Visibility::Hidden)
}

fn stable_entity_key(entity: Entity) -> (u32, u32) {
    (entity.index_u32(), entity.generation().to_bits())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "profiling")]
    #[test]
    fn passive_owner_point_probe_refuses_missing_camera_and_invalid_requests() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();
        let before = world.entities().len();
        for points in [
            vec![],
            vec![Vec2::NAN],
            vec![Vec2::ZERO; 6],
            vec![Vec2::ZERO],
        ] {
            assert!(native_direct_owner_point(&mut world, owner, &points).is_none());
        }
        assert_eq!(world.entities().len(), before);
        assert!(!world.contains_resource::<hw_core::selection::SelectedEntity>());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn passive_owner_point_uses_normal_ranking_without_changing_selection_or_ownership() {
        use bevy::camera::{ComputedCameraValues, RenderTargetInfo};
        let mut world = World::new();
        world.init_resource::<WorldMap>();
        world.init_resource::<FamiliarSpatialGrid>();
        world.init_resource::<SpatialGrid>();
        world.init_resource::<ResourceSpatialGrid>();
        world.init_resource::<StockpileSpatialGrid>();
        world.init_resource::<SelectableObstacleSpatialGrid>();
        let camera = world
            .spawn((
                MainCamera,
                Camera {
                    computed: ComputedCameraValues {
                        clip_from_view: Mat4::orthographic_rh(-960., 960., -540., 540., 0., 2000.),
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(1920, 1080),
                            scale_factor: 1.,
                        }),
                        ..default()
                    },
                    ..default()
                },
                GlobalTransform::from_translation(Vec3::new(672., 0., 1000.)),
            ))
            .id();
        let owner = world
            .spawn((
                Building {
                    kind: BuildingType::SoulSpa,
                    is_provisional: false,
                },
                Transform::from_xyz(672., 0., 0.12),
            ))
            .id();
        let cells = [(70, 50), (71, 50), (70, 49), (71, 49)];
        for grid in cells {
            world.resource_mut::<WorldMap>().set_building(grid, owner);
        }
        let occupied = WorldMap::grid_to_world(70, 50);
        let free = WorldMap::grid_to_world(71, 50);
        let soul = world
            .spawn((
                DamnedSoul::default(),
                Transform::from_translation(occupied.extend(1.)),
                Visibility::Visible,
            ))
            .id();
        world.resource_mut::<SpatialGrid>().insert(soul, occupied);
        world.insert_resource(hw_core::selection::SelectedEntity(Some(soul)));
        let before = world.resource::<WorldMap>().snapshot_owner(owner);
        let count = world.entities().len();
        let (point, _, rank) = native_direct_owner_point(&mut world, owner, &[occupied]).unwrap();
        assert_eq!(point, occupied);
        assert_eq!(rank, 1);
        let (point, screen, rank) =
            native_direct_owner_point(&mut world, owner, &[occupied, free]).unwrap();
        assert_eq!(rank, 0);
        assert_eq!(point, free);
        assert!(screen.distance(Vec2::new(976., 524.)) < 0.001);
        assert_eq!(
            world.resource::<hw_core::selection::SelectedEntity>().0,
            Some(soul)
        );
        assert_eq!(
            world
                .resource::<WorldMap>()
                .snapshot_owner(owner)
                .building_grids,
            before.building_grids
        );
        assert_eq!(world.entities().len(), count);
        let foreign = world.spawn_empty().id();
        assert!(native_direct_owner_point(&mut world, foreign, &[free]).is_none());
        world.get_mut::<Camera>(camera).unwrap().is_active = false;
        assert!(native_direct_owner_point(&mut world, owner, &[free]).is_none());
    }

    #[test]
    fn canonical_owner_cells_cover_a_bridge_without_center_radius_guessing() {
        let cells: Vec<_> = (4..=8).flat_map(|y| [(10, y), (11, y)]).collect();
        let (min, max) = grid_footprint_world_bounds(&cells).unwrap();

        assert_eq!(max - min, Vec2::new(TILE_SIZE * 2.0, TILE_SIZE * 5.0));
        assert_eq!(
            min,
            WorldMap::grid_to_world(10, 4) - Vec2::splat(TILE_SIZE / 2.0)
        );
        assert_eq!(
            max,
            WorldMap::grid_to_world(11, 8) + Vec2::splat(TILE_SIZE / 2.0)
        );
    }

    #[test]
    fn empty_owner_has_no_selectable_footprint() {
        assert!(grid_footprint_world_bounds(&[]).is_none());
    }
}
