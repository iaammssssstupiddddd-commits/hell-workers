# command クレート分離計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `command-crate-extraction-plan-2026-03-12` |
| ステータス | `Archived` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `AI (Codex)` → ブラッシュアップ: `AI (Copilot)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

> **コードサーベイ基準日**: 2026-03-12。実コードを精読して関数シグネチャ・依存を確認済み。

## 1. 目的

- 解決したい課題: `src/systems/command/` には pure helper と app shell が混在しており、crate 境界が不明瞭なまま root crate に集約されている。
- 到達したい状態: `command` のうち共有モデル・純ロジック・world/logistics ドメイン判定を既存 crate に分離し、root 側は入力・camera・UI・`Commands` に依存する shell のみを保持する。
- 成功指標:
  - `TaskArea` / `TaskMode` に続く追加の `command` ロジックが既存 crate に移設される
  - root `src/systems/command/` の責務が「入力 orchestration / visual / ECS apply」に縮退する
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が継続して成功する

## 2. スコープ

### 対象（In Scope）

- `area_selection/geometry.rs` の pure helper を `hw_core::area` に移設
- `zone_placement/connectivity.rs` の `identify_removal_targets` を `hw_world::zones` に移設
- `zone_placement/placement.rs` の read-only geometry helper を `hw_world::zones` に移設
- manual haul 選定ロジックの view model 化 + `hw_logistics::manual_haul_selector` 新設
- `src/systems/command/mod.rs` の re-export / thin shell 整理
- crate 境界の docs 更新

### 非対象（Out of Scope）

- `AreaEditHandleKind` の `hw_core` への移設（`AreaEditHandleVisual` Component とセットのため、ROI が低い）
- `task_area_selection_system` / `zone_placement_system` / `zone_removal_system` 本体の即時移設
- `area_selection_indicator_system` / `designation_visual_system` など visual 系の crate 化
- 新規 `hw_command` crate の追加
- gameplay 仕様変更

## 3. 現状とギャップ（精査済み）

### 3-1. `area_selection/geometry.rs` の関数別分類

実際のシグネチャを確認し、以下のように分類する。

#### `hw_core::area` 移設可能（pure, TaskArea/Vec2/TILE_SIZE のみ依存）

| 関数名 | 現在の pub レベル | 移設理由 |
| --- | --- | --- |
| `wall_line_area(start: Vec2, end: Vec2) -> TaskArea` | `pub` | Vec2 と TILE_SIZE のみ |
| `area_from_center_and_size(center: Vec2, size: Vec2) -> TaskArea` | `pub(super)` | 同上 |
| `count_positions_in_area(area: &TaskArea, positions: impl Iterator<Item=Vec2>) -> usize` | `pub` | TaskArea.contains_with_margin のみ |
| `overlap_summary_from_areas(selected: Entity, selected_area: &TaskArea, areas: impl Iterator<Item=(Entity, TaskArea)>) -> Option<(usize, f32)>` | `pub` | Entity は Bevy プリミティブ、hw_core は bevy 依存あり |
| `get_drag_start(mode: TaskMode) -> Option<Vec2>` | `pub` | `TaskMode` は既に `hw_core::game_state` に存在 |

#### root 残留（Bevy ECS/window/cursor 依存）

| 関数名 | 残留理由 |
| --- | --- |
| `hotkey_slot_index(keyboard: &ButtonInput<KeyCode>) -> Option<usize>` | `ButtonInput<KeyCode>` |
| `get_indicator_color(mode: TaskMode, is_valid: bool) -> LinearRgba` | `LinearRgba`（Bevy カラー）、視覚的判定 |
| `clamp_area_to_site(area: &TaskArea, q_sites: &Query<&Site>) -> TaskArea` | `Query<&Site>` |
| `world_cursor_pos(...)` | Window + Camera |
| `detect_area_edit_operation(area: &TaskArea, pos: Vec2) -> Option<Operation>` | `Operation` は `AreaEditHandleKind` 依存（root 定義） |
| `apply_area_edit_drag(active_drag: &Drag, snapped: Vec2) -> TaskArea` | `Drag` struct が `Entity` を持つ root 型 |
| `cursor_icon_for_operation(op: Operation, dragging: bool) -> CursorIcon` | `CursorIcon`（Bevy window 型） |
| `in_selection_area(area: &TaskArea, pos: Vec2) -> bool` | `TaskArea.contains_with_margin` のラッパー。thin すぎて移設不要 |

### 3-2. `zone_placement/connectivity.rs` の分類

| 関数名 | 依存 | 判定 |
| --- | --- | --- |
| `identify_removal_targets(world_map: &WorldMap, area: &AreaBounds) -> (Vec<(i32,i32)>, Vec<(i32,i32)>)` | `WorldMap` + `AreaBounds` のみ | `hw_world` 移設可能 |

- `WorldMap::world_to_grid` と `WorldMap::stockpile_entries()` のみ使用。
- `hw_world::zones` に追加するか、`hw_world::zone_query` など新規 module を作るかは実装者判断でよい。
  既存 `hw_world::zones.rs` はコンポーネント定義が中心なので **`crates/hw_world/src/zone_ops.rs`（新設）** が望ましい。

### 3-3. `zone_placement/placement.rs` の関数別分類

#### `hw_world::zone_ops` 移設可能（`AreaBounds` / `Yard` / `Site` / `hw_world::coords` のみ）

| 関数名 | 移設先シグネチャ |
| --- | --- |
| `rectangles_overlap_site(area: &AreaBounds, site: &Site) -> bool` | そのまま |
| `rectangles_overlap(area: &AreaBounds, yard: &Yard) -> bool` | そのまま |
| `expand_yard_area(yard: &Yard, drag_area: &AreaBounds) -> AreaBounds` | そのまま |
| `area_tile_size(area: &AreaBounds) -> (usize, usize)` | `hw_world::coords::world_to_grid` が使えるのでそのまま |

#### root 残留（`Query<...>` / `Commands` 依存）

| 関数名 | 残留理由 |
| --- | --- |
| `is_stockpile_area_within_yards(area: &AreaBounds, q: &Query<(Entity, &Yard)>) -> bool` | Query 依存。内側のタイル判定ループは `hw_world` の helper を呼ぶ形に書き換え可 |
| `is_yard_expansion_area_valid(start: Vec2, area: &AreaBounds, q_sites: ..., q_yards: ...) -> bool` | Query x2 依存。バリデーション本体は `hw_world` helper を呼ぶ形に切り出し可 |
| `pick_yard_for_position` / `pick_stockpile_owner_yard` | Query 依存 |
| `apply_zone_placement` / `apply_yard_expansion` | Commands + WorldMapWrite 依存 |
| `zone_placement_system` 本体 | Bevy system, UI/input 依存 |

### 3-4. `area_selection/manual_haul.rs` の分析

現在の `pick_manual_haul_stockpile_anchor` は `DesignationTargetQuery`（15要素 tuple）を直接イテレートしている。

問題: `DesignationTargetQuery` は root 定義の型で、`hw_logistics` が直接参照できない。

解決策: root adapter がクエリ結果を以下の view model に変換してから `hw_logistics` の純関数に渡す。

```rust
// crates/hw_logistics/src/manual_haul_selector.rs に定義する型
pub struct StockpileCandidateView {
    pub entity: Entity,
    pub pos: Vec2,
    pub owner: Option<Entity>,
    pub resource_type: Option<ResourceType>,
    pub capacity: usize,
    pub current_stored: usize,
    pub is_bucket_storage: bool,
}

