use crate::constants::{
    FATIGUE_GATHERING_THRESHOLD, FATIGUE_THRESHOLD, MOTIVATION_THRESHOLD, TILE_SIZE,
};
use crate::entities::damned_soul::{
    DamnedSoul, Destination, GatheringBehavior, IdleBehavior, IdleState, Path,
};
use crate::systems::work::AssignedTask;
use crate::world::map::{GatheringArea, WorldMap};
use bevy::prelude::*;
use rand::Rng;

// ===== 集会関連の定数 =====
/// 集会エリアに「到着した」とみなす半径
const GATHERING_ARRIVAL_RADIUS: f32 = TILE_SIZE * 3.0;
/// 集会中の行動パターン変更間隔（秒）
const GATHERING_BEHAVIOR_DURATION_MIN: f32 = 60.0;
const GATHERING_BEHAVIOR_DURATION_MAX: f32 = 90.0;
/// 重なり回避の最小間隔
const GATHERING_MIN_SEPARATION: f32 = TILE_SIZE * 1.2;

// ===== ヘルパー関数 =====
/// ランダムな集会中のサブ行動を選択
fn random_gathering_behavior() -> GatheringBehavior {
    let mut rng = rand::thread_rng();
    match rng.gen_range(0..4) {
        0 => GatheringBehavior::Wandering,
        1 => GatheringBehavior::Sleeping,
        2 => GatheringBehavior::Standing,
        _ => GatheringBehavior::Dancing,
    }
}

/// ランダムな集会行動の持続時間を取得
fn random_gathering_duration() -> f32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(GATHERING_BEHAVIOR_DURATION_MIN..GATHERING_BEHAVIOR_DURATION_MAX)
}

/// 集会エリア周辺のランダムな位置を取得
fn random_position_around(center: Vec2, min_dist: f32, max_dist: f32) -> Vec2 {
    let mut rng = rand::thread_rng();
    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
    let dist: f32 = rng.gen_range(min_dist..max_dist);
    center + Vec2::new(angle.cos() * dist, angle.sin() * dist)
}

