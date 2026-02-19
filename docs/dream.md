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

### 4.1 Soul スプライト色 (`idle_visual_system`)

#### 通常睡眠 (`IdleBehavior::Sleeping`)

| 質 | 色 |
| :--- | :--- |
| `VividDream` | `(0.5, 0.6, 0.9, 1.0)` |
| `NightTerror` | `(0.8, 0.4, 0.4, 1.0)` |
| その他 | `(0.6, 0.6, 0.7, 1.0)` |

#### 集会中睡眠 (`GatheringBehavior::Sleeping`)

| 質 | 色 |
| :--- | :--- |
| `VividDream` | `(0.5, 0.5, 0.9, 0.7)` |
| `NightTerror` | `(0.8, 0.4, 0.5, 0.6)` |
| その他 | `(0.6, 0.5, 0.8, 0.6)` |

### 4.2 Dream 粒子 (`dream_particle_*`)

- 睡眠中かつ `DreamQuality != Awake` の Soul に発生
- 品質ごとに間隔・寿命・色・揺れ量が変化
- Soul ごとの同時粒子数は `DREAM_PARTICLE_MAX_PER_SOUL` で制限
- `NightTerror` でも粒子は発生（赤系）

### 4.3 `+Dream` ポップアップ (`dream_popup_*`)

- `DreamVisualState.popup_accumulated += gain_rate * dt`
- 累積が `DREAM_POPUP_THRESHOLD` を超えるたびに `+Dream` 浮遊テキストを生成
- `NightTerror` は gain rate が 0 のため生成されない

## 5. UI 表示

右上の時間コントロール領域（タスクサマリー下）に `Dream: X.X` を表示します。

- `UiSlot::DreamPoolText` でノード管理
- `update_dream_pool_display_system` が `DreamPool` 変更時に文言更新
- Dream 増加量が `DREAM_UI_PULSE_TRIGGER_DELTA` に達するごとにテキストを短時間発光
  - パルス時間: `DREAM_UI_PULSE_DURATION`
  - 明るさ係数: `DREAM_UI_PULSE_BRIGHTNESS`

補足:

- Building 情報パネルでも RestArea の現在 Dream 生成レートを  
  `Resting: current/capacity | Dream: x.xx/s` で表示

## 6. ゲームデザイン上の意図

### 労働 vs 休息のジレンマ

- Soul を働かせる: 物理リソース生産
- Soul を休ませる: Dream 生産（睡眠 + 休憩所）
- 同一 Soul は同時に両立できないため、配置判断が必要

### ストレス管理の重要性

- 高ストレス睡眠は `NightTerror` になり、睡眠由来 Dream を得られない
- 休憩/集会でのケアは Dream 生産効率に直結

## 9. Dream 消費：植林（DreamPlanting）

Dream を消費してプレイヤーが指定した矩形範囲に木を植えるシステムです。

### 9.1 操作フロー

1. 下部バーの **「Dream」ボタン** を押してサブメニューを開く
2. **「Plant Trees」ボタン** を選択 → `TaskMode::DreamPlanting` に移行
3. マップ上をドラッグして植林エリアを指定
4. ドラッグ解放でイベントが発行され、即時処理される

### 9.2 植林ルール

| 項目 | 値 | 説明 |
| :--- | :--- | :--- |
| スポーン率 | 0.25 本/タイル | 指定タイル数 × 0.25 を目安に生成 |
| 最低面積 | 4 タイル（2×2） | これ未満はキャンセル（無消費） |
| 1回あたり上限 | 20 本 | `DREAM_TREE_MAX_PER_CAST` |
| 全体木上限 | 300 本 | `DREAM_TREE_GLOBAL_CAP`（自然再生と共有） |
| コスト | 20 Dream/本 | `DREAM_TREE_COST_PER_TREE` |

### 9.3 制約条件

スポーン候補タイルは以下を**除外**します：

- 歩行不可タイル（壁・岩など）
- 建物が存在するタイル
- アイテムが落ちているタイル

最終生成本数は **スポーン率・候補数・1回上限・全体上限・Dream残高** の最小値で決まります。
いずれかが 0 の場合は Dream を消費せずに終了します。

### 9.4 資源再生との関係

- `tree_regrowth_system`（自然再生）も同じ `DREAM_TREE_GLOBAL_CAP` を参照
- 上限 300 本に達すると自然再生も Dream 植林も停止

