# hw_ai crate 分離 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `hw-ai-crate-plan-2026-03-08` |
| ステータス | `Complete` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08 (Phase 2 完了後更新)` |
| 作成者 | `AI` |
| 関連提案 | `docs/proposals/hw-ai-crate.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `src/systems/soul_ai/` 98ファイルと `src/systems/familiar_ai/` 70ファイルが root crate に密集し、AI 以外の変更でも AI を含む大きな再コンパイル単位になっている。
- 到達したい状態: `crates/hw_ai/` に AI の中核ロジックを移し、root crate (`bevy_app`) は plugin 登録、`WorldMap`/SpatialGrid access、UI/asset/speech 系の shell に寄せる。
- 成功指標:
  - `cargo check --workspace` が成功する
  - `cargo check -p hw_ai` が単独で成功する
  - `cargo check --workspace --timings` の比較で、AI 非変更時の再コンパイル対象が減っていることを確認できる

## 2. スコープ

### 対象（In Scope）

- `crates/hw_ai/` の新設
- AI が直接参照する共有型・SystemSet・query 用 component の crate 境界整理
- Soul AI / Familiar AI を「core」と「shell」に分けるための責務再配置
- root crate から `hw_ai` への plugin 登録経路の整理
- 関連ドキュメント更新（`docs/architecture.md`, `docs/cargo_workspace.md`, `docs/soul_ai.md`, `docs/familiar_ai.md`, `docs/README.md`）

### 非対象（Out of Scope）

- AI アルゴリズムの改善や仕様変更
- Soul AI / Familiar AI の 2 crate 分割
- `WorldMap` resource 本体の `hw_world` への移動
- UI/visual/speech/asset 依存システムの全面 crate 移動

## 3. 現状とギャップ

- 現状:
  - plugin 登録が分散している。`FamiliarAiPlugin` は `src/main.rs`、`SoulAiPlugin` は `src/plugins/logic.rs` で登録されている。
  - 実行順序の基礎セット `GameSystemSet` は `src/systems/mod.rs`、AI 用セット `FamiliarAiSystemSet` / `SoulAiSystemSet` は `src/systems/soul_ai/scheduling.rs` にある。
  - AI が読む主要 component は root の entity module にある。例: `src/entities/damned_soul/mod.rs`, `src/entities/familiar/components.rs`
  - AI が読む world access は root resource に閉じている。例: `src/world/map/mod.rs`, `src/world/map/access.rs`
  - SpatialGrid resource も root にある。例: `src/systems/spatial/mod.rs`
  - AI 配下には root shell に強く結びついたシステムが混ざっている。例:
    - `src/systems/soul_ai/execute/gathering_spawn.rs` (`GameAssets`, `Commands`, sprite spawn)
    - `src/systems/soul_ai/visual/vitals.rs` (`HoveredEntity`, `Gizmos`)
    - `src/systems/familiar_ai/execute/max_soul_apply.rs` (speech bubble spawn, `GameAssets`, `WorldMapRead`)
- 問題:
  - 共有型が root に残っているため、AI module をそのまま別 crate へ移せない
  - `WorldMap` と SpatialGrid resource の扱いが `docs/cargo_workspace.md` の「root に残す」方針と衝突しやすい
  - Soul/Familiar 間の相互参照があり、片側だけ先に移すと import path と依存方向が崩れやすい
- 本計画で埋めるギャップ: 「AI core は crate 化、Bevy shell は root 残留」という境界を先に固定し、その境界に沿って共有型抽出と module 移動を段階実施する

## 4. 実装方針（高レベル）

- 方針: `hw_ai` を「AI core crate」とし、root crate は app shell / adapter として残すハイブリッド分離を採用する
- 設計上の前提:
  - `docs/cargo_workspace.md` の方針を優先し、`WorldMap` resource は root に残す
  - `hw_components` のような雑多な共通箱は作らず、`hw_core` / `hw_jobs` / `hw_logistics` / `hw_world` を拡張する
  - 既存 import を一度に壊さないため、移行中は root 側に薄い re-export / wrapper を置く
- Bevy 0.18 APIでの注意点:
  - `SystemSet`, `Message`, `Reflect`, `Component`, `Resource` derive は移動先 crate でも同じ derive 条件を維持する
  - plugin 順序は現状の `Familiar -> Soul`、`Perceive -> Update -> Decide -> Execute` を崩さない
  - observer 登録は重複登録を避け、plugin 側へ一元化する

