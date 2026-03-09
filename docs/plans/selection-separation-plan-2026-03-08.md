# selection 分離 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `selection-separation-plan-2026-03-08` |
| ステータス | `Completed` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-09` |
| 作成者 | `AI` |
| 関連提案 | `docs/proposals/selection-separation-2026-03-08.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  - `crates/hw_ui/src/selection/mod.rs` は state resource と shared 型を持つが、selection の副作用実行は引き続き root 側に残っている。
  - `src/interface/selection/input.rs` は hover 判定、entity 選択、`TaskContext` 更新、使い魔移動命令まで同一システムで処理している。
  - `src/interface/selection/building_place/placement.rs` と `src/systems/visual/placement_ghost.rs` が、建築配置の占有判定・描画位置計算・door/bridge/tank 条件を別実装で持っている。
  - `src/interface/selection/building_move/mod.rs` は preview 判定、状態遷移、`TransportRequest` cleanup、`unassign_task(...)`、`MovePlantTask` 生成を抱えており、分離の最後のボトルネックになっている。
- 到達したい状態:
  - `hw_ui::selection` が cursor 正規化、配置 footprint、preview/validation 結果、selection intent の共有モデルを持つ。
  - root 側の `src/interface/selection/` は snapshot 作成と `Commands` / `WorldMapWrite` / `NextState<PlayMode>` 適用に責務を絞る。
  - preview 系 (`src/systems/visual/placement_ghost.rs`, `src/interface/selection/building_move/preview.rs`) と commit 系が同じ判定ロジックを共有する。
- 成功指標:
  - `crates/hw_ui/src/selection/` が shared module として機能し、`mod.rs` は facade と再エクスポート中心になる。
  - `src/interface/selection/building_place/*` と `src/systems/visual/placement_ghost.rs` が同じ配置判定 API を使う。
  - `src/interface/selection/building_move/mod.rs` から geometry/validation が分離され、副作用中心の adapter に縮小される。
  - `cargo check -p hw_ui` と `cargo check --workspace` が成功する。

## 1.5. 実装戦略（実行順）

1. 最初に共有型と trait 境界だけを追加し、挙動変更なしで root wrapper を用意する。
2. duplication が明確な `building_place` と `floor_place` の preview/validation から共通化する。
3. 次に `input.rs` / `hit_test.rs` の分岐を outcome 化し、selection 意図と root の state mutation を切り分ける。
4. `building_move` は最後に着手し、preview/validation だけ先に共有化し、`unassign_task` を伴う commit は root 残留にする。
5. 全フェーズで `src/interface/ui/plugins/core.rs`, `src/plugins/input.rs`, `src/interface/ui/plugins/tooltip.rs` の system 順序を維持する。

## 2. スコープ

### 対象（In Scope）

- `crates/hw_ui/src/selection/` の拡張と、配下 submodule の新設
- `src/interface/selection/` の wrapper / adapter 化
- `src/systems/visual/placement_ghost.rs` の preview 判定共有化
- `src/interface/ui/plugins/core.rs`, `src/interface/ui/plugins/tooltip.rs`, `src/plugins/input.rs`, `src/plugins/startup/mod.rs` の配線更新
- `docs/architecture.md`, `docs/cargo_workspace.md`, `docs/README.md`, `docs/plans/README.md` の同期

### 非対象（Out of Scope）

- `PanCamera` や `MainCamera` の入力設計の全面変更
- `TaskArea` / `AreaSelection` ショートカット体系の再設計
- `soul_ai`, `tasks`, `pathfinding`, `WorldMap` 本体のアルゴリズム変更
- selection の新 UI / 新演出 / 新 UX 追加
- `hw_selection` のような新規 crate 追加を前提とした大規模 workspace 再編

## 3. 現状とギャップ

### 3.1 現在の責務分布

