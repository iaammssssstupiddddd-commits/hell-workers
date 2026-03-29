use crate::app_contexts::TaskContext;
use crate::assets::GameAssets;
use crate::interface::ui::UiInputState;
use crate::plugins::startup::Building3dHandles;
use crate::systems::command::TaskMode;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_energy::YardPowerGrid;
use hw_ui::camera::MainCamera;
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

pub fn soul_spa_place_input_system(
    mut p: SoulSpaPlaceInput,
    q: SoulSpaPlaceQueries,
    mut world_map: WorldMapWrite,
    game_assets: Res<GameAssets>,
    handles_3d: Res<Building3dHandles>,
    mut commands: Commands,
) {
    if p.ui_state.pointer_over_ui {
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

    let (gx, gy) = WorldMap::world_to_grid(world_pos);
    // 2×2 footprint: top-left=(gx,gy), top-right=(gx+1,gy), bottom-left=(gx,gy-1), bottom-right=(gx+1,gy-1)
    let tiles: [(i32, i32); 4] = [(gx, gy), (gx + 1, gy), (gx, gy - 1), (gx + 1, gy - 1)];

    let all_valid = tiles.iter().all(|&(tx, ty)| {
        let wpos = WorldMap::grid_to_world(tx, ty);
        let in_yard = q.q_yards.iter().any(|(_, y)| y.contains(wpos));
        let walkable = world_map.is_walkable(tx, ty);
        let no_building = world_map.building_entity((tx, ty)).is_none();
        in_yard && walkable && no_building
    });

    if !all_valid {
        return;
    }

    let center_grid = (gx, gy);
    let center_pos = WorldMap::grid_to_world(center_grid.0, center_grid.1);

    let yard_entity = q
        .q_yards
        .iter()
        .find(|(_, yard)| yard.contains(center_pos))
        .map(|(e, _)| e);

    let power_grid_entity = yard_entity.and_then(|ye| {
        q.q_power_grids
            .iter()
            .find(|(_, ypg)| ypg.0 == ye)
            .map(|(g, _)| g)
    });

    super::spawn::spawn_soul_spa(
        &mut commands,
        &mut world_map,
        tiles,
        center_pos,
        power_grid_entity,
        &game_assets,
        &handles_3d,
    );

    p.task_context.0 = TaskMode::None;
}
