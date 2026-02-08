use super::mode;
use crate::game_state::{BuildContext, PlayMode, TaskContext, ZoneContext};
use crate::interface::ui::components::*;
use bevy::prelude::*;
use std::time::Duration;

pub fn update_mode_text_system(
    play_mode: Res<State<PlayMode>>,
    build_context: Res<BuildContext>,
    zone_context: Res<ZoneContext>,
    task_context: Res<TaskContext>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<&mut Text>,
) {
    if !play_mode.is_changed()
        && !build_context.is_changed()
        && !zone_context.is_changed()
        && !task_context.is_changed()
    {
        return;
    }
    let Some(entity) = ui_nodes.get_slot(UiSlot::ModeText) else {
        return;
    };
    if let Ok(mut text) = q_text.get_mut(entity) {
        text.0 = mode::build_mode_text(
            play_mode.get(),
            &build_context,
            &zone_context,
            &task_context,
        );
    }
}

pub fn task_summary_ui_system(
    q_designations: Query<&crate::systems::jobs::Priority, With<crate::systems::jobs::Designation>>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<&mut Text>,
) {
    let Some(entity) = ui_nodes.get_slot(UiSlot::TaskSummaryText) else {
        return;
    };
    if let Ok(mut text) = q_text.get_mut(entity) {
        let total = q_designations.iter().count();
        let high = q_designations.iter().filter(|p| p.0 > 0).count();
        text.0 = format!("Tasks: {} ({} High)", total, high);
    }
}

#[derive(Default)]
pub struct FpsCounter {
    pub frame_count: u32,
    pub elapsed_time: Duration,
}

pub fn update_fps_display_system(
    time: Res<Time>,
    mut fps_counter: Local<FpsCounter>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<&mut Text>,
) {
    fps_counter.elapsed_time += time.delta();
    fps_counter.frame_count += 1;

    if fps_counter.elapsed_time >= Duration::from_secs(1) {
        let Some(entity) = ui_nodes.get_slot(UiSlot::FpsText) else {
            return;
        };
        if let Ok(mut text) = q_text.get_mut(entity) {
            let fps = fps_counter.frame_count as f32 / fps_counter.elapsed_time.as_secs_f32();
            text.0 = format!("FPS: {:.0}", fps);
            fps_counter.frame_count = 0;
            fps_counter.elapsed_time = Duration::ZERO;
        }
    }
}
