# Phase 3 詳細実装計画: 命名規則のドキュメント化 & GridPos 型エイリアス

## 背景

Phase 1・2 で構造整理・ファサード削除が完了した。  
Phase 3 は「命名規則」と「共通型」に関する整備で、**コードの書き換えを最小限に保ちながら**
将来の一貫性を確立する。

---

## Phase 3-A: DEVELOPMENT.md に命名規則を追記

### 問題

コードベースには `_system` サフィックス付き関数定義が約 90 本、`on_*` プレフィックスの
Observer が 30 本以上あるが、その規約が DEVELOPMENT.md に記述されていない。  
新規コードや AI エージェントがルールを推測に頼って実装することになる。

### 調査結果（実際の命名パターン）

| 関数の種別 | 主なパターン | 例 |
|-----------|------------|-----|
| システム関数 | `{verb}_{topic}_system` | `update_resource_spatial_grid_system` |
| Observer | `on_{event}` | `on_task_assigned`, `on_building_added` |
| ヘルパー | 規則なし（サフィックス不要） | `process_task_delegation_and_movement`, `apply_door_state` |
| AI フェーズ内 | `{phase}_{topic}_system` | `pathfinding_system`, `drifting_decision_system` |

**主要な動詞一覧（実際の出現数順）**:
| 動詞 | 主な用途 |
|------|---------|
| `update` | コンポーネントの状態変更、UI の再描画 |
| `sync` | 2 システム間のデータ同期・整合取り |
| `apply` | メッセージキューの消費・実体への反映 |
| `cleanup` | 不要エンティティ・コンポーネントの削除 |
| `detect` | 状態変化のスキャン（rooms, commands, state） |
| `spawn` | エンティティの生成 |
| `tick` | タイマー進行 |
| `animate` / `animation` | ビジュアルアニメーション |
| `perceive` / `decide` / `execute` | AI フェーズ対応システム |

### 対象ファイル

- **変更**: `docs/DEVELOPMENT.md`（既存ルール 11 の後に新しいセクション 12 として追記）

### 追記内容（Markdown）

```markdown
### 12. 関数命名規則

#### 12.1 システム関数（`add_systems` で登録するもの）

`{動詞}_{対象}_{..._}system` の形式を使う。

```rust
// ✅ 推奨
pub fn update_resource_spatial_grid_system(...) { ... }
pub fn cleanup_commanded_souls_system(...) { ... }
pub fn sync_wall_tile_visual_system(...) { ... }
pub fn apply_task_assignment_requests_system(...) { ... }

// ❌ 動詞なし（理由が伝わらない）
pub fn resource_spatial_grid_system(...) { ... }
pub fn animation_system(...) { ... }
```

**承認済みの動詞**:

| 動詞 | 意図 |
|------|------|
| `update` | コンポーネント値の更新・UI 再描画 |
| `sync` | 2つのデータ間の整合取り |
| `apply` | メッセージ/リクエストキューの消費・反映 |
| `cleanup` | エンティティ・コンポーネントの削除 |
| `detect` | 状態変化のスキャン |
| `spawn` | エンティティ生成 |
| `tick` | タイマーの進行 |
| `animate` | ビジュアルアニメーション更新 |
| `perceive` | AI Perceive フェーズ |
| `decide` | AI Decide フェーズ |
| `execute` | AI Execute フェーズ |
| `process` | 複数ステップの複合処理（ヘルパー化できない場合） |

#### 12.2 Observer 関数（`add_observer` で登録するもの）

`on_{イベント名}` の形式を使う。

```rust
// ✅ 推奨
pub fn on_task_assigned(trigger: Trigger<TaskAssigned>, ...) { ... }
pub fn on_building_added(trigger: Trigger<BuildingAdded>, ...) { ... }

// ❌ _system サフィックスを使わない（Bevy の System ではない）
pub fn task_assigned_system(trigger: Trigger<TaskAssigned>, ...) { ... }
```

#### 12.3 ヘルパー関数（`add_systems` / `add_observer` で直接登録しないもの）

`_system` サフィックスを付けない。名前は動詞から始める自由形式。

```rust
// ✅ ヘルパー（Bevy に直接登録されない）
pub fn process_task_delegation_and_movement(...) { ... }
pub fn apply_door_state(world_map: &mut WorldMap, ...) { ... }
pub fn is_soul_available_for_work(assigned: &AssignedTask) -> bool { ... }
```
```

---

## Phase 3-B: GridPos 型エイリアスを hw_core に追加

### 問題

グリッド座標を表す `(i32, i32)` が 276 箇所に出現しているが、型名がない。  
`grid_pos: (i32, i32)` を読んだとき「これが何を表しているか」は文脈依存。

### 解決策

`type GridPos = (i32, i32)` を `hw_core` に追加する。  

Rust の type alias は完全透過（型互換）なので、**呼び出し側のタプルリテラル `(3, 4)` は変更不要**。  
`GridPos` に変えるのは宣言側（struct フィールド・関数シグネチャ）のみ。

### スコープ（変更する箇所）

全 276 箇所を一括変換するのではなく、**意味が明確に向上する主要箇所のみ**変更する。

