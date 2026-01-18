use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{Destination, Path};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use rand::Rng;

/// 使い魔のスポーンイベント
#[derive(Message)]
pub struct FamiliarSpawnEvent {
    pub position: Vec2,
    pub familiar_type: FamiliarType,
}

/// 使い魔の名前リスト（10候補）- 下級悪魔風
const FAMILIAR_NAMES: [&str; 10] = [
    "Skrix",   // 小鬼
    "Grubble", // 這いずり
    "Snitch",  // 密告者
    "Grimkin", // 陰気な小者
    "Blotch",  // シミ
    "Scraps",  // くず拾い
    "Nub",     // ちび
    "Whimper", // めそめそ
    "Cringe",  // へつらい
    "Slunk",   // こそこそ
];

/// 使い魔のコンポーネント
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Familiar {
    pub familiar_type: FamiliarType,
    pub command_radius: f32, // 指示を出せる範囲
    pub efficiency: f32,     // 人間を動かす効率 (0.0-1.0)
    pub name: String,        // 使い魔の名前
}

impl Default for Familiar {
    fn default() -> Self {
        Self {
            familiar_type: FamiliarType::default(),
            command_radius: TILE_SIZE * 7.0, // Impのデフォルト値
            efficiency: 0.5,                 // Impのデフォルト値
            name: String::new(),
        }
    }
}

impl Familiar {
    pub fn new(familiar_type: FamiliarType) -> Self {
        let (command_radius, efficiency) = match familiar_type {
            FamiliarType::Imp => (TILE_SIZE * 7.0, 0.5), // 5 -> 7
            FamiliarType::Taskmaster => (TILE_SIZE * 10.0, 0.3), // 8 -> 10
            FamiliarType::Whisperer => (TILE_SIZE * 4.0, 0.8), // 3 -> 4
        };
        let mut rng = rand::thread_rng();
        let name = FAMILIAR_NAMES[rng.gen_range(0..FAMILIAR_NAMES.len())].to_string();
        Self {
            familiar_type,
            command_radius,
            efficiency,
            name,
        }
    }
}

/// オーラ演出用コンポーネント
#[derive(Component)]
pub struct FamiliarAura {
    pub pulse_timer: f32,
}

/// オーラのレイヤー種別
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuraLayer {
    Border,  // 固定範囲（実際の影響範囲）
    Pulse,   // パルスアニメーション
    Outline, // 選択時の強調用アウトライン
}

/// 使い魔の種類（パラメーター調整用に拡張可能）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
#[allow(dead_code)]
pub enum FamiliarType {
    #[default]
    Imp, // インプ - 汎用型、バランス
    Taskmaster, // 監督官 - 広範囲、低効率
    Whisperer,  // 囁き手 - 狭範囲、高効率
}

/// 使い魔への指示
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
#[allow(dead_code)]
pub enum FamiliarCommand {
    #[default]
    Idle, // 待機
    GatherResources,   // 収集指示
    Patrol,            // 巡回（監視）
    Construct(Entity), // 建設命令
}

/// 現在のアクティブな指示
#[derive(Component, Default)]
pub struct ActiveCommand {
    pub command: FamiliarCommand,
}

/// 使い魔のアニメーション状態
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct FamiliarAnimation {
    pub timer: f32,
    pub frame: usize,
    pub is_moving: bool,
    pub facing_right: bool,
}

/// 使い魔の運用設定（閾値など）
#[derive(Component, Debug, Clone)]
pub struct FamiliarOperation {
    pub fatigue_threshold: f32,     // この疲労度以下のソウルのみ受け入れる
    pub max_controlled_soul: usize, // 最大使役数
}

impl Default for FamiliarOperation {
    fn default() -> Self {
        Self {
            fatigue_threshold: FATIGUE_THRESHOLD,
            max_controlled_soul: 2, // デフォルトを2人に変更
        }
    }
}

// UnderCommand は relationships.rs の CommandedBy に移行
// 後方互換性のため、エイリアスを提供
pub use crate::relationships::CommandedBy as UnderCommand;

use crate::systems::visual::speech::phrases::LatinPhrase;

/// 使い魔の「口癖」傾向
#[derive(Component)]
pub struct FamiliarVoice {
    /// 各コマンドの「お気に入りフレーズ」インデックス (0-4)
    pub preferences: [usize; LatinPhrase::COUNT],
    /// お気に入りを使う確率 (0.6〜0.9)
    pub preference_weight: f32,
}

impl FamiliarVoice {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let mut preferences = [0; LatinPhrase::COUNT];
        for p in preferences.iter_mut() {
            *p = rng.gen_range(0..5); // 各フレーズは5個ずつ
        }
        Self {
            preferences,
            preference_weight: rng.gen_range(0.6..0.9),
        }
    }

    /// 指定フレーズのお気に入りインデックスを取得
    pub fn get_preference(&self, phrase: LatinPhrase) -> usize {
        self.preferences[phrase.index()]
    }
}

