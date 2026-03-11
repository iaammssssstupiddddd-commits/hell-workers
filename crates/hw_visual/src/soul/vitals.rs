use bevy::prelude::*;

use hw_core::familiar::{ActiveCommand, Familiar};
use hw_core::relationships::CommandedBy;
use hw_core::soul::DamnedSoul;
use hw_ui::selection::HoveredEntity;

/// 使い魔にホバーした際、使役中の魂との間に線を引く
pub fn familiar_hover_visualization_system(
    hovered_entity: Res<HoveredEntity>,
    q_familiars: Query<(&GlobalTransform, &ActiveCommand), With<Familiar>>,
    q_souls: Query<(&GlobalTransform, &CommandedBy), With<DamnedSoul>>,
    mut gizmos: Gizmos,
) {
    if let Some(hovered) = hovered_entity.0 {
        if let Ok((fam_transform, _)) = q_familiars.get(hovered) {
            let fam_pos = fam_transform.translation().truncate();

            for (soul_transform, commanded_by) in q_souls.iter() {
                if commanded_by.0 == hovered {
                    let soul_pos = soul_transform.translation().truncate();
                    gizmos.line_2d(fam_pos, soul_pos, Color::srgba(1.0, 1.0, 1.0, 0.7));
                }
            }
        }
    }
}
