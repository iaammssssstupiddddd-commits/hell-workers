# Dream バブル物理システム — 実装計画

UIパーティクルのBezier曲線をネイティブ物理シミュレーションに置き換え、
泡が「ふわふわ漂い → 吸い込まれる」挙動を力学から創発させる。

## アーキテクチャ

```
World Layer (ワールド座標)
  Soul/RestArea が DreamPopupThreshold を超える
    → camera.world_to_viewport() でスクリーン座標に変換
    → BubbleBody を生成
         ↓
Bubble Physics Layer (スクリーン座標)  ← 新設
  毎フレーム力の合成:
    浮力(上方) + 吸引(Icon方向) + 揺らぎ(ランダム) + 境界反発 + 抵抗
  → velocity 更新 → position 更新 → UI Node に反映
  → 近接泡同士の合体判定
  → 到着判定 (Icon との距離)
         ↓
UI Layer (UI空間)
  DreamPoolIcon: 吸収反応 (膨張+発光)
  DreamPoolPulse: テキスト発光 (既存)
```

## 変更対象ファイル一覧

| ファイル | Phase | 変更内容 |
|----------|-------|----------|
| `assets/textures/ui/dream_bubble.png` | 1 | 新規テクスチャ |
| `src/assets.rs` | 1 | dream_bubble ハンドル追加 |
| `src/constants/dream.rs` | 1,2,3 | 物理/合体/Trail/Icon定数 |
| `src/systems/visual/dream/components.rs` | 1,2,3 | BubbleBody, TrailGhost, IconAbsorb |
| `src/systems/visual/dream/ui_particle.rs` | 1,2,3 | 全面書き換え（物理エンジン） |
| `src/systems/visual/dream/gain_visual.rs` | 1 | spawn呼び出し変更 |
| `src/systems/visual/dream/particle.rs` | 1 | RestArea spawn呼び出し変更 |
| `src/systems/visual/dream/mod.rs` | 1,2,3 | pub use 更新 |
| `src/interface/ui/setup/time_control.rs` | 3 | DreamIconAbsorb 付与 |
| `src/plugins/visual.rs` | 1,2,3 | system登録更新 |

---

## Phase 1: 物理コア — Bezier → 力学シミュレーション置換

### 目標
UIパーティクルの移動を Bezier 曲線から力学シミュレーションに完全置換。
泡テクスチャと wobble アニメーションで不定形の泡ビジュアルを実現。

### 1-1. テクスチャ作成

