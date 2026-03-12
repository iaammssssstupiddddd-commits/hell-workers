# hw_ai — Familiar AI・Soul AI 意思決定システム

## 役割

Familiar（監督役）と Soul（労働者）の自律的な意思決定・行動を実装するクレート。
全ての AI 処理は `Logic` フェーズ内の **Perceive → Update → Decide → Execute** サイクルで実行される。

## ディレクトリ構成

```
hw_ai/src/
├── lib.rs               FamiliarAiCorePlugin, SoulAiCorePlugin の公開
├── familiar_ai/         Familiar（監督）の AI
└── soul_ai/             Soul（労働者）の AI
```

### familiar_ai/ ディレクトリ

| ディレクトリ/ファイル | フェーズ | 内容 |
|---|---|---|
| `perceive/state_detection.rs` | Perceive | Change Detection で状態変化を検出 |
| `decide/following.rs` | Decide | Familiar のターゲット追跡移動 |
| `decide/query_types.rs` | Decide | Familiar Decide 用の narrow query 定義 |
| `decide/helpers.rs` | Decide | `finalize_state_transitions` / `process_squad_management` などの pure helper |
| `decide/recruitment.rs` | Decide | `SpatialGridOps` ベースのリクルート判定 |
| `decide/state_decision.rs` | Decide | Familiar の state decision dispatch (`FamiliarDecisionPath`) と pure result 型 (`FamiliarStateDecisionResult`) |
| `decide/encouragement.rs` | Decide | 激励対象選定と `EncouragementCooldown` |
| `decide/auto_gather_for_blueprint/` | Decide | Blueprint auto gather の需要供給計画ヘルパ |
| `decide/squad.rs` | Decide | 分隊検証・疲労メンバー解放判定 |
| `decide/scouting.rs` | Decide | スカウト状態ロジックと `ScoutingOutcome` |
| `decide/supervising.rs` | Decide | 監視状態ロジック |
| `decide/state_handlers/` | Decide | Idle / Searching / Scouting / Supervising の状態ハンドラー |
| `execute/state_apply.rs` | Execute | 状態遷移の適用 |
| `execute/state_log.rs` | Execute | 状態変化イベントハンドリング |

Familiar の状態: `Idle / SearchingTask / Scouting / Supervising`

### soul_ai/ ディレクトリ

| ディレクトリ/ファイル | フェーズ | 内容 |
|---|---|---|
| `perceive/escaping.rs` | Perceive | 近くの Familiar による脅威検出 |
| `update/vitals_update.rs` | Update | 疲労・ストレスの時間経過蓄積 |
| `update/vitals_influence.rs` | Update | バイタルが行動に与える影響 |
| `update/gathering_tick.rs` | Update | 集会グレースピリアド管理 |
| `update/dream_update.rs` | Update | 夢蓄積システム |
| `update/rest_area_update.rs` | Update | 休憩場所追跡 |
| `update/state_sanity.rs` | Update | コンポーネント整合性チェック |
| `update/vitals.rs` | Update | バイタル変化の Observer ハンドラ |
| `decide/gathering_mgmt.rs` | Decide | 集会行動ロジック |
| `decide/separation.rs` | Decide | 過密回避 |
| `decide/escaping.rs` | Decide | 脱走判断（0.5 秒毎） |
| `decide/idle_behavior/` | Decide | アイドル状態機械（下表） |
| `decide/work/auto_refine.rs` | Decide | MudMixer の自動精製指定発行 |
| `decide/work/auto_build.rs` | Decide | 資材完了 Blueprint への自動割り当て |
| `execute/designation_apply.rs` | Execute | 作業指定の適用 |
| `execute/gathering_apply.rs` | Execute | 集会場所への移動実行 |
| `execute/gathering_spawn.rs` | Execute | 集会発生判定と `GatheringSpawnRequest` 発行 |
| `execute/task_assignment_apply.rs` | Execute | `TaskAssignmentRequest` の適用、idle 正規化、予約反映、`DeliveringTo` 付与 |
| `execute/idle_behavior_apply.rs` | Execute | アイドル行動の実行 |
| `execute/escaping_apply.rs` | Execute | 脱走移動の実行 |

