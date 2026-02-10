//! 選択とカメラフォーカス

use bevy::prelude::*;

pub(super) fn select_entity_and_focus_camera(
    target: Entity,
    label: &str,
    selected_entity: &mut ResMut<crate::interface::selection::SelectedEntity>,
    q_camera: &mut Query<&mut Transform, With<crate::interface::camera::MainCamera>>,
    q_transforms: &Query<&GlobalTransform>,
) {
    selected_entity.0 = Some(target);
    info!("LIST: Selected {} {:?}", label, target);

    if let Ok(target_transform) = q_transforms.get(target) {
        if let Some(mut cam_transform) = q_camera.iter_mut().next() {
            let target_pos = target_transform.translation().truncate();
            cam_transform.translation.x = target_pos.x;
            cam_transform.translation.y = target_pos.y;
        }
    }
}
