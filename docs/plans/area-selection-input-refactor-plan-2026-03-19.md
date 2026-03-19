# Area Selection Input Refactor Plan

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `area-selection-input-refactor-plan-2026-03-19` |
| ステータス | `Draft` |
| 作成日 | `2026-03-19` |
| 最終更新日 | `2026-03-19` |
| 作成者 | `Copilot` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

> **コードサーベイ基準日**: 2026-03-19。`README.md`、`docs/DEVELOPMENT.md`、`docs/README.md`、`docs/architecture.md`、`docs/cargo_workspace.md` と、`systems/command/area_selection/**` / `hw_ui::area_edit` / `hw_ui::camera` の実コードを確認済み。全関数の行番号・型依存・import パスを突き合わせ済み。

## 1. 目的

- 解決したい課題: `area_selection` の入力フローが root adapter として肥大化し、mode ごとの副作用と helper ownership が混在している。
  - `crates/bevy_app/src/systems/command/area_selection/input.rs` が UI ガード、ドラッグ開始、ドラッグ中更新、モード遷移、Dream 植林開始を同居させており、root adapter として厚すぎる。
  - `crates/bevy_app/src/systems/command/area_selection/input/release.rs` の `handle_left_just_released_input` が AreaSelection、Designation、CancelDesignation、DreamPlanting の 4 系統を 1 関数で処理しており、Query 群と副作用の対応関係が追いにくい。
  - `crates/bevy_app/src/systems/command/area_selection/geometry.rs` が `hw_ui::camera::world_cursor_pos` と重複するカーソル座標取得を持ち、`AreaEditOperation` / `AreaEditDrag` に紐づく helper が root に残っている。
- 到達したい状態:
  - root 側の `area_selection` は「入力 orchestration / ECS apply 呼び出し / visual」だけを持ち、純粋な area-edit helper は `hw_ui` に寄せる。
  - press / drag / release の責務を分離し、release 側は mode ごとの小さな handler に分割する。
  - `task_area_selection_system` の公開シグネチャと既存挙動を維持したまま内部構造だけを整理する。
- 成功指標:
  - `input.rs` を facade 化し、mode ごとの責務を別モジュールへ分割できる。
  - `release` 側の mode 分岐が単一巨大 match ではなく、専用 handler に分離される。
  - `hw_ui::camera::world_cursor_pos` と area-edit helper の ownership が整理され、root 側の重複が消える。
  - `cargo check --workspace` が成功する。

## 2. スコープ

### 対象（In Scope）

- `crates/bevy_app/src/systems/command/area_selection/input.rs` の分割と facade 化
- `crates/bevy_app/src/systems/command/area_selection/input/release.rs` の mode 別 handler への分割
- `crates/bevy_app/src/systems/command/area_selection/cursor.rs` / `geometry.rs` の helper ownership 整理
- `crates/hw_ui/src/area_edit/` への area-edit helper 追加
- `crates/bevy_app/src/systems/command/README.md` と、必要なら `docs/architecture.md` / `docs/cargo_workspace.md` の境界説明更新

### 非対象（Out of Scope）

- `TaskMode` や Dream 植林仕様そのものの変更
- `apply.rs` の designation / manual haul / cancel のゲームロジック変更
- `zone_placement` / `assign_task` / building placement 系の refactor
- 新規 crate (`hw_command` 等) の追加
- visual 表現やショートカット仕様の変更

## 3. 現状とギャップ（コードサーベイ結果）

### 3-1. 型・モジュール構造の現状

