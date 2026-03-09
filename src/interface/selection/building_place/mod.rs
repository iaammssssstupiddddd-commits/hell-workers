mod companion;
mod flow;
mod placement;

use crate::app_contexts::{
    BuildContext, CompanionParentKind, CompanionPlacementKind, CompanionPlacementState,
};
use crate::assets::GameAssets;
use crate::interface::camera::MainCamera;
use crate::interface::ui::UiInputState;
use crate::systems::jobs::{Blueprint, Building, BuildingType};
use crate::systems::world::zones::{Site, Yard};
use crate::world::map::{RIVER_Y_MIN, WorldMap, WorldMapWrite};
use bevy::prelude::*;
use hw_ui::selection::building_spawn_pos;

use companion::make_companion_placement;
use flow::handle_companion_flow;
use placement::place_building_blueprint;

pub fn blueprint_placement(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut world_map: WorldMapWrite,
    build_context: Res<BuildContext>,
    mut companion_state: ResMut<CompanionPlacementState>,
    q_blueprints_by_entity: Query<&Blueprint>,
    q_sites: Query<&Site>,
    q_yards: Query<&Yard>,
    q_buildings: Query<&Building>,
    game_assets: Res<GameAssets>,
    mut commands: Commands,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(world_pos) = super::placement_common::world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let grid = WorldMap::world_to_grid(world_pos);

    // companion 配置中は通常建築を抑止
    if handle_companion_flow(
        &mut companion_state,
        &mut commands,
        &mut world_map,
        &game_assets,
        &q_buildings,
        &q_blueprints_by_entity,
        &q_sites,
        &q_yards,
        world_pos,
        grid,
    ) {
        return;
    }

    let Some(building_type) = build_context.0 else {
        return;
    };
    let spawn_pos = building_spawn_pos(building_type, grid, RIVER_Y_MIN);

    if building_type == BuildingType::Tank {
        companion_state.0 = Some(make_companion_placement(
            CompanionParentKind::Tank,
            grid,
            CompanionPlacementKind::BucketStorage,
            spawn_pos,
        ));
    } else {
        let _ = place_building_blueprint(
            &mut commands,
            &mut world_map,
            &game_assets,
            building_type,
            grid,
            &q_buildings,
            &q_blueprints_by_entity,
            &q_sites,
            &q_yards,
        );
    }
}
