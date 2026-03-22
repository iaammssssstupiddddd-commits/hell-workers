# 構造整理リファクタリング: visual_sync 分割 / source_selector 計測分離 / haul 分割

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `visual-sync-source-selector-haul-refactor-2026-03-22` |
| ステータス | `Completed` |
| 作成日 | `2026-03-22` |
| 最終更新日 | `2026-03-22` |
| 作成者 | `Copilot` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

> **ブラッシュアップ記録 (2026-03-22)**: 実コードとの照合により以下を修正・追記した。
> - M1: `building_type_to_visual` の配置を `sync.rs` → `mod.rs` に変更（両サブモジュールが参照するため）
> - M1: 各新規ファイルの具体的な `use` ブロックを追加
> - M2: `mark_*` 関数の可視性を `fn` → `pub(super)` に修正、呼び出し経路の記述を修正
> - M3: `issue_return_wheelbarrow` を `haul.rs` → `wheelbarrow_haul.rs` に移動（`HaulWithWheelbarrow` タスクを生成するため）
> - M3: 各新規ファイルの具体的な `use` ブロックを追加、`issue_haul_to_mixer` のインライン `use` をファイル先頭へ移動する旨を追記

## 1. 目的

- **解決したい課題**: Phase 1-4 リファクタリング完了後も残存する中規模の責務混在。
  具体的には (1) Observer と System の同一ファイル混在、(2) 計測コードとコアロジックの混在、
  (3) 異なる運搬メカニズムの builder 混在の 3 点。
- **到達したい状態**: 各ファイルが単一の責務を持ち、Observer/System/計測の各レイヤーが
  ファイル単位で分離されている。
- **成功指標**:
  - `cargo check` がクリーン
  - public API パス（クレート外からの `use` パス）が一切変わらない
  - 各ファイルが明確な単一責務を持つ

## 2. スコープ

### 対象（In Scope）

| # | ファイル | 現行行数 | フェーズ |
|---|---------|---------|---------|
| 1 | `crates/hw_jobs/src/visual_sync.rs` | 332 行 | M1 |
| 2 | `crates/hw_familiar_ai/.../policy/haul/source_selector.rs` | 391 行 | M2 |
| 3 | `crates/hw_familiar_ai/.../builders/haul.rs` | 355 行 | M3 |

### 非対象（Out of Scope）

- `building_move/mod.rs`（406行）— WorldMap / CompanionPlacement 依存が深く先送り
- `idle_behavior/mod.rs`（387行）— すでにサブモジュール分割済み
- `recruitment.rs`（355行）— 実質 2 関数のみで分割効果が低い
- bevy_app/systems ファサード整理 — 呼び出し元が 50+ ファイルで対費用効果を再評価
- 挙動変更・AI ロジックの再設計・パフォーマンス改善

## 3. 現状とギャップ

- **現状**: 対象 3 ファイルの合計 1,078 行にわたり、責務の異なるコードが混在している。
- **問題**:
  - `visual_sync.rs`: ECS ライフサイクル Observer (`on_*`) と定期同期 System (`*_system`) が
    同一ファイルに 13 関数存在。アーキテクチャ規約（Observer/System の責務分離）に反する。
  - `source_selector.rs`: `static AtomicU32` カウンターと `take_source_selector_scan_snapshot` が
    セレクタロジックに埋め込まれており、計測ロジックの独立テスト・置換が困難。
  - `haul.rs`: 手運搬系 4 関数と一輪車系 5 関数が混在。一輪車タスクは予約操作が複雑で
    独立管理が望ましい。
- **本計画で埋めるギャップ**: ファイル分割のみ（シグネチャ・挙動変更なし）で上記 3 点を解消。

## 4. 実装方針（高レベル）

- **方針**: ファイル分割のみ。型・関数のシグネチャ・ロジックは変更しない。
- **設計上の前提**:
  - 既存の `pub use` 経由パスを維持するため、分割後の `mod.rs` に `pub use` を置く。
  - Rust の `impl` 分割ではなく関数レベルの移動のみ。
