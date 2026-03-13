# Soul / Jobs システムのクレート境界リファクタリング計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `soul-jobs-migration-plan-2026-03-13` |
| ステータス | `Draft` |
| 作成日 | `2026-03-13` |
| 最終更新日 | `2026-03-13` |
| 作成者 | `Gemini Agent` |
| 関連提案 | `docs/proposals/crate-boundaries-refactor-plan.md` (Archived) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `bevy_app` に Soul AI の経路探索オーケストレーションや Jobs の建設完了判定システムが残留しており、ドメイン境界が曖昧になっている状態を解消する。
- 到達したい状態: 該当システムがそれぞれの所有者である `hw_soul_ai`, `hw_jobs` クレートに正しく配置され、各クレートの Plugin 内で登録（`add_systems`）される状態になること。
- 成功指標: システムの移動と Plugin 登録の移譲が完了し、`cargo check --workspace` を通過。ゲーム内で移動と建設が正常に行われること。

## 2. スコープ

### 対象（In Scope）

- `crates/bevy_app/src/entities/damned_soul/movement/pathfinding/mod.rs` (パスファインディング統括) の `hw_soul_ai` への移動。
- `crates/bevy_app/src/systems/jobs/building_completion/world_update.rs` (建設完了時のワールド更新) の `hw_jobs` への移動。

### 非対象（Out of Scope）

- `hw_world` 内にある経路探索コアアルゴリズム自体の修正。
- Familiar AI アダプターの分離（別計画で実施）。

## 3. 現状とギャップ

- 現状: 本来 Leaf クレートが所有・登録すべきドメイン固有のシステムが `bevy_app` で定義され、`bevy_app` の Plugin で登録されている。
- 問題: 依存関係の制約はないのに移動が行われておらず、将来的なコードベース拡大時の見通しを悪くしている。
- 本計画で埋めるギャップ: `Cargo.toml` の依存だけで完結しているシステムを、あるべき場所（Leaf クレート）へ戻す。

## 4. 実装方針（高レベル）

- 方針: 対象システムをディレクトリごと移動し、`bevy_app` 側の Plugin から該当の `.add_systems(...)` を削除。移動先の `hw_soul_ai` / `hw_jobs` の Plugin に同等の登録処理を追加する。
- 設計上の前提: 移動元で定義されていたシステム実行順序の制約（`.before()`, `.after()`）を移動先でも厳密に維持する。

## 5. マイルストーン

### M1: 建設完了判定の hw_jobs への移動

- 変更内容: `building_completion/world_update.rs` を `hw_jobs` に移動し、`hw_jobs` の Plugin で登録する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/jobs/building_completion/`
  - `crates/hw_jobs/src/...`
- 完了条件:
  - [ ] システムが移動され、コンパイルが通る
- 検証:
  - `cargo check --workspace`

### M2: 経路探索統括の hw_soul_ai への移動

- 変更内容: `movement/pathfinding/` を `hw_soul_ai/src/soul_ai/execute/` 周辺に移動し、`hw_soul_ai` の Plugin で登録する。
- 変更ファイル:
  - `crates/bevy_app/src/entities/damned_soul/movement/pathfinding/`
  - `crates/hw_soul_ai/src/...`
- 完了条件:
  - [ ] システムが移動され、コンパイルが通る
- 検証:
  - `cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 登録順序の喪失による 1 フレーム遅れの発生 | 高 | 移動前の `bevy_app` 側 Plugin に記述されていた順序制約を移動先の Plugin でも正確に再現する。 |
| 未知の Root リソース依存の発覚 | 中 | もし `bevy_app` 固有リソースに依存していた場合は、無理に移動せず、別計画に切り出して抽象化を先行する。 |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- 手動確認シナリオ:
  - ゲームを起動し、タスクを割り当てられた魂が障害物を避けて正しく目的地に到達することを確認する。
  - 建設（壁や床など）が完了した際に、その上の通行判定などが正しく更新されることを確認する。

## 8. ロールバック方針

- どの単位で戻せるか: Gitコミット単位（M1, M2の完了ごと）。
- 戻す時の手順: 該当する移動コミットを `git revert` する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1

### 次のAIが最初にやること

1. `crates/bevy_app/src/systems/jobs/building_completion/world_update.rs` の内容と依存関係を確認する。
2. これを `hw_jobs` クレートに移動する。
3. Plugin登録を `bevy_app` から `hw_jobs` に移す。

### ブロッカー/注意点

- 他の移行計画と並行して進める場合は、ファイルのコンフリクトに注意する。

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