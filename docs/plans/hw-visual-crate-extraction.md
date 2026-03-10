# hw_visual クレート化 実装計画

## 概要

`src/systems/visual/`（59ファイル・6,649行）および `src/systems/utils/`（5ファイル・469行）を
`hw_visual` クレートとして切り出す。

### 設計方針（改訂版）

**ゲーム固有のリソースは hw_core に持ち込まない**:
- `GameAssets`（134フィールドのゲーム固有アセットカタログ）は root に残す
- `LatinPhrase`（ゲーム固有のラテン語フレーバーテキスト）は **hw_visual に置く**
- `FamiliarVoice`（LatinPhrase に依存する AI コンポーネント）も **hw_visual に置く**
- hw_visual が必要とするアセットハンドルは hw_visual 内に定義した **ビジュアルハンドルリソース** として持ち、
  root が startup 時に GameAssets から値を注入する

---

## 完了済み作業

| 項目 | 状態 |
|:---|:---|
| `WorldMap` を hw_world に移動 | ✅ Phase 1 以前に完了 |
| レイアウト定数（`RIVER_Y_MIN` 等） | ✅ Phase 1 以前に完了 |
| hw_core が rand に依存 | ✅ Phase 1 以前に完了 |
| `AnimationState` → hw_core::soul | ✅ Phase 1-1 完了 |
| `WorldMapRead` / `WorldMapWrite` → hw_world::map | ✅ Phase 1-2 完了 |
| hw_visual クレート作成 + utils + handles.rs | ✅ Phase 2 完了 |
| `LatinPhrase` → hw_visual::speech | ✅ Phase 3 完了 |
| `FamiliarVoice` → hw_visual::speech | ✅ Phase 3 完了 |
| SpeechPlugin → hw_visual::speech | ✅ Phase 3 完了 |

**未完了（Phase 4〜5）**:

| 項目 | 方針 |
|:---|:---|
| `GameAssets` への依存（19ファイル） | hw_visual 内のビジュアルハンドルリソースに置き換え |
| visual サブシステム移動（dream, blueprint, gather 等） | Phase 4 で順次移動 |
| HwVisualPlugin 統合 + root 側注入コード | Phase 5 で実施 |

---

## 現状の依存分析

### A. 既にクレート化済み（Cargo.toml 追加のみ）

| 依存先 | クレート | visual 内使用ファイル数 |
|:---|:---|---:|
| `DamnedSoul`, `IdleBehavior`, `IdleState`, `DreamQuality` | `hw_core::soul` | 10 |
| `Familiar` | `hw_core::familiar` | 1 |
| 全 Relationships（`CommandedBy` 等） | `hw_core::relationships` | 7 |
| `TaskArea`, `TaskMode`, `PlayMode` | `hw_core::area`, `hw_core::game_state` | 3 |
| `TILE_SIZE`, `Z_*` 等の定数 | `hw_core::constants` | 多数 |
| `Blueprint`, `Building`, `WorkType` 等 | `hw_jobs` | 11 |
| `AssignedTask`, `BuildPhase` 等 | `hw_jobs::assigned_task` | 3 |
| `ResourceType`, `Inventory`, `Wheelbarrow` 等 | `hw_logistics` | 5 |
| `SpatialGrid`, `SpatialGridOps` | `hw_spatial` | 1 |
| `Site`, `Yard`, `AreaBounds` | `hw_world` | 2 |
| `MainCamera`, `HoveredEntity` 等 | `hw_ui` | 5 |

### B. Phase 1〜3 で対処済み

| 型 | 移動先 | 状態 |
|:---|:---|:---|
| `AnimationState` | `hw_core::soul` | ✅ root は re-export 経由 |
| `WorldMapRead` / `WorldMapWrite` | `hw_world::map::access` | ✅ root は re-export 経由 |
| `LatinPhrase` | `hw_visual::speech::phrases` | ✅ root は re-export 経由 |
| `FamiliarVoice` | `hw_visual::speech::voice` | ✅ root は re-export 経由 |
| `SpeechPlugin` | `hw_visual::speech` | ✅ root は re-export 経由 |

### C. Phase 4 で対処（未完了）

| 型 | visual 内使用ファイル | 対処 |
|:---|:---|:---|
| `GameAssets`（19ファイル） | 下記参照 | ビジュアルハンドルリソースに置き換え |

