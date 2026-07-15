pub mod app_contexts;
mod assets;
mod entities;
pub mod interface;
pub mod plugins;
pub mod systems;
pub mod world;

use std::env;

use bevy::prelude::*;

pub use entities::damned_soul::DamnedSoulPlugin;
pub use hw_core::events::{
    DesignationRequest, EncouragementRequest, EscapeRequest, FamiliarAiStateChangedEvent,
    FamiliarIdleVisualRequest, FamiliarOperationMaxSoulChangedEvent, FamiliarStateRequest,
    GatheringManagementRequest, GatheringSpawnRequest, IdleBehaviorRequest, OnExhausted,
    OnGatheringParticipated, OnSoulRecruited, OnStressBreakdown, OnTaskAbandoned, OnTaskAssigned,
    OnTaskCompleted, ResourceReservationRequest, SoulTaskUnassignRequest, SquadManagementOperation,
    SquadManagementRequest,
};
pub use hw_jobs::events::TaskAssignmentRequest;
pub use plugins::game::HellWorkersGamePlugin;

/// ゲーム内のデバッグ情報の表示状態（独自実装用）
#[derive(Resource, Default)]
pub struct DebugVisible(pub bool);

/// 3D表示（RtT レンダリング）の有効/無効状態
#[derive(Resource)]
pub struct Render3dVisible(pub bool);

/// 3D RtT の固定費を切り分けるための個別トグル。
#[derive(Resource)]
pub struct RenderPerfToggles {
    pub soul_mask_enabled: bool,
    pub directional_light_enabled: bool,
    pub extra_directional_light_enabled: bool,
    pub terrain_enabled: bool,
    pub scene_objects_enabled: bool,
}

/// デバッグ用：壁建築を即時完成させるトグル
#[derive(Resource, Default)]
pub struct DebugInstantBuild(pub bool);

impl Default for Render3dVisible {
    fn default() -> Self {
        Self(true)
    }
}

impl Default for RenderPerfToggles {
    fn default() -> Self {
        Self {
            soul_mask_enabled: !env_flag_is_true("HW_DISABLE_SOUL_MASK"),
            directional_light_enabled: !env_flag_is_true("HW_DISABLE_RTT_DIRECTIONAL_LIGHT"),
            extra_directional_light_enabled: env_flag_is_true(
                "HW_ENABLE_RTT_EXTRA_DIRECTIONAL_LIGHT",
            ),
            terrain_enabled: !env_flag_is_true("HW_DISABLE_RTT_TERRAIN"),
            scene_objects_enabled: !env_flag_is_true("HW_DISABLE_RTT_SCENE_OBJECTS"),
        }
    }
}

impl RenderPerfToggles {
    pub const fn all_disabled() -> Self {
        Self {
            soul_mask_enabled: false,
            directional_light_enabled: false,
            extra_directional_light_enabled: false,
            terrain_enabled: false,
            scene_objects_enabled: false,
        }
    }

    pub const fn gpu_baseline() -> Self {
        Self {
            soul_mask_enabled: true,
            directional_light_enabled: true,
            extra_directional_light_enabled: false,
            terrain_enabled: true,
            scene_objects_enabled: true,
        }
    }
}

fn env_flag_is_true(name: &str) -> bool {
    env::var(name)
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
}

#[cfg(test)]
pub(crate) mod test_support;
