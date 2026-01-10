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
|------|--------|-----------|----|
| 使役中の待機 | -0.01 × dt | 約100秒 | `fatigue_update_system` で処理 |
| 通常の待機 | -0.05 × dt | 約20秒 | `fatigue_update_system` で処理 |

## 閾値と動作

| 定数名 | 値 | 説明 |
|--------|-----|------|
| `FATIGUE_THRESHOLD` | 0.8 (80%) | この値以上でタスクを受け付けない |
| `FATIGUE_GATHERING_THRESHOLD` | 0.9 (90%) | この値以上で強制的に集会へ向かう |

### 閾値による状態変化

| 疲労値 | 状態 | 動作 |
|--------|------|------|
| < 80% | 通常 | タスクを受け付ける |
| ≥ 80% | 疲労 | 新規タスクを受け付けない |
| > 90% | 極度の疲労 | 使役解除 + 強制的に集会エリアへ移動 + やる気低下 |

## 使い魔の疲労閾値

使い魔ごとに `fatigue_threshold` を設定可能（UIで調整可）。

- デフォルト: 0.8 (80%)
- この閾値以上の魂はタスク割り当て対象外
- この閾値を超えた魂は使役から解放

## 疲労と他ステータスの相互作用

### やる気（Motivation）との関係

疲労が90%を超えると、`fatigue_penalty_system` によりやる気が強制的に減少：
- 減少率: -0.5 × dt

### 怠惰行動（Idle Behavior）への影響

怠惰行動システムでは以下の条件で行動が変化：
- `fatigue >= FATIGUE_THRESHOLD` かつ `motivation <= MOTIVATION_THRESHOLD`: 怠惰行動を開始
- `fatigue > FATIGUE_GATHERING_THRESHOLD`: `ExhaustedGathering` 状態へ移行

## UI表示

疲労が80%を超えると、魂の上に「Zzz」テキストが赤色で表示される。

## 関連ファイル

- `src/constants.rs` - `FATIGUE_THRESHOLD`, `FATIGUE_GATHERING_THRESHOLD` 定義
- `src/systems/fatigue.rs` - `fatigue_update_system`, `fatigue_penalty_system`（メイン）
- `src/systems/task_execution.rs` - タスク完了時の疲労増加
- `src/systems/idle.rs` - 疲労に基づく怠惰行動
- `src/systems/familiar_ai.rs` - 使い魔のリクルート時の疲労チェック
- `src/entities/familiar.rs` - `FamiliarOperationState.fatigue_threshold`

