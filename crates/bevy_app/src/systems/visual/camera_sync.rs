//! Camera2d ↔ Camera3d 同期システム
//!
//! PanCamera が毎フレーム Camera2d の `transform.translation.xy` と `transform.scale` を更新する。
//! 本システムはその結果を Camera3d（RtT）へ反映する。
//!
//! 座標マッピング:
//!   - 2D translation.x → 3D translation.x（方向によりオフセット）
//!   - 2D translation.y → 3D translation.z の符号反転（Camera3d up=NEG_Z のため画面上=World-Z）
//!   - 3D translation.y は TopDown 時 `VIEW_HEIGHT`、矢視時は elevation_view.rs が設定した値
//!   - 2D transform.scale → 3D OrthographicProjection.scale（Camera3d 自身は等倍を維持）
//!
//! 矢視（Elevation）モード時:
//!   - パン・ズームに追従するため XZ と OrthographicProjection.scale は毎フレーム同期
//!   - 回転・Y 高度は elevation_view_input_system が設定した値を維持
//!
//! ## World Foreground Camera（`WorldForeground2dCamera`）
//! RtT composite より後に `LAYER_2D` を再描画する第 2 の Camera2d がある。
//! `PanCamera` は `MainCamera` だけ更新するため、このカメラへ **毎フレーム `Transform` と
//! `Camera::is_active` をコピー**しないと、木・資源・Familiar 等がパン・ズームと連動しない。

use crate::plugins::startup::{Camera3dRtt, Camera3dSoulMaskRtt};
use crate::systems::visual::elevation_view::{
    ELEVATION_DISTANCE, ElevationDirection, ElevationViewState,
};
use bevy::prelude::*;
use hw_core::constants::{VIEW_HEIGHT, Z_OFFSET};
use hw_ui::camera::MainCamera;

type MainCameraTransformQuery<'w, 's> = Query<
    'w,
    's,
    &'static Transform,
    (
        With<MainCamera>,
        Without<Camera3dRtt>,
        Without<Camera3dSoulMaskRtt>,
    ),
>;
type SyncedCamera3dQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Projection),
    Or<(With<Camera3dRtt>, With<Camera3dSoulMaskRtt>)>,
>;

/// RtT composite より後に `LAYER_2D` を描画する Camera2d（`startup_systems::setup` で spawn）。
#[derive(Component)]
pub struct WorldForeground2dCamera;

/// `MainCamera` と同じビューで第 2 の `LAYER_2D` カメラを描画する（PanCamera の追従）。
///
/// `Transform` / `Camera` を両方の Query で触るため、`Without<>` でエンティティ集合を
/// 非交差にしなければならない（Bevy B0001）。
pub fn sync_world_foreground_2d_camera_system(
    q_main: Query<
        (&Transform, &Camera),
        (With<MainCamera>, Without<WorldForeground2dCamera>),
    >,
    mut q_foreground: Query<
        (&mut Transform, &mut Camera),
        (With<WorldForeground2dCamera>, Without<MainCamera>),
    >,
) {
    let Ok((main_tf, main_cam)) = q_main.single() else {
        return;
    };
    let Ok((mut fg_tf, mut fg_cam)) = q_foreground.single_mut() else {
        return;
    };
    *fg_tf = *main_tf;
    fg_cam.is_active = main_cam.is_active;
}

/// Camera2d（MainCamera）の Transform を Camera3d（Camera3dRtt）へ毎フレーム同期する。
pub fn sync_camera3d_system(
    q_cam2d: MainCameraTransformQuery,
    mut q_cam3d: SyncedCamera3dQuery,
    elevation: Res<ElevationViewState>,
) {
    let Ok(cam2d) = q_cam2d.single() else { return };
    if q_cam3d.is_empty() {
        return;
    }

    let scene_z = -cam2d.translation.y; // 2D y → 3D z 変換

    for (mut cam3d, mut projection) in &mut q_cam3d {
        match elevation.direction {
            ElevationDirection::TopDown => {
                cam3d.translation.x = cam2d.translation.x;
                cam3d.translation.z = scene_z + Z_OFFSET;
                cam3d.translation.y = VIEW_HEIGHT;
                cam3d.rotation = elevation.direction.camera_rotation();
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

        cam3d.scale = Vec3::ONE;
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scale = cam2d.scale.x;
        }
    }

    // 回転・Y（矢視高度）は elevation_view_input_system が V キー時に設定した値を維持
}
