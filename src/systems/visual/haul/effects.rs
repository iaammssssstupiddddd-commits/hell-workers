//! 運搬完了時のエフェクト（ドロップポップアップ）

use bevy::prelude::*;

use super::DROP_POPUP_LIFETIME;
use super::components::DropPopup;

/// ドロップ時のポップアップアニメーション更新と削除
pub fn update_drop_popup_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_popups: Query<(Entity, &mut DropPopup, &mut Transform, &mut TextColor)>,
) {
    for (entity, mut popup, mut transform, mut color) in q_popups.iter_mut() {
        popup.lifetime -= time.delta_secs();

        if popup.lifetime <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }

        // 上昇アニメーション
        transform.translation.y += 20.0 * time.delta_secs();

        // フェードアウト
        let alpha = (popup.lifetime / DROP_POPUP_LIFETIME).min(1.0);
        color.0 = color.0.with_alpha(alpha);
    }
}