/// 使い魔をスポーンする
pub fn spawn_familiar(mut spawn_events: MessageWriter<FamiliarSpawnEvent>) {
    spawn_events.write(FamiliarSpawnEvent {
        position: Vec2::new(-20.0, 0.0),
        familiar_type: FamiliarType::Imp,
    });
    spawn_events.write(FamiliarSpawnEvent {
        position: Vec2::new(20.0, 0.0),
        familiar_type: FamiliarType::Imp,
    });
}

/// 使い魔のスポーンを処理するシステム
pub fn familiar_spawning_system(
    mut commands: Commands,
    mut spawn_events: MessageReader<FamiliarSpawnEvent>,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
) {
    for event in spawn_events.read() {
        spawn_familiar_at(
            &mut commands,
            &game_assets,
            &world_map,
            event.position,
            event.familiar_type,
        );
    }
}

/// 指定座標に使い魔をスポーンする
pub fn spawn_familiar_at(
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    world_map: &Res<WorldMap>,
    pos: Vec2,
    familiar_type: FamiliarType,
) {
    let spawn_grid = WorldMap::world_to_grid(pos);

    // 歩ける場所を探す
    let mut actual_grid = spawn_grid;
    'search: for dx in -3..=3 {
        for dy in -3..=3 {
            let test = (spawn_grid.0 + dx, spawn_grid.1 + dy);
            if world_map.is_walkable(test.0, test.1) {
                actual_grid = test;
                break 'search;
            }
        }
    }
    let actual_pos = WorldMap::grid_to_world(actual_grid.0, actual_grid.1);

    let familiar = Familiar::new(familiar_type);
    let familiar_name = familiar.name.clone();
    let command_radius = familiar.command_radius;

    let fam_entity = commands
        .spawn((
            familiar,
            Name::new(familiar_name.clone()),
            FamiliarOperation::default(),
            ActiveCommand::default(),
            crate::systems::familiar_ai::FamiliarAiState::default(),
            crate::relationships::Commanding::default(), // 部下リスト（Relationship自動管理）
            crate::relationships::ManagedTasks::default(), // 管理タスクリスト（Relationship自動管理）
            Destination(actual_pos),                       // 移動先
            Path::default(),                               // 経路
            FamiliarAnimation::default(),                  // アニメーション
            FamiliarVoice::random(),                       // ランダムな口癖傾向
            Sprite {
                image: game_assets.familiar.clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: game_assets.familiar_layout.clone(),
                    index: 0,
                }),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.9)),
                color: Color::WHITE,
                ..default()
            },
            Transform::from_xyz(actual_pos.x, actual_pos.y, Z_CHARACTER + 0.5), // 人間より少し上に表示
        ))
        .id();

    // オーラ外枠（固定範囲 - 実際の影響範囲を示す）
    commands.spawn((
        FamiliarRangeIndicator(fam_entity),
        AuraLayer::Border,
        Sprite {
            image: game_assets.aura_circle.clone(),
            color: Color::srgba(1.0, 0.3, 0.0, 0.3), // オレンジ色の枠
            custom_size: Some(Vec2::splat(command_radius * 2.0)),
            ..default()
        },
        Transform::from_translation(actual_pos.extend(Z_AURA)),
    ));

    // オーラ強調用アウトライン（選択時のみ表示される細い線）
    commands.spawn((
        FamiliarRangeIndicator(fam_entity),
        AuraLayer::Outline,
        Sprite {
            image: game_assets.aura_ring.clone(),
            color: Color::srgba(1.0, 1.0, 0.0, 0.0), // 初期状態は透明
            custom_size: Some(Vec2::splat(command_radius * 2.0)),
            ..default()
        },
        Transform::from_translation(actual_pos.extend(Z_AURA + 0.01)),
    ));

    // オーラ内側（パルスアニメーション）
    commands.spawn((
        FamiliarAura { pulse_timer: 0.0 },
        FamiliarRangeIndicator(fam_entity),
        AuraLayer::Pulse,
        Sprite {
            image: game_assets.aura_circle.clone(),
            color: Color::srgba(1.0, 0.6, 0.0, 0.15), // 明るいオレンジ
            custom_size: Some(Vec2::splat(command_radius * 1.8)),
            ..default()
        },
        Transform::from_translation(actual_pos.extend(Z_AURA + 0.03)),
    ));

    info!(
        "SPAWN: Familiar '{}' ({:?}) at {:?}",
        familiar_name, familiar_type, actual_pos
    );
}

/// 使い魔の範囲表示用コンポーネント
#[derive(Component)]
pub struct FamiliarRangeIndicator(pub Entity); // 親の使い魔Entity

