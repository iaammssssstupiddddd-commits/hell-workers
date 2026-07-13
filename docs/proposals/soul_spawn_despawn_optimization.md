# Soul Spawn/Despawn 最適化提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `soul-spawn-despawn-optimization` |
| ステータス | `Draft / Active` |
| 最終更新日 | `2026-07-13` |
| 対象バージョン | `Bevy 0.19` |

## 前提

- 川南岸の広めのランダム範囲と `SOUL_SPAWN_INITIAL=10` / `SOUL_POPULATION_BASE_CAP=10` は現仕様として維持する。
- 本提案は上記2点を変更しない。
- 判定は並行作業の差分を含めず HEAD を基準とする。

---

## 実装状況

| 項目 | 状態 | 概要 |
|:--|:--:|:--|
| 予兆演出（60秒/90秒） | 未着手 | 脱走前の視覚・吹き出し予兆を段階表示 |
| 端到達フェードアウト | 未着手 | Drifting の即時消滅を段階的消滅に変更 |
| `SpawnSource` インターフェース | 延期 | source 別の実需がないため先行抽象化しない |
| `total_spawned` 統計整合 | 未着手 | HEAD では初期スポーン分だけ集計されない |

---

## Phase 1: 脱走予兆の可視化

### 目的
- プレイヤーが脱走前に「管理すべき Soul」を認識できる状態にする。

### 提案
- 60秒以上未管理:
  - `SoulFaceState` または低頻度の小さな表情 cue で不満を示す。
- 90秒以上未管理:
  - 低頻度で不満系絵文字を表示（例: `😓`, `😤`）。
- 120秒以上:
  - 現行どおり Drifting 判定へ移行。

### 変更候補ファイル
- `crates/bevy_app/src/systems/soul_ai/visual/idle.rs`
- `crates/bevy_app/src/systems/visual/speech/periodic.rs`
- `crates/bevy_app/src/constants/speech.rs`

### 実装メモ
- 予兆判定は「未管理」の定義に揃える。
  - `CommandedBy` なし
  - `AssignedTask::None`
  - `Resting/GoingToRest` 以外
- `IdleState.total_idle_time` を一次指標として再利用する（新規タイマーは導入しない）。
- Soul body の `CharacterMaterial` handle は共有されているため、個体ごとの `base_color` を直接変更しない。色変化が必要なら per-instance body material 化を別判断とし、本提案の初手は既存 face / speech 経路を使う。

### 受け入れ基準
- 未管理 60 秒以降に個体単位の予兆が始まり、他 Soul の見た目を変えない。
- 未管理 90 秒以降に不満系吹き出しが断続的に出る。
- 指揮/タスク再付与時に予兆表示が停止する。

---

## Phase 2: マップ端到達時のフェードアウト

### 目的
- Drifting の退出を視覚的に自然化する（「気づいたらいない」体験の強化）。

### 提案
- 端到達時に即時 `despawn` せず、`DriftDespawning { elapsed, duration }` へ移行する。
- visible / shadow / mask の各 Soul proxy root を同じ進捗で縮小または上方へ消散させ、完了後に owner Soul を `despawn` する。
- `PopulationManager.total_escaped` はフェード開始時に1回だけ加算。

### 変更候補ファイル
- `crates/hw_soul_ai/src/soul_ai/execute/drifting.rs`
- `crates/bevy_app/src/systems/visual/character_proxy_3d.rs`
- `crates/bevy_app/src/plugins/visual.rs`
- `crates/hw_core/src/soul.rs`

### 実装メモ
- `hw_core::visual::FadeOut` / `hw_visual::fade_out_system` は `Sprite` 専用なので再利用しない。
- body material、mask material、shadow material は共有 handle を含むため、共有 material の alpha を変更しない。root Transform ベースの消散なら個体単位に閉じられる。
- 二重加算防止のため `DriftDespawning` を追加し、フェード中は drifting の移動更新と再度の端判定から除外する。
- save/load 対象にするかは duration の短さを踏まえて明示する。保存しない場合は save 前に通常 drifting として扱う復元契約を記載する。

### 受け入れ基準
- Drifting Soul は端到達後すぐ消えず、短時間の個体単位の消散を経て消える。
- `total_escaped` は1体につき1回のみ増加する。

---

## Phase 3: 初期スポーン統計の整合

### 目的
- 初期/定期/緊急で `PopulationManager.total_spawned` の加算契約を一致させる。

### 提案
- `spawn_damned_souls` が `queue_river_spawn_events` から受け取る実生成数を `total_spawned` に加算する。
- `SpawnSource` は source 別ログ・UI・ゲームルールの具体的な consumer が発生するまで導入しない。

### 変更候補ファイル
- `crates/bevy_app/src/entities/damned_soul/spawn.rs`
- `docs/population_system.md`（仕様同期）

### 実装メモ
- event 型は変更せず、queue に成功した件数だけを一度加算する。
- perf scenario の初期生成を通常統計へ含めるかは `docs/population_system.md` で固定し、テストで分岐を明示する。

### 受け入れ基準
- 初期/定期/緊急スポーン後の `total_spawned` が、仕様上カウント対象の実生成数と一致する。
- source 抽象化を増やさず既存 `DamnedSoulSpawnEvent` の互換性を維持する。

---

## リスクと対応

- 予兆演出の過多で視認性が落ちる可能性:
  - face cue は弱く、吹き出しは低頻度で開始する。
- フェード導入で更新順の競合が出る可能性:
  - Drifting 実行系と Fade 系の順序を固定する。
- 3D proxy の一部だけが残る可能性:
  - owner cache を使い visible / shadow / mask の3系統を同じ進捗で扱い、owner despawn 後の既存 cleanup を最終防衛線にする。

---

## 検証手順

1. `cargo check --workspace`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo run` で通常プレイし、未管理 Soul の 60/90/120 秒挙動を目視確認
4. 端到達時に visible / shadow / mask が同期して消え、`total_escaped` が単回加算されることを確認
5. 初期/定期/緊急スポーン後の `total_spawned` 整合を確認