- **Bevy 0.18 API での注意点**: システム登録・Observer 登録の呼び出し元（`plugins/logic.rs` 等）は
  変更しない。パス維持で透過的。

## 5. マイルストーン

---

## M1: `hw_jobs/visual_sync.rs` — Observer と System の分離

### 変更内容

`visual_sync.rs` をディレクトリ化し、以下の 3 ファイルに分割する。

```
hw_jobs/src/
├── visual_sync/
│   ├── mod.rs        ← building_type_to_visual（共有 private helper）+ pub re-export
│   ├── observers.rs  ← on_* 関数群（ECS ライフサイクル Observer）
│   └── sync.rs       ← *_system 関数群（定期同期 System）
```

> **⚠️ 注意: `building_type_to_visual` の配置**
> この関数は `on_building_added_sync_visual`（observers.rs 行き）と
> `sync_building_visual_system`（sync.rs 行き）の両方から呼ばれる。
> 両サブモジュールから `super::building_type_to_visual` として参照できるよう、
> `mod.rs` に private fn として置く（`sync.rs` には置かない）。

**observers.rs へ移動する関数（5 個）**

| 関数名 | 現行行 | 説明 |
|--------|--------|------|
| `on_designation_added` | 14 | Designation 追加時に GatherHighlightMarker を付与 |
| `on_designation_removed` | 24 | Designation 削除時に GatherHighlightMarker を除去 |
| `on_rest_area_added` | 28 | RestArea 追加時に RestAreaVisual を付与 |
| `on_building_added_sync_visual` | 280 | 建物追加時に BuildingVisualState を付与 |
| `on_mud_mixer_storage_added` | 304 | MudMixerStorage 追加時に MudMixerVisualState を付与 |

**sync.rs へ移動する関数（8 個）**

| 関数名 | 現行行 | 説明 |
|--------|--------|------|
| `sync_soul_task_visual_system` | 36 | Soul タスク状態の visual mirror 同期 |
| `sync_blueprint_visual_system` | 150 | Blueprint visual 同期 |
| `sync_floor_tile_visual_system` | 182 | 床タイル visual 同期 |
| `sync_wall_tile_visual_system` | 205 | 壁タイル visual 同期 |
| `sync_floor_site_visual_system` | 225 | 床サイト visual 同期 |
| `sync_wall_site_visual_system` | 242 | 壁サイト visual 同期 |
| `sync_building_visual_system` | 294 | 建物 visual 同期 |
| `sync_mud_mixer_active_system` | 310 | MudMixer アクティブ状態 visual 同期 |

### 各ファイルの `use` テンプレート

**`visual_sync/mod.rs`**（re-export + 共有 helper）
```rust
mod observers;
mod sync;

pub use observers::*;
pub use sync::*;

use hw_core::visual_mirror::building::BuildingTypeVisual;
use crate::model::BuildingType;

fn building_type_to_visual(kind: BuildingType) -> BuildingTypeVisual {
    match kind {
        BuildingType::Wall               => BuildingTypeVisual::Wall,
        BuildingType::Door               => BuildingTypeVisual::Door,
        BuildingType::Floor              => BuildingTypeVisual::Floor,
        BuildingType::Tank               => BuildingTypeVisual::Tank,
        BuildingType::MudMixer           => BuildingTypeVisual::MudMixer,
        BuildingType::RestArea           => BuildingTypeVisual::RestArea,
        BuildingType::Bridge             => BuildingTypeVisual::Bridge,
        BuildingType::SandPile           => BuildingTypeVisual::SandPile,
        BuildingType::BonePile           => BuildingTypeVisual::BonePile,
        BuildingType::WheelbarrowParking => BuildingTypeVisual::WheelbarrowParking,
    }
}
```

**`visual_sync/observers.rs`**
```rust
use bevy::ecs::lifecycle::{Add, Remove};
use bevy::prelude::*;

use hw_core::visual_mirror::building::{BuildingVisualState, MudMixerVisualState};
use hw_core::visual_mirror::gather::{GatherHighlightMarker, RestAreaVisual};

use crate::model::{Building, Designation, RestArea, Rock, Tree};
use crate::mud_mixer::MudMixerStorage;

use super::building_type_to_visual;

// on_designation_added, on_designation_removed, on_rest_area_added,
// on_building_added_sync_visual, on_mud_mixer_storage_added
```

