//! RtT（Render-to-Texture）インフラ: オフスクリーンテクスチャとCamera3dマーカー

use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::PrimaryWindow;
use hw_core::quality::QualitySettings;

/// RtT パイプラインの runtime state を一元管理する Resource。
/// 初期化・リサイズ・品質切り替えの全経路が同じ struct を更新する。
#[derive(Resource)]
pub struct RttRuntime {
    pub viewport: RttViewportSize,
    /// RtT の物理 pixel を Window と同じ論理 viewport へ対応させる倍率。
    ///
    /// `viewport / target_scale_factor` が整数 pixel の丸め誤差内で
    /// Window logical size と一致する状態を維持する。
    pub target_scale_factor: f32,
    pub scene: Handle<Image>,
    pub soul_mask: Handle<Image>,
}

impl RttRuntime {
    pub fn new(
        viewport: RttViewportSize,
        target_scale_factor: f32,
        images: &mut Assets<Image>,
    ) -> Self {
        Self {
            scene: create_rtt_texture(
                viewport.width,
                viewport.height,
                "hell-workers-rtt-scene",
                "hell-workers-rtt-scene-view",
                images,
            ),
            soul_mask: create_rtt_texture(
                viewport.width,
                viewport.height,
                "hell-workers-rtt-soul-mask",
                "hell-workers-rtt-soul-mask-view",
                images,
            ),
            viewport,
            target_scale_factor,
        }
    }

    pub fn recreate(
        &mut self,
        viewport: RttViewportSize,
        target_scale_factor: f32,
        images: &mut Assets<Image>,
    ) {
        self.viewport = viewport;
        self.target_scale_factor = target_scale_factor;
        self.scene = create_rtt_texture(
            viewport.width,
            viewport.height,
            "hell-workers-rtt-scene",
            "hell-workers-rtt-scene-view",
            images,
        );
        self.soul_mask = create_rtt_texture(
            viewport.width,
            viewport.height,
            "hell-workers-rtt-soul-mask",
            "hell-workers-rtt-soul-mask-view",
            images,
        );
    }

    pub fn scene_render_target(&self) -> RenderTarget {
        image_render_target(self.scene.clone(), self.target_scale_factor)
    }

