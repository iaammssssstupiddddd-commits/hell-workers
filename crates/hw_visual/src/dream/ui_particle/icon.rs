use bevy::prelude::*;
use hw_core::constants::{DREAM_ICON_ABSORB_DURATION, DREAM_ICON_BASE_SIZE, DREAM_ICON_PULSE_SIZE};

use crate::dream::DreamIconAbsorb;

fn dream_icon_base_color() -> Color {
    Color::srgb(0.5, 0.7, 1.0)
}

pub fn dream_icon_absorb_system(
    time: Res<Time>,
    mut q_icon: Query<(
        &mut Node,
        &mut BackgroundColor,
        &mut DreamIconAbsorb,
        &mut Transform,
    )>,
) {
    let dt = time.delta_secs();
    for (mut node, mut color, mut absorb, mut transform) in q_icon.iter_mut() {
        if absorb.pulse_count > 0 {
            absorb.timer = DREAM_ICON_ABSORB_DURATION;
            absorb.pulse_count = 0;
        }

        if absorb.timer > 0.0 {
            absorb.timer -= dt;
            let progress = 1.0 - (absorb.timer / DREAM_ICON_ABSORB_DURATION).clamp(0.0, 1.0);
            let sin_val = (progress * std::f32::consts::PI).sin();

            let size =
                DREAM_ICON_BASE_SIZE + (DREAM_ICON_PULSE_SIZE - DREAM_ICON_BASE_SIZE) * sin_val;
            node.width = Val::Px(size);
            node.height = Val::Px(size);

            let impact_offset = (1.0 - progress) * sin_val * 4.0;
            transform.translation.y = impact_offset;

            let base = dream_icon_base_color();
            let r = base.to_srgba().red + (1.0 - base.to_srgba().red) * sin_val * 0.5;
            let g = base.to_srgba().green + (1.0 - base.to_srgba().green) * sin_val * 0.5;
            let b = base.to_srgba().blue + (1.0 - base.to_srgba().blue) * sin_val * 0.5;
            color.0 = Color::srgb(r, g, b);
        } else {
            node.width = Val::Px(DREAM_ICON_BASE_SIZE);
            node.height = Val::Px(DREAM_ICON_BASE_SIZE);
            transform.translation.y = 0.0;
            color.0 = dream_icon_base_color();
        }
    }
}