**`visual_sync/sync.rs`**
```rust
use bevy::prelude::*;

use hw_core::visual_mirror::building::{BuildingVisualState, BuildingTypeVisual};
use hw_core::visual_mirror::construction::{
    BlueprintVisualState, FloorConstructionPhaseMirror, FloorSiteVisualState, FloorTileStateMirror,
    FloorTileVisualMirror, WallSiteVisualState, WallTileStateMirror, WallTileVisualMirror,
};
use hw_core::visual_mirror::task::{SoulTaskPhaseVisual, SoulTaskVisualState};

use crate::construction::{
    FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
    WallConstructionPhase, WallConstructionSite, WallTileBlueprint, WallTileState,
};
use crate::model::{Blueprint, Building, BuildingType};
use crate::tasks::{
    AssignedTask, CoatWallPhase, FrameWallPhase, GatherPhase, HaulPhase, PourFloorPhase,
    RefinePhase, ReinforceFloorPhase,
};

use super::building_type_to_visual;

// sync_soul_task_visual_system, sync_blueprint_visual_system, sync_floor_tile_visual_system,
// sync_wall_tile_visual_system, sync_floor_site_visual_system, sync_wall_site_visual_system,
// sync_building_visual_system, sync_mud_mixer_active_system
```

> **⚠️ 注意: 現行ファイルの `use` 文が散在**
> `visual_sync.rs` は `use` 宣言が行 1-13、139-148、258-260、277 に散らばっている。
> 分割時は各新規ファイルの先頭にまとめること。

### `logic.rs` の明示 import 確認

`logic.rs` は `pub use *` ではなく個別列挙で 13 シンボルを import している：
```rust
use hw_jobs::visual_sync::{
    on_building_added_sync_visual, on_designation_added, on_designation_removed,
    on_mud_mixer_storage_added, on_rest_area_added, sync_blueprint_visual_system,
    sync_building_visual_system, sync_floor_site_visual_system, sync_floor_tile_visual_system,
    sync_mud_mixer_active_system, sync_soul_task_visual_system, sync_wall_site_visual_system,
    sync_wall_tile_visual_system,
};
```
`mod.rs` の `pub use observers::*; pub use sync::*;` で全 13 シンボルが再エクスポートされれば変更不要。

### 変更ファイル

- `crates/hw_jobs/src/visual_sync.rs` → `crates/hw_jobs/src/visual_sync/mod.rs` に変換
- `crates/hw_jobs/src/visual_sync/observers.rs` （新規作成）
- `crates/hw_jobs/src/visual_sync/sync.rs` （新規作成）
- `crates/hw_jobs/src/lib.rs` — 変更不要（`pub mod visual_sync;` のまま）
- `crates/bevy_app/src/plugins/logic.rs` — 変更不要（パス維持）

### 完了条件

- [ ] `visual_sync.rs` → `visual_sync/mod.rs` + `observers.rs` + `sync.rs` に分割完了
- [ ] `mod.rs` が全 13 関数を `pub use` で再エクスポートしている
- [ ] `building_type_to_visual` が `mod.rs` に置かれ、両サブモジュールから `super::building_type_to_visual` で参照されている
- [ ] `cargo check` がクリーン

### 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

---

## M2: `source_selector.rs` — 計測コードの分離

### 変更内容

`source_selector.rs` から計測用 static カウンターと関連関数を `selector_metrics.rs` に抽出する。

```
hw_familiar_ai/.../policy/haul/
├── source_selector.rs    ← セレクタ本体（変更後: ~350 行程度）
└── selector_metrics.rs   ← 計測コード（新規: ~35 行程度）
```

**selector_metrics.rs へ移動するもの（現行 source_selector.rs 行 4-35）**

