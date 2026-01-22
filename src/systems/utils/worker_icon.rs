//! 汎用ワーカーアイコンユーティリティ
//!
//! ワーカーの頭上に表示するアイコン（ハンマー、斧、ツルハシ等）の共通実装

use bevy::prelude::*;

/// ワーカーアイコンの設定
#[derive(Debug, Clone)]
pub struct WorkerIconConfig {
    /// アイコンサイズ
    pub size: Vec2,
    /// ワーカーからのY軸オフセット
    pub y_offset: f32,
    /// アイコンの色（ティント）
    pub color: Color,
    /// bob（上下揺れ）アニメーションの速度
    pub bob_speed: f32,
    /// bob（上下揺れ）アニメーションの振幅
    pub bob_amplitude: f32,
    /// Z軸の値
    pub z_index: f32,
}

impl Default for WorkerIconConfig {
    fn default() -> Self {
        Self {
            size: Vec2::splat(16.0),
            y_offset: 32.0,
            color: Color::WHITE,
            bob_speed: 5.0,
            bob_amplitude: 2.5,
            z_index: 0.5,
        }
    }
}

/// ワーカーアイコンコンポーネント
#[derive(Component)]
pub struct WorkerIcon {
    /// アイコン設定
    pub config: WorkerIconConfig,
}

/// ワーカーアイコンを生成する
pub fn spawn_worker_icon(
    commands: &mut Commands,
    _worker_entity: Entity,
    worker_transform: &Transform,
    icon_image: Handle<Image>,
    config: WorkerIconConfig,
) -> Entity {
    let icon_pos = worker_transform.translation + Vec3::new(0.0, config.y_offset, config.z_index);

    commands
        .spawn((
            WorkerIcon {
                config: config.clone(),
            },
            Sprite {
                image: icon_image,
                custom_size: Some(config.size),
                color: config.color,
                ..default()
            },
            Transform::from_translation(icon_pos),
            Name::new("WorkerIcon"),
        ))
        .id()
}

/// ワーカーアイコンの位置を更新する（bob付き）
/// 返り値: ワーカーがまだ存在するかどうか
pub fn update_worker_icon_position(
    time: &Time,
    worker_transform: Option<&Transform>,
    icon: &WorkerIcon,
    icon_transform: &mut Transform,
) -> bool {
    match worker_transform {
        Some(transform) => {
            // bobアニメーション
            let bob =
                (time.elapsed_secs() * icon.config.bob_speed).sin() * icon.config.bob_amplitude;
            icon_transform.translation = transform.translation
                + Vec3::new(0.0, icon.config.y_offset + bob, icon.config.z_index);
            true
        }
        None => false,
    }
}
