//! 汎用プログレスバー実装
//!
//! 任意のエンティティにプログレスバーを表示するためのユーティリティ

use crate::constants::*;
use bevy::prelude::*;

/// プログレスバーの設定
#[derive(Debug, Clone)]
pub struct ProgressBarConfig {
    /// バーの幅
    pub width: f32,
    /// バーの高さ
    pub height: f32,
    /// 親エンティティからのY軸オフセット
    pub y_offset: f32,
    /// 背景色
    pub bg_color: Color,
    /// 前景色
    pub fill_color: Color,
    /// Z軸の値
    pub z_index: f32,
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        Self {
            width: 24.0,
            height: 4.0,
            y_offset: -18.0,
            bg_color: Color::srgba(0.1, 0.1, 0.1, 0.9),
            fill_color: Color::srgba(1.0, 0.7, 0.1, 1.0),
            z_index: Z_BAR_BG,
        }
    }
}

/// プログレスバーのマーカーコンポーネント
#[derive(Component)]
pub struct GenericProgressBar {
    /// 設定
    pub config: ProgressBarConfig,
}

/// プログレスバーの背景
#[derive(Component)]
pub struct ProgressBarBackground;

/// プログレスバーの前景（進捗部分）
#[derive(Component)]
pub struct ProgressBarFill;

/// プログレスバーを生成する
pub fn spawn_progress_bar(
    commands: &mut Commands,
    _parent: Entity,
    parent_transform: &Transform,
    config: ProgressBarConfig,
) -> (Entity, Entity) {
    let bar_pos = parent_transform.translation + Vec3::new(0.0, config.y_offset, config.z_index);

    // 背景バー
    let bg_entity = commands
        .spawn((
            GenericProgressBar {
                config: config.clone(),
            },
            ProgressBarBackground,
            Sprite {
                color: config.bg_color,
                custom_size: Some(Vec2::new(config.width, config.height)),
                ..default()
            },
            Transform::from_translation(bar_pos),
            Name::new("ProgressBar Background"),
        ))
        .id();

    // 前景バー（進捗部分） - 最初は幅0
    let fill_entity = commands
        .spawn((
            ProgressBarFill,
            Sprite {
                color: config.fill_color,
                custom_size: Some(Vec2::new(0.0, 0.0)),
                ..default()
            },
            GenericProgressBar { config },
            Transform::from_translation(bar_pos + Vec3::new(0.0, 0.0, 0.1)),
            Name::new("ProgressBar Fill"),
        ))
        .id();

    (bg_entity, fill_entity)
}

/// プログレスバーの進捗を更新する
pub fn update_progress_bar_fill(
    progress: f32,
    config: &ProgressBarConfig,
    sprite: &mut Sprite,
    transform: &mut Transform,
    fill_color: Option<Color>,
) {
    let fill_width = config.width * progress.clamp(0.0, 1.0);
    let fill_height = config.height - 1.0;

    sprite.custom_size = Some(Vec2::new(fill_width, fill_height));

    // バーを左寄せにするためのオフセット
    // 背景バーの左端 = -config.width/2
    // fill_widthの中心を左端に合わせる: -config.width/2 + fill_width/2
    transform.translation.x = (fill_width - config.width) / 2.0;
    transform.translation.z = Z_BAR_FILL;

    // 色を更新（指定がある場合）
    if let Some(color) = fill_color {
        sprite.color = color;
    }
}

/// プログレスバーの位置を親エンティティに追従させる
pub fn sync_progress_bar_position(
    parent_transform: &Transform,
    config: &ProgressBarConfig,
    bar_transform: &mut Transform,
) {
    bar_transform.translation.x = parent_transform.translation.x;
    bar_transform.translation.y = parent_transform.translation.y + config.y_offset;
    bar_transform.translation.z = Z_BAR_BG;
}

/// プログレスバーのFill位置を親エンティティに追従させる（左寄せオフセットを考慮）
pub fn sync_progress_bar_fill_position(
    parent_transform: &Transform,
    config: &ProgressBarConfig,
    fill_width: f32,
    fill_transform: &mut Transform,
) {
    let offset_x = (fill_width - config.width) / 2.0;
    fill_transform.translation.x = parent_transform.translation.x + offset_x;
    fill_transform.translation.y = parent_transform.translation.y + config.y_offset;
    fill_transform.translation.z = Z_BAR_FILL;
}
