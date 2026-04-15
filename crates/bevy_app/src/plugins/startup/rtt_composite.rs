//! RtT 合成メッシュ（恒久実装）
//!
//! Camera3d がオフスクリーンテクスチャに描画した 3D コンテンツを、
//! Overlay Camera 経由で全画面メッシュに貼り付ける。
//! Soul 専用 mask も同時に受け取り、最終合成時にシルエットを少し丸める。

use crate::plugins::startup::{Camera3dRtt, Camera3dSoulMaskRtt, RttRuntime};
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, MeshMaterial2d};
use bevy::window::PrimaryWindow;
use hw_core::constants::{
    LAYER_OVERLAY, Z_RTT_COMPOSITE, topdown_rtt_vertical_compensation, topdown_sun_direction_world,
};

/// RtT composite entity のマーカー。3D表示切り替えで可視性を制御する。
#[derive(Component)]
pub struct RttCompositeSprite;

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct RttCompositeParams {
    pub pixel_size: Vec2,
    pub mask_radius_px: f32,
    pub mask_feather: f32,
    pub shadow_offset_uv: Vec2,
    pub shadow_width_px: f32,
    pub shadow_strength: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct RttCompositeMaterial {
    #[uniform(0)]
    pub params: RttCompositeParams,
    #[texture(1)]
    #[sampler(2)]
    pub scene_texture: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub soul_mask_texture: Handle<Image>,
}

impl Material2d for RttCompositeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/rtt_composite_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// RtT テクスチャをワールド原点に固定した全画面メッシュとして合成表示する。
pub fn spawn_rtt_composite_sprite(
    mut commands: Commands,
    runtime: Res<RttRuntime>,
    perf_toggles: Res<crate::RenderPerfToggles>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RttCompositeMaterial>>,
) {
    let custom_size = q_window.single().ok().map(composite_logical_size);
    let mesh = meshes.add(Rectangle::default().mesh());
    let size = custom_size.unwrap_or(Vec2::new(1280.0, 720.0));
    let material = materials.add(RttCompositeMaterial {
        params: RttCompositeParams {
            pixel_size: runtime.pixel_size(),
            mask_radius_px: if perf_toggles.soul_mask_enabled {
                2.25
            } else {
                0.0
            },
            mask_feather: 0.28,
            shadow_offset_uv: Vec2::new(0.018, -0.012),
            shadow_width_px: 22.0,
            shadow_strength: 0.0,
        },
        scene_texture: runtime.scene.clone(),
        soul_mask_texture: runtime.soul_mask.clone(),
    });

    commands.spawn((
        Mesh2d(mesh),
        MeshMaterial2d(material),
        Transform::from_xyz(0.0, 0.0, Z_RTT_COMPOSITE).with_scale(size.extend(1.0)),
        Visibility::Visible,
        RenderLayers::layer(LAYER_OVERLAY),
        RttCompositeSprite,
    ));
}

/// RtT の出力先と合成マテリアルの参照を同期する。
pub fn sync_rtt_output_bindings(
    runtime: Res<RttRuntime>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut main_camera_targets: Query<
        &mut RenderTarget,
        (With<Camera3dRtt>, Without<Camera3dSoulMaskRtt>),
    >,
    mut soul_mask_targets: Query<
        &mut RenderTarget,
        (With<Camera3dSoulMaskRtt>, Without<Camera3dRtt>),
    >,
    mut quads: Query<
        (&MeshMaterial2d<RttCompositeMaterial>, &mut Transform),
        With<RttCompositeSprite>,
    >,
    mut materials: ResMut<Assets<RttCompositeMaterial>>,
) {
    let logical_size = q_window.single().ok().map(composite_logical_size);

    // メッシュスケールはウィンドウリサイズで常時追従（RttRuntime 変化とは独立）
    for (_, mut tf) in quads.iter_mut() {
        if let Some(size) = logical_size {
            tf.scale = size.extend(1.0);
        }
        tf.translation.z = Z_RTT_COMPOSITE;
    }

    // テクスチャ参照とカメラ RenderTarget の差し替えは RttRuntime が変化したときだけ行う
    if !runtime.is_changed() {
        return;
    }

    if let Ok(mut target) = main_camera_targets.single_mut() {
        *target = RenderTarget::Image(runtime.scene.clone().into());
    }
    if let Ok(mut target) = soul_mask_targets.single_mut() {
        *target = RenderTarget::Image(runtime.soul_mask.clone().into());
    }
    for (material_handle, _) in quads.iter() {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.scene_texture = runtime.scene.clone();
            material.soul_mask_texture = runtime.soul_mask.clone();
            material.params.pixel_size = runtime.pixel_size();
        }
    }
}

/// Soul mask の有効/無効に合わせて composite material のマスク半径を更新する。
pub fn sync_rtt_composite_perf_params_system(
    perf_toggles: Res<crate::RenderPerfToggles>,
    q_camera: Query<(&Transform, &Projection), With<Camera3dRtt>>,
    quads: Query<&MeshMaterial2d<RttCompositeMaterial>, With<RttCompositeSprite>>,
    mut materials: ResMut<Assets<RttCompositeMaterial>>,
) {
    let Ok((camera_transform, projection)) = q_camera.single() else {
        return;
    };

    let shadow_offset_uv = composite_shadow_offset_uv(camera_transform, projection);
    if !perf_toggles.is_changed() && shadow_offset_uv.is_none() {
        return;
    }

    let next_radius = if perf_toggles.soul_mask_enabled {
        2.25
    } else {
        0.0
    };

    for material_handle in quads.iter() {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.params.mask_radius_px = next_radius;
            if let Some(offset_uv) = shadow_offset_uv {
                material.params.shadow_offset_uv = offset_uv;
            }
        }
    }
}

/// RtT 合成メッシュの論理サイズを返す（LOD 観測システムと共有）。
pub(crate) fn composite_logical_size(window: &Window) -> Vec2 {
    let size = window.size();
    Vec2::new(size.x, size.y * topdown_rtt_vertical_compensation())
}

fn composite_shadow_offset_uv(
    camera_transform: &Transform,
    projection: &Projection,
) -> Option<Vec2> {
    let Projection::Orthographic(ortho) = projection else {
        return None;
    };

    let area_size = ortho.area.size();
    if area_size.x.abs() <= f32::EPSILON || area_size.y.abs() <= f32::EPSILON {
        return None;
    }

    let shadow_world = -topdown_sun_direction_world() * 34.0;
    let right = *camera_transform.right();
    let up = *camera_transform.up();

    Some(Vec2::new(
        shadow_world.dot(right) / area_size.x,
        -shadow_world.dot(up) / area_size.y,
    ))
}
