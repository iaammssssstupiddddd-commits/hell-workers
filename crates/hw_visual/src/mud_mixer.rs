//! MudMixer の精製中アニメーション制御

use crate::handles::BuildingAnimHandles;
use crate::layer::VisualLayerKind;
use bevy::prelude::*;
use hw_core::visual_mirror::building::MudMixerVisualState;

const MUD_MIXER_ANIMATION_FPS: f32 = 6.0;

/// 精製中の MudMixer に対してアニメーションフレームを切り替える
pub fn update_mud_mixer_visual_system(
    handles: Res<BuildingAnimHandles>,
    time: Res<Time>,
    q_mixers: Query<(Entity, &MudMixerVisualState)>,
    q_children: Query<&Children>,
    mut q_visual_layers: Query<(&VisualLayerKind, &mut Sprite)>,
) {
    let frames = [
        &handles.mud_mixer_anim_1,
        &handles.mud_mixer_anim_2,
        &handles.mud_mixer_anim_3,
        &handles.mud_mixer_anim_4,
    ];
    let frame_idx = ((time.elapsed_secs() * MUD_MIXER_ANIMATION_FPS) as usize) % frames.len();

    for (mixer_entity, visual_state) in q_mixers.iter() {
        if let Ok(children) = q_children.get(mixer_entity) {
            for child in children.iter() {
                if let Ok((kind, mut sprite)) = q_visual_layers.get_mut(child) {
                    if *kind == VisualLayerKind::Struct {
                        sprite.image = if visual_state.is_active {
                            frames[frame_idx].clone()
                        } else {
                            handles.mud_mixer_idle.clone()
                        };
                        break;
                    }
                }
            }
        }
    }
}