```
hw_ui::area_edit::state   ← AreaEditSession, AreaEditDrag(=Drag), AreaEditOperation(=Operation),
                              AreaEditHandleKind, AreaEditHistory, AreaEditClipboard, AreaEditPresets
hw_ui::camera             ← world_cursor_pos (pub fn, l.8-20)  ← ★正規入口

crate::systems::command::AreaEditHandleKind
                          ← pub use hw_ui::area_edit::AreaEditHandleKind (mod.rs l.48) ← 再エクスポートのみ

area_selection/state.rs   ← pub use hw_ui::area_edit::*  +  type alias Drag/Operation (6 行のみ)
area_selection/geometry.rs
  ├── world_cursor_pos    (l.60-74)  ← hw_ui::camera::world_cursor_pos と完全重複 (★削除対象)
  ├── detect_area_edit_operation (l.76-113) ← hw_ui 型のみ依存 (★移動対象)
  ├── apply_area_edit_drag       (l.115-163) ← hw_ui 型 + hw_core のみ依存 (★移動対象)
  ├── cursor_icon_for_operation  (l.166-190) ← hw_ui 型 + Bevy CursorIcon (★移動対象)
  ├── hotkey_slot_index / area_from_center_and_size / get_indicator_color
  ├── clamp_area_to_site  (★root に残す: Site, TaskArea が必要)
  └── in_selection_area   (★root に残す: TaskArea + AREA_CONTAINS_MARGIN)

area_selection/input.rs   ← 単一ファイル (l.1-319), mod release; のみ submod
  ├── should_exit_after_apply   (l.32-34)
  ├── reset_designation_mode    (l.36-44)
  ├── try_start_direct_edit_drag (l.46-76)
  ├── despawn_selection_indicators (l.78-85)
  ├── handle_active_drag_input  (l.87-174)
  ├── handle_left_just_pressed_input (l.176-226)
  └── task_area_selection_system (l.228-319) ← pub, 2×ParamSet システム

area_selection/input/release.rs ← handle_left_just_released_input (l.1-233, ~230行)
  ├── TaskMode::AreaSelection (l.27-62)   ~36行
  ├── TaskMode::Designate*   (l.63-72)   ~10行  ← 3種 match arm を1つにまとめている
  ├── TaskMode::CancelDesignation (l.73-220) ~147行 ← 最重量 (点キャンセル+範囲キャンセル混在)
  │    ├── 点キャンセル (l.77-177): DesignationTarget 15-tuple 2回イテレート + Floor/Wall site 探索
  │    └── 範囲キャンセル (l.178-218): apply_designation_in_area + Floor/Wall site 範囲探索
  └── TaskMode::DreamPlanting (l.222-230)  ~9行

area_selection/cursor.rs  ← world_cursor_pos を geometry.rs 経由で参照 (★hw_ui に切替)
```

### 3-2. 問題点（具体的）

- **重複 1**: `geometry.rs:60-74` の `world_cursor_pos` は `hw_ui::camera.rs:8-20` と本文が完全一致。
- **重複 2**: `geometry.rs` の 3 関数 (`detect_area_edit_operation` / `apply_area_edit_drag` / `cursor_icon_for_operation`) が参照する型は全て `hw_ui` または `hw_core` 由来であり、root crate の型に依存しない。それでも root 側 `geometry.rs` に置かれている。
- **肥大 1**: `release.rs::handle_left_just_released_input` が 233 行。特に `CancelDesignation` 節 (147 行) は `DesignationTargetQuery` (15-tuple) を 2 回イテレートし、`ParamSet::p0/p1/p2` の借用をネストして取り回している。
- **肥大 2**: `input.rs` が 320 行。press / drag / transitions / facade の 4 つの責務が混在している。
- **難読**: `CancelDesignation` 点キャンセルの `q_targets.iter()` で 15 要素 destructure が 2 回登場し、パターン変数の多くが `_` で埋まっている。

### 3-3. 本計画で埋めるギャップ

- area-edit 操作 helper 3 関数 + `world_cursor_pos` を `hw_ui::area_edit::interaction` に移し、root の重複を除去する。
- `input.rs` を `input/mod.rs` に変換し、press / drag / transitions の各責務を独立 module に切り出す。
- `release.rs` の 4 モード分岐を専用ファイルに分割し、`CancelDesignation` の点/範囲経路を名前付き helper に抽出する。

## 4. 実装方針（高レベル）