pub struct ExistingHaulRequestView {
    pub entity: Entity,
    pub fixed_source: Entity,
}

pub fn select_stockpile_anchor(
    source_pos: Vec2,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    candidates: impl Iterator<Item = StockpileCandidateView>,
) -> Option<Entity>

pub fn find_existing_request(
    source_entity: Entity,
    requests: impl Iterator<Item = ExistingHaulRequestView>,
) -> Option<Entity>
```

root adapter (`area_selection/manual_haul.rs`) は以下のように変わる:

```rust
// root 側 adapter（変更後）
fn pick_manual_haul_stockpile_anchor(
    source_pos: Vec2,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    q_targets: &DesignationTargetQuery,
) -> Option<Entity> {
    let candidates = q_targets.iter().filter_map(|row| {
        let stockpile = row.11?;
        Some(StockpileCandidateView {
            entity: row.0,
            pos: row.1.translation.truncate(),
            owner: row.8.map(|b| b.0),
            resource_type: stockpile.resource_type,
            capacity: stockpile.capacity,
            current_stored: row.12.map(|s| s.len()).unwrap_or(0),
            is_bucket_storage: row.13.is_some(),
        })
    });
    hw_logistics::manual_haul_selector::select_stockpile_anchor(
        source_pos, resource_type, item_owner, candidates,
    )
}
```

## 4. 実装方針

- 新 crate は作らず、責務に応じて `hw_core` / `hw_world` / `hw_logistics` へ寄せる。
- 先に pure helper と read-only 判定ロジックを移し（M1→M2）、その後 query-heavy な選定ロジックを adapter 化（M3）。
- root 側 system は当面維持し、呼び出す helper の所有 crate だけを段階的に置き換える。
- `Commands`, `Query`, `Res`, `NextState`, `PrimaryWindow`, camera 依存の system は root shell に残す。
- 各マイルストーンを **1 コミット単位** に閉じ、単独ロールバック可能にする。

## 5. マイルストーン

---

### M1: `geometry.rs` の pure helper を `hw_core::area` に移設する

**変更ファイル一覧**

| ファイル | 操作 |
| --- | --- |
| `crates/hw_core/src/area.rs` | 5関数を追記 |
| `src/systems/command/area_selection/geometry.rs` | 5関数を削除し `hw_core::area::*` の use に置換 |
| `src/systems/command/mod.rs` | 公開 re-export を `hw_core::area` から行うよう修正 |

**移設する関数とシグネチャ（そのままコピー可能）**

```rust
// crates/hw_core/src/area.rs に追記