| 領域 | 主ファイル | 現在の責務 | 分離先の方向 |
| --- | --- | --- | --- |
| 選択 input / hover | `src/interface/selection/input.rs`, `src/interface/selection/hit_test.rs`, `src/interface/selection/placement_common.rs` | カメラ座標変換、hover 判定、選択確定、`TaskContext` / `Destination` 更新 | outcome 生成は `hw_ui::selection`、実 resource 更新は root |
| building placement rule | `crates/hw_ui/src/selection/placement.rs`, `src/interface/selection/building_place/placement.rs` | shared geometry / validation と Blueprint spawn / occupancy 予約 | geometry + validation は shared、spawn は root |
| building placement preview | `src/systems/visual/placement_ghost.rs` | カーソル取得、footprint 計算、配置可否判定、ghost 描画 | 判定は shared、ghost 描画だけ root |
| floor / wall placement | `crates/hw_ui/src/selection/placement.rs`, `src/interface/selection/floor_place/validation.rs`, `input.rs`, `floor_apply.rs`, `wall_apply.rs` | shared tile validation / area helper と drag 状態、site / tile spawn | validation + area model は shared、spawn と tooltip は root |
| building move preview | `crates/hw_ui/src/selection/placement.rs`, `src/interface/selection/building_move/placement.rs`, `preview.rs` | shared geometry / validation と ghost 更新 | geometry + validation は shared、ghost 描画は root |
| building move commit | `src/interface/selection/building_move/mod.rs` | state clear, `TransportRequest` cleanup, `unassign_task`, `MovePlantTask` 生成 | root 残留 |
| selection state | `crates/hw_ui/src/selection/mod.rs`, `src/interface/selection/state.rs` | `SelectedEntity`, `HoveredEntity`, `SelectionIndicator` 定義・再エクスポート | 維持しつつ shared model の入口にする |

### 3.2 問題

- `building_place` と `placement_ghost_system` の validation 分岐が二重化しており、Door/Bridge/Tank companion 条件の差分事故を起こしやすい。
- `floor_place` は `validation.rs` が比較的純粋だが、area 正規化と reject reason 集約が `input.rs` / `floor_apply.rs` / `wall_apply.rs` に分散している。
- `building_move/placement.rs` は純粋寄りなのに、`preview.rs` と `mod.rs` が別々に cursor -> grid -> validation を繰り返している。
- `input.rs` は「selection decision」と「ゲーム状態変更」を同じ if/match で処理しているため、テストしづらく root 依存も減らせない。
- `hw_ui` 側 selection モジュールが state resource しか持たず、`hw_ui` 分離後の follow-up 受け皿になっていない。

### 3.3 本計画で埋めるギャップ

- `hw_ui::selection` に shared model / traits / intent を追加し、selection ロジックを root から一段薄く分離する。
- preview と commit が同じ validation 経路を使うようにし、配置判定の二重実装を解消する。
- `building_move` のうち root 専有でない部分だけを先に抽出し、`unassign_task` / request cleanup を伴う commit は adapter として残す。
- `Selection` の single source of truth を増やさず、`SelectedEntity` / `HoveredEntity` / `TaskContext` / `MoveContext` の mutation は root のみが担当する。

## 4. 実装方針（高レベル）

- 方針:
  - `selection` を一度に crate 移動せず、`hw_ui::selection` を shared logic 層として育て、root 側を adapter 化する。
  - read-only な geometry / validation / intent resolution から抽出し、`Commands` / `WorldMapWrite` / `NextState<PlayMode>` を使う commit 系は最後に整理する。
  - preview と commit の両方が使う型を先に固定し、後続フェーズで side effect 実装を差し替えても判定契約が崩れないようにする。
- 設計上の前提:
  - `SelectedEntity`, `HoveredEntity`, `SelectionIndicator` は現状どおり `hw_ui::selection` に置き、root は再エクスポートで利用する。
  - `TaskContext`, `BuildContext`, `MoveContext`, `CompanionPlacementState`, `PlacementFailureTooltip` は root resource のまま保持する。
  - `WorldMap::world_to_grid`, `grid_to_world`, `snap_to_grid_edge`, `snap_to_grid_center` の呼び出しは shared model に閉じず、必要なら trait / snapshot 経由で正規化する。
- Bevy 0.18 APIでの注意点:
  - `src/plugins/input.rs` の `handle_mouse_input.run_if(in_state(PlayMode::Normal))` を壊さない。
  - `src/interface/ui/plugins/tooltip.rs` の `hover_tooltip_system.after(update_selection_indicator).before(blueprint_placement)` を維持する。
  - `src/interface/ui/plugins/core.rs` の selection 系 system 群は `GameSystemSet::Interface` の順序を維持し、unordered tuple へ戻さない。
  - `viewport_to_world_2d` を呼ぶ箇所は cursor 正規化 helper に寄せるが、Bevy 0.18 の戻り値 (`Result<Vec2, _>`) に合わせて `None` / `Invalid` を安全側へ倒す。

