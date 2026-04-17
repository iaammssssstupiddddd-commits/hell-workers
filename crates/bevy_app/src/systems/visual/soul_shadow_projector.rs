use crate::plugins::startup::{Building3dHandles, Camera3dRtt, Terrain3dHandles};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::{
    MAX_SOUL_SHADOW_PROJECTORS, SOUL_SHADOW_PROJECTOR_FEATHER,
    SOUL_SHADOW_PROJECTOR_FORWARD_EXTENT, SOUL_SHADOW_PROJECTOR_RADIUS,
    SOUL_SHADOW_PROJECTOR_STRENGTH,
};
use hw_core::soul::DamnedSoul;
use hw_visual::{
    SectionMaterial, TerrainSurfaceMaterial, TerrainSurfaceMaterialLod1Lite,
    TerrainSurfaceMaterialLod2,
};

#[derive(SystemParam)]
pub struct SyncSoulShadowProjectorsParams<'w, 's> {
    building_handles: Res<'w, Building3dHandles>,
    terrain_handles: Res<'w, Terrain3dHandles>,
    section_materials: ResMut<'w, Assets<SectionMaterial>>,
    terrain_surface_materials: ResMut<'w, Assets<TerrainSurfaceMaterial>>,
    terrain_surface_materials_lod1_lite: ResMut<'w, Assets<TerrainSurfaceMaterialLod1Lite>>,
    terrain_surface_materials_lod2: ResMut<'w, Assets<TerrainSurfaceMaterialLod2>>,
    _marker: Local<'s, ()>,
}

pub fn sync_soul_shadow_projectors_system(
    q_souls: Query<&Transform, With<DamnedSoul>>,
    q_camera: Query<&Transform, With<Camera3dRtt>>,
    mut params: SyncSoulShadowProjectorsParams,
) {
    let Ok(camera_transform) = q_camera.single() else {
        return;
    };

    let camera_center = Vec2::new(
        camera_transform.translation.x,
        camera_transform.translation.z,
    );
    let mut projectors = q_souls
        .iter()
        .map(|transform| {
            let world_center = Vec3::new(transform.translation.x, 0.0, -transform.translation.y);
            let dx = world_center.x - camera_center.x;
            let dz = world_center.z - camera_center.y;
            let distance_sq = dx * dx + dz * dz;
            (distance_sq, world_center)
        })
        .collect::<Vec<_>>();
    if projectors.len() > MAX_SOUL_SHADOW_PROJECTORS {
        projectors.select_nth_unstable_by(MAX_SOUL_SHADOW_PROJECTORS, |a, b| {
            a.0.total_cmp(&b.0)
        });
        projectors.truncate(MAX_SOUL_SHADOW_PROJECTORS);
    }
    projectors.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut projector_array = [Vec4::ZERO; MAX_SOUL_SHADOW_PROJECTORS];
    let projector_count = projectors.len().min(MAX_SOUL_SHADOW_PROJECTORS);
    for (slot, (_, center)) in projectors.into_iter().take(projector_count).enumerate() {
        projector_array[slot] = center.extend(SOUL_SHADOW_PROJECTOR_RADIUS);
    }
    let projector_meta = Vec4::new(
        projector_count as f32,
        SOUL_SHADOW_PROJECTOR_FEATHER,
        SOUL_SHADOW_PROJECTOR_STRENGTH,
        SOUL_SHADOW_PROJECTOR_FORWARD_EXTENT,
    );

    let building_handles = params.building_handles.as_ref();
    let terrain_handles = params.terrain_handles.as_ref();

    sync_section_material_projectors(
        &mut params.section_materials,
        &building_handles.wall_material,
        &projector_array,
        projector_meta,
    );
    sync_section_material_projectors(
        &mut params.section_materials,
        &building_handles.wall_provisional_material,
        &projector_array,
        projector_meta,
    );
    sync_terrain_surface_material_projectors(
        &mut params.terrain_surface_materials,
        &terrain_handles.lod1,
        &projector_array,
        projector_meta,
    );
    sync_terrain_surface_material_lod1_lite_projectors(
        &mut params.terrain_surface_materials_lod1_lite,
        &terrain_handles.lod1_lite,
        &projector_array,
        projector_meta,
    );
    sync_terrain_surface_material_lod2_projectors(
        &mut params.terrain_surface_materials_lod2,
        &terrain_handles.lod2,
        &projector_array,
        projector_meta,
    );
}

fn sync_section_material_projectors(
    materials: &mut Assets<SectionMaterial>,
    handle: &Handle<SectionMaterial>,
    projectors: &[Vec4; MAX_SOUL_SHADOW_PROJECTORS],
    projector_meta: Vec4,
) {
    let Some(material) = materials.get_mut(handle) else {
        return;
    };
    let uniforms = &mut material.extension.uniforms;
    if uniforms.soul_shadow_projectors == *projectors
        && uniforms.soul_shadow_projector_meta == projector_meta
    {
        return;
    }
    uniforms.soul_shadow_projectors = *projectors;
    uniforms.soul_shadow_projector_meta = projector_meta;
}

fn sync_terrain_surface_material_projectors(
    materials: &mut Assets<TerrainSurfaceMaterial>,
    handle: &Handle<TerrainSurfaceMaterial>,
    projectors: &[Vec4; MAX_SOUL_SHADOW_PROJECTORS],
    projector_meta: Vec4,
) {
    let Some(material) = materials.get_mut(handle) else {
        return;
    };
    let uniforms = &mut material.extension.uniforms;
    if uniforms.soul_shadow_projectors == *projectors
        && uniforms.soul_shadow_projector_meta == projector_meta
    {
        return;
    }
    uniforms.soul_shadow_projectors = *projectors;
    uniforms.soul_shadow_projector_meta = projector_meta;
}

fn sync_terrain_surface_material_lod1_lite_projectors(
    materials: &mut Assets<TerrainSurfaceMaterialLod1Lite>,
    handle: &Handle<TerrainSurfaceMaterialLod1Lite>,
    projectors: &[Vec4; MAX_SOUL_SHADOW_PROJECTORS],
    projector_meta: Vec4,
) {
    let Some(material) = materials.get_mut(handle) else {
        return;
    };
    let uniforms = &mut material.extension.uniforms;
    if uniforms.soul_shadow_projectors == *projectors
        && uniforms.soul_shadow_projector_meta == projector_meta
    {
        return;
    }
    uniforms.soul_shadow_projectors = *projectors;
    uniforms.soul_shadow_projector_meta = projector_meta;
}

fn sync_terrain_surface_material_lod2_projectors(
    materials: &mut Assets<TerrainSurfaceMaterialLod2>,
    handle: &Handle<TerrainSurfaceMaterialLod2>,
    projectors: &[Vec4; MAX_SOUL_SHADOW_PROJECTORS],
    projector_meta: Vec4,
) {
    let Some(material) = materials.get_mut(handle) else {
        return;
    };
    let uniforms = &mut material.extension.uniforms;
    if uniforms.soul_shadow_projectors == *projectors
        && uniforms.soul_shadow_projector_meta == projector_meta
    {
        return;
    }
    uniforms.soul_shadow_projectors = *projectors;
    uniforms.soul_shadow_projector_meta = projector_meta;
}
