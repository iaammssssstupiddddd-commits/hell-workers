# familiar_ai — Familiar（使い魔）AI 意思決定システム

## 役割

`Familiar` エンティティの自律的な監督・タスク委譲・スカウト・リクルートを実装する。
基本的な AI ロジックの一部は `hw_familiar_ai::familiar_ai` に定義されており、このディレクトリは
**root adapter / wrapper / orchestration / visual apply** を担う。

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
| `state_handlers/` | `hw_familiar_ai` 実装を公開する薄い re-export |
| `task_management/` | `hw_familiar_ai` 実装を公開する thin bridge |
| `auto_gather_for_blueprint/` | root helper / thin re-export 群。`actions.rs` と `is_reachable` は root、純計画層は `hw_familiar_ai` |
| `auto_gather_for_blueprint.rs` | `Commands` / pathfinding を持つ orchestration entrypoint |
| `encouragement.rs` | `hw_familiar_ai` の対象選定を呼ぶ request 出力 adapter |
| `familiar_processor.rs` | recruitment / task_delegation と `hw_familiar_ai` helper の橋渡しを行う root adapter |
| `following` | `mod.rs` inline module から `hw_familiar_ai` 実装を公開する薄い re-export |
| `recruitment` | `mod.rs` inline module から `hw_familiar_ai` 実装を公開する薄い re-export |
| `scouting` | `mod.rs` inline module から `hw_familiar_ai` 実装を公開する薄い re-export |
| `squad` | `mod.rs` inline module から `hw_familiar_ai` 実装を公開する薄い re-export |
| `state_decision.rs` | concrete `SpatialGrid` / `transmute_lens_filtered` / request 出力を持つ状態遷移 adapter。branch dispatch と pure result 型の実体は `hw_familiar_ai` |
| `supervising` | `mod.rs` inline module から `hw_familiar_ai` 実装を公開する薄い re-export |
| `task_delegation.rs` | `WorldMapRead` / concrete SpatialGrid / pathfinding を束ねる root wrapper |

### task_management/ ディレクトリ

root 側の `task_management/` は
[mod.rs](/home/satotakumi/projects/hell-workers/crates/bevy_app/src/systems/familiar_ai/decide/mod.rs)
の inline module として thin bridge を残し、実装本体は
`hw_familiar_ai::familiar_ai::decide::task_management` にある。

## execute/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `encouragement_apply.rs` | 激励の適用 |
| `idle_visual_apply.rs` | アイドルビジュアル状態の適用 |
| `max_soul_apply.rs` | 最大Soul数制限の適用 |
| `squad_apply.rs` | 配下管理の適用 |
| `state_apply` | `mod.rs` inline module から `hw_familiar_ai` 実装を公開する薄い re-export |
| `state_log` | `mod.rs` inline module から `hw_familiar_ai` 実装を公開する薄い re-export |

## Familiar の状態

`Idle → SearchingTask → Scouting / Supervising`

各状態の詳細は `docs/familiar_ai.md` を参照。

---

## hw_familiar_ai との境界

Familiar AI は `hw_familiar_ai::familiar_ai` と `src/systems/familiar_ai` に分割されている。

### hw_familiar_ai に置かれているもの（純粋ロジック）

`hw_familiar_ai::FamiliarAiCorePlugin` が直接登録・管理するコアエントリ:

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
| `decide/state_decision` | `FamiliarDecisionPath` による branch dispatch と `FamiliarStateDecisionResult` のような pure result 型 |
| `decide/query_types.rs` | Familiar Decide 用の narrow query 定義 |
| `decide/helpers.rs` | `finalize_state_transitions` / `process_squad_management` |
| `decide/recruitment.rs` | `SpatialGridOps` ベースのリクルート選定・スカウト開始判定 |
| `decide/auto_gather_for_blueprint/{planning,demand,supply,helpers}` | Blueprint auto gather の純計画層 |
| `decide/squad.rs` / `scouting.rs` / `supervising.rs` | 分隊管理・スカウト・監視の純ロジック |
| `decide/state_handlers/` | 状態別ハンドラー |

### src/ に置かれているもの（root shell / ゲーム固有）

`src/systems/familiar_ai` が追加するシステム群:

| モジュール | 理由 |
|---|---|
| `perceive/resource_sync.rs` | ゲーム固有リソース予約の同期。`sync_reservations_system` と `ReservationSyncTimer` を持ち、`SharedResourceCache` は `hw_logistics` から re-export |
| `decide/task_delegation.rs` | タスク検索・割り当て wrapper。`WorldMapRead` / concrete SpatialGrid / `PathfindingContext` / `ConstructionSiteAccess` / perf metrics を束ねる |
| `decide/task_management` | `hw_familiar_ai::familiar_ai::decide::task_management` への thin bridge。root 側は inline module で互換 path を維持 |
| `decide/familiar_processor.rs` | `FamiliarDelegationContext` が `WorldMap` / `PathfindingContext` / `transmute_lens_filtered` を直接扱う root adapter |
| `decide/auto_gather_for_blueprint.rs` | `Commands` / pathfinding / Blueprint 直接 query を束ねる orchestration |
| `decide/auto_gather_for_blueprint/actions.rs` / `helpers.rs` | designation 更新と `is_reachable` を担当する root helper |
| `decide/encouragement.rs` | `Time` / concrete `SpatialGrid` / request message 出力の adapter |
| `decide/state_decision.rs` | concrete `SpatialGrid` / `transmute_lens_filtered` / request message 出力の adapter。branch dispatch と pure result 型の実体は `hw_familiar_ai` |
| `helpers/query_types.rs` | `FamiliarSoulQuery` / `FamiliarStateQuery` / `FamiliarTaskQuery` を集約する root full-fat query bridge |
| `execute/squad_apply.rs` | `ManagedBy` Relationship の生成・削除 |

### プラグイン構成の二層構造

```rust
// src/systems/familiar_ai/mod.rs
impl Plugin for FamiliarAiPlugin {
    fn build(&self, app: &mut App) {
        // hw_familiar_ai のコアシステムを登録
        app.add_plugins(hw_familiar_ai::FamiliarAiCorePlugin);
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

## 設計メモ

- root 側は「thin shell 以上の意味を持つ adapter / wrapper / orchestration」だけを保持する
- `hw_familiar_ai` 側には shared crate 型と Bevy 汎用 API だけで閉じる pure core を置く
- `WorldMapRead` / concrete `SpatialGrid` / `PathfindingContext` / root `MessageWriter` / designation cleanup の最終適用を要求するものは root に残す
