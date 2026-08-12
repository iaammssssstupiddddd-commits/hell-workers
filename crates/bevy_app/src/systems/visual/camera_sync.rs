//! Fixed TopDown Camera2d to Camera3d synchronization.

use crate::plugins::startup::Camera3dRtt;
use bevy::prelude::*;
use hw_core::constants::{VIEW_HEIGHT, Z_OFFSET};
use hw_ui::camera::MainCamera;

type MainCameraTransformQuery<'w, 's> =
    Query<'w, 's, &'static Transform, (With<MainCamera>, Without<Camera3dRtt>)>;
type SyncedCamera3dQuery<'w, 's> =
    Query<'w, 's, (&'static mut Transform, &'static mut Projection), With<Camera3dRtt>>;

pub fn topdown_camera_rotation() -> Quat {
    Transform::from_xyz(0.0, VIEW_HEIGHT, Z_OFFSET)
        .looking_at(Vec3::ZERO, Vec3::NEG_Z)
        .rotation
}

/// Maps the canonical 2D pan/zoom state into the single fixed TopDown RtT
/// camera. Production has no elevation branch.
pub fn sync_camera3d_system(q_cam2d: MainCameraTransformQuery, mut q_cam3d: SyncedCamera3dQuery) {
    let Ok(cam2d) = q_cam2d.single() else {
        return;
    };
    let desired_translation = Vec3::new(
        cam2d.translation.x,
        VIEW_HEIGHT,
        -cam2d.translation.y + Z_OFFSET,
    );
    let desired_rotation = topdown_camera_rotation();

    for (mut cam3d, mut projection) in &mut q_cam3d {
        if cam3d.translation != desired_translation {
            cam3d.translation = desired_translation;
        }
        if cam3d.rotation != desired_rotation {
            cam3d.rotation = desired_rotation;
        }
        if cam3d.scale != Vec3::ONE {
            cam3d.scale = Vec3::ONE;
        }
        if let Projection::Orthographic(ortho) = &mut *projection
            && ortho.scale != cam2d.scale.x
        {
            ortho.scale = cam2d.scale.x;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topdown_rotation_is_stable() {
        assert_eq!(topdown_camera_rotation(), topdown_camera_rotation());
    }
}
