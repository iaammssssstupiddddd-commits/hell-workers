//! RtT（Render-to-Texture）インフラ: オフスクリーンテクスチャとCamera3dマーカー

use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::PrimaryWindow;
use hw_core::quality::QualitySettings;

/// RtT パイプラインの runtime state を一元管理する Resource。
/// 初期化・リサイズ・品質切り替えの全経路が同じ struct を更新する。
#[derive(Resource)]
pub struct RttRuntime {
    pub viewport: RttViewportSize,
    pub scene: Handle<Image>,
    pub soul_mask: Handle<Image>,
}

impl RttRuntime {
    pub fn new(viewport: RttViewportSize, images: &mut Assets<Image>) -> Self {
        Self {
            scene: create_rtt_texture(viewport.width, viewport.height, images),
            soul_mask: create_rtt_texture(viewport.width, viewport.height, images),
            viewport,
        }
    }

    pub fn recreate(&mut self, viewport: RttViewportSize, images: &mut Assets<Image>) {
        self.viewport = viewport;
        self.scene = create_rtt_texture(viewport.width, viewport.height, images);
        self.soul_mask = create_rtt_texture(viewport.width, viewport.height, images);
    }

    pub fn pixel_size(&self) -> Vec2 {
        self.viewport.pixel_size()
    }
}

/// window 解像度と quality から RttRuntime を生成して返す。
/// window が取れない場合は fallback (1280×720) を使用する。
pub fn initialize_rtt_runtime(
    window: Option<&Window>,
    quality: QualitySettings,
    images: &mut Assets<Image>,
) -> RttRuntime {
    let viewport = window
        .map(|w| RttViewportSize::from_window(w, quality))
        .unwrap_or_else(|| RttViewportSize::from_physical_size(1280, 720, quality.rtt_scale()));
    RttRuntime::new(viewport, images)
}

/// RtT が追従している現在の物理解像度
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

pub fn sync_rtt_texture_size_to_window_and_quality(
    q_window: Query<Ref<Window>, With<PrimaryWindow>>,
    quality: Res<QualitySettings>,
    mut runtime: ResMut<RttRuntime>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    if !window.is_changed() && !quality.is_changed() {
        return;
    }

    let next_size = RttViewportSize::from_window(window.as_ref(), *quality);
    if runtime.viewport == next_size {
        return;
    }

    runtime.recreate(next_size, &mut images);
}