- 方針:
  - 先に helper ownership を整理 (M1)、その後で `input` を module 化 (M2)、最後に `release` を分割 (M3) の順で進める。
  - `task_area_selection_system`、`task_area_edit_cursor_system`、`task_area_edit_history_shortcuts_system` など既存公開システム名は維持する。
  - gameplay 変更は避け、`apply.rs` / `cancel.rs` の副作用呼び出し順も維持する。
- 設計上の前提:
  - `clamp_area_to_site` と `in_selection_area` は `Site` / `TaskArea` 依存があり root 専用 helper として残す。
  - `detect_area_edit_operation` / `apply_area_edit_drag` / `cursor_icon_for_operation` は `hw_ui` / `hw_core` 型だけで完結するため `hw_ui::area_edit::interaction` に移動できる。
  - `world_cursor_pos` の正規入口は `hw_ui::camera::world_cursor_pos`。`geometry.rs:60-74` は削除し、`input.rs` / `cursor.rs` の呼び出し元を `hw_ui::camera::world_cursor_pos` に切り替える。
  - `bevy_app` 側 call site の `crate::systems::command::AreaEditHandleKind` はそのまま維持できるが、新設する `crates/hw_ui/src/area_edit/interaction.rs` の中では `crate::systems::command::*` は使えない。`hw_ui` 側では `super::{AreaEditDrag, AreaEditHandleKind, AreaEditOperation}` か `crate::area_edit::*` を使ってローカル型として import する。
- Bevy 0.18 API での注意点:
  - `viewport_to_world_2d` の戻り値処理と `Query::single()` の失敗時挙動は現状互換に保つ。
  - `ParamSet` や `Query` の借用順序を崩して `B0001` を導入しない。`release.rs` の `p0/p1/p2` 借用ブロックのスコープを変えない。
  - `NextState<PlayMode>` 更新タイミングと `TaskContext` の戻し方は click / drag 判定に直結するため、関数抽出だけで順序を変えない。

## 5. 期待効果

- 保守性:
  - mode ごとの副作用位置が固定され、今後の仕様追加時に差分を局所化できる。
  - root adapter と shared helper の境界が揃い、crate 方針とのズレを減らせる。
- レビュー性:
  - 1 関数 1 責務に近づき、release 時の分岐漏れや副作用順序をレビューしやすくなる。
  - Query と `Commands` の使用箇所が縮み、借用競合を見つけやすくなる。
- パフォーマンス:
  - CPU 最適化が主目的ではないが、重複 helper 呼び出しと不要な分岐ネストを減らすことで入力処理の見通しを改善できる。
  - 期待できる実行時改善は小さい。主な利益は保守性と将来の refactor 容易性。

## 6. マイルストーン

## M1: Helper ownership を整理する

- 変更内容:
  1. `crates/hw_ui/src/area_edit/interaction.rs` を新規作成し、以下の 3 関数を `geometry.rs` からコピー移動する。
     - `detect_area_edit_operation(area: &TaskArea, world_pos: Vec2) -> Option<AreaEditOperation>` (現 `geometry.rs:76-113`)
     - `apply_area_edit_drag(active_drag: &AreaEditDrag, current_snapped: Vec2) -> TaskArea` (現 `geometry.rs:115-163`)
     - `cursor_icon_for_operation(operation: AreaEditOperation, dragging: bool) -> CursorIcon` (現 `geometry.rs:166-190`)
  2. `hw_ui/src/area_edit/mod.rs` で `pub mod interaction; pub use interaction::{detect_area_edit_operation, apply_area_edit_drag, cursor_icon_for_operation};` を追加する。
  3. `geometry.rs` から重複 `world_cursor_pos` (l.60-74) を削除する。
  4. `geometry.rs` の残り関数 (`detect_area_edit_operation` / `apply_area_edit_drag` / `cursor_icon_for_operation`) を削除し、import を `hw_ui::area_edit` に切り替える。
  5. `input.rs` / `cursor.rs` / `indicator.rs` で `super::geometry::world_cursor_pos` を使っている箇所を `hw_ui::camera::world_cursor_pos` に切り替える。
     - `input.rs:5`: `use super::geometry::{apply_area_edit_drag, detect_area_edit_operation, world_cursor_pos};` → `world_cursor_pos` を除去して `use hw_ui::camera::world_cursor_pos;` を追加
     - `cursor.rs:1`: 同様に切り替え
     - `indicator.rs:1`: 同様に切り替え。`area_selection_indicator_system` と `dream_tree_planting_preview_system` の両方で参照しているため漏れなく更新する
  6. `state.rs` の type alias (`Drag`, `Operation`) を不要化する。M1 完了後は call site を `hw_ui::area_edit::{AreaEditDrag, AreaEditOperation}` へ切り替え、alias 行を削除する。
