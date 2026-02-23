use crate::constants::{
    DREAM_UI_MERGE_DURATION, DREAM_UI_MERGE_MAX_COUNT, DREAM_UI_MERGE_MAX_MASS,
    DREAM_UI_MERGE_RADIUS,
};
use bevy::prelude::*;

use crate::systems::visual::dream::DreamGainUiParticle;

pub fn ui_particle_merge_system(mut q_particles: Query<(Entity, &mut DreamGainUiParticle, &Node)>) {
    let positions: Vec<(Entity, Vec2, f32, bool)> = q_particles
        .iter()
        .map(|(e, p, n)| {
            let pos = Vec2::new(
                match n.left {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
                match n.top {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
            );
            let t = (p.time_alive / 3.5).clamp(0.0, 1.0);
            let merging = p.merging_into.is_some();
            (e, pos, t, merging)
        })
        .collect();

    let mut merge_pair: Option<(Entity, Entity)> = None;
    'outer: for i in 0..positions.len() {
        if positions[i].3 {
            continue;
        }
        if positions[i].2 < 0.05 {
            continue;
        }
        for j in (i + 1)..positions.len() {
            if positions[j].3 {
                continue;
            }
            if positions[j].2 < 0.05 {
                continue;
            }
            let dist = positions[i].1.distance(positions[j].1);
            if dist < DREAM_UI_MERGE_RADIUS {
                if positions[i].2 < positions[j].2 {
                    merge_pair = Some((positions[i].0, positions[j].0));
                } else {
                    merge_pair = Some((positions[j].0, positions[i].0));
                }
                break 'outer;
            }
        }
    }

    if let Some((absorbed, absorber)) = merge_pair {
        if let Ok([(_, mut absorbed_p, _), (_, mut absorber_p, _)]) =
            q_particles.get_many_mut([absorbed, absorber])
        {
            // 合体回数だけでなく、質量そのものにも上限を設ける。巨大になりすぎて軌道が壊れるのを防ぐ
            if absorber_p.merge_count >= DREAM_UI_MERGE_MAX_COUNT
                || absorber_p.mass > DREAM_UI_MERGE_MAX_MASS
            {
                return;
            }

            absorbed_p.merging_into = Some(absorber);
            absorbed_p.merge_timer = DREAM_UI_MERGE_DURATION;

            absorber_p.merge_count += 1;
            absorber_p.mass += absorbed_p.mass;
        }
    }
}
