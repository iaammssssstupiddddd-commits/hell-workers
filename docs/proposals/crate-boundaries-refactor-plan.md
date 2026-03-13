# クレート境界とコアロジック分離のリファクタリング提案

`docs/crate-boundaries.md` で定義された規則に基づき、現在 `bevy_app` に残存しているロジックを対象としたリファクタリング計画の提案です。

## 1. 目的

- 責務に合わない型やロジックを `bevy_app` から各リーフクレート（`hw_*`）へ移動する。
- 純粋なドメインロジック（思考・計算）と、副作用（ECSの操作）の分離を徹底する。
- 循環依存を防ぎ、テスト容易性と保守性の高いクリーンなアーキテクチャを実現する。

## 2. リファクタリング対象と移行計画

### 2.1. 型・ドメインモデルの移動

現在 `bevy_app` に存在するが、他クレートから参照されるべき、あるいはドメインに属する型を移動します。

| 対象の型 / モジュール | 現在の場所 | 移動先 | フェーズ | 理由 / 備考 |
| :--- | :--- | :--- | :---: | :--- |
| `GameTime` | `bevy_app/src/systems/time.rs` | `hw_core` | 1 | 複数のクレート（AI、植物の成長など）から参照される基盤的な状態。他の移動に依存しない。 |
| `DreamTreePlantingPlan` | `bevy_app/src/systems/dream_tree_planting.rs` | `hw_jobs` or `hw_world` | 1 | 植林の計画という純粋な計算・ドメインロジック。外部依存が薄い。 |
| `AreaEditSession` 等 | `bevy_app/src/systems/command/area_selection/` | `hw_ui` | 2 | ユーザーのUI操作状態（プレゼンテーション層）。`hw_ui` の既存型に依存。 |
| `Room`, `RoomTileLookup` | `bevy_app/src/systems/room/` | `hw_world` | 2 | `room_detection` のコアが既に `hw_world` に存在。ただし `bevy_app` 側の detection/validation システムの書き換えを伴うため、型移動だけでは完結しない。 |

### 2.2. AI 意思決定ロジックの抽出（純粋関数化）

`bevy_app` のシステムとして実装されているAIの意思決定（思考）部分を、副作用を持たない純粋関数として `hw_*` 側に抽出し、`bevy_app` 側はそれを呼び出すだけのアダプター（オーケストレーター）に作り変えます。

> **注:** Familiar AI については、先行リファクタ（`hw_familiar_ai` への pure core 移設）が完了済み。`state_decision` / `task_delegation` / `familiar_processor` の root 側ファイルは既にアダプター/オーケストレーターとして機能しており、残る作業は pure core のさらなる抽出と Plugin 登録の移譲。

| 対象システム | 現在の場所 | 移動先 | フェーズ | 理由 / 備考 |
| :--- | :--- | :--- | :---: | :--- |
| `state_decision_system` | `bevy_app/.../familiar_ai/decide/state_decision.rs` | `hw_familiar_ai` | 1→3 | pure core は移設済み。残りは root adapter から Plugin 登録を移譲するフェーズ3の作業。 |
| `familiar_processor_system` | `bevy_app/.../familiar_ai/decide/familiar_processor.rs` | `hw_familiar_ai` | 1→3 | 同上。`FamiliarDelegationContext` が `WorldMap` / `PathfindingContext` を直接保持するため root adapter として残留中。 |
| `task_delegation_system` | `bevy_app/.../familiar_ai/decide/task_delegation.rs` | `hw_familiar_ai` | 1→3 | 同上。concrete `SpatialGrid` / `WorldMapRead` / `PathfindingContext` を束ねる root wrapper。 |
| パスファインディング統括 | `bevy_app/.../damned_soul/movement/pathfinding/mod.rs` | `hw_soul_ai` | 2 | 魂の経路探索オーケストレーション（パス再利用・スタック脱出・バジェット管理）。コアアルゴリズムは `hw_world::pathfinding` に委譲。 |
| 建設完了判定 | `bevy_app/.../jobs/building_completion/world_update.rs` | `hw_jobs` | 2 | 建設完了時のワールド更新ロジック。`hw_jobs` の既存型のみに依存。 |

## 3. 実行手順のアプローチ

依存関係の制約（循環依存）を避けるため、以下の段階的な手順でリファクタリングを実施します。

### フェーズ 1: 依存なしで移動可能な純粋ロジック・基礎型の移動
- `Commands` や `bevy_app` 固有のリソース（`GameAssets` 等）に依存していない純粋な関数を、即座に `hw_familiar_ai` や `hw_soul_ai` などの適切なクレートへ移動します。
- `GameTime` 等の基礎型を `hw_core` へ移動します。
- `bevy_app` 側は移動した関数・型を `use` して参照するように修正します。

### フェーズ 2: Leaf crate 間の依存で動くシステム・型の移動
- `Room` 等の型と、パスファインディング統括・建設完了判定等のシステムを移動します。
- `Cargo.toml` の依存関係が解決済みであれば移動可能なものが対象です。

### フェーズ 3: リーフ内システムの Plugin 登録移行
- 他のリーフクレートにのみ依存し、`bevy_app` 固有の型に依存しないシステムを、それぞれのクレートの `Plugin` 内での `add_systems` 登録へ移行します。
- Familiar AI のアダプター群（`state_decision` / `task_delegation` / `familiar_processor`）は、root 固有型への依存が解消された範囲で段階的に移譲します。

### フェーズ 4: アセット等の抽象化と移動
- `GameAssets` を直接参照しているために `bevy_app` に残っているロジックがある場合、既存の `UiAssets` トレイトパターンに倣い、ドメインごとの Trait / Resource を定義して注入する形にリファクタリングし、依存を断ち切った上で移動します。

## 4. リスクと対策

| リスク | 影響 | 対策 |
| :--- | :---: | :--- |
| 型移動時の参照箇所の取りこぼし | 高 | 各移動後に `cargo check --workspace` で即座に検出。一度に移動する型は1〜2個に絞る |
| pure core と root adapter の境界を曖昧にしたまま移設する | 高 | 「system 単位」ではなく「責務単位」で移設判断する。`Commands` / `WorldMapWrite` / root `MessageWriter` を使う関数は root に残す |
| Familiar AI の root adapter を削りすぎて root resource 依存が crate 側へ漏れる | 高 | `WorldMapRead` / concrete `SpatialGrid` / `PathfindingContext` を束ねるシステムは root 固定。crate 側は pure algorithm のみ |
| `Room` 移動に伴う detection/validation システムの大規模書き換え | 中 | Room 型の移動と detection システムの移動は分離して段階的に実施する |
| パスファインディング統括の移動で Familiar 側の到達判定に影響 | 低 | Familiar は `hw_world::find_path` を直接使用しており、soul の orchestration 層には依存していない |

## 5. 検証方法

各フェーズの完了時に以下を実施する:

1. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` → エラーなし
2. `cargo run` による動作確認（AI の意思決定・タスク割り当て・パスファインディングが正常に機能すること）
3. 移動先クレートの公開 API が最小限であること（不要な `pub` がないこと）を目視確認
4. `bevy_app` 側の残留ファイルが adapter / facade / orchestration / plugin wiring のいずれかで説明できることを確認
