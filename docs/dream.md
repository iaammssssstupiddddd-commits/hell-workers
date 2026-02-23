# Dream システム

Soul が生み出す「Dream（夢）」をグローバルプールに蓄積するシステムです。  
現在は **睡眠** と **休憩所滞在** の 2 経路で Dream が増加します。

## 1. 概要

- Dream は共有リソースで、`DreamPool.points` に集約される。
- Soul 個別に保持せず、獲得分は即時にグローバルプールへ加算される。
- 睡眠時は `DreamQuality`（夢の質）でレートが変化する。
- 休憩所滞在時は `DreamQuality` と無関係な固定レートで加算される。

## 2. データモデル

### 2.1 `DreamQuality`（夢の質）

睡眠開始時（`Awake` -> 睡眠状態）に 1 回だけ判定され、睡眠中は固定されます。

| 質 | 蓄積レート | 発生条件 |
| :--- | :--- | :--- |
| `VividDream` | +0.15/s | `stress < 0.3` かつ集会中睡眠 |
| `NormalDream` | +0.10/s | 睡眠中かつ上記以外 |
| `NightTerror` | 0/s（獲得なし） | `stress > 0.7` |
| `Awake` | — | 睡眠状態でない |

> 実装上、境界値は `>` / `<` 判定です（`stress == 0.7` は `NightTerror` にならない、`stress == 0.3` は `VividDream` にならない）。

### 2.2 `DreamState` コンポーネント

Soul ごとの夢状態トラッキングです。

```rust
#[derive(Component, Reflect, Default)]
pub struct DreamState {
    pub quality: DreamQuality,
}
```

- スポーン時は `DreamState::default()`（`Awake`）。
- 睡眠開始時に `DreamQuality` を確定。
- 起床時（睡眠条件を満たさないフレーム）に `Awake` へ戻る。

### 2.3 `DreamPool` リソース

全 Soul 共有の Dream プールです。

```rust
#[derive(Resource, Default, Reflect)]
pub struct DreamPool {
    pub points: f32,
}
```

- `DamnedSoulPlugin` で `init_resource::<DreamPool>()`。
- 睡眠・休憩所の両方から加算される。

### 2.4 `DreamVisualState` コンポーネント

Dream 演出（粒子/ポップアップ）用の Soul 個別状態です。

- `particle_cooldown`
- `popup_accumulated`
- `active_particles`

`ensure_dream_visual_state_system` が `DreamState` を持つ Soul に自動付与します。

## 3. Dream 蓄積ロジック

Dream は `SoulAiSystemSet::Update` で以下 2 系統から加算されます。

### 3.1 睡眠由来 (`dream_update_system`)

対象: `IdleBehavior::Sleeping` または  
`IdleBehavior::Gathering && gathering_behavior == Sleeping && ParticipatingIn あり`

処理フロー:

1. 睡眠状態を判定
2. `DreamState.quality == Awake` の場合のみ `DreamQuality` を決定
3. 質に応じて `DreamPool.points += rate * delta_time`
4. 非睡眠時は `DreamState.quality = Awake`

### 3.2 休憩所由来 (`rest_area_update_system`)

各 `RestArea` について:

- `occupant_count = min(RestAreaOccupants.len(), capacity)`
- `DreamPool.points += occupant_count as f32 * REST_AREA_DREAM_RATE * delta_time`

特性:

- `DreamQuality` やストレス値に依存しない固定加算
- 休憩所が複数ある場合は合算
- 同システム内で疲労/ストレス回復、自動退出、クールダウン更新も実施

## 4. ビジュアルフィードバック

→ **[dream-visual.md](dream-visual.md)** を参照してください。

## 5. ゲームデザイン上の意図

### 労働 vs 休息のジレンマ

- Soul を働かせる: 物理リソース生産
- Soul を休ませる: Dream 生産（睡眠 + 休憩所）
- 同一 Soul は同時に両立できないため、配置判断が必要

