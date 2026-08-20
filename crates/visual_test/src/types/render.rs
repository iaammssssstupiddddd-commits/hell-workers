use super::*;
use bevy::camera::{ImageRenderTarget, RenderTarget};

// ─── RtT 合成マテリアル ──────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct RttCompositeParams {
    pub pixel_size: Vec2,
    pub shadow_offset_uv: Vec2,
    pub shadow_width_px: f32,
    pub shadow_strength: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct LocalRttCompositeMaterial {
    #[uniform(0)]
    pub params: RttCompositeParams,
    #[texture(1)]
    #[sampler(2)]
    pub scene_texture: Handle<Image>,
}

impl Material2d for LocalRttCompositeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/rtt_composite_material.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

// ─── マーカーコンポーネント ───────────────────────────────────────────────────

#[derive(Component)]
pub struct LocalRttComposite;
#[derive(Component)]
pub struct Camera3dRtt;
#[derive(Component)]
pub struct TestMainCamera;

#[derive(Resource)]
pub struct VisualTestRttRuntime {
    pub physical_size: UVec2,
    pub target_scale_factor: f32,
    pub scene: Handle<Image>,
}

impl VisualTestRttRuntime {
    pub fn scene_target(&self) -> RenderTarget {
        image_target(self.scene.clone(), self.target_scale_factor)
    }

    pub fn pixel_size(&self) -> Vec2 {
        Vec2::new(
            1.0 / self.physical_size.x.max(1) as f32,
            1.0 / self.physical_size.y.max(1) as f32,
        )
    }
}

fn image_target(handle: Handle<Image>, scale_factor: f32) -> RenderTarget {
    RenderTarget::Image(ImageRenderTarget {
        handle,
        scale_factor,
    })
}

// ─── クエリ型エイリアス ───────────────────────────────────────────────────────

pub type Cam3dSyncQuery<'w, 's> =
    Query<'w, 's, (&'static mut Transform, &'static mut Projection), With<Camera3dRtt>>;

pub type Cam2dQuery<'w, 's> =
    Query<'w, 's, &'static Transform, (With<TestMainCamera>, Without<Camera3dRtt>)>;
