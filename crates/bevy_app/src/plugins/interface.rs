//! インターフェース関連のプラグイン

use crate::interface::ui::dev_panel::{
    toggle_instant_build_button_system, toggle_render3d_button_system,
    toggle_rtt_extra_light_button_system, toggle_rtt_light_button_system,
    toggle_rtt_scene_objects_button_system, toggle_rtt_terrain_button_system,
    toggle_soul_mask_button_system, update_instant_build_button_visual_system,
    update_lod_indicator_system, update_render_perf_status_system,
    update_render3d_button_visual_system, update_rtt_extra_light_button_visual_system,
    update_rtt_light_button_visual_system, update_rtt_scene_objects_button_visual_system,
    update_rtt_terrain_button_visual_system, update_soul_mask_button_visual_system,
};
use crate::interface::ui::plugins;
use crate::plugins::interface_debug::debug_spawn_system;
use crate::systems::GameSystemSet;
use bevy::prelude::*;

pub struct InterfacePlugin;

impl Plugin for InterfacePlugin {
    fn build(&self, app: &mut App) {
        plugins::register_ui_plugins(app);
        app.add_systems(
            Update,
            debug_spawn_system
                .run_if(|debug: Res<crate::DebugVisible>| debug.0)
                .in_set(GameSystemSet::Interface),
        );
        app.add_systems(
            Update,
            (
                toggle_render3d_button_system,
                update_render3d_button_visual_system,
                toggle_instant_build_button_system,
                update_instant_build_button_visual_system,
                toggle_soul_mask_button_system,
                update_soul_mask_button_visual_system,
                toggle_rtt_light_button_system,
                update_rtt_light_button_visual_system,
                toggle_rtt_extra_light_button_system,
                update_rtt_extra_light_button_visual_system,
                toggle_rtt_terrain_button_system,
                update_rtt_terrain_button_visual_system,
                toggle_rtt_scene_objects_button_system,
                update_rtt_scene_objects_button_visual_system,
                update_lod_indicator_system,
                update_render_perf_status_system,
            )
                .in_set(GameSystemSet::Interface),
        );
    }
}
