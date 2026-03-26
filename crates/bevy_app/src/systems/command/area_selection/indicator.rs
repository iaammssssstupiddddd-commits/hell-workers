use crate::app_contexts::TaskContext;
use crate::assets::GameAssets;
use crate::entities::damned_soul::DreamPool;
use crate::systems::command::{
    AreaEditSession, AreaSelectionIndicator, DreamTreePreviewIndicator, TaskArea, TaskMode,
    TaskModeZoneType,
};
use crate::systems::dream_tree_planting::build_dream_tree_planting_plan;
use crate::systems::jobs::Tree;
use crate::systems::logistics::ResourceItem;
use crate::systems::visual::task_area_visual::TaskAreaMaterial;
use crate::world::map::{WorldMap, WorldMapRead};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::area::{get_drag_start, wall_line_area};
use hw_core::constants::{TILE_SIZE, Z_DREAM_TREE_PREVIEW};
use hw_ui::camera::{MainCamera, world_cursor_pos};
use hw_world::zones::{Site, Yard};

#[derive(SystemParam)]
pub struct IndicatorInputQueries<'w, 's> {
    task_context: Res<'w, TaskContext>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    q_yards: Query<'w, 's, (Entity, &'static Yard)>,
    q_sites: Query<'w, 's, &'static Site>,
}

pub fn area_selection_indicator_system(
    ctx: IndicatorInputQueries,
    mut q_indicator: Query<
        (
            &mut Transform,
            &MeshMaterial2d<TaskAreaMaterial>,
            &mut Visibility,
        ),
        With<AreaSelectionIndicator>,
    >,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TaskAreaMaterial>>,
) {
    let drag_start = get_drag_start(ctx.task_context.0);

    if let Some(start_pos) = drag_start
        && let Some(world_pos) = world_cursor_pos(&ctx.q_window, &ctx.q_camera)
    {
        let area = match ctx.task_context.0 {
            TaskMode::WallPlace(_) => {
                let end_pos = WorldMap::snap_to_grid_edge(world_pos);
                wall_line_area(start_pos, end_pos)
            }
            TaskMode::DreamPlanting(_) => {
                let start_grid = WorldMap::world_to_grid(start_pos);
                let end_grid = WorldMap::world_to_grid(WorldMap::snap_to_grid_center(world_pos));
                let gx_min = start_grid.0.min(end_grid.0);
                let gx_max = start_grid.0.max(end_grid.0);
                let gy_min = start_grid.1.min(end_grid.1);
                let gy_max = start_grid.1.max(end_grid.1);
                let min_center = WorldMap::grid_to_world(gx_min, gy_min);
                let max_center = WorldMap::grid_to_world(gx_max, gy_max);
                let half = Vec2::splat(TILE_SIZE * 0.5);
                TaskArea::from_points(min_center - half, max_center + half)
            }
            _ => {
                let end_pos = WorldMap::snap_to_grid_edge(world_pos);
                TaskArea::from_points(start_pos, end_pos)
            }
        };
        let area_bounds = area.bounds();
        let is_area_valid = match ctx.task_context.0 {
            TaskMode::ZonePlacement(TaskModeZoneType::Stockpile, _) => {
                crate::systems::command::zone_placement::is_stockpile_area_within_yards(
                    &area_bounds,
                    &ctx.q_yards,
                )
            }
            TaskMode::ZonePlacement(TaskModeZoneType::Yard, Some(start_pos)) => {
                crate::systems::command::zone_placement::is_yard_expansion_area_valid(
                    start_pos,
                    &area_bounds,
                    &ctx.q_sites,
                    &ctx.q_yards,
                )
            }
            _ => true,
        };
        let center = area.center();
        let size = area.size();
        let color = super::geometry::get_indicator_color(ctx.task_context.0, is_area_valid);

        if let Some((mut transform, material_handle, mut visibility)) =
            q_indicator.iter_mut().next()
        {
            transform.translation = center.extend(0.6);
            transform.scale = size.extend(1.0);
            if let Some(material) = materials.get_mut(&material_handle.0) {
                material.color = color;
                material.size = size;
                material.state = 3; // Editing state (dashed border)
            }
            *visibility = Visibility::Visible;
        } else {
            commands.spawn((
                AreaSelectionIndicator,
                Mesh2d(meshes.add(Rectangle::default().mesh())),
                MeshMaterial2d(materials.add(TaskAreaMaterial {
                    color,
                    size,
                    time: 0.0,
                    state: 3,
                })),
                Transform::from_translation(center.extend(0.6)).with_scale(size.extend(1.0)),
                Visibility::Visible,
            ));
        }
        return;
    }

    if let Some((_, _, mut visibility)) = q_indicator.iter_mut().next() {
        *visibility = Visibility::Hidden;
    }
}

