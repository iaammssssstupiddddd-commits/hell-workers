//! 地形フィーチャーマップ生成
//!
//! WFC が生成した `WorldMasks` を起動時に 1 枚の RGBA テクスチャへ焼き付ける。
//! シェーダーはワールド座標でこのテクスチャを参照し、タイルの意味差（shore/inland/rock field）を描画に反映する。

use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};

use super::spawn::GeneratedWorldLayoutResource;

/// 地形フィーチャーマップのハンドルを保持するリソース。
///
/// `build_terrain_feature_map` システムが `PostStartup` 最初期に生成して挿入する。
///
/// テクスチャ形式: `Rgba8Unorm`、サイズ `MAP_WIDTH × MAP_HEIGHT`、nearest サンプリング。
/// - R: shore sand（`final_sand_mask AND NOT inland_sand_mask`）→ 0 or 255
/// - G: inland sand（`inland_sand_mask`）→ 0 or 255
/// - B: rock field（`rock_field_mask`）→ 0 or 255
/// - A: zone bias（grass zone=0, neutral=128, dirt zone=255）
#[derive(Resource)]
pub struct TerrainFeatureMap {
    pub image: Handle<Image>,
}

/// `GeneratedWorldLayoutResource` の `WorldMasks` からフィーチャーマップテクスチャを生成し、
/// `TerrainFeatureMap` リソースとして挿入する `PostStartup` システム。
///
/// `init_visual_handles` より前に `.chain()` で配置すること。
pub fn build_terrain_feature_map(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    layout: Res<GeneratedWorldLayoutResource>,
) {
    let masks = &layout.layout.masks;
    let w = MAP_WIDTH as usize;
    let h = MAP_HEIGHT as usize;

    let mut pixels: Vec<u8> = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let pos = (x as i32, y as i32);
            let is_final_sand = masks.final_sand_mask.get(pos);
            let is_inland_sand = masks.inland_sand_mask.get(pos);
            let shore = if is_final_sand && !is_inland_sand { 255u8 } else { 0u8 };
            let inland = if is_inland_sand { 255u8 } else { 0u8 };
            let rock = if masks.rock_field_mask.get(pos) { 255u8 } else { 0u8 };
            let zone_bias = if masks.grass_zone_mask.get(pos) {
                0u8
            } else if masks.dirt_zone_mask.get(pos) {
                255u8
            } else {
                128u8
            };
            pixels.push(shore);
            pixels.push(inland);
            pixels.push(rock);
            pixels.push(zone_bias);
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        // linear data（色ではなく重み値）のため UnormSrgb でなく Unorm を使う
        TextureFormat::Rgba8Unorm,
        default(),
    );
    // セル境界をシャープに保つため nearest サンプリング
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        ..default()
    });

    let handle = images.add(image);
    commands.insert_resource(TerrainFeatureMap { image: handle });
    info!(
        "BEVY_STARTUP: terrain feature map built ({}×{} px, Rgba8Unorm)",
        w, h
    );
}
