//! RtT（Render-to-Texture）インフラ: オフスクリーンテクスチャとCamera3dマーカー

use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// Camera3d（RtT）がオフスクリーン描画するテクスチャのハンドルを保持するリソース
#[derive(Resource)]
pub struct RttTextures {
    /// 3D シーンのオフスクリーンレンダリング先テクスチャ
    pub texture_3d: Handle<Image>,
}

/// Camera3d（RtT オフスクリーン）のマーカーコンポーネント。M3 カメラ同期システムで使用。
#[derive(Component)]
pub struct Camera3dRtt;

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
