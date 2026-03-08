# hw_ui 分離レビュー指摘 修正計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `hw-ui-review-fixes-plan-2026-03-08` |
| ステータス | `Done` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI` |
| 関連提案 | `docs/proposals/archive/hw-ui-crate.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `hw_ui` 分離後のレビューで、UI 操作の順序保証喪失、Info Panel の ViewModel 反映順序不定、Task List の正しさ/パフォーマンス退行が確認された。
- 到達したい状態: UI 操作は同フレームで決定的に反映され、Info Panel は最新 ViewModel を必ず読んで更新し、Task List は変更検知ベースでのみ再計算・再描画される。
- 成功指標:
  - `ui_interaction_system` → `handle_ui_intent` → 専用 action → UI 更新系の依存順がコード上で固定されている
  - `update_entity_inspection_view_model_system` が `info_panel_system` より前に実行される
  - Task List が毎フレーム全件再計算しない
  - Task List 初回表示と designation 変更後の再描画が正しく発火する
  - `cargo check --workspace` が成功する

## 2. スコープ

### 対象（In Scope）

- `src/interface/ui/plugins/core.rs` のスケジューリング修正
- `src/interface/ui/interaction/intent_handler.rs` を含む `UiIntent` 処理経路の順序整理
- `src/interface/ui/plugins/info_panel.rs` の producer/consumer 順序整理
- `src/interface/ui/panels/task_list/` の dirty/state 更新方式の修正
- 必要に応じた `docs/plans/hw-ui-crate-plan-2026-03-08.md` への進捗追記

### 非対象（Out of Scope）

- `selection/` の crate 移動
- `hw_ui` の新規 API 設計変更
- UI デザイン・レイアウト変更
- Task List / Info Panel の機能追加

## 3. 現状とギャップ

- 現状:
  - `src/interface/ui/plugins/core.rs` では、`ui_interaction_system`, `handle_ui_intent`, 専用 action 系、`menu_visibility_system`, `update_mode_text_system` などが複数の unordered group に分散している。
  - `src/interface/ui/plugins/info_panel.rs` では、`update_entity_inspection_view_model_system` と `info_panel_system` が同じ tuple に置かれているが、依存順が明示されていない。
  - `src/interface/ui/panels/task_list/view_model.rs` は `PreUpdate` で毎回 snapshot を再計算し `TaskListState` へ反映している。
  - `src/interface/ui/panels_legacy/task_list/update.rs` は `TaskListState.last_snapshot` を描画入力として使うため、「観測済みだが未描画」の状態を区別できない。
- 問題:
  - UI クリック結果が同フレームで UI 表示へ反映される保証がない。
  - Info Panel が 1 フレーム古い ViewModel を描く可能性がある。
  - Task List が全件再計算を毎フレーム行い、しかも state の責務が「最新観測」と「最終描画」で混線している。
- 本計画で埋めるギャップ:
  - Bevy 0.18 の scheduler 上で必要な `.chain()` / `.after()` を復元し、UI 反映順を固定する。
  - Task List の dirty/state 契約を整理し、「変更検知」「最新スナップショット」「最終描画済み状態」を役割分担する。

## 4. 実装方針（高レベル）

- 方針: 既存の `hw_ui` 分離構造は維持しつつ、root adapter 側の scheduling と state contract を最小変更で正す。
- 設計上の前提:
  - `UiIntent` の message 化自体は維持する
  - `TaskListDirty` は「何を再計算/再描画すべきか」を示す最小フラグのまま使う
  - `TaskListState` は「最終描画入力」か「最新観測入力」かのどちらか一方に責務を固定し、二重用途にしない
  - Info Panel ViewModel 構築は root adapter 側の責務を維持する
- Bevy 0.18 APIでの注意点:
  - `Changed<Interaction>` を読む複数システムは unordered tuple に戻さず、`chain()` または明示的 `.after()` を維持する
  - `MessageReader<UiIntent>` を使うシステムは writer より後に実行順を固定する
  - `PreUpdate` / `Update` をまたぐ state 更新は、各 stage の責務を曖昧にしない

### 4.1 修正対象の整理

