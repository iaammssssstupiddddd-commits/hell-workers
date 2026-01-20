//! 動的集会システム (Dynamic Gathering System)
//!
//! Soulの待機行動に基づいて自然発生的に集会所が生成され、
//! 人が集まるにつれて拡大し、距離に応じて統合され、過疎化すると消滅する。

use bevy::prelude::*;
use rand::Rng;

use crate::constants::*;

// ============================================================
// 定数
// ============================================================

/// 集会発生までの基本待機時間 (秒)
pub const GATHERING_SPAWN_BASE_TIME: f32 = 10.0;
/// 近くにいるSoul1人あたりの発生時間短縮 (秒)
pub const GATHERING_SPAWN_TIME_REDUCTION_PER_SOUL: f32 = 1.5;
/// 近傍Soul検出半径
pub const GATHERING_DETECTION_RADIUS: f32 = TILE_SIZE * 5.0;
/// 集会の最大参加人数
pub const GATHERING_MAX_CAPACITY: usize = 8;
/// 集会維持に必要な最低人数
pub const GATHERING_MIN_PARTICIPANTS: usize = 2;
/// 集会消滅までの猶予時間 (秒)
pub const GATHERING_GRACE_PERIOD: f32 = 10.0;
/// 統合の初期距離 (タイル)
pub const GATHERING_MERGE_INITIAL_DISTANCE: f32 = TILE_SIZE * 2.0;
/// 統合の最大距離 (タイル)
pub const GATHERING_MERGE_MAX_DISTANCE: f32 = TILE_SIZE * 10.0;
/// 統合距離の基本拡大速度 (タイル/秒)
pub const GATHERING_MERGE_GROWTH_BASE_SPEED: f32 = TILE_SIZE * 0.3;
/// オーラの基本サイズ
pub const GATHERING_AURA_BASE_SIZE: f32 = TILE_SIZE * 3.0;
/// オーラの1人あたりサイズ増加
pub const GATHERING_AURA_SIZE_PER_PERSON: f32 = TILE_SIZE * 0.5;

/// 集会から離脱する半径 (検出半径より大きくしてチャタリングを防ぐ)
pub const GATHERING_LEAVE_RADIUS: f32 = TILE_SIZE * 7.5;

// ============================================================
// コンポーネント
// ============================================================

/// 集会の中心オブジェクトタイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum GatheringObjectType {
    #[default]
    Nothing, // オブジェクトなし (オーラのみ)
    CardTable, // トランプ台
    Campfire,  // 焚き火
    Barrel,    // 酒樽
}

impl GatheringObjectType {
    /// 参加人数に応じた確率テーブルでランダム選択
    pub fn random_weighted(participant_count: usize) -> Self {
        let mut rng = rand::thread_rng();
        let roll: f32 = rng.gen_range(0.0..1.0);

        // 人数による確率テーブル
        let (nothing, card_table, campfire) = match participant_count {
            0..=4 => (0.50, 0.80, 0.90), // Nothing 50%, CardTable 30%, Campfire 10%, Barrel 10%
            5..=6 => (0.20, 0.70, 0.90), // Nothing 20%, CardTable 50%, Campfire 20%, Barrel 10%
            _ => (0.05, 0.30, 0.70),     // Nothing 5%, CardTable 25%, Campfire 40%, Barrel 30%
        };

        if roll < nothing {
            GatheringObjectType::Nothing
        } else if roll < card_table {
            GatheringObjectType::CardTable
        } else if roll < campfire {
            GatheringObjectType::Campfire
        } else {
            GatheringObjectType::Barrel
        }
    }

    /// アセットパス (Nothing の場合は None)
    pub fn asset_path(&self) -> Option<&'static str> {
        match self {
            GatheringObjectType::Nothing => None,
            GatheringObjectType::CardTable => Some("textures/ui/card_table.png"),
            GatheringObjectType::Campfire => Some("textures/ui/campfire.png"),
            GatheringObjectType::Barrel => Some("textures/ui/barrel.png"),
        }
    }
}

