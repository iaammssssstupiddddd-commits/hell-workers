# Dream システム

Soul が生み出す「Dream（夢）」をグローバルプールに蓄積するシステムです。  
Soul は **労働・行動中に内部 dream を蓄積し**、**睡眠・休憩で放出** して `DreamPool` へ変換します。

## 1. 概要

- Dream は共有リソースで、`DreamPool.points` に集約される。
- Soul はそれぞれ **`DamnedSoul.dream`**（0.0–100.0）を保持し、起きている間に蓄積する。
- 睡眠・休憩時に `soul.dream` を消費し `DreamPool` へ変換する（睡眠 1.0/s、休憩 0.5/s）。
- `dream == 0` のときは睡眠・休憩に入れない（dream がないと夢を見られない）。
- `DreamQuality` はビジュアル用として維持される（放出レートには影響しない）。

## 2. データモデル

### 2.1 `DamnedSoul.dream`（Soul 個別の夢貯蔵量）

```rust
pub struct DamnedSoul {
    pub dream: f32, // 夢の貯蔵量 (0.0-100.0)
    // ...
}
```

- スポーン時: `0.0`
- 起きている間（非睡眠・非休憩）に行動種別に応じたレートで増加。
- 睡眠中に `DREAM_DRAIN_RATE (1.0/s)`、休憩中に `DREAM_DRAIN_RATE_REST (0.5/s)` で `DreamPool.points` へ転換。

### 2.2 `DreamQuality`（夢の質）— ビジュアル専用

睡眠開始時（`Awake` -> 睡眠状態）に判定され、睡眠中は固定されます。  
**放出レートには影響しません**（ビジュアルエフェクトのみ）。

| 質 | 発生条件 |
| :--- | :--- |
| `VividDream` | `stress < 0.3` かつ集会中睡眠 |
| `NormalDream` | 睡眠中かつ上記以外 |
| `NightTerror` | `stress > 0.7` |
| `Awake` | 睡眠状態でない |

> 実装上、境界値は `>` / `<` 判定です。

### 2.3 `DreamState` コンポーネント

Soul ごとの夢状態トラッキングです。

```rust
#[derive(Component, Reflect, Default)]
pub struct DreamState {
    pub quality: DreamQuality,
}
```

### 2.4 `DreamPool` リソース

全 Soul 共有の Dream プールです。

```rust
#[derive(Resource, Default, Reflect)]
pub struct DreamPool {
    pub points: f32,
}
```

- `DamnedSoulPlugin` で `init_resource::<DreamPool>()`。

### 2.5 `DreamVisualState` コンポーネント

Dream 演出（粒子/ポップアップ）用の Soul 個別状態です。

- `particle_cooldown`
- `popup_accumulated`
- `active_particles`

`ensure_dream_visual_state_system` が `DreamState` を持つ Soul に自動付与します。

## 3. Dream 蓄積・放出ロジック

### 3.1 蓄積：起きている間 (`dream_update_system`)

対象: 非睡眠・非休憩状態の全 Soul

| 行動状態 | 蓄積レート |
| :--- | :--- |
| タスク中（労働） | `DREAM_ACCUMULATE_RATE_WORKING` (0.5/s) |
| 集会中 | `DREAM_ACCUMULATE_RATE_GATHERING` (0.3/s) |
| 逃走中 | `DREAM_ACCUMULATE_RATE_ESCAPING` (0.5/s) |
| その他アイドル | `DREAM_ACCUMULATE_RATE_IDLE` (0.1/s) |

- 上限: `DREAM_MAX` (100.0)

### 3.2 放出：睡眠中 (`dream_update_system`)

対象: `IdleBehavior::Sleeping` または `Gathering && gathering_behavior == Sleeping && ParticipatingIn あり`

処理:
1. `DreamQuality` を判定（ビジュアル用）
2. `drain = min(DREAM_DRAIN_RATE * dt, soul.dream)` を `DreamPool.points` へ加算
3. `soul.dream -= drain`

### 3.3 放出：休憩所 (`rest_area_update_system`)

対象: `IdleBehavior::Resting` かつ `RestingIn` あり の Soul

