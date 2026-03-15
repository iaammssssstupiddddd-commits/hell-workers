//! 矢視モード（Elevation View）— 東西南北からの側面3D表示
//!
//! V キーでトップダウン → 北 → 東 → 南 → 西 とサイクル切替する。
//! Camera3d を各方向のプリセット Transform に切り替えることで側面視を実現する。
//! camera_sync.rs は矢視中は XZ 平行移動のみ同期し、回転・Y は保持する。

use crate::plugins::startup::Camera3dRtt;
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_ui::camera::MainCamera;

/// 矢視カメラがシーンを外側から見るための距離（world units）
pub const ELEVATION_DISTANCE: f32 = 800.0;

/// 矢視方向
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ElevationDirection {
    #[default]
    TopDown,
    North, // 北側から南向き（+Z 軸方向を見る）
    East,  // 東側から西向き（-X 軸方向を見る）
    South, // 南側から北向き（-Z 軸方向を見る）
    West,  // 西側から東向き（+X 軸方向を見る）
}

impl ElevationDirection {
    pub fn next(self) -> Self {
        match self {
            Self::TopDown => Self::North,
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::TopDown,
        }
    }

    pub fn is_top_down(self) -> bool {
        self == Self::TopDown
    }

    /// この方向の Camera3d 回転クォータニオンを返す（水平視点）。
    ///
    /// looking_at の起点・終点を同じ Y 高度にすることで完全水平な側面視を実現する。
    pub fn camera_rotation(self) -> Quat {
        match self {
            // TopDown: up=NEG_Z で XZ 平面を俯瞰
            Self::TopDown => Transform::from_translation(Vec3::new(0.0, 100.0, 0.0))
                .looking_at(Vec3::ZERO, Vec3::NEG_Z)
                .rotation,
            // North: +Z 側から -Z 方向を水平に見る
            Self::North => Transform::from_xyz(0.0, 0.0, 1.0)
                .looking_at(Vec3::ZERO, Vec3::Y)
                .rotation,
            // South: -Z 側から +Z 方向を水平に見る
            Self::South => Transform::from_xyz(0.0, 0.0, -1.0)
                .looking_at(Vec3::ZERO, Vec3::Y)
                .rotation,
            // East: +X 側から -X 方向を水平に見る
            Self::East => Transform::from_xyz(1.0, 0.0, 0.0)
                .looking_at(Vec3::ZERO, Vec3::Y)
                .rotation,
            // West: -X 側から +X 方向を水平に見る
            Self::West => Transform::from_xyz(-1.0, 0.0, 0.0)
                .looking_at(Vec3::ZERO, Vec3::Y)
                .rotation,
        }
    }
}

/// 矢視状態リソース
#[derive(Resource, Default, Debug)]
pub struct ElevationViewState {
    pub direction: ElevationDirection,
}

/// V キーで矢視方向をサイクル切替する。
pub fn elevation_view_input_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ElevationViewState>,
    mut q_cam3d: Query<&mut Transform, With<Camera3dRtt>>,
    mut q_cam2d: Query<(&Transform, &mut Camera), (With<MainCamera>, Without<Camera3dRtt>)>,
) {
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }

    state.direction = state.direction.next();

    let Ok(mut cam3d) = q_cam3d.single_mut() else {
        return;
    };

    // 回転プリセットを即時適用（XZ 位置は sync_camera3d_system が毎フレーム追従）
    cam3d.rotation = state.direction.camera_rotation();
    // Y は矢視中は壁の中心高度に固定（TopDown 時は sync_camera3d_system が 100.0 に戻す）
    if !state.direction.is_top_down() {
        cam3d.translation.y = TILE_SIZE / 2.0;
    }

    if let Ok((_, mut cam2d_camera)) = q_cam2d.single_mut() {
        // 矢視モード時は MainCamera を無効化して 2D スプライトを非表示にする
        // RtT composite sprite は LAYER_OVERLAY の overlay camera が描画し続ける
        cam2d_camera.is_active = state.direction.is_top_down();
    }

    info!(
        "ElevationView: switched to {:?}",
        state.direction
    );
}