/// 集会スポットコンポーネント
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct GatheringSpot {
    /// 中心座標
    pub center: Vec2,
    /// 現在の参加人数
    pub participants: usize,
    /// 最大参加人数
    pub max_capacity: usize,
    /// 消滅猶予タイマー (残り秒)
    pub grace_timer: f32,
    /// 猶予が発動中か
    pub grace_active: bool,
    /// 中心オブジェクトの種類
    pub object_type: GatheringObjectType,
    /// 生成時刻 (統合時の先着判定用)
    pub created_at: f32,
}

impl Default for GatheringSpot {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            participants: 0,
            max_capacity: GATHERING_MAX_CAPACITY,
            grace_timer: GATHERING_GRACE_PERIOD,
            grace_active: true, // 発生直後は猶予期間
            object_type: GatheringObjectType::Nothing,
            created_at: 0.0,
        }
    }
}

/// 集会スポットのビジュアル要素へのリンク
#[derive(Component, Debug)]
pub struct GatheringVisuals {
    /// オーラエンティティ
    pub aura_entity: Entity,
    /// 中心オブジェクトエンティティ (Nothing の場合は None)
    pub object_entity: Option<Entity>,
}

/// Soulが参加中の集会スポットへの参照
#[derive(Component, Debug)]
pub struct ParticipatingIn(pub Entity);

/// 集会発生の準備状態 (Soulに付与)
#[derive(Component, Debug, Default)]
pub struct GatheringReadiness {
    /// 集会発生までの累計待機時間
    pub idle_time: f32,
}

// ============================================================
// ヘルパー関数
// ============================================================

/// 統合距離を計算 (時間経過で拡大、人数が少ないほど速く拡大)
pub fn calculate_merge_distance(participant_count: usize, elapsed_time: f32) -> f32 {
    let count = (participant_count.max(1)) as f32;
    let speed = GATHERING_MERGE_GROWTH_BASE_SPEED * (GATHERING_MAX_CAPACITY as f32 / count);
    let distance = GATHERING_MERGE_INITIAL_DISTANCE + elapsed_time * speed;
    distance.min(GATHERING_MERGE_MAX_DISTANCE)
}

/// オーラサイズを計算
pub fn calculate_aura_size(participant_count: usize) -> f32 {
    GATHERING_AURA_BASE_SIZE + (participant_count as f32) * GATHERING_AURA_SIZE_PER_PERSON
}

// ============================================================
// システム
// ============================================================

use crate::assets::GameAssets;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::systems::soul_ai::task_execution::AssignedTask;

/// 集会スポットの発生システム
/// アイドル状態のSoulが一定時間経過すると新しい集会を発生させる
pub fn gathering_spawn_system(
    time: Res<Time>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game_assets: Res<GameAssets>,
    q_souls: Query<
        (Entity, &Transform, &IdleState, &AssignedTask),
        (With<DamnedSoul>, Without<ParticipatingIn>),
    >,
    q_existing_spots: Query<(Entity, &GatheringSpot)>,
    mut q_readiness: Query<&mut GatheringReadiness>,
) {
    let dt = time.delta_secs();
    let current_time = time.elapsed_secs();

    for (entity, transform, idle, task) in q_souls.iter() {
        // タスクなし & Idle/Wandering 状態のみ対象
        if !matches!(task, AssignedTask::None) {
            continue;
        }
        if !matches!(
            idle.behavior,
            IdleBehavior::Wandering | IdleBehavior::Sitting | IdleBehavior::Sleeping
        ) {
            continue;
        }

        let pos = transform.translation.truncate();

        // 既存の集会所が近くにあるかチェック
        let mut has_nearby_spot = false;
        for (_, spot) in q_existing_spots.iter() {
            if (spot.center - pos).length() < GATHERING_DETECTION_RADIUS {
                has_nearby_spot = true;
                break;
            }
        }
        if has_nearby_spot {
            continue;
        }

        // 近傍のSoul数をカウント
        let nearby_souls = q_souls
            .iter()
            .filter(|(e, t, _, _)| {
                *e != entity
                    && (t.translation.truncate() - pos).length() < GATHERING_DETECTION_RADIUS
            })
            .count();

        // 発生時間を計算
        let spawn_time = (GATHERING_SPAWN_BASE_TIME
            - nearby_souls as f32 * GATHERING_SPAWN_TIME_REDUCTION_PER_SOUL)
            .max(2.0);

        // GatheringReadiness を更新または追加
        if let Ok(mut readiness) = q_readiness.get_mut(entity) {
            readiness.idle_time += dt;
            if readiness.idle_time >= spawn_time {
                // 集会発生!
                let object_type = GatheringObjectType::random_weighted(nearby_souls + 1);
                let spot_entity = spawn_gathering_spot(
                    &mut commands,
                    &asset_server,
                    &game_assets,
                    pos,
                    object_type,
                    current_time,
                );
                // 発起人を参加者として登録
                commands.entity(entity).insert(ParticipatingIn(spot_entity));
                readiness.idle_time = 0.0;
                info!(
                    "GATHERING: New spot spawned at {:?} with {:?}, initiator {:?}",
                    pos, object_type, entity
                );
            }
        } else {
            commands
                .entity(entity)
                .insert(GatheringReadiness::default());
        }
    }
}

