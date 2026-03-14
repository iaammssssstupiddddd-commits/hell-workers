# 型・ドメインモデルのクレート境界リファクタリング計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `types-migration-plan-2026-03-13` |
| ステータス | `Done` |
| 作成日 | `2026-03-13` |
| 最終更新日 | `2026-03-13` |
| 作成者 | `Gemini Agent` |
| ブラッシュアップ | `Copilot` |
| 関連ドキュメント | `docs/crate-boundaries.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- **解決したい課題**: `bevy_app` にドメイン固有の型（`GameTime`, `Room`, `AreaEditSession` 等）が残留し、他の Leaf クレートがこれらを参照できない。`bevy_app` への逆依存はコンパイルエラーになるため、ドメインロジック分離（将来計画）のブロッカーになっている。
- **到達したい状態**: 各型が `docs/crate-boundaries.md §2` のルールに従った適切な `hw_*` クレートに配置され、複数の Leaf クレートから参照可能な状態。
- **成功指標**: `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が警告・エラーなしで通過する。

## 2. スコープ

### 対象（In Scope）— 型のみ移動、システム関数は bevy_app に留める

| 移動対象 | 現在の場所 | 移動先 | 理由 |
| --- | --- | --- | --- |
| `GameTime` | `bevy_app/src/systems/time.rs` | `hw_core/src/time.rs` | 基盤型（パターン A）。`regrowth` 等の将来的な Leaf 化を見据える。 |
| `DreamTreePlantingPlan` | `bevy_app/src/systems/dream_tree_planting.rs` | `hw_world/src/tree_planting.rs` | 世界タイル座標のみを持つ純粋データ構造。`hw_world` のドメインと一致。※hw_jobs は不適（後述） |
| `AreaEditSession` / `AreaEditHistory` / `AreaEditClipboard` / `AreaEditPresets` | `bevy_app/src/systems/command/area_selection/state.rs` | `hw_ui/src/area_edit/state.rs` | UI 操作状態（プレゼンテーション層）。`hw_ui` が所有すべき型。 |
| `Room` | `bevy_app/src/systems/room/components.rs` | `hw_world/src/room_detection.rs` (既存ファイルに追記) | `RoomBounds` は既に `hw_world::room_detection` が所有。同モジュールにまとめる自然な配置。 |
| `RoomTileLookup` / `RoomDetectionState` / `RoomValidationState` | `bevy_app/src/systems/room/resources.rs` | `hw_world/src/room_detection.rs` (既存ファイルに追記) | 同上。Room に関連するリソースをすべて hw_world に集約する。 |

### 非対象（Out of Scope）

- システム関数（`game_time_system`, `detect_rooms_system`, `dream_tree_planting_system` 等）の移動は別計画で実施。型移動のみ行う。
- AI 意思決定ロジックの純粋関数化（別計画で実施）。
- `build_dream_tree_planting_plan()` 関数の移動。`GameAssets` / `DreamPool` 等の bevy_app 固有型に依存するため移動不可（**注意点** 参照）。

## 3. 現状と課題の詳細

### クレート依存グラフ（現状）

```
bevy_app → hw_core, hw_jobs, hw_logistics, hw_ui, hw_world, hw_ai, hw_familiar_ai, hw_soul_ai, hw_spatial, hw_visual
hw_world → hw_core, hw_jobs
hw_ui    → hw_core, hw_jobs, hw_logistics
hw_jobs  → hw_core
hw_core  → bevy (のみ)
```

循環依存を生まない制約:
- `hw_core` は他の `hw_*` に依存してはいけない
- `hw_world` は `bevy_app` に依存してはいけない
- `hw_ui` は `bevy_app` に依存してはいけない

### なぜ DreamTreePlantingPlan を hw_jobs ではなく hw_world に入れるか

