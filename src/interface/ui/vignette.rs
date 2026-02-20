use crate::game_state::TaskContext;
use bevy::prelude::*;

#[derive(Component)]
pub struct DreamVignette {
    pub timer: f32,
}

pub fn spawn_vignette_ui(
    commands: &mut Commands,
    overlay_parent: Entity,
) {
    let vignette = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.2, 0.4, 0.0)), // より暗く彩度を落とした青
            ZIndex(-10), // オーバーレイの最背面（他のUIの邪魔にならないよう）
            DreamVignette { timer: 0.0 },
            Name::new("Dream Vignette"),
        ))
        .id();

    commands.entity(overlay_parent).add_child(vignette);
}

pub fn update_vignette_system(
    time: Res<Time>,
    task_context: Res<TaskContext>,
    mut q_vignette: Query<(&mut Node, &mut BackgroundColor, &mut DreamVignette)>,
) {
    let is_active_mode = matches!(
        task_context.0,
        crate::systems::command::TaskMode::DreamPlanting(_)
    );

    for (mut node, mut bg_color, mut vignette) in q_vignette.iter_mut() {
        if is_active_mode {
            node.display = Display::Flex;
            vignette.timer += time.delta_secs();
            
            // アルファ値をさらに控えめに (0.02 ~ 0.05 程度)
            let alpha = 0.02 + (vignette.timer * 1.5).sin() * 0.03;
            let mut color = bg_color.0.to_srgba();
            color.alpha = alpha.max(0.0);
            bg_color.0 = color.into();
        } else {
            node.display = Display::None;
            vignette.timer = 0.0;
        }
    }
}