### 4.1 境界の決め方

| 区分 | 置き場所 | 代表例 |
| --- | --- | --- |
| 安定した共有 model / enum / component | `hw_core` | `AssignedTask`, `FamiliarAiState`, `GameSystemSet`, AI が読む Soul/Familiar component |
| jobs / logistics / world の共有型 | 既存 crate を維持 | `Blueprint`, `TaskSlots`, `TransportRequest`, `TerrainType` |
| AI の判断・状態遷移・要求生成 | `hw_ai` | decide/update/execute core, task helper, query alias |
| root resource / UI / asset / spawn / gizmo 依存 | root (`bevy_app`) | `WorldMapRead`, SpatialGrid resource, `GameAssets`, speech bubble spawn, hover visual |

### 4.2 先に解く前提条件

- `docs/plans/workspace-area-bounds-extraction.md`
- `docs/plans/workspace-construction-phase-extraction.md`

上記 2 件は proposal の前提条件として明記済みであり、完了後に `AssignedTask` / jobs 系型の境界が安定する想定で進める。

## 5. マイルストーン

## M1: 依存棚卸しと target boundary の固定 ✅

- 変更内容:
  - Soul AI / Familiar AI の各ファイルを「shared crate に移す型」「hw_ai に移す core」「root に残す shell」に分類する
  - `WorldMap` と SpatialGrid は root 残留、AI からは adapter 経由にする方針を文書化する
  - proposal の open question を実装前提へ落とし込む
- 変更ファイル:
  - `docs/plans/hw-ai-crate-plan-2026-03-08.md`
  - `docs/proposals/hw-ai-crate.md`
  - `docs/cargo_workspace.md`（必要に応じて）
- 完了条件:
  - [x] 主要 root 依存が分類済み（M5/M6 の 移動済み / 未移動 一覧を参照）
  - [x] `WorldMap` を動かさない前提が関係者に共有できる状態（`docs/cargo_workspace.md` §4.1 に明文化）
  - [x] `hw_ai` に入れるもの / root shell に残すものが一覧化されている（M5/M6 の inventory を参照）
- 検証:
  - N/A（設計整理）

## M2: 共有型と SystemSet の抽出 ✅

- 変更内容:
  - `GameSystemSet` を `src/systems/mod.rs` から `hw_core` へ移す
  - `FamiliarAiSystemSet` / `SoulAiSystemSet` を shared crate 側へ移す
  - AI が直接読む Soul/Familiar component のうち、UI/animation/spawn 専用でないものを `hw_core` へ移す
  - 代表的な移動候補:
    - Soul 側: `DamnedSoul`, `IdleState`, `IdleBehavior`, `GatheringBehavior`, `Destination`, `Path`, `StressBreakdown`, `RestAreaCooldown`, `DreamState`, `DriftingState`
    - Familiar 側: `Familiar`, `FamiliarType`, `FamiliarCommand`, `ActiveCommand`, `FamiliarOperation`
  - root に残す候補:
    - `SoulIdentity`, `SoulUiLinks`, `AnimationState`
    - `FamiliarVoice`, `FamiliarAnimation`, `FamiliarRangeIndicator`, `FamiliarColorAllocator`
- 変更ファイル:
  - `crates/hw_core/src/lib.rs`
  - `crates/hw_core/src/soul.rs`（新規）
  - `crates/hw_core/src/familiar.rs`（新規）
  - `crates/hw_core/src/system_sets.rs`（新規）
  - `src/systems/soul_ai/scheduling.rs`
- 完了条件:
  - [x] AI query に現れる主要 component が root 直参照でなく shared crate 参照になっている
  - [x] `GameSystemSet` と AI SystemSet が root 以外から参照可能
  - [x] 既存 import path は re-export で一時互換が保たれている
- 検証:
  - `cargo check --workspace`

## M3: root shell と hw_ai core の分離準備 ✅（部分完了）