処理:
- `DREAM_DRAIN_RATE_REST (0.5/s)` で per-soul drain
- `soul.dream <= 0.0` になったら即座に退出（`LeaveRestArea` を発行）

同システムで疲労・ストレス回復、滞在時間による退出も実施。

## 4. Dream = 0 ガード

dream が枯渇した Soul には以下の制限がかかります：

| 状況 | 挙動 |
| :--- | :--- |
| 行動選択時 (`select_next_behavior`) | `dream <= 0` なら `Sleeping` 選択肢を除外 |
| 集会サブ行動選択時 (`random_gathering_behavior`) | `dream <= 0` なら `GatheringBehavior::Sleeping` を除外 |
| 休憩所へ向かう意欲 (`wants_rest_area`) | `dream <= 0` なら休憩所へ行かない |
| 休憩所クールダウン中 (`RestAreaCooldown`) | 予約・移動開始・入所を行わない |
| 睡眠中に dream が枯渇 | 即座に `IdleBehavior::Wandering` へ強制遷移 |
| 集会中 Sleeping サブ行動で dream が枯渇 | 即座に他サブ行動へ切り替え |

## 5. ストレス乗算

dream の蓄積量がストレス増加速度にペナルティを与えます。

```rust
let dream_stress_factor = 1.0 + soul.dream * DREAM_STRESS_MULTIPLIER;
// STRESS_WORK_RATE, ESCAPE_PROXIMITY_STRESS_RATE, SUPERVISION_STRESS_SCALE に乗算
```

- 係数: `DREAM_STRESS_MULTIPLIER` (0.005)
- ストレス**回復**（集会・アイドル）には適用しない

## 6. ビジュアルフィードバック

→ **[dream-visual.md](dream-visual.md)** を参照してください。

## 7. ゲームデザイン上の意図

### 労働 vs 休息のジレンマ

- Soul を働かせる: 物理リソース生産 + dream 蓄積加速
- Soul を休ませる: dream → DreamPool 変換（睡眠・休憩所）
- 同一 Soul は同時に両立できないため、配置判断が必要

### dream 圧力

- dream が満タンに近いほどストレスが増加しやすい
- dream が 0 では眠れないため、働きすぎ → 蓄積なし → 眠れないデスパイラルはない
  （アイドル中も微量蓄積するため、完全枯渇後も自力回復可能）

## 8. Dream 消費：植林（DreamPlanting）

→ 内容は変更なし。[dream.md の旧 §6 相当の内容を参照]

Dream を消費してプレイヤーが指定した矩形範囲に木を植えるシステムです。

### 8.1 操作フロー

1. 下部バーの **「Dream」ボタン** を押してサブメニューを開く
2. **「Plant Trees」ボタン** を選択 → `TaskMode::DreamPlanting` に移行
3. マップ上でドラッグ開始（開始時にプレビュー用シードを固定）
4. ドラッグ中は、実際に生成される候補位置を半透明ツリーでプレビュー表示
5. ドラッグ解放でイベントが発行され、同じシード・同じ計画関数で植林候補を確定する

### 8.2 植林ルール

| 項目 | 値 | 説明 |
| :--- | :--- | :--- |
| スポーン率 | 0.25 本/タイル | 指定タイル数 × 0.25 を目安に生成 |
| 最低サイズ | 幅2かつ高さ2タイル以上 | 2×2 正方形以上を必須とする（例: 1×4 は不可） |
| 1回あたり上限 | 20 本 | `DREAM_TREE_MAX_PER_CAST` |
| 全体木上限 | 300 本 | `DREAM_TREE_GLOBAL_CAP`（自然再生と共有） |
| コスト | 20 Dream/本 | `DREAM_TREE_COST_PER_TREE` |
| プレビュー一致 | あり | プレビューと確定で同一の計画関数・シードを使用 |

### 8.3 制約条件

スポーン候補タイルは以下を**除外**します：

- 歩行不可タイル（壁・岩など）
- 建物が存在するタイル
- アイテムが落ちているタイル

最終生成本数は **スポーン率・候補数・1回上限・全体上限・Dream残高** の最小値で決まります。  
いずれかが 0 の場合は Dream を消費せずに終了します。  
また、最小サイズ制約は「面積」ではなく「幅・高さを個別判定」します。

