//! Building placement system (root shell)
//!
//! Root shell: entity spawn + `WorldMap` occupancy update + `GameAssets` + `BuildContext` に依存。
//! hw_ui / hw_jobs crate への移設には WorldMap / GameAssets の抽象化が必要であり、
//! 現段階では意図的に root に残す。純バリデーション API は hw_ui::selection::placement を参照。
mod companion;
mod flow;
mod placement;

use crate::app_contexts::{
    BuildContext, CompanionParentKind, CompanionPlacementKind, CompanionPlacementState,
};
use crate::assets::GameAssets;
use crate::interface::ui::UiInputState;
use crate::systems::jobs::{Blueprint, Building, BuildingType};
use crate::world::map::{RIVER_Y_MIN, WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::camera::MainCamera;
use hw_ui::selection::building_spawn_pos;
use hw_world::zones::{Site, Yard};

use companion::make_companion_placement;
use flow::handle_companion_flow;
use placement::place_building_blueprint;

#[derive(SystemParam)]
pub struct BuildPlaceInput<'w, 's> {
    pub buttons: Res<'w, ButtonInput<MouseButton>>,
    pub q_window: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    pub q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    pub ui_input_state: Res<'w, UiInputState>,
}

#[derive(SystemParam)]
pub struct BuildingStateQueries<'w, 's> {
    pub q_blueprints_by_entity: Query<'w, 's, &'static Blueprint>,
    pub q_sites: Query<'w, 's, &'static Site>,
    pub q_yards: Query<'w, 's, &'static Yard>,
    pub q_buildings: Query<'w, 's, &'static Building>,
}

pub(super) struct PlacementQueries<'a, 'w, 's> {
    pub q_buildings: &'a Query<'w, 's, &'static Building>,
    pub q_blueprints_by_entity: &'a Query<'w, 's, &'static Blueprint>,
    pub q_sites: &'a Query<'w, 's, &'static Site>,
    pub q_yards: &'a Query<'w, 's, &'static Yard>,
}

pub fn blueprint_placement(
    input: BuildPlaceInput,
    mut world_map: WorldMapWrite,
    build_context: Res<BuildContext>,
    mut companion_state: ResMut<CompanionPlacementState>,
    queries: BuildingStateQueries,
    game_assets: Res<GameAssets>,
    mut commands: Commands,
) {
    if input.ui_input_state.pointer_over_ui {
        return;
    }

    if !input.buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&input.q_window, &input.q_camera) else {
        return;
    };
    let grid = WorldMap::world_to_grid(world_pos);

    let pq = PlacementQueries {
        q_buildings: &queries.q_buildings,
        q_blueprints_by_entity: &queries.q_blueprints_by_entity,
        q_sites: &queries.q_sites,
        q_yards: &queries.q_yards,
    };

    // companion 配置中は通常建築を抑止
    if handle_companion_flow(
        &mut companion_state,
        &mut commands,
        &mut world_map,
        &game_assets,
        &pq,
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
            &pq,
        );
    }
}