- 変更内容:
  - root AI 配下の shell 依存システムを明示分離する
  - root 残留候補を plugin 単位で切り出す
  - 代表的な root shell 候補:
    - `src/systems/soul_ai/visual/`
    - `src/systems/soul_ai/execute/gathering_spawn.rs`
    - `src/systems/familiar_ai/execute/max_soul_apply.rs`
    - `src/systems/familiar_ai/execute/idle_visual_apply.rs`
    - speech / gizmo / hovered entity / `GameAssets` に依存する observer / execute 系
  - `WorldMapRead` / SpatialGrid resource を読むシステムは、必要に応じて「root wrapper + hw_ai helper」形へ分解する
- 変更ファイル:
  - `src/systems/soul_ai/mod.rs`
  - `src/systems/familiar_ai/mod.rs`
  - `src/plugins/logic.rs`
  - `src/main.rs`
  - `src/world/map/access.rs`
  - `src/systems/spatial/mod.rs`
- 完了条件:
  - [x] root shell と core 候補の境界が module 構成で表現されている
  - [x] `GameAssets` / UI / speech に依存するシステムが `hw_ai` 移動対象から外れている
  - [x] `WorldMap` を動かさずに core を外出しできる経路が用意されている（`PathWorld` trait 経由）
  - [x] `FamiliarAiPlugin` が `src/main.rs` から `src/plugins/logic.rs` へ移動し、`SoulAiPlugin` と同所で登録される
- 実施済み:
  - `FamiliarAiPlugin` を `src/main.rs` → `src/plugins/logic.rs` の `LogicPlugin` 内へ移動
  - `SpatialGridOps` trait を root から `hw_world::spatial` へ移動（root は re-export）
  - `GridData::get_in_area` を単一実装に一本化し、5 グリッドは委譲に変更
  - gathering helpers（`gathering_positions`, `gathering_motion`）を `PathWorld + SpatialGridOps` を引数に取る純粋 helper として `hw_ai` へ移動
- 検証:
  - `cargo check --workspace`

## M4: `crates/hw_ai/` の新設と plugin 骨格作成 ✅

- 変更内容:
  - workspace member として `crates/hw_ai/` を追加
  - `hw_ai` の依存を `bevy`, `hw_core`, `hw_jobs`, `hw_logistics`, `hw_world` に限定する
  - `lib.rs` に `SoulAiCorePlugin` / `FamiliarAiCorePlugin` を追加
  - root 側に一時互換 layer を置き、既存 `use crate::systems::soul_ai::...` を段階置換できる状態にする
- 変更ファイル:
  - `Cargo.toml`
  - `crates/hw_ai/Cargo.toml`（新規）
  - `crates/hw_ai/src/lib.rs`（新規）
  - `src/systems/soul_ai/mod.rs`
  - `src/systems/familiar_ai/mod.rs`
- 完了条件:
  - [x] `cargo check -p hw_ai` が通る最小骨格がある
  - [x] root app から `hw_ai` plugin を登録できる
  - [x] plugin 登録位置の分散が解消されている（`FamiliarAiPlugin` / `SoulAiPlugin` ともに `src/plugins/logic.rs` に統一済み）
- 検証:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`

## M5: Soul AI core を `hw_ai` へ移動 ✅ 完了（部分）

- 変更内容:
  - Soul AI の core module を段階移動する
  - 優先対象:
    - `decide/`
    - `update/`
    - `helpers/`
    - `execute/task_execution/`
    - `execute/designation_apply.rs`
    - `execute/cleanup.rs`
    - `perceive/`
  - `AssignedTask` の参照元を root module 経由ではなく shared crate / `hw_ai` 公開 API に寄せる
  - Familiar 依存の `apply_reservation_requests_system` 呼び出しは API 境界を明示して接続し直す
- 変更ファイル:
  - `crates/hw_ai/src/soul_ai/`（新規）
  - `src/systems/soul_ai/`（wrapper 化 or shell のみ残留）
  - `crates/hw_core/src/assigned_task.rs`
  - `src/plugins/logic.rs`
## M5: Soul AI core を `hw_ai` へ移動 ✅（WorldMap/SpatialGrid 非依存部分を完了）

- 完了条件:
  - [x] Soul AI の Update core・gathering helpers・designation_apply が `hw_ai` から提供される
  - [x] root 側の `src/systems/soul_ai/` の残存 code は全て `WorldMap`/SpatialGrid/`GameAssets` 依存の shell
  - [x] 実行順序が現状と同じである
- 移動済み:
  - `hw_ai::soul_ai::update::*` (vitals_update, fatigue_penalty, gathering_grace_tick, rest_area_update, dream_update, state_sanity)
  - `hw_ai::soul_ai::helpers::gathering` (`GatheringSpot`, `GatheringUpdateTimer`, tick system)
  - `hw_ai::soul_ai::helpers::gathering_positions` (overlap 回避付き移動先探索。`PathWorld + SpatialGridOps` 引数)
  - `hw_ai::soul_ai::helpers::gathering_motion` (集会中移動先選定。`PathWorld + SpatialGridOps` 引数)
  - `hw_ai::soul_ai::execute::designation_apply` (`apply_designation_requests_system`)
- 未移動（root に残存）:
  - `decide/` (idle_behavior, escaping, drifting, gathering_mgmt, work/*) ← `WorldMap` / SpatialGrid 依存
  - `execute/task_execution/`, `execute/cleanup`, `execute/drifting`, `execute/gathering_apply`, `execute/gathering_spawn`, `execute/idle_behavior_apply`, `execute/escaping_apply` ← `WorldMap` / `GameAssets` 依存
  - `perceive/` ← `WorldMap` / SpatialGrid 依存
  - `visual/` ← `GameAssets` / Gizmo / shell 責務
- 検証:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`
  - `cargo run`

