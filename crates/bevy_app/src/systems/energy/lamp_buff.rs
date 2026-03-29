use bevy::prelude::*;
use hw_core::soul::DamnedSoul;
use hw_energy::{
    LAMP_FATIGUE_RECOVERY_BONUS, LAMP_STRESS_REDUCTION_RATE, OUTDOOR_LAMP_EFFECT_RADIUS,
    PowerConsumer, Unpowered,
};

type PoweredLampQuery<'w, 's> =
    Query<'w, 's, &'static Transform, (With<PowerConsumer>, Without<Unpowered>)>;

/// 点灯中のランプ半径内にいる Soul の stress と fatigue を軽減する。
/// Unpowered ランプはスキップされるため、停電時はバフが自動停止する。
pub fn lamp_buff_system(
    q_lamps: PoweredLampQuery,
    mut q_souls: Query<(&Transform, &mut DamnedSoul)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let r2 = OUTDOOR_LAMP_EFFECT_RADIUS * OUTDOOR_LAMP_EFFECT_RADIUS;

    for lamp_tf in q_lamps.iter() {
        let lamp_pos = lamp_tf.translation.truncate();
        for (soul_tf, mut soul) in q_souls.iter_mut() {
            if soul_tf.translation.truncate().distance_squared(lamp_pos) <= r2 {
                soul.stress = (soul.stress - LAMP_STRESS_REDUCTION_RATE * dt).max(0.0);
                soul.fatigue = (soul.fatigue - LAMP_FATIGUE_RECOVERY_BONUS * dt).max(0.0);
            }
        }
    }
}