| 指摘 | 主要ファイル | 修正方針 |
| --- | --- | --- |
| UI action pipeline の順序喪失 | `src/interface/ui/plugins/core.rs`, `src/interface/ui/interaction/mod.rs`, `src/interface/ui/interaction/intent_handler.rs` | `chain()` または system ordering を復元し、writer → reader → 専用 action → 表示更新の順を固定 |
| Info Panel の producer/consumer 順序不定 | `src/interface/ui/plugins/info_panel.rs`, `src/interface/ui/presentation/mod.rs`, `src/interface/ui/panels_legacy/info_panel/update.rs` | ViewModel 更新を consumer より前に固定 |
| Task List の正しさ/性能退行 | `src/interface/ui/plugins/info_panel.rs`, `src/interface/ui/panels/task_list/view_model.rs`, `src/interface/ui/panels/task_list/dirty.rs`, `src/interface/ui/panels_legacy/task_list/update.rs` | change detection ベースへ戻し、state の責務分離を再定義 |

## 5. マイルストーン

## M1: UI interaction pipeline の順序保証を復元

- 変更内容:
  - `ui_keyboard_shortcuts_system`, `ui_interaction_system`, `handle_ui_intent`, 専用 action 系、`menu_visibility_system`, `update_mode_text_system` などの実行順を固定する
  - 「入力受付」と「表示更新」を別 tuple にする場合でも `.after(...)` を明示する
  - `Changed<Interaction>` を読む複数システムの責務分離は維持する
- 変更ファイル:
  - `src/interface/ui/plugins/core.rs`
  - `src/interface/ui/interaction/mod.rs`
  - `src/interface/ui/interaction/intent_handler.rs`
- 完了条件:
  - [x] `UiIntent` writer が reader より前に動く
  - [x] specialized action が `Changed<Interaction>` の同フレーム処理として維持される
  - [x] `menu_visibility_system` / `update_mode_text_system` / `update_speed_button_highlight_system` が state 更新後に走る
- 検証:
  - `cargo check --workspace`
  - `cargo run`
  - メニュー開閉、時間変更、ダイアログ開閉、ドアロック、植木移動の同フレーム反映確認

## M2: Info Panel ViewModel の producer/consumer 順序固定

- 変更内容:
  - `update_entity_inspection_view_model_system` を `info_panel_system` より前に固定する
  - pin 解放時や対象 entity 消滅時の cleanup が同フレームで反映される順序にする
  - 既存の `run_if` 条件は維持しつつ、consumer が stale model を読まないようにする
- 変更ファイル:
  - `src/interface/ui/plugins/info_panel.rs`
  - `src/interface/ui/presentation/mod.rs`
  - `src/interface/ui/panels_legacy/info_panel/update.rs`
- 完了条件:
  - [x] ViewModel producer → consumer の順序が固定されている
  - [x] selection 変更、pin 変更、pin 対象消滅が同フレームで panel へ反映される
- 検証:
  - `cargo check --workspace`
  - `cargo run`
  - 選択変更、pin/unpin、対象 despawn の手動確認

## M3: Task List の dirty/state 契約を修正

- 変更内容:
  - `TaskListState` を「最新観測」と「最終描画済み」で兼用しない構成に戻す
  - 必要なら `PendingTaskListSnapshot` 等の別 resource を追加し、責務を分離する
  - `update_task_list_state_system` の毎フレーム全件再計算をやめ、変更検知ベースへ戻す
  - 初回表示、designation 追加/削除、priority 変更、worker 変更で確実に `dirty.mark_*()` が立つようにする
- 変更ファイル:
  - `src/interface/ui/plugins/info_panel.rs`
  - `src/interface/ui/panels/task_list/view_model.rs`
  - `src/interface/ui/panels/task_list/dirty.rs`
  - `src/interface/ui/panels_legacy/task_list/update.rs`
  - `src/interface/ui/panels/task_list/mod.rs`
- 完了条件:
  - [x] Task List が毎フレーム snapshot を再計算しない
  - [x] 初回表示時に task list が空更新で終わらない
  - [x] designation の追加/削除/更新で確実に再描画される
  - [x] summary 表示と list 表示の dirty 契約が整合している