use crate::game_state::TaskMode;
use crate::constants::TILE_SIZE;

pub fn get_drag_start(mode: TaskMode) -> Option<Vec2> { ... }

pub fn wall_line_area(start_pos: Vec2, end_pos: Vec2) -> TaskArea { ... }

pub fn area_from_center_and_size(center: Vec2, size: Vec2) -> TaskArea { ... }

pub fn count_positions_in_area(
    area: &TaskArea,
    positions: impl Iterator<Item = Vec2>,
) -> usize { ... }

pub fn overlap_summary_from_areas(
    selected_entity: Entity,
    selected_area: &TaskArea,
    areas: impl Iterator<Item = (Entity, TaskArea)>,
) -> Option<(usize, f32)> { ... }
```

**geometry.rs 変更後の use 宣言（追加分）**

```rust
use hw_core::area::{
    area_from_center_and_size, count_positions_in_area, get_drag_start,
    overlap_summary_from_areas, wall_line_area,
};
```

**mod.rs の re-export 変更**

現在 `src/systems/command/mod.rs` は:
```rust
pub use area_selection::{count_positions_in_area, overlap_summary_from_areas, ...};
```
これを:
```rust
pub use hw_core::area::{count_positions_in_area, get_drag_start, overlap_summary_from_areas, wall_line_area};
```
に変更する（`area_from_center_and_size` は `pub(super)` だったので公開不要）。

**完了条件**
- [ ] 5関数が `hw_core::area.rs` に存在する
- [ ] `geometry.rs` から当該関数の本体が消え、`hw_core::area` からの use になっている
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功

---

### M2: zone placement / removal の read-only helper を `hw_world` に移設する

**M2-a: `identify_removal_targets` → `hw_world::zone_ops`（新規ファイル）**

新規ファイル: `crates/hw_world/src/zone_ops.rs`

```rust
// crates/hw_world/src/zone_ops.rs
use std::collections::{HashSet, VecDeque};
use crate::map::WorldMap;
pub use hw_core::area::AreaBounds;

pub fn identify_removal_targets(
    world_map: &WorldMap,
    area: &AreaBounds,
) -> (Vec<(i32, i32)>, Vec<(i32, i32)>) { ... }
```

`crates/hw_world/src/lib.rs` に追加:
```rust
pub mod zone_ops;
pub use zone_ops::identify_removal_targets;
```

root 側 (`src/systems/command/zone_placement/connectivity.rs`) を削除し、
`zone_placement/removal_preview.rs` の呼び出しを `hw_world::identify_removal_targets` に置換する。

**M2-b: geometry helper → `hw_world::zone_ops`**

追記する関数 4つ（シグネチャ）:

```rust
// crates/hw_world/src/zone_ops.rs に追記

use crate::coords::world_to_grid;
use crate::zones::{Site, Yard};

pub fn area_tile_size(area: &AreaBounds) -> (usize, usize) { ... }

pub fn rectangles_overlap_site(area: &AreaBounds, site: &Site) -> bool { ... }