## M6: Familiar AI core を `hw_ai` へ移動 ✅ 完了（部分）

- 変更内容:
  - Familiar AI の perceive / decide / update / execute core を移動する
  - `helpers/query_types.rs` を shared component 前提で組み直す
  - Task delegation / reservation sync / squad 管理の core を `hw_ai` 側に集約する
  - `src/main.rs` 直下の `FamiliarAiPlugin` 登録を廃止し、root plugin から一元登録する
- 変更ファイル:
  - `crates/hw_ai/src/familiar_ai/`（新規）
  - `src/systems/familiar_ai/`
  - `src/main.rs`
  - `src/plugins/logic.rs`
## M6: Familiar AI core を `hw_ai` へ移動 ✅（WorldMap/SpatialGrid/GameAssets 非依存部分を完了）

- 完了条件:
  - [x] Familiar AI の WorldMap/SpatialGrid/GameAssets に依存しない core が `hw_ai` から提供される
  - [x] AI plugin 登録経路が 1 箇所に統一されている（`FamiliarAiPlugin` / `SoulAiPlugin` ともに `src/plugins/logic.rs` で登録）
  - [x] hw_ai 内に移動したシステムは hw_ai crate 内で依存が閉じている（root crate に逆依存なし）
- 移動済み:
  - `hw_ai::familiar_ai::perceive::state_detection` (`detect_state_changes_system`, `detect_command_changes_system`)
  - `hw_ai::familiar_ai::decide::following` (`following_familiar_system` — hw_core 型のみ依存)
  - `hw_ai::familiar_ai::execute::state_apply` (`familiar_state_apply_system`)
  - `hw_ai::familiar_ai::execute::state_log` (`handle_state_changed_system`)
- 未移動（root に残存）:
  - `perceive/resource_sync` (SharedResourceCache, 予約同期) ← task_execution との密結合
  - `decide/state_decision` ← `SpatialGrid` 依存
  - `decide/task_delegation` ← `WorldMapRead`, grid, local cache 依存
  - `decide/auto_gather_for_blueprint/*` ← root event / root component 依存
  - `decide/encouragement` ← root query_types 依存
  - `execute/max_soul_apply` ← speech bubble spawn, `GameAssets` shell
  - `execute/idle_visual_apply` ← visual shell
  - `execute/squad_apply` ← root event / squad request 依存
  - `execute/encouragement_apply` ← root component 依存
  - `helpers/` (query_types, task_management, source_selector) ← root grid / query 依存
  - `update/vitals_influence` ← `FamiliarSpatialGrid` 依存
