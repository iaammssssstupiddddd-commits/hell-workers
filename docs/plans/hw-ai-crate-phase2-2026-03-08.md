# hw_ai Phase 2 — WorldMap / SpatialGrid 境界確定計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `hw-ai-crate-phase2-2026-03-08` |
| ステータス | `Complete` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI` |
| 前フェーズ | `docs/plans/hw-ai-crate-plan-2026-03-08.md` |
| 関連提案 | `docs/proposals/hw-ai-crate.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Phase 1 時点では `hw_ai` の骨格と shared 型の整理までは進んでいるが、残存 AI システムの大半は `WorldMap` または SpatialGrid resource に依存しており、`hw_ai` へ直接移せるものと root に残すべき shell が混在している。このままだと実装の最初の一歩が毎回止まりやすい。
- 到達したい状態: Phase 2 の実装で、まず `state_apply.rs` / `state_log.rs` を `hw_ai` へ移し、続いて `WorldMap` / SpatialGrid 依存領域を root shell・root adapter・後続で `hw_ai` へ移す候補に分割できる状態にする。
- 成功指標:
  - `src/systems/familiar_ai/execute/state_apply.rs` と `state_log.rs` が `crates/hw_ai/src/familiar_ai/execute/` へ移動し、root 側は thin wrapper または re-export になっている
  - `WorldMap` / SpatialGrid 依存ファイルのうち、次に helper 抽出へ進む対象が 2 系統以上選定され、root shell と adapter の境界がコード変更対象として固定されている
  - `cargo check -p hw_ai` と `cargo check --workspace` が成功し、Familiar AI の状態遷移挙動に回帰がない

## 2. スコープ

### 対象（In Scope）

- `WorldMap` / SpatialGrid の責務境界を文書上で固定する
- 残存 AI システムを「root shell に残すもの」と「adapter を噛ませれば `hw_ai` へ寄せられるもの」に分類する
- `hw_world` / root / `hw_ai` に対する具体的な変更方針を決める
- 後続コード作業のマイルストーンと優先順位を決める

### 非対象（Out of Scope）

- `WorldMap` resource 本体の `hw_world` への移動
- `hw_spatial` の実装自体ではなく、既存計画で決まった境界実装
- `hw_core` に world/spatial 専用の抽象レイヤーを追加すること
- UI / visual / speech / asset / sprite spawn 系 shell の `hw_ai` への移動
- AI アルゴリズムの変更

### 2.1 Phase 1 の達成状況（前提）

本計画は [前フェーズ計画](docs/plans/hw-ai-crate-plan-2026-03-08.md) の進捗を前提とする。
開始時点の snapshot は以下。

| 項目 | 状態 |
| --- | --- |
| `hw_core` への SystemSet・共有 Component 移動 | ✅ |
| `hw_ai` crate 新設、`SoulAiCorePlugin` / `FamiliarAiCorePlugin` 追加 | ✅ |
| plugin 登録を `src/plugins/logic.rs` に一本化 | ✅ |
| `hw_ai::soul_ai::update::*`, `helpers::gathering`, `execute::designation_apply` | ✅ |
| `hw_ai::familiar_ai::perceive::state_detection`, `decide::following` | ✅ |
| `WorldMap` / SpatialGrid 依存の大半の decide / execute / helper | root 残留 |

## 3. 現状とギャップ

- 現状:
  - `docs/cargo_workspace.md` は `WorldMap` resource を root に残す方針を明記している
  - `hw_world` 側にはすでに `PathWorld` trait と query helper があり、root 側は `impl PathWorld for WorldMap` で接続している
  - `src/systems/spatial/grid.rs` には `SpatialGridOps` があるが、矩形検索 `get_in_area` は各 concrete grid に重複実装されている
  - `src/systems/soul_ai/` と `src/systems/familiar_ai/` では `WorldMap` 参照ファイルが 61 件、SpatialGrid 参照ファイルが 24 件ある
  - `crates/hw_ai/src/` 側には `WorldMap` / SpatialGrid 参照がまだ存在しない
- 問題:
  - 提案書 5.2 は `hw_spatial` 方針へ合わせて整理し直し、workspace ガイドと衝突しない記述へ更新済み
  - `GridData` / `SpatialGridOps` を `hw_core` に置く案は、crate 責務として world/spatial concern を core crate に持ち込む
  - 何を「移動不可」とみなすかの基準が曖昧で、後続の `hw_ai` 移行範囲が毎回再議論になる
