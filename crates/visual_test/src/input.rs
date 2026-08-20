use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::building::{
    TestBuilding, TestBuilding3dHandles, TestBuilding3dVisual, TestBuildingAssets,
    despawn_test_building_at, spawn_test_building,
};
use crate::types::*;

#[derive(SystemParam)]
pub struct BuildingInputContext<'w, 's> {
    assets: Option<Res<'w, TestBuildingAssets>>,
    handles_3d: Option<Res<'w, TestBuilding3dHandles>>,
    buildings: Query<'w, 's, (Entity, &'static TestBuilding)>,
    visuals_3d: Query<'w, 's, (Entity, &'static TestBuilding3dVisual)>,
}

pub fn keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<TestState>,
    mut commands: Commands,
    building: BuildingInputContext,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
        return;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        state.menu_visible = !state.menu_visible;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        state.building_kind = state.building_kind.prev();
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        state.building_kind = state.building_kind.next();
    }

    if keys.just_pressed(KeyCode::Enter) {
        let grid = state.building_cursor;
        let occupied = building.buildings.iter().any(|(_, item)| item.grid == grid);
        if occupied {
            despawn_test_building_at(
                &mut commands,
                grid,
                &building.buildings,
                &building.visuals_3d,
            );
        } else if let (Some(assets), Some(handles)) =
            (building.assets.as_deref(), building.handles_3d.as_deref())
        {
            spawn_test_building(&mut commands, state.building_kind, grid, assets, handles);
        }
    }

    if keys.just_pressed(KeyCode::Delete) {
        for (entity, _) in building.buildings.iter() {
            commands.entity(entity).despawn();
        }
        for (entity, _) in building.visuals_3d.iter() {
            commands.entity(entity).despawn();
        }
    }
}
