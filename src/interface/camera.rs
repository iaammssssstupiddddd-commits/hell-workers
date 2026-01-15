use bevy::prelude::*;
use bevy::camera::Projection;

#[derive(Component)]
pub struct MainCamera;

pub fn camera_movement(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Transform, &Projection), With<MainCamera>>,
) {
    let Ok((mut transform, projection)) = query.single_mut() else { return; };
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
        if let Projection::Orthographic(ortho) = projection {
            let speed = 500.0 * ortho.scale;
            transform.translation += direction.normalize() * speed * time.delta_secs();
        }
    }
}

pub fn camera_zoom(
    mut mouse_wheel_events: MessageReader<bevy::input::mouse::MouseWheel>,
    mut query: Query<&mut Projection, With<MainCamera>>,
) {
    let Ok(mut projection) = query.single_mut() else { return; };

    if let Projection::Orthographic(ref mut ortho) = *projection {
        for event in mouse_wheel_events.read() {
            let zoom_factor = 1.1;
            if event.y > 0.0 {
                ortho.scale /= zoom_factor;
            } else if event.y < 0.0 {
                ortho.scale *= zoom_factor;
            }
        }

        ortho.scale = ortho.scale.clamp(0.1, 5.0);
    }
}
