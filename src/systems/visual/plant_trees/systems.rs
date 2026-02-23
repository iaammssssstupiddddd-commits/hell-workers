use super::components::{
    PlantTreeLifeSpark, PlantTreeMagicCircle, PlantTreeVisualPhase, PlantTreeVisualState,
};
use crate::assets::GameAssets;
use crate::constants::*;
use bevy::prelude::*;
use rand::Rng;

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

pub fn setup_plant_tree_visual_state_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut q_new_trees: Query<(Entity, &mut Transform, &mut Sprite), Added<PlantTreeVisualState>>,
) {
    for (_tree_entity, mut tree_transform, mut tree_sprite) in q_new_trees.iter_mut() {
        tree_transform.scale = Vec3::splat(DREAM_TREE_GROWTH_SCALE_START);
        let (r, g, b, a) = DREAM_TREE_GROWTH_GLOW_COLOR;
        tree_sprite.color = Color::srgba(r, g, b, a);

        let (circle_r, circle_g, circle_b, circle_a) = DREAM_TREE_MAGIC_CIRCLE_COLOR;
        commands.spawn((
            PlantTreeMagicCircle { elapsed: 0.0 },
            Sprite {
                image: game_assets.plant_tree_magic_circle.clone(),
                color: Color::srgba(circle_r, circle_g, circle_b, circle_a * 0.0),
                custom_size: Some(Vec2::splat(TILE_SIZE * 1.2)),
                ..default()
            },
            Transform::from_xyz(
                tree_transform.translation.x,
                tree_transform.translation.y,
                Z_DREAM_TREE_MAGIC_CIRCLE,
            )
            .with_scale(Vec3::splat(DREAM_TREE_MAGIC_CIRCLE_SCALE_START)),
            Name::new("PlantTreeMagicCircle"),
        ));
    }
}

pub fn update_plant_tree_magic_circle_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_magic_circles: Query<(
        Entity,
        &mut PlantTreeMagicCircle,
        &mut Transform,
        &mut Sprite,
    )>,
) {
    let dt = time.delta_secs();
    let (r, g, b, a) = DREAM_TREE_MAGIC_CIRCLE_COLOR;

    for (entity, mut circle, mut transform, mut sprite) in q_magic_circles.iter_mut() {
        circle.elapsed += dt;
        let ratio = (circle.elapsed / DREAM_TREE_MAGIC_CIRCLE_DURATION).clamp(0.0, 1.0);
        let alpha = if ratio < 0.35 {
            ratio / 0.35
        } else {
            (1.0 - ratio) / 0.65
        }
        .clamp(0.0, 1.0);

        sprite.color = Color::srgba(r, g, b, a * alpha);
        transform.scale = Vec3::splat(lerp(
            DREAM_TREE_MAGIC_CIRCLE_SCALE_START,
            DREAM_TREE_MAGIC_CIRCLE_SCALE_END,
            ratio,
        ));

        if ratio >= 1.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

pub fn update_plant_tree_growth_system(
    mut commands: Commands,
    time: Res<Time>,
    game_assets: Res<GameAssets>,
    mut q_trees: Query<(
        Entity,
        &mut PlantTreeVisualState,
        &mut Transform,
        &mut Sprite,
    )>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (tree_entity, mut visual_state, mut transform, mut sprite) in q_trees.iter_mut() {
        visual_state.phase_elapsed += dt;

        match visual_state.phase {
            PlantTreeVisualPhase::MagicCircle => {
                transform.scale = Vec3::splat(DREAM_TREE_GROWTH_SCALE_START);
                let (r, g, b, a) = DREAM_TREE_GROWTH_GLOW_COLOR;
                sprite.color = Color::srgba(r, g, b, a);
                if visual_state.phase_elapsed >= DREAM_TREE_MAGIC_CIRCLE_DURATION {
                    visual_state.phase = PlantTreeVisualPhase::Growth;
                    visual_state.phase_elapsed = 0.0;
                }
            }
            PlantTreeVisualPhase::Growth => {
                let ratio =
                    (visual_state.phase_elapsed / DREAM_TREE_GROWTH_DURATION).clamp(0.0, 1.0);
                transform.scale = Vec3::splat(lerp(DREAM_TREE_GROWTH_SCALE_START, 1.0, ratio));

                let (sr, sg, sb, sa) = DREAM_TREE_GROWTH_GLOW_COLOR;
                sprite.color = Color::srgba(
                    lerp(sr, 1.0, ratio),
                    lerp(sg, 1.0, ratio),
                    lerp(sb, 1.0, ratio),
                    lerp(sa, 1.0, ratio),
                );

                if ratio >= 1.0 {
                    visual_state.phase = PlantTreeVisualPhase::LifeSpark;
                    visual_state.phase_elapsed = 0.0;

                    let (r, g, b, a) = DREAM_TREE_LIFE_SPARK_COLOR;
                    let root = transform.translation.truncate();
                    for _ in 0..DREAM_TREE_LIFE_SPARK_COUNT {
                        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                        let direction = Vec2::from_angle(angle);
                        let speed = rng.gen_range(
                            DREAM_TREE_LIFE_SPARK_SPEED_MIN..=DREAM_TREE_LIFE_SPARK_SPEED_MAX,
                        );
                        let offset =
                            direction * rng.gen_range(0.0..=DREAM_TREE_LIFE_SPARK_START_RADIUS);
                        commands.spawn((
                            PlantTreeLifeSpark {
                                velocity: direction * speed,
                                lifetime: DREAM_TREE_LIFE_SPARK_DURATION,
                                max_lifetime: DREAM_TREE_LIFE_SPARK_DURATION,
                            },
                            Sprite {
                                image: game_assets.plant_tree_life_spark.clone(),
                                custom_size: Some(Vec2::splat(DREAM_TREE_LIFE_SPARK_SIZE)),
                                color: Color::srgba(r, g, b, a),
                                ..default()
                            },
                            Transform::from_xyz(
                                root.x + offset.x,
                                root.y + offset.y,
                                Z_DREAM_TREE_LIFE_SPARK,
                            ),
                            Name::new("PlantTreeLifeSpark"),
                        ));
                    }
                }
            }
            PlantTreeVisualPhase::LifeSpark => {
                transform.scale = Vec3::ONE;
                sprite.color = Color::WHITE;

                if visual_state.phase_elapsed >= DREAM_TREE_LIFE_SPARK_DURATION {
                    commands
                        .entity(tree_entity)
                        .remove::<PlantTreeVisualState>();
                }
            }
        }
    }
}

pub fn update_plant_tree_life_spark_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_sparks: Query<(Entity, &mut PlantTreeLifeSpark, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    let (r, g, b, a) = DREAM_TREE_LIFE_SPARK_COLOR;

    for (entity, mut spark, mut transform, mut sprite) in q_sparks.iter_mut() {
        spark.lifetime -= dt;

        if spark.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }

        transform.translation.x += spark.velocity.x * dt;
        transform.translation.y += spark.velocity.y * dt;
        spark.velocity *= 0.92;

        let life_ratio = (spark.lifetime / spark.max_lifetime).clamp(0.0, 1.0);
        sprite.color = Color::srgba(r, g, b, a * life_ratio);
    }
}