### D. アプリ固有（root 残留）

| 型 | 対応方針 |
|:---|:---|
| `GameAssets`（struct・ロード）| root 残留。startup 時にビジュアルハンドルリソースを populate する |
| `TaskContext` | root 残留。`task_area_visual.rs` のシステム部分も残留 |
| `BuildContext`, `CompanionPlacementState` 等 | root 残留。`placement_ghost.rs` 全体も残留 |
| `DebugVisible` | root 残留。`plugins/visual.rs` が `run_if` で使用 |

---

## ビジュアルハンドルリソース（GameAssets 置き換えの核心）

hw_visual が自ら必要なアセットハンドルを Resource として定義し、
root の startup システムが GameAssets から値を注入するパターン。

### hw_visual 内で定義するリソース一覧（✅ Phase 2 で作成済み）

`crates/hw_visual/src/handles.rs` に以下の 7 Resource が定義済み:

| Resource | 主な使用先 | フィールド数 |
|:---|:---|---:|
| `WallVisualHandles` | wall_connection, wall_construction, floor_construction | 45（石壁16 + ドア2 + 泥壁16 + 泥床1） |
| `BuildingAnimHandles` | mud_mixer, tank | 8 |
| `WorkIconHandles` | gather/worker_indicator, blueprint/worker_indicator | 5 |
| `MaterialIconHandles` | blueprint/material_display, haul/carrying_item, floor/wall_construction | 7（画像6 + font_ui） |
| `HaulItemHandles` | haul/carrying_item | 7 |
| `SpeechHandles` | speech/spawn, speech/emitter, soul | 5（bubble_9slice, glow_circle, font×3） |
| `PlantTreeHandles` | plant_trees/systems | 2 |

### root 側の注入コード（startup）

```rust
// src/plugins/startup/visual_handles.rs（新規）
pub fn init_visual_handles(mut commands: Commands, game_assets: Res<GameAssets>) {
    commands.insert_resource(WallVisualHandles {
        stone_isolated: game_assets.wall_isolated.clone(),
        // ...
    });
    commands.insert_resource(BuildingAnimHandles {
        mud_mixer_idle: game_assets.mud_mixer.clone(),
        // ...
    });
    // ... 残りの resource も同様
}
```

### 変換パターン（hw_visual 各システム）

```
// Before（src/ 内）
fn update_wall(..., game_assets: Res<GameAssets>, ...)
    sprite.image = game_assets.wall_isolated.clone();

// After（hw_visual 内）
fn update_wall(..., handles: Res<WallVisualHandles>, ...)
    sprite.image = handles.stone_isolated.clone();
```

---

## 完了済み型移動の結果

### FamiliarVoice / LatinPhrase（✅ Phase 3 完了）

- 定義: `hw_visual::speech::{phrases::LatinPhrase, voice::FamiliarVoice}`
- root re-export: `src/systems/visual/speech/mod.rs` → `pub use hw_visual::speech::*`
- root re-export: `src/entities/familiar/voice.rs` → `pub use hw_visual::speech::FamiliarVoice`
- familiar_ai 3ファイルは re-export 経由（`crate::systems::visual::speech::phrases::LatinPhrase`）でアクセス
  - Phase 4 で speech re-export 削除時に `hw_visual::speech::LatinPhrase` へ直接参照に更新する

### AnimationState（✅ Phase 1-1 完了）

- 定義: `hw_core::soul::AnimationState`
- root re-export: `src/entities/damned_soul/mod.rs` → `pub use hw_core::soul::AnimationState`

### WorldMapRead / WorldMapWrite（✅ Phase 1-2 完了）

- 定義: `hw_world::map::access::{WorldMapRead, WorldMapWrite}`
- hw_world ディレクトリ化: `map.rs` → `map/mod.rs` + `map/access.rs`
- root re-export: `src/world/map/access.rs` → `pub use hw_world::{WorldMapRead, WorldMapWrite}`

---

## HwVisualPlugin の構成

hw_visual は `HwVisualPlugin` を公開し、Material 登録 + 全システム登録を自ら引き受ける。
root の `VisualPlugin` は薄いラッパーになる。

### hw_visual 側（HwVisualPlugin）

