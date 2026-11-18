//! Floor and wall construction drag-drop placement system (root shell)
//!
//! Root shell: `TaskContext` / `WorldMap` 占有更新 / `FloorTileBlueprint` spawn に依存。
//! hw_ui / hw_jobs crate への移設には TaskContext / WorldMap の抽象化が必要であり、
//! 現段階では意図的に root に残す。純バリデーション API は hw_ui::selection::placement を参照。

mod floor_apply;
mod input;
mod validation;
mod wall_apply;

use crate::app_contexts::TaskContext;
use crate::interface::ui::UiInputState;
use crate::plugins::startup::Building3dHandles;
use crate::systems::command::TaskMode;
use crate::systems::jobs::Building;
use crate::systems::jobs::floor_construction::FloorTileBlueprint;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::PrimaryWindow;
use hw_core::game_state::PlayMode;
use hw_ui::camera::MainCamera;
use hw_ui::selection::PlacementFeedbackState;
use std::collections::HashSet;

use input::{
    FloorQueryGroup, FloorReleaseData, FloorReleaseState, handle_cancel, handle_drag_start,
    handle_release,
};
use validation::{
    build_floor_placement_plan, build_wall_placement_plan, existing_floor_building_grids,
};

#[derive(Resource, Default, PartialEq, Eq)]
pub struct FloorPlacementPreview(pub Option<String>);

fn floor_plan_summary(
    plan: &hw_ui::selection::AreaPlacementPlan,
    floor: bool,
    instant: bool,
) -> String {
    use hw_core::constants::{
        FLOOR_BONES_PER_TILE, FLOOR_MUD_PER_TILE, WALL_MUD_PER_TILE, WALL_WOOD_PER_TILE,
    };
    let accepted = plan.valid_tiles.len();
    let rejected = plan.total_tile_count.saturating_sub(accepted);
    let materials = if !floor && instant {
        "即時建築: 資材不要".to_owned()
    } else if floor {
        format!(
            "必要資材: Bone×{} / Mud×{}",
            accepted as u64 * u64::from(FLOOR_BONES_PER_TILE),
            accepted as u64 * u64::from(FLOOR_MUD_PER_TILE)
        )
    } else {
        format!(
            "必要資材: Wood×{} / Mud×{}",
            accepted as u64 * u64::from(WALL_WOOD_PER_TILE),
            accepted as u64 * u64::from(WALL_MUD_PER_TILE)
        )
    };
    format!("採用 {accepted} / 除外 {rejected} タイル — {materials}")
}

#[derive(SystemParam)]
pub struct FloorPlaceInput<'w, 's> {
    pub buttons: Res<'w, ButtonInput<MouseButton>>,
    pub q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    pub q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    pub ui_input_state: Res<'w, UiInputState>,
}

#[derive(SystemParam)]
pub struct FloorPlaceContext<'w, 's> {
    pub task_context: ResMut<'w, TaskContext>,
    pub next_play_mode: ResMut<'w, NextState<PlayMode>>,
    pub placement_feedback: ResMut<'w, PlacementFeedbackState>,
    pub real_time: Res<'w, Time<Real>>,
    pub q_existing_floor_tiles: Query<'w, 's, &'static FloorTileBlueprint>,
    pub q_floor_buildings: Query<'w, 's, (&'static Building, &'static Transform)>,
    pub debug_instant_build: Res<'w, crate::DebugInstantBuild>,
    pub building_3d_handles: Res<'w, Building3dHandles>,
}

pub fn floor_placement_system(
    input: FloorPlaceInput,
    mut context: FloorPlaceContext,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
) {
    if input.ui_input_state.world_input_blocked() {
        return;
    }

    let (is_floor_mode, start_pos_opt) = match context.task_context.0 {
        TaskMode::FloorPlace(start_pos_opt) => (true, start_pos_opt),
        TaskMode::WallPlace(start_pos_opt) => (false, start_pos_opt),
        _ => return,
    };

    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&input.q_window, &input.q_camera) else {
        return;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);

    if handle_drag_start(
        &input.buttons,
        &mut context.task_context.0,
        is_floor_mode,
        snapped_pos,
    ) {
        return;
    }

    let data = FloorReleaseData {
        start_pos_opt,
        is_floor_mode,
        snapped_pos,
        instant_build: context.debug_instant_build.0,
    };
    let fq = FloorQueryGroup {
        q_existing_floor_tiles: &context.q_existing_floor_tiles,
        q_floor_buildings: &context.q_floor_buildings,
    };

    let now = context.real_time.elapsed();
    if handle_release(
        &input.buttons,
        data,
        &fq,
        &mut commands,
        &mut world_map,
        FloorReleaseState {
            placement_feedback: &mut context.placement_feedback,
            task_mode: &mut context.task_context.0,
            building_3d_handles: &context.building_3d_handles,
            now,
        },
    ) {
        return;
    }

    let _ = handle_cancel(
        &input.buttons,
        &mut context.task_context.0,
        &mut context.next_play_mode,
    );
}