/// 集会スポットをスポーン
fn spawn_gathering_spot(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    game_assets: &Res<GameAssets>,
    center: Vec2,
    object_type: GatheringObjectType,
    created_at: f32,
) -> Entity {
    let spot = GatheringSpot {
        center,
        participants: 1, // 発起人
        max_capacity: GATHERING_MAX_CAPACITY,
        grace_timer: GATHERING_GRACE_PERIOD,
        grace_active: true,
        object_type,
        created_at,
    };

    let aura_size = calculate_aura_size(1);

    // オーラエンティティ
    let aura_entity = commands
        .spawn((
            Sprite {
                image: game_assets.aura_circle.clone(),
                custom_size: Some(Vec2::splat(aura_size)),
                color: Color::srgba(0.5, 0.2, 0.8, 0.3),
                ..default()
            },
            Transform::from_xyz(center.x, center.y, Z_AURA),
        ))
        .id();

    // 中心オブジェクトエンティティ (もしあれば)
    let object_entity = object_type.asset_path().map(|path| {
        commands
            .spawn((
                Sprite {
                    image: asset_server.load(path),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                    ..default()
                },
                Transform::from_xyz(center.x, center.y, Z_ITEM),
            ))
            .id()
    });

    let visuals = GatheringVisuals {
        aura_entity,
        object_entity,
    };

    commands.spawn((spot, visuals)).id()
}

/// 集会スポットの維持・消滅システム
pub fn gathering_maintenance_system(
    time: Res<Time>,
    mut commands: Commands,
    mut q_spots: Query<(Entity, &mut GatheringSpot, &GatheringVisuals)>,
    q_participants: Query<&ParticipatingIn>,
) {
    let dt = time.delta_secs();

    for (spot_entity, mut spot, visuals) in q_spots.iter_mut() {
        // 参加人数をカウント
        let participant_count = q_participants.iter().filter(|p| p.0 == spot_entity).count();
        spot.participants = participant_count;

        // 人数が最低未満の場合
        if participant_count < GATHERING_MIN_PARTICIPANTS {
            if !spot.grace_active {
                spot.grace_active = true;
                spot.grace_timer = GATHERING_GRACE_PERIOD;
            }
            spot.grace_timer -= dt;

            if spot.grace_timer <= 0.0 {
                // 集会消滅
                info!(
                    "GATHERING: Spot at {:?} dissolved (insufficient participants)",
                    spot.center
                );
                commands.entity(visuals.aura_entity).despawn();
                if let Some(obj) = visuals.object_entity {
                    commands.entity(obj).despawn();
                }
                commands.entity(spot_entity).despawn();
            }
        } else {
            spot.grace_active = false;
            spot.grace_timer = GATHERING_GRACE_PERIOD;
        }
    }
}

