# Familiar UI `hw_ui` 移設計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-ui-hw-ui-migration-plan-2026-03-11` |
| ステータス | `In Progress` |
| 作成日 | `2026-03-11` |
| 最終更新日 | `2026-03-11` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  - `familiar_ui` 相当の Entity List Familiar セクション実装が root と `hw_ui` に分散しており、純 UI ロジックとゲーム依存 adapter の境界がまだ粗い。
- 到達したい状態:
  - UI ノード生成・差分同期・表示更新の純 UI 部分を `hw_ui` へ寄せ、root 側には `Familiar` / `DamnedSoul` / `AssignedTask` などから ViewModel を構築する adapter と app 固有操作だけを残す。
- 成功指標:
  - `src/interface/ui/list/` の Familiar 向け実装で、ゲーム型に依存しないものが `hw_ui` へ移設される。
  - root 側の Entity List プラグイン登録コードが「thin shell」として読める状態になる。
  - `docs/cargo_workspace.md` と `docs/entity_list_ui.md` の境界記述と実装が一致する。

## 2. スコープ

### 対象（In Scope）

- Familiar/Soul Entity List の純 UI ノード生成ロジック
- Familiar/Soul Entity List の差分同期アルゴリズム
- Entity List 用アセット参照の抽象化
- root 側 Entity List システムの thin adapter 化
- 関連ドキュメント更新

### 非対象（Out of Scope）

- `Familiar` / `DamnedSoul` / `AssignedTask` からの ViewModel 構築ロジック移設
- `TaskContext` / `SelectedEntity` / `MainCamera` 依存の操作全般
- DnD 配属フローの crate 移設
- Info Panel / Task List の同時リファクタ
- 新規 crate 追加

## 3. 現状とギャップ

- 現状:
  - `hw_ui` には `EntityListViewModel`、`DragState`、`EntityListDirty`、minimize/resize、共通ハイライトなどが既にある。
  - root 側には `view_model.rs`、`spawn/`、`sync/`、`interaction.rs`、`drag_drop.rs` が残っている。
  - `docs/cargo_workspace.md` では root 残留を adapter 責務に寄せる方針が明記されている。
- 問題:
  - `spawn/` と `sync/` の中に、ゲーム型非依存の UI 差分更新まで root に残っている。
  - `GameAssets` を直接受け取るため、`hw_ui` に再利用可能な部品として閉じていない。
  - `interaction.rs` は「折りたたみ/行クリック」と「FamiliarOperation 更新/イベント発行」が同居しており、移設単位が不明瞭。
- 本計画で埋めるギャップ:
  - `spawn/sync` の純 UI 部を `hw_ui::list` の公開 API に寄せる。
  - Entity List 専用 asset trait を導入し、root 側は `GameAssets` 実装だけを持つ。
  - `interaction.rs` は最終的に「共通 UI 操作」と「ゲーム副作用あり操作」を分離できる形に整理する。

## 4. 実装方針（高レベル）

- 方針:
  - 新しい `familiar_ui` crate は作らず、既存 `hw_ui` に段階的に寄せる。
  - まずは `spawn/sync` と表示更新 helper の移設を優先し、ゲーム固有 Query と副作用は root に残す。
  - 1 回で全部動かさず、「純 UI helper 移設」→「system thin shell 化」→「必要なら interaction 分割」の順で進める。
- 設計上の前提:
  - `hw_ui` は `Familiar` / `DamnedSoul` / `AssignedTask` に直接依存しない。
  - Bevy system 関数では `Res<dyn Trait>` を受け取れないため、trait object は system の外側 helper 呼び出しで使う。
  - `EntityListViewModel` は現状のまま root で構築し、`hw_ui` には ViewModel 消費側だけを集約する。
- Bevy 0.18 APIでの注意点:
  - `Commands` / `Children` / `replace_children` を使う差分同期は helper 化しても system 実行順を維持する。
  - system 間 ordering は root plugin 側で維持し、`GameSystemSet::Interface` の登録順を崩さない。

## 5. マイルストーン

## M1: Entity List 用 asset abstraction 導入

