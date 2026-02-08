use bevy::prelude::*;

use crate::systems::soul_ai::gathering::{
    GATHERING_GRACE_PERIOD, GATHERING_MIN_PARTICIPANTS, GatheringSpot, GatheringUpdateTimer,
};

/// 集会猶予タイマーの減算のみを行う（Update Phase）
pub fn gathering_grace_tick_system(
    mut q_spots: Query<&mut GatheringSpot>,
    update_timer: Res<GatheringUpdateTimer>,
) {
    if !update_timer.timer.just_finished() {
        return;
    }

    let dt = update_timer.timer.duration().as_secs_f32();

    for mut spot in q_spots.iter_mut() {
        if spot.participants < GATHERING_MIN_PARTICIPANTS {
            if !spot.grace_active {
                spot.grace_active = true;
                spot.grace_timer = GATHERING_GRACE_PERIOD;
            }
            spot.grace_timer -= dt;
        } else {
            spot.grace_active = false;
            spot.grace_timer = GATHERING_GRACE_PERIOD;
        }
    }
}
