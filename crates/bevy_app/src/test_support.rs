//! Minimal Bevy App construction for focused library unit tests.

use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;

pub(crate) fn minimal_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
    app
}

pub(crate) fn empty_building_3d_handles() -> crate::plugins::startup::Building3dHandles {
    crate::plugins::startup::Building3dHandles {
        wall_mesh: Handle::default(),
        wall_material: Handle::default(),
        wall_provisional_material: Handle::default(),
        floor_mesh: Handle::default(),
        floor_material: Handle::default(),
        bridge_mesh: Handle::default(),
        bridge_material: Handle::default(),
        door_mesh: Handle::default(),
        door_closed_material: Handle::default(),
        door_open_material: Handle::default(),
        door_locked_material: Handle::default(),
        equipment_1x1_mesh: Handle::default(),
        equipment_2x2_mesh: Handle::default(),
        equipment_material: Handle::default(),
        tank_partial_material: Handle::default(),
        tank_full_material: Handle::default(),
        mixer_idle_material: Handle::default(),
        mixer_active_material: Handle::default(),
        soul_billboards: crate::plugins::startup::SoulBillboardHandles {
            mesh: Handle::default(),
            normal: Handle::default(),
            exhausted: Handle::default(),
            happy: Handle::default(),
            sleep: Handle::default(),
            wine: Handle::default(),
            trump: Handle::default(),
            stress: Handle::default(),
            stress_breakdown: Handle::default(),
        },
        render_layers: bevy::camera::visibility::RenderLayers::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct UpdateCount(u32);

    fn increment_update_count(mut count: ResMut<UpdateCount>) {
        count.0 += 1;
    }

    #[test]
    fn runs_a_focused_update_system_without_window_or_renderer() {
        let mut app = minimal_app();
        app.init_resource::<UpdateCount>();
        app.add_systems(Update, increment_update_count);

        app.update();

        assert_eq!(app.world().resource::<UpdateCount>().0, 1);
    }
}
