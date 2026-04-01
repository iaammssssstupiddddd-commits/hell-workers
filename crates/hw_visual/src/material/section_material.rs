use bevy::pbr::{ExtendedMaterial, MaterialExtension, OpaqueRendererMethod, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use hw_core::constants::TILE_SIZE;

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct SectionMaterialUniform {
    pub cut_position: Vec4,
    pub cut_normal: Vec4,
    pub thickness: f32,
    pub cut_active: f32,
    pub build_progress: f32,
    pub wall_height: f32,
    /// 0.0 = メッシュ UV（建物など）、1.0 = ワールド XZ UV（地形タイル）
    pub albedo_uv_mode: f32,
    /// ワールド UV スケール。地形: `1.0 / TILE_SIZE`、非地形: 0.0
    pub uv_scale: f32,
    /// UV スクロール速度（U・画面上は左→右の流れ）。river: ~0.03、grass/dirt/sand: 0.0（停止）
    pub uv_scroll_speed: f32,
    /// A3 低周波 UV 歪みの振幅（**UV 空間**、おおよそテクスチャ 1 周に対する割合）。草のみ非ゼロ推奨。0.0 で無効
    pub uv_distort_strength: f32,
    /// A3 明度変調。`base_color.rgb *= 1 + sin(wx·freq) * 本値`。草のみ非ゼロ推奨。0.0 で無効
    pub brightness_variation_strength: f32,
    /// uniform 末尾アライメント用（常に 0）。`[f32; 3]` は encase の uniform で不可
    pub _pad_section_tail_0: f32,
    pub _pad_section_tail_1: f32,
    pub _pad_section_tail_2: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SectionMaterialExt {
    #[uniform(100)]
    pub uniforms: SectionMaterialUniform,
}

impl Default for SectionMaterialExt {
    fn default() -> Self {
        Self {
            uniforms: SectionMaterialUniform {
                cut_position: Vec4::ZERO,
                cut_normal: Vec3::NEG_Z.extend(0.0),
                thickness: TILE_SIZE * 5.0,
                cut_active: 0.0,
                build_progress: 1.0,
                wall_height: 0.0,
                albedo_uv_mode: 0.0,
                uv_scale: 0.0,
                uv_scroll_speed: 0.0,
                uv_distort_strength: 0.0,
                brightness_variation_strength: 0.0,
                _pad_section_tail_0: 0.0,
                _pad_section_tail_1: 0.0,
                _pad_section_tail_2: 0.0,
            },
        }
    }
}

impl MaterialExtension for SectionMaterialExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/section_material.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/section_material_prepass.wgsl".into()
    }
}

pub type SectionMaterial = ExtendedMaterial<StandardMaterial, SectionMaterialExt>;

pub fn make_section_material(base_color: LinearRgba) -> SectionMaterial {
    SectionMaterial {
        base: StandardMaterial {
            base_color: Color::linear_rgba(
                base_color.red,
                base_color.green,
                base_color.blue,
                base_color.alpha,
            ),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            opaque_render_method: OpaqueRendererMethod::Forward,
            ..default()
        },
        extension: SectionMaterialExt::default(),
    }
}

/// 草タイル用 A3（低周波 UV 歪み）の既定振幅（UV 空間）。土・砂・川は `0.0`。
pub const TERRAIN_GRASS_UV_DISTORT_STRENGTH: f32 = 0.03;

/// 草タイル用 A3 明度変調の既定振幅（`1 ±` の係数、`sin` に掛ける）。土・砂・川は `0.0`。
pub const TERRAIN_GRASS_BRIGHTNESS_VARIATION_STRENGTH: f32 = 0.04;

/// 地形タイル専用 `SectionMaterial`。
/// ワールド XZ UV モード・サンプラ Repeat を前提とした設定を有効化する。
/// `uv_scroll_speed`: river は ~0.03（画面上は左→右）、grass/dirt/sand は 0.0（停止）。
/// `uv_distort_strength` / `brightness_variation_strength`: 草のみ非ゼロ可、他は 0.0。
pub fn make_terrain_section_material(
    texture: Handle<Image>,
    uv_scroll_speed: f32,
    uv_distort_strength: f32,
    brightness_variation_strength: f32,
) -> SectionMaterial {
    SectionMaterial {
        base: StandardMaterial {
            base_color_texture: Some(texture),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            opaque_render_method: OpaqueRendererMethod::Forward,
            ..default()
        },
        extension: SectionMaterialExt {
            uniforms: SectionMaterialUniform {
                cut_position: Vec4::ZERO,
                cut_normal: Vec3::NEG_Z.extend(0.0),
                thickness: TILE_SIZE * 5.0,
                cut_active: 0.0,
                build_progress: 1.0,
                wall_height: 0.0,
                albedo_uv_mode: 1.0,
                uv_scale: 1.0 / TILE_SIZE,
                uv_scroll_speed,
                uv_distort_strength,
                brightness_variation_strength,
                _pad_section_tail_0: 0.0,
                _pad_section_tail_1: 0.0,
                _pad_section_tail_2: 0.0,
            },
        },
    }
}

/// テクスチャ付き `SectionMaterial` を生成するヘルパー。
/// テレインタイルのように `Handle<Image>` をベースカラーとして使いたい場合に利用する。
pub fn make_section_material_textured(texture: Handle<Image>) -> SectionMaterial {
    SectionMaterial {
        base: StandardMaterial {
            base_color_texture: Some(texture),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            opaque_render_method: OpaqueRendererMethod::Forward,
            ..default()
        },
        extension: SectionMaterialExt::default(),
    }
}

pub fn with_alpha_mode(mut material: SectionMaterial, alpha_mode: AlphaMode) -> SectionMaterial {
    material.base.alpha_mode = alpha_mode;
    material
}

/// 矢視モードの切断スラブ設定。
#[derive(Resource, Clone, Copy, Debug)]
pub struct SectionCut {
    pub position: Vec3,
    pub normal: Vec3,
    pub thickness: f32,
    pub active: bool,
}

impl Default for SectionCut {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            normal: Vec3::NEG_Z,
            thickness: TILE_SIZE * 5.0,
            active: false,
        }
    }
}

pub fn sync_section_cut_to_materials_system(
    cut: Res<SectionCut>,
    mut materials: ResMut<Assets<SectionMaterial>>,
) {
    if !cut.is_changed() {
        return;
    }

    let cut_position = cut.position.extend(0.0);
    let cut_normal = cut.normal.normalize_or_zero().extend(0.0);
    let thickness = cut.thickness.max(0.0);
    let cut_active = if cut.active { 1.0 } else { 0.0 };

    for (_, material) in materials.iter_mut() {
        material.extension.uniforms.cut_position = cut_position;
        material.extension.uniforms.cut_normal = cut_normal;
        material.extension.uniforms.thickness = thickness;
        material.extension.uniforms.cut_active = cut_active;
    }
}