#[derive(SystemParam)]
pub struct FloorPlacePreviewContext<'w, 's> {
    pub summary: ResMut<'w, FloorPlacementPreview>,
    pub task_context: ResMut<'w, TaskContext>,
    pub time: Res<'w, Time<Virtual>>,
    pub placement_feedback: ResMut<'w, PlacementFeedbackState>,
    pub q_existing_floor_tiles: Query<'w, 's, &'static FloorTileBlueprint>,
    pub q_floor_buildings: Query<'w, 's, (&'static Building, &'static Transform)>,
    pub debug_instant_build: Res<'w, crate::DebugInstantBuild>,
}

pub fn floor_placement_preview_system(
    input: FloorPlaceInput,
    mut context: FloorPlacePreviewContext,
    world_map: crate::world::map::WorldMapRead,
) {
    let (is_floor_mode, start_pos) = match context.task_context.0 {
        TaskMode::FloorPlace(Some(start)) => (true, start),
        TaskMode::WallPlace(Some(start)) => (false, start),
        _ => {
            if context.summary.0.is_some() {
                context.summary.0 = None;
            }
            return;
        }
    };

    let can_use_world = !context.time.is_paused() && !input.ui_input_state.world_input_blocked();
    let cursor = can_use_world
        .then(|| hw_ui::camera::world_cursor_pos(&input.q_window, &input.q_camera))
        .flatten();
    let held = input.buttons.pressed(MouseButton::Left);
    let released = input.buttons.just_released(MouseButton::Left);
    if !held && (!released || cursor.is_none()) {
        context.task_context.0 = if is_floor_mode {
            TaskMode::FloorPlace(None)
        } else {
            TaskMode::WallPlace(None)
        };
    }
    let Some(world_pos) = cursor.filter(|_| held || released) else {
        if context.summary.0.is_some() {
            context.summary.0 = None;
        }
        context.placement_feedback.live = None;
        return;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);
    let existing_floor_building_grids = existing_floor_building_grids(&context.q_floor_buildings);
    let plan = if is_floor_mode {
        let area = crate::systems::command::TaskArea::from_points(start_pos, snapped_pos);
        let existing_floor_tile_grids: HashSet<_> = context
            .q_existing_floor_tiles
            .iter()
            .map(|tile| tile.grid_pos)
            .collect();
        build_floor_placement_plan(
            &area,
            world_map.as_ref(),
            &existing_floor_tile_grids,
            &existing_floor_building_grids,
        )
    } else {
        let area = crate::systems::command::wall_line_area(start_pos, snapped_pos);
        build_wall_placement_plan(
            &area,
            world_map.as_ref(),
            &existing_floor_building_grids,
            context.debug_instant_build.0,
        )
    };
    context.placement_feedback.set_live_area_plan(&plan);
    let summary = Some(floor_plan_summary(
        &plan,
        is_floor_mode,
        context.debug_instant_build.0,
    ));
    if context.summary.0 != summary {
        context.summary.0 = summary;
    }
}