```rust
use std::sync::atomic::{AtomicU32, Ordering};

// static カウンター（private）
static SOURCE_SELECTOR_CALLS: AtomicU32 = AtomicU32::new(0);
static SOURCE_SELECTOR_CACHE_BUILD_SCANNED_ITEMS: AtomicU32 = AtomicU32::new(0);
static SOURCE_SELECTOR_CANDIDATE_SCANNED_ITEMS: AtomicU32 = AtomicU32::new(0);

// helpers: pub(super) にして source_selector.rs から呼べるようにする
pub(super) fn mark_source_selector_call() { ... }
pub(super) fn mark_cache_build_scanned_item() { ... }
pub(super) fn mark_candidate_scanned_item() { ... }

// public API（外部公開は policy/haul/mod.rs -> policy/mod.rs ->
// task_management/mod.rs の pub use 経由）
pub fn take_source_selector_scan_snapshot() -> (u32, u32, u32) { ... }
```

> **⚠️ 注意: `mark_*` の可視性**
> 元ファイルでは private fn だが、`selector_metrics.rs` から `source_selector.rs`
>（兄弟モジュール）が呼ぶため `pub(super)` にする必要がある。
> `take_source_selector_scan_snapshot` は `pub` のまま（`policy/haul/mod.rs` 経由で公開）。

**source_selector.rs の変更点**

1. `use std::sync::atomic::{AtomicU32, Ordering};` を削除
2. static 定数 3 つを削除
3. `mark_*` 関数 3 つを削除
4. `take_source_selector_scan_snapshot` を削除
5. ファイル先頭に以下を追加：
   ```rust
   use super::selector_metrics::{
       mark_cache_build_scanned_item, mark_candidate_scanned_item, mark_source_selector_call,
   };
   ```
6. `mark_source_selector_call()` の呼び出し箇所（行 171, 183, 247, 257, 341）はそのまま維持（インポートで解決）

**公開パスの維持**

`policy/haul/mod.rs` 行 23 の `pub use source_selector::take_source_selector_scan_snapshot;` は変更不要。
`policy/mod.rs` と `task_management/mod.rs` の再エクスポートもそのまま維持されるため、
外部から使われている `task_management::take_source_selector_scan_snapshot()` パスは変わらない。
`source_selector.rs` には `pub use super::selector_metrics::take_source_selector_scan_snapshot;` を追加する。

### 変更ファイル

- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/haul/source_selector.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/haul/selector_metrics.rs` （新規作成）
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/haul/mod.rs` — `mod selector_metrics;` を追加（`mod source_selector;` の前後どちらでも可）

### 完了条件

- [ ] `selector_metrics.rs` に静的カウンター・`mark_*`（pub(super)）・`take_*`（pub）が移動している
- [ ] `source_selector.rs` から `AtomicU32`・static 変数・`mark_*` が消えている
- [ ] `source_selector.rs` 先頭に `use super::selector_metrics::{...}` が追加されている
- [ ] `source_selector.rs` に `pub use super::selector_metrics::take_source_selector_scan_snapshot;` がある
- [ ] `policy/haul/mod.rs` に `mod selector_metrics;` が追加されている
- [ ] `take_source_selector_scan_snapshot` の呼び出し元パス（`task_management::take_source_selector_scan_snapshot`）が維持されている
- [ ] `cargo check` がクリーン

### 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

---

## M3: `haul.rs` — 手運搬 / 一輪車 builder 分割

**前提: M1・M2 完了後に着手**

### 変更内容

`haul.rs` を 2 ファイルに分割し、`mod.rs` が `pub use` で再エクスポートする。

```
hw_familiar_ai/.../builders/
├── mod.rs               ← 共通ヘルパー（既存）+ mod/pub use 追加
├── haul.rs              ← 手運搬系 builder（変更後: ~110 行程度、3 関数）
└── wheelbarrow_haul.rs  ← 一輪車系 builder（新規: ~260 行程度、6 関数）
```

**haul.rs に残す関数（3 個）**

