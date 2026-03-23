use crate::app_contexts::TaskContext;
use crate::interface::ui::UiInputState;
use crate::systems::command::TaskMode;
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::game_state::PlayMode;
use hw_ui::camera::MainCamera;
use hw_world::identify_removal_targets;
use hw_world::zones::AreaBounds;

use super::removal_preview::{
    ZoneRemovalPreviewState, clear_removal_preview, update_removal_preview,
};

pub fn zone_removal_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
    mut q_sprites: Query<&mut Sprite>,
    mut preview_state: ResMut<ZoneRemovalPreviewState>,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    let TaskMode::ZoneRemoval(zone_type, start_pos_opt) = task_context.0 else {
        return;
    };

    let Some(world_pos) = super::world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);

    // 開始
    if buttons.just_pressed(MouseButton::Left) {
        task_context.0 = TaskMode::ZoneRemoval(zone_type, Some(snapped_pos));
        preview_state.clear();
        return;
    }

    // プレビュー更新 (ドラッグ中のみ)
    if let Some(start_pos) = start_pos_opt {
        let area = AreaBounds::from_points(start_pos, snapped_pos);
        update_removal_preview(&world_map, &area, &mut q_sprites, &mut preview_state);
    }

    // 確定
    if buttons.just_released(MouseButton::Left) {
        if let Some(start_pos) = start_pos_opt {
            let area = AreaBounds::from_points(start_pos, snapped_pos);
            apply_zone_removal(&mut commands, &mut world_map, &area);

            // Shift押下で継続、そうでなければ解除
            task_context.0 = TaskMode::ZoneRemoval(zone_type, None);
        }
        clear_removal_preview(&world_map, &mut q_sprites, &mut preview_state);
        return;
    }

    // キャンセル (右クリック)
    if buttons.just_pressed(MouseButton::Right) {
        if start_pos_opt.is_some() {
            task_context.0 = TaskMode::ZoneRemoval(zone_type, None);
            clear_removal_preview(&world_map, &mut q_sprites, &mut preview_state);
        } else {
            task_context.0 = TaskMode::None;
            next_play_mode.set(PlayMode::Normal);
        }
    }
}

fn apply_zone_removal(commands: &mut Commands, world_map: &mut WorldMap, area: &AreaBounds) {
    let (to_remove, fragments) = identify_removal_targets(world_map, area);

    let removed =
        world_map.take_stockpile_tiles(to_remove.into_iter().chain(fragments.into_iter()));
    for entity in removed {
        commands.entity(entity).despawn();
    }
}