/// オーラのパルスアニメーションと位置追従システム
pub fn update_familiar_range_indicator(
    time: Res<Time>,
    q_familiars: Query<(Entity, &Transform, &Familiar)>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    mut q_indicators: Query<
        (
            &FamiliarRangeIndicator,
            &mut Transform,
            &mut Sprite,
            Option<&mut FamiliarAura>,
            Option<&AuraLayer>,
        ),
        Without<Familiar>,
    >,
) {
    let selected_fam = selected.0;

    for (indicator, mut transform, mut sprite, aura_opt, layer_opt) in q_indicators.iter_mut() {
        // 親の使い魔の位置を取得
        if let Ok((_, fam_transform, familiar)) = q_familiars.get(indicator.0) {
            // 位置追従
            let z = match layer_opt {
                Some(AuraLayer::Border) => Z_AURA,
                Some(AuraLayer::Outline) => Z_AURA + 0.01,
                Some(AuraLayer::Pulse) => Z_AURA + 0.03,
                None => Z_AURA,
            };
            transform.translation = fam_transform.translation.truncate().extend(z);

            // 選択状態を確認
            let is_selected = selected_fam == Some(indicator.0);

            // レイヤーに応じた処理
            match layer_opt {
                Some(AuraLayer::Border) => {
                    // 固定サイズ（実際の影響範囲）
                    sprite.custom_size = Some(Vec2::splat(familiar.command_radius * 2.0));
                    let alpha = if is_selected { 0.2 } else { 0.1 };
                    sprite.color = Color::srgba(1.0, 0.3, 0.0, alpha);
                }
                Some(AuraLayer::Outline) => {
                    // 選択時のみ強調
                    sprite.custom_size = Some(Vec2::splat(familiar.command_radius * 2.0));
                    let alpha = if is_selected { 0.8 } else { 0.0 };
                    sprite.color = Color::srgba(1.0, 1.0, 0.0, alpha); // 黄色の強調線
                }
                Some(AuraLayer::Pulse) => {
                    // パルスアニメーション
                    if let Some(mut aura) = aura_opt {
                        aura.pulse_timer += time.delta_secs() * 1.5;
                        let pulse = (aura.pulse_timer.sin() * 0.15 + 0.9).clamp(0.7, 1.0);
                        sprite.custom_size =
                            Some(Vec2::splat(familiar.command_radius * 2.0 * pulse));
                    }
                    let alpha = if is_selected { 0.15 } else { 0.05 };
                    sprite.color = Color::srgba(1.0, 0.6, 0.0, alpha);
                }
                None => {}
            }
        }
    }
}

/// 使い魔の移動システム
pub fn familiar_movement(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Path, &mut FamiliarAnimation), With<Familiar>>,
) {
    for (mut transform, mut path, mut anim) in query.iter_mut() {
        if path.current_index < path.waypoints.len() {
            let target = path.waypoints[path.current_index];
            let current_pos = transform.translation.truncate();
            let to_target = target - current_pos;
            let distance = to_target.length();

            if distance > 1.0 {
                let speed = 100.0; // 使い魔は速く動く
                let move_dist = (speed * time.delta_secs()).min(distance);
                let direction = to_target.normalize();
                let velocity = direction * move_dist;
                transform.translation += velocity.extend(0.0);

                anim.is_moving = true;
                if move_dist > 0.0 {
                    debug!(
                        "FAM_MOV: Moving towards waypoint. dist: {:.1}, move: {:.1}",
                        distance, move_dist
                    );
                }
                if direction.x.abs() > 0.1 {
                    anim.facing_right = direction.x > 0.0;
                }
            } else {
                info!("FAM_MOV: Reached waypoint index {}", path.current_index);
                path.current_index += 1;
            }
        } else {
            anim.is_moving = false;
        }
    }
}

/// 使い魔のアニメーション更新システム
pub fn familiar_animation_system(
    time: Res<Time>,
    mut query: Query<(&mut Sprite, &mut FamiliarAnimation), With<Familiar>>,
) {
    for (mut sprite, mut anim) in query.iter_mut() {
        // 向きの更新
        // アセットがデフォルトで左向きのため、右を向くときに flip_x を true にする
        sprite.flip_x = anim.facing_right;

        // アニメーションフレームの更新
        if anim.is_moving {
            anim.timer += time.delta_secs() * 5.0; // 5 FPS 程度 (以前は 10.0)
            anim.frame = (anim.timer as usize) % 3; // 3フレームのループ
        } else {
            anim.timer = 0.0;
            anim.frame = 0; // 停止時は最初のフレーム
        }

        if let Some(atlas) = &mut sprite.texture_atlas {
            atlas.index = anim.frame;
        }
    }
}