- 画像生成: 不規則な輪郭の柔らかい泡、内部に微かな濃淡ムラ
  - magenta背景(#FF00FF)、art style は docs/world_lore.md §6.2 参照
- `python scripts/convert_to_png.py` で透過変換
- `assets/textures/ui/dream_bubble.png` に配置
- `src/assets.rs` の GameAssets に `dream_bubble: Handle<Image>` 追加

### 1-2. 定数追加 (`constants/dream.rs`)

```rust
// Bubble physics
pub const DREAM_UI_PARTICLE_SIZE: f32 = 10.0;
pub const DREAM_UI_PARTICLE_LIFETIME: f32 = 2.5;    // 物理ベースは長め
pub const DREAM_UI_BUOYANCY: f32 = 30.0;             // 上方向の浮力 (px/s²)
pub const DREAM_UI_BASE_ATTRACTION: f32 = 8.0;       // 基礎吸引力 (px/s²)
pub const DREAM_UI_ATTRACTION_EXPONENT: f32 = 3.0;   // 吸引力の指数増加係数
pub const DREAM_UI_DRAG: f32 = 0.96;                 // 抵抗係数 (per frame)
pub const DREAM_UI_NOISE_STRENGTH: f32 = 60.0;       // ランダム揺らぎ強度 (px/s²)
pub const DREAM_UI_NOISE_INTERVAL: f32 = 0.3;        // 揺らぎ方向更新間隔 (秒)
pub const DREAM_UI_BOUNDARY_MARGIN: f32 = 20.0;      // 画面端マージン (px)
pub const DREAM_UI_BOUNDARY_PUSH: f32 = 100.0;       // 境界反発力 (px/s²)
pub const DREAM_UI_ARRIVAL_RADIUS: f32 = 12.0;       // 到着判定半径 (px)
pub const DREAM_UI_BUBBLE_DRIFT_STRENGTH: f32 = 3.0; // 位置揺らぎ振幅 (px)
```

### 1-3. コンポーネント (`components.rs`)

既存の `DreamGainUiParticle` を `BubbleBody` に置換:

```rust
#[derive(Component)]
pub struct BubbleBody {
    pub velocity: Vec2,
    pub target_pos: Vec2,           // DreamPoolIcon のスクリーン座標
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub phase: f32,                 // wobble + noise の位相
    pub noise_direction: Vec2,      // 現在のランダム揺らぎ方向
    pub noise_timer: f32,           // 次の揺らぎ方向更新までの時間
    pub merge_count: u8,            // 合体回数 (Phase 2)
    pub merging_into: Option<Entity>, // 吸収中の相手 (Phase 2)
    pub merge_timer: f32,           // 吸収アニメーション残時間 (Phase 2)
    pub trail_cooldown: f32,        // Trail 用 (Phase 2)
}
```

Phase 1 では `merge_count`, `merging_into`, `merge_timer`, `trail_cooldown` は
初期値のまま未使用（Phase 2 で活性化）。

### 1-4. 物理システム (`ui_particle.rs` 全面改修)

**旧:** `ui_particle_update_system` (Bezier補間)
**新:** `bubble_physics_system` (力学シミュレーション)

```rust
pub fn bubble_physics_system(
    mut commands: Commands,
    time: Res<Time>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_icon_absorb: Query<&mut DreamIconAbsorb>,
    mut q_bubbles: Query<(
        Entity, &mut BubbleBody, &mut Node, &mut BackgroundColor
    )>,
    q_camera: Query<&Camera, With<MainCamera>>,
) {
    let dt = time.delta_secs();
    let viewport_size = ...; // カメラから取得

    for (entity, mut body, mut node, mut color) in q_bubbles.iter_mut() {
        body.lifetime -= dt;
        if body.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }

        // --- 力の合成 ---
        let life_ratio = 1.0 - (body.lifetime / body.max_lifetime); // 0→1

        let current_pos = Vec2::new(
            node.left.resolve(0.0, viewport_size.x).unwrap_or(0.0),
            node.top.resolve(0.0, viewport_size.y).unwrap_or(0.0),
        );

        // 1. 浮力（上方向 = Y負）
        let buoyancy = Vec2::new(0.0, -DREAM_UI_BUOYANCY);

        // 2. 吸引力（指数的増加）
        let to_target = body.target_pos - current_pos;
        let distance = to_target.length().max(1.0);
        let attraction_strength =
            DREAM_UI_BASE_ATTRACTION * (life_ratio * DREAM_UI_ATTRACTION_EXPONENT).exp();
        let attraction = to_target.normalize_or_zero() * attraction_strength;

        // 3. ランダム揺らぎ（一定間隔で方向更新）
        body.noise_timer -= dt;
        if body.noise_timer <= 0.0 {
            body.noise_direction = random_unit_vec2(&mut rng);
            body.noise_timer = DREAM_UI_NOISE_INTERVAL;
        }
        let noise = body.noise_direction * DREAM_UI_NOISE_STRENGTH
            * (1.0 - life_ratio).max(0.0); // 後半は揺らぎ減衰

        // 4. 境界反発
        let boundary = boundary_force(current_pos, viewport_size);

        // 力の合成
        let total_force = buoyancy + attraction + noise + boundary;
        body.velocity += total_force * dt;
        body.velocity *= DREAM_UI_DRAG;

        // 位置更新
        let new_pos = current_pos + body.velocity * dt;
        node.left = Val::Px(new_pos.x);
        node.top = Val::Px(new_pos.y);

        // --- Wobble (形状揺れ) ---
        body.phase += dt * 6.0;
        let wobble_strength = (1.0 - life_ratio * 1.3).clamp(0.0, 1.0);
        let base_size = DREAM_UI_PARTICLE_SIZE
            + body.merge_count as f32 * DREAM_UI_MERGE_SIZE_BONUS;
        let shrink = 1.0 - life_ratio * 0.7;
        let current_size = base_size * shrink;
        let wx = (body.phase * 3.5).sin() * 1.5 * wobble_strength;
        let wy = (body.phase * 4.8 + 1.7).cos() * 1.5 * wobble_strength;
        node.width = Val::Px((current_size + wx).max(2.0));
        node.height = Val::Px((current_size + wy).max(2.0));

        // --- 色変化 ---
        let brightness = (life_ratio - 0.6).max(0.0) / 0.4;
        let r = 0.65 + brightness * 0.25;
        let g = 0.9 + brightness * 0.07;
        let b = 1.0;
        let alpha = (life_ratio / 0.05).clamp(0.0, 1.0) * 0.9; // フェードイン
        color.0 = Color::srgba(r, g, b, alpha);

        // --- 到着判定 ---
        if distance < DREAM_UI_ARRIVAL_RADIUS {
            // Icon吸収通知
            if let Some(icon_e) = ui_nodes.get_slot(UiSlot::DreamPoolIcon) {
                if let Ok(mut absorb) = q_icon_absorb.get_mut(icon_e) {
                    absorb.pulse_count += 1;
                }
            }
            commands.entity(entity).try_despawn();
        }
    }
}
```

### 1-5. スポーン関数変更

`spawn_ui_particle` → `spawn_bubble`:

```rust
pub fn spawn_bubble(
    commands: &mut Commands,
    start_pos: Vec2,
    target_pos: Vec2,
    ui_root: Entity,
    assets: &GameAssets,
) {
    let mut rng = rand::thread_rng();
    commands.spawn((
        BubbleBody {
            velocity: Vec2::new(
                rng.gen_range(-15.0..15.0),
                rng.gen_range(-30.0..-10.0), // 初速: やや上向き
            ),
            target_pos,
            lifetime: DREAM_UI_PARTICLE_LIFETIME,
            max_lifetime: DREAM_UI_PARTICLE_LIFETIME,
            phase: rng.gen_range(0.0..TAU),
            noise_direction: random_unit_vec2(&mut rng),
            noise_timer: rng.gen_range(0.0..DREAM_UI_NOISE_INTERVAL),
            merge_count: 0,
            merging_into: None,
            merge_timer: 0.0,
            trail_cooldown: 0.0,
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(start_pos.x),
            top: Val::Px(start_pos.y),
            width: Val::Px(DREAM_UI_PARTICLE_SIZE),
            height: Val::Px(DREAM_UI_PARTICLE_SIZE),
            ..default()
        },
        ImageNode::new(assets.dream_bubble.clone()),
        BackgroundColor(Color::srgb(0.65, 0.9, 1.0).with_alpha(0.0)),
        ZIndex(0),
        Name::new("DreamBubble"),
    )).set_parent(ui_root);
}
```

### 1-6. 呼び出し元変更

- `gain_visual.rs`: `spawn_ui_particle(...)` → `spawn_bubble(...)`
  - シグネチャ変更: viewport_size, lifetime 引数を削除
- `particle.rs` (rest_area): 同上

### 1-7. プラグイン登録 (`visual.rs`)

```rust
// 旧: ui_particle_update_system
// 新: bubble_physics_system
```

### 1-8. コンパイル確認

`cargo check` → エラー修正 → 動作確認

### Phase 1 完了基準
- 泡テクスチャが表示される
- 泡が浮力で上方向に漂い、徐々にアイコンに吸い込まれる
- Wobble で泡が不定形に揺れる
- 後半で縮小+明度上昇する
- アイコン付近で到着判定により消滅する

---

## Phase 2: 合体 + Trail

### 目標
近接する泡が合体し画面が整理される。薄い軌跡で泡の経路を示す。

### 2-1. 合体定数 (`constants/dream.rs`)

```rust
pub const DREAM_UI_MERGE_RADIUS: f32 = 20.0;
pub const DREAM_UI_MERGE_SIZE_BONUS: f32 = 2.0;
pub const DREAM_UI_MERGE_MAX_COUNT: u8 = 4;
pub const DREAM_UI_MERGE_DURATION: f32 = 0.15;
```

### 2-2. Trail定数

```rust
pub const DREAM_UI_TRAIL_INTERVAL: f32 = 0.12;
pub const DREAM_UI_TRAIL_LIFETIME: f32 = 0.15;
pub const DREAM_UI_TRAIL_SIZE_RATIO: f32 = 0.5;
pub const DREAM_UI_TRAIL_ALPHA: f32 = 0.2;
```

### 2-3. 合体判定 system (`ui_particle_merge_system`)

```rust
pub fn ui_particle_merge_system(
    mut q_bubbles: Query<(Entity, &mut BubbleBody, &Node)>,
)
```

1. 全 BubbleBody の位置を収集 (`merging_into.is_some()` は除外)
2. ペアワイズ距離チェック (O(n²)、n≤10)
3. 距離 < `MERGE_RADIUS` かつ両方 `life_ratio > 0.15`:
   - `life_ratio` が大きい方（より長く生存）を吸収者に
   - 吸収対象: `merging_into = Some(absorber)`, `merge_timer = MERGE_DURATION`
   - 吸収者: `merge_count += 1` (上限チェック)
4. 1フレーム1ペアのみ処理

### 2-4. 吸収アニメーション (`bubble_physics_system` 内に追加)

`merging_into == Some(target)` の泡:
1. 通常の力学計算を停止
2. target の現在位置に向かって線形補間で移動
3. サイズ: `current_size * (1.0 - progress)` で縮小
4. alpha: `0.9 * (1.0 - progress)` でフェード
5. `merge_timer <= 0` で despawn

### 2-5. Trail system (`dream_trail_ghost_update_system`)

**Trail生成** (`bubble_physics_system` 内):
- `trail_cooldown` 消化時、`life_ratio: 0.1~0.7` でゴースト生成
- 吸収中(merging_into.is_some())の泡はTrail生成しない

**Trailゴースト更新**:
```rust
pub fn dream_trail_ghost_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q_ghosts: Query<(Entity, &mut DreamTrailGhost, &mut BackgroundColor)>,
)
```
- lifetime減算、alpha = (lifetime/max) * TRAIL_ALPHA
- lifetime <= 0 で despawn

### 2-6. プラグイン登録更新

chain に `ui_particle_merge_system`, `dream_trail_ghost_update_system` 追加。

### Phase 2 完了基準
- 近接泡が縮みながら吸い寄せられて合体する
- 合体後の泡がサイズアップしている
- 薄い軌跡が漂いフェーズで表示され、すぐに消える
- 大量生成時に合体で画面が整理される

---

## Phase 3: Icon 吸収反応

### 目標
泡到着時に DreamPoolIcon が視覚的に反応する。

### 3-1. 定数 (`constants/dream.rs`)

```rust
pub const DREAM_ICON_ABSORB_DURATION: f32 = 0.25;
pub const DREAM_ICON_BASE_SIZE: f32 = 16.0;
pub const DREAM_ICON_PULSE_SIZE: f32 = 20.0;
```

### 3-2. コンポーネント (`components.rs`)

```rust
#[derive(Component, Default)]
pub struct DreamIconAbsorb {
    pub timer: f32,
    pub pulse_count: u8,
}
```

### 3-3. Icon Absorb system

```rust
pub fn dream_icon_absorb_system(
    time: Res<Time>,
    theme: Res<UiTheme>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_icon: Query<(&mut Node, &mut BackgroundColor, &mut DreamIconAbsorb)>,
)
```

- `pulse_count > 0`: timer = DURATION, count = 0
- timer > 0: sin(progress * PI) で 16px → 20px → 16px
- BackgroundColor: accent_soul_bright → 白方向に50%補間 → 復帰
- timer <= 0: ベースサイズ・色に復帰

### 3-4. DreamPoolIcon 変更 (`time_control.rs`)

spawn に `DreamIconAbsorb::default()` 追加。

### 3-5. プラグイン登録更新

chain に `dream_icon_absorb_system` 追加。

### Phase 3 完了基準
- 泡到着時にアイコンが膨張→収縮する
- アイコンが白方向にフラッシュする
- 連続到着でもスムーズに脈動する

---

## 定数チューニングガイド

実装後、以下の値をゲームプレイを見ながら調整:

| 定数 | 効果 | 上げると | 下げると |
|------|------|----------|----------|
| `BUOYANCY` | 浮力 | 泡が速く上昇 | 泡がゆっくり |
| `BASE_ATTRACTION` | 基礎吸引力 | 最初から引かれる | 漂い時間が長い |
| `ATTRACTION_EXPONENT` | 吸引の加速度 | 後半が急激 | なだらかに到着 |
| `DRAG` | 抵抗 | 速度上限が上がる | 泡が重い |
| `NOISE_STRENGTH` | 揺らぎ | 泡がふらつく | 直線的に移動 |
| `PARTICLE_LIFETIME` | 寿命 | 長く漂う | 短く吸収 |
| `MERGE_RADIUS` | 合体距離 | 合体しやすい | 合体しにくい |
| `ARRIVAL_RADIUS` | 到着距離 | 遠くで消える | 正確に到着 |
| `DREAM_POPUP_THRESHOLD` | 発生頻度 | 頻度下がる | 頻度上がる |