```rust
// crates/hw_visual/src/lib.rs
pub struct HwVisualPlugin;

impl Plugin for HwVisualPlugin {
    fn build(&self, app: &mut App) {
        // Material plugins（型の所有者が登録）
        app.add_plugins(Material2dPlugin::<DreamBubbleMaterial>::default());
        app.add_plugins(UiMaterialPlugin::<DreamBubbleUiMaterial>::default());
        app.add_plugins(Material2dPlugin::<TaskAreaMaterial>::default());

        // サブプラグイン
        app.add_plugins(SpeechPlugin);
        app.add_plugins(WallConnectionPlugin);

        // 全システム登録（GameSystemSet は hw_core から利用可能）
        app.add_systems(Update, (...).chain().in_set(GameSystemSet::Visual));
    }
}
```

### root 側（VisualPlugin 変更後）

```rust
// src/plugins/visual.rs
impl Plugin for VisualPlugin {
    fn build(&self, app: &mut App) {
        // hw_visual の一括登録
        app.add_plugins(HwVisualPlugin);

        // root 残留システムのみ（app_contexts 依存）
        app.add_systems(Update, (
            placement_ghost_system,
            update_task_area_material_system,
        ).in_set(GameSystemSet::Visual));

        // DebugVisible ゲート（root 専有リソース）
        // task_link_system は hw_visual が pub fn で公開、run_if は root が適用
        app.add_systems(Update,
            task_link_system
                .run_if(|debug: Res<DebugVisible>| debug.0)
                .in_set(GameSystemSet::Visual),
        );
    }
}
```

### DebugVisible の扱い

- `DebugVisible` は root に残す（`src/main.rs` で定義・初期化）
- `task_link_system`（`hw_visual::soul` に移動）は `pub fn` として公開のみ
- hw_visual 内部では `task_link_system` をシステム登録 **しない**
- root の `VisualPlugin` が `run_if` 付きで登録する責務を持つ

---

## root に残留するファイル

| ファイル | 残留理由 | hw_visual 側の対応 |
|:---|:---|:---|
| `src/systems/visual/placement_ghost.rs` | `BuildContext`, `CompanionPlacementState` 等（app_contexts） | 変更なし |
| `src/systems/visual/task_area_visual.rs`（システム関数） | `TaskContext`（app_contexts） | `TaskAreaMaterial` + `TaskAreaVisual` 定義のみ hw_visual に分離 |
| `src/plugins/visual.rs` | root 残留システム + `DebugVisible` ゲート | `HwVisualPlugin` を呼び出す薄いラッパーに変更 |

### task_area_visual.rs の分割

```
// hw_visual に移動:
pub struct TaskAreaMaterial { color, size, time, state }
pub struct TaskAreaVisual { familiar: Entity }

// root 残留:
fn update_task_area_material_system(ctx: Res<TaskContext>, ...)
```

---

## hw_visual クレート構成

```
crates/hw_visual/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── handles.rs             ✅ ビジュアルハンドルリソース定義（7 Resource）
    ├── animations.rs          ✅ src/systems/utils/animations.rs から移動済み
    ├── floating_text.rs       ✅ src/systems/utils/floating_text.rs から移動済み
    ├── progress_bar.rs        ✅ src/systems/utils/progress_bar.rs から移動済み
    ├── worker_icon.rs         ✅ src/systems/utils/worker_icon.rs から移動済み
    ├── fade.rs                ✅ src/systems/visual/fade.rs から移動済み
    ├── speech/                ✅ src/systems/visual/speech/ + voice.rs 移動済み
    │   ├── mod.rs, phrases.rs, voice.rs
    │   ├── animation.rs, components.rs, cooldown.rs, emitter.rs
    │   ├── observers.rs, periodic.rs, spawn.rs, typewriter.rs, update.rs
    │   └── conversation/
    ├── blueprint/             ⬚ Phase 4-2 で移動予定
    ├── dream/                 ⬚ Phase 4-1 で移動予定
    ├── gather/                ⬚ Phase 4-3 で移動予定
    ├── haul/                  ⬚ Phase 4-4 で移動予定
    ├── soul.rs                ⬚ Phase 4-6 で移動予定
    ├── mud_mixer.rs           ⬚ Phase 4-7 で移動予定
    ├── tank.rs                ⬚ Phase 4-8 で移動予定
    ├── wall_connection.rs     ⬚ Phase 4-9 で移動予定
    ├── wall_construction.rs   ⬚ Phase 4-10 で移動予定
    ├── floor_construction.rs  ⬚ Phase 4-11 で移動予定
    ├── plant_trees/           ⬚ Phase 4-5 で移動予定
    ├── site_yard_visual.rs    ⬚ Phase 4-12 で移動予定
    └── task_area_material.rs  ⬚ Phase 4-13 で移動予定
```

