# スピーチシステムの最適化提案 (Scale: Soul 300, Familiar 30)

## 実装状況

| 項目 | 状態 | 概要 |
|:--|:--:|:--|
| 3.1 スタッキング計算のイベント駆動化 | ✅ 完了 | `RemovedComponents` + `Added` で変更時のみ再計算 |
| 3.2 感情判定の分散実行 | ✅ 完了 | `PERIODIC_EMOTION_FRAME_DIVISOR=10` で分散 |
| 3.3 話者→吹き出しの直接参照 | 未着手 | 現在は Query ループで O(n) 検索 |
| 3.4 アニメーション更新の抑制 | 未着手 | 閾値による書き込みスキップ |

---

## 完了済み

### 3.1. スタッキング計算のイベント駆動化
- `update_bubble_stacking`（`update.rs`）は `RemovedComponents<SpeechBubble>` と `Added<SpeechBubble>` を検知し、変更があったフレームのみ HashMap を再構築する。
- `Transform` の追従は毎フレーム実行するが、`offset.y` の再計算は変更時のみ。

### 3.2. 感情判定の分散実行 (Scattered Update)
- `periodic_emotion_system`（`periodic.rs`）はフレームカウンタを 0〜9 で回転。
- 各 Soul は `entity.to_bits() % 10 == current_frame` で判定対象を決定。
- 1フレームあたり全体の 1/10 のみ感情判定を実行。

---

## 未着手

### 3.3. 話者から吹き出しへの直接参照
- **現状**: `spawn_familiar_bubble` は `Query<(Entity, &SpeechBubble), With<FamiliarBubble>>` で全 FamiliarBubble をイテレーションし、同一話者を探して削除している。
- **提案**: 話者コンポーネントに `active_bubble: Option<Entity>` を持たせ O(1) で特定。
- **導入判断**: Familiar 数が 30+ で吹き出し生成頻度が高い場合に検討。現規模では影響軽微。

### 3.4. アニメーション更新の抑制
- **現状**: `PopOut` フェーズ中、毎フレーム `Sprite` の色（アルファ）を更新。
- **提案**: アルファ値の変化が閾値未満の場合は書き込みをスキップ。
- **導入判断**: 同時表示吹き出し数が 50+ になった場合に検討。