- 本計画で埋めるギャップ:
  - `WorldMap` / SpatialGrid の最終方針を明文化し、提案書の選択肢を concrete policy へ置き換える
  - root shell / adapter / `hw_ai` core の境界をコード移動前に固定する

### 3.1 即着手スライス（低リスク）

次の 2 ファイルは `hw_core` 型のみで成立しており、`WorldMap` / SpatialGrid の整理を待たずに `hw_ai` へ移動できる。

| ファイル | 理由 | 移動先 |
| --- | --- | --- |
| `src/systems/familiar_ai/execute/state_apply.rs` | `FamiliarStateRequest`, `FamiliarAiState` など shared 型のみを扱う | `crates/hw_ai/src/familiar_ai/execute/state_apply.rs` |
| `src/systems/familiar_ai/execute/state_log.rs` | state change event のログ出力のみ | `crates/hw_ai/src/familiar_ai/execute/state_log.rs` |

### 3.2 root 残留 inventory（着手前の棚卸し）

以下は Phase 2 開始時点で root 残留とみなす初期 inventory。実装で helper 化できたものは後続 milestone で再判定する。

**Soul AI**

| ファイル群 | 現時点の残留理由 |
| --- | --- |
| `decide/drifting.rs` | `WorldMap`, `PopulationManager` 依存 |
| `decide/gathering_mgmt.rs` | `SoulSpatialGrid`, `FamiliarSpatialGrid` 依存 |
| `decide/escaping.rs` | `SoulSpatialGrid`, `FamiliarSpatialGrid` 依存 |
| `decide/idle_behavior/*` | `WorldMap`, root event, root grid 依存が混在 |
| `decide/separation.rs` | `SoulSpatialGrid` 依存 |
| `decide/work/*` | `WorldMap`, `Blueprint` 周辺の root access が残る |
| `perceive/escaping.rs` | `FamiliarSpatialGrid`, `WorldMap`, pathfinding 依存 |
| `execute/cleanup.rs` | `WorldMapRead` 依存 |
| `execute/task_execution/*` | `WorldMap` 依存が広範囲 |
| `execute/gathering_spawn.rs` | `GameAssets` + `Commands` による shell |
| `execute/drifting.rs` | `PopulationManager` 依存 |
| `update/vitals_influence.rs` | `FamiliarSpatialGrid` 依存 |
| `visual/*` | `GameAssets`, `Gizmos`, UI 依存の shell |

**Familiar AI**

| ファイル群 | 現時点の残留理由 |
| --- | --- |
| `decide/state_decision.rs` | `SpatialGrid` と root 側複合 context 依存 |
| `decide/task_management/*` | `WorldMap`, SpatialGrid, root query 群へ多重依存 |
| `decide/auto_gather_for_blueprint/*` | root event / root component 依存 |
| `decide/encouragement.rs` | root query_types 依存 |
| `decide/task_delegation.rs` | `WorldMapRead`, grid, local cache を抱える |
| `execute/max_soul_apply.rs` | speech / `GameAssets` shell |
| `execute/idle_visual_apply.rs` | visual shell |
| `execute/squad_apply.rs` | root event / squad request 依存 |
| `execute/encouragement_apply.rs` | root component 依存 |
| `perceive/resource_sync.rs` | reservation sync と task_execution 側との結合が強い |
| `helpers/query_types.rs` | root component / grid / query 定義が集中 |

### 3.3 shell として root 残留確定

| ファイル群 | 理由 |
| --- | --- |
| `src/systems/soul_ai/visual/*` | visual / gizmo / hover 表示責務 |
| `src/systems/soul_ai/execute/gathering_spawn.rs` | sprite spawn, `Commands`, `GameAssets` |
| `src/systems/familiar_ai/execute/max_soul_apply.rs` | speech bubble spawn, `GameAssets` |
| `src/systems/familiar_ai/execute/idle_visual_apply.rs` | visual state apply |

## 4. 実装方針（高レベル）

- 方針: Phase 2 は「`WorldMap` / SpatialGrid の抽象化」ではなく、「root adapter 境界の確定」として進める
- 設計上の前提:
  - `WorldMap` resource 本体は root (`bevy_app`) に残す
  - `WorldMapRead` / `WorldMapWrite` の `SystemParam` も root adapter として維持する
  - world/pathfinding/query に必要な trait は `hw_world` に置き、consumer の近くに定義する
  - `hw_core` は shared model / enum / events / relationships に留め、world/spatial 抽象は増やさない
  - SpatialGrid resource 実体と update system は root に残す
  - SpatialGrid の共有化が必要な場合でも、候補は `hw_world` であり `hw_core` ではない