### Cargo.toml（hw_visual）

```toml
[package]
name = "hw_visual"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = { workspace = true }
hw_core = { path = "../hw_core" }
hw_jobs = { path = "../hw_jobs" }
hw_logistics = { path = "../hw_logistics" }
hw_spatial = { path = "../hw_spatial" }
hw_world = { path = "../hw_world" }
hw_ui = { path = "../hw_ui" }
rand = { workspace = true }
```

---

## 実装手順（Phase 別）

### Phase 1: 事前型移動 ✅ 完了

#### Phase 1-1: `AnimationState` → hw_core::soul ✅

- `crates/hw_core/src/soul.rs` に AnimationState struct + Default impl を追加
- `src/entities/damned_soul/mod.rs` → `pub use hw_core::soul::AnimationState;`

#### Phase 1-2: `WorldMapRead` / `WorldMapWrite` → hw_world::map ✅

- `crates/hw_world/src/map.rs` → `crates/hw_world/src/map/mod.rs` + `map/access.rs`
- `src/world/map/access.rs` → `pub use hw_world::{WorldMapRead, WorldMapWrite};`

### Phase 2: hw_visual クレート作成 + ユーティリティ移動 ✅ 完了

- `crates/hw_visual/` 作成（Cargo.toml, src/lib.rs）
- utils 4ファイル + fade.rs を移動、root は re-export
- `handles.rs` に 7 Resource 定義を新規作成

### Phase 3: speech + FamiliarVoice / LatinPhrase の移動 ✅ 完了

- `src/systems/visual/speech/` 全ファイルを `crates/hw_visual/src/speech/` に移動
- FamiliarVoice を `hw_visual::speech::voice` に移動
- `GameAssets` → `SpeechHandles` に置き換え済み
- root 側 re-export 設定済み
- **備考**: familiar_ai 3ファイルは re-export 経由でアクセス中。Phase 4 で直接参照に更新する

### Phase 4: visual サブシステム移動

各サブシステムを順番に移動し、都度 `cargo check`:

| ステップ | 対象 | 主な GameAssets 置き換え |
|:---|:---|:---|
| 4-1 | `dream/` | `GameAssets` → （dream は handles 不使用の可能性あり、確認して対応） |
| 4-2 | `blueprint/` | `GameAssets` → `WorkIconHandles` + `MaterialIconHandles` |
| 4-3 | `gather/` | `GameAssets` → `WorkIconHandles` |
| 4-4 | `haul/` | `GameAssets` → `HaulItemHandles` + `MaterialIconHandles`<br>`AnimationState` → `hw_core::soul::AnimationState` |
| 4-5 | `plant_trees/` | `GameAssets` → `PlantTreeHandles` |
| 4-6 | `soul.rs` | `GameAssets` → `SpeechHandles`（font_soul_name のみ） |
| 4-7 | `mud_mixer.rs` | `GameAssets` → `BuildingAnimHandles` |
| 4-8 | `tank.rs` | `GameAssets` → `BuildingAnimHandles` |
| 4-9 | `wall_connection.rs` | `GameAssets` → `WallVisualHandles`<br>`WorldMapRead` → `hw_world::WorldMapRead` |
| 4-10 | `wall_construction.rs` | `GameAssets` → `MaterialIconHandles` |
| 4-11 | `floor_construction.rs` | `GameAssets` → `MaterialIconHandles` |
| 4-12 | `site_yard_visual.rs` | `GameAssets` なければそのまま移動 |
| 4-13 | `task_area_visual.rs` | `TaskAreaMaterial/Visual` を hw_visual に分離、システム関数は root 残留 |

#### 共通インポート変換パターン（Phase 4 全体）

