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
| `state_handlers/` | 状態別決定ロジック（`idle.rs`, `searching.rs`, `scouting.rs`, `supervising.rs`） |
| `task_management/` | タスク検索・割り当てコア（下表） |
| `auto_gather_for_blueprint/` | ブループリント向け自動採取 |
| `auto_gather_for_blueprint.rs` | エントリポイント |
| `encouragement.rs` | Soul への激励処理 |
| `familiar_processor.rs` | Familiar ごとの Decide メインループ |
| `following.rs` | ターゲット追跡移動 |
| `recruitment.rs` | Soul リクルート判断 |
| `scouting.rs` | スカウト行動 |
| `squad.rs` | 配下 Soul の管理 |
| `state_decision.rs` | 状態遷移の最終決定 |
| `supervising.rs` | 監督行動 |
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

`hw_ai::FamiliarAiCorePlugin` が登録するシステム群:

| モジュール | 内容 |
|---|---|
| `perceive/state_detection.rs` | Change Detection による状態変化検出 |
| `decide/following.rs` | ターゲット追跡移動（純粋ベクトル計算） |
| `execute/state_apply.rs` | 状態遷移の適用 |
| `execute/state_log.rs` | 状態変化イベントのログ |

### src/ に置かれているもの（ゲーム固有）

`src/systems/familiar_ai` が追加するシステム群:

| モジュール | 理由 |
|---|---|
| `perceive/resource_sync.rs` | ゲーム固有リソース予約の同期（`SharedResourceCache`） |
| `decide/task_delegation.rs` | タスク検索・割り当て（空間グリッド・`WorldMap` 参照） |
| `decide/task_management/` | スコアリング・バリデーター・ポリシー（全クエリがゲーム固有） |
| `decide/state_decision.rs` | `DesignationSpatialGrid` 等を参照した状態遷移決定 |
| `decide/auto_gather_for_blueprint/` | Blueprint エンティティへの直接クエリ |
| `decide/encouragement.rs` | Soul エンティティへの励ましコンポーネント付与 |
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