`build_dream_tree_planting_plan()` は `WorldMap`（`hw_world`）を引数に取る。将来この関数を Leaf に移したい場合、`hw_jobs` に置くと `hw_jobs → hw_world` の依存が必要になり、**現在の `hw_world → hw_jobs` と循環**してしまう。`hw_world` に置けば自クレート内で完結する。

## 4. マイルストーン

---

### M1: GameTime → hw_core

**作業規模**: 小（新ファイル 1 つ + 既存 use パス 3 箇所）

#### 手順

**Step 1**: `hw_core/src/time.rs` を新規作成

```rust
use bevy::prelude::*;

#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct GameTime {
    pub seconds: f32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}
```

**Step 2**: `hw_core/src/lib.rs` に追記

```rust
pub mod time;
pub use time::GameTime;
```

**Step 3**: `bevy_app/src/systems/time.rs` の `GameTime` 定義を削除し、re-export に差し替える

```rust
// 削除: GameTime の struct 定義ブロック全体
// 追加:
pub use hw_core::GameTime;
```

（`game_time_system` はそのまま残す。`ClockText` を使うため bevy_app 外に出せない）

**Step 4**: `bevy_app/src/world/regrowth.rs` の import を確認・更新

```rust
// 変更前（推測パス）: use crate::systems::time::GameTime;
// 変更後: use crate::systems::time::GameTime; (re-export があるので変更不要の可能性あり)
// → cargo check で確認してから判断
```

**Cargo.toml 変更**: なし（`bevy_app` は既に `hw_core` に依存済み）

**完了条件**: `cargo check --workspace` が通る

---

### M2: DreamTreePlantingPlan → hw_world

**作業規模**: 小（新ファイル 1 つ + 既存 use パス 2 箇所）

#### 手順

**Step 1**: `hw_world/src/tree_planting.rs` を新規作成

```rust
/// Dream植林の計画データ。build_dream_tree_planting_plan() の戻り値として使われる。
/// ビルダー関数本体は bevy_app 固有型 (GameAssets 等) に依存するため bevy_app に残留。
#[derive(Debug, Clone)]
pub struct DreamTreePlantingPlan {
    pub width_tiles: u32,
    pub height_tiles: u32,
    pub min_square_side: u32,
    pub planned_spawn: u32,
    pub cap_remaining: u32,
    pub affordable: u32,
    pub candidate_count: u32,
    pub selected_tiles: Vec<(i32, i32)>,
}

impl DreamTreePlantingPlan {
    pub fn final_spawn(&self) -> u32 {
        self.selected_tiles.len() as u32
    }

    pub fn cost(&self) -> f32 {
        // hw_core::constants の DREAM_TREE_COST_PER_TREE を参照
        use hw_core::constants::DREAM_TREE_COST_PER_TREE;
        self.final_spawn() as f32 * DREAM_TREE_COST_PER_TREE
    }
}
```

> **確認済み**: `DREAM_TREE_COST_PER_TREE` は `hw_core/src/constants/dream.rs` に存在する。`use hw_core::constants::DREAM_TREE_COST_PER_TREE;` をそのまま使用可能。

**Step 2**: `hw_world/src/lib.rs` に追記

```rust
pub mod tree_planting;
pub use tree_planting::DreamTreePlantingPlan;
```

**Step 3**: `bevy_app/src/systems/dream_tree_planting.rs` の定義を削除し、import に差し替える

```rust
// 削除: DreamTreePlantingPlan の struct + impl ブロック
// 追加:
use hw_world::DreamTreePlantingPlan;
```

**Step 4**: `bevy_app/src/systems/command/area_selection/indicator.rs` の import を更新

```rust
// 変更前: use crate::systems::dream_tree_planting::DreamTreePlantingPlan; (または類似)
// 変更後: use hw_world::DreamTreePlantingPlan;
```

**Cargo.toml 変更**: なし（`bevy_app` は既に `hw_world` に依存済み）

**完了条件**: `cargo check --workspace` が通る

