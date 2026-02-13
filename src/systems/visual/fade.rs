use bevy::prelude::*;

#[derive(Component)]
pub struct FadeOut {
    pub speed: f32,
}

pub fn fade_out_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Sprite, &FadeOut)>,
) {
    for (entity, mut sprite, fade) in query.iter_mut() {
        let a = sprite.color.alpha() - fade.speed * time.delta_secs();
        sprite.color.set_alpha(a);
        if a <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
