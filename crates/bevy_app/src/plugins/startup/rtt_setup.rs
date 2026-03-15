//! RtT（Render-to-Texture）インフラ: オフスクリーンテクスチャとCamera3dマーカー

use bevy::prelude::*;

/// Camera3d（RtT）がオフスクリーン描画するテクスチャのハンドルを保持するリソース
#[derive(Resource)]
pub struct RttTextures {
    /// 3D シーンのオフスクリーンレンダリング先テクスチャ
    pub texture_3d: Handle<Image>,
}

/// Camera3d（RtT オフスクリーン）のマーカーコンポーネント。M3 カメラ同期システムで使用。
#[derive(Component)]
pub struct Camera3dRtt;
