//! リソース（木、岩等）のハイライト表示システム

use bevy::prelude::*;

use super::components::{ResourceHighlightState, ResourceVisual};
use crate::animations::{PulseAnimation, PulseAnimationConfig, update_pulse_animation};
use hw_core::relationships::TaskWorkers;
use hw_core::visual_mirror::gather::GatherHighlightMarker;

pub const COLOR_DESIGNATED_TINT: Color = Color::srgba(0.6, 0.8, 1.0, 1.0);
pub const COLOR_WORKING_TINT: Color = Color::srgba(0.8, 0.9, 1.0, 1.0);

type NewResourcesQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Sprite),
    (With<GatherHighlightMarker>, Without<ResourceVisual>),
>;

pub fn attach_resource_visual_system(
    mut commands: Commands,
    q_resources: NewResourcesQuery,
) {
    for (entity, sprite) in q_resources.iter() {
        commands.entity(entity).try_insert(ResourceVisual {
            state: ResourceHighlightState::Designated,
            pulse_animation: Some(PulseAnimation {
                timer: 0.0,
                config: PulseAnimationConfig {
                    period: 1.0,
                    min_value: 0.7,
                    max_value: 1.0,
                },
            }),
            original_color: Some(sprite.color),
        });
    }
}

pub fn update_resource_visual_system(
    time: Res<Time>,
    q_task_workers: Query<&TaskWorkers>,
    mut q_resources: Query<(
        Entity,
        &mut ResourceVisual,
        &mut Sprite,
        Option<&GatherHighlightMarker>,
    )>,
) {
    for (entity, mut visual, mut sprite, highlight) in q_resources.iter_mut() {
        if highlight.is_none() {
            if visual.state != ResourceHighlightState::Normal {
                visual.state = ResourceHighlightState::Normal;
                if let Some(original_color) = visual.original_color {
                    sprite.color = original_color;
                }
            }
            continue;
        }

        let is_being_worked = q_task_workers
            .get(entity)
            .map(|workers| !workers.is_empty())
            .unwrap_or(false);

        let new_state = if is_being_worked {
            ResourceHighlightState::Working
        } else {
            ResourceHighlightState::Designated
        };

        if visual.state != new_state {
            visual.state = new_state;
            if new_state == ResourceHighlightState::Designated {
                visual.pulse_animation = Some(PulseAnimation {
                    timer: 0.0,
                    config: PulseAnimationConfig {
                        period: 1.0,
                        min_value: 0.7,
                        max_value: 1.0,
                    },
                });
            }
        }

        match visual.state {
            ResourceHighlightState::Designated => {
                if let Some(ref mut pulse) = visual.pulse_animation {
                    let alpha = update_pulse_animation(&time, pulse);
                    sprite.color = COLOR_DESIGNATED_TINT.with_alpha(alpha);
                }
            }
            ResourceHighlightState::Working => {
                sprite.color = COLOR_WORKING_TINT;
            }
            ResourceHighlightState::Normal => {}
        }
    }
}

pub fn cleanup_resource_visual_system(
    mut commands: Commands,
    q_resources: Query<(Entity, &ResourceVisual, &Sprite), Without<GatherHighlightMarker>>,
) {
    for (entity, visual, sprite) in q_resources.iter() {
        if let Some(original_color) = visual.original_color {
            let mut sprite_copy = sprite.clone();
            sprite_copy.color = original_color;
            commands.entity(entity).try_insert(sprite_copy);
        }
        commands.entity(entity).remove::<ResourceVisual>();
    }
}