### ストレス管理の重要性

- 高ストレス睡眠は `NightTerror` になり、睡眠由来 Dream を得られない
- 休憩/集会でのケアは Dream 生産効率に直結

## 6. Dream 消費：植林（DreamPlanting）

Dream を消費してプレイヤーが指定した矩形範囲に木を植えるシステムです。

### 6.1 操作フロー

1. 下部バーの **「Dream」ボタン** を押してサブメニューを開く
2. **「Plant Trees」ボタン** を選択 → `TaskMode::DreamPlanting` に移行
3. マップ上でドラッグ開始（開始時にプレビュー用シードを固定）
4. ドラッグ中は、実際に生成される候補位置を半透明ツリーでプレビュー表示
5. ドラッグ解放でイベントが発行され、同じシード・同じ計画関数で植林候補を確定する

### 6.2 植林ルール

| 項目 | 値 | 説明 |
| :--- | :--- | :--- |
| スポーン率 | 0.25 本/タイル | 指定タイル数 × 0.25 を目安に生成 |
| 最低サイズ | 幅2かつ高さ2タイル以上 | 2×2 正方形以上を必須とする（例: 1×4 は不可） |
| 1回あたり上限 | 20 本 | `DREAM_TREE_MAX_PER_CAST` |
| 全体木上限 | 300 本 | `DREAM_TREE_GLOBAL_CAP`（自然再生と共有） |
| コスト | 20 Dream/本 | `DREAM_TREE_COST_PER_TREE` |
| プレビュー一致 | あり | プレビューと確定で同一の計画関数・シードを使用 |

### 6.3 制約条件

スポーン候補タイルは以下を**除外**します：

- 歩行不可タイル（壁・岩など）
- 建物が存在するタイル
- アイテムが落ちているタイル

最終生成本数は **スポーン率・候補数・1回上限・全体上限・Dream残高** の最小値で決まります。
いずれかが 0 の場合は Dream を消費せずに終了します。
また、最小サイズ制約は「面積」ではなく「幅・高さを個別判定」します。

### 6.4 資源再生との関係

- `tree_regrowth_system`（自然再生）も同じ `DREAM_TREE_GLOBAL_CAP` を参照
- 上限 300 本に達すると自然再生も Dream 植林も停止

### 6.5 関連定数（`src/constants/dream.rs`）

| 定数 | 値 |
| :--- | :--- |
| `DREAM_TREE_SPAWN_RATE_PER_TILE` | 0.25 |
| `DREAM_TREE_COST_PER_TREE` | 20.0 |
| `DREAM_TREE_MAX_PER_CAST` | 20 |
| `DREAM_TREE_GLOBAL_CAP` | 300 |
| `DREAM_TREE_MAGIC_CIRCLE_DURATION` | 0.20 |
| `DREAM_TREE_GROWTH_DURATION` | 0.35 |
| `DREAM_TREE_LIFE_SPARK_DURATION` | 0.28 |
| `DREAM_TREE_LIFE_SPARK_COUNT` | 8 |

### 6.6 関連ファイル

| ファイル | 内容 |
| :--- | :--- |
| `src/systems/dream_tree_planting.rs` | 植林コアロジック |
| `src/systems/command/area_selection/state.rs` | `pending_dream_planting` と `dream_planting_preview_seed` の保持 |
| `src/systems/command/area_selection/input.rs` | `DreamPlanting` モードの入力分岐とドラッグ入力処理 |
| `src/systems/command/area_selection/input/release.rs` | `DreamPlanting` モードのリリース確定処理 |
| `src/systems/command/area_selection/indicator.rs` | 植林候補プレビュー描画（`DreamTreePreviewIndicator`） |
| `src/interface/ui/components.rs` | `MenuState::Dream`, `MenuAction::{ToggleDream, SelectDreamPlanting}`, `DreamSubMenu` |
| `src/interface/ui/setup/submenus.rs` | Dream サブメニューのスポーン |
| `src/plugins/logic.rs` | `dream_tree_planting_system` 登録 |
| `src/plugins/visual.rs` | Plant Trees 演出システム登録 |
| `src/systems/visual/plant_trees/systems.rs` | 魔法陣/成長/生命力スパークの更新 |
| `src/world/regrowth.rs` | グローバル木上限チェック追加 |
| `assets/textures/ui/plant_tree_magic_circle.png` | 植林予兆エフェクト（プレースホルダー） |
| `assets/textures/ui/plant_tree_life_spark.png` | 生命力スパーク（プレースホルダー） |

