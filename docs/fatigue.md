# 疲労（Fatigue）システム

魂（Soul）は活動に応じて疲労が蓄積し、限界に達すると休息を取ります。

## 疲労値

- **範囲**: 0.0 ～ 1.0（0% ～ 100%）
- **初期値**: 0.0

## 増減ロジック

### 増加

| 状況 | 変化率 | 100%到達時間 | 備考 |
|------|--------|-------------|------|
| タスク完了（採集系） | +0.10（瞬時） | - | 木や岩を採集完了時 |
| タスク完了（運搬系） | +0.05（瞬時） | - | 運搬タスク完了時 |
| タスク実行中 | +0.01 × dt | 約100秒 | `fatigue_update_system` で処理 |

### 減少

| 状況 | 変化率 | 0%到達時間 | 備考 |
|------|--------|-----------|------|
| 使役中の待機 | -0.01 × dt | 約100秒 | `fatigue_update_system` で処理 |
| 通常の待機 | -0.05 × dt | 約20秒 | `fatigue_update_system` で処理 |

## 閾値と定数

### グローバル定数

| 定数名 | 値 | 説明 |
|--------|-----|------|
| `FATIGUE_THRESHOLD` | 0.8 (80%) | 使い魔ごとの疲労閾値のデフォルト値 |
| `FATIGUE_IDLE_THRESHOLD` | 0.8 (80%) | 怠惰行動開始の閾値（グローバル） |
| `FATIGUE_GATHERING_THRESHOLD` | 0.9 (90%) | 強制集会の閾値（グローバル） |

### 使い魔ごとの閾値

各使い魔は `fatigue_threshold` を個別に持ち、UIで調整可能。

- デフォルト: 0.8 (80%)
- この閾値以上の魂はタスク割り当て対象外
- この閾値を超えた魂は使役から解放

### 閾値による状態変化

| 疲労値 | 状態 | 動作 |
|--------|------|------|
| < 使い魔の閾値 | 通常 | タスクを受け付ける |
| ≥ 使い魔の閾値 | 疲労 | 新規タスク拒否 + 使役解除 |
| ≥ 80% (IDLE) | 怠惰 | 怠惰行動を開始 |
| > 90% (GATHERING) | 極度の疲労 | 強制的に集会エリアへ移動 |

## OnExhausted イベント

疲労が90%（`FATIGUE_GATHERING_THRESHOLD`）を超えると `OnExhausted` イベントがトリガーされ、Observerによって以下が即座に実行されます：

1. **使役状態の解除**: `CommandedBy`Relationshipコンポーネント（旧 `UnderCommand`）を削除。
2. **現在のタスクの放棄**: `WorkingOn`Relationship（旧 `ClaimedBy`）を解除し、アイテムはドロップ。
3. `ExhaustedGathering` 状態への移行
4. 集会所への移動開始

### ExhaustedGathering → Gathering 遷移

- `ExhaustedGathering` 状態のワーカーは集会所へ向かう
- 集会所に到着すると `Gathering` 状態に遷移
- `Gathering` 状態のワーカーは疲労が高くてもリクルート対象になる

## リクルート条件

使い魔がワーカーをリクルートする際の条件：

1. **影響範囲内**: 使い魔の `command_radius` 内にいること
2. **未使役**: `CommandedBy` コンポーネントがないこと
3. **タスクなし**: `AssignedTask::None` であること
4. **疲労OK**: 疲労が使い魔の閾値未満、または `Gathering` 状態であること
5. **ストレスOK**: `StressBreakdown` 状態でないこと
6. **休息中でない**: `ExhaustedGathering` 状態でないこと

## 疲労と他ステータスの相互作用

### やる気（Motivation）との関係

疲労が90%を超えると、`fatigue_penalty_system` によりやる気が強制的に減少：
- 減少率: -0.5 × dt

### 怠惰行動（Idle Behavior）への影響

怠惰行動システムでは以下の条件で行動が変化：
- `fatigue >= FATIGUE_IDLE_THRESHOLD (80%)` かつ `motivation <= MOTIVATION_THRESHOLD`: 怠惰行動を開始
- `fatigue > FATIGUE_GATHERING_THRESHOLD (90%)`: `ExhaustedGathering` 状態へ移行

## UI表示

疲労が80%を超えると、魂の上に「Zzz」テキストが赤色で表示される。

## 関連ファイル

- `src/constants.rs` - 各閾値の定義
- `src/events.rs` - `OnExhausted` イベント定義
- `src/entities/damned_soul.rs` - `on_exhausted` Observer ハンドラの実装
- `src/entities/familiar.rs` - `FamiliarOperationState.fatigue_threshold`
- `src/systems/fatigue.rs` - `fatigue_update_system`, `fatigue_penalty_system`
- `src/systems/task_execution.rs` - タスク完了時の疲労増加
- `src/systems/idle.rs` - 疲労に基づく怠惰行動、ExhaustedGathering→Gathering遷移
- `src/systems/familiar_ai.rs` - 使い魔のリクルート時の疲労チェック
- `src/systems/spatial.rs` - グリッド登録判定
