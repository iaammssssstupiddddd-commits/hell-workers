# Dream システム

Soulが睡眠中に「Dream（夢）」を獲得し、グローバルプールに蓄積する機能です。
労働（物理リソース生産）vs 睡眠（Dream獲得）のトレードオフを生むことがコアコンセプトです。

## 1. 概要

- Dreamは**通貨のように蓄積・消費されるリソース**で、睡眠中にのみ獲得できる。
- 各Soulが個別にポイントを持つのではなく、**グローバルな共有プール `DreamPool`** に即座に加算される。
- 夢の質（`DreamQuality`）はストレスと集会参加状態によって決定され、蓄積レートに影響する。

## 2. データモデル

### 2.1 DreamQuality（夢の質）

睡眠開始時に1回判定され、その睡眠中は固定。

| 質 | 蓄積レート | 発生条件 |
| :--- | :--- | :--- |
| `VividDream` | +0.15/s | stress < 0.3 かつ集会中睡眠 |
| `NormalDream` | +0.10/s | stress < 0.7（上記以外） |
| `NightTerror` | 0/s（獲得なし） | stress >= 0.7 |
| `Awake` | — | 起きている（睡眠中でない） |

### 2.2 DreamState コンポーネント

Soul個別に付与。夢の質の追跡用。

```rust
#[derive(Component, Reflect, Default)]
pub struct DreamState {
    pub quality: DreamQuality,
}
```

- スポーン時に `DreamState::default()`（= `Awake`）で初期化。
- 睡眠開始時に質を判定、睡眠終了時に `Awake` にリセット。

### 2.3 DreamPool リソース

グローバルな共有Dreamプール。

```rust
#[derive(Resource, Default, Reflect)]
pub struct DreamPool {
    pub points: f32,
}
```

- `DamnedSoulPlugin` で `init_resource::<DreamPool>()` により初期化。
- 各Soulの睡眠から獲得したポイントが即座に加算される。

## 3. 睡眠の検出

以下の2パターンで睡眠中と判定する:

1. **通常睡眠**: `idle.behavior == IdleBehavior::Sleeping`
2. **集会中睡眠**: `idle.behavior == IdleBehavior::Gathering` かつ `idle.gathering_behavior == GatheringBehavior::Sleeping` かつ `ParticipatingIn` あり

どちらでもない場合、`DreamState.quality` は `Awake` にリセットされる。

## 4. 蓄積システム

`dream_update_system`（`SoulAiSystemSet::Update`）で毎フレーム処理。

1. 各Soulの睡眠状態を判定
2. 睡眠開始時（`quality == Awake` → 睡眠中）に `DreamQuality` を決定:
   - `stress >= 0.7` → `NightTerror`
   - `stress < 0.3` かつ集会中 → `VividDream`
   - それ以外 → `NormalDream`
3. 質に応じたレートで `DreamPool.points += rate * delta_time`
4. 起床時に `quality = Awake`

## 5. ビジュアルフィードバック

`idle_visual_system` にて、睡眠中のSoulの色が夢の質に応じて変化する。

### 通常睡眠
| 質 | 色 |
| :--- | :--- |
| `VividDream` | 青味がかった明るい色 `(0.5, 0.6, 0.9, 1.0)` |
| `NightTerror` | 赤味がかった暗い色 `(0.8, 0.4, 0.4, 1.0)` |
| その他 | 既存の睡眠色 `(0.6, 0.6, 0.7, 1.0)` |

### 集会中睡眠
| 質 | 色 |
| :--- | :--- |
| `VividDream` | 青紫系 `(0.5, 0.5, 0.9, 0.7)` |
| `NightTerror` | 赤紫系 `(0.8, 0.4, 0.5, 0.6)` |
| その他 | 既存の集会睡眠色 `(0.6, 0.5, 0.8, 0.6)` |

## 6. UI表示

右上の時間コントロール領域（タスクサマリーの下）に `Dream: X.X` を表示。

- `UiSlot::DreamPoolText` で管理。
- `update_dream_pool_display_system`（`GameSystemSet::Interface`）で `DreamPool` リソース変更時に更新。
- 色は `accent_soul_bright`（明るい青）。

## 7. ゲームデザイン上の意図

### 労働 vs 睡眠のジレンマ
- Soulを働かせる → 物理リソース（Wood, Rock等）を生産
- Soulを寝かせる → Dreamを獲得
- **両方を同時には得られない** → プレイヤーに判断を迫る

### ストレス管理の重要性
- ストレスが高い → 悪夢 → Dream獲得なし
- Soulのケア（集会、休息）が**Dream生産の前提条件**
- 「使い捨て労働」戦略にペナルティを与える

## 8. 定数一覧

| 定数 | 値 | 用途 |
| :--- | :--- | :--- |
| `DREAM_RATE_VIVID` | 0.15 | VividDream の蓄積レート（/秒） |
| `DREAM_RATE_NORMAL` | 0.10 | NormalDream の蓄積レート（/秒） |
| `DREAM_NIGHTMARE_STRESS_THRESHOLD` | 0.7 | NightTerror 判定のストレス閾値 |
| `DREAM_VIVID_STRESS_THRESHOLD` | 0.3 | VividDream 判定のストレス閾値 |

## 9. 関連ファイル

| ファイル | 内容 |
| :--- | :--- |
| `src/entities/damned_soul/mod.rs` | `DreamQuality`, `DreamState`, `DreamPool` 定義 |
| `src/entities/damned_soul/spawn.rs` | スポーン時の `DreamState` 初期化 |
| `src/constants/dream.rs` | Dream関連定数 |
| `src/systems/soul_ai/update/dream_update.rs` | Dream蓄積システム |
| `src/systems/soul_ai/visual/idle.rs` | 夢の質に応じたビジュアル |
| `src/interface/ui/setup/time_control.rs` | DreamPool UI表示のスポーン |
| `src/interface/ui/interaction/status_display.rs` | DreamPool UI更新システム |

## 10. 未実装（将来拡張）

- Dream消費UI（ボタン、メニュー）
- Dream消費による効果適用（Soul鼓舞、作業速度バフ、集団鼓舞等）
- Dream獲得時のポップアップエフェクト
- Familiarによる睡眠命令