- 変更内容:
  - `hw_ui::list` から使うフォント・アイコン群をまとめた trait を追加する。
  - `setup::UiAssets` へ統合するか、`list::EntityListAssets` を新設するかを決める。
  - root の `GameAssets` に adapter 実装を追加する。
- 変更ファイル:
  - `crates/hw_ui/src/list/mod.rs`
  - `crates/hw_ui/src/list/*.rs` または `crates/hw_ui/src/setup/mod.rs`
  - `src/assets.rs`
  - `src/interface/ui/setup/mod.rs` または Entity List adapter 追加先
  - `docs/entity_list_ui.md`
- 完了条件:
  - [x] `spawn/sync` helper が `GameAssets` 具象型ではなく trait 経由でアセットを参照できる
  - [x] `arrow_right`, `arrow_down`, gender/task icon, UI fonts の供給経路が一箇所にまとまる
- 検証:
  - `cargo check --workspace`

## M2: Familiar/Soul の spawn/sync helper を `hw_ui::list` へ移設

- 変更内容:
  - 以下を `hw_ui::list` 側へ移す。
    - `src/interface/ui/list/spawn/familiar_section.rs`
    - `src/interface/ui/list/spawn/soul_row.rs`
    - `src/interface/ui/list/sync/familiar.rs`
    - `src/interface/ui/list/sync/unassigned.rs`
  - `EntityListNodeIndex` / `FamiliarSectionNodes` の所有位置も `hw_ui` 側へ寄せるか検討し、少なくとも差分同期 API の入出力を `hw_ui` で完結させる。
  - root 側の `sync_entity_list_from_view_model_system` は `Res<GameAssets>` を取得して `hw_ui` helper を呼ぶ thin shell に縮小する。
- 変更ファイル:
  - `crates/hw_ui/src/list/mod.rs`
  - `crates/hw_ui/src/list/spawn.rs` または新設サブモジュール
  - `crates/hw_ui/src/list/sync.rs` または新設サブモジュール
  - `src/interface/ui/list/mod.rs`
  - `src/interface/ui/list/spawn.rs`
  - `src/interface/ui/list/sync.rs`
  - `src/interface/ui/plugins/entity_list.rs`
  - `docs/entity_list_ui.md`
  - `docs/cargo_workspace.md`
- 完了条件:
  - [x] root 側 `spawn/` `sync/` 実装が削減され、主処理が `hw_ui` 側 helper 呼び出しになる
  - [x] `hw_ui` からゲーム型への逆依存が発生しない
  - [ ] Familiar セクションの追加/削除/折りたたみ/並べ替え/空分隊表示が従来通り動く
- 検証:
  - `cargo check --workspace`
  - `cargo run`

## M3: interaction の責務分離

- 変更内容:
  - `entity_list_interaction_system` を「共通 UI 操作」と「ゲーム副作用あり操作」に分割する。
  - 移設候補:
    - 折りたたみボタンの色更新
    - Familiar/Soul 行クリック時の共通処理の前段
  - root 残留:
    - `FamiliarOperation` 更新
    - `FamiliarOperationMaxSoulChangedEvent` 発行
    - `SelectedEntity` / `MainCamera` / `TaskContext` 依存の処理
  - 必要なら intent を entity 指定可能な形へ拡張し、ボタン処理を message 駆動へ寄せる。
- 変更ファイル:
  - `src/interface/ui/list/interaction.rs`
  - `src/interface/ui/list/interaction/navigation.rs`
  - `crates/hw_ui/src/intents.rs`
  - `crates/hw_ui/src/list/*.rs`
  - `src/interface/ui/interaction/intent_handler.rs`
  - `docs/entity_list_ui.md`
  - `docs/cargo_workspace.md`
- 完了条件:
  - [x] `interaction.rs` の UI-only 処理と game-side effect 処理が読み分けられる
  - [x] `FamiliarOperation` 変更経路が 1 つに整理される
  - [x] optimistic header update の仕様が維持される
