pub mod animations;
pub mod blueprint;
pub mod dream;
pub mod fade;
pub mod floating_text;
pub mod floor_construction;
pub mod gather;
pub mod handles;
pub mod haul;
pub mod mud_mixer;
pub mod plant_trees;
pub mod progress_bar;
pub mod selection_indicator;
pub mod site_yard_visual;
pub mod soul;
pub mod speech;
pub mod tank;
pub mod task_area_visual;
pub mod wall_connection;
pub mod wall_construction;
pub mod worker_icon;

pub use selection_indicator::update_selection_indicator;

pub use handles::{
    BuildingAnimHandles, GatheringVisualHandles, HaulItemHandles, MaterialIconHandles,
    PlantTreeHandles, SpeechHandles, WallVisualHandles, WorkIconHandles,
};

pub use hw_core::visual::SoulTaskHandles;
pub use task_area_visual::{TaskAreaMaterial, TaskAreaVisual};

use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;
use bevy::ui_render::prelude::UiMaterialPlugin;
use hw_core::system_sets::GameSystemSet;

pub struct HwVisualPlugin;

impl Plugin for HwVisualPlugin {
    fn build(&self, app: &mut App) {
        // Material plugins (型の所有者が登録)
        app.add_plugins((
            Material2dPlugin::<dream::DreamBubbleMaterial>::default(),
            UiMaterialPlugin::<dream::DreamBubbleUiMaterial>::default(),
            Material2dPlugin::<TaskAreaMaterial>::default(),
        ));

        app.add_plugins(speech::SpeechPlugin);

        // Standalone systems
        app.add_systems(
            Update,
            (
                wall_connection::wall_connections_system,
                site_yard_visual::sync_site_yard_boundaries_system,
            )
                .in_set(GameSystemSet::Visual),
        );

        // Blueprint visual systems
        app.add_systems(
            Update,
            (
                blueprint::attach_blueprint_visual_system,
                blueprint::update_blueprint_visual_system,
                blueprint::blueprint_pulse_animation_system,
                blueprint::blueprint_scale_animation_system,
                blueprint::spawn_progress_bar_system,
                blueprint::update_progress_bar_fill_system,
                blueprint::sync_progress_bar_position_system,
                blueprint::cleanup_progress_bars_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Soul progress bar + status visual systems
        app.add_systems(
            Update,
            (
                soul::progress_bar_system,
                soul::update_progress_bar_fill_system,
                soul::sync_progress_bar_position_system,
                soul::soul_status_visual_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Soul idle / gathering / vitals visual systems
        app.add_systems(
            Update,
            (
                soul::idle::idle_visual_system,
                soul::vitals::familiar_hover_visualization_system,
                soul::gathering::gathering_visual_update_system,
                soul::gathering::gathering_debug_visualization_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Blueprint detail systems
        app.add_systems(
            Update,
            (
                blueprint::spawn_material_display_system,
                blueprint::update_material_counter_system,
                blueprint::material_delivery_vfx_system,
                blueprint::update_delivery_popup_system,
                blueprint::update_completion_text_system,
                blueprint::building_bounce_animation_system,
                blueprint::spawn_worker_indicators_system,
                blueprint::update_worker_indicators_system,
                blueprint::cleanup_material_display_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Gather + resource highlight + fade systems
        app.add_systems(
            Update,
            (
                gather::spawn_gather_indicators_system,
                gather::update_gather_indicators_system,
                gather::attach_resource_visual_system,
                gather::update_resource_visual_system,
                gather::cleanup_resource_visual_system,
                fade::fade_out_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Haul visual systems
        app.add_systems(
            Update,
            (
                haul::spawn_carrying_item_system,
                haul::update_carrying_item_system,
                haul::update_drop_popup_system,
                haul::wheelbarrow_follow_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Dream + floating text systems
        app.add_systems(
            Update,
            (
                dream::ensure_dream_visual_state_system,
                dream::dream_particle_spawn_system,
                dream::rest_area_dream_particle_spawn_system,
                dream::dream_popup_spawn_system,
                dream::dream_particle_update_system,
                dream::dream_popup_update_system,
                dream::ui_particle_update_system,
                dream::ui_particle_merge_system,
                dream::dream_trail_ghost_update_system,
                dream::dream_icon_absorb_system,
                floating_text::update_all_floating_texts_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Plant trees visual systems
        app.add_systems(
            Update,
            (
                plant_trees::setup_plant_tree_visual_state_system,
                plant_trees::update_plant_tree_magic_circle_system,
                plant_trees::update_plant_tree_growth_system,
                plant_trees::update_plant_tree_life_spark_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );

        // Building animation systems
        app.add_systems(
            Update,
            (
                tank::update_tank_visual_system,
                mud_mixer::update_mud_mixer_visual_system,
            )
                .in_set(GameSystemSet::Visual),
        );

        // Floor / wall construction visual systems
        app.add_systems(
            Update,
            (
                floor_construction::manage_floor_curing_progress_bars_system,
                floor_construction::update_floor_curing_progress_bars_system,
                floor_construction::update_floor_tile_visuals_system,
                floor_construction::sync_floor_tile_bone_visuals_system,
                wall_construction::manage_wall_progress_bars_system,
                wall_construction::update_wall_progress_bars_system,
                wall_construction::update_wall_tile_visuals_system,
            )
                .chain()
                .in_set(GameSystemSet::Visual),
        );
    }
}