- Bevy 0.18 APIでの注意点:
  - `Res<T>` / `ResMut<T>` / `RemovedComponents<T>` を読む grid update system は root に残す
  - `SystemParam` を crate 越しに無理に共有せず、root wrapper から plain reference / helper 呼び出しへ変換する
  - plugin 順序は現状の `Familiar -> Soul` および `Perceive -> Update -> Decide -> Execute` を維持する

### 4.1 固定する判断

| 論点 | 採用方針 | 理由 |
| --- | --- | --- |
| `WorldMap` の置き場所 | root に残す | `Entity` を持つ occupancy resource であり、`BuildingType` / `DoorState` を含む app shell 寄りの責務が強い |
| `WorldMap` 抽象の置き場所 | `hw_world` に寄せる | 既に `PathWorld` が存在し、pathfinding/query consumer と同じ crate に trait を置ける |
| `hw_core::WorldAccess` | 作らない | 抽象が広すぎて責務が曖昧になり、world concern を core crate に持ち込むため |
| SpatialGrid resource 本体 | `hw_spatial` へ移設（7 種）／2種は root 残置 | `Res` / `RemovedComponents` / 各 domain component を読む update shell は root |
| `GridData` / read trait の移動先 | `hw_spatial`（resource）または `hw_world`（trait） | spatial concern は world/spatial crate で責務を分離する |
| 新規 `hw_spatial` crate | 採用（7 種 concrete grid を収容） | `GatheringSpotSpatialGrid` と `FloorConstructionSpatialGrid` は root 残置 |

### 4.2 後続実装の設計ルール

1. 最初に low-risk な `state_apply.rs` / `state_log.rs` を `hw_ai` へ移し、plugin wiring を確認する。
2. `WorldMap` を読む root system をすぐに `hw_ai` へ移さない。まず root wrapper から呼ぶ純粋 helper を抽出する。
3. helper が必要とする world capability が `PathWorld` で足りるなら、そのまま generic にする。
4. `PathWorld` で不足する場合のみ、小さい trait を `hw_world` に追加する。`WorldAccess` のような omnibus trait は作らない。
5. SpatialGrid も同様に、まず read-only helper を切り出し、必要最小限の trait だけを共通化する。
6. concrete grid に依存する主因が `get_in_area` であるため、最初の共通化対象は矩形範囲検索 API とする。
7. `GameAssets`, `Commands`, speech bubble spawn, gizmo, UI state に触るシステムは Phase 2 の対象外として root shell に残す。

### 4.3 root shell / adapter / core の切り分け

| 区分 | 置き場所 | 代表例 |
| --- | --- | --- |
| Bevy resource / `SystemParam` access | root | `WorldMapRead`, `WorldMapWrite`, SpatialGrid resources |
| world/spatial generic algorithm | `hw_world` または `hw_ai` helper | `PathWorld` を受ける pathfinding helper、read-only spatial query helper |
| AI 判断・状態遷移・要求生成 | `hw_ai` | `decide/*`, `update/*`, generic helper 化できた `execute` の一部 |
| UI / visual / asset / spawn shell | root | `gathering_spawn`, `visual/*`, `max_soul_apply`, speech 系 |

## 5. マイルストーン

## M1: `state_apply` / `state_log` の `hw_ai` 移動

- 変更内容:
  - `src/systems/familiar_ai/execute/state_apply.rs` を `crates/hw_ai/src/familiar_ai/execute/state_apply.rs` へ移動
  - `src/systems/familiar_ai/execute/state_log.rs` を `crates/hw_ai/src/familiar_ai/execute/state_log.rs` へ移動
  - `FamiliarAiCorePlugin` に Execute フェーズ登録を追加
  - root 側は re-export または thin wrapper に縮退する
- 変更ファイル:
  - `crates/hw_ai/src/familiar_ai/execute/`
  - `crates/hw_ai/src/familiar_ai/mod.rs`
  - `src/systems/familiar_ai/execute/state_apply.rs`
  - `src/systems/familiar_ai/execute/state_log.rs`
  - `src/systems/familiar_ai/mod.rs`
- 完了条件:
  - [x] `state_apply_system` / `handle_state_changed_system` が `hw_ai` から提供される
  - [x] root 側の同ファイルが shell / re-export のみになっている
  - [x] `cargo check -p hw_ai` と `cargo check --workspace` が通る