/// 集会スポットの統合システム
pub fn gathering_merge_system(
    time: Res<Time>,
    mut commands: Commands,
    q_spots: Query<(Entity, &GatheringSpot, &GatheringVisuals)>,
    mut q_participants: Query<&mut ParticipatingIn>,
) {
    let current_time = time.elapsed_secs();
    let spots: Vec<_> = q_spots.iter().collect();

    for i in 0..spots.len() {
        for j in (i + 1)..spots.len() {
            let (entity_a, spot_a, visuals_a) = &spots[i];
            let (entity_b, spot_b, visuals_b) = &spots[j];

            // 統合後の合計人数が定員を超える場合はスキップ
            let combined_participants = spot_a.participants + spot_b.participants;
            if combined_participants > GATHERING_MAX_CAPACITY {
                continue;
            }

            let distance = (spot_a.center - spot_b.center).length();
            let elapsed_a = current_time - spot_a.created_at;
            let elapsed_b = current_time - spot_b.created_at;
            let merge_distance_a = calculate_merge_distance(spot_a.participants, elapsed_a);
            let merge_distance_b = calculate_merge_distance(spot_b.participants, elapsed_b);

            // どちらかの統合距離内にあるか
            if distance < merge_distance_a.max(merge_distance_b) {
                // 小さい方を大きい方に吸収
                let (absorber, absorbed, absorbed_visuals) =
                    if spot_a.participants > spot_b.participants {
                        (*entity_a, *entity_b, visuals_b)
                    } else if spot_b.participants > spot_a.participants {
                        (*entity_b, *entity_a, visuals_a)
                    } else {
                        // 同数の場合は古い方が残る
                        if spot_a.created_at < spot_b.created_at {
                            (*entity_a, *entity_b, visuals_b)
                        } else {
                            (*entity_b, *entity_a, visuals_a)
                        }
                    };

                info!("GATHERING: Merging spot {:?} into {:?}", absorbed, absorber);

                // 参加者のターゲットを変更
                for mut participating in q_participants.iter_mut() {
                    if participating.0 == absorbed {
                        participating.0 = absorber;
                    }
                }

                // 吸収された側のビジュアルを削除
                commands.entity(absorbed_visuals.aura_entity).despawn();
                if let Some(obj) = absorbed_visuals.object_entity {
                    commands.entity(obj).despawn();
                }
                commands.entity(absorbed).despawn();

                // 今の反復を終了（状態が変わったため）
                return;
            }
        }
    }
}

/// 集会オーラのサイズと位置の更新システム
pub fn gathering_visual_update_system(
    q_spots: Query<(Entity, &GatheringSpot, &GatheringVisuals)>,
    mut q_visuals: Query<
        (&mut Sprite, &mut Transform),
        (Without<DamnedSoul>, Without<ParticipatingIn>),
    >,
) {
    for (_spot_entity, spot, visuals) in q_spots.iter() {
        // ビジュアルの更新 (サイズのみ - 位置はスポーン時のcenterを維持)
        let target_size = calculate_aura_size(spot.participants);

        // オーラの更新
        if let Ok((mut sprite, mut transform)) = q_visuals.get_mut(visuals.aura_entity) {
            sprite.custom_size = Some(Vec2::splat(target_size));
            transform.translation = spot.center.extend(Z_AURA);
        }

        // 中心オブジェクトの更新
        if let Some(obj_entity) = visuals.object_entity {
            if let Ok((_, mut transform)) = q_visuals.get_mut(obj_entity) {
                transform.translation = spot.center.extend(Z_ITEM);
            }
        }
    }
}