fn clear_dream_tree_preview_markers(
    commands: &mut Commands,
    q_preview_markers: &Query<Entity, With<DreamTreePreviewIndicator>>,
) {
    for entity in q_preview_markers.iter() {
        commands.entity(entity).try_despawn();
    }
}

#[derive(SystemParam)]
pub struct DreamPreviewInput<'w, 's> {
    task_context: Res<'w, TaskContext>,
    area_edit_session: Res<'w, AreaEditSession>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    world_map: WorldMapRead<'w>,
    dream_pool: Res<'w, DreamPool>,
    game_assets: Res<'w, GameAssets>,
}

pub fn dream_tree_planting_preview_system(
    mut commands: Commands,
    preview: DreamPreviewInput,
    q_trees: Query<&Transform, With<Tree>>,
    q_items: Query<&Transform, With<ResourceItem>>,
    q_preview_markers: Query<Entity, With<DreamTreePreviewIndicator>>,
) {
    let TaskMode::DreamPlanting(Some(start_pos)) = preview.task_context.0 else {
        clear_dream_tree_preview_markers(&mut commands, &q_preview_markers);
        return;
    };

    let Some(world_pos) = world_cursor_pos(&preview.q_window, &preview.q_camera) else {
        clear_dream_tree_preview_markers(&mut commands, &q_preview_markers);
        return;
    };

    if preview.game_assets.trees.is_empty() {
        clear_dream_tree_preview_markers(&mut commands, &q_preview_markers);
        return;
    }

    let end_pos = WorldMap::snap_to_grid_center(world_pos);
    let (sx, sy) = WorldMap::world_to_grid(start_pos);
    let (ex, ey) = WorldMap::world_to_grid(end_pos);
    let seed = preview
        .area_edit_session
        .dream_planting_preview_seed
        .unwrap_or(
            (sx as i64 as u64).wrapping_mul(73_856_093)
                ^ (sy as i64 as u64).wrapping_mul(19_349_663)
                ^ (ex as i64 as u64).wrapping_mul(83_492_791)
                ^ (ey as i64 as u64).wrapping_mul(2_654_435_761),
        );

    let plan = build_dream_tree_planting_plan(
        start_pos,
        end_pos,
        seed,
        preview.world_map.as_ref(),
        preview.dream_pool.points,
        q_trees.iter().count() as u32,
        &q_items,
    );

    clear_dream_tree_preview_markers(&mut commands, &q_preview_markers);

    for (index, (gx, gy)) in plan.selected_tiles.iter().copied().enumerate() {
        let pos = WorldMap::grid_to_world(gx, gy);
        let variant_seed = seed.wrapping_add(index as u64 * 7_919);
        let variant_index = (variant_seed as usize) % preview.game_assets.trees.len();

        commands.spawn((
            DreamTreePreviewIndicator,
            Sprite {
                image: preview.game_assets.trees[variant_index].clone(),
                color: Color::srgba(0.60, 0.90, 1.0, 0.50),
                custom_size: Some(Vec2::splat(TILE_SIZE * 1.4)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, Z_DREAM_TREE_PREVIEW),
            Name::new("DreamTreePreview"),
        ));
    }
}