### 4.1 採用する shared contract

| 区分 | 置き場所 | 内容 |
| --- | --- | --- |
| state resource | `crates/hw_ui/src/selection/mod.rs` | `SelectedEntity`, `HoveredEntity`, `SelectionIndicator` を維持 |
| cursor / input snapshot | `hw_ui::selection` 新規 submodule | world cursor, hovered target, selected target, play/task mode の参照用 snapshot |
| placement model | `hw_ui::selection` 新規 submodule | footprint, draw_pos, validation result, reject reason, preview tint |
| selection intent | `hw_ui::selection` 新規 submodule | select / clear / start area selection / move familiar / commit placement / cancel placement などの outcome enum |
| read-only world trait | `hw_ui::selection` 新規 submodule | building / stockpile / walkable / river / floor completion / bucket storage 参照など |
| root adapter | `src/interface/selection/*` / `src/systems/visual/placement_ghost.rs` | Query/Res から snapshot 構築、shared result を `Commands` / resource 更新へ適用 |

### 4.2 Root に残す責務

- `WorldMapWrite` を伴う Blueprint / Site / Move task 生成
- `TransportRequest` cleanup と `unassign_task(...)`
- `NextState<PlayMode>` / `TaskContext` / `Destination` / `MoveContext` の最終 mutation
- ghost entity / selection indicator entity の spawn/despawn
- `UiInputState.pointer_over_ui` と `PanCamera` ガードの最終適用

## 5. マイルストーン

## M1: `hw_ui::selection` の shared model 骨格を追加

- 変更内容:
  - `crates/hw_ui/src/selection/mod.rs` を facade 化し、新規 submodule を公開する
  - `SelectionIntent`, `PlacementValidation`, `PlacementRejectReason`, `WorldReadApi` などの shared 型を追加する
  - root 側で使う shared placement / intent API の土台を定義する
  - root 側は `src/interface/selection/mod.rs` と `src/plugins/startup/mod.rs` の配線だけ更新し、既存挙動は変えない
- 変更ファイル:
  - `crates/hw_ui/src/selection/mod.rs`
  - `crates/hw_ui/src/selection/*.rs`（新規）
  - `src/interface/selection/mod.rs`
  - `src/plugins/startup/mod.rs`
- 完了条件:
  - [ ] `hw_ui::selection` に shared 型と trait 契約が追加されている
  - [ ] 既存の `SelectedEntity` / `HoveredEntity` / `SelectionIndicator` の公開 API が壊れない
  - [ ] root 側に duplicate state を増やしていない
- 検証:
  - `cargo check -p hw_ui`
  - `cargo check --workspace`

## M2: Building placement の preview / validation を shared 化

- 変更内容:
  - building placement の footprint / draw_pos / size / occupancy / door adjacency 判定を `hw_ui::selection::placement` へ寄せる
  - Door / Bridge / 2x2 建物 / Tank companion の配置可否判定を shared path にまとめる
  - `src/interface/selection/building_place/placement.rs` は `Commands` / `WorldMapWrite` / Blueprint spawn 専用の apply 層に寄せる
  - `src/systems/visual/placement_ghost.rs` は shared validation 結果を受けて ghost 表示だけ行う
- 変更ファイル:
  - `crates/hw_ui/src/selection/mod.rs`
  - `crates/hw_ui/src/selection/*.rs`
  - `src/interface/selection/building_place/mod.rs`
  - `src/interface/selection/building_place/placement.rs`
  - `src/interface/selection/building_place/flow.rs`
  - `src/systems/visual/placement_ghost.rs`
- 完了条件:
  - [ ] Door / Bridge / Tank / MudMixer / RestArea / WheelbarrowParking の preview と commit が同じ validation 経路を使う
  - [ ] Tank companion の範囲制約と rollback 挙動が維持される
  - [ ] `placement_ghost_system` から生の world 判定分岐が大幅に減っている
- 検証:
  - `cargo check --workspace`
  - `cargo run`
  - 建物配置: wall / door / tank / bridge の preview → commit → cancel

## M3: Floor / Wall placement の area model と reject reason を shared 化

