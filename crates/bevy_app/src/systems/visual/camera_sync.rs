//! Camera2d ↔ Camera3d 同期システム
//!
//! PanCamera が毎フレーム Camera2d の `transform.translation.xy` と `transform.scale` を更新する。
//! 本システムはその結果を Camera3d（RtT）へ反映する。
//!
//! 座標マッピング（TopDown モード）:
//!   - 2D translation.x → 3D translation.x（同一軸）
//!   - 2D translation.y → 3D translation.z の符号反転（Camera3d up=NEG_Z のため画面上=World-Z）
//!   - 3D translation.y は起動時設定値（100.0）を維持
//!   - 2D transform.scale → 3D transform.scale（PanCamera はズームに scale を使用）
//!
//! 矢視（Elevation）モード時:
//!   - XZ 平行移動とスケールのみ同期
//!   - Camera3d の回転・Y 高度は elevation_view.rs が設定した値を維持する

use crate::plugins::startup::Camera3dRtt;
use crate::systems::visual::elevation_view::ElevationViewState;
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

    // XZ 平行移動とスケールは常に同期（TopDown・矢視共通）
    cam3d.translation.x = cam2d.translation.x;
    cam3d.translation.z = -cam2d.translation.y; // up=NEG_Z: 画面上=World-Z なので符号反転
    cam3d.scale = cam2d.scale;

    // TopDown モードのみ Y（俯瞰高度）を維持値にリセット
    // 矢視モードでは elevation_view.rs が設定した Y と回転を保持する
    if elevation.direction.is_top_down() {
        cam3d.translation.y = 100.0;
    }
}