pub fn rectangles_overlap(area: &AreaBounds, yard: &Yard) -> bool { ... }

pub fn expand_yard_area(yard: &Yard, drag_area: &AreaBounds) -> AreaBounds { ... }
```

root 側 `placement.rs` で:
- `area_tile_size` / `rectangles_overlap_site` / `rectangles_overlap` / `expand_yard_area` の本体を削除
- `use hw_world::{area_tile_size, rectangles_overlap_site, rectangles_overlap, expand_yard_area};` に置換

**変更ファイル一覧**

| ファイル | 操作 |
| --- | --- |
| `crates/hw_world/src/zone_ops.rs` | 新規作成（5関数） |
| `crates/hw_world/src/lib.rs` | `pub mod zone_ops;` および re-export 追加 |
| `src/systems/command/zone_placement/connectivity.rs` | 削除 |
| `src/systems/command/zone_placement/mod.rs` | `mod connectivity` 削除 |
| `src/systems/command/zone_placement/removal_preview.rs` | 呼び出しを `hw_world::identify_removal_targets` に変更 |
| `src/systems/command/zone_placement/placement.rs` | 4関数の本体削除 + use 追加 |

**完了条件**
- [ ] `connectivity.rs` が削除されている（ファイル自体が存在しない）
- [ ] `zone_ops.rs` に 5関数が存在し、単体で compile できる
- [ ] root 側の `is_stockpile_area_within_yards` / `is_yard_expansion_area_valid` は `hw_world::zone_ops` の helper を呼ぶ形に refactor されている（Query iteration は root 残留）
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功
- [ ] Stockpile 配置 / 削除プレビューを手動確認

---

### M3: manual haul 選定ロジックを `hw_logistics` へ移設する

**新規ファイル: `crates/hw_logistics/src/manual_haul_selector.rs`**

```rust
use bevy::prelude::Entity;
use bevy::math::Vec2;
use crate::types::ResourceType;

pub struct StockpileCandidateView {
    pub entity: Entity,
    pub pos: Vec2,
    pub owner: Option<Entity>,
    pub resource_type: Option<ResourceType>,
    pub capacity: usize,
    pub current_stored: usize,
    pub is_bucket_storage: bool,
}

pub struct ExistingHaulRequestView {
    pub entity: Entity,
    pub fixed_source: Entity,
}

pub fn select_stockpile_anchor(
    source_pos: Vec2,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    candidates: impl Iterator<Item = StockpileCandidateView>,
) -> Option<Entity> {
    // 現在の pick_manual_haul_stockpile_anchor のアルゴリズムをそのまま移植
    // ただし QueryIterator の代わりに StockpileCandidateView を操作する
    ...
}

pub fn find_existing_request(
    source_entity: Entity,
    requests: impl Iterator<Item = ExistingHaulRequestView>,
) -> Option<Entity> {
    // 現在の find_manual_request_for_source のアルゴリズムを移植
    ...
}
```

`crates/hw_logistics/src/lib.rs` に追加:
```rust
pub mod manual_haul_selector;
```

**root adapter の変更 (`src/systems/command/area_selection/manual_haul.rs`)**

`DesignationTargetQuery` から view model への変換は root に残す:

```rust
use hw_logistics::manual_haul_selector::{
    ExistingHaulRequestView, StockpileCandidateView, find_existing_request,
    select_stockpile_anchor,
};

pub(super) fn pick_manual_haul_stockpile_anchor(
    source_pos: Vec2,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    q_targets: &DesignationTargetQuery,
) -> Option<Entity> {
    let candidates = q_targets.iter().filter_map(|row| {
        let stockpile = row.11?;
        Some(StockpileCandidateView {
            entity:           row.0,
            pos:              row.1.translation.truncate(),
            owner:            row.8.map(|b| b.0),
            resource_type:    stockpile.resource_type,
            capacity:         stockpile.capacity,
            current_stored:   row.12.map(|s| s.len()).unwrap_or(0),
            is_bucket_storage: row.13.is_some(),
        })
    });
    select_stockpile_anchor(source_pos, resource_type, item_owner, candidates)
}

