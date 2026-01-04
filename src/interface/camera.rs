use bevy::prelude::*;

#[derive(Component)]
pub struct MainCamera;

pub fn camera_movement(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &OrthographicProjection), With<MainCamera>>,
) {
    let (mut transform, projection) = query.single_mut();
    let mut direction = Vec3::ZERO;

    if keyboard_input.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }

    if direction != Vec3::ZERO {
        let speed = 500.0 * projection.scale;
        transform.translation += direction.normalize() * speed * time.delta_secs();
    }
}

pub fn camera_zoom(
    mut mouse_wheel_events: EventReader<bevy::input::mouse::MouseWheel>,
    mut query: Query<&mut OrthographicProjection, With<MainCamera>>,
) {
    let mut projection = query.single_mut();

    for event in mouse_wheel_events.read() {
        let zoom_factor = 1.1;
        if event.y > 0.0 {
            projection.scale /= zoom_factor;
        } else if event.y < 0.0 {
            projection.scale *= zoom_factor;
        }
    }

    projection.scale = projection.scale.clamp(0.1, 5.0);
}
