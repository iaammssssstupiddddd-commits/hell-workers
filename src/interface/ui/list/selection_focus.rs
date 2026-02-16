//! 選択とカメラフォーカス

use bevy::prelude::*;

/// 指定エンティティの位置にカメラを移動（リストクリック等で再利用）
pub fn focus_camera_on_entity<F>(
    target: Entity,
    q_camera: &mut Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: &Query<&GlobalTransform, F>,
) where
    F: bevy::ecs::query::QueryFilter,
{
    if let Ok(target_transform) = q_transforms.get(target) {
        if let Some(mut cam_transform) = q_camera.iter_mut().next() {
            let target_pos = target_transform.translation().truncate();
            cam_transform.translation.x = target_pos.x;
            cam_transform.translation.y = target_pos.y;
        }
    }
}

pub(super) fn select_entity_and_focus_camera(
    target: Entity,
    _label: &str,
    selected_entity: &mut ResMut<crate::interface::selection::SelectedEntity>,
    q_camera: &mut Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: &Query<&GlobalTransform>,
) {
    selected_entity.0 = Some(target);
    focus_camera_on_entity(target, q_camera, q_transforms);
}
