//! キャラクター3Dプロキシ同期・クリーンアップシステム
//!
//! DamnedSoul / Familiar の 2D Transform を対応する 3D プロキシエンティティに毎フレーム同期する。
//! 2D 座標 (x, y) → 3D 座標 (x, height/2, -y) の変換を使用する。

use crate::plugins::startup::{Camera3dRtt, CharacterHandles};
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::gltf::GltfMeshName;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::Mesh3d;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use hw_core::constants::{
    LAYER_3D, LAYER_3D_SOUL_MASK, LAYER_3D_SOUL_SHADOW, SOUL_FACE_SCALE_MULTIPLIER,
    SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES,
};
use hw_core::familiar::Familiar;
use hw_core::soul::DamnedSoul;
use hw_visual::familiar::FamiliarVisualOffset;
use hw_visual::visual3d::{FamiliarProxy3d, SoulMaskProxy3d, SoulProxy3d, SoulShadowProxy3d};
use hw_visual::{
    CharacterMaterial, SoulAnimationPlayer3d, SoulBodyAnimState, SoulFaceMaterial3d,
    SoulMaskMaterial, SoulProxyOwnerCache, SoulShadowMaterial,
};

mod cache;
mod gltf_ready;
mod sync;

pub use cache::{
    cleanup_familiar_proxy_3d_system, cleanup_soul_mask_proxy_3d_system,
    cleanup_soul_proxy_3d_system, cleanup_soul_shadow_proxy_3d_system,
    register_familiar_proxy_3d_system, register_soul_mask_proxy_3d_system,
    register_soul_proxy_3d_system, register_soul_shadow_proxy_3d_system,
};
pub use gltf_ready::{
    SoulGltfApplyParams, SoulMaskGltfApplyParams, SoulShadowGltfApplyParams,
    apply_soul_gltf_render_layers_on_ready, apply_soul_mask_gltf_render_layers_on_ready,
    apply_soul_shadow_gltf_render_layers_on_ready,
};
pub use sync::{
    sync_familiar_proxy_3d_system, sync_soul_mask_proxy_3d_system, sync_soul_proxy_3d_system,
    sync_soul_shadow_proxy_3d_system,
};

#[cfg(test)]
use sync::{familiar_proxy_transform, soul_proxy_transform};

#[cfg(test)]
mod tests;