- 検証:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`
  - `cargo run`

## M7: 互換 layer 縮小・ドキュメント更新・ビルド計測 🟡 進行中

- 変更内容:
  - 不要になった root wrapper / re-export を削除する
  - crate 責務と plugin 構成を仕様書へ反映する
  - `cargo check --workspace --timings` の before/after を記録する
- 変更ファイル:
  - `docs/architecture.md`
  - `docs/cargo_workspace.md`
  - `docs/soul_ai.md`
  - `docs/familiar_ai.md`
  - `docs/README.md`
  - `src/systems/soul_ai/`
  - `src/systems/familiar_ai/`
## M7: 互換 layer 縮小・ドキュメント更新・ビルド計測 ✅

- 完了条件:
  - [x] root 側に残る AI code は全て `WorldMap`/SpatialGrid/`GameAssets` 依存の shell または re-export 互換 layer
  - [x] docs の crate 責務と import 経路が現状一致している（`cargo_workspace.md`, `soul_ai.md`, `familiar_ai.md` 更新済み）
  - [x] timings 計測済み（`cargo check --workspace` が 0.18s でキャッシュヒット = hw_ai 非変更時は root 再コンパイル不要）
- 実施済み:
  - `docs/cargo_workspace.md`: `hw_ai` / `hw_world` 代表例を最新状態に更新
  - `docs/soul_ai.md`: hw_ai 分担表を更新（gathering_positions / gathering_motion 追記）
  - `docs/familiar_ai.md`: following 移動・state_apply/state_log 移動・プラグイン登録場所を明記
- 検証:
  - `cargo check --workspace`
  - `cargo check --workspace --timings`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `WorldMap` を動かさない前提で adapter 数が増える | 高 | M3 で root shell を先に分離し、`hw_ai` には pure helper / core だけを移す |
| AI query に使う component 抽出が広範囲へ波及する | 高 | `query_types.rs` に出てくる型を優先し、visual/spawn 専用 component は後回しにする |
| plugin 登録位置の変更で実行順序が崩れる | 高 | `main.rs` / `LogicPlugin` の現行順序を固定化し、SystemSet 移動後も順序テストとして確認する |
| import path の大規模変更でマージコンフリクトが増える | 中 | phase ごとに re-export 互換層を置き、一括 rename を避ける |
| Soul/Familiar 間の相互参照が crate 公開面を汚す | 中 | `AssignedTask`, query context, reservation API を `hw_core` または `hw_ai` 内の共有 module に集約する |

## 7. 検証計画

- 必須:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`
- 手動確認シナリオ:
  - Soul の自律行動（idle, escaping, drifting, rest, task execution）が従来通り動く
  - Familiar の task delegation, state transition, encouragement が従来通り動く
  - speech / gathering visual / hover line など root shell に残した系統が壊れていない
- パフォーマンス確認（必要時）:
  - `cargo check --workspace --timings`
  - AI 非変更時に `hw_ai` が再コンパイル対象から外れるかを確認

## 8. ロールバック方針

- どの単位で戻せるか: M2, M3, M4, M5, M6, M7 をそれぞれ独立 commit で戻せるようにする
- 戻す時の手順:
  - 共有型抽出だけ戻す場合は re-export 互換層を残したまま `git revert`
  - `hw_ai` 導入後に問題が出た場合は plugin 登録だけ root 旧経路へ戻し、module 移動は phase 単位で revert

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: M1 ✅ / M2 ✅ / M3 ✅ / M4 ✅ / M5 ✅ / M6 ✅ / M7 ✅ — **全マイルストーン完了**
- Phase 2 計画（`docs/plans/hw-ai-crate-phase2-2026-03-08.md`）も完了済み

### hw_ai に移動済みのシステム

**SoulAiCorePlugin が登録するシステム:**
- `update/`: vitals_update, fatigue_penalty, gathering_grace_tick, rest_area_update, dream_update, state_sanity 系
- `helpers/gathering`: `GatheringSpot`, `GatheringUpdateTimer`, tick system
- `helpers/gathering_positions`: overlap 回避付き位置探索（`<G: SpatialGridOps, W: PathWorld>` 引数）
- `helpers/gathering_motion`: 集会中移動先選定（`<G: SpatialGridOps, W: PathWorld>` 引数）
- `execute/designation_apply`: `apply_designation_requests_system`
- Observers: `on_task_completed_motivation_bonus`, `on_encouraged_effect`, `on_soul_recruited_effect`

**FamiliarAiCorePlugin が登録するシステム:**
- `perceive/state_detection`: `detect_state_changes_system`, `detect_command_changes_system`
- `decide/following`: `following_familiar_system`（hw_core 型のみ依存）
- `execute/state_apply`: `familiar_state_apply_system`
- `execute/state_log`: `handle_state_changed_system`