| 関数名 | 現行行 | 説明 |
|--------|--------|------|
| `issue_haul_to_blueprint_with_source` | 18 | 設計図への手運搬 |
| `issue_haul_to_stockpile_with_source` | 44 | 備蓄場所への手運搬 |
| `issue_haul_to_mixer` | 70 | Mixer への手運搬 |

> **⚠️ `issue_haul_to_mixer` の inline `use` を先頭へ移動**
> 現行の行 81 に `use hw_jobs::{HaulToMixerData, HaulToMixerPhase};` が関数内に埋め込まれている。
> 分割後の `haul.rs` では先頭の `use` ブロックに移動すること。

**wheelbarrow_haul.rs へ移動する関数（6 個）**

| 関数名 | 現行行 | 説明 |
|--------|--------|------|
| `issue_return_wheelbarrow` | 142 | 一輪車を駐車場へ返却 |
| `issue_haul_with_wheelbarrow` | 106 | 一輪車による標準運搬 |
| `issue_collect_sand_with_wheelbarrow_to_blueprint` | 175 | 砂を一輪車で設計図へ収集 |
| `issue_collect_sand_with_wheelbarrow_to_mixer` | 219 | 砂を一輪車で Mixer へ収集 |
| `issue_collect_bone_with_wheelbarrow_to_blueprint` | 269 | 骨を一輪車で設計図へ収集 |
| `issue_collect_bone_with_wheelbarrow_to_floor` | 313 | 骨を一輪車で床へ収集 |

> **⚠️ `issue_return_wheelbarrow` の移動先変更（初版から修正）**
> `issue_return_wheelbarrow` は `AssignedTask::HaulWithWheelbarrow` を生成し、
> `HaulWithWheelbarrowData`・`HaulWithWheelbarrowPhase`・`WheelbarrowDestination` を使う。
> 一輪車専用の型に依存するため、`haul.rs` ではなく `wheelbarrow_haul.rs` に移動する。

### 各ファイルの `use` テンプレート

**新 `haul.rs`**（3 関数、一輪車依存型を完全に除去）
```rust
use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use hw_jobs::WorkType;
use hw_jobs::{
    AssignedTask, HaulData, HaulPhase, HaulToBlueprintData, HaulToBpPhase,
    HaulToMixerData, HaulToMixerPhase,  // ← issue_haul_to_mixer の inline use から移動
};

use super::{
    build_mixer_destination_reservation_ops, build_source_reservation_ops,
    submit_assignment_with_reservation_ops, submit_assignment_with_source_entities,
};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};
```

**新 `wheelbarrow_haul.rs`**（6 関数、一輪車専用）
```rust
use bevy::prelude::*;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};
use hw_jobs::WorkType;
use hw_jobs::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase};

use super::{
    build_mixer_destination_reservation_ops, build_source_reservation_ops,
    build_wheelbarrow_reservation_ops, submit_assignment_with_reservation_ops,
    submit_assignment_with_source_entities,
};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};
```

### `builders/mod.rs` の変更

```diff
  mod basic;
  mod haul;
  mod water;
+ mod wheelbarrow_haul;

  pub use basic::*;
  pub use haul::*;
  pub use water::*;
+ pub use wheelbarrow_haul::*;
```

> **確認事項**: 既存の `pub use haul::*;` を `mod haul;` + `pub use haul::*;` に変える必要はない。
> `mod haul;` はすでに行 2 に存在する。`mod wheelbarrow_haul;` と `pub use wheelbarrow_haul::*;` を追加するだけ。

### 変更ファイル

- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/builders/haul.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/builders/wheelbarrow_haul.rs` （新規作成）
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/builders/mod.rs`
  — `mod wheelbarrow_haul;` + `pub use wheelbarrow_haul::*;` を追加

### 完了条件

- [ ] `wheelbarrow_haul.rs` に一輪車系 builder 6 関数が移動している
  （`issue_return_wheelbarrow` を含む）
- [ ] `haul.rs` が手運搬系 3 関数のみになっている
- [ ] `haul.rs` から `HaulWithWheelbarrowData`・`HaulWithWheelbarrowPhase`・`WheelbarrowDestination` の import が消えている
- [ ] `issue_haul_to_mixer` の inline `use` がファイル先頭に移動している
- [ ] `builders/mod.rs` が `wheelbarrow_haul` を `mod` + `pub use *` で追加している
- [ ] `cargo check` がクリーン

