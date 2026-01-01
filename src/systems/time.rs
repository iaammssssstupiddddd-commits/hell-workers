use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct GameTime {
    pub seconds: f32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeSpeed {
    Paused,
    Normal,
    Fast,
    Super,
}

#[derive(Component)]
pub struct SpeedButton(pub TimeSpeed);

#[derive(Component)]
pub struct ClockText;

pub fn game_time_system(
    time: Res<Time>,
    mut game_time: ResMut<GameTime>,
    mut q_clock: Query<&mut Text, With<ClockText>>,
) {
    game_time.seconds += time.delta_secs();
    
    let total_mins = (game_time.seconds / 60.0) as u32;
    game_time.minute = total_mins % 60;
    
    let total_hours = total_mins / 60;
    game_time.hour = total_hours % 24;
    
    game_time.day = (total_hours / 24) + 1;

    if let Ok(mut text) = q_clock.get_single_mut() {
        text.0 = format!("Day {}, {:02}:{:02}", game_time.day, game_time.hour, game_time.minute);
    }
}

pub fn time_control_keyboard_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut time: ResMut<Time<Virtual>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
    
    if keyboard.just_pressed(KeyCode::Digit1) {
        time.unpause();
        time.set_relative_speed(1.0);
    }
    if keyboard.just_pressed(KeyCode::Digit2) {
        time.unpause();
        time.set_relative_speed(2.0);
    }
    if keyboard.just_pressed(KeyCode::Digit3) {
        time.unpause();
        time.set_relative_speed(4.0);
    }
}

pub fn time_control_ui_system(
    mut interaction_query: Query<
        (&Interaction, &SpeedButton, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut time: ResMut<Time<Virtual>>,
) {
    for (interaction, speed_button, mut color) in interaction_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                match speed_button.0 {
                    TimeSpeed::Paused => time.pause(),
                    TimeSpeed::Normal => {
                        time.unpause();
                        time.set_relative_speed(1.0);
                    }
                    TimeSpeed::Fast => {
                        time.unpause();
                        time.set_relative_speed(2.0);
                    }
                    TimeSpeed::Super => {
                        time.unpause();
                        time.set_relative_speed(4.0);
                    }
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.4, 0.4, 0.4));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgb(0.2, 0.2, 0.2));
            }
        }
    }
}
