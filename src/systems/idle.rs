use bevy::prelude::*;
use rand::Rng;
use crate::constants::{MOTIVATION_THRESHOLD, FATIGUE_THRESHOLD};
use crate::entities::damned_soul::{DamnedSoul, IdleState, IdleBehavior, Destination, Path};
use crate::world::map::WorldMap;
use crate::systems::work::AssignedTask;

/// 怠惰行動のAIシステム
/// やる気が低い人間は怠惰な行動をする
/// タスクがある人間は怠惰行動をしない
pub fn idle_behavior_system(
    time: Res<Time>,
    world_map: Res<WorldMap>,
    mut query: Query<(&Transform, &mut IdleState, &mut Destination, &DamnedSoul, &Path, &AssignedTask)>,
) {
    let dt = time.delta_secs();

    for (transform, mut idle, mut dest, soul, path, task) in query.iter_mut() {
        // タスクがある場合は怠惰行動をしない
        if !matches!(task, AssignedTask::None) {
            continue;
        }

        // やる気があり疲労が閾値未満の場合は怠惰行動をしない（次のタスクを待つ）
        // これにより、ワーカーはモチベーションが続く限り疲労が閾値に達するまで継続的にタスクを行う
        if soul.motivation > MOTIVATION_THRESHOLD && soul.fatigue < FATIGUE_THRESHOLD {
            continue;
        }

        idle.idle_timer += dt;

        // 行動の持続時間が過ぎたら新しい行動を選択
        if idle.idle_timer >= idle.behavior_duration {
            idle.idle_timer = 0.0;
            
            // 怠惰レベルに応じて行動を選択
            let mut rng = rand::thread_rng();
            let roll: f32 = rng.gen_range(0.0..1.0);
            
            idle.behavior = if soul.laziness > 0.8 {
                // 非常に怠惰：ほとんど寝ている
                if roll < 0.6 {
                    IdleBehavior::Sleeping
                } else if roll < 0.9 {
                    IdleBehavior::Sitting
                } else {
                    IdleBehavior::Wandering
                }
            } else if soul.laziness > 0.5 {
                // 中程度の怠惰
                if roll < 0.3 {
                    IdleBehavior::Sleeping
                } else if roll < 0.6 {
                    IdleBehavior::Sitting
                } else {
                    IdleBehavior::Wandering
                }
            } else {
                // 比較的やる気がある
                if roll < 0.7 {
                    IdleBehavior::Wandering
                } else {
                    IdleBehavior::Sitting
                }
            };

            // 新しい行動の持続時間
            idle.behavior_duration = match idle.behavior {
                IdleBehavior::Sleeping => rng.gen_range(5.0..10.0),
                IdleBehavior::Sitting => rng.gen_range(3.0..6.0),
                IdleBehavior::Wandering => rng.gen_range(2.0..4.0),
            };
        }

        // 行動を実行
        match idle.behavior {
            IdleBehavior::Wandering => {
                // 経路が空なら新しい目的地を設定
                if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
                    let current_pos = transform.translation.truncate();
                    let current_grid = WorldMap::world_to_grid(current_pos);
                    
                    // 近くのランダムな場所を目的地に
                    let mut rng = rand::thread_rng();
                    for _ in 0..10 {
                        let dx: i32 = rng.gen_range(-5..=5);
                        let dy: i32 = rng.gen_range(-5..=5);
                        let new_grid = (current_grid.0 + dx, current_grid.1 + dy);
                        
                        if world_map.is_walkable(new_grid.0, new_grid.1) {
                            let new_pos = WorldMap::grid_to_world(new_grid.0, new_grid.1);
                            dest.0 = new_pos;
                            break;
                        }
                    }
                }
            }
            IdleBehavior::Sitting | IdleBehavior::Sleeping => {
                // 動かない - 現在位置に留まる
            }
        }
    }
}

/// 怠惰行動のビジュアルフィードバック
pub fn idle_visual_system(
    mut query: Query<(&mut Transform, &mut Sprite, &IdleState, &DamnedSoul)>,
) {
    for (mut transform, mut sprite, idle, soul) in query.iter_mut() {
        match idle.behavior {
            IdleBehavior::Sleeping => {
                // 寝ている：横倒しになる
                transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                sprite.color = Color::srgba(0.6, 0.6, 0.7, 1.0);  // 少し暗く
            }
            IdleBehavior::Sitting => {
                // 座っている：少し縮む
                transform.rotation = Quat::IDENTITY;
                transform.scale.y = 0.8;
                sprite.color = Color::srgba(0.8, 0.8, 0.8, 1.0);
            }
            IdleBehavior::Wandering => {
                // 歩いている：通常
                transform.rotation = Quat::IDENTITY;
                sprite.color = Color::WHITE;
            }
        }
        
        // やる気が高い場合は明るく表示
        if soul.motivation > 0.5 {
            sprite.color = Color::srgb(1.0, 1.0, 0.8);  // 少し黄色がかる
            transform.rotation = Quat::IDENTITY;
            transform.scale = Vec3::ONE;
        }
    }
}
