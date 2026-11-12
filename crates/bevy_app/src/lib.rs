pub mod app_contexts;
mod assets;
mod entities;
pub mod input_actions;
pub mod interface;
pub mod plugins;
#[cfg(feature = "profiling-memory")]
pub(crate) mod profiling_allocator;
pub mod systems;
pub mod world;

use std::env;

use bevy::prelude::*;

#[cfg(feature = "profiling")]
pub use assets::building_asset_set::acceptance::configure_building_art;
#[cfg(feature = "profiling")]
pub use assets::building_asset_set::acceptance::m2_ui_pair_admitted_from_process;
pub use assets::building_asset_set::configure_building_asset_releases;
pub use assets::building_asset_set::project_building_asset_json;
pub use entities::damned_soul::DamnedSoulPlugin;
pub use hw_core::events::{
    DesignationRequest, EncouragementRequest, EscapeRequest, FamiliarAiStateChangedEvent,
    FamiliarIdleVisualRequest, FamiliarRosterReleasedVisualMessage, FamiliarStateRequest,
    GatheringManagementRequest, GatheringSpawnRequest, IdleBehaviorRequest, OnExhausted,
    OnGatheringParticipated, OnSoulRecruited, OnStressBreakdown, OnTaskAbandoned, OnTaskAssigned,
    OnTaskCompleted, ResourceReservationRequest, SoulTaskUnassignRequest, SquadManagementOperation,
    SquadManagementRequest,
};
pub use hw_familiar_ai::{
    FamiliarSettingsChangeOutcome, FamiliarSettingsChangeRequest, FamiliarSettingsChangeStatus,
    FamiliarSettingsRejection,
};
pub use hw_jobs::events::TaskAssignmentRequest;
pub use plugins::game::HellWorkersGamePlugin;

/// Only an admitted profiling group gallery gets a frameless fixed client.
/// Normal gameplay, older galleries and performance windows retain decoration.
pub fn building_art_window_decorations(world: &World) -> bool {
    #[cfg(feature = "profiling")]
    {
        !world.contains_resource::<assets::building_asset_set::group_art::GroupArtSession>()
    }
    #[cfg(not(feature = "profiling"))]
    {
        let _ = world;
        true
    }
}

/// ゲーム内のデバッグ情報の表示状態（独自実装用）
#[derive(Resource, Default)]
pub struct DebugVisible(pub bool);

/// 3D表示（RtT レンダリング）の有効/無効状態
#[derive(Resource)]
pub struct Render3dVisible(pub bool);

/// 3D RtT の固定費を切り分けるための個別トグル。
#[derive(Resource)]
pub struct RenderPerfToggles {
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
            directional_light_enabled: false,
            extra_directional_light_enabled: false,
            terrain_enabled: false,
            scene_objects_enabled: false,
        }
    }

    pub const fn gpu_baseline() -> Self {
        Self {
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

#[cfg(test)]
mod group_art_window_tests {
    use super::*;

    #[test]
    fn ordinary_world_keeps_its_existing_window_decoration() {
        assert!(building_art_window_decorations(&World::new()));
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn only_the_typed_admitted_group_resource_selects_a_frameless_client() {
        let mut world = World::new();
        let session = assets::building_asset_set::group_art::test_session();
        world.insert_resource(session);
        assert!(!building_art_window_decorations(&world));
        world.remove_resource::<assets::building_asset_set::group_art::GroupArtSession>();
        assert!(building_art_window_decorations(&world));
    }
}