| 対象 | 変更数 | 意義 |
|------|-------|------|
| `hw_core/src/world.rs` | 1（型定義追加） | GridPos の定義元 |
| `hw_core/src/lib.rs` | 1（pub use） | クレート全体への公開 |
| `hw_core/src/events.rs` | 1（フィールド） | `grid: (i32, i32)` → `grid: GridPos` |
| `hw_jobs/src/construction.rs` | 4（フィールド+メソッド引数） | `grid_pos: (i32, i32)` → `grid_pos: GridPos` |
| `hw_world/src/room_detection/ecs.rs` | 1（`grid_pos` フィールドのみ。同ファイル内の `tiles`・`tile_to_room`・`dirty_tiles` 等は対象外） | `grid_pos: (i32, i32)` → `grid_pos: GridPos` |
| `hw_world/src/pathfinding.rs` | 8（関数シグネチャ） | 公開 API の明確化 |

合計: 約 16 箇所の変更。

### 実装内容

#### ステップ1: `hw_core/src/world.rs` に型エイリアスを追加

```rust
// 先頭に追加
/// グリッド座標（マップ上の整数タイル位置 x, y）
pub type GridPos = (i32, i32);
```

#### ステップ2: `hw_core/src/lib.rs` に pub use 追加

```rust
pub use world::GridPos;  // 既存の pub use time::GameTime; の後に追記
```

#### ステップ3: `hw_core/src/events.rs` のフィールド更新

```rust
// Before:
pub grid: (i32, i32),
// After:
pub grid: GridPos,
```
（use 行に `use hw_core::GridPos;` または `use crate::GridPos;` を追記）

#### ステップ4: `hw_jobs/src/construction.rs` の更新

```rust
use hw_core::GridPos;  // 追加

// Before:
pub grid_pos: (i32, i32),
// After:
pub grid_pos: GridPos,

// コンストラクタ引数も同様:
pub fn new(parent_site: Entity, grid_pos: GridPos) -> Self { ... }
```
2つの構造体（FloorTileDesignation, WallTileDesignation相当）に適用。

#### ステップ5: `hw_world/src/room_detection/ecs.rs` の更新

```rust
use hw_core::GridPos;  // 追加

pub grid_pos: GridPos,
```

#### ステップ6: `hw_world/src/pathfinding.rs` の公開関数シグネチャを更新

```rust
use hw_core::GridPos;  // 追加

// find_path
pub fn find_path<W: PathWorld>(
    world: &W,
    ctx: &mut PathfindingContext,
    start: GridPos,
    goal: GridPos,
    policy: PathGoalPolicy,
) -> Option<Vec<GridPos>>

// find_path_to_adjacent
pub fn find_path_to_adjacent<W: PathWorld>(
    world: &W,
    ctx: &mut PathfindingContext,
    start: GridPos,
    target: GridPos,
    include_diagonal: bool,
) -> Option<Vec<GridPos>>

// can_reach_target
pub fn can_reach_target<W: PathWorld>(
    world: &W,
    ctx: &mut PathfindingContext,
    start: GridPos,
    target: GridPos,
) -> bool

// find_path_to_boundary
pub fn find_path_to_boundary<W: PathWorld>(
    world: &W,
    ctx: &mut PathfindingContext,
    start: GridPos,
    target_grids: &[GridPos],
) -> Option<Vec<GridPos>>

// find_path_world_waypoints (Phase 2-A で追加)
pub fn find_path_world_waypoints(
    world_map: &crate::map::WorldMap,
    pf_context: &mut PathfindingContext,
    start_grid: GridPos,
    goal_grid: GridPos,
) -> Option<Vec<bevy::math::Vec2>>
```

また、`PathWorld` トレイトの `idx_to_pos` 戻り値も更新:
```rust
pub trait PathWorld {
    fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize>;
    fn idx_to_pos(&self, idx: usize) -> GridPos;  // (i32, i32) → GridPos
    fn is_walkable(&self, x: i32, y: i32) -> bool;
    fn get_door_cost(&self, x: i32, y: i32) -> i32;
}
```

### 注意事項

- `hw_world` は `hw_core` に依存済みなので、`use hw_core::GridPos;` は新たな依存追加なし
- `hw_jobs` も `hw_core` に依存済み
- pathfinding.rs の内部（`astar_impl` 等の private 関数）は変更不要（`(i32, i32)` のまま可）
- テスト内 `TestWorld` の `fn idx_to_pos` 戻り値も `GridPos` に変更が必要

---

## 実施手順

```
Phase 3-A: DEVELOPMENT.md 追記 → 確認（cargo check 不要）
Phase 3-B: hw_core → hw_world → hw_jobs の順 → cargo check
```

## 検証コマンド

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 変更ファイル一覧

### Phase 3-A
| 操作 | ファイル |
|------|---------|
| 変更 | `docs/DEVELOPMENT.md`（セクション 12 追記） |

### Phase 3-B
| 操作 | ファイル |
|------|---------|
| 変更 | `crates/hw_core/src/world.rs`（GridPos 型定義追加） |
| 変更 | `crates/hw_core/src/lib.rs`（pub use GridPos 追加） |
| 変更 | `crates/hw_core/src/events.rs`（grid フィールド型変更） |
| 変更 | `crates/hw_jobs/src/construction.rs`（grid_pos フィールド・引数型変更） |
| 変更 | `crates/hw_world/src/room_detection/ecs.rs`（grid_pos フィールド型変更） |
| 変更 | `crates/hw_world/src/pathfinding.rs`（公開関数シグネチャ更新） |