- 変更内容:
  - `src/interface/selection/floor_place/validation.rs` の reject reason と tile 判定を shared helper に寄せ、floor / wall の area 正規化も shared helper に寄せる
  - `src/interface/selection/floor_place/input.rs` は drag 開始・release・cancel の adapter に縮小する
  - `floor_apply.rs` / `wall_apply.rs` は site / tile spawn と `PlacementFailureTooltip` 更新のみ担当する
  - 必要なら floor / wall 共通の `PlacementBatch` または tile list model を導入する
- 変更ファイル:
  - `crates/hw_ui/src/selection/mod.rs`
  - `crates/hw_ui/src/selection/*.rs`
  - `src/interface/selection/floor_place/mod.rs`
  - `src/interface/selection/floor_place/input.rs`
  - `src/interface/selection/floor_place/validation.rs`
  - `src/interface/selection/floor_place/floor_apply.rs`
  - `src/interface/selection/floor_place/wall_apply.rs`
- 完了条件:
  - [ ] floor / wall の tile reject reason が shared path で生成される
  - [ ] area size 上限、wall の `1xn` 制約、completed floor 必須条件が維持される
  - [ ] root 側の `TaskContext` リセットと `PlacementFailureTooltip` 表示が既存互換で動く
- 検証:
  - `cargo check --workspace`
  - `cargo run`
  - floor / wall drag の開始 → release → reject tooltip → cancel

## M4: 選択 input / hover 判定を outcome 化

- 変更内容:
  - `src/interface/selection/input.rs`, `hit_test.rs`, `placement_common.rs` を分離し、cursor 正規化と hit 判定結果から `SelectionIntent` を返す形へ寄せる
  - `src/plugins/input.rs` は引き続き input plugin と `PanCamera` guard を持ち、selection adapter 呼び出しだけ行う
  - familiar の area border 選択、worker 選択、右クリック移動を pure outcome と root mutation に分ける
- 変更ファイル:
  - `crates/hw_ui/src/selection/mod.rs`
  - `crates/hw_ui/src/selection/*.rs`
  - `src/interface/selection/input.rs`
  - `src/interface/selection/hit_test.rs`
  - `src/interface/selection/placement_common.rs`
  - `src/plugins/input.rs`
- 完了条件:
  - [ ] selection click 分岐が shared outcome enum 経由になっている
  - [ ] `SelectedEntity`, `TaskContext`, `NextState<PlayMode>`, `Destination` の mutation は root adapter に限定される
  - [ ] `UiInputState.pointer_over_ui` の guard が selection 系全体で一貫している
- 検証:
  - `cargo check --workspace`
  - `cargo run`
  - worker selection、task area border selection、familiar move command、UI 上 hover 無効化

## M5: Building move の preview / validation を shared 化し、commit を root adapter に閉じる

- 変更内容:
  - `src/interface/selection/building_move/geometry.rs` / `placement.rs` / `preview.rs` の read-only 部分を shared model へ移す
  - `src/interface/selection/building_move/mod.rs` は click 入力と root resource の state 遷移を扱う adapter とし、placement validation / geometry 本体は shared helper に委譲する
  - tank move companion 判定も shared validation に含めるが、`unassign_task(...)` と `MovePlantTask` 生成は root 残留にする
- 変更ファイル:
  - `crates/hw_ui/src/selection/mod.rs`
  - `crates/hw_ui/src/selection/*.rs`
  - `src/interface/selection/building_move/mod.rs`
  - `src/interface/selection/building_move/geometry.rs`
  - `src/interface/selection/building_move/placement.rs`
  - `src/interface/selection/building_move/preview.rs`
- 完了条件:
  - [ ] move preview と commit が同じ occupied / companion validation を使う
  - [ ] `building_move/mod.rs` に残るのが state clear / cleanup / task spawn など root 副作用中心になっている
  - [ ] tank companion move、MudMixer move の挙動が回帰しない
- 検証:
  - `cargo check --workspace`
  - `cargo run`
  - tank move、bucket storage companion move、MudMixer move、右クリックキャンセル

## M6: docs 同期と follow-up 整理

- 変更内容:
  - `docs/architecture.md`, `docs/cargo_workspace.md`, `docs/README.md` に selection の新境界を反映する
  - `docs/plans/hw-ui-crate-plan-2026-03-08.md` の follow-up 状況を更新する
  - `docs/plans/README.md` を再生成し、必要なら selection の残課題を Notes に補足する
  - 可能なら `hw_ui` selection model に unit test を追加する
