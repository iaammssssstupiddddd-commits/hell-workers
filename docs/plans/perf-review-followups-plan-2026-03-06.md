# パフォーマンス改善フォローアップ計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `perf-review-followups-plan-2026-03-06` |
| ステータス | `Draft` |
| 作成日 | `2026-03-06` |
| 最終更新日 | `2026-03-06` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: スケール時に効きやすい全件走査・全UI再構築・線形 `contains` を残したままになっている。
- 到達したい状態: タスクUI、資材ソース探索、境界パス探索、資材数表示が差分更新または近傍探索ベースになり、Soul/Familiar/Item 数が増えても劣化が緩やかになる。
- 成功指標:
  - 500 Soul / 30 Familiar の perf scenario で、改善対象システムの処理量がフルスキャン前提から減っている。
  - `task_list_update_system` と `resource_count_display_system` が毎フレーム全件再構築を行わない。
  - `find_path_to_boundary` が `target_grids.len()` に比例した membership 判定を探索中に繰り返さない。

## 2. スコープ

### 対象（In Scope）

- タスクリスト UI の dirty gate 導入
- Familiar source selector の近傍探索化または差分キャッシュ化
- `find_path_to_boundary` のターゲット領域 membership 判定改善
- 資材数表示 UI の差分更新または低頻度更新化
- 必要に応じた perf scenario での再確認

### 非対象（Out of Scope）

- ゲーム全体の包括的な profiler 導入
- Room detection や pathfinding executor 全体の再設計
- 見た目だけの UI 文言変更
- 既存 archived plan の棚卸しや整理

## 3. 現状とギャップ

- 現状:
  - タスクリストは表示中毎フレーム `Designation` 全件から snapshot を再生成し、差分があると子 UI を全削除して再生成している。
  - Familiar の `source_selector` は委譲サイクルごとに free item / stockpile item を全走査してフレームキャッシュを作っている。
  - `find_path_to_boundary` は A* 中に `target_grids.contains(...)` を複数箇所で線形評価している。
  - 資材カウント表示は毎フレーム全 `ResourceItem` を集計してラベルを同期している。
- 問題:
  - 件数増加に対して `O(N)` または `O(N log N)` の処理が毎フレーム、または高頻度タイマーごとに残る。
  - UI が ECS 変更差分を活用しきれていない。
  - 既存の `ResourceSpatialGrid` などの基盤があるのに、探索系で活用できていない箇所がある。
- 本計画で埋めるギャップ:
  - 「常時全件処理」を「変更時のみ」「近傍のみ」「集合 membership を O(1) 化」に置き換える。

## 4. 実装方針（高レベル）

- 方針:
  - まず UI の dirty gate と再構築抑制を入れて、常時負荷を削る。
  - 次に source selector を `ResourceSpatialGrid` または位置バケットで絞り、全件走査の頻度を落とす。
  - その後で boundary pathfinding の `target_grids` を集合化し、探索中の membership 判定を改善する。
  - 最後に資材数表示を差分更新へ寄せる。差分実装が複雑すぎる場合は低頻度タイマー化を中間策とする。
- 設計上の前提:
  - Bevy ECS の `Changed` / `Added` / `RemovedComponents` を優先し、独自の全件同期を増やさない。
  - 既存の `ResourceSpatialGrid`、`TaskListState`、UI dirty resource を活用し、重複キャッシュを増やしすぎない。
  - perf 指標はまず処理量削減を重視し、見た目や挙動を変えない。
- Bevy 0.18 APIでの注意点:
  - UI 再構築抑制では `Changed<Interaction>` を読む既存システムの chain を壊さない。
  - Query 追加で `error[B0001]` を起こさないよう、既存 Query 群や `SystemParam` を優先再利用する。

## 5. マイルストーン

## M1: タスクリストとステータス UI の常時再集計を抑制

- 変更内容:
  - `task_list_update_system` に dirty gate を入れ、表示モード切替・Designation 関連変更・Priority/TaskWorkers 変更時のみ snapshot を再生成する。
  - 可能なら `task_summary_ui_system` も同じ dirty source を共有し、毎フレーム `count()` を避ける。
  - `update_mode_text_system` / `update_area_edit_preview_ui_system` の全 Query 走査は、選択変更または area 編集中に限定できるか確認する。
- 変更ファイル:
  - `src/interface/ui/panels/task_list/update.rs`
  - `src/interface/ui/panels/task_list/view_model.rs`
  - `src/interface/ui/list/change_detection.rs`
  - `src/interface/ui/interaction/status_display/mode_panel.rs`
  - `docs/plans/perf-review-followups-plan-2026-03-06.md`
- 完了条件:
  - [ ] タスクリストが表示中でも無変更フレームでは snapshot 再生成を行わない
  - [ ] UI の表示内容と既存挙動が変わらない
  - [ ] `cargo check` が通る