### 9.5 関連定数（`src/constants/dream.rs`）

| 定数 | 値 |
| :--- | :--- |
| `DREAM_TREE_SPAWN_RATE_PER_TILE` | 0.25 |
| `DREAM_TREE_COST_PER_TREE` | 20.0 |
| `DREAM_TREE_MAX_PER_CAST` | 20 |
| `DREAM_TREE_GLOBAL_CAP` | 300 |

### 9.6 関連ファイル

| ファイル | 内容 |
| :--- | :--- |
| `src/systems/dream_tree_planting.rs` | 植林コアロジック |
| `src/systems/command/area_selection/state.rs` | `AreaEditSession.pending_dream_planting` フィールド |
| `src/systems/command/area_selection/input.rs` | `DreamPlanting` モードのドラッグ入力処理 |
| `src/interface/ui/components.rs` | `MenuState::Dream`, `MenuAction::{ToggleDream, SelectDreamPlanting}`, `DreamSubMenu` |
| `src/interface/ui/setup/submenus.rs` | Dream サブメニューのスポーン |
| `src/plugins/logic.rs` | `dream_tree_planting_system` 登録 |
| `src/world/regrowth.rs` | グローバル木上限チェック追加 |

---

## 10. 主要定数

| 定数 | 値 | 用途 |
| :--- | :--- | :--- |
| `DREAM_RATE_VIVID` | 0.15 | `VividDream` の睡眠蓄積レート |
| `DREAM_RATE_NORMAL` | 0.10 | `NormalDream` の睡眠蓄積レート |
| `DREAM_NIGHTMARE_STRESS_THRESHOLD` | 0.7 | `stress > 0.7` で `NightTerror` |
| `DREAM_VIVID_STRESS_THRESHOLD` | 0.3 | `stress < 0.3` かつ集会中で `VividDream` |
| `REST_AREA_DREAM_RATE` | 0.12 | RestArea 滞在者 1 人あたりの蓄積レート |
| `DREAM_PARTICLE_MAX_PER_SOUL` | 5 | Soul ごとの同時粒子上限 |
| `DREAM_POPUP_THRESHOLD` | 0.08 | `+Dream` 表示の発生閾値 |
| `DREAM_UI_PULSE_TRIGGER_DELTA` | 0.05 | UI パルス発火に必要な増加量 |
| `DREAM_UI_PULSE_DURATION` | 0.35 | UI パルス時間（秒） |
| `DREAM_TREE_SPAWN_RATE_PER_TILE` | 0.25 | 植林レート（本/タイル） |
| `DREAM_TREE_COST_PER_TREE` | 20.0 | 植林コスト（Dream/本） |
| `DREAM_TREE_MAX_PER_CAST` | 20 | 1 回あたりの最大植林本数 |
| `DREAM_TREE_GLOBAL_CAP` | 300 | 全体の木の上限本数 |

## 11. 関連ファイル

| ファイル | 内容 |
| :--- | :--- |
| `src/entities/damned_soul/mod.rs` | `DreamQuality`, `DreamState`, `DreamPool` 定義/初期化 |
| `src/entities/damned_soul/spawn.rs` | Soul スポーン時の `DreamState::default()` |
| `src/constants/dream.rs` | Dream 関連全定数 |
| `src/constants/ai.rs` | `REST_AREA_DREAM_RATE` など AI 側定数 |
| `src/systems/soul_ai/update/dream_update.rs` | 睡眠由来 Dream 蓄積 |
| `src/systems/soul_ai/update/rest_area_update.rs` | 休憩所由来 Dream 蓄積 + 休憩更新 |
| `src/systems/soul_ai/visual/idle.rs` | 夢の質に応じた Soul 色変化 |
| `src/systems/visual/dream/particle.rs` | Dream 粒子生成/更新 |
| `src/systems/visual/dream/popup.rs` | `+Dream` ポップアップ生成/更新 |
| `src/systems/dream_tree_planting.rs` | Dream 植林コアロジック |
| `src/interface/ui/setup/time_control.rs` | Dream テキストノード生成 |
| `src/interface/ui/interaction/status_display.rs` | Dream 表示更新とパルス演出 |
| `src/interface/ui/presentation/builders.rs` | RestArea ツールチップの Dream/s 表示 |

## 12. 未実装（将来拡張）

- Soul 鼓舞・作業速度バフなど Dream 消費効果
- Familiar からの明示的な睡眠命令