---

### M3: AreaEditSession 等の UI 状態 → hw_ui

**作業規模**: 中（新モジュール追加 + `pub(super)` 可視性の調整 + use パス多数）

#### 背景と注意点

`AreaEditSession` の `active_drag` フィールドは `pub(super)` で定義されており、`area_selection/` モジュール内のシステムが直接書き換えている。型を `hw_ui` に移動すると、**bevy_app 側のシステムから `pub(super)` フィールドにアクセスできなくなる**。

対策として、移動時に `pub(super)` を `pub(crate)` （hw_ui クレート内限定公開）に変更し、bevy_app 側には公開フィールド経由でアクセスできる専用の API メソッドを生やすか、フィールド全体を `pub` にする。

この計画では**`pub(super)` フィールドを `pub` に変更する**方針とする（`AreaEditPresets.slots` は既存アクセサメソッドで十分なので変更不要）。

**⚠️ 循環依存の回避（重要）**: `AreaEditOperation::Resize(AreaEditHandleKind)` が参照する `AreaEditHandleKind` は `bevy_app/src/systems/command/mod.rs` で定義されている。この型を `hw_ui` に移動せずに `AreaEditOperation` だけ移動すると `hw_ui → bevy_app` の依存が生じ、既存の `bevy_app → hw_ui` と**循環依存**になる。**`AreaEditHandleKind` も `hw_ui` に移動する必要がある**。

#### 移動対象の型

| 型 | 種別 | 内容 |
| --- | --- | --- |
| `AreaEditHandleKind` | enum | リサイズハンドルの種類（**循環依存回避のため hw_ui へ移動必須**）|
| `AreaEditSession` | Resource | ドラッグ状態 + dream planting キュー |
| `AreaEditDrag` | 内部型 | ドラッグ操作の詳細（familiar_entity, operation, etc.）|
| `AreaEditOperation` | 内部 enum | Move / Resize |
| `AreaEditHistory` | Resource | undo/redo スタック |
| `AreaEditHistoryEntry` | 内部型 | undo/redo 1エントリ |
| `AreaEditClipboard` | Resource | コピーした TaskArea |
| `AreaEditPresets` | Resource | サイズプリセット 3スロット |

**`TaskArea` は `hw_core::area` 由来。`hw_ui` は既に `hw_jobs` に依存しており `hw_core` にも推移依存しているので追加の Cargo.toml 変更は不要。**

#### 手順

**Step 1**: `hw_ui/src/area_edit/` ディレクトリを作成し、`state.rs` を置く

ファイル: `hw_ui/src/area_edit/state.rs`