### 8.4 資源再生との関係

- `tree_regrowth_system`（自然再生）も同じ `DREAM_TREE_GLOBAL_CAP` を参照
- 上限 300 本に達すると自然再生も Dream 植林も停止

### 8.5 関連定数（`src/constants/dream.rs`）

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

### 8.6 植林ビジュアル（3フェーズ）

Dream 植林で生成された木は `PlantTreeVisualState` を持って開始し、`GameSystemSet::Visual` で次の順に演出されます。

1. **魔法陣**：対象タイルに青白い円がフェードイン → 拡大 → フェードアウト
2. **急成長**：木スプライトを縮小状態から等倍へ補間し、発光色から白へ遷移
3. **生命力スパーク**：根元から短寿命の粒子を円状に放射して消滅

## 9. 主要定数

| 定数 | 値 | 用途 |
| :--- | :--- | :--- |
| `DREAM_MAX` | 100.0 | Soul 個別 dream の上限 |
| `DREAM_ACCUMULATE_RATE_WORKING` | 0.5 | 労働中の蓄積レート |
| `DREAM_ACCUMULATE_RATE_GATHERING` | 0.3 | 集会中の蓄積レート |
| `DREAM_ACCUMULATE_RATE_ESCAPING` | 0.5 | 逃走中の蓄積レート |
| `DREAM_ACCUMULATE_RATE_IDLE` | 0.1 | アイドル中の蓄積レート |
| `DREAM_DRAIN_RATE` | 1.0 | 睡眠中の放出レート |
| `DREAM_DRAIN_RATE_REST` | 0.5 | 休憩中の放出レート |
| `DREAM_STRESS_MULTIPLIER` | 0.005 | dream によるストレス増加係数 |
| `DREAM_NIGHTMARE_STRESS_THRESHOLD` | 0.7 | `stress > 0.7` で `NightTerror` |
| `DREAM_VIVID_STRESS_THRESHOLD` | 0.3 | `stress < 0.3` かつ集会中で `VividDream` |
| `DREAM_TREE_SPAWN_RATE_PER_TILE` | 0.25 | 植林レート（本/タイル） |
| `DREAM_TREE_COST_PER_TREE` | 20.0 | 植林コスト（Dream/本） |
| `DREAM_TREE_MAX_PER_CAST` | 20 | 1 回あたりの最大植林本数 |
| `DREAM_TREE_GLOBAL_CAP` | 300 | 全体の木の上限本数 |

ビジュアル関連の定数は [dream-visual.md](dream-visual.md) を参照してください。

## 10. 関連ファイル

| ファイル | 内容 |
| :--- | :--- |
| `src/entities/damned_soul/mod.rs` | `DamnedSoul.dream`, `DreamQuality`, `DreamState`, `DreamPool` 定義/初期化 |
| `src/entities/damned_soul/spawn.rs` | Soul スポーン時の `DreamState::default()` |
| `src/constants/dream.rs` | Dream 関連全定数（蓄積・放出・ストレス・ビジュアル） |
| `src/systems/soul_ai/update/dream_update.rs` | dream 蓄積（起動中）・放出（睡眠中）・DreamQuality 判定 |
| `src/systems/soul_ai/update/rest_area_update.rs` | 休憩所での per-soul dream 放出 + 休憩更新 |
| `src/systems/soul_ai/update/vitals_influence.rs` | dream 乗算ストレス増加 |
| `src/systems/soul_ai/decide/idle_behavior/transitions.rs` | dream=0 ガード付き行動選択関数 |
| `src/systems/soul_ai/decide/idle_behavior/mod.rs` | dream=0 強制起床・wants_rest_area/休憩クールダウン ガード |
| `src/systems/soul_ai/decide/idle_behavior/motion_dispatch.rs` | 集会 Sleeping サブ行動 dream=0 チェック |
| `src/systems/dream_tree_planting.rs` | Dream 植林コアロジック |
| [dream-visual.md](dream-visual.md) | ビジュアルフィードバック全般 |

## 11. 未実装（将来拡張）

- UI への dream バー表示
- Soul 鼓舞・作業速度バフなど Dream 消費効果
- Familiar からの明示的な睡眠命令
