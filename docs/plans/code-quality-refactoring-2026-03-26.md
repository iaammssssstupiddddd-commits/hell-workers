# コード品質リファクタリング（大ファイル分割・重複除去）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `code-quality-refactoring-2026-03-26` |
| ステータス | `Draft` |
| 作成日 | `2026-03-26` |
| 最終更新日 | `2026-03-26` |
| 作成者 | `Copilot` |
| 関連提案 | [refactoring-plan-2026-03-23.md](refactoring-plan-2026-03-23.md)（`Complete`）— コードベース整理・DRY の広域計画は完了済み。本計画は **大ファイル分割と局所重複** に絞った追補。内容の重複があればそちらを優先し、本書はファイル単位の実行手順として使う。 |
| 関連Issue/PR | N/A |

## 1. 目的

- **解決したい課題**: 300〜460行の大ファイルが複数存在し、責務混在・重複パターンが保守コストを高めている
- **到達したい状態**: 各ファイルが単一責務を持ち、重複ロジックが共通ヘルパーに集約されている
- **成功指標**:
  - 対象ファイルの行数が 200 行以下（または明確な単一責務）
  - `cargo check` ゼロエラー・ゼロ Clippy 警告を維持
  - ロジックの振る舞いに変更なし

## 2. スコープ

### 対象（In Scope）

- ファイル分割（新モジュール作成と `mod.rs` / `pub use` 更新）
- 重複コードの共通ヘルパー関数への抽出
- 関数の引数整理（同一パラメータグループのまとめ）

### 非対象（Out of Scope）

- ロジック変更・機能追加
- 新しいクレートの追加
- 3D RTT 関連の変更
- `cargo clippy` で新規警告を生む変更

## 3. 現状とギャップ

- **現状**: Clippy ゼロ・dead code ゼロの良好な状態。ただし 300〜460 行の大ファイルが 8 本（計 3,022 行）存在。
- **問題**: 一部のファイルで複数の責務が混在し、ほぼ同一のコードが 3〜6 回重複している。
- **ギャップ**: ファイル分割と重複除去により、可読性・変更局所性を向上させる。

## 4. 実装方針（高レベル）

- **方針**: 純粋な構造整理のみ。移動した関数は `pub use` で後方互換を保つ。
- **設計上の前提**: Bevy の `SystemParam` 分割は lifetimes を明示して対応。Query 型は型エイリアスで type_complexity を回避。
- **Bevy 0.18 の注意点**: `QueryLens` / `transmute_lens_filtered` は Bevy 0.18 固有 API。既存パターンをそのまま活用。

## 5. マイルストーン

---

### M1: `hw_visual` — `conversation/systems.rs` 分割（378行）

**問題**: `process_conversation_logic`（L151〜L333）に emoji 選択・トーン判定・バブル生成が混在。同一の `spawn_soul_bubble` 呼び出しパターンが 3 箇所に重複（L183-194, L230-262, L285-296）。

**重複パターン（具体例）:**
```rust
// L183-194 / L230-241 / L285-296 — 3箇所で同じ形
let emoji = EMOJIS_XXX.choose(&mut rng).expect("...");
spawn_soul_bubble(&mut commands, entity, emoji, pos, &handles,
    BubbleEmotion::XXX, BubblePriority::Normal);
```

**変更内容**:

新規 `bubble_spawn_helpers.rs` を作成:
```rust
// 3つの重複を関数に集約
pub fn spawn_greeting_bubble(commands, entity, pos, handles, rng)
pub fn spawn_chatting_bubble(commands, entity, pos, handles, rng, participant, ev_tone)
  // BubbleEmotion の判定ロジック（L232-253）ここに移動
pub fn spawn_agreement_bubble(commands, entity, pos, handles, rng, participant, ev_tone)
```

`systems.rs` は 6 つのシステム関数のみに縮小:
- `check_conversation_triggers` (L52)
- `handle_conversation_requests` (L118)
- `process_conversation_logic` → bubble spawn 呼び出しのみに（ロジックは helpers に委譲）
- `end_conversation` (L335)
- `apply_conversation_rewards` (L344)
- `update_conversation_cooldowns` (L366)

