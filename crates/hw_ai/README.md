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
| `execute/idle_behavior_apply.rs` | Execute | アイドル行動の実行 |
| `execute/escaping_apply.rs` | Execute | 脱走移動の実行 |

### decide/idle_behavior/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `transitions.rs` | アイドル状態遷移ロジック |
| `task_override.rs` | アイドルをタスクで上書き |
| `rest_decision.rs` | 休憩判断 |
| `rest_area.rs` | 休憩場所の選択 |
| `motion_dispatch.rs` | 移動先の選択 |
| `exhausted_gathering.rs` | 強制集会行動 |

### helpers/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `gathering.rs` | 集会場所・タイマー管理 |
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

## 依存クレート

- `hw_core`, `hw_jobs`, `hw_logistics`, `hw_world`, `hw_spatial`, `bevy`, `rand`
- 全 hw_* クレートに依存する最上位クレート

---

## src/ との境界

hw_ai は**ゲームエンティティ非依存の純粋 AI ロジック**のみを提供する。
ゲーム固有のタスク実行・エンティティ操作は `src/systems/` に実装する。

### hw_ai に置かれているもの（純粋ロジック）

- **Soul AI**: バイタル更新・集会タイマー・状態整合・脱走判断・アイドル行動・分離行動
- **Familiar AI**: 状態変化検出・ターゲット追跡・状態機械・分隊管理・監視/スカウト判断・リクルート判定・激励対象選定
- **Blueprint Auto Gather**: 需要供給集計・必要 designation 数の pure planning
- 純粋ヘルパー関数（`is_soul_available_for_work` 等）
- root adapter が request message を発行できるよう、`ScoutingOutcome` / `SquadManagementOutcome` のような pure outcome を返す
- `soul_ai::decide::work::{auto_refine, auto_build}` のように、shared 型だけで閉じる request 生成システムは `hw_ai` 側に置ける

### src/ に置かれているもの（ゲーム固有）

| モジュール | hw_ai ではなく src/ にある理由 |
|---|---|
| `soul_ai/execute/task_execution/` (23ファイル) | `WorldMap`・`Transform`・`Visibility`・ECS Relationship に依存 |
| `soul_ai/execute/drifting.rs` | `Path` 書き換え + 境界経路探索 |
| `soul_ai/execute/gathering_spawn.rs` | `GatheringSpot` エンティティをスポーン |
| `soul_ai/helpers/work.rs::unassign_task` | `WorldMap`・`Visibility` 操作あり |
| `familiar_ai/decide/task_delegation.rs` | 空間グリッド・`WorldMap` を参照 |
| `familiar_ai/decide/task_management/` | 全クエリがゲーム固有エンティティ |
| `familiar_ai/decide/auto_gather_for_blueprint.rs` | `Commands` / pathfinding / Blueprint 直接 query に依存 |
| `familiar_ai/decide/encouragement.rs` | `Time` / concrete `SpatialGrid` / request message 出力の adapter を担当 |
| `familiar_ai/decide/state_decision.rs` | concrete `SpatialGrid` と request message 出力の adapter を担当 |
| `familiar_ai/perceive/resource_sync.rs` | `SharedResourceCache` 予約再構築のみ（apply helper は `hw_logistics` に移設済み） |

root 側の `src/systems/soul_ai/decide/work/*.rs` は互換 re-export shell のみで、実体は `hw_ai::soul_ai::decide::work::*` にある。

### 移設の判断基準

```
以下のいずれかを参照・変更するか？
  - WorldMapRead/Write / PathfindingContext
  - concrete SpatialGrid resource
  - Commands / request message 出力 / Time / app shell wiring
  - speech / visual 専用型
  - soul_ai::task_execution に密結合な full-fat query
    → YES: src/ に置く
    → NO:  hw_ai に置ける（テスト・再利用が容易）
```
