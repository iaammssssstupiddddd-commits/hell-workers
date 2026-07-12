use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use hw_visual::{
    TerrainSurfaceMaterial, TerrainSurfaceMaterialLod1Lite, TerrainSurfaceMaterialLod2,
};

use crate::plugins::startup::Terrain3dHandles;
use crate::world::map::spawn::GeneratedWorldLayoutResource;

use super::extract::extract_boundary_edges;
use super::geometry::{
    boundary_polyline_noise_params, chamfer_polyline_points, displace_polyline, sample_catmull_rom,
};
use super::params::{
    BOUNDARY_PROXIMITY_RES, CATMULL_ROM_STEPS, CHAMFER_COS_THRESHOLD, CHAMFER_DISTANCE,
    NOISE_AMPLITUDE, NOISE_FREQ, TERRAIN_REGION_RES,
};
use super::polyline::{
    boundary_junction_corner_keys, chain_edges_to_polylines, world_to_corner_key,
};
use super::raster::{
    bake_boundary_proximity_mask, downsample_boundary_proximity_mask, rasterize_terrain_regions,
};
use super::types::BoundarySliceSpatialIndex;

pub fn spawn_boundary_meshes(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    layout: Res<GeneratedWorldLayoutResource>,
    terrain_handles: Res<Terrain3dHandles>,
    mut terrain_surface_materials: ResMut<Assets<TerrainSurfaceMaterial>>,
    mut terrain_surface_materials_lod1_lite: ResMut<Assets<TerrainSurfaceMaterialLod1Lite>>,
    mut terrain_surface_materials_lod2: ResMut<Assets<TerrainSurfaceMaterialLod2>>,
) {
    let terrain_tiles = &layout.layout.terrain_tiles;
    let master_seed = layout.master_seed;

    let edges = extract_boundary_edges(terrain_tiles, &layout.layout.masks);
    let junctions = boundary_junction_corner_keys(&edges);
    let polylines = chain_edges_to_polylines(edges);
    let count = polylines.len();

    let mut sampled_polylines: Vec<Vec<Vec2>> = Vec::new();
    let mut endpoint_blobs: Vec<Vec2> = Vec::new();

    for polyline in polylines {
        let noise = boundary_polyline_noise_params(master_seed, &polyline);
        let displaced =
            displace_polyline(&polyline, &noise, NOISE_FREQ, NOISE_AMPLITUDE, &junctions);
        let chamfered = chamfer_polyline_points(
            &displaced,
            polyline.is_closed,
            &junctions,
            CHAMFER_DISTANCE,
            CHAMFER_COS_THRESHOLD,
        );
        let sampled = sample_catmull_rom(&chamfered, polyline.is_closed, CATMULL_ROM_STEPS);
        if sampled.len() < 2 {
            continue;
        }

        // 非 junction 開端点を endpoint_blobs に追加（ギャップ閉鎖用）
        if !polyline.is_closed {
            if !polyline.points.is_empty()
                && !junctions.contains(&world_to_corner_key(polyline.points[0]))
            {
                endpoint_blobs.push(sampled[0]);
            }
            if polyline.points.len() > 1
                && !junctions.contains(&world_to_corner_key(*polyline.points.last().unwrap()))
            {
                endpoint_blobs.push(*sampled.last().unwrap());
            }
        }

        sampled_polylines.push(sampled);
    }

    let buf = rasterize_terrain_regions(
        terrain_tiles,
        &layout.layout.masks,
        &sampled_polylines,
        &endpoint_blobs,
    );
    let proximity_full = bake_boundary_proximity_mask(&buf, TERRAIN_REGION_RES);
    let proximity_buf = downsample_boundary_proximity_mask(
        &proximity_full,
        TERRAIN_REGION_RES,
        BOUNDARY_PROXIMITY_RES,
    );

    let mut image = Image::new(
        Extent3d {
            width: TERRAIN_REGION_RES as u32,
            height: TERRAIN_REGION_RES as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        buf,
        TextureFormat::R8Unorm,
        default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        ..default()
    });

    let handle = images.add(image);
    let mut proximity_image = Image::new(
        Extent3d {
            width: BOUNDARY_PROXIMITY_RES as u32,
            height: BOUNDARY_PROXIMITY_RES as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        proximity_buf,
        TextureFormat::R8Unorm,
        default(),
    );
    proximity_image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        ..default()
    });
    let proximity_handle = images.add(proximity_image);

    if let Some(mut mat) = terrain_surface_materials.get_mut(&terrain_handles.lod1) {
        mat.extension.boundary_mask = Some(handle.clone());
        mat.extension.boundary_proximity_mask = Some(proximity_handle.clone());
    }
    if let Some(mut mat) = terrain_surface_materials_lod1_lite.get_mut(&terrain_handles.lod1_lite) {
        mat.extension.boundary_mask = Some(handle.clone());
        mat.extension.boundary_proximity_mask = Some(proximity_handle.clone());
    }
    if let Some(mut mat) = terrain_surface_materials_lod2.get_mut(&terrain_handles.lod2) {
        mat.extension.boundary_mask = Some(handle);
        mat.extension.boundary_proximity_mask = Some(proximity_handle);
    }

    commands.insert_resource(BoundarySliceSpatialIndex);

    info!(
        "BEVY_STARTUP: Rasterized terrain_region_map from {} boundary polylines",
        count
    );
}
