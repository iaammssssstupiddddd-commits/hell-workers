//! RtT 合成メッシュ（恒久実装）
//!
//! Camera3d がオフスクリーンテクスチャに描画した 3D コンテンツを、
//! Overlay Camera 経由で全画面メッシュに貼り付ける。
//! Soul 専用 mask も同時に受け取り、最終合成時にシルエットを少し丸める。

use crate::plugins::startup::{Camera3dRtt, Camera3dSoulMaskRtt, RttTextures, RttViewportSize};
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, MeshMaterial2d};
use bevy::window::PrimaryWindow;
use hw_core::constants::{LAYER_OVERLAY, Z_RTT_COMPOSITE, topdown_rtt_vertical_compensation};

/// RtT composite entity のマーカー。3D表示切り替えで可視性を制御する。
#[derive(Component)]
pub struct RttCompositeSprite;

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct RttCompositeParams {
    pub pixel_size: Vec2,
    pub mask_radius_px: f32,
    pub mask_feather: f32,
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
    rtt: Res<RttTextures>,
    viewport_size: Res<RttViewportSize>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RttCompositeMaterial>>,
) {
    let custom_size = q_window.single().ok().map(logical_composite_size);
    let mesh = meshes.add(Rectangle::default().mesh());
    let size = custom_size.unwrap_or(Vec2::new(1280.0, 720.0));
    let material = materials.add(RttCompositeMaterial {
        params: RttCompositeParams {
            pixel_size: viewport_size.pixel_size(),
            mask_radius_px: 2.25,
            mask_feather: 0.28,
        },
        scene_texture: rtt.texture_3d.clone(),
        soul_mask_texture: rtt.texture_soul_mask.clone(),
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
    rtt: Res<RttTextures>,
    viewport_size: Res<RttViewportSize>,
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
    let logical_size = q_window.single().ok().map(logical_composite_size);

    if let Ok(mut target) = main_camera_targets.single_mut() {
        *target = RenderTarget::Image(rtt.texture_3d.clone().into());
    }
    if let Ok(mut target) = soul_mask_targets.single_mut() {
        *target = RenderTarget::Image(rtt.texture_soul_mask.clone().into());
    }

    for (material_handle, mut tf) in quads.iter_mut() {
        if let Some(size) = logical_size {
            tf.scale = size.extend(1.0);
        }
        tf.translation.z = Z_RTT_COMPOSITE;

        if !rtt.is_changed() && !viewport_size.is_changed() && logical_size.is_none() {
            continue;
        }

        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.scene_texture = rtt.texture_3d.clone();
            material.soul_mask_texture = rtt.texture_soul_mask.clone();
            material.params.pixel_size = viewport_size.pixel_size();
        }
    }
}

fn logical_composite_size(window: &Window) -> Vec2 {
    let size = window.size();
    Vec2::new(size.x, size.y * topdown_rtt_vertical_compensation())
}