### 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `pub use` 再エクスポートパスの抜け | コンパイルエラー | `cargo check` を各 M 完了後に実行 |
| `use` 宣言の重複（Rustコンパイラ警告） | 警告増加 | 分割後に `use` を各ファイルで最小化 |
| `mod.rs` の `pub use` が public path を変えてしまう | 呼び出し元破損 | 分割前後で同じシンボルが同じパスで参照できることを `grep` で確認 |
| M3 で `builders/mod.rs` の re-export 追加時に名前衝突を見落とす | コンパイルエラー | 既存の `mod haul;` / `pub use haul::*;` は維持し、`wheelbarrow_haul` 追加後に `cargo check` で確認 |

## 7. 検証計画

- **必須**:
  ```bash
  CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
  ```
- **手動確認シナリオ**: なし（挙動変更なしのため）
- **パフォーマンス確認**: なし

## 8. ロールバック方針

- **戻せる単位**: マイルストーン単位（M1/M2/M3 はそれぞれ独立した git commit）
- **戻す手順**: `git revert <commit>` または `git checkout HEAD~1 -- <files>`

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1 → M2 → M3 の順

### 次のAIが最初にやること

1. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` でベースラインを確認
2. M1 から着手: `crates/hw_jobs/src/visual_sync.rs` の内容を読んでディレクトリ化
3. 各 M 完了後に `cargo check` してクリーンを確認してから次の M へ進む

### ブロッカー/注意点

- M3 は M1・M2 完了後に着手（依存なしだが作業連続性のため）
- **M1**: `building_type_to_visual` は `mod.rs` に置く。`observers.rs` と `sync.rs` 両方から `super::building_type_to_visual` で参照。
- **M2**: `mark_*` は `pub(super)` にすること（`fn` のままだと `source_selector.rs` から参照不可）
- **M2**: `source_selector.rs` の `mark_*()` 呼び出し行（171, 183, 247, 257, 341）は変更不要（import 追加で解決）
- **M3**: `issue_return_wheelbarrow` は `wheelbarrow_haul.rs` に移動（手運搬系ではなく一輪車系）
- **M3**: `builders/mod.rs` の `mod haul;` はすでに存在、`mod wheelbarrow_haul;` + `pub use wheelbarrow_haul::*;` を追加するだけ

### 参照必須ファイル

- `docs/cargo_workspace.md` — クレート境界ルール
- `crates/hw_jobs/src/visual_sync.rs` — M1 対象（現行 332 行）
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/haul/source_selector.rs` — M2 対象（現行 391 行）
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/builders/haul.rs` — M3 対象（現行 355 行、9 関数）
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/builders/mod.rs` — M3 変更対象
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/haul/mod.rs` — M2 変更対象
- `crates/bevy_app/src/plugins/logic.rs` — visual_sync 呼び出し元（明示 import 13 シンボル、変更不要を確認）

### 最終確認ログ

- 最終 `cargo check`: `2026-03-22` / `pass`
- 未解決エラー: なし

### Definition of Done

- [ ] M1 完了: `visual_sync/` ディレクトリ化、`observers.rs` + `sync.rs` 分離、`building_type_to_visual` は `mod.rs`
- [ ] M2 完了: `selector_metrics.rs` 分離、`mark_*` は `pub(super)`
- [ ] M3 完了: `wheelbarrow_haul.rs` 分離（6 関数）、`haul.rs` は 3 関数のみ
- [ ] 全 M: `cargo check` がクリーン
- [ ] public API パスに変更なし

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-22` | `Copilot` | 初版作成 |
| `2026-03-22` | `Copilot` | 実コード照合によるブラッシュアップ: `building_type_to_visual` 配置修正 / `mark_*` 可視性修正 / `issue_return_wheelbarrow` 移動先修正 / 各ファイルの `use` ブロック追記 |
