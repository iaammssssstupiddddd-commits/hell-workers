//! MudMixer の精製中アニメーション制御

use crate::handles::BuildingAnimHandles;
use bevy::prelude::*;
use hw_jobs::AssignedTask;
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::RefinePhase;
use std::collections::HashSet;

const MUD_MIXER_ANIMATION_FPS: f32 = 6.0;

/// 精製中の MudMixer に対してアニメーションフレームを切り替える
pub fn update_mud_mixer_visual_system(
    handles: Res<BuildingAnimHandles>,
    time: Res<Time>,
    q_souls: Query<&AssignedTask>,
    mut q_mixers: Query<(Entity, &mut Sprite), With<MudMixerStorage>>,
) {
    let refining_mixers: HashSet<Entity> = q_souls
        .iter()
        .filter_map(|task| match task {
            AssignedTask::Refine(data) if matches!(data.phase, RefinePhase::Refining { .. }) => {
                Some(data.mixer)
            }
            _ => None,
        })
        .collect();

    let frames = [
        &handles.mud_mixer_anim_1,
        &handles.mud_mixer_anim_2,
        &handles.mud_mixer_anim_3,
        &handles.mud_mixer_anim_4,
    ];
    let frame_idx = ((time.elapsed_secs() * MUD_MIXER_ANIMATION_FPS) as usize) % frames.len();

    for (mixer_entity, mut sprite) in q_mixers.iter_mut() {
        if refining_mixers.contains(&mixer_entity) {
            sprite.image = frames[frame_idx].clone();
        } else {
            sprite.image = handles.mud_mixer_idle.clone();
        }
    }
}