- 変更ファイル:
  - `crates/hw_ui/src/area_edit/mod.rs`
  - `crates/hw_ui/src/area_edit/interaction.rs`（新規）
  - `crates/bevy_app/src/systems/command/area_selection/geometry.rs`
  - `crates/bevy_app/src/systems/command/area_selection/cursor.rs`
  - `crates/bevy_app/src/systems/command/area_selection/input.rs`
  - `crates/bevy_app/src/systems/command/area_selection/indicator.rs`
  - `crates/bevy_app/src/systems/command/area_selection/state.rs`
- 完了条件:
  - [ ] `geometry.rs` に `world_cursor_pos` が存在しない。
  - [ ] `detect_area_edit_operation` / `apply_area_edit_drag` / `cursor_icon_for_operation` が `hw_ui::area_edit::interaction` に存在し、root 側から `hw_ui::area_edit::*` 経由で参照される。
  - [ ] `indicator.rs` も `hw_ui::camera::world_cursor_pos` を使っており、削除済み helper を参照しない。
  - [ ] root `geometry.rs` に残る関数が `hotkey_slot_index`, `area_from_center_and_size`, `get_indicator_color`, `clamp_area_to_site`, `in_selection_area` のみになる。
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## M2: `input.rs` を facade + phase module に分割する

- 現状: `input.rs`(320行) が `mod release;` のみを持つ単一ファイル。`input/` ディレクトリにはすでに `release.rs` が置かれている。
- 変更内容:
  1. `input.rs` を `input/mod.rs` へ変換（ファイルを移動）。
  2. `input/transitions.rs` を新規作成し、以下を移動する:
     - `should_exit_after_apply(keyboard: &ButtonInput<KeyCode>) -> bool` (現 `input.rs:32-34`)
     - `reset_designation_mode(mode: TaskMode) -> TaskMode` (現 `input.rs:36-44`)
  3. `input/press.rs` を新規作成し、以下を移動する:
     - `try_start_direct_edit_drag(...)-> bool` (現 `input.rs:46-76`)
     - `handle_left_just_pressed_input(...)-> bool` (現 `input.rs:176-226`)
  4. `input/drag.rs` を新規作成し、以下を移動する:
     - `handle_active_drag_input(...)-> bool` (現 `input.rs:87-174`)
     - `despawn_selection_indicators(...)` (現 `input.rs:78-85`) は `drag.rs` か `mod.rs` のどちらかに置く（`release.rs` からも呼ばれるため、`mod.rs` の `pub(super) fn` として残す方が import が少なくて済む）
  5. `input/mod.rs` には `task_area_selection_system` (facade) と `despawn_selection_indicators` のみを残す。各 submod の `use` は `pub(super)` 関数の呼び出しで完結させる。
- 変更後のモジュール構造:
  ```
  input/
    mod.rs         ← task_area_selection_system + despawn_selection_indicators
    press.rs       ← try_start_direct_edit_drag + handle_left_just_pressed_input
    drag.rs        ← handle_active_drag_input
    transitions.rs ← should_exit_after_apply + reset_designation_mode
    release.rs     ← handle_left_just_released_input (M3 で分割予定)
  ```