**変更ファイル**:
- `crates/hw_visual/src/speech/conversation/systems.rs`（縮小）
- `crates/hw_visual/src/speech/conversation/bubble_spawn_helpers.rs`（新規）
- `crates/hw_visual/src/speech/conversation/mod.rs`（`mod bubble_spawn_helpers;` 追加）

**完了条件**:
- [ ] `systems.rs` が 200 行以下
- [ ] `spawn_soul_bubble` を直接呼ぶ 3 箇所がヘルパー経由に統一
- [ ] `cargo check` ゼロエラー

**検証**: `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

---

### M2: `hw_soul_ai` — `task_execution/common.rs` 分割（444行）

**問題（A: パス・移動）**: パスキャッシュ検証ロジックが 2 関数（`update_destination_to_adjacent` L22、`update_destination_to_blueprint` L135）にほぼ同一の形で重複（L34-45, L157-171）。末尾のウェイポイント設定パターン（L113-125, L180-185）も重複。

**問題（B: Collect クリーンアップ）**: `cleanup_collect_target`（L378）と `finalize_collect_task`（L429）で **クリーンアップ手順が同一**。こちらは `path_cache` ではなく **Collect タスク専用**のため、`path_cache.rs` には載せない。

**重複パターン（具体例）:**
```rust
// L34-45 と L157-171 — パスキャッシュ検証
if !path.waypoints.is_empty() && path.current_index < path.waypoints.len()
    && let Some(last_wp) = path.waypoints.last() {
        let last_grid = WorldMap::world_to_grid(*last_wp);
        let dx = (last_grid.0 - target_grid.0).abs();
        let dy = (last_grid.1 - target_grid.1).abs();
        if dx <= 1 && dy <= 1 { dest.0 = *last_wp; return true; }
    }

// L113-125 と L180-185 — ウェイポイント設定
path.waypoints = grid_path.iter()
    .map(|&(x, y)| WorldMap::grid_to_world(x, y)).collect();
path.current_index = 0;
```

**変更内容**:

新規 `path_cache.rs` を作成（問題 A のみ）:
```rust
// パスキャッシュ検証の共通ロジック
pub fn is_path_cache_valid_for_adjacent(path, dest, target_grid) -> bool
  // L34-45 の実装をここに集約

pub fn is_path_cache_valid_for_blueprint(path, occupied_grids) -> bool
  // L157-171 の実装をここに集約

pub fn apply_grid_path(path, dest, grid_path) -> bool
  // L113-125 / L180-185 を共通化
```

`common.rs` 内で問題 B に対応:
- `cleanup_collect_target` / `finalize_collect_task` から呼ぶ **private 共通手順**（現行 2 関数の共通部分のみ）を 1 つにまとめる。2 関数にしか残らない差分（引数・早期 return 等）は各呼び出し側に残す。

`common.rs` に残すその他の代表（現行の汎用ヘルパー群を維持）:
- `clear_task_and_path` (L192)
- `cancel_task_if_designation_missing` (L201)
- `pickup_item` / `drop_item` (L216, L240)
- `update_stockpile_on_item_removal` (L249)
- `release_mixer_mud_storage_for_item` (L271)
- `try_pickup_item` (L298)
- `navigate_to_adjacent` / `navigate_to_pos` (L346, L402) → `path_cache` を内部利用
- `cleanup_collect_target` / `finalize_collect_task` (L378, L429) → 上記 private 共通化を利用

**変更ファイル**:
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/common.rs`（縮小・Collect 共通 private 追加）
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/path_cache.rs`（新規）
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/mod.rs`（`pub mod path_cache;` 追加）

