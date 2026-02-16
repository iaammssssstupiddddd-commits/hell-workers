//! 使い魔の指揮範囲オーラ表示

use bevy::prelude::*;

use crate::constants::Z_AURA;

use super::components::*;

/// オーラのパルスアニメーションと位置追従システム
pub fn update_familiar_range_indicator(
    time: Res<Time>,
    q_familiars: Query<(Entity, &Transform, &Familiar, &FamiliarAnimation)>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    mut q_indicators: Query<
        (
            &FamiliarRangeIndicator,
            &mut Transform,
            &mut Sprite,
            Option<&mut FamiliarAura>,
            Option<&AuraLayer>,
        ),
        Without<Familiar>,
    >,
) {
    let selected_fam = selected.0;

    for (indicator, mut transform, mut sprite, aura_opt, layer_opt) in q_indicators.iter_mut() {
        if let Ok((_, fam_transform, familiar, fam_anim)) = q_familiars.get(indicator.0) {
            let z = match layer_opt {
                Some(AuraLayer::Border) => Z_AURA,
                Some(AuraLayer::Outline) => Z_AURA + 0.01,
                Some(AuraLayer::Pulse) => Z_AURA + 0.03,
                None => Z_AURA,
            };
            let ground_pos = fam_transform.translation - Vec3::Y * fam_anim.hover_offset;
            transform.translation = ground_pos.truncate().extend(z);

            let is_selected = selected_fam == Some(indicator.0);

            match layer_opt {
                Some(AuraLayer::Border) => {
                    sprite.custom_size = Some(Vec2::splat(familiar.command_radius * 2.0));
                    let alpha = if is_selected { 0.2 } else { 0.1 };
                    sprite.color = Color::srgba(1.0, 0.3, 0.0, alpha);
                }
                Some(AuraLayer::Outline) => {
                    sprite.custom_size = Some(Vec2::splat(familiar.command_radius * 2.0));
                    let alpha = if is_selected { 0.8 } else { 0.0 };
                    sprite.color = Color::srgba(1.0, 1.0, 0.0, alpha);
                }
                Some(AuraLayer::Pulse) => {
                    if let Some(mut aura) = aura_opt {
                        aura.pulse_timer += time.delta_secs() * 1.5;
                        let pulse = (aura.pulse_timer.sin() * 0.15 + 0.9).clamp(0.7, 1.0);
                        sprite.custom_size =
                            Some(Vec2::splat(familiar.command_radius * 2.0 * pulse));
                    }
                    let alpha = if is_selected { 0.15 } else { 0.05 };
                    sprite.color = Color::srgba(1.0, 0.6, 0.0, alpha);
                }
                None => {}
            }
        }
    }
}