- 変更ファイル:
  - `crates/bevy_app/src/systems/command/area_selection/input.rs` → `input/mod.rs` (移動・削除)
  - `crates/bevy_app/src/systems/command/area_selection/input/press.rs`（新規）
  - `crates/bevy_app/src/systems/command/area_selection/input/drag.rs`（新規）
  - `crates/bevy_app/src/systems/command/area_selection/input/transitions.rs`（新規）
  - `crates/bevy_app/src/systems/command/area_selection.rs` (mod input の path 変更は不要; Rust は `input.rs` と `input/mod.rs` を同一パスとして扱う)
- 完了条件:
  - [ ] `input.rs` が存在せず `input/mod.rs` に置き換わっている。
  - [ ] `task_area_selection_system` が `mod.rs` の facade として残る。
  - [ ] press / drag / transitions の分岐が専用 module に移り、`mod.rs` が 100 行以下になる。
  - [ ] `TaskMode::DreamPlanting` 開始時の `dream_planting_preview_seed` セット処理が維持される。
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## M3: release 処理を mode 別 handler に分割する

- 変更内容:
  1. `release.rs` を `release/mod.rs` に変換し、`handle_left_just_released_input` を dispatcher に縮退させる。
  2. `release/area.rs` を新規作成し、`AreaSelection` 分岐 (現 `release.rs:27-62`, ~36 行) を移す。
  3. `release/designation.rs` を新規作成し、`DesignateChop/Mine/Haul` 分岐 (現 `release.rs:63-72`, ~10 行) を移す。
  4. `release/cancel.rs` を新規作成し、`CancelDesignation` 分岐 (現 `release.rs:73-220`, ~147 行) を移す。
     さらに内部を以下の named helper に分割する:
     - `cancel_point_nearest_designation(commands, start_pos, q_targets)`: 点キャンセル時の `DesignationTargetQuery` 最近傍探索 + `cancel_single_designation` 呼び出し (現 l.79-141)
     - `cancel_point_construction_site(commands, start_pos, q_floor_tiles, q_wall_tiles)`: 点キャンセル時の Floor/Wall site (現 l.143-177)
     - `cancel_area_designations(commands, area, selected_entity, q_targets)`: 範囲取消の designation 適用 (現 l.181-188)
     - `cancel_area_construction_sites(commands, area, q_floor_tiles, q_wall_tiles)`: 範囲取消の Floor/Wall site (現 l.190-217)
     これにより 15-tuple `_` パターンの繰り返しが named helper に隠蔽される。
  5. `release/dream.rs` を新規作成し、`DreamPlanting` 分岐 (現 `release.rs:222-230`, ~9 行) を移す。
- 変更後のモジュール構造:
  ```
  input/
    release/
      mod.rs          ← handle_left_just_released_input (dispatcher のみ)
      area.rs         ← handle_release_area_selection
      designation.rs  ← handle_release_designation
      cancel.rs       ← handle_release_cancel_designation (+ 4 helper)
      dream.rs        ← handle_release_dream_planting
  ```
- 変更ファイル:
  - `crates/bevy_app/src/systems/command/area_selection/input/release.rs` → `release/mod.rs`
  - `crates/bevy_app/src/systems/command/area_selection/input/release/area.rs`（新規）
  - `crates/bevy_app/src/systems/command/area_selection/input/release/designation.rs`（新規）
  - `crates/bevy_app/src/systems/command/area_selection/input/release/cancel.rs`（新規）
  - `crates/bevy_app/src/systems/command/area_selection/input/release/dream.rs`（新規）
- 完了条件:
  - [ ] `release/mod.rs` の `handle_left_just_released_input` が dispatcher (match arm が 1-3 行程度) に縮退する。
  - [ ] `CancelDesignation` の点/範囲経路が named helper に分離され、15-tuple destructure が helper 内に隠蔽される。
  - [ ] `apply_area_and_record_history` / `apply_designation_in_area` / `cancel_single_designation` の呼び出し順が現状互換のまま維持される。
  - [ ] `ParamSet::p0/p1/p2` の借用スコープが壊れていない（`B0001` が出ない）。
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## M4: docs と境界説明を同期する