pub(super) fn find_manual_request_for_source(
    source_entity: Entity,
    q_targets: &DesignationTargetQuery,
) -> Option<Entity> {
    let requests = q_targets.iter().filter_map(|row| {
        (row.9.is_some() && row.14.is_some()).then(|| ExistingHaulRequestView {
            entity:       row.0,
            fixed_source: row.10?.0,
        })
    });
    find_existing_request(source_entity, requests)
}
```

**変更ファイル一覧**

| ファイル | 操作 |
| --- | --- |
| `crates/hw_logistics/src/manual_haul_selector.rs` | 新規作成（2型 + 2関数） |
| `crates/hw_logistics/src/lib.rs` | `pub mod manual_haul_selector;` 追加 |
| `src/systems/command/area_selection/manual_haul.rs` | 本体削除 + view model 変換 adapter に置換 |

**完了条件**
- [ ] `hw_logistics::manual_haul_selector` に 2関数が存在し、`DesignationTargetQuery` を import していない
- [ ] root 側 `manual_haul.rs` は view model 変換のみを行う thin adapter になっている
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功
- [ ] 手動 haul 指定で `TransportRequest` が正しく upsert されることを手動確認

---

### M4: root `command` を shell/visual 中心に整理し docs を更新する

**変更内容**

- `src/systems/command/mod.rs` の公開 API コメントを「shell + visual + ECS apply」と整理する。
- `src/systems/command/README.md` にロール境界（crate 所有 helper / root shell）を追記する。
- `docs/cargo_workspace.md` に `hw_world::zone_ops` / `hw_logistics::manual_haul_selector` の追加を反映する。
- `docs/architecture.md` の command セクションを現状に合わせて更新する。

**root に残る責務（確定）**

| ファイル | 残留理由 |
| --- | --- |
| `input.rs` | Bevy キー/マウス入力、UI input gating |
| `assign_task.rs` | `Commands` + `TaskContext` 依存 |
| `area_selection/input.rs`, `input/release.rs` | Bevy input 依存 |
| `area_selection/indicator.rs` | `GameAssets` + mesh/material spawn |
| `area_selection/state.rs` | `Resource` 型（`AreaEditSession` 等）、`Entity` 保持 |
| `area_selection/apply.rs` | `Commands` + ECS 更新 |
| `area_selection/cursor.rs` | Window/cursor 依存 |
| `zone_placement/placement.rs` (system 本体) | `Commands` + `WorldMapWrite` |
| `zone_placement/removal.rs` | 同上 |
| `zone_placement/removal_preview.rs` | Resource 更新 |
| `indicators.rs` | visual spawn |
| `visualization.rs` | visual spawn |

**変更ファイル一覧**

| ファイル | 操作 |
| --- | --- |
| `src/systems/command/mod.rs` | コメント整理、re-export 最終確認 |
| `src/systems/command/README.md` | ロール境界の説明追記 |
| `docs/cargo_workspace.md` | 新規 module 追記 |
| `docs/architecture.md` | command セクション更新 |

**完了条件**
- [ ] README.md に「何を crate に移したか / 何を root に残したか」が記載されている
- [ ] `docs/cargo_workspace.md` が最新状態
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `get_drag_start` や `wall_line_area` を hw_core に移したとき `TaskMode` の variant 追加と衝突する | 中 | hw_core 側の `TaskMode` の定義を先に確認してから移植。variant が増えたときは hw_core 側の関数も同時修正する |
| `area_tile_size` が `WorldMap::world_to_grid` のロジックと乖離する | 低 | `hw_world::coords::world_to_grid` を直接呼ぶことで一元化。`WorldMap::world_to_grid` は内部で同じ関数を呼んでいるため問題なし |
| `find_manual_request_for_source` の view model 変換で `row.10` が `None` の行を誤ってフィルタアウトする | 高 | 移植前に元実装のフィルタ条件（`manual_opt.is_some() && transport_request_opt.is_some() && fixed_source_opt == Some(source_entity)`）をテストケース相当のコメントで残し、adapter で同等の条件を再現する |
| `hw_logistics` へ query tuple を引きずり込んで crate 境界が悪化する | 高 | `manual_haul_selector.rs` は `DesignationTargetQuery` を一切 import しない（CI でチェック可能） |
| zone validation を移した結果、preview と apply の判定がずれる | 高 | preview (`removal_preview.rs`) と apply (`removal.rs`) の双方が `hw_world::identify_removal_targets` を呼ぶよう統一する |
| `AreaEditSession` など Resource 型を早く移しすぎて UI 依存が混ざる | 中 | 本計画では Resource 型を移さない。`AreaEditSession` / `AreaEditHistory` / `AreaEditClipboard` / `AreaEditPresets` は root 残留 |

## 7. 検証計画

**必須（各マイルストーン後に実行）**

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
```

