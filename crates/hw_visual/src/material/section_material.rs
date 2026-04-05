use bevy::pbr::{ExtendedMaterial, MaterialExtension, OpaqueRendererMethod, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};

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
    /// A3 明度変調。`base_color.rgb *= 1 + lf_noise(wx·freq) * 本値`。0.0 で無効
    pub brightness_variation_strength: f32,
    /// ワールド空間の地形域幅（MAP_WIDTH × TILE_SIZE）。地形以外は 0.0（lookup 無効化）
    pub map_world_width: f32,
    /// ワールド空間の地形域高さ（MAP_HEIGHT × TILE_SIZE）。地形以外は 0.0
    pub map_world_height: f32,
    /// domain warp 振幅（ワールド座標スケール）。0.0 で無効。地形タイル専用
    pub domain_warp_strength: f32,
    /// 0=grass, 1=dirt, 2=sand, 3=river。非地形は 0 のままで未使用。
    pub terrain_kind: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SectionMaterialExt {
    #[uniform(100)]
    pub uniforms: SectionMaterialUniform,
    /// 地形フィーチャーマップ（R: shore sand, G: inland sand, B: rock field, A: zone bias）。
    /// `None` のとき Bevy がデフォルト 1×1 白テクスチャをバインドする。建物は `None` のまま。
    #[texture(101)]
    #[sampler(102)]
    pub terrain_feature_map: Option<Handle<Image>>,
    /// 共通 macro noise（RGB）。domain warp と明度ムラに使う。
    #[texture(103)]
    #[sampler(104)]
    pub terrain_macro_noise: Option<Handle<Image>>,
    /// 地形種別ごとの macro overlay。草/土/砂の面の塊感を足す。
    #[texture(105)]
    #[sampler(106)]
    pub terrain_macro_overlay: Option<Handle<Image>>,
    /// 川専用の flow noise。river の V 方向ゆらぎに使う。
    #[texture(107)]
    #[sampler(108)]
    pub river_flow_noise: Option<Handle<Image>>,
    /// feature ごとの tint / roughness を引く LUT（256x1）。
    #[texture(109)]
    #[sampler(110)]
    pub terrain_feature_lut: Option<Handle<Image>>,
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
                map_world_width: 0.0,
                map_world_height: 0.0,
                domain_warp_strength: 0.0,
                terrain_kind: 0.0,
            },
            terrain_feature_map: None,
            terrain_macro_noise: None,
            terrain_macro_overlay: None,
            river_flow_noise: None,
            terrain_feature_lut: None,
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

/// 草タイル用明度変調の既定振幅（`1 ±` の係数、`lf_noise` に掛ける）。
pub const TERRAIN_GRASS_BRIGHTNESS_VARIATION_STRENGTH: f32 = 0.08;

/// 土タイル用明度変調の既定振幅。
pub const TERRAIN_DIRT_BRIGHTNESS_VARIATION_STRENGTH: f32 = 0.10;

/// 砂タイル用明度変調の既定振幅。
pub const TERRAIN_SAND_BRIGHTNESS_VARIATION_STRENGTH: f32 = 0.08;

/// 草タイル用 domain warp 振幅（ワールド座標スケール）。
/// TILE_SIZE=32 のとき最大 ±10 world units ≈ ±0.3 タイル幅のシフトを生む。
pub const TERRAIN_GRASS_DOMAIN_WARP_STRENGTH: f32 = 16.0;

/// 土タイル用 domain warp 振幅。
pub const TERRAIN_DIRT_DOMAIN_WARP_STRENGTH: f32 = 12.0;

/// 砂タイル用 domain warp 振幅。
pub const TERRAIN_SAND_DOMAIN_WARP_STRENGTH: f32 = 10.0;
// 川は domain warp なし（直線的な流れを保つ）

pub const TERRAIN_KIND_GRASS: f32 = 0.0;
pub const TERRAIN_KIND_DIRT: f32 = 1.0;
pub const TERRAIN_KIND_SAND: f32 = 2.0;
pub const TERRAIN_KIND_RIVER: f32 = 3.0;

#[derive(Clone, Debug, Default)]
pub struct TerrainMaterialMaps {
    pub feature_map: Option<Handle<Image>>,
    pub macro_noise: Option<Handle<Image>>,
    pub macro_overlay: Option<Handle<Image>>,
    pub river_flow_noise: Option<Handle<Image>>,
    pub feature_lut: Option<Handle<Image>>,
}

/// 地形タイル専用 `SectionMaterial`。
/// ワールド XZ UV モード・サンプラ Repeat を前提とした設定を有効化する。
///
/// - `maps.feature_map`: フィーチャーマップハンドル。`None` のとき feature tint/lookup 無効。
/// - `maps.macro_noise`: 共通 macro noise。`None` のとき domain warp/brightness は式ベース fallback。
/// - `maps.macro_overlay`: 地形種別 overlay。`None` のとき追加塊感なし。
/// - `maps.river_flow_noise`: 川専用 flow noise。`None` のとき river は式ベース fallback。
/// - `maps.feature_lut`: feature tint / roughness LUT。
/// - `uv_scroll_speed`: river は ~0.03（画面上は左→右）、grass/dirt/sand は 0.0。
/// - `uv_distort_strength` / `brightness_variation_strength`: 草のみ非ゼロ可、他は 0.0。
/// - `domain_warp_strength`: UV サンプル位置をワールド座標スケールでゆらがせる（0.0 で無効）。
pub fn make_terrain_section_material(
    texture: Handle<Image>,
    maps: TerrainMaterialMaps,
    terrain_kind: f32,
    uv_scroll_speed: f32,
    uv_distort_strength: f32,
    brightness_variation_strength: f32,
    domain_warp_strength: f32,
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
                map_world_width: MAP_WIDTH as f32 * TILE_SIZE,
                map_world_height: MAP_HEIGHT as f32 * TILE_SIZE,
                domain_warp_strength,
                terrain_kind,
            },
            terrain_feature_map: maps.feature_map,
            terrain_macro_noise: maps.macro_noise,
            terrain_macro_overlay: maps.macro_overlay,
            river_flow_noise: maps.river_flow_noise,
            terrain_feature_lut: maps.feature_lut,
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
