//! 矢視モード（Elevation View）— 東西南北からの側面3D表示
//!
//! V キーでトップダウン → 北 → 東 → 南 → 西 とサイクル切替する。
//! Camera3d を各方向のプリセット Transform に切り替えることで側面視を実現する。
//! camera_sync.rs は矢視中は XZ 平行移動のみ同期し、回転・Y は保持する。

use crate::plugins::startup::Camera3dRtt;
use bevy::prelude::*;
use hw_ui::camera::MainCamera;

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

    /// この方向の Camera3d ローカル Transform を返す。
    ///
    /// Camera3d は world origin 相対の向きを設定する。
    /// 平行移動は camera_sync.rs が毎フレーム上書きするため、
    /// ここでは回転のみ確定させる。
    pub fn camera_rotation(self) -> Quat {
        match self {
            // TopDown: up=NEG_Z で XZ 平面を俯瞰
            Self::TopDown => Transform::from_translation(Vec3::new(0.0, 100.0, 0.0))
                .looking_at(Vec3::ZERO, Vec3::NEG_Z)
                .rotation,
            // North: Y=50 に浮かせ、-Z 方向（south→north）を見る
            Self::North => Transform::from_translation(Vec3::new(0.0, 50.0, 300.0))
                .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y)
                .rotation,
            // East: -X 方向（west→east）を見る
            Self::East => Transform::from_translation(Vec3::new(-300.0, 50.0, 0.0))
                .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y)
                .rotation,
            // South: +Z 方向（north→south）を見る
            Self::South => Transform::from_translation(Vec3::new(0.0, 50.0, -300.0))
                .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y)
                .rotation,
            // West: +X 方向（east→west）を見る
            Self::West => Transform::from_translation(Vec3::new(300.0, 50.0, 0.0))
                .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y)
                .rotation,
        }
    }

    /// 矢視モード時の Camera3d の Y 高度
    pub fn camera_y(self) -> f32 {
        match self {
            Self::TopDown => 100.0,
            _ => 50.0,
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
    q_cam2d: Query<&Transform, (With<MainCamera>, Without<Camera3dRtt>)>,
) {
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }

    state.direction = state.direction.next();

    let Ok(mut cam3d) = q_cam3d.single_mut() else {
        return;
    };

    // 回転プリセットを即時適用
    cam3d.rotation = state.direction.camera_rotation();
    cam3d.translation.y = state.direction.camera_y();

    // 2D カメラの現在位置に合わせて XZ を初期化
    if let Ok(cam2d) = q_cam2d.single() {
        cam3d.translation.x = cam2d.translation.x;
        cam3d.translation.z = -cam2d.translation.y;
    }

    info!(
        "ElevationView: switched to {:?}",
        state.direction
    );
}