- 検証:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`

## M2: WorldMap / SpatialGrid 依存の first slice 固定

- 変更内容:
  - `WorldMap` を読む helper のうち、`SystemParam` や `Commands` を必要としないものを root wrapper と pure helper に分割する対象を 2 系統以上固定する
  - 最初の対象候補:
    - `src/systems/soul_ai/helpers/gathering_positions.rs`
    - `src/systems/soul_ai/decide/idle_behavior/gathering_motion.rs`
    - `src/systems/soul_ai/execute/task_execution/common.rs`
    - `src/systems/familiar_ai/decide/task_management/task_finder/filter.rs`
  - root shell に残すファイル群は inventory どおりに据え置き、helper 抽出対象と混ぜない
- 変更ファイル:
  - `src/world/map/access.rs`
  - `src/systems/soul_ai/helpers/*`
  - `src/systems/familiar_ai/decide/task_management/*`
  - `crates/hw_ai/src/...`（必要に応じて）
  - `crates/hw_world/src/...`（必要に応じて）
- 完了条件:
  - [x] helper 抽出の first slice が 2 系統以上に固定されている
  - [x] root shell に残す対象と adapter 抽出対象が混在していない
  - [x] 各対象について、変更ファイルと検証方法が決まっている
- 検証:
  - N/A（実装 slice 固定）

## M3: root adapter 抽出の最小スライス

- 変更内容:
  - `WorldMap` を読む helper のうち、`SystemParam` や `Commands` を必要としないものを root wrapper と pure helper に分割する
  - helper の引数は `&WorldMap` 直参照か、必要最小限の `hw_world` trait bound に絞る
  - M2 で固定した first slice から着手する
- 変更ファイル:
  - `src/world/map/access.rs`
  - `src/systems/soul_ai/helpers/*`
  - `src/systems/familiar_ai/decide/task_management/*`
  - `crates/hw_ai/src/...`（必要に応じて）
  - `crates/hw_world/src/...`（必要に応じて）
- 完了条件:
  - [x] root wrapper から呼べる pure helper が少なくとも 2 系統抽出されている
  - [x] 新設 trait がある場合、それは `hw_world` にあり 1 つの狭い責務だけを持つ
  - [x] `hw_core` に world/spatial 専用 trait が追加されていない
- 検証:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`

## M4: SpatialGrid read API の共通化

- 変更内容:
  - `get_in_area` を複数 grid で共有できる read-only API にまとめる
  - 対象は `DesignationSpatialGrid`, `TransportRequestSpatialGrid`, `BlueprintSpatialGrid`, `StockpileSpatialGrid`, `FloorConstructionSpatialGrid`
  - `task_finder` と `auto_build` など concrete grid 依存の強い箇所を、可能な範囲で共通 helper / trait 経由に寄せる
- 変更ファイル:
  - `src/systems/spatial/grid.rs`
  - `src/systems/spatial/designation.rs`
  - `src/systems/spatial/transport_request.rs`
  - `src/systems/spatial/blueprint.rs`
  - `src/systems/spatial/stockpile.rs`
  - `src/systems/spatial/floor_construction.rs`
  - `src/systems/familiar_ai/decide/task_management/task_finder/*`
  - `src/systems/soul_ai/decide/work/auto_build.rs`
  - `crates/hw_world/src/...`（必要に応じて）
- 完了条件:
  - [x] `get_in_area` の重複実装が 1 箇所に集約されている、または 1 つの共通 helper から呼ばれている
  - [x] concrete grid 依存の一部が read-only trait / helper に置換されている
  - [x] resource 実体と update system は root に残っている
- 検証:
  - `cargo check --workspace`

## M5: `hw_ai` へ移せる残存システムの追加移行

- 変更内容:
  - Phase 2 の adapter 整理で root 依存を剥がせたシステムを `hw_ai` へ移す
  - まずは `hw_core` 型のみで成立するもの、次に root wrapper から generic helper を呼べるものを対象にする
  - 典型例:
    - `WorldMapRead` を root wrapper に閉じ込められる helper 群
- 変更ファイル:
  - `crates/hw_ai/src/familiar_ai/execute/*`
  - `crates/hw_ai/src/soul_ai/*`
  - `src/systems/familiar_ai/*`
  - `src/systems/soul_ai/*`
  - `src/plugins/logic.rs`
- 完了条件:
  - [x] `hw_ai` 側へ追加移動したシステムがある
  - [x] root 側の該当ファイルは shell / re-export / adapter に縮退している
  - [x] `WorldMap` / SpatialGrid 直参照が必要な shell と core の境界がコード上でも見える
- 検証:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`
  - `cargo run`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 最初の slice が大きすぎて実装が止まる | 高 | `state_apply` / `state_log` の 2 ファイル移動を最初の完了単位に固定し、`WorldMap` 依存は M2 で 2 系統に絞る |
| broad trait を先に設計してしまい API が肥大化する | 高 | trait は consumer 近傍の `hw_world` に小さく追加し、用途ごとに分ける |
| `GridData` を `hw_core` に寄せて crate 責務が崩れる | 中 | spatial concern は `hw_world` か root に残し、`hw_core` には入れない方針を固定する |
| `WorldMap` 参照 61 ファイルを一気に触ってマージ衝突が増える | 高 | helper 単位で抽出し、root shell と `hw_ai` core を段階的に分離する |
| UI / asset / speech shell を誤って `hw_ai` に混ぜる | 中 | `GameAssets`, `Commands`, gizmo, speech bubble を使うものは root 残留ルールを維持する |

## 7. 検証計画

- 必須:
  - `python scripts/update_docs_index.py`
  - `cargo check -p hw_ai`
  - `cargo check --workspace`
- 手動確認シナリオ:
  - Familiar AI の状態遷移（Idle → SearchingTask → Scouting → Supervising）が維持される
  - 状態変更ログが従来どおり出る
- パフォーマンス確認（必要時）:
  - Phase 2 のコード実装後に `cargo check --workspace --timings`

## 8. ロールバック方針

- どの単位で戻せるか:
  - docs 整理は本計画書と提案書のコミット単位
  - コード実装は M2 / M3 / M4 の単位
- 戻す時の手順:
  - docs のみ問題がある場合は計画書 / 提案書更新コミットを revert
  - code 側は milestone 単位で revert し、`WorldMap` / SpatialGrid の境界方針自体は維持する

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `90%`
- 完了済みマイルストーン: `M1`〜`M5`（`hw_ai` への移設と境界確定の一次フェーズ）
- 未着手/進行中: `M6 以降の追加移設`

### 次のAIが最初にやること

1. `state_apply.rs` / `state_log.rs` を `hw_ai` 側へ移す
2. `FamiliarAiCorePlugin` に Execute フェーズ登録を追加する
3. `cargo check -p hw_ai` と `cargo check --workspace` を通す
4. M2 として `WorldMap` helper の first slice を 2 系統に固定する

### ブロッカー/注意点

- `WorldMap` は root に残す前提で固定済み。移動案に戻さないこと
- 新しい world/spatial trait は `hw_core` ではなく `hw_world` に置くこと
- `GameAssets`, `Commands`, speech bubble, gizmo 依存システムは Phase 2 の移動対象に含めないこと
- `perceive/resource_sync.rs` と `task_execution/*` の密結合は M2 の対象に含めないこと

### 参照必須ファイル

- `docs/proposals/hw-ai-crate.md`
- `docs/cargo_workspace.md`
- `src/systems/familiar_ai/execute/state_apply.rs`
- `src/systems/familiar_ai/execute/state_log.rs`
- `crates/hw_ai/src/familiar_ai/mod.rs`
- `src/world/map/mod.rs`
- `src/world/pathfinding.rs`
- `crates/hw_world/src/pathfinding.rs`
- `src/systems/spatial/grid.rs`
- `src/systems/familiar_ai/decide/task_management/task_finder/filter.rs`

### 最終確認ログ

- 最終 `cargo check -p hw_ai`: `2026-03-08 / pass`
- 最終 `cargo check --workspace`: `2026-03-08 / pass`
- 未解決エラー: `N/A`

### Definition of Done

- [x] `state_apply` / `state_log` が `hw_ai` 側へ移動している
- [x] root 残留 inventory を参照しながら後続の helper 抽出が進められる
- [x] `WorldMap` / SpatialGrid の境界方針に沿って後続コード実装が着手されている
- [x] adapter / shell / core の切り分けがコード上で見える
- [x] `cargo check -p hw_ai` が成功
- [x] `cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
| `2026-03-08` | `AI` | `WorldMap` / SpatialGrid の concrete boundary policy を追加し、Phase 2 を root adapter 境界確定フェーズとして再定義 |
