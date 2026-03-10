# familiar_ai — Familiar（使い魔）AI 意思決定システム

## 役割

`Familiar` エンティティの自律的な監督・タスク委譲・スカウト・リクルートを実装する。
基本的な AI ロジックの一部は `hw_ai::familiar_ai` に定義されており、このディレクトリは**ゲーム固有のロジック**を担う。

## ディレクトリ構成

| ディレクトリ | フェーズ | 内容 |
|---|---|---|
| `perceive/` | Perceive | 環境情報の読み取り |
| `update/` | Update | 時間経過による内部状態更新 |
| `decide/` | Decide | 次行動の選択・リクエスト生成 |
| `execute/` | Execute | 決定された行動の実行 |
| `helpers/` | 共通 | 共有クエリ・ユーティリティ |

## decide/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `state_handlers/` | `hw_ai` 実装を公開する薄い re-export |
| `task_management/` | タスク検索・割り当てコア（下表） |
| `auto_gather_for_blueprint/` | root helper / thin re-export 群。`actions.rs` と `is_reachable` は root、純計画層は `hw_ai` |
| `auto_gather_for_blueprint.rs` | `Commands` / pathfinding を持つ orchestration entrypoint |
| `encouragement.rs` | `hw_ai` の対象選定を呼ぶ request 出力 adapter |
| `familiar_processor.rs` | recruitment / task_delegation と `hw_ai` helper の橋渡し |
| `following.rs` | ターゲット追跡移動 |
| `recruitment.rs` | `hw_ai` 実装を公開する薄い re-export |
| `scouting.rs` | `hw_ai` 実装を公開する薄い re-export |
| `squad.rs` | `hw_ai` 実装を公開する薄い re-export |
| `state_decision.rs` | concrete `SpatialGrid` / request 出力を持つ状態遷移 adapter |
| `supervising.rs` | `hw_ai` 実装を公開する薄い re-export |
| `task_delegation.rs` | タスク委譲エントリポイント |

### task_management/ ディレクトリ

タスク検索・割り当ての中核サブシステム。

| ファイル/ディレクトリ | 内容 |
|---|---|
| `task_finder/` | タスク候補収集・スコアリング |
| `builders/` | タスク割り当てリクエストのビルダー |
| `delegation/` | `TaskManager` — タスク委譲の実行 |
| `policy/` | タスク選択ポリシー（ソースセレクタ） |
| `validator/` | 割り当て可能性のバリデーション |
| `task_assigner.rs` | `assign_task_to_worker` — コア割り当て関数 |

## execute/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `encouragement_apply.rs` | 激励の適用 |
| `idle_visual_apply.rs` | アイドルビジュアル状態の適用 |
| `max_soul_apply.rs` | 最大Soul数制限の適用 |
| `squad_apply.rs` | 配下管理の適用 |
| `state_apply.rs` | 状態遷移の適用 |
| `state_log.rs` | 状態変化イベントのログ |

## Familiar の状態

`Idle → SearchingTask → Scouting / Supervising`

各状態の詳細は `docs/familiar_ai.md` を参照。

---

## hw_ai との境界

Familiar AI は `hw_ai::familiar_ai` と `src/systems/familiar_ai` に分割されている。

### hw_ai に置かれているもの（純粋ロジック）

`hw_ai::FamiliarAiCorePlugin` が直接登録・管理するコアエントリ:

| モジュール | 内容 |
|---|---|
| `perceive/state_detection.rs` | Change Detection による状態変化検出 |
| `decide/following.rs` | ターゲット追跡移動（純粋ベクトル計算） |
| `execute/state_apply.rs` | 状態遷移の適用 |
| `execute/state_log.rs` | 状態変化イベントのログ |
| `decide/encouragement.rs` | `EncouragementCooldown` の type registration と激励対象選定 helper |

root adapter から呼ばれる pure logic / helper:

| モジュール | 内容 |
|---|---|
| `decide/query_types.rs` | Familiar Decide 用の narrow query 定義 |
| `decide/helpers.rs` | `finalize_state_transitions` / `process_squad_management` |
| `decide/recruitment.rs` | `SpatialGridOps` ベースのリクルート選定・スカウト開始判定 |
| `decide/auto_gather_for_blueprint/{planning,demand,supply,helpers}` | Blueprint auto gather の純計画層 |
| `decide/squad.rs` / `scouting.rs` / `supervising.rs` | 分隊管理・スカウト・監視の純ロジック |
| `decide/state_handlers/` | 状態別ハンドラー |

### src/ に置かれているもの（ゲーム固有）

`src/systems/familiar_ai` が追加するシステム群:

| モジュール | 理由 |
|---|---|
| `perceive/resource_sync.rs` | ゲーム固有リソース予約の同期。`sync_reservations_system` と `ReservationSyncTimer` を持ち、`SharedResourceCache` は `hw_logistics` から re-export |
| `decide/task_delegation.rs` | タスク検索・割り当て（空間グリッド・`WorldMap` 参照） |
| `decide/task_management/` | スコアリング・バリデーター・ポリシー（全クエリがゲーム固有） |
| `decide/auto_gather_for_blueprint.rs` | `Commands` / pathfinding / Blueprint 直接 query を束ねる orchestration |
| `decide/auto_gather_for_blueprint/actions.rs` / `helpers.rs` | designation 更新と `is_reachable` を担当する root helper |
| `decide/encouragement.rs` | `Time` / concrete `SpatialGrid` / request message 出力の adapter |
| `decide/state_decision.rs` | concrete `SpatialGrid` と request message 出力の adapter |
| `execute/squad_apply.rs` | `ManagedBy` Relationship の生成・削除 |

### プラグイン構成の二層構造

```rust
// src/systems/familiar_ai/mod.rs
impl Plugin for FamiliarAiPlugin {
    fn build(&self, app: &mut App) {
        // hw_ai のコアシステムを登録
        app.add_plugins(hw_ai::FamiliarAiCorePlugin);
        // src/ 固有のシステムを追加登録
        app.add_systems(Update, (
            perceive::resource_sync::sync_reservations_system, // src/ 固有
            decide::task_delegation::familiar_task_delegation_system, // src/ 固有
            // ...
        ));
    }
}
```

`ResourceReservationRequest` の message 登録は `MessagesPlugin`、`SharedResourceCache` の `init_resource` は `FamiliarAiPlugin` が担当する。`hw_logistics::apply_reservation_requests_system` はその app shell 初期化を前提に動作する。
