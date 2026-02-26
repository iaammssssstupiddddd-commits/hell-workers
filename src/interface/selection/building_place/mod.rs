mod companion;
mod door_rules;
mod flow;
mod geometry;
mod placement;

use crate::assets::GameAssets;
use crate::game_state::{BuildContext, CompanionParentKind, CompanionPlacementKind, CompanionPlacementState};
use crate::interface::camera::MainCamera;
use crate::interface::ui::UiInputState;
use crate::systems::jobs::{Blueprint, Building, BuildingType, SandPile};
use crate::world::map::WorldMap;
use bevy::prelude::*;

use companion::{has_nearby_sandpile, make_companion_placement};
use flow::handle_companion_flow;
use geometry::{building_spawn_pos, occupied_grids_for_building};
use placement::place_building_blueprint;

pub fn blueprint_placement(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut world_map: ResMut<WorldMap>,
    build_context: Res<BuildContext>,
    mut companion_state: ResMut<CompanionPlacementState>,
    q_blueprints: Query<(Entity, &Blueprint, &Transform)>,
    q_blueprints_by_entity: Query<&Blueprint>,
    q_buildings: Query<&Building>,
    q_sand_piles: Query<&Transform, With<SandPile>>,
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
        world_pos,
        grid,
    ) {
        return;
    }

    let Some(building_type) = build_context.0 else {
        return;
    };
    let occupied_grids = occupied_grids_for_building(building_type, grid);
    let spawn_pos = building_spawn_pos(building_type, grid);

    if building_type == BuildingType::Tank {
        companion_state.0 = Some(make_companion_placement(
            CompanionParentKind::Tank,
            grid,
            CompanionPlacementKind::BucketStorage,
            spawn_pos,
        ));
    } else if building_type == BuildingType::MudMixer
        && !has_nearby_sandpile(&occupied_grids, &q_sand_piles, &q_blueprints, None)
    {
        companion_state.0 = Some(make_companion_placement(
            CompanionParentKind::MudMixer,
            grid,
            CompanionPlacementKind::SandPile,
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
        );
    }
}