/// 怠惰行動のAIシステム
/// やる気が低い人間は怠惰な行動をする
/// タスクがある人間は怠惰行動をしない
pub fn idle_behavior_system(
    time: Res<Time>,
    world_map: Res<WorldMap>,
    gathering_area: Res<GatheringArea>,
    mut query: Query<(
        &Transform,
        &mut IdleState,
        &mut Destination,
        &DamnedSoul,
        &Path,
        &AssignedTask,
    )>,
) {
    let dt = time.delta_secs();

    for (transform, mut idle, mut dest, soul, path, task) in query.iter_mut() {
        // タスクがある場合は放置時間をリセットし、怠惰行動も行わない
        if !matches!(task, AssignedTask::None) {
            idle.total_idle_time = 0.0;
            continue;
        }

        idle.total_idle_time += dt;

        // やる気があり疲労が閾値未満の場合は怠惰行動をしない（次のタスクを待つ）
        // これにより、ワーカーはモチベーションが続く限り疲労が閾値に達するまで継続的にタスクを行う
        if soul.motivation > MOTIVATION_THRESHOLD && soul.fatigue < FATIGUE_THRESHOLD {
            continue;
        }

        idle.idle_timer += dt;

        // 行動の持続時間が過ぎたら新しい行動を選択
        if idle.idle_timer >= idle.behavior_duration {
            idle.idle_timer = 0.0;

            // 疲労度が高いか、放置時間が長い場合は集会エリアへ移動
            if soul.fatigue > FATIGUE_GATHERING_THRESHOLD || idle.total_idle_time > 30.0 {
                // 集会状態に入る時に最初の行動をランダムに設定
                if idle.behavior != IdleBehavior::Gathering
                    && idle.behavior != IdleBehavior::ExhaustedGathering
                {
                    idle.gathering_behavior = random_gathering_behavior();
                    idle.gathering_behavior_timer = 0.0;
                    idle.gathering_behavior_duration = random_gathering_duration();
                    idle.needs_separation = true;
                }

                // 疲労度による集会移動の場合は ExhaustedGathering を使用
                if soul.fatigue > FATIGUE_GATHERING_THRESHOLD {
                    idle.behavior = IdleBehavior::ExhaustedGathering;
                } else {
                    idle.behavior = IdleBehavior::Gathering;
                }
            } else {
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
            }

            // 新しい行動の持続時間
            let mut rng = rand::thread_rng();
            idle.behavior_duration = match idle.behavior {
                IdleBehavior::Sleeping => rng.gen_range(5.0..10.0),
                IdleBehavior::Sitting => rng.gen_range(3.0..6.0),
                IdleBehavior::Wandering => rng.gen_range(2.0..4.0),
                IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
                    rng.gen_range(2.0..4.0)
                } // 頻繁にうろうろ
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
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
                let current_pos = transform.translation.truncate();
                let center = gathering_area.0;
                let dist_from_center = (center - current_pos).length();

                // 集会中のサブ行動タイマーを更新
                idle.gathering_behavior_timer += dt;
                if idle.gathering_behavior_timer >= idle.gathering_behavior_duration {
                    idle.gathering_behavior_timer = 0.0;
                    idle.gathering_behavior = random_gathering_behavior();
                    idle.gathering_behavior_duration = random_gathering_duration();
                    idle.needs_separation = true;
                }

                if dist_from_center > GATHERING_ARRIVAL_RADIUS {
                    // まだ集会エリアに到着していない：集会エリアへ向かう
                    if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
                        dest.0 = center;
                    }
                } else {
                    // 集会エリアに到着済み

                    // ExhaustedGathering から通常の Gathering へ遷移
                    if idle.behavior == IdleBehavior::ExhaustedGathering {
                        idle.behavior = IdleBehavior::Gathering;
                    }

                    // サブ行動に応じて動作
                    match idle.gathering_behavior {
                        GatheringBehavior::Wandering => {
                            // うろうろ：2〜3秒おきにゆっくりと漂う
                            let path_complete = path.waypoints.is_empty()
                                || path.current_index >= path.waypoints.len();
                            if path_complete && idle.idle_timer >= idle.behavior_duration * 0.8 {
                                let new_target = random_position_around(
                                    center,
                                    TILE_SIZE * 0.5,
                                    TILE_SIZE * 1.5,
                                );
                                let target_grid = WorldMap::world_to_grid(new_target);
                                if world_map.is_walkable(target_grid.0, target_grid.1) {
                                    dest.0 = new_target;
                                } else {
                                    dest.0 = center;
                                }
                                idle.idle_timer = 0.0;
                                let mut rng = rand::thread_rng();
                                idle.behavior_duration = rng.gen_range(2.0..3.0);
                            }
                        }
                        GatheringBehavior::Sleeping
                        | GatheringBehavior::Standing
                        | GatheringBehavior::Dancing => {
                            // これらの行動では移動しない（ビジュアルのみ）
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
    gathering_area: Res<GatheringArea>,
    mut query: Query<(
        &mut Transform,
        &mut Sprite,
        &IdleState,
        &DamnedSoul,
        &AssignedTask,
    )>,
) {
    for (mut transform, mut sprite, idle, soul, task) in query.iter_mut() {
        // タスクがある場合はビジュアルをリセット
        if !matches!(task, AssignedTask::None) {
            transform.rotation = Quat::IDENTITY;
            transform.scale = Vec3::ONE;
            sprite.color = Color::WHITE;
            continue;
        }

        match idle.behavior {
            IdleBehavior::Sleeping => {
                // 寝ている：横倒しになる
                transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                sprite.color = Color::srgba(0.6, 0.6, 0.7, 1.0);
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
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => {
                // 位置ベースで到着判定
                let current_pos = transform.translation.truncate();
                let center = gathering_area.0;
                let dist_from_center = (center - current_pos).length();
                let has_arrived = dist_from_center <= GATHERING_ARRIVAL_RADIUS;

                if !has_arrived {
                    // 集会エリアに向かっている途中
                    transform.rotation = Quat::IDENTITY;
                    transform.scale = Vec3::ONE;

                    if idle.behavior == IdleBehavior::ExhaustedGathering {
                        // 疲労による集会移動中は、より疲れた色合いに
                        sprite.color = Color::srgba(0.7, 0.6, 0.8, 0.9);
                    } else {
                        sprite.color = Color::srgba(0.85, 0.75, 1.0, 0.85); // 淡いラベンダー色（移動中）
                    }
                } else {
                    // 集会エリアに到着済み：サブ行動に応じたビジュアル
                    sprite.color = Color::srgba(0.8, 0.7, 1.0, 0.7);

                    match idle.gathering_behavior {
                        GatheringBehavior::Wandering => {
                            // うろうろ：軽い呼吸アニメーション
                            transform.rotation = Quat::IDENTITY;
                            let pulse_speed = 0.5;
                            let scale_offset =
                                (idle.total_idle_time * pulse_speed).sin() * 0.03 + 1.0;
                            transform.scale = Vec3::splat(scale_offset);
                        }
                        GatheringBehavior::Sleeping => {
                            // 寝ている：横倒しになる
                            transform.rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
                            sprite.color = Color::srgba(0.6, 0.5, 0.8, 0.6); // より暗く
                            let breath = (idle.total_idle_time * 0.3).sin() * 0.02 + 0.95;
                            transform.scale = Vec3::splat(breath);
                        }
                        GatheringBehavior::Standing => {
                            // 立ち尽くす：静止（微かな呼吸のみ）
                            transform.rotation = Quat::IDENTITY;
                            let breath = (idle.total_idle_time * 0.2).sin() * 0.01 + 1.0;
                            transform.scale = Vec3::splat(breath);
                        }
                        GatheringBehavior::Dancing => {
                            // 踊り（揺れ）：左右に揺れる
                            let sway_angle = (idle.total_idle_time * 3.0).sin() * 0.15;
                            transform.rotation = Quat::from_rotation_z(sway_angle);
                            let bounce = (idle.total_idle_time * 4.0).sin() * 0.05 + 1.0;
                            transform.scale = Vec3::new(1.0, bounce, 1.0);
                            sprite.color = Color::srgba(1.0, 0.8, 1.0, 0.8); // 少し明るく
                        }
                    }
                }
            }
        }

        // やる気が高い場合は明るく表示
        if soul.motivation > 0.5 {
            sprite.color = Color::srgb(1.0, 1.0, 0.8); // 少し黄色がかる
            transform.rotation = Quat::IDENTITY;
            transform.scale = Vec3::ONE;
        }
    }
}

/// 集会エリアでの魂の重なり回避システム
/// うろつき以外の行動（睡眠、立ち尽くす、踊り）で重なりを解消
/// パフォーマンス最適化：初回到着時とパターン変更時のみ実行
pub fn gathering_separation_system(
    gathering_area: Res<GatheringArea>,
    world_map: Res<WorldMap>,
    mut query: Query<(
        Entity,
        &Transform,
        &mut Destination,
        &mut IdleState,
        &Path,
        &AssignedTask,
    )>,
) {
    // まず全ての集会中の魂の位置を収集
    let gathering_positions: Vec<(Entity, Vec2)> = query
        .iter()
        .filter(|(_, _, _, idle, _, task)| {
            matches!(task, AssignedTask::None)
                && (idle.behavior == IdleBehavior::Gathering
                    || idle.behavior == IdleBehavior::ExhaustedGathering)
                && idle.gathering_behavior != GatheringBehavior::Wandering
        })
        .map(|(entity, transform, _, _, _, _)| (entity, transform.translation.truncate()))
        .collect();

    // needs_separation が true の魂のみ処理
    for (entity, transform, mut dest, mut idle, path, task) in query.iter_mut() {
        // needs_separation フラグがない場合はスキップ
        if !idle.needs_separation {
            continue;
        }

        // 集会中かつ静止系の行動でない場合はスキップ
        if !matches!(task, AssignedTask::None) {
            idle.needs_separation = false;
            continue;
        }
        if idle.behavior != IdleBehavior::Gathering
            && idle.behavior != IdleBehavior::ExhaustedGathering
        {
            idle.needs_separation = false;
            continue;
        }
        if idle.gathering_behavior == GatheringBehavior::Wandering {
            idle.needs_separation = false;
            continue;
        }

        let current_pos = transform.translation.truncate();
        let center = gathering_area.0;
        let dist_from_center = (center - current_pos).length();

        // 集会エリアに到着していない場合はスキップ（フラグは維持）
        if dist_from_center > GATHERING_ARRIVAL_RADIUS {
            continue;
        }

        // 経路がある場合（移動中）はスキップ（フラグは維持）
        if !path.waypoints.is_empty() && path.current_index < path.waypoints.len() {
            continue;
        }

        // 他の魂との重なりをチェック
        let mut is_overlapping = false;
        for (other_entity, other_pos) in &gathering_positions {
            if *other_entity == entity {
                continue;
            }
            let dist = (current_pos - *other_pos).length();
            if dist < GATHERING_MIN_SEPARATION {
                is_overlapping = true;
                break;
            }
        }

        // 重なっている場合は新しい位置を探す
        if is_overlapping {
            let mut rng = rand::thread_rng();
            for _ in 0..10 {
                let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
                let dist: f32 = rng.gen_range(TILE_SIZE..TILE_SIZE * 2.5);
                let offset = Vec2::new(angle.cos() * dist, angle.sin() * dist);
                let new_pos = center + offset;

                // 他の魂と重ならないかチェック
                let mut valid = true;
                for (other_entity, other_pos) in &gathering_positions {
                    if *other_entity == entity {
                        continue;
                    }
                    if (new_pos - *other_pos).length() < GATHERING_MIN_SEPARATION {
                        valid = false;
                        break;
                    }
                }

                if valid {
                    let target_grid = WorldMap::world_to_grid(new_pos);
                    if world_map.is_walkable(target_grid.0, target_grid.1) {
                        dest.0 = new_pos;
                        break;
                    }
                }
            }
        }

        // 処理完了、フラグをクリア
        idle.needs_separation = false;
    }
}