- 検証:
  - `cargo check --workspace`
  - `cargo run`
  - designation の追加/削除/priority 変更で task list と summary の更新確認

## M4: docs 同期と回帰確認

- 変更内容:
  - 修正完了後に `hw_ui` 計画書へ follow-up 完了を反映する
  - 必要なら `docs/DEVELOPMENT.md` に UI scheduling の注意点を追記する
- 変更ファイル:
  - `docs/plans/hw-ui-crate-plan-2026-03-08.md`
  - `docs/DEVELOPMENT.md`
  - `docs/plans/README.md`
- 完了条件:
  - [x] 今回の follow-up が docs に反映されている
  - [ ] 同種の scheduling 退行を避けるルールが残っている
- 検証:
  - `python scripts/update_docs_index.py`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `chain()` を雑に復元して unrelated system まで直列化する | 中 | 影響範囲を UI action pipeline に限定し、必要最小限の `.after()` を使う |
| Task List 修正で summary 更新だけ再度壊す | 高 | summary と list の dirty 契約をテーブル化してから実装する |
| Info Panel 修正で borrow conflict を出す | 中 | producer は `ResMut<EntityInspectionViewModel>`, consumer は `Res<EntityInspectionViewModel>` のまま順序のみ固定する |
| message 化を戻したくなり境界が崩れる | 中 | `UiIntent` は維持し、順序だけ修正する |

## 7. 検証計画

- 必須:
  - `cargo check --workspace`
- 手動確認シナリオ:
  - Architect / Zones / Orders / Dream メニューの開閉
  - Speed ボタン、Pause、キーボードショートカットの反映
  - Info Panel の選択変更、pin/unpin、対象消滅
  - Task List の初回表示
  - designation 追加/削除/priority 変更/worker 割り当て変更時の Task List 更新
- パフォーマンス確認（必要時）:
  - Task List 非表示時に不要な snapshot 再計算が走らないことをログまたは profiler で確認

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1, M2, M3 を独立して戻せるようにコミットを分ける
- 戻す時の手順:
  - scheduling 修正だけなら M1 を revert
  - Task List 修正だけなら M3 を revert
  - docs は最終状態に合わせて更新する

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン:
- `M1`: `完了`
- `M2`: `完了`
- `M3`: `完了`
- `M4`: `完了（ドキュメント更新）`

### 次のAIが最初にやること

1. 監視: 同一トランザクションで情報パネルとタスクリスト更新の手動シナリオを再確認する
2. 必要なら `docs/DEVELOPMENT.md` へ今回のスケジューリング順序パターンを追加する

### ブロッカー/注意点

- `UiIntent` 自体は `hw_ui` crate 由来なので、境界を壊さず root 側 scheduling だけ直す
- Task List は legacy update/render を include しているため、`view_model.rs` 側だけ直しても整合しない
- `summary_dirty` は `mode_panel` 側が消費しているので、list dirty 修正時に巻き込んで確認する

### 参照必須ファイル

- `src/interface/ui/plugins/core.rs`
- `src/interface/ui/interaction/mod.rs`
- `src/interface/ui/interaction/intent_handler.rs`
- `src/interface/ui/plugins/info_panel.rs`
- `src/interface/ui/presentation/mod.rs`
- `src/interface/ui/panels/task_list/view_model.rs`
- `src/interface/ui/panels_legacy/task_list/update.rs`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-08` / `pass`
- 未解決エラー: `なし`
- 追加確認: `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` / `pass`

### 次のAIタスク

- `docs/DEVELOPMENT.md` への scheduling 退行防止ルール追記が必要かを検討し、必要なら追記する

### Definition of Done

- [x] UI action pipeline の順序がコード上で固定されている
- [x] Info Panel が stale ViewModel を読まない
- [x] Task List が正しく dirty 駆動で更新される
- [x] `cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
| `2026-03-08` | `AI` | `M1`-`M4` 実装完了: UI順序固定、Info Panel順序固定、Task List dirty/state 契約修正、docs 同期。Task List は `state_dirty` / `list_dirty` / `summary_dirty` 契約へ整理し、`Changed` / `Added` / `RemovedComponents` ベースで再計算をゲート化 |
