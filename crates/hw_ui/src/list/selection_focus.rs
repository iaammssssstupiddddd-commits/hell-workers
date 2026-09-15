// 選択とカメラフォーカス

use bevy::prelude::*;

/// 指定エンティティの位置にカメラを移動（リストクリック等で再利用）
pub fn focus_camera_on_entity<F>(
    target: Entity,
    q_camera: &mut Query<&mut Transform, With<crate::camera::MainCamera>>,
    q_transforms: &Query<&GlobalTransform, F>,
) where
    F: bevy::ecs::query::QueryFilter,
{
    if let Ok(target_transform) = q_transforms.get(target)
        && let Some(mut cam_transform) = q_camera.iter_mut().next()
    {
        let target_pos = target_transform.translation().truncate();
        cam_transform.translation.x = target_pos.x;
        cam_transform.translation.y = target_pos.y;
    }
}