```
# Before (src/systems/visual/)            # After (crates/hw_visual/src/)
crate::assets::GameAssets             → 各 VisualHandles Resource
crate::entities::damned_soul::AnimationState → hw_core::soul::AnimationState
crate::world::map::WorldMapRead        → hw_world::WorldMapRead
crate::systems::utils::animations::*  → crate::animations::*（hw_visual 内部）
super::phrases::LatinPhrase            → crate::speech::LatinPhrase
super::super::X                        → crate::Y（絶対パスに変換）
```

#### Phase 4 追加作業: familiar_ai import 更新

speech の re-export 削除に伴い、以下3ファイルの import を直接参照に更新:
- `src/systems/familiar_ai/execute/idle_visual_apply.rs`
- `src/systems/familiar_ai/execute/max_soul_apply.rs`
- `src/systems/familiar_ai/execute/squad_apply.rs`

```
# Before
crate::systems::visual::speech::phrases::LatinPhrase
# After
hw_visual::speech::phrases::LatinPhrase
```

### Phase 5: HwVisualPlugin 統合 + クリーンアップ

1. `src/plugins/startup/visual_handles.rs` を新規作成（init_visual_handles システム）
2. startup pipeline に init_visual_handles を追加（既存のアセット初期化後）
3. `src/plugins/visual.rs` を書き換え:
   - `app.add_plugins(HwVisualPlugin)` で hw_visual の全システムを一括登録
   - root 残留システム（placement_ghost, update_task_area_material）のみ個別登録
   - `task_link_system` を `run_if(|debug: Res<DebugVisible>| debug.0)` 付きで登録
   - 旧 import（`crate::systems::visual::*`）を `hw_visual::*` に置き換え
   - Material2dPlugin / UiMaterialPlugin の登録を削除（HwVisualPlugin 内で実行されるため）
4. `src/systems/visual/mod.rs` を整理（placement_ghost, task_area_visual のみ残留）
5. `src/systems/utils/mod.rs` 削除
6. 最終 `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

---

## 残作業量見積もり

| Phase | 変更ファイル数 | 主な内容 |
|:---|---:|:---|
| ~~Phase 1~~ | ~~7~~ | ✅ 完了 |
| ~~Phase 2~~ | ~~10~~ | ✅ 完了 |
| ~~Phase 3~~ | ~~15~~ | ✅ 完了 |
| Phase 4 (visual 移動) | ~55 | 47ファイル移動 + GameAssets → handles 置き換え |
| Phase 5 (Plugin 統合 + 整理) | ~10 | startup 追加 + plugin 更新 + mod.rs 整理 |

---

## リスク評価（残作業）

| Phase | リスク | 軽減策 |
|:---|:---|:---|
| Phase 4 | 中〜高 — 大量ファイル移動 + handles 置き換え | サブシステム単位で移動・検証。`super::super::` 相対パスの変換漏れに注意 |
| Phase 5 | 低 — 整理のみ | 最終 `cargo check` + 動作確認 |

---

## 依存グラフ（完成後）

```
hw_core  ← AnimationState のみ追加（GameAssets/LatinPhrase/FamiliarVoice は入れない）
│
├── hw_jobs
├── hw_world  ← WorldMapRead/WorldMapWrite 追加
│   ├── hw_core
│   └── hw_spatial
├── hw_logistics
├── hw_ui
│
└── hw_visual ★ NEW
    ├── hw_core      (AnimationState, DamnedSoul, Familiar, ...)
    ├── hw_jobs      (Blueprint, WorkType, AssignedTask, ...)
    ├── hw_logistics (ResourceType, Inventory, ...)
    ├── hw_spatial   (SpatialGrid, ...)
    ├── hw_world     (WorldMap, WorldMapRead, Site, ...)
    └── hw_ui        (MainCamera, HoveredEntity, UiTheme, ...)
    ※ GameAssets には依存しない（handles.rs の Resource で代替）

root crate:
  ├── GameAssets（struct + ロード処理）← 変更なし、root 専有
  ├── src/plugins/startup/visual_handles.rs  ← GameAssets → hw_visual Resource 注入
  ├── src/systems/familiar_ai/execute/*.rs   ← hw_visual::speech::LatinPhrase を利用
  ├── src/systems/visual/placement_ghost.rs  ← BuildContext 依存で残留
  ├── src/systems/visual/task_area_visual.rs ← TaskContext 依存で残留（システム部分）
  └── src/plugins/visual.rs                  ← hw_visual を組み込む
```