**完了条件**:
- [ ] `common.rs` が 280 行以下
- [ ] パスキャッシュ検証の 2 箇所の重複が `path_cache.rs` のヘルパーに統一
- [ ] `cleanup_collect_target` と `finalize_collect_task` の重複手順が `common.rs` の private ヘルパー（1 箇所）に集約されている
- [ ] `cargo check` ゼロエラー

**検証**: `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

---

### M3: `bevy_app` — `intent_handler.rs` 分割（364行）

**問題**: `handle_ui_intent`（L50-260）に 21 アームの `match` ブロックが集中。`toggle_menu_and_reset_mode` 呼び出しパターンが 4 回（L90-97, L101-108, L112-119, L123-130）、`set_*_mode` 呼び出しが複数回重複。加えて、`IntentModeCtx` / `IntentSelectionCtx` / `IntentFamiliarQueries` / `IntentUiQueries` が同ファイル内にあり、field も private のため、単純に `handlers/` へ分けても sibling module から再利用しづらい。

**match アームの分類:**

| グループ | アーム | 現在の行 |
|---------|--------|---------|
| 選択/ピン | `InspectEntity`, `ClearInspectPin` | L82-88 |
| トグルメニュー | `ToggleArchitect`, `ToggleOrders`, `ToggleZones`, `ToggleDream` | L89-131 |
| モード選択 | `SelectBuild`, `SelectFloorPlace`, `SelectZone`, `RemoveZone`, `SelectTaskMode`, `SelectAreaTask`, `SelectDreamPlanting` | L132-199 |
| ダイアログ | `OpenOperationDialog`, `CloseDialog` | L200-205 |
| Familiar 設定 | `AdjustFatigueThreshold`, `AdjustMaxControlledSoul`, `AdjustMaxControlledSoulFor` | L206-229 |
| ゲーム速度 | `TogglePause`, `SetTimeSpeed` | L231-252 |
| その他 | `ToggleDoorLock`, `SelectArchitectCategory`, `MovePlantBuilding` | L253-258 |

**変更内容**:

新規 `intent_context.rs` と `handlers/` ディレクトリを作成:

`intent_context.rs`:
```rust
// intent_handler.rs から移動
#[derive(SystemParam)]
pub(crate) struct IntentModeCtx<'w> { pub(crate) ... }
#[derive(SystemParam)]
pub(crate) struct IntentSelectionCtx<'w> { pub(crate) ... }
#[derive(SystemParam)]
pub(crate) struct IntentFamiliarQueries<'w, 's> { pub(crate) ... }
#[derive(SystemParam)]
pub(crate) struct IntentUiQueries<'w, 's> { pub(crate) ... }