- 検証:
  - `cargo check --workspace`
  - `cargo run`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `GameAssets` 直接依存を外す途中でアイコン参照が壊れる | Entity List が空描画または誤アイコン表示になる | M1 で asset trait の責務を先に固定し、アイコン一覧を doc に列挙する |
| `EntityListNodeIndex` の所有場所変更で同期バグが出る | Familiar/Soul 行の二重生成・並び替え不整合 | M2 は API 移設だけを優先し、index resource 自体は必要なら root 残留で段階移行する |
| `interaction` 分割時にイベント経路が二重化する | `max_controlled_soul` が二重更新される | `FamiliarOperation` 更新経路を 1 系統に限定し、旧経路削除まで同時に行う |
| plugin ordering を崩す | UI 更新タイミングが変わり表示ちらつきが出る | system 関数名は変わっても root plugin で `.after(...)` / `.chain()` の順序を維持する |
| スコープが拡大して DnD や TaskContext まで巻き込む | 予定より大きい refactor になる | DnD と navigation は明示的に out-of-scope とし、別計画に切り出す |

## 7. 検証計画

- 必須:
  - `cargo check --workspace`
- 手動確認シナリオ:
  - Familiar セクションの折りたたみ/展開
  - Familiar の `-` / `+` でヘッダーの `{現在/最大}` が即時更新されること
  - Familiar 配下 Soul の追加/削除/並び替え
  - `Unassigned Souls` の折りたたみ/展開と差分更新
  - 行クリックによる選択とカメラフォーカス
  - Tab / Shift+Tab の巡回挙動
- パフォーマンス確認（必要時）:
  - Familiar 数・Soul 数を増やした状態で Entity List 更新時に不要な全再生成が増えていないか確認する

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1, M2, M3 を独立コミットとして戻せるようにする。
- 戻す時の手順:
  - M3 が不安定なら interaction 分割だけを戻し、M1/M2 の helper 移設は維持する。
  - M2 が不安定なら root 側 `spawn/sync` 実装へ戻し、asset trait 追加だけは残してもよい。
  - M1 で問題が出た場合は `GameAssets` 直接参照へ戻してから再設計する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `75%`
- 完了済みマイルストーン:
  - M1: Entity List 用 asset abstraction 導入
  - M2: Familiar/Soul の spawn/sync helper を `hw_ui::list` へ移設
  - M3: interaction の責務分離
- 未着手/進行中:
  - 手動 UI 検証のみ未完了

### 次のAIが最初にやること

1. `docs/cargo_workspace.md` と `docs/entity_list_ui.md` の境界記述を再確認する。
2. `cargo run` で Entity List の手動確認項目を消化する。
3. 問題がなければ本計画を `Completed` または archive に移す。

### ブロッカー/注意点

- `view_model.rs` は `Familiar` / `DamnedSoul` / `AssignedTask` 依存が強く、今回の移設対象ではない。
- `entity_list_section_toggle_system` は `hw_ui` へ移設済み。
- `UiIntent::AdjustMaxControlledSoulFor(Entity, isize)` を追加し、list button と dialog の更新を `handle_ui_intent` に集約済み。

### 参照必須ファイル

- `docs/cargo_workspace.md`
- `docs/entity_list_ui.md`
- `src/interface/ui/README.md`
- `src/interface/ui/list/view_model.rs`
- `src/interface/ui/list/sync.rs`
- `src/interface/ui/list/interaction.rs`
- `crates/hw_ui/src/list/models.rs`
- `crates/hw_ui/src/list/spawn.rs`
- `crates/hw_ui/src/list/sync.rs`
- `crates/hw_ui/src/list/section_toggle.rs`
- `crates/hw_ui/src/intents.rs`

### 最終確認ログ

 - 最終 `cargo check`: `2026-03-11` / `pass`
- 未解決エラー:
  - なし（手動 UI 検証は未実施）

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-11` | `Codex` | 初版作成 |
| `2026-03-11` | `Codex` | M1/M2 完了、M3 一部着手に合わせてステータスと引継ぎメモを更新 |
| `2026-03-11` | `Codex` | M3 実装完了。`UiIntent` 経由で max soul 更新経路と optimistic header update を一本化 |
