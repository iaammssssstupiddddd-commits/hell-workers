//! Camera2d ↔ Camera3d 同期システム
//!
//! PanCamera が毎フレーム Camera2d の `transform.translation.xy` と `transform.scale` を更新する。
//! 本システムはその結果を Camera3d（RtT）へ反映する。
//!
//! 座標マッピング:
//!   - 2D translation.x → 3D translation.x（方向によりオフセット）
//!   - 2D translation.y → 3D translation.z の符号反転（Camera3d up=NEG_Z のため画面上=World-Z）
//!   - 3D translation.y は TopDown 時 100.0、矢視時は elevation_view.rs が設定した値
//!   - 2D transform.scale → 3D transform.scale（PanCamera はズームに scale を使用）
//!
//! 矢視（Elevation）モード時:
//!   - パン・ズームに追従するため XZ・scale は毎フレーム同期
//!   - 回転・Y 高度は elevation_view_input_system が設定した値を維持

use crate::plugins::startup::Camera3dRtt;
use crate::systems::visual::elevation_view::{ElevationDirection, ELEVATION_DISTANCE, ElevationViewState};
use bevy::prelude::*;
use hw_ui::camera::MainCamera;

/// Camera2d（MainCamera）の Transform を Camera3d（Camera3dRtt）へ毎フレーム同期する。
pub fn sync_camera3d_system(
    q_cam2d: Query<&Transform, (With<MainCamera>, Without<Camera3dRtt>)>,
    mut q_cam3d: Query<&mut Transform, With<Camera3dRtt>>,
    elevation: Res<ElevationViewState>,
) {
    let Ok(cam2d) = q_cam2d.single() else { return };
    let Ok(mut cam3d) = q_cam3d.single_mut() else { return };

    let scene_z = -cam2d.translation.y; // 2D y → 3D z 変換

    cam3d.scale = cam2d.scale;

    match elevation.direction {
        ElevationDirection::TopDown => {
            cam3d.translation.x = cam2d.translation.x;
            cam3d.translation.z = scene_z;
            cam3d.translation.y = 100.0;
        }
        ElevationDirection::North => {
            cam3d.translation.x = cam2d.translation.x;
            cam3d.translation.z = scene_z + ELEVATION_DISTANCE;
        }
        ElevationDirection::South => {
            cam3d.translation.x = cam2d.translation.x;
            cam3d.translation.z = scene_z - ELEVATION_DISTANCE;
        }
        ElevationDirection::East => {
            cam3d.translation.x = cam2d.translation.x + ELEVATION_DISTANCE;
            cam3d.translation.z = scene_z;
        }
        ElevationDirection::West => {
            cam3d.translation.x = cam2d.translation.x - ELEVATION_DISTANCE;
            cam3d.translation.z = scene_z;
        }
    }
    // 回転・Y（矢視高度）は elevation_view_input_system が V キー時に設定した値を維持
}