pub(crate) fn ensure_familiar_selected(...)
```

`handlers/mod.rs`:
```rust
pub(crate) mod general;
pub(crate) mod mode_selection;
pub(crate) mod mode_toggle;
pub(crate) mod familiar_settings;
```

`handlers/mode_toggle.rs`:
```rust
// toggle_menu_and_reset_mode の 4 回重複呼び出しを 1 関数に統合
pub fn handle_toggle_mode_intents(
    intent: &UiIntent, mode_ctx: &mut IntentModeCtx,
) -> bool  // 処理したか否か
```

`handlers/familiar_settings.rs`:
```rust
// L206-229 の Familiar 設定 3 アームを移動
pub fn handle_familiar_setting_intents(
    intent: &UiIntent, selection_ctx: &mut IntentSelectionCtx,
    familiar_queries: &mut IntentFamiliarQueries,
) -> bool
// 既存 private fn もここへ移動:
//   adjust_fatigue_threshold (L292)
//   adjust_max_controlled_soul (L304)
//   update_familiar_max_soul_header (L330)
//   familiar_state_label (L357)
```

`handlers/general.rs`:
```rust
// 選択/ピン、ダイアログ、ゲーム速度、no-op variants を処理
pub fn handle_general_intents(...) -> bool
```

**行数目標の定義（`intent_handler.rs` の「80 行以下」）**: `use` 宣言、`intent_context` / `handlers` import、`handle_ui_intent` の **ディスパッチ層のみ** を指す（`for intent in ...`、早期 `continue`、各 `handlers::...` への **1 行委譲** のみ）。`SystemParam` 定義や helper 本体は含めない。**各アームの本体ロジックは `intent_context.rs` または `handlers/` に置く。**

`intent_handler.rs` はディスパッチャのみに縮小（上記定義で 80 行以下）:
```rust
pub(crate) fn handle_ui_intent(intent: &UiIntent, ...) {
    if handlers::general::handle_general_intents(intent, ...) { return; }
    if handle_toggle_mode_intents(intent, &mut mode_ctx) { return; }
    if handle_familiar_setting_intents(intent, ...) { return; }
    if handlers::mode_selection::handle(intent, ...) { return; }
}
```

**変更ファイル**:
- `crates/bevy_app/src/interface/ui/interaction/intent_context.rs`（新規）
- `crates/bevy_app/src/interface/ui/interaction/intent_handler.rs`（縮小）
- `crates/bevy_app/src/interface/ui/interaction/handlers/mod.rs`（新規）
- `crates/bevy_app/src/interface/ui/interaction/handlers/general.rs`（新規）
- `crates/bevy_app/src/interface/ui/interaction/handlers/mode_toggle.rs`（新規）
- `crates/bevy_app/src/interface/ui/interaction/handlers/familiar_settings.rs`（新規）
- `crates/bevy_app/src/interface/ui/interaction/handlers/mode_selection.rs`（新規）
- `crates/bevy_app/src/interface/ui/interaction/mod.rs`（`mod intent_context; mod handlers;` 追加）

**完了条件**:
- [ ] `intent_handler.rs` が **ディスパッチ層の定義どおり** 80 行以下（長い処理は `handlers/` 側）
- [ ] `Intent*Ctx` / `Intent*Queries` が `intent_context.rs` に移動し、`handlers/` から `pub(crate)` で利用できる
- [ ] `toggle_menu_and_reset_mode` 重複 4 箇所が 1 関数に統合
- [ ] Familiar 設定 private fn 群が `familiar_settings.rs` に集約
- [ ] `cargo check` ゼロエラー

**検証**: `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

---

### M4-A: `hw_familiar_ai` — `state_decision/system.rs` 重複除去（339行）

**問題**: `query transmute` パターンが 6 回（L98-106, L133-142, L196-203, L216-224, L263-269, L281-289）ほぼ同一で重複。

**重複パターン（具体例）:**
```rust
// 6箇所で同じ形（コンポーネントセットだけが異なる）
let mut q_lens = q_souls.transmute_lens_filtered::<(
    Entity, &Transform, &DamnedSoul, &AssignedTask, Option<&CommandedBy>,
), Without<Familiar>>();
let q = q_lens.query();
```

**変更内容**:

`system.rs` 内に **宣言的マクロ**（推奨）または借用が許せば **関数ラッパー** で 6 箇所の `transmute_lens_filtered::<...>` をまとめる。コンポーネントセットが 3 種類あるため、マクロに型引数を渡す形が現実的なことが多い。

**実装時の注意（型・ライフタイム）**: 以下は方向性のメモであり、**正しい戻り値型としては使わないこと**。Bevy 0.18 の `SoulQuery` / `Query` / `transmute_lens_filtered` が要求する **実際のタプル型とライフタイム** に合わせて書く。`&'static Component` のような誤ったライフタイムを付けるとコンパイルに失敗する。

```rust
// 例: マクロで「レンズ生成 + query()」までを 1 箇所に集約（シグネチャは実コードに合わせて定義）
// macro_rules! soul_lens { ... }
```

※ Bevy の `transmute_lens_filtered` は `&mut Query` を消費するため、  
  Rust の借用規則上 ヘルパー化には注意が必要。  
  難しい場合は `// 理由: transmute_lens は &mut self を消費` コメントを付けて現状維持とする。

**変更ファイル**:
- `crates/hw_familiar_ai/src/familiar_ai/decide/state_decision/system.rs`

