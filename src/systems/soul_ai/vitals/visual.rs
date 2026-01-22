use bevy::prelude::*;

use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::ActiveCommand;
use crate::entities::familiar::Familiar;
use crate::entities::familiar::UnderCommand;

/// ホバー線の描画用コンポーネント
#[derive(Component)]
pub struct HoverLineIndicator;

/// 使い魔にホバーした際、使役中の魂との間に線を引く
pub fn familiar_hover_visualization_system(
    mut commands: Commands,
    hovered_entity: Res<crate::interface::selection::HoveredEntity>,
    q_familiars: Query<(&GlobalTransform, &ActiveCommand), With<Familiar>>,
    q_souls: Query<(&GlobalTransform, &UnderCommand), With<DamnedSoul>>,
    q_lines: Query<Entity, With<HoverLineIndicator>>,
    mut gizmos: Gizmos,
) {
    for entity in q_lines.iter() {
        commands.entity(entity).despawn();
    }

    if let Some(hovered) = hovered_entity.0 {
        if let Ok((fam_transform, _)) = q_familiars.get(hovered) {
            let fam_pos = fam_transform.translation().truncate();

            for (soul_transform, under_command) in q_souls.iter() {
                let soul_transform: &GlobalTransform = soul_transform;
                if under_command.0 == hovered {
                    let soul_pos = soul_transform.translation().truncate();
                    gizmos.line_2d(fam_pos, soul_pos, Color::srgba(1.0, 1.0, 1.0, 0.7));
                }
            }
        }
    }
}