- 変更内容:
  - `systems/command/README.md` に input facade 化と helper ownership の更新を反映する。
  - `hw_ui::area_edit::interaction` 追加に伴い `docs/architecture.md` と `docs/cargo_workspace.md` の該当箇所を同期する。
  - `docs/plans/README.md` の索引を更新する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/command/README.md`
  - `docs/architecture.md`
  - `docs/cargo_workspace.md`
  - `docs/plans/README.md`
- 完了条件:
  - [ ] 実装境界の説明がコード構造と一致する。
  - [ ] `docs/plans/README.md` に本計画が掲載される。
- 検証:
  - `python scripts/update_docs_index.py` (存在する場合)
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| click と drag の分岐条件を変えてしまう | エリア選択・取消の UX 回帰 | `start_pos.distance(end_pos) < 0.1` (AreaSelection) と `< TILE_SIZE * 0.5` (Cancel) の閾値はそのまま残し、抽出後に値を変更しない |
| `ParamSet` 分割で借用競合を導入する | 実行時パニック (`B0001`) | `release/cancel.rs` の `p0/p1/p2` 借用ブロックを helper 関数に渡す際は `&mut ParamSet<...>` ごと渡さず、`.p0()` / `.p1()` を呼び出した結果の一時参照を渡す。スコープを変えない |
| `detect_area_edit_operation` の移動で import が循環する | コンパイルエラー | `hw_ui::area_edit::interaction` は `hw_core` / Bevy 型のみに依存させる。`Site` / `TaskMode` を持ち込まない |
| helper を `hw_ui` に寄せすぎて root 依存を逆流させる | crate 境界崩壊 | `Commands` / `Query` / `Site` / `PlayMode` / `TaskMode` 依存は root に残す。`TaskArea` + `AreaEdit*` だけで表現できる関数のみ移す |
| `state.rs` の型エイリアス (`Drag`, `Operation`) が残るかどうか | 混乱 | M1 完了後は `input.rs` 内の `use super::state::{Drag, ...}` を `use hw_ui::area_edit::{AreaEditDrag as Drag, ...}` に変更し、`state.rs` の alias 行を削除する (6行ファイルが 3行になる) |
| docs 更新がコードとずれる | 次回作業で誤読を招く | M4 を必須マイルストーンにし、`architecture.md` / `cargo_workspace.md` / `systems/command/README.md` を同時更新する |

## 8. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- 手動確認シナリオ:
  - Familiar を選択して task area を新規ドラッグ作成する。
  - 既存 task area の辺・角・中央をドラッグし、resize / move が維持される。
  - Chop / Mine / Haul designation をドラッグ指定し、モード継続条件（Shift）を確認する。
  - CancelDesignation で **単点クリック**（距離 < `TILE_SIZE * 0.5`）と **範囲ドラッグ**取消の両方を確認する。
  - CancelDesignation で FloorTileBlueprint / WallTileBlueprint のある site 上を点クリックし、`FloorConstructionCancelRequested` / `WallConstructionCancelRequested` が発行されることを確認する。
  - DreamPlanting の preview seed と確定結果が一致することを確認する（`pending_dream_planting` の seed が `dream_planting_preview_seed` と同値）。
  - cursor icon が move / resize ハンドル位置に応じて正しく変わることを確認する。
- パフォーマンス確認（必要時）:
  - 高負荷計測は不要。入力連打時にフレーム落ちやカーソル遅延が悪化しないことを目視確認する。

## 9. ロールバック方針

- どの単位で戻せるか:
  - M1 は helper ownership 移動だけを単独で戻せる。
  - M2 は `input` module 化だけを戻せる。
  - M3 は release handler 分割だけを戻せる。
- 戻す時の手順:
  1. `input/mod.rs` を単一ファイル構成へ戻す。
  2. `hw_ui::area_edit` に移した helper を root `geometry.rs` へ戻す。
  3. `cargo check --workspace` で import / module path の整合を確認する。

## 10. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1` `M2` `M3` `M4`