**完了条件**:
- [ ] transmute 重複が 3 種のヘルパー呼び出しに置き換えられている（または借用制約で不可の旨コメント記載）
- [ ] `cargo check` ゼロエラー

---

### M4-B: `hw_familiar_ai` — `validator/resolver.rs` 分割（336行）

**問題**: `effective_free` 計算パターンが 3 箇所（L70-80, L96-106, L147-152）に重複。水物流ロジック（`resolve_gather_water_inputs` L127、`find_tank_bucket_for_water_mixer` L271-336）が肥大化。

**重複パターン（具体例）:**
```rust
// L70-80 / L96-106 / L147-152 — 3箇所で同じ形
let incoming = queries.reservation.incoming_deliveries_query
    .get(entity).ok().map(|(_, inc)| inc.len()).unwrap_or(0);
let shadow_incoming = shadow.destination_reserved_total(entity);
let effective_free = stock.capacity.saturating_sub(stored + incoming + shadow_incoming);
```

**変更内容**:

新規 `capacity_helpers.rs` を作成:
```rust
pub fn get_incoming_count(
    queries: &FamiliarTaskAssignmentQueries, entity: Entity,
) -> usize

pub fn effective_free_capacity(
    stored: usize,
    capacity: usize,
    incoming: usize,
    shadow_incoming: usize,
) -> usize
```

新規 `water_resolver.rs` を作成:
```rust
// resolver.rs から移動
pub fn resolve_gather_water_inputs(...) -> Option<(Entity, Entity)>
pub fn resolve_haul_water_to_mixer_inputs(...) -> Option<(Entity, Entity, Entity)>
fn find_tank_bucket_for_water_mixer(...) -> Option<(Entity, Entity)>
```

`validator/mod.rs` では `water_resolver` を公開する。可能なら **`pub use water_resolver::*;` のワイルドカードは避け**、`pub use water_resolver::{resolve_gather_water_inputs, resolve_haul_water_to_mixer_inputs};` のように **明示列挙**する（アーカイブ計画 `re-export-consolidation` と同方針。外部から参照しているシンボルが増えたら列挙に追加）。

**変更ファイル**:
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/resolver.rs`（縮小）
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/capacity_helpers.rs`（新規）
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/water_resolver.rs`（新規）
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/mod.rs`（更新）

**完了条件**:
- [ ] `effective_free` 計算 3 箇所が `capacity_helpers` 経由に統一
- [ ] `resolver.rs` が 220 行以下
- [ ] 水物流 2 関数 + プライベートヘルパーが `water_resolver.rs` に移動
- [ ] `validator/mod.rs` の `water_resolver` 再公開は **`pub use ...::*` を避け**、必要シンボルを明示列挙している
- [ ] `cargo check` ゼロエラー

---

### M5: `hw_ui` — `list/sync.rs` 共通化（390行）

**問題**: `sync_familiar_member_rows`（L26-141）と `sync_unassigned_souls`（L282-390）が 6 段階の sync 処理を同じ順序で実行（stale 削除 → 生成/更新 → 並び替え）。各段階の実装が ~90% 同一。

**近似重複の対応箇所:**

| 処理段階 | familiar側 | unassigned側 |
|---------|-----------|-------------|
| stale 削除 | L81-90 | L331-341 |
| 生成/更新 | L96-126 | L343-377 |
| 並び替え | L128-140 | L379-389 |

**変更内容**:

`sync.rs` 内に private ヘルパーを追加（新ファイルは作らず、関数抽出のみ）:
```rust
fn remove_stale_rows(
    commands: &mut Commands,
    current_ids: impl Iterator<Item = Entity>,
    rows: &mut HashMap<Entity, Entity>,
)
// L81-90 / L331-341 を共通化

fn reorder_row_children(
    commands: &mut Commands,
    container: Entity,
    ordered_ids: impl Iterator<Item = Entity>,
    rows: &HashMap<Entity, Entity>,
)
// L128-140 / L379-389 を共通化
```