### decide/idle_behavior/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | `idle_behavior_decision_system` 本体 |
| `transitions.rs` | アイドル状態遷移ロジック |
| `task_override.rs` | アイドルをタスクで上書き |
| `rest_decision.rs` | 休憩判断 |
| `rest_area.rs` | 休憩場所の選択 |
| `motion_dispatch.rs` | 移動先の選択 |
| `exhausted_gathering.rs` | 強制集会行動 |

### helpers/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `gathering.rs` | `hw_core::gathering` の互換 re-export と timer helper |
| `gathering_positions.rs` | 集会スポット探索 |
| `gathering_motion.rs` | 集会中の移動 |
| `work.rs` | 作業実行ヘルパー |
| `query_types.rs` | 共有クエリ定義 |

## Soul バイタルシステム

| バイタル | 範囲 | 増加 | 減少 |
|---|---|---|---|
| `fatigue` | 0.0–1.0 | 作業中 | 休憩・アイドル |
| `stress` | 0.0–1.0 | Familiar 監視下 | 自由行動・休憩 |
| `motivation` | 0.0–1.0 | — | — |
| `laziness` | 0.0–1.0 | 長時間アイドル | Familiar 監視 |
| `dream` | 0.0–100.0 | 睡眠 | 消費 |

ストレスが 1.0 に達すると `StressBreakdown`（1 秒フリーズ）が発生する。

## アイドル行動種別

`Wandering / Sitting / Sleeping / Gathering / ExhaustedGathering / Resting / GoingToRest / Escaping / Drifting`

## 設計上の注意

- **Decide** フェーズは pure outcome または shared request を生成するのみ。ECS 状態の直接変更は **Execute** フェーズで行い、root-only context を要する adapter は `src/` 側に残す。
- `app.add_observer(...)` による一元登録を使い、スポーン時の `.observe(...)` による二重登録を避ける。
- crate へ移設済みの system は、この crate の Plugin を唯一の登録元にする。root 側の thin re-export は互換パスと ordering 参照のためだけに残し、同じ system function を再登録しない。

## 依存クレート

- `hw_core`, `hw_jobs`, `hw_logistics`, `hw_world`, `hw_spatial`, `bevy`, `rand`
- 全 hw_* クレートに依存する最上位クレート

---

## src/ との境界

`hw_ai` は **root-only 契約に依存しない shared AI / execute core** を提供する。
`src/systems/` は root wrapper・root facade・root adapter を所有し、app 側でしか確定できない契約を持つ。

### hw_ai に置かれているもの（純粋ロジック）

- **Soul AI**: バイタル更新・集会タイマー・状態整合・脱走判断・アイドル行動・分離行動
- **Soul AI Execute**: `gathering_spawn_logic_system` のような shared request 生成システム
- **Soul AI Execute**: `task_assignment_apply` のような shared 型だけで閉じる apply system とその helper 群
- **Soul AI Execute**: `task_execution/*` の core 実装・handler・query/context・`cleanup_task_assignment`
- **Familiar AI**: 状態変化検出・ターゲット追跡・状態機械・分隊管理・監視/スカウト判断・リクルート判定・激励対象選定
- **Familiar AI State Decision Core**: `determine_decision_path` と `FamiliarStateDecisionResult` のような root adapter 向けの pure dispatch / outcome 集約
- **Blueprint Auto Gather**: 需要供給集計・必要 designation 数の pure planning
- 純粋ヘルパー関数（`is_soul_available_for_work` 等）
- root adapter が request message を発行できるよう、`ScoutingOutcome` / `SquadManagementOutcome` のような pure outcome を返す
- `soul_ai::decide::work::{auto_refine, auto_build}` のように、shared 型だけで閉じる request 生成システムは `hw_ai` 側に置ける

