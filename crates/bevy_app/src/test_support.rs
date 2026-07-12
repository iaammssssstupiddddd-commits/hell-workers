//! Minimal Bevy App construction for focused library unit tests.

use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;

pub(crate) fn minimal_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
    app
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
