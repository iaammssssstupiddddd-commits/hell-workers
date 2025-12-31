use bevy::prelude::*;

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    App::new()
        .insert_resource(ClearColor(Color::srgb(0.5, 0.2, 0.2)))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                canvas: Some("#bevy".to_string()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }).set(bevy::log::LogPlugin {
            level: bevy::log::Level::DEBUG,
            filter: "wgpu=error,bevy_render=info,bevy_ecs=debug".to_string(),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, log_periodically)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    info!("BEVY_STARTUP: Setup completed");
    println!("BEVY_STARTUP: println works!");
}

fn log_periodically(time: Res<Time>, mut timer: Local<f32>) {
    *timer += time.delta_secs();
    if *timer > 1.0 {
        info!("BEVY_RUNNING: Current time is {:.1}", time.elapsed_secs());
        *timer = 0.0;
    }
}
