use bevy::prelude::*;

#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
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
pub struct ClockText;

pub fn game_time_system(
    time: Res<Time<Virtual>>,
    mut game_time: ResMut<GameTime>,
    mut q_clock: Query<&mut Text, With<ClockText>>,
) {
    // 1秒(実時間) = 1分(ゲーム中) に調整 (60倍速)
    game_time.seconds += time.delta_secs() * 60.0;

    let total_mins = (game_time.seconds / 60.0) as u32;
    game_time.minute = total_mins % 60;

    let total_hours = total_mins / 60;
    game_time.hour = total_hours % 24;

    game_time.day = (total_hours / 24) + 1;

    if let Ok(mut text) = q_clock.single_mut() {
        text.0 = format!(
            "Day {}, {:02}:{:02}",
            game_time.day, game_time.hour, game_time.minute
        );
    }
}
