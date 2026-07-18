use crate::app_contexts::TaskContext;
use crate::assets::GameAssets;
use crate::interface::ui::UiInputState;
use crate::plugins::startup::Building3dHandles;
use crate::systems::command::TaskMode;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::PrimaryWindow;
use hw_energy::YardPowerGrid;
use hw_ui::camera::MainCamera;
use hw_ui::selection::PlacementFeedbackState;
use hw_world::zones::Yard;

#[derive(SystemParam)]
pub struct SoulSpaPlaceInput<'w, 's> {
    input: Res<'w, ButtonInput<MouseButton>>,
    ui_state: Res<'w, UiInputState>,
    task_context: ResMut<'w, TaskContext>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
}

#[derive(SystemParam)]
pub struct SoulSpaPlaceQueries<'w, 's> {
    q_yards: Query<'w, 's, (Entity, &'static Yard)>,
    q_power_grids: Query<'w, 's, (Entity, &'static YardPowerGrid)>,
}

#[derive(SystemParam)]
pub struct SoulSpaPlaceRuntime<'w> {
    world_map: WorldMapWrite<'w>,
    game_assets: Res<'w, GameAssets>,
    handles_3d: Res<'w, Building3dHandles>,
    real_time: Res<'w, Time<Real>>,
    placement_feedback: ResMut<'w, PlacementFeedbackState>,
}

pub fn soul_spa_place_input_system(
    mut p: SoulSpaPlaceInput,
    q: SoulSpaPlaceQueries,
    mut runtime: SoulSpaPlaceRuntime,
    mut commands: Commands,
) {
    if p.ui_state.world_input_blocked() {
        return;
    }

    if !matches!(p.task_context.0, TaskMode::SoulSpaPlace(_)) {
        return;
    }

    if !p.input.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&p.q_window, &p.q_camera) else {
        return;
    };

    let anchor = WorldMap::world_to_grid(world_pos);
    let candidate_geometry = hw_ui::selection::building_geometry(
        crate::systems::jobs::BuildingType::SoulSpa,
        anchor,
        crate::world::map::RIVER_Y_MIN,
    );
    let yard_entity = q
        .q_yards
        .iter()
        .find(|(_, yard)| {
            candidate_geometry
                .occupied_grids
                .iter()
                .all(|&(gx, gy)| yard.contains(WorldMap::grid_to_world(gx, gy)))
        })
        .map(|(entity, _)| entity);
    let (geometry, validation) =
        super::validate_soul_spa_placement(&runtime.world_map, anchor, yard_entity.is_some());
    if !validation.can_place {
        let rejection = validation
            .rejection(anchor)
            .expect("rejected SoulSpa placement must carry a reason");
        runtime.placement_feedback.show_recent_rejection(
            rejection.reason,
            rejection.grid,
            runtime.real_time.elapsed(),
        );
        return;
    }

    let power_grid_entity = yard_entity.and_then(|ye| {
        q.q_power_grids
            .iter()
            .find(|(_, ypg)| ypg.0 == ye)
            .map(|(g, _)| g)
    });

    super::spawn::spawn_soul_spa(
        &mut commands,
        &mut runtime.world_map,
        &geometry.occupied_grids,
        geometry.draw_pos,
        power_grid_entity,
        &runtime.game_assets,
        &runtime.handles_3d,
    );
    runtime.placement_feedback.clear_recent_failure();

    p.task_context.0 = TaskMode::None;
}
