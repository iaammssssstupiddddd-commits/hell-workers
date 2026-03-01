# UI MenuAction 責務境界整理 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `ui-menu-action-boundary-plan-2026-03-01` |
| ステータス | `Completed` |
| 作成日 | `2026-03-01` |
| 最終更新日 | `2026-03-01` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `MenuAction` の処理が `ui_interaction_system` 内と専用システムに分散し、`match` 内 no-op 分岐が残っている。
- 到達したい状態: Action ごとに処理責務が明確で、追跡時に「どこで処理されるか」が一意にわかる。
- 成功指標:
  - no-op 分岐（`ToggleDoorLock`, `SelectArchitectCategory`）の扱いが設計上明示される。
  - UI interaction の責務分割がコード構造に反映される。
  - `cargo check` 成功。

## 2. スコープ

### 対象（In Scope）

- `src/interface/ui/interaction/mod.rs` と `menu_actions.rs` の責務整理。
- `MenuAction` の処理分類（汎用/専用）の明示化。
- コメント・命名・モジュール構造の整備。

### 非対象（Out of Scope）

- 新規 UI 機能追加。
- 各アクションが実行するゲーム仕様自体の変更。
- ボタンレイアウトや見た目変更。

## 3. 現状とギャップ

- 現状: 一部アクションは `handle_pressed_action` で no-op、別システムで実処理される。
- 問題:
  - 追跡時に実処理場所が直感的でない。
  - 将来の Action 追加で同様の分散が再発しやすい。
- 本計画で埋めるギャップ:
  - Action 処理フローを明文化し、実装もその境界に合わせる。

## 4. 実装方針（高レベル）

- 方針:
  - Action を `Generic` / `Specialized`（仮称）で分類する。
  - `handle_pressed_action` は汎用アクションのみ処理。
  - 専用アクションは専用システムと明示的に紐付ける。
- 設計上の前提:
  - イベント発火順序は現状を維持。
  - `ui_interaction_system` の入力取得方式は変更しない。
- Bevy 0.18 APIでの注意点:
  - `Changed<Interaction>` を読む複数システム間の順序影響に注意し、必要なら `.chain()` か `SystemSet` 順序を固定する。

## 5. マイルストーン

## M1: Action 分類の定義

- 変更内容:
  - `MenuAction` ごとの処理担当を一覧化。
  - no-op 分岐を「専用システムへ委譲」として明示。
- 変更ファイル:
  - `src/interface/ui/interaction/menu_actions.rs`
  - `src/interface/ui/interaction/mod.rs`
  - `docs/entity_list_ui.md`（必要時）
- 完了条件:
- [x] Action 責務表がコードコメントまたは docs へ反映
- [x] no-op の意図が読める状態
- 検証:
  - `cargo check`

## M2: 処理ルーティングの整理

- 変更内容:
  - 汎用処理と専用処理の呼び出し経路を整理。
  - 必要なら専用システム群をサブモジュール化。
- 変更ファイル:
  - `src/interface/ui/interaction/mod.rs`
  - `src/interface/ui/interaction/menu_actions.rs`
- 完了条件:
- [x] `MenuAction` 追加時の実装ガイドが明確
- [x] 既存挙動を維持
- 検証:
  - `cargo check`

## M3: 文書同期と最終確認

- 変更内容:
  - UI interaction の責務境界を docs に反映。
  - コードコメント最小化と可読性確認。
- 変更ファイル:
  - `docs/DEVELOPMENT.md`（必要時）
  - `docs/entity_list_ui.md`（必要時）
  - `src/interface/ui/interaction/*.rs`
- 完了条件:
- [x] 設計方針が文書と一致
- [x] `cargo check` 成功
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| システム順序変更で挙動差分 | UI反応の欠落/二重実行 | `Update` 順序を固定し、Action 単位で動作確認する |
| 責務分割が過剰 | ファイル分散で可読性低下 | 「アクション分類単位」で分割し、最小限に留める |
| no-op 解除で重複実行 | 予期しない副作用 | `Pressed` 条件と対象 Action の排他をテスト観点化する |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - Architect カテゴリ切替、Door lock 切替、モード切替、ダイアログ開閉。
  - `Interaction::Pressed` で1回だけ処理されること。
- パフォーマンス確認（必要時）:
  - 不要（ロジック境界整理が主目的）。

## 8. ロールバック方針

- どの単位で戻せるか:
  - `interaction` モジュール単位で戻せる。
- 戻す時の手順:
  - 責務整理コミットを revert。
  - `cargo check` と UI 手動確認を再実行。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1` / `M2` / `M3`
- 未着手/進行中: なし

### 次のAIが最初にやること

1. `MenuAction` の全バリアントを「どのシステムで処理されるか」で棚卸しする。
2. `menu_actions.rs` の no-op 分岐を明示方針に変更する。
3. `arch_category_action_system` / `door_lock_action_system` の順序を確認する。

### ブロッカー/注意点

- `Changed<Interaction>` を読む複数システムは実行順が重要な場合がある。

### 参照必須ファイル

- `src/interface/ui/interaction/mod.rs`
- `src/interface/ui/interaction/menu_actions.rs`
- `src/interface/ui/plugins/core.rs`
- `docs/DEVELOPMENT.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-01` / `pass` (`CARGO_TARGET_DIR=/tmp/hell-workers-target-check cargo check`)
- 未解決エラー: なし（計画作成時点）

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-01` | `Codex` | 初版作成 |