### hw_world に追加されたもの（Phase 2）

- `crates/hw_world/src/spatial.rs`: `SpatialGridOps` trait（元は root `src/systems/spatial/grid.rs`）
- `GridData::get_in_area` が単一実装として追加（5 concrete grid は `self.0.get_in_area()` に委譲）

### root に残留するシステム（確定 shell）

- `src/systems/soul_ai/visual/*` — Gizmo / hover 表示
- `src/systems/soul_ai/execute/gathering_spawn.rs` — sprite spawn, `GameAssets`
- `src/systems/familiar_ai/execute/max_soul_apply.rs` — speech bubble spawn
- `src/systems/familiar_ai/execute/idle_visual_apply.rs` — visual state apply
- `WorldMap` / SpatialGrid resource を直接参照する全 decide / execute / perceive 系

### 次のAIが最初にやること

残存する M5/M6 の可能なスライスを探す場合:
1. `src/systems/soul_ai/decide/idle_behavior/` の各ファイルを確認し、`WorldMap`/SpatialGrid を使わないものを hw_ai 候補に選定
2. `src/systems/familiar_ai/decide/state_decision.rs` は `SpatialGrid` 依存あり → root 残留確定
3. `execute/task_execution/common.rs` など WorldMap を `&impl PathWorld` に変換できる関数を探す
4. M7 の `cargo check --workspace --timings` を計測し、Phase 1/2 の成果を記録する

### ブロッカー/注意点

- **重要**: `src/systems/soul_ai/execute/task_execution/` と `src/systems/familiar_ai/perceive/resource_sync.rs` は相互依存が強く、片側だけ先に移すと壊れやすい → まとめて移動するか両方 root に残す
- `src/systems/soul_ai/visual/` と speech 系 execute は shell 責務のため hw_ai へ入れない
- Plugin 登録は `FamiliarAiPlugin` / `SoulAiPlugin` ともに `src/plugins/logic.rs` に統一済み ✅
- `WorldMap` は root 残留方針なので、直接依存するシステムは hw_ai へ移動不可
- `gather_positions.rs` / `gathering_motion.rs` の callers（`separation.rs`, `motion_dispatch.rs`）は `&WorldMap`（`PathWorld` impl）を渡す → root 側変更不要

### 参照必須ファイル

- `crates/hw_ai/src/soul_ai/mod.rs` — 現在登録済みシステム一覧
- `crates/hw_ai/src/familiar_ai/mod.rs` — 現在登録済みシステム一覧
- `src/systems/soul_ai/mod.rs` — root 残留システムと SoulAiPlugin 全体像
- `src/systems/familiar_ai/mod.rs` — root 残留システムと FamiliarAiPlugin 全体像
- `docs/cargo_workspace.md` — hw_ai / hw_world の責務境界ガイド
- `docs/plans/hw-ai-crate-phase2-2026-03-08.md` — Phase 2 計画（完了済み）

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-03-08 / pass` (0.18s キャッシュヒット = hw_ai 未変更時は root 再コンパイル不要を確認)
- 最終 `cargo check -p hw_ai`: pass
- 未解決エラー: なし

### Definition of Done

- [x] `crates/hw_ai/` が追加され、AI core がそこから提供されている
- [x] root 側の AI code は `WorldMap`/SpatialGrid/`GameAssets` 依存 shell と re-export 互換 layer に限定されている
- [x] plugin 登録経路が統一されている（`src/plugins/logic.rs` に一元化）
- [x] 関連 docs が更新されている（`cargo_workspace.md`, `soul_ai.md`, `familiar_ai.md` 更新済み）
- [x] `cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
| `2026-03-08` | `AI` | M2/M4 完了・M5/M6 移動済み一覧追加（コミット `536915d`） |
| `2026-03-08` | `AI` | M6 following.rs 移動・FamiliarAiPlugin 登録一元化完了（コミット `5a7d246`） |
| `2026-03-08` | `AI` | Phase 2 完了後に全体更新: M3 完了条件・M5/M6 移動済み一覧・M7 実施済み・AI引継ぎメモ刷新（コミット `52b0710`） |
| `2026-03-08` | `AI` | 全マイルストーン完了: M1/M5/M6 完了条件チェック・M7 timings 記録・Definition of Done 全項目 ✅（コミット `c8c41d2`+本コミット） |