**手動確認シナリオ**

| シナリオ | 対象 M |
| --- | --- |
| Familiar 選択時の task area ドラッグ・Undo/Redo・preset 操作 | M1 |
| Stockpile 配置プレビュー（有効/無効の色変化） | M2 |
| Yard 拡張の有効/無効判定 | M2 |
| Stockpile 削除プレビュー（孤立フラグメントの検出） | M2 |
| Manual Haul 指定で `TransportRequest` が生成・再利用されること | M3 |

**パフォーマンス確認（必要時）**

```bash
cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario --perf-log-fps
```

## 8. ロールバック方針

- M1, M2, M3 を独立コミット単位で戻せる構成にする（M4 は docs のみなので随時マージ可）。
- crate 側 helper 導入と root 呼び出し差し替えを同一コミットに閉じる。
- 回帰時は当該マイルストーンのコミットだけを `git revert` し、docs の境界説明も同時に戻す。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: **M1, M2, M3, M4**
- 未着手/進行中: なし

### 次のAIが最初にやること

1. `crates/hw_world/src/zone_ops.rs` を新規作成し M2 に進む。
   - `identify_removal_targets` を `src/systems/command/zone_placement/connectivity.rs` からコピー
   - `area_tile_size`, `rectangles_overlap_site`, `rectangles_overlap`, `expand_yard_area` を `placement.rs` からコピー
2. `crates/hw_world/src/lib.rs` に `pub mod zone_ops;` と re-export を追加。
3. `connectivity.rs` を削除し、`mod.rs` から `mod connectivity` を削除。
4. `removal_preview.rs` の呼び出しを `hw_world::identify_removal_targets` に変更。
5. `placement.rs` の4関数本体を削除し `use hw_world::...` に置換。
6. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` で M2 を確認。

### ブロッカー/注意点

- `AreaEditHandleKind` は **root (`src/systems/command/mod.rs`) に留める**。`AreaEditHandleVisual` Component とセットで定義されており、今回のスコープ外。
- `task_area_selection_system` は `TaskContext`, `NextState<PlayMode>`, camera/window, `Commands` に依存するため、いきなり crate へ移さない。
- `area_selection/indicator.rs` は `GameAssets` と mesh/material spawn を使うため root 残留前提。
- `ZoneRemovalPreviewState` 自体は root resource のままでよい。移すべきなのは preview state ではなく `identify_removal_targets` アルゴリズム。
- `find_manual_request_for_source` の adapter 実装では `row.10` が `None` の場合は skip（filter_map で落とす）してよい。元実装の条件と等価であることをコメントで明示すること。
- `hw_world::zone_ops` を `lib.rs` で pub re-export するときは既存の `pub use zones::{...}` と名前が衝突しないよう注意（`area_tile_size` 等は新規なので衝突なし）。

### 参照必須ファイル

- `crates/hw_core/src/area.rs`（移設先、`TaskArea` / `AreaBounds` 定義済み）
- `crates/hw_core/src/game_state.rs`（`TaskMode` の全 variant 確認）
- `crates/hw_world/src/lib.rs`（re-export 追加先）
- `crates/hw_world/src/coords.rs`（`world_to_grid` の実装確認）
- `crates/hw_world/src/zones.rs`（`Site` / `Yard` の定義確認）
- `crates/hw_logistics/src/lib.rs`（`pub mod manual_haul_selector` 追加先）
- `crates/hw_logistics/src/types.rs`（`ResourceType` の定義確認）
- `src/systems/command/area_selection/geometry.rs`（移設元）
- `src/systems/command/area_selection/manual_haul.rs`（adapter 化元）
- `src/systems/command/area_selection/queries.rs`（`DesignationTargetQuery` の tuple 構造確認）
- `src/systems/command/zone_placement/connectivity.rs`（削除元）
- `src/systems/command/zone_placement/placement.rs`（部分削除元）

### 最終確認ログ

- 最終 `cargo check`: `未実行`
- 未解決エラー: なし（計画書更新のみ）

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み（README.md / cargo_workspace.md / architecture.md）
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `AI (Codex)` | 初版作成 |
| `2026-03-12` | `AI (Copilot)` | 実コード精査に基づき全セクションをブラッシュアップ。関数シグネチャ・移設先ファイル・view model 定義・adapter コード例を追加 |
