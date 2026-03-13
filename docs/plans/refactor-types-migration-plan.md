# 型・ドメインモデルのクレート境界リファクタリング計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `types-migration-plan-2026-03-13` |
| ステータス | `Draft` |
| 作成日 | `2026-03-13` |
| 最終更新日 | `2026-03-13` |
| 作成者 | `Gemini Agent` |
| 関連提案 | `docs/proposals/crate-boundaries-refactor-plan.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `bevy_app` にドメイン固有の型や汎用的な基礎型（`GameTime`, `Room` など）が残留しており、アーキテクチャの境界原則（`docs/crate-boundaries.md`）に違反している状態を解消する。
- 到達したい状態: 基礎型とドメインモデルがそれぞれの所有者である `hw_*` クレートに正しく配置され、循環依存を気にせずに他クレートから参照できる状態になること。
- 成功指標: 指定した型がすべて Leaf クレートへ移動完了し、`cargo check --workspace` が警告・エラーなしで通過すること。

## 2. スコープ

### 対象（In Scope）

- `GameTime` (現在 `bevy_app/src/systems/time.rs`) の `hw_core` への移動
- `DreamTreePlantingPlan` (現在 `bevy_app/src/systems/dream_tree_planting.rs`) の `hw_jobs` への移動
- `AreaEditSession` 等の UI 状態 (現在 `bevy_app/src/systems/command/area_selection/`) の `hw_ui` への移動
- `Room`, `RoomTileLookup` (現在 `bevy_app/src/systems/room/`) の `hw_world` への移動

### 非対象（Out of Scope）

- AI の意思決定ロジックの純粋関数化（別計画で実施）
- 経路探索や建設完了判定など、独立したシステム全体の移動（別計画で実施）

## 3. 現状とギャップ

- 現状: `bevy_app` が多数の型を所有している。他クレートからこれらの型を参照しようとすると、`bevy_app` への逆依存となるためコンパイルエラーになる。
- 問題: 型の所有権がアーキテクチャの原則に反しているため、今後のドメインロジックの分離（別計画の作業）を阻害するブロッカーとなっている。
- 本計画で埋めるギャップ: 各型を適切なドメイン層へ引き下げることで、アーキテクチャの「土台」を整える。

## 4. 実装方針（高レベル）

- 方針: `docs/crate-boundaries.md` の「§2. 型定義と所有権のルール」に従い、最も凝集度の高いクレートへ型を移動する。
- 設計上の前提: 型の移動に伴い、`bevy_app` 側のインポートパス（`use`）の大規模な書き換えが発生する。
- Bevy 0.18 APIでの注意点: `Component` や `Resource` マクロの派生を含めてそのまま移動する。

## 5. マイルストーン

### M1: GameTime と DreamTreePlantingPlan の移動

- 変更内容:
  - `GameTime` を `hw_core` へ移動。
  - `DreamTreePlantingPlan` を `hw_jobs` へ移動。
- 変更ファイル:
  - `crates/hw_core/src/...`
  - `crates/hw_jobs/src/...`
  - `crates/bevy_app/src/systems/time.rs` 等
- 完了条件:
  - [ ] 型の移動が完了し、`bevy_app` からの参照が通る
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

### M2: UI 操作状態（AreaEditSession 等）の移動

- 変更内容: `AreaEditSession` などコマンド・選択状態の型を `hw_ui` 内の適切なモジュール（例: `interaction/` や `selection/`）へ移動。
- 変更ファイル:
  - `crates/hw_ui/src/...`
  - `crates/bevy_app/src/systems/command/area_selection/`
- 完了条件:
  - [ ] `hw_ui` が状態型を所有するようになる
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

### M3: Room コンポーネントの移動

- 変更内容: `Room`, `RoomTileLookup` を `hw_world` の `room_detection` モジュール付近へ移動。
- 変更ファイル:
  - `crates/hw_world/src/room_detection/...`
  - `crates/bevy_app/src/systems/room/`
- 完了条件:
  - [ ] `Room` 型が `hw_world` に配置される
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 大量ファイルの `use` パス修正漏れ | 高 | `rust-analyzer` の診断と `cargo check` をステップごとに細かく実行し、取りこぼしを防ぐ。 |
| 移動先での意図しない循環依存の発生 | 中 | 移動先のクレートの `Cargo.toml` の `dependencies` に違反しないか事前に確認する。 |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- 手動確認シナリオ:
  - アプリを起動し、時間の経過(`GameTime`)、ドラッグによる植林プレビュー(`DreamTree...`)、範囲選択(`AreaEditSession`)、部屋の認識(`Room`) が壊れていないか確認する。

## 8. ロールバック方針

- どの単位で戻せるか: Gitコミット単位（M1〜M3の各マイルストーン完了ごとにコミットする）。
- 戻す時の手順: 該当する型の移動コミットを `git revert` する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1

### 次のAIが最初にやること

1. `GameTime` の定義を `hw_core` に新設し、`bevy_app` から移動する。
2. `DreamTreePlantingPlan` の定義を `hw_jobs` に新設する。
3. `cargo check` で壊れたパスを修正する。

### ブロッカー/注意点

- `CARGO_HOME` プレフィックスを必ずつけてコマンドを実行すること。

### 参照必須ファイル

- `docs/crate-boundaries.md`

### 最終確認ログ

- 最終 `cargo check`: N/A
- 未解決エラー: なし

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-13` | Gemini Agent | 初版ドラフト作成 |