### 6.7 植林ビジュアル（3フェーズ）

Dream 植林で生成された木は `PlantTreeVisualState` を持って開始し、`GameSystemSet::Visual` で次の順に演出されます。

1. **魔法陣**：対象タイルに青白い円がフェードイン → 拡大 → フェードアウト
2. **急成長**：木スプライトを縮小状態から等倍へ補間し、発光色から白へ遷移
3. **生命力スパーク**：根元から短寿命の粒子を円状に放射して消滅

補足:

- 木の障害物状態は生成時点で `ObstaclePosition` と `world_map.add_obstacle` により即座に反映される
- 地形タイル（Dirt/Grass/Sand/River）は書き換えない
- ドラッグ中は `DreamTreePreviewIndicator` を表示し、解放時に同じ計画結果で確定する

---

## 7. 主要定数

| 定数 | 値 | 用途 |
| :--- | :--- | :--- |
| `DREAM_RATE_VIVID` | 0.15 | `VividDream` の睡眠蓄積レート |
| `DREAM_RATE_NORMAL` | 0.10 | `NormalDream` の睡眠蓄積レート |
| `DREAM_NIGHTMARE_STRESS_THRESHOLD` | 0.7 | `stress > 0.7` で `NightTerror` |
| `DREAM_VIVID_STRESS_THRESHOLD` | 0.3 | `stress < 0.3` かつ集会中で `VividDream` |
| `REST_AREA_DREAM_RATE` | 0.12 | RestArea 滞在者 1 人あたりの蓄積レート |
| `DREAM_TREE_SPAWN_RATE_PER_TILE` | 0.25 | 植林レート（本/タイル） |
| `DREAM_TREE_COST_PER_TREE` | 20.0 | 植林コスト（Dream/本） |
| `DREAM_TREE_MAX_PER_CAST` | 20 | 1 回あたりの最大植林本数 |
| `DREAM_TREE_GLOBAL_CAP` | 300 | 全体の木の上限本数 |

ビジュアル関連の定数は [dream-visual.md](dream-visual.md) を参照してください。

## 8. 関連ファイル

| ファイル | 内容 |
| :--- | :--- |
| `src/entities/damned_soul/mod.rs` | `DreamQuality`, `DreamState`, `DreamPool` 定義/初期化 |
| `src/entities/damned_soul/spawn.rs` | Soul スポーン時の `DreamState::default()` |
| `src/constants/dream.rs` | Dream 関連全定数 |
| `src/constants/ai.rs` | `REST_AREA_DREAM_RATE` など AI 側定数 |
| `src/systems/soul_ai/update/dream_update.rs` | 睡眠由来 Dream 蓄積 |
| `src/systems/soul_ai/update/rest_area_update.rs` | 休憩所由来 Dream 蓄積 + 休憩更新 |
| `src/systems/dream_tree_planting.rs` | Dream 植林コアロジック |
| `src/systems/visual/plant_trees/components.rs` | 植林演出状態コンポーネント |
| `src/constants/render.rs` | 植林演出用の Z レイヤー定義 |
| [dream-visual.md](dream-visual.md) | ビジュアルフィードバック全般 |

## 9. 未実装（将来拡張）

- Soul 鼓舞・作業速度バフなど Dream 消費効果
- Familiar からの明示的な睡眠命令