- 変更ファイル:
  - `docs/architecture.md`
  - `docs/cargo_workspace.md`
  - `docs/README.md`
  - `docs/plans/hw-ui-crate-plan-2026-03-08.md`
  - `docs/plans/README.md`
  - `crates/hw_ui/src/selection/*.rs`（テスト追加時）
- 完了条件:
  - [ ] docs に root shell / shared logic / root-only side effect の境界が反映されている
  - [ ] follow-up として残る root 専有責務が文書化されている
  - [ ] `cargo check -p hw_ui` と `cargo check --workspace` が成功する
- 検証:
  - `python scripts/update_docs_index.py`
  - `cargo check -p hw_ui`
  - `cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| trait 境界が `WorldMap` の詳細を漏らしすぎて肥大化する | 高 | `WorldReadApi` / `BuildingPlacementContext` は occupancy / walkable / river / floor completion など read-only 最小 API に限定し、`Commands` / `WorldMapWrite` は入れない |
| preview と commit の座標正規化がずれて off-by-half-tile が出る | 高 | `draw_pos`, `anchor_grid`, `occupied_grids` を shared model の単一路径にまとめ、`placement_ghost_system` と commit 側で再計算しない |
| `building_move` 分離中に `unassign_task` cleanup を壊す | 高 | `cancel_tasks_and_requests_for_moved_building` は M5 まで root 残留、先に preview/validation だけを抜く |
| `TaskContext` / `MoveContext` / `CompanionPlacementState` の二重状態が生まれる | 高 | shared model は snapshot / outcome のみ保持し、永続 resource は root 既存 resource を single source of truth にする |
| plugin 順序変更で tooltip / ghost / selection indicator のタイミングが崩れる | 中 | `src/interface/ui/plugins/core.rs` と `src/interface/ui/plugins/tooltip.rs` の ordering edge を維持し、必要なら `.after()` を明示する |
| docs だけ先に古くなる | 中 | M6 で `architecture.md` / `cargo_workspace.md` / `hw-ui-crate-plan` を同時更新する |

## 7. 検証計画

- 必須:
  - `cargo check -p hw_ui`
  - `cargo check --workspace`
- 手動確認シナリオ:
  - worker / familiar の hover と selection が壊れない
  - task area border 選択から `PlayMode::TaskDesignation` への遷移が壊れない
  - 建物配置 preview と commit が同じ可否を返す
  - floor / wall 配置で reject reason と tooltip が期待どおり出る
  - tank / MudMixer の move preview / commit / cancel が壊れない
  - UI 上にポインタがあるとき selection / camera guard が正しく働く
- パフォーマンス確認（必要時）:
  - preview 系が毎フレーム余計な Query 走査や duplicate validation を増やしていないことを profiler / log で確認する

## 8. ロールバック方針

- どの単位で戻せるか:
  - M2, M3, M4, M5 を個別に revert できるようコミットを分ける
  - `hw_ui::selection` の shared model 追加だけであれば M1 単体で戻せる
- 戻す時の手順:
  1. 直前マイルストーンの adapter 切替コミットを revert する
  2. `src/interface/ui/plugins/core.rs` / `src/plugins/input.rs` の配線を旧 system に戻す
  3. `cargo check --workspace` で型整合を確認する
  4. docs を実際の境界に合わせて更新する

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1` `M2` `M3` `M4` `M5` `M6`

### Definition of Done

- [x] `hw_ui::selection` に shared model / trait / intent が追加されている
- [x] building / floor / move の preview と validation が shared 経路を使う
- [x] root 側 `selection` は adapter / side effect 中心に整理されている
- [x] 関連 docs が更新されている
- [x] `cargo check -p hw_ui` と `cargo check --workspace` が成功する

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
| `2026-03-08` | `AI` | `cargo check --workspace` 成功に合わせて最終確認ログを更新 |
| `2026-03-08` | `AI` | building placement shared logic の実装状況に合わせて進捗と引継ぎメモを更新 |
| `2026-03-08` | `ユーザー` | レビュー修正後 50% に差し戻し（M3/M5/M6 未着手） |
| `2026-03-08` | `AI` | M3（area validation shared 化）・M5（move geometry shared 化）・M6（docs 同期）完了 → 100% |
| `2026-03-09` | `AI` | tank companion preview/commit 再整合、floor/wall tile validation shared 化、docs index 同期 |