```rust
use bevy::prelude::*;
use hw_core::area::TaskArea;

// --- AreaEditHandleKind（bevy_app から移動）---
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaEditHandleKind {
    TopLeft, Top, TopRight, Right, BottomRight, Bottom, BottomLeft, Left, Center,
}

// --- AreaEditOperation ---
#[derive(Clone, Copy, Debug)]
pub enum AreaEditOperation {
    Move,
    Resize(AreaEditHandleKind),
}

// --- AreaEditDrag ---
#[derive(Clone)]
pub struct AreaEditDrag {
    pub familiar_entity: Entity,
    pub operation: AreaEditOperation,
    pub original_area: TaskArea,
    pub drag_start: Vec2,
}

// --- AreaEditSession ---
#[derive(Resource, Default)]
pub struct AreaEditSession {
    pub active_drag: Option<AreaEditDrag>,
    pub pending_dream_planting: Option<(Vec2, Vec2, u64)>,
    pub dream_planting_preview_seed: Option<u64>,
}

impl AreaEditSession {
    pub fn is_dragging(&self) -> bool { self.active_drag.is_some() }
    pub fn operation_label(&self) -> Option<&'static str> { /* 既存実装をそのまま移植 */ }
}

// --- AreaEditHistoryEntry ---
#[derive(Clone)]
pub struct AreaEditHistoryEntry {
    pub familiar_entity: Entity,
    pub before: Option<TaskArea>,
    pub after: Option<TaskArea>,
}

// --- AreaEditHistory ---
#[derive(Resource, Default)]
pub struct AreaEditHistory {
    pub undo_stack: Vec<AreaEditHistoryEntry>,
    pub redo_stack: Vec<AreaEditHistoryEntry>,
}

impl AreaEditHistory {
    pub fn push(
        &mut self,
        familiar_entity: Entity,
        before: Option<TaskArea>,
        after: Option<TaskArea>,
    ) {
        // 既存実装をそのまま移植（MAX_HISTORY = 64、同一なら早期 return）
    }
}

// --- AreaEditClipboard ---
#[derive(Resource, Default)]
pub struct AreaEditClipboard {
    pub area: Option<TaskArea>,
}

impl AreaEditClipboard {
    pub fn has_area(&self) -> bool { self.area.is_some() }
}

// --- AreaEditPresets ---
#[derive(Resource, Default)]
pub struct AreaEditPresets {
    slots: [Option<Vec2>; 3], // フィールドは非公開のままアクセサで対応
}

impl AreaEditPresets {
    pub fn save_size(&mut self, slot: usize, size: Vec2) { /* 既存実装を移植 */ }
    pub fn get_size(&self, slot: usize) -> Option<Vec2> { /* 既存実装を移植 */ }
}
```

**Step 2**: `hw_ui/src/area_edit/mod.rs` を作成

```rust
mod state;
pub use state::{
    AreaEditClipboard, AreaEditDrag, AreaEditHandleKind, AreaEditHistory,
    AreaEditOperation, AreaEditPresets, AreaEditSession,
};
```

**Step 3**: `hw_ui/src/lib.rs` に追記

```rust
pub mod area_edit;
pub use area_edit::{
    AreaEditClipboard, AreaEditHandleKind, AreaEditHistory, AreaEditPresets, AreaEditSession,
};
```

**Step 4**: `bevy_app/src/systems/command/area_selection/state.rs` の型定義を削除し、re-export に差し替える

```rust
// 削除: 全型定義ブロック（AreaEditDrag/Operation/Session/History/Clipboard/Presets）
// 追加:
pub use hw_ui::area_edit::{
    AreaEditClipboard, AreaEditDrag, AreaEditHistory, AreaEditOperation,
    AreaEditPresets, AreaEditSession,
};
```

（`area_selection/` 内の他ファイルは `use super::state::*` 等で参照しているので re-export があれば変更不要の可能性が高い。`cargo check` で確認）

**Step 5**: `bevy_app/src/systems/command/mod.rs` の `AreaEditHandleKind` 定義を削除し、re-export に差し替える

```rust
// 削除: AreaEditHandleKind の enum 定義ブロック
// 追加:
pub use hw_ui::AreaEditHandleKind;
```

`AreaEditHandleVisual` コンポーネントは `bevy_app` に残したままでよい（`AreaEditHandleKind` を re-export 経由で参照するため `mod.rs` の他のコードに変更は不要）。

**Cargo.toml 変更**: なし（`bevy_app` は既に `hw_ui` に依存済み）

**完了条件**: `cargo check --workspace` が通る

---

### M4: Room / RoomTileLookup 等 → hw_world

**作業規模**: 中（既存ファイルへの追記 + use パス多数）

#### 手順

**Step 1**: `hw_world/src/room_detection.rs`（既存、493行）の末尾に型定義を追記する