### src/ に置かれているもの（root wrapper / facade / adapter）

| モジュール | hw_ai ではなく src/ にある理由 |
|---|---|
| `soul_ai/execute/task_execution/mod.rs` | `TaskExecutionSoulQuery` / `WorldMapRead` / `OnTaskCompleted` / root `unassign_task` を束ねる root wrapper |
| `soul_ai/execute/task_execution/{types,common,handler,move_plant}` | 互換 import path を維持する thin shell re-export |
| `soul_ai/execute/task_execution/context/mod.rs` | context/query 型を束ねる root facade |
| `soul_ai/execute/task_execution/transport_common/*` | root 側互換 helper と `hw_jobs::lifecycle` re-export |
| `soul_ai/execute/gathering_spawn.rs` | `GameAssets` を使う visual spawn と request 消費時の app 状態再検証を行う |
| `soul_ai/helpers/work.rs::unassign_task` | task解除の公開 facade。`OnTaskAbandoned` / `WorkingOn` を root 側で確定し、低レベル cleanup は `hw_ai::soul_ai::helpers::work::cleanup_task_assignment` へ委譲 |
| `familiar_ai/decide/task_delegation.rs` | 空間グリッド・`WorldMap` を参照 |
| `familiar_ai/decide/task_management/` | Familiar のタスク検索・割り当てコア。root 側は `WorldMap`/SpatialGrid orchestration と construction site bridge のみ保持 |
| `familiar_ai/decide/auto_gather_for_blueprint.rs` | `Commands` / pathfinding / Blueprint 直接 query に依存 |
| `familiar_ai/decide/encouragement.rs` | `Time` / concrete `SpatialGrid` / request message 出力の adapter を担当 |
| `familiar_ai/decide/state_decision.rs` | concrete `SpatialGrid` / `transmute_lens_filtered` / request message 出力を持つ root adapter。`hw_ai` 側は dispatch と pure result 型のみを所有 |
| `familiar_ai/perceive/resource_sync.rs` | `SharedResourceCache` 予約再構築のみ（apply helper は `hw_logistics` に移設済み） |

root 側の `src/systems/soul_ai/decide/work/*.rs` と `src/systems/soul_ai/decide/idle_behavior/mod.rs` は互換 re-export shell のみで、実体は `hw_ai::soul_ai::*` にある。`src/systems/soul_ai/execute/task_execution/mod.rs` の `apply_task_assignment_requests_system` も同様に re-export のみで、system 登録責務は `SoulAiCorePlugin` が持つ。`task_execution_system` だけが root wrapper system で、`unassign_task` は別の root facade として残る。

### 移設の判断基準

```
以下のいずれかを参照・変更するか？
  - root-only resource / wrapper（WorldMapRead/Write, PathfindingContext, concrete SpatialGrid, GameAssets, PopulationManager）
  - request 消費時の stale 再検証や relationship / event の最終確定
  - 互換 import path のための thin shell / root facade / root wrapper
    → YES: src/ に置く
shared crate 型 + Bevy 汎用 API だけで閉じるか？
    → YES: hw_ai に置ける（テスト・再利用が容易）
    → NO:  src/ に置く
```

### 共有型の置き場所

- `SoulTaskHandles`, `FadeOut`, `WheelbarrowMovement` は `hw_core::visual` に配置する。
- 理由:
  - `hw_ai` と `hw_visual` の両方から参照される
  - visual system が読むだけの shared resource / marker component であり、`hw_visual` 所有にすると `hw_ai -> hw_visual` 依存が復活する

### 用語

- thin shell: `pub use` のみを持つ互換モジュール
- root wrapper system: root-only query/resource/event を束ねて crate 実装を呼ぶ system
- root facade/helper: 公開 API や互換 helper を root が所有し、低レベル実装へ委譲する層
- root adapter: request 消費時の再検証や visual/UI/resource 依存を伴うゲーム側 system
