use bevy::prelude::*;
use hw_core::visual_mirror::PoweredVisualState;

const COLOR_POWERED: Color = Color::WHITE;
const COLOR_UNPOWERED: Color = Color::srgba(0.4, 0.4, 0.4, 1.0);

/// PoweredVisualState が変化したとき、エンティティ自身および子 Sprite のカラーを更新する。
/// powered=true → 白（明）、powered=false → グレー（暗）。
pub fn sync_powered_visual_system(
    q: Query<(Entity, &PoweredVisualState), Changed<PoweredVisualState>>,
    q_children: Query<&Children>,
    mut q_sprites: Query<&mut Sprite>,
) {
    for (entity, vis) in q.iter() {
        let color = if vis.is_powered {
            COLOR_POWERED
        } else {
            COLOR_UNPOWERED
        };
        if let Ok(mut sprite) = q_sprites.get_mut(entity) {
            sprite.color = color;
        }
        if let Ok(children) = q_children.get(entity) {
            for child in children.iter() {
                if let Ok(mut sprite) = q_sprites.get_mut(child) {
                    sprite.color = color;
                }
            }
        }
    }
}