```rust
// === ECS Components & Resources (hw_world に所有権を持つ) ===
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use hw_core::constants::{ROOM_DETECTION_COOLDOWN_SECS, ROOM_VALIDATION_INTERVAL_SECS};

#[derive(Component, Debug, Clone)]
pub struct Room {
    pub tiles: Vec<(i32, i32)>,
    pub wall_tiles: Vec<(i32, i32)>,
    pub door_tiles: Vec<(i32, i32)>,
    pub bounds: RoomBounds,   // RoomBounds は既にこのファイルで定義済み
    pub tile_count: usize,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomOverlayTile {
    pub grid_pos: (i32, i32),
}

#[derive(Resource, Default, Debug)]
pub struct RoomTileLookup {
    pub tile_to_room: HashMap<(i32, i32), Entity>,
}

#[derive(Resource)]
pub struct RoomDetectionState {
    pub dirty_tiles: HashSet<(i32, i32)>,
    pub cooldown: Timer,
}

impl Default for RoomDetectionState {
    fn default() -> Self {
        Self {
            dirty_tiles: HashSet::new(),
            cooldown: Timer::from_seconds(ROOM_DETECTION_COOLDOWN_SECS, TimerMode::Repeating),
        }
    }
}

impl RoomDetectionState {
    pub fn mark_dirty(&mut self, tile: (i32, i32)) { /* 既存実装を移植 */ }
    pub fn mark_dirty_many<I: IntoIterator<Item = (i32, i32)>>(&mut self, tiles: I) { /* 同上 */ }
}

#[derive(Resource)]
pub struct RoomValidationState {
    pub timer: Timer,
}

impl Default for RoomValidationState {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(ROOM_VALIDATION_INTERVAL_SECS, TimerMode::Repeating),
        }
    }
}
```

**Step 2**: `hw_world/src/lib.rs` に re-export を追記

```rust
pub use room_detection::{
    // 既存の re-export はそのまま維持
    // 追加:
    Room, RoomDetectionState, RoomOverlayTile, RoomTileLookup, RoomValidationState,
};
```

**Step 3**: `bevy_app/src/systems/room/components.rs` の型定義を削除し、re-export に差し替える

```rust
// 削除: Room, RoomOverlayTile の struct 定義
// 追加:
pub use hw_world::{Room, RoomOverlayTile};
```

**Step 4**: `bevy_app/src/systems/room/resources.rs` の型定義を削除し、re-export に差し替える

```rust
// 削除: RoomTileLookup, RoomDetectionState, RoomValidationState の struct 定義
// 追加:
pub use hw_world::{RoomDetectionState, RoomTileLookup, RoomValidationState};
```

**Step 5**: `bevy_app/src/systems/room/mod.rs` と `bevy_app/src/plugins/logic.rs` の import が壊れていないか `cargo check` で確認し、必要なパスを修正する。

**Cargo.toml 変更**: なし（`bevy_app` は既に `hw_world` に依存済み）

**完了条件**: `cargo check --workspace` が通る

---

## 5. 作業順序の推奨

依存関係が少なくリスクの低いものから着手する:

```
M1 (GameTime) → M4 (Room) → M2 (DreamTreePlantingPlan) → M3 (AreaEditSession)
```

- **M1**: 最小変更。失敗時のロールバックが容易。
- **M4**: 最も「自然な移動」。`RoomBounds` が既に `hw_world` にある。
- **M2**: 定数の所在確認が必要（事前調査してから）。
- **M3**: `AreaEditHandleKind` の確認と可視性変更が必要。最後に実施。

## 6. リスクと対策

| リスク | 影響 | 具体的な対策 |
| --- | --- | --- |
| `pub(super)` → `pub` 変更で意図しないアクセスが発生 | 低 | `AreaEditDrag` 等の内部型は `pub(crate)` に留めて外部クレートへの露出を最小化する。`AreaEditPresets.slots` はアクセサ経由で対応済みのため pub 変更不要 |
| `DREAM_TREE_COST_PER_TREE` 定数の所在 | 解決済み | `hw_core/src/constants/dream.rs` に存在確認済み。`use hw_core::constants::DREAM_TREE_COST_PER_TREE;` で参照可能 |
| `AreaEditHandleKind` の循環依存 | 解決済み | 定義が `bevy_app/src/systems/command/mod.rs` にあることを確認。`hw_ui` へ移動し `bevy_app/command/mod.rs` で re-export することで解決（M3 スコープに含めた）|
| `room_detection.rs` が 493 行に追記で肥大化する | 低 | 追記後に 700 行超になるようであれば `room_detection/` サブモジュール化を検討（本計画のスコープ外） |
| re-export チェーンで `use` パスが増えた際に重複 re-export が出る | 低 | `cargo check --workspace` の `unused_imports` 警告を都度修正する |