- 検証:
  - `cargo check`
  - タスク追加/完了/優先度変更/左パネル切替で UI が正しく更新されることを手動確認

## M2: Familiar source selector の全件走査を縮小

- 変更内容:
  - `source_selector` の free item 探索を `ResourceSpatialGrid` 利用、またはセル単位の近傍候補抽出へ置き換える。
  - stockpile item 側も、必要な resource type と候補セルに絞って索引化する。
  - 既存 perf counter は維持し、改善前後の scanned item の差分が比較できるようにする。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/task_management/policy/haul/source_selector.rs`
  - `src/systems/familiar_ai/decide/task_delegation.rs`
  - `src/systems/familiar_ai/decide/task_management/...`
  - `src/systems/spatial/resource.rs`
  - `docs/plans/perf-review-followups-plan-2026-03-06.md`
- 完了条件:
  - [ ] 資材探索が全 free item を毎回総当たりしない
  - [ ] 予約判定と owner 互換性判定が壊れていない
  - [ ] 既存の task assignment 挙動に回帰がない
- 検証:
  - `cargo check`
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`

## M3: 境界パス探索と資材数表示の軽量化

- 変更内容:
  - `find_path_to_boundary` の `target_grids` を `HashSet` などに変換し、探索中 membership 判定を O(1) 化する。
  - `resource_count_display_system` を差分更新型にする。差分化が大きい場合は低頻度タイマーを中間策として先に入れる。
  - 変更後に perf scenario で UI/visual 起因の負荷が悪化していないことを確認する。
- 変更ファイル:
  - `src/world/pathfinding.rs`
  - `src/systems/logistics/ui.rs`
  - `src/plugins/visual.rs`
  - `docs/plans/perf-review-followups-plan-2026-03-06.md`
- 完了条件:
  - [ ] `find_path_to_boundary` が線形 `contains` 連打をしない
  - [ ] 資材ラベル更新が毎フレーム全件再集計前提でなくなる
  - [ ] 見た目の挙動が維持される
- 検証:
  - `cargo check`
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| UI dirty 条件の漏れで表示更新が止まる | 中 | 変更トリガを `Added/Changed/Removed` ベースで列挙し、左パネル切替も明示トリガに含める |
| source selector の近傍化で遠距離 fallback が欠落する | 高 | 近傍候補が空のときだけ探索半径を段階拡張する |
| pathfinding の集合化で挙動差が出る | 中 | 既存テストに加えて 1x1 / 2x2 / 開始地点が target 内のケースを維持確認する |
| resource label の差分更新が複雑化する | 中 | まず低頻度更新で効果を見て、必要なら後続で厳密差分化する |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - タスクリストを開いたまま task を追加・完了・priority 変更する
  - Blueprint / stockpile / ground item が混在する状態で Familiar が正しく資材を拾う
  - 2x2 建物や construction site への移動で境界停止が崩れない
  - 地面資材の増減で count label が不整合を起こさない
- パフォーマンス確認（必要時）:
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`
  - 改善対象の scanned items / UI rebuild 頻度 / 体感 FPS を比較する

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1, M2, M3 を個別コミット単位で戻せるようにする
- 戻す時の手順:
  - 回帰が UI 限定なら M1 のみ戻す
  - 資材探索回帰なら M2 のみ戻す
  - 経路探索回帰なら M3 の pathfinding 部分のみ戻す

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン:
- 未着手/進行中:
  - M1: 未着手
  - M2: 未着手
  - M3: 未着手

### 次のAIが最初にやること

1. `task_list_update_system` と関連 dirty source を確認し、無変更フレームの snapshot 生成を止める
2. `source_selector` が `ResourceSpatialGrid` を直接使えるか、必要な owner/filter 条件を洗い出す
3. `find_path_to_boundary` の membership 判定と `resource_count_display_system` の更新頻度を削減する

### ブロッカー/注意点

- `docs/plans/README.md` には実ファイルが存在しない計画書エントリがあるため、別件で索引の整合確認が必要
- UI 系は chain と `Changed<Interaction>` の順序依存を壊さない
- source selector の予約ロジックは `ReservationShadow` と一体で確認する

### 参照必須ファイル

- `docs/DEVELOPMENT.md`
- `docs/architecture.md`
- `src/interface/ui/panels/task_list/update.rs`
- `src/interface/ui/interaction/status_display/mode_panel.rs`
- `src/systems/familiar_ai/decide/task_management/policy/haul/source_selector.rs`
- `src/world/pathfinding.rs`
- `src/systems/logistics/ui.rs`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-06` / `pass`
- 未解決エラー:

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-06` | `Codex` | 初版作成 |
