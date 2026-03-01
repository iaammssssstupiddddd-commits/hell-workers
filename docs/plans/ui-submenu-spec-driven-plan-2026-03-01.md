# サブメニュー生成のデータ駆動化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `ui-submenu-spec-driven-plan-2026-03-01` |
| ステータス | `Completed` |
| 作成日 | `2026-03-01` |
| 最終更新日 | `2026-03-01` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `submenus.rs` でサブメニューコンテナ生成とボタン定義が重複しており、項目追加時の修正点が多い。
- 到達したい状態: サブメニューを `Spec` で宣言し、描画生成は共通関数で行う。
- 成功指標:
  - Architect/Zones/Orders/Dream の共通コンテナ処理が統一される。
  - メニュー項目追加時にデータ定義のみで済む箇所が増える。
  - `cargo check` 成功。

## 2. スコープ

### 対象（In Scope）

- `src/interface/ui/setup/submenus.rs` の生成ロジック共通化。
- メニュー項目定義のデータ化（label/action/color）。
- Architect カテゴリ/建物パネル定義の整理。

### 非対象（Out of Scope）

- UI テーマ変更。
- ボタンサイズや表示位置のデザイン変更。
- `menu_visibility_system` のロジック変更（必要最小限の対応を除く）。

## 3. 現状とギャップ

- 現状:
  - `Node` 共通設定と `Button` 生成が複数箇所で手書きされる。
- 問題:
  - 追加・削除時にコピペ編集が必要。
  - 小さな差分がバグ原因になりやすい。
- 本計画で埋めるギャップ:
  - 生成コードを共通化し、差分は spec テーブルへ集約。

## 4. 実装方針（高レベル）

- 方針:
  - `SubmenuSpec` / `MenuEntrySpec`（仮称）を導入。
  - 共通 `spawn_submenu_container` と `spawn_menu_entries` を実装。
  - Architect 固有のカテゴリパネルだけ専用生成を残す。
- 設計上の前提:
  - 既存 `MenuAction` と UI コンポーネント型は再利用。
  - 表示座標は `UiTheme` の既存値を使う。
- Bevy 0.18 APIでの注意点:
  - `Commands` と `ChildSpawnerCommands` のライフタイム制約を崩さないよう、共通関数は引数を明確化する。

## 5. マイルストーン

## M1: サブメニュー共通コンテナ抽出

- 変更内容:
  - Architect/Zones/Orders/Dream で共通な `Node` 設定を抽出。
  - コンテナ spawn を1関数へ統一。
- 変更ファイル:
  - `src/interface/ui/setup/submenus.rs`
  - `src/interface/ui/components.rs`（必要時）
- 完了条件:
  - [x] 4メニューのコンテナ生成重複が削減される
  - [x] コンポーネント付与が既存どおり維持される
- 検証:
  - `cargo check`

## M2: メニュー項目の spec 化

- 変更内容:
  - Zones/Orders/Dream をデータ定義ベースへ移行。
  - Architect のカテゴリ/建物パネル定義も可能な範囲で spec 化。
- 変更ファイル:
  - `src/interface/ui/setup/submenus.rs`
  - `src/systems/jobs/mod.rs`（カテゴリラベル連携が必要な場合）
- 完了条件:
  - [x] 項目追加がデータ追記中心で可能
  - [x] 色指定やアクション指定が保持される
- 検証:
  - `cargo check`

## M3: 可読性調整と docs 同期

- 変更内容:
  - 生成ルールをコメント/ドキュメントに反映。
  - `menu_visibility_system` 前提に変更があれば同期更新。
- 変更ファイル:
  - `src/interface/ui/setup/submenus.rs`
  - `docs/task_list_ui.md`（必要時）
  - `docs/DEVELOPMENT.md`（必要時）
- 完了条件:
  - [x] 構造が読みやすく、責務境界が明確
  - [x] `cargo check` 成功
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 抽象化しすぎで逆に読みにくくなる | 保守性低下 | Architect 固有処理は分離し、共通部のみ抽出する |
| spec と実コンポーネント不一致 | メニュー表示不具合 | 生成後 Query 条件（SubMenu marker）を手動確認する |
| action 取り違え | 誤モード遷移 | 既存 action 一覧と diff レビューを必須化する |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - Architect/Zones/Orders/Dream の開閉。
  - Architect カテゴリ選択→建物列展開→再クリックでトグル閉じ。
  - Zone remove ボタンの色と動作。
- パフォーマンス確認（必要時）:
  - 不要（UI 生成コードの保守性向上が主目的）。

## 8. ロールバック方針

- どの単位で戻せるか:
  - `submenus.rs` 単位で戻せる。
- 戻す時の手順:
  - spec 化コミットを revert。
  - `cargo check` と UI 手動確認を実施。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1` / `M2` / `M3`
- 未着手/進行中: なし

### Architect カテゴリ構造（実装済み）

カテゴリ2段階メニューは既に実装済み（本計画の実施前に完了）:
- `BuildingCategory` enum（`src/systems/jobs/mod.rs`）で Structure/Architecture/Plant/Temporary を定義
- `ArchitectCategoryState(Option<BuildingCategory>)` Resource でカテゴリ選択状態を管理
- `ArchitectCategoryListPanel` / `ArchitectBuildingPanel(BuildingCategory)` Component で表示制御
- カテゴリ列（左）＋建物列（右展開）の横並び構成、再クリックでトグル閉じ
- Back ボタンは廃止（カテゴリ再クリックでトグル）

### 次のAIが最初にやること

1. メニュー項目追加時は `*_menu_specs` と `architect_building_specs` のみを更新する。
2. コンテナ共通スタイル変更時は `spawn_submenu_container` を単一更新点として使う。
3. 変更後に `cargo check` を実行する。

### ブロッカー/注意点

- `menu_visibility_system` の Query 条件と marker コンポーネント付与は崩さない（特に6クエリの `Without<>` 制約が必要）。
- Architectカテゴリ構造は変更しない（実装済み仕様として固定）。

### 参照必須ファイル

- `src/interface/ui/setup/submenus.rs`
- `src/interface/ui/panels/menu.rs`
- `src/interface/ui/components.rs`
- `docs/DEVELOPMENT.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-01` / `pass` (`cargo check --target-dir /tmp/hell-workers-target`)
- 未解決エラー: なし

### Definition of Done

- [x] 目的に対応するマイルストーンが全て完了
- [x] 影響ドキュメントが更新済み
- [x] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-01` | `Codex` | 初版作成 |
| `2026-03-01` | `Codex` | 実装完了に合わせてステータス・進捗・DoDを更新 |
