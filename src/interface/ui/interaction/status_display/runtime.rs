use crate::interface::ui::components::{SpeedButtonMarker, UiNodeRegistry, UiSlot};
use crate::interface::ui::theme::UiTheme;
use crate::systems::time::TimeSpeed;
use bevy::prelude::*;
use std::time::Duration;

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

pub fn update_speed_button_highlight_system(
    time: Res<Time<Virtual>>,
    theme: Res<UiTheme>,
    mut q_buttons: Query<(&SpeedButtonMarker, &mut BackgroundColor, &mut BorderColor)>,
) {
    let current_speed = if time.is_paused() {
        TimeSpeed::Paused
    } else {
        let speed = time.relative_speed();
        if speed <= 1.0 {
            TimeSpeed::Normal
        } else if speed <= 2.0 {
            TimeSpeed::Fast
        } else {
            TimeSpeed::Super
        }
    };

    for (marker, mut bg, mut border) in q_buttons.iter_mut() {
        if marker.0 == current_speed {
            bg.0 = theme.colors.speed_button_active;
            *border = BorderColor::all(theme.colors.accent_ember);
        } else {
            bg.0 = theme.colors.button_default;
            *border = BorderColor::all(Color::NONE);
        }
    }
}