#[cfg(test)]
mod summary_tests {
    use super::*;
    #[test]
    fn obstructed_floor_and_wall_release_consumes_drag_test() {
        use bevy::camera::{ComputedCameraValues, RenderTargetInfo};

        for mode in [
            TaskMode::FloorPlace as fn(Option<Vec2>) -> TaskMode,
            TaskMode::WallPlace,
        ] {
            let mut app = App::new();
            app.init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<Time<Virtual>>()
                .init_resource::<WorldMap>()
                .init_resource::<PlacementFeedbackState>()
                .init_resource::<crate::DebugInstantBuild>()
                .init_resource::<UiInputState>()
                .insert_resource(TaskContext(mode(Some(Vec2::ZERO))))
                .insert_resource(FloorPlacementPreview(Some("old preview".to_owned())))
                .add_systems(Update, floor_placement_preview_system);
            let mut window = Window {
                resolution: (100, 100).into(),
                ..default()
            };
            window.set_cursor_position(Some(Vec2::splat(50.0)));
            app.world_mut().spawn((window, PrimaryWindow));
            app.world_mut().spawn((
                MainCamera,
                Camera {
                    computed: ComputedCameraValues {
                        clip_from_view: Mat4::IDENTITY,
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::splat(100),
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    ..default()
                },
                GlobalTransform::IDENTITY,
            ));

            app.world_mut()
                .resource_mut::<UiInputState>()
                .pointer_over_ui = true;
            let mut buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            buttons.press(MouseButton::Left);
            buttons.release(MouseButton::Left);
            app.update();
            assert_eq!(app.world().resource::<TaskContext>().0, mode(None));
            assert!(app.world().resource::<FloorPlacementPreview>().0.is_none());
            assert!(
                app.world()
                    .resource::<PlacementFeedbackState>()
                    .live
                    .is_none()
            );
            assert_eq!(
                app.world_mut()
                    .query::<&FloorTileBlueprint>()
                    .iter(app.world())
                    .count(),
                0
            );
        }
    }

    #[test]
    fn valid_drag_commits_floor_and_wall_sites_with_matching_occupancy() {
        use bevy::camera::{ComputedCameraValues, RenderTargetInfo};
        use hw_jobs::construction::{
            FloorConstructionSite, WallConstructionSite, WallTileBlueprint,
        };

        let tile = (30, 30);
        let center = WorldMap::grid_to_world(tile.0, tile.1);
        for floor in [true, false] {
            let mode = if floor {
                TaskMode::FloorPlace as fn(Option<Vec2>) -> TaskMode
            } else {
                TaskMode::WallPlace
            };
            let mut app = App::new();
            app.init_resource::<ButtonInput<MouseButton>>()
                .init_resource::<Time<Real>>()
                .init_resource::<Time<Virtual>>()
                .init_resource::<WorldMap>()
                .init_resource::<NextState<PlayMode>>()
                .init_resource::<FloorPlacementPreview>()
                .init_resource::<PlacementFeedbackState>()
                .init_resource::<crate::DebugInstantBuild>()
                .init_resource::<UiInputState>()
                .insert_resource(crate::test_support::empty_building_3d_handles())
                .insert_resource(TaskContext(mode(None)))
                .add_systems(
                    Update,
                    (floor_placement_preview_system, floor_placement_system).chain(),
                );

            let mut window = Window {
                resolution: (100, 100).into(),
                ..default()
            };
            window.set_cursor_position(Some(Vec2::new(25.0, 75.0)));
            let window = app.world_mut().spawn((window, PrimaryWindow)).id();
            app.world_mut().spawn((
                MainCamera,
                Camera {
                    computed: ComputedCameraValues {
                        clip_from_view: Mat4::from_scale(Vec3::new(1.0 / 32.0, 1.0 / 32.0, 1.0)),
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::splat(100),
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    ..default()
                },
                GlobalTransform::from_translation(center.extend(0.0)),
            ));
            if !floor {
                let floor = app
                    .world_mut()
                    .spawn((
                        Building {
                            kind: hw_jobs::BuildingType::Floor,
                            is_provisional: false,
                        },
                        Transform::from_translation(center.extend(0.0)),
                    ))
                    .id();
                app.world_mut()
                    .resource_mut::<WorldMap>()
                    .set_floor(tile, floor);
            }

            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            app.update();
            assert!(matches!(
                app.world().resource::<TaskContext>().0,
                TaskMode::FloorPlace(Some(_)) | TaskMode::WallPlace(Some(_))
            ));
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(if floor {
                    Vec2::new(75.0, 25.0)
                } else {
                    Vec2::new(75.0, 75.0)
                }));
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            app.update();
            assert!(app.world().resource::<FloorPlacementPreview>().0.is_some());
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
            app.update();
            assert_eq!(app.world().resource::<TaskContext>().0, mode(None));

            let world = app.world_mut();
            if floor {
                let site = world
                    .query_filtered::<Entity, With<FloorConstructionSite>>()
                    .single(world)
                    .unwrap();
                let blueprint = world.query::<&FloorTileBlueprint>().single(world).unwrap();
                assert_eq!(blueprint.grid_pos, tile);
                assert_eq!(blueprint.parent_site, site);
                assert!(world.resource::<WorldMap>().floor_entity(tile).is_none());
                assert!(!world.resource::<WorldMap>().has_building(tile));
            } else {
                let site = world
                    .query_filtered::<Entity, With<WallConstructionSite>>()
                    .single(world)
                    .unwrap();
                let blueprint = world.query::<&WallTileBlueprint>().single(world).unwrap();
                assert_eq!(blueprint.grid_pos, tile);
                assert_eq!(blueprint.parent_site, site);
                assert_eq!(
                    world.resource::<WorldMap>().building_entity(tile),
                    Some(site)
                );
                assert!(world.resource::<WorldMap>().floor_entity(tile).is_some());
            }
        }
    }

    #[test]
    fn floor_and_wall_costs_count_only_adopted_tiles() {
        let plan = hw_ui::selection::AreaPlacementPlan {
            valid_tiles: vec![(1, 1), (1, 2)],
            total_tile_count: 3,
            first_reject: None,
        };
        assert_eq!(
            floor_plan_summary(&plan, true, false),
            "採用 2 / 除外 1 タイル — 必要資材: Bone×4 / Mud×2"
        );
        assert_eq!(
            floor_plan_summary(&plan, false, false),
            "採用 2 / 除外 1 タイル — 必要資材: Wood×2 / Mud×2"
        );
        assert!(floor_plan_summary(&plan, false, true).ends_with("即時建築: 資材不要"));
        let rejected = hw_ui::selection::AreaPlacementPlan {
            valid_tiles: vec![],
            total_tile_count: 3,
            first_reject: None,
        };
        assert!(floor_plan_summary(&rejected, true, false).ends_with("Bone×0 / Mud×0"));
    }
}
