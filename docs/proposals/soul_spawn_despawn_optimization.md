# Soul Spawn/Despawn 最適化提案

## 前提

- `docs/plans/soul-spawn-despawn.md` の差分レビュー結果のうち、以下は現仕様として維持する。
  - 項目1: 川南岸スポーン位置の運用差（広めの南側ランダム範囲）
  - 項目4: `SOUL_SPAWN_INITIAL=10` / `SOUL_POPULATION_BASE_CAP=10`
- 本提案は上記2点を変更しない。

---

## 実装状況

| 項目 | 状態 | 概要 |
|:--|:--:|:--|
| 予兆演出（60秒/90秒） | 未着手 | 脱走前の視覚・吹き出し予兆を段階表示 |
| 端到達フェードアウト | 未着手 | Drifting の即時消滅を段階的消滅に変更 |
| `SpawnSource` インターフェース | 未着手 | 将来のイベント駆動スポーン拡張点を追加 |
| `total_spawned` 統計整合 | 未着手 | 初期スポーン分の集計漏れを解消 |

---

## Phase 1: 脱走予兆の可視化

### 目的
- プレイヤーが脱走前に「管理すべき Soul」を認識できる状態にする。

### 提案
- 60秒以上未管理:
  - Soul 色をわずかに不満寄りへ補間。
- 90秒以上未管理:
  - 低頻度で不満系絵文字を表示（例: `😓`, `😤`）。
- 120秒以上:
  - 現行どおり Drifting 判定へ移行。

### 変更候補ファイル
- `src/systems/soul_ai/visual/idle.rs`
- `src/systems/visual/speech/periodic.rs`
- `src/constants/speech.rs`

### 実装メモ
- 予兆判定は「未管理」の定義に揃える。
  - `CommandedBy` なし
  - `AssignedTask::None`
  - `Resting/GoingToRest` 以外
- `IdleState.total_idle_time` を一次指標として再利用する（新規タイマーは導入しない）。

### 受け入れ基準
- 未管理 60 秒以降に色変化が始まる。
- 未管理 90 秒以降に不満系吹き出しが断続的に出る。
- 指揮/タスク再付与時に予兆表示が停止する。

---

## Phase 2: マップ端到達時のフェードアウト

### 目的
- Drifting の退出を視覚的に自然化する（「気づいたらいない」体験の強化）。

### 提案
- 端到達時に即時 `despawn` せず、短時間のフェードアウトへ移行。
- フェード完了後に `despawn`。
- `PopulationManager.total_escaped` はフェード開始時に1回だけ加算。

### 変更候補ファイル
- `src/systems/soul_ai/execute/drifting.rs`
- `src/systems/visual/fade.rs`
- `src/systems/soul_ai/mod.rs`（必要なら実行順調整）

### 実装メモ
- 二重加算防止のため、フェード中マーカー（例: `DriftDespawning`）を追加する。
- フェード中は移動更新対象から除外する。

### 受け入れ基準
- Drifting Soul は端到達後すぐ消えず、短時間で透明化して消える。
- `total_escaped` は1体につき1回のみ増加する。

---

## Phase 3: `SpawnSource` とスポーン統計の整備

### 目的
- 将来のイベント駆動スポーン追加時に、既存ロジックを壊さず拡張可能にする。
- 生成統計を初期/定期/緊急で一貫管理する。

### 提案
- `SpawnSource` を導入（例: `Initial`, `Periodic`, `Emergency`, `Event { bonus_count }`）。
- スポーン要求時に source を保持し、集計・ログに反映。
- 初期スポーンも `total_spawned` に加算。

### 変更候補ファイル
- `src/entities/damned_soul/spawn.rs`
- `src/entities/damned_soul/mod.rs`（イベント型を拡張する場合）
- `docs/population_system.md`（仕様同期）

### 実装メモ
- 既存 `DamnedSoulSpawnEvent` を source 付きに拡張するか、内部集計専用 API を用意する。
- UI デバッグ表示追加時に source 内訳を流用できる設計にする。

### 受け入れ基準
- 初期/定期/緊急スポーンが source 別にトレースできる。
- `total_spawned` が実際の累計スポーン数と一致する。

---

## リスクと対応

- 予兆演出の過多で視認性が落ちる可能性:
  - 色変化は低強度、吹き出しは低頻度で開始する。
- フェード導入で更新順の競合が出る可能性:
  - Drifting 実行系と Fade 系の順序を固定する。
- source 拡張でイベント互換性が崩れる可能性:
  - 既存呼び出しにデフォルト source を割り当てる。

---

## 検証手順

1. `cargo check`
2. `cargo run` で通常プレイし、未管理 Soul の 60/90/120 秒挙動を目視確認
3. 端到達時のフェードアウトと `total_escaped` の単回加算をログで確認
4. 初期/定期/緊急スポーン後の `total_spawned` 整合を確認