`sync_familiar_member_rows` と `sync_unassigned_souls` はこれらを呼び出す形に変更。

**変更ファイル**:
- `crates/hw_ui/src/list/sync.rs`（共通ヘルパー追加・重複削除）

**完了条件**:
- [ ] stale 削除・並び替えの重複が共通関数に統一
- [ ] `sync.rs` が 300 行以下
- [ ] `cargo check` ゼロエラー

---

### M6: `bevy_app` — `building_move/system.rs` 分割（454行）

**問題**: UI 状態受付（L90-180）・衝突/配置検証（L214-268）・移動確定+タスクキャンセル（L308-454）の 3 責務が 1 ファイルに混在。現状 454 行あり、`finalize_move_request` 以降だけを外へ出しても `system.rs` 側に 300 行超が残るため、行数目標を満たせない。

**3責務の行範囲:**

| 責務 | 行範囲 | 内容 |
|-----|--------|------|
| UI 状態 | L90-180 | 入力検証・同伴クリック/初期クリックの分岐 |
| 配置検証 | L214-268 | `can_place_moved_building` / `validate_tank_companion_for_move` |
| 確定処理 | L308-454 | `finalize_move_request` / `cancel_tasks_and_requests_for_moved_building` / `task_targets_building` |

**変更内容**:

新規 `context.rs` を作成:
```rust
// system.rs 先頭の型定義を移動
type SoulTaskQuery<'w, 's> = Query<...>;
const COMPANION_PLACEMENT_RADIUS_TILES: f32 = 5.0;

#[derive(SystemParam)]
pub struct BuildMoveInput<'w, 's> { ... }
#[derive(SystemParam)]
pub struct BuildMoveState<'w> { ... }
#[derive(SystemParam)]
pub struct BuildMoveQueries<'w, 's> { ... }

pub(super) struct MoveStateCtx<'a> { ... }
pub(super) struct MoveOpCtx<'a, ...> { ... }
```

新規 `click_handlers.rs` を作成:
```rust
pub(super) fn handle_companion_click(...)
pub(super) fn handle_initial_click(...)
pub(super) fn clear_move_states(...)
```

新規 `finalization.rs` を作成:
```rust
// system.rs L308-454 から移動
pub fn finalize_move_request(op, target_entity, building, transform,
    destination_grid, companion_anchor)
fn cancel_tasks_and_requests_for_moved_building(commands, building_entity, ...)
fn task_targets_building(task, building_entity) -> bool
```

`system.rs` の残留内容:
- `building_move_system`（入力受付・分岐・`click_handlers` / `finalization` 呼び出し）
- `use super::{click_handlers, context, finalization};` 程度の import

**変更ファイル**:
- `crates/bevy_app/src/interface/selection/building_move/context.rs`（新規）
- `crates/bevy_app/src/interface/selection/building_move/click_handlers.rs`（新規）
- `crates/bevy_app/src/interface/selection/building_move/system.rs`（縮小）
- `crates/bevy_app/src/interface/selection/building_move/finalization.rs`（新規）
- `crates/bevy_app/src/interface/selection/building_move/mod.rs`（更新）

**完了条件**:
- [ ] `system.rs` が 180 行以下（entry system と薄い分岐のみに限定）
- [ ] 型定義群（`SoulTaskQuery`, `BuildMove*`, `Move*Ctx`, 定数）が `context.rs` に移動
- [ ] companion/initial click と state reset が `click_handlers.rs` に移動
- [ ] `finalize_move_request` 以降 3 関数が `finalization.rs` に移動
- [ ] `cargo check` ゼロエラー

---

### M7: 低優先度の小規模整理（任意）

M1〜M6 完了後に判断。

