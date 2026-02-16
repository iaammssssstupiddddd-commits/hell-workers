//! インターフェース関連のプラグイン

use crate::entities::damned_soul::DamnedSoulSpawnEvent;
use crate::entities::familiar::{FamiliarSpawnEvent, FamiliarType};
use crate::interface::camera::MainCamera;
use crate::interface::ui::plugins::{
    UiCorePlugin, UiEntityListPlugin, UiFoundationPlugin, UiInfoPanelPlugin, UiTooltipPlugin,
};
use crate::systems::GameSystemSet;
use bevy::prelude::*;

pub struct InterfacePlugin;

impl Plugin for InterfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            UiFoundationPlugin,
            UiCorePlugin,
            UiTooltipPlugin,
            UiInfoPanelPlugin,
            UiEntityListPlugin,
        ))
        .add_systems(Update, debug_spawn_system.in_set(GameSystemSet::Interface));
    }
}

fn debug_spawn_system(
    buttons: Res<ButtonInput<KeyCode>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut soul_spawn_events: MessageWriter<DamnedSoulSpawnEvent>,
    mut familiar_spawn_events: MessageWriter<FamiliarSpawnEvent>,
) {
    let mut spawn_pos = Vec2::ZERO;

    if let Ok(window) = q_window.single()
        && let Some(cursor_pos) = window.cursor_position()
        && let Ok((camera, camera_transform)) = q_camera.single()
        && let Ok(pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos)
    {
        spawn_pos = pos;
    }

    if buttons.just_pressed(KeyCode::KeyP) {
        soul_spawn_events.write(DamnedSoulSpawnEvent {
            position: spawn_pos,
        });
    }

    if buttons.just_pressed(KeyCode::KeyO) {
        familiar_spawn_events.write(FamiliarSpawnEvent {
            position: spawn_pos,
            familiar_type: FamiliarType::Imp,
        });
    }
}