    pub fn soul_mask_render_target(&self) -> RenderTarget {
        image_render_target(self.soul_mask.clone(), self.target_scale_factor)
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
    let (viewport, target_scale_factor) = window.map_or_else(
        || {
            (
                RttViewportSize::from_physical_size(1280, 720, quality.rtt_scale()),
                quality.rtt_scale(),
            )
        },
        |window| {
            (
                RttViewportSize::from_window(window, quality),
                rtt_target_scale_factor(window, quality),
            )
        },
    );
    RttRuntime::new(viewport, target_scale_factor, images)
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

/// RtT 用 DirectionalLight のマーカー。
#[derive(Component)]
pub struct RttDirectionalLight;

/// 追加テスト用 RtT DirectionalLight のマーカー。
#[derive(Component)]
pub struct RttExtraDirectionalLight;

/// RtT テクスチャを生成して Assets に登録し、ハンドルを返す。
/// ウィンドウリサイズ時に呼び直すことで全参照箇所が追従する。
pub fn create_rtt_texture(
    width: u32,
    height: u32,
    texture_label: &'static str,
    view_label: &'static str,
    images: &mut Assets<Image>,
) -> Handle<Image> {
    let mut image = Image::new_target_texture(
        width,
        height,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    image.texture_descriptor.label = Some(texture_label);
    if let Some(view) = image.texture_view_descriptor.as_mut() {
        view.label = Some(view_label);
    }
    images.add(image)
}

fn image_render_target(handle: Handle<Image>, scale_factor: f32) -> RenderTarget {
    RenderTarget::Image(ImageRenderTarget {
        handle,
        scale_factor,
    })
}

fn rtt_target_scale_factor(window: &Window, quality: QualitySettings) -> f32 {
    window.scale_factor() * quality.rtt_scale()
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
    let next_target_scale_factor = rtt_target_scale_factor(window.as_ref(), *quality);
    if runtime.viewport == next_size && runtime.target_scale_factor == next_target_scale_factor {
        return;
    }

    // Bevy 0.19 の camera_system は Changed<RenderTarget> 自体を再計算条件にしない。
    // factor だけが変わる場合も image を再生成し、新 handle の AssetEvent で
    // target info / projection の更新を確実に発火させる。
    runtime.recreate(next_size, next_target_scale_factor, &mut images);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::window::WindowResolution;
    use hw_core::quality::RttQualityPreset;

    fn quality(rtt: RttQualityPreset) -> QualitySettings {
        QualitySettings { rtt }
    }

    fn window_with_scale_factor(scale_factor: f32) -> Window {
        Window {
            resolution: WindowResolution::new(1920, 1080).with_scale_factor_override(scale_factor),
            ..default()
        }
    }

    #[test]
    fn rtt_target_factor_preserves_window_logical_viewport_for_all_qualities() {
        for dpi_scale in [1.0, 1.5, 2.0] {
            let window = window_with_scale_factor(dpi_scale);
            for preset in [
                RttQualityPreset::High,
                RttQualityPreset::Medium,
                RttQualityPreset::Low,
            ] {
                let quality = quality(preset);
                let viewport = RttViewportSize::from_window(&window, quality);
                let target_scale_factor = rtt_target_scale_factor(&window, quality);

                let logical_width = viewport.width as f32 / target_scale_factor;
                let logical_height = viewport.height as f32 / target_scale_factor;
                assert!((logical_width - window.width()).abs() < 0.001);
                assert!((logical_height - window.height()).abs() < 0.001);
            }
        }
    }

    #[test]
    fn rtt_target_factor_bounds_odd_resolution_rounding_to_half_a_target_pixel() {
        let window = Window {
            resolution: WindowResolution::new(1919, 1079).with_scale_factor_override(1.5),
            ..default()
        };
        let quality = quality(RttQualityPreset::Medium);
        let viewport = RttViewportSize::from_window(&window, quality);
        let target_scale_factor = rtt_target_scale_factor(&window, quality);
        let max_logical_error = 0.5 / target_scale_factor + f32::EPSILON;

        assert!(
            (viewport.width as f32 / target_scale_factor - window.width()).abs()
                <= max_logical_error
        );
        assert!(
            (viewport.height as f32 / target_scale_factor - window.height()).abs()
                <= max_logical_error
        );
    }

    #[test]
    fn image_target_keeps_the_explicit_scale_factor() {
        let handle = Handle::<Image>::default();
        let RenderTarget::Image(target) = image_render_target(handle.clone(), 0.75) else {
            panic!("RtT helper must create an image target");
        };

        assert_eq!(target.handle, handle);
        assert_eq!(target.scale_factor, 0.75);
    }

    #[test]
    fn rtt_runtime_recreates_targets_when_dpi_or_quality_changes() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .insert_resource(quality(RttQualityPreset::High))
            .add_systems(Update, sync_rtt_texture_size_to_window_and_quality);

        let window = window_with_scale_factor(1.0);
        let initial_viewport =
            RttViewportSize::from_window(&window, quality(RttQualityPreset::High));
        let initial_factor = rtt_target_scale_factor(&window, quality(RttQualityPreset::High));
        let runtime = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            RttRuntime::new(initial_viewport, initial_factor, &mut images)
        };
        let initial_scene = runtime.scene.clone();
        app.insert_resource(runtime);
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        app.update();
        app.world_mut()
            .entity_mut(window_entity)
            .get_mut::<Window>()
            .unwrap()
            .resolution
            .set_scale_factor_override(Some(2.0));
        app.update();

        let runtime = app.world().resource::<RttRuntime>();
        assert_eq!(runtime.viewport, initial_viewport);
        assert_eq!(runtime.target_scale_factor, 2.0);
        assert_ne!(runtime.scene, initial_scene);
        let scene_after_dpi_change = runtime.scene.clone();

        app.world_mut().resource_mut::<QualitySettings>().rtt = RttQualityPreset::Low;
        app.update();

        let runtime = app.world().resource::<RttRuntime>();
        assert_eq!(runtime.viewport.width, 960);
        assert_eq!(runtime.viewport.height, 540);
        assert_eq!(runtime.target_scale_factor, 1.0);
        assert_ne!(runtime.scene, scene_after_dpi_change);
    }
}