## 7. 検証計画

各マイルストーン完了後に必ず実行:

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
```

全マイルストーン完了後の手動確認シナリオ:

| 確認項目 | 操作 |
| --- | --- |
| `GameTime` が正常に進行 | ゲーム起動後、右上のクロック表示が更新される |
| `DreamTreePlantingPlan` プレビューが正常 | Dream モードでドラッグするとプレビューが表示される |
| `AreaEditSession` のドラッグが正常 | タスクエリアのドラッグ・リサイズ・undo が動作する |
| `Room` が正常に検出 | 壁で囲まれたエリアに部屋のオーバーレイが表示される |

## 8. ロールバック方針

- 各マイルストーン完了ごとにコミット（`feat: M1 GameTime → hw_core` 等の命名）。
- 問題発生時は `git revert <commit>` で単一マイルストーン単位でロールバック。
- コミット前に必ず `cargo check --workspace` が通っていることを確認する。

## 9. AI 引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1（GameTime → hw_core から開始）

### 次の AI が最初にやること

1. M1 の Step 1 から順番に作業を開始する。
2. 各 Step 後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` を実行して確認する。

### ブロッカー/注意点

- **`CARGO_HOME` プレフィックスを必ずつけてコマンドを実行すること**（ルートユーザーとしての実行時にキャッシュが別になるため）。
- `game_time_system` は `ClockText`（`hw_ui` 型）を使うため **絶対に hw_core へ移動してはいけない**。型定義のみ移動する。
- `build_dream_tree_planting_plan()` は `GameAssets` / `DreamPool` 等の bevy_app 固有型を引数に持つため、**関数本体は bevy_app に残す**。
- `hw_world → hw_jobs` の依存があるため、`DreamTreePlantingPlan` を `hw_jobs` に入れると将来 `hw_jobs → hw_world` が必要になり**循環依存**になる。必ず `hw_world` に入れること。

### 参照必須ファイル

- `docs/crate-boundaries.md`
- `crates/hw_core/src/constants.rs`（定数の所在確認）
- `crates/bevy_app/src/systems/command/mod.rs`（`AreaEditHandleKind` の定義 → hw_ui へ移動対象）
- `crates/bevy_app/src/systems/command/area_selection/state.rs`（移動前の型定義一式）

### 最終確認ログ

- 最終 `cargo check`: N/A（未着手）
- 未解決エラー: なし

### Definition of Done

- [ ] M1: `GameTime` が `hw_core::time::GameTime` として参照可能
- [ ] M2: `DreamTreePlantingPlan` が `hw_world::DreamTreePlantingPlan` として参照可能
- [ ] M3: `AreaEditSession` 等が `hw_ui::area_edit` から参照可能
- [ ] M4: `Room` / `RoomTileLookup` 等が `hw_world` から参照可能
- [ ] 影響ドキュメント（`docs/architecture.md` 等）が更新済み
- [ ] `cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-13` | Gemini Agent | 初版ドラフト作成 |
| `2026-03-13` | Copilot | コード調査に基づき具体的手順・リスク・注意点を全面ブラッシュアップ。DreamTreePlantingPlan の移動先を hw_jobs → hw_world に訂正。M3 の pub(super) 可視性問題を特定・対処方針を追記。作業順序を明確化。 |