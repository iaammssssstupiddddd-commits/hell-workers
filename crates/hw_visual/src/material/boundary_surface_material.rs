use bevy::pbr::{ExtendedMaterial, MaterialExtension, OpaqueRendererMethod};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use hw_core::constants::TILE_SIZE;

/// 境界 ribbon メッシュ用のシェーダーユニフォーム。
#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct BoundarySurfaceUniform {
    /// left terrain の粗い ID (0=Grass, 1=Dirt, 2=Sand, 3=River)。f32 として渡し shader で u32 に変換する。
    pub left_terrain_id: f32,
    /// right terrain の粗い ID。
    pub right_terrain_id: f32,
    /// world-space テクスチャ UV スケール (= 1.0 / TILE_SIZE)。
    pub uv_scale: f32,
    /// u=0.5 近傍の blend 遷移幅（smoothstep の半幅）。0.0 で鋭いエッジ、0.5 で全体グラデーション。
    pub blend_softness: f32,
}

impl Default for BoundarySurfaceUniform {
    fn default() -> Self {
        Self {
            left_terrain_id: 0.0,
            right_terrain_id: 1.0,
            uv_scale: 1.0 / TILE_SIZE,
            blend_softness: 0.15,
        }
    }
}

/// 境界 ribbon の `MaterialExtension`。
///
/// `ExtendedMaterial<StandardMaterial, BoundarySurfaceMaterialExt>` として使用する。
/// terrain アルベドテクスチャを共有 Handle で保持し、world-space UV でサンプルして
/// left/right terrain をリボン幅方向にブレンドする。
/// macro_overlay テクスチャを使って地形シェーダーと同じ輝度変調・カラーグレーディングを適用する。
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct BoundarySurfaceMaterialExt {
    #[uniform(100)]
    pub uniforms: BoundarySurfaceUniform,

    #[texture(101)]
    #[sampler(102)]
    pub grass_albedo: Option<Handle<Image>>,

    #[texture(103)]
    #[sampler(104)]
    pub dirt_albedo: Option<Handle<Image>>,

    #[texture(105)]
    #[sampler(106)]
    pub sand_albedo: Option<Handle<Image>>,

    #[texture(107)]
    #[sampler(108)]
    pub river_albedo: Option<Handle<Image>>,

    #[texture(109)]
    #[sampler(110)]
    pub terrain_macro_noise: Option<Handle<Image>>,

    #[texture(111)]
    #[sampler(112)]
    pub grass_macro_overlay: Option<Handle<Image>>,

    #[texture(113)]
    #[sampler(114)]
    pub dirt_macro_overlay: Option<Handle<Image>>,

    #[texture(115)]
    #[sampler(116)]
    pub sand_macro_overlay: Option<Handle<Image>>,

    #[texture(117)]
    #[sampler(118)]
    pub terrain_feature_map: Option<Handle<Image>>,

    #[texture(119)]
    #[sampler(120)]
    pub terrain_feature_lut: Option<Handle<Image>>,

    #[texture(121)]
    #[sampler(122)]
    pub shoreline_detail: Option<Handle<Image>>,
}

impl MaterialExtension for BoundarySurfaceMaterialExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/boundary_surface_material.wgsl".into()
    }
}

/// 境界 ribbon メッシュに使用する複合マテリアル型。
pub type BoundarySurfaceMaterial = ExtendedMaterial<StandardMaterial, BoundarySurfaceMaterialExt>;

/// `BoundarySurfaceMaterial` インスタンスを生成するヘルパー。
///
/// `alpha_mode: AlphaMode::Blend` を設定してリボン端のフェードを有効にする。
pub fn make_boundary_surface_material(extension: BoundarySurfaceMaterialExt) -> BoundarySurfaceMaterial {
    BoundarySurfaceMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            alpha_mode: AlphaMode::Blend,
            opaque_render_method: OpaqueRendererMethod::Forward,
            double_sided: false,
            cull_mode: None,
            ..default()
        },
        extension,
    }
}
