//! インターフェース関連のプラグイン

// インポート整理完了
use crate::entities::damned_soul::DamnedSoulSpawnEvent;
use crate::entities::familiar::{
    FamiliarSpawnEvent, FamiliarType, update_familiar_range_indicator,
};
use crate::game_state::PlayMode;
use crate::interface::camera::MainCamera;
use crate::interface::selection::blueprint_placement;
use crate::interface::selection::{update_hover_entity, update_selection_indicator};
use crate::interface::ui::{
    familiar_context_menu_system, hover_tooltip_system, info_panel_system, menu_visibility_system,
    task_summary_ui_system, ui_interaction_system, update_fps_display_system,
    update_mode_text_system, update_operation_dialog_system,
};
use crate::systems::GameSystemSet;
use crate::systems::logistics::zone_placement;
use crate::systems::soul_ai::vitals::visual::familiar_hover_visualization_system;
use crate::systems::soul_ai::work::task_area_auto_haul_system;
use crate::systems::time::{
    game_time_system, time_control_keyboard_system, time_control_ui_system,
};
use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;
use std::time::Duration;

pub struct InterfacePlugin;

impl Plugin for InterfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                update_hover_entity,
                update_selection_indicator,
                hover_tooltip_system,
                blueprint_placement.run_if(in_state(PlayMode::BuildingPlace)),
                zone_placement.run_if(in_state(PlayMode::ZonePlace)),
                ui_interaction_system,
                menu_visibility_system,
                info_panel_system.run_if(
                    |selected: Res<crate::interface::selection::SelectedEntity>| {
                        selected.0.is_some()
                    },
                ),
                update_mode_text_system,
                familiar_hover_visualization_system,
            )
                .chain()
                .in_set(GameSystemSet::Interface),
        )
        .add_systems(
            Update,
            (
                familiar_context_menu_system,
                task_summary_ui_system,
                update_operation_dialog_system.run_if(
                    |selected: Res<crate::interface::selection::SelectedEntity>| {
                        selected.0.is_some()
                    },
                ),
                update_familiar_range_indicator,
                game_time_system,
                time_control_keyboard_system,
                time_control_ui_system,
                update_fps_display_system,
                debug_spawn_system,
                crate::interface::ui::entity_list_interaction_system,
                crate::interface::ui::update_unassigned_arrow_icon_system,
            )
                .in_set(GameSystemSet::Interface),
        )
        .add_systems(
            Update,
            (
                task_area_auto_haul_system,
                crate::interface::ui::rebuild_entity_list_system,
            )
                .run_if(on_timer(Duration::from_millis(100))),
        );
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

    if let Ok(window) = q_window.single() {
        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok((camera, camera_transform)) = q_camera.single() {
                if let Ok(pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                    spawn_pos = pos;
                }
            }
        }
    }

    if buttons.just_pressed(KeyCode::KeyP) {
        soul_spawn_events.write(DamnedSoulSpawnEvent {
            position: spawn_pos,
        });
        info!("DEBUG_SPAWN: Soul at {:?}", spawn_pos);
    }

    if buttons.just_pressed(KeyCode::KeyO) {
        familiar_spawn_events.write(FamiliarSpawnEvent {
            position: spawn_pos,
            familiar_type: FamiliarType::Imp,
        });
        info!("DEBUG_SPAWN: Familiar at {:?}", spawn_pos);
    }
}
