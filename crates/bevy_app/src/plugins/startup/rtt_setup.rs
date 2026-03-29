//! RtT（Render-to-Texture）インフラ: オフスクリーンテクスチャとCamera3dマーカー

use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::PrimaryWindow;
use hw_core::quality::QualitySettings;

/// Camera3d（RtT）がオフスクリーン描画するテクスチャのハンドルを保持するリソース
#[derive(Resource)]
pub struct RttTextures {
    /// 3D シーンのオフスクリーンレンダリング先テクスチャ
    pub texture_3d: Handle<Image>,
    /// Soul シルエット mask のオフスクリーンレンダリング先テクスチャ
    pub texture_soul_mask: Handle<Image>,
}

/// RtT が追従している現在の物理解像度
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RttViewportSize {
    pub width: u32,
    pub height: u32,
}

/// Camera3d（RtT オフスクリーン）のマーカーコンポーネント。M3 カメラ同期システムで使用。
#[derive(Component)]
pub struct Camera3dRtt;

/// Soul mask RtT 用 Camera3d のマーカー。
#[derive(Component)]
pub struct Camera3dSoulMaskRtt;

/// RtT テクスチャを生成して Assets に登録し、ハンドルを返す。
/// ウィンドウリサイズ時に呼び直すことで全参照箇所が追従する。
pub fn create_rtt_texture(width: u32, height: u32, images: &mut Assets<Image>) -> Handle<Image> {
    let image = Image::new_target_texture(
        width,
        height,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    images.add(image)
}

impl RttViewportSize {
    pub fn from_window(window: &Window, quality: QualitySettings) -> Self {
        Self::from_physical_size(
            window.physical_width(),
            window.physical_height(),
            quality.rtt_scale(),
        )
    }

    pub fn from_physical_size(width: u32, height: u32, scale: f32) -> Self {
        Self {
            width: scaled_dimension(width, scale),
            height: scaled_dimension(height, scale),
        }
    }

    pub fn pixel_size(self) -> Vec2 {
        Vec2::new(
            1.0 / self.width.max(1) as f32,
            1.0 / self.height.max(1) as f32,
        )
    }
}

fn scaled_dimension(value: u32, scale: f32) -> u32 {
    ((value.max(1) as f32) * scale).round().max(1.0) as u32
}

pub fn recreate_rtt_textures(
    next_size: RttViewportSize,
    viewport_size: &mut RttViewportSize,
    rtt: &mut RttTextures,
    images: &mut Assets<Image>,
) {
    *viewport_size = next_size;
    rtt.texture_3d = create_rtt_texture(next_size.width, next_size.height, images);
    rtt.texture_soul_mask = create_rtt_texture(next_size.width, next_size.height, images);
}

/// PrimaryWindow の物理解像度と品質係数に合わせて RtT テクスチャを再生成する。
pub fn sync_rtt_texture_size_to_window_and_quality(
    q_window: Query<Ref<Window>, With<PrimaryWindow>>,
    quality: Res<QualitySettings>,
    mut viewport_size: ResMut<RttViewportSize>,
    mut rtt: ResMut<RttTextures>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    if !window.is_changed() && !quality.is_changed() {
        return;
    }

    let next_size = RttViewportSize::from_window(window.as_ref(), *quality);
    if *viewport_size == next_size {
        return;
    }

    recreate_rtt_textures(next_size, &mut viewport_size, &mut rtt, &mut images);
}