/// 集会エリア内の未参加Soulを自動的に参加させるシステム
pub fn gathering_recruitment_system(
    mut commands: Commands,
    q_spots: Query<(Entity, &GatheringSpot)>,
    q_souls: Query<(Entity, &Transform), (With<DamnedSoul>, Without<ParticipatingIn>)>,
) {
    for (spot_entity, spot) in q_spots.iter() {
        // 定員オーバーならスキップ
        if spot.participants >= spot.max_capacity {
            continue;
        }

        for (soul_entity, transform) in q_souls.iter() {
            let dist = (spot.center - transform.translation.truncate()).length();
            // 集会検出半径内に入れば自動参加
            if dist < GATHERING_DETECTION_RADIUS {
                commands
                    .entity(soul_entity)
                    .insert(ParticipatingIn(spot_entity));
                info!(
                    "GATHERING: Soul {:?} automatically recruited to spot {:?}",
                    soul_entity, spot_entity
                );
            }
        }
    }
}

/// 集会中でない参加者が中心から離れた時に参加を解除するシステム
pub fn gathering_leave_system(
    mut commands: Commands,
    q_spots: Query<&GatheringSpot>,
    q_participants: Query<(Entity, &Transform, &IdleState, &ParticipatingIn), With<DamnedSoul>>,
) {
    for (entity, transform, idle, participating_in) in q_participants.iter() {
        // 自発的に参加中（集会行動中）のSoulは離脱判定をしない
        if matches!(
            idle.behavior,
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
        ) {
            continue;
        }

        if let Ok(spot) = q_spots.get(participating_in.0) {
            let dist = (spot.center - transform.translation.truncate()).length();
            // 一定距離以上離れたら参加を解除
            if dist > GATHERING_LEAVE_RADIUS {
                commands.entity(entity).remove::<ParticipatingIn>();
                info!(
                    "GATHERING: Soul {:?} left spot {:?} (too far away)",
                    entity, participating_in.0
                );
            }
        } else {
            // スポット自体が消滅している場合はコンポーネントを削除
            commands.entity(entity).remove::<ParticipatingIn>();
        }
    }
}

/// 集会スポットホバー時に参加者との間に紫の線を引くデバッグシステム
pub fn gathering_debug_visualization_system(
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<crate::interface::camera::MainCamera>>,
    hovered_entity: Res<crate::interface::selection::HoveredEntity>,
    q_spots: Query<(Entity, &GatheringSpot)>,
    q_participants: Query<(&GlobalTransform, &ParticipatingIn), With<DamnedSoul>>,
    mut gizmos: Gizmos,
) {
    let Ok(window) = q_window.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };

    let cursor_world_pos = window.cursor_position().and_then(|cursor_pos| {
        camera
            .viewport_to_world_2d(camera_transform, cursor_pos)
            .ok()
    });

    // 表示対象のスポットIDを保持するセット
    let mut target_spots = std::collections::HashSet::new();

    // 1. マウス座標がスポットの中心に近いかチェック (1タイル以内)
    if let Some(world_pos) = cursor_world_pos {
        for (entity, spot) in q_spots.iter() {
            if spot.center.distance(world_pos) < TILE_SIZE {
                target_spots.insert(entity);
            }
        }
    }

    // 2. もしSoulをホバーしていたら、そのSoulが参加しているスポットを対象にする
    if let Some(hovered) = hovered_entity.0 {
        if let Ok((_, participating_in)) = q_participants.get(hovered) {
            target_spots.insert(participating_in.0);
        }
    }

    // 対象のスポットをすべて描画
    for spot_entity in target_spots {
        if let Ok((_, spot)) = q_spots.get(spot_entity) {
            let center = spot.center;

            for (soul_transform, participating_in) in q_participants.iter() {
                if participating_in.0 == spot_entity {
                    let soul_pos = soul_transform.translation().truncate();
                    // 紫の線とドット
                    gizmos.line_2d(center, soul_pos, Color::srgba(0.8, 0.4, 1.0, 0.8));
                    gizmos.circle_2d(soul_pos, 4.0, Color::srgba(0.8, 0.4, 1.0, 0.6));
                }
            }

            // 中心に目立つ円を描く
            gizmos.circle_2d(center, 16.0, Color::srgba(0.8, 0.4, 1.0, 1.0));
        }
    }
}