### 次のAIが最初にやること

1. `crates/bevy_app/src/systems/command/area_selection/geometry.rs` の `detect_area_edit_operation` (l.76) / `apply_area_edit_drag` (l.115) / `cursor_icon_for_operation` (l.166) の import を確認し、root crate 型への依存がないことを再確認する。
2. `crates/hw_ui/src/area_edit/interaction.rs` を新規作成し、上記 3 関数を移動する（M1 最初の一手）。
3. `geometry.rs:60-74` の `world_cursor_pos` を削除し、呼び出し元の `input.rs` / `cursor.rs` / `indicator.rs` を `hw_ui::camera::world_cursor_pos` に切り替える。
4. M1 完了後 `cargo check --workspace` を実行して安全確認してから M2 に進む。

### ブロッカー/注意点

- `TaskMode` の戻し方と `NextState<PlayMode>` のセット順は UX に直結するため、順序変更を避ける。
- `CancelDesignation` の `drag_distance < TILE_SIZE * 0.5` 閾値と `dist < TILE_SIZE` 近傍探索半径は変更しない。`release/cancel.rs` の helper に引数として渡すか定数として定義する。
- `CancelDesignation` では `DesignationTargetQuery` (15-tuple) の p0 借用と FloorTileBlueprint の p1、WallTileBlueprint の p2 借用がネストしている。named helper に分けるときは `p0()` の結果を先に使い切ってから `p1()` を呼ぶ現状のスコープを保つこと。
- `DreamPlanting` は `pending_dream_planting` と `dream_planting_preview_seed` のペアで成立している。`release/dream.rs` に移動するときに `area_edit_session.dream_planting_preview_seed.take()` の `.take()` を忘れない。
- `state.rs` の type alias (`Drag`, `Operation`) は M1 完了後に `input.rs` 側の import を直接 `hw_ui::area_edit` に切り替え、alias 行を削除して `state.rs` を 3 行に整理する。

### 参照必須ファイル

- `crates/bevy_app/src/systems/command/README.md`
- `crates/bevy_app/src/systems/command/area_selection.rs`
- `crates/bevy_app/src/systems/command/area_selection/input.rs` (l.1-319)
- `crates/bevy_app/src/systems/command/area_selection/input/release.rs` (l.1-233)
- `crates/bevy_app/src/systems/command/area_selection/geometry.rs` (l.1-195)
- `crates/bevy_app/src/systems/command/area_selection/cursor.rs`
- `crates/bevy_app/src/systems/command/area_selection/apply.rs`
- `crates/bevy_app/src/systems/command/area_selection/state.rs`
- `crates/bevy_app/src/systems/command/area_selection/queries.rs`
- `crates/hw_ui/src/area_edit/mod.rs`
- `crates/hw_ui/src/area_edit/state.rs`
- `crates/hw_ui/src/camera.rs` (world_cursor_pos の正規入口)
- `docs/architecture.md`
- `docs/cargo_workspace.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-19` / `pass`
- 未解決エラー: `N/A`

### Definition of Done

- [ ] M1〜M4 が完了している
- [ ] `geometry.rs` に `world_cursor_pos` / `detect_area_edit_operation` / `apply_area_edit_drag` / `cursor_icon_for_operation` が存在しない
- [ ] `input/` が `mod.rs` / `press.rs` / `drag.rs` / `transitions.rs` / `release/` の構造になっている
- [ ] `release/` が `mod.rs` (dispatcher) / `area.rs` / `designation.rs` / `cancel.rs` / `dream.rs` の構造になっている
- [ ] docs の境界説明がコードと一致している
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功している

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-19` | `Copilot` | コードサーベイ結果を反映。行番号・型依存・モジュール構造図・named helper 分割方針を追加 |
| `2026-03-19` | `Codex` | レビュー反映。M1 の `indicator.rs` / `state.rs` 更新漏れと `hw_ui` 側 import path 注意点を追記 |