| ファイル | 行数 | 対応内容 |
|---------|------|---------|
| `soul_ai/decide/idle_behavior/system.rs` | 397 | 決定パスをサブ関数へ委譲 |
| `familiar_ai/.../haul/source_selector.rs` | 353 | wheelbarrow 収集の半径違いのみの 2 関数を 1 関数に統合 |
| `hw_world/src/room_systems.rs` | 364 | 必要に応じた Observer ボイラープレート整理 |

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `pub use` 再エクスポート漏れ | コンパイルエラー | 各 M 完了時に `cargo check` を必ず実行 |
| Clippy `type_complexity` 警告 | Clippy ゼロ違反 | 分割した Query 型に型エイリアスを追加 |
| `transmute_lens` の借用制約（M4-A） | ヘルパー化不可 | 不可の場合は理由コメントを付けて現状維持 |
| 並行作業との競合 | マージ競合 | M 単位で 1 ファイルずつ完結させる |

## 7. 検証計画

- **必須**: `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`（各 M 完了ごと）
- **必須**: `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace`（全 M 完了後）
- **手動確認**: `cargo run` で起動し既存動作に変化がないことを目視確認（全 M 完了後）

## 8. ロールバック方針

- 各 M は独立して進め、**M ごとにコミットを分ける**。戻す場合は `git revert` 相当の**打ち消しコミット**を基本とする
- ファイル単位の手戻りが必要でも、**`git checkout -- <file>` は使わない**。先に `git log --oneline -5` と `git diff HEAD -- <file>` で破棄内容を確認し、必要なら reverse patch / 手修正で戻す
- M 開始前に作業ブランチを切ることを推奨

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M7 すべて未着手

### 次のAIが最初にやること

1. このファイルを読む
2. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` でベースライン確認
3. M1 から着手（`crates/hw_visual/src/speech/conversation/systems.rs` を全文読む）
4. 本書の **行番号（Lxxx）はマージでズレうる**。着手前に **関数名で grep** し、記載行は目安として再定位する

### ブロッカー/注意点

- ロジック変更は行わない（純粋な構造整理）
- M4-A の `transmute_lens_filtered` は `&mut self` を消費するため、ヘルパー関数化が Rust 借用規則上できない場合がある。無理に変更せずコメントで理由を記載する
- M3 は `handlers/` だけでなく `intent_context.rs` を追加し、`Intent*Ctx` / `Intent*Queries` と `ensure_familiar_selected` を先に移す。handler 側から使う field は `pub(crate)` にする
- M6 は `finalization.rs` だけでは行数目標に届かない。`context.rs` と `click_handlers.rs` も同じ M で追加して `system.rs` を entrypoint に絞る
- 広域の整理方針と重なる場合は [refactoring-plan-2026-03-23.md](refactoring-plan-2026-03-23.md)（完了済み）と矛盾がないかだけ確認する

### 参照必須ファイル

- `docs/architecture.md`
- `docs/plans/code-quality-refactoring-2026-03-26.md`（本ファイル）
- `docs/plans/refactoring-plan-2026-03-23.md`（関連・完了済みの広域整理）
- `CLAUDE.md`

### 最終確認ログ

- 最終 `cargo check`: 未実施
- 未解決エラー: なし（ベースライン確認前）

### Definition of Done

- [ ] M1〜M6 のすべての完了条件を満たす
- [ ] `cargo check` ゼロエラー
- [ ] `cargo clippy --workspace` ゼロ警告
- [ ] `cargo run` で既存動作に変化なし

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-26` | `Copilot` | 初版作成 |
| `2026-03-26` | `Copilot` | 各ファイルの実際のコードを読んで具体化（関数名・行番号・重複コード例を記載） |
| `2026-03-26` | レビュー反映 | M2 の Collect 共通化を変更内容・完了条件に追記。M3 の 80 行定義と `mode_selection.rs`。M4-A の型メモ修正。M4-B の明示 `pub use`。関連計画・行番号注意・参照ファイルを更新。 |
| `2026-03-26` | `Codex` | M3 に `intent_context.rs` / `handlers/general.rs` を追加して分割成立条件を明記。M6 に `context.rs` / `click_handlers.rs` を追加して行数目標を実現可能化。ロールバック方針を AGENTS.md 準拠に修正。 |
