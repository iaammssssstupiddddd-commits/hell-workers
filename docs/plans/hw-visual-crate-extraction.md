# hw_visual クレート化 実装計画

## 概要

`src/systems/visual/`（56ファイル、6,649行）全体を `hw_visual` クレートとして切り出す。
アプリ固有の型（GameAssets 等）は root に残す方針のため、visual が依存する未クレート化の型を先に他クレートへ移動する。

## 現状の依存分析

`src/systems/visual/` が参照する `crate::` パスの分類:

### A. 既にクレート化済み（変更不要、hw_visual の依存に追加するだけ）

| 依存先 | クレート | 使用ファイル数 |
|:---|:---|---:|
| DamnedSoul, IdleBehavior, IdleState, DreamQuality | hw_core::soul | 10 |
| Familiar | hw_core::familiar | 1 |
| 全 Relationships (CommandedBy, TaskWorkers 等) | hw_core::relationships | 7 |
| TaskArea, TaskMode | hw_core::area, hw_core::game_state | 1 |
| constants (TILE_SIZE, Z_* 等) | hw_core::constants | 多数 |
| PlayMode | hw_core::game_state | 2 |
| 全 Events | hw_core::events | 1 |
| Blueprint, Building, BuildingType, WorkType, Designation, Tree, Rock, RestArea, MudMixerStorage | hw_jobs | 11 |
| FloorTile/WallTile 建設型 | hw_jobs::construction | 2 |
| AssignedTask, BuildPhase, GatherPhase, RefinePhase | hw_jobs::assigned_task | 3 |
| ResourceType, Inventory, ResourceItem, Wheelbarrow, Stockpile | hw_logistics | 5 |
| SpatialGrid, SpatialGridOps | hw_spatial | 1 |
| Site, Yard, AreaBounds | hw_world | 2 |
| MainCamera, HoveredEntity, SelectedEntity | hw_ui::camera, hw_ui::selection | 2 |
| UiMountSlot, UiNodeRegistry, UiSlot, UiTheme | hw_ui | 3 |

### B. 未クレート化だが移動可能（事前作業が必要）

| 型 | 現在の場所 | 使用箇所 (visual内) | 移動先 |
|:---|:---|:---|:---|
| `AnimationState` | `src/entities/damned_soul/mod.rs` | haul/wheelbarrow_follow.rs | **hw_core::soul** |
| `FamiliarVoice` | `src/entities/familiar/voice.rs` | speech/spawn.rs, speech/emitter.rs, speech/observers.rs | **hw_core::familiar** ※後述 |
| `WorldMap` (re-export) | `src/world/map/mod.rs` → `hw_world::map::WorldMap` | wall_connection.rs, placement_ghost.rs | **直接 hw_world を使う** |
| `WorldMapRead` (SystemParam) | `src/world/map/access.rs` | wall_connection.rs, placement_ghost.rs | **hw_world に移動** |
| `RIVER_Y_MIN` 等のレイアウト定数 | `src/world/map/layout.rs` | placement_ghost.rs | **hw_world に移動** |

### C. アプリ固有（root に残す → hw_visual から参照しない設計が必要）

| 型 | 現在の場所 | 使用箇所 (visual内) | 対応方針 |
|:---|:---|:---|:---|
| `GameAssets` | `src/assets.rs` | **20ファイル** | root に残す。hw_visual の各システムは `Handle<Image>` を直接受け取る or Query で解決 |
| `TaskContext` | `src/app_contexts.rs` | task_area_visual.rs, placement_ghost.rs | root に残す。この2ファイルは root に残留 |
| `BuildContext`, `CompanionPlacementState` 等 | `src/app_contexts.rs` | placement_ghost.rs | 同上 |
| `DebugVisible` | `src/main.rs` | VisualPlugin 登録時のみ (plugins/visual.rs) | root 側で run_if を適用 |

## FamiliarVoice の依存問題

`FamiliarVoice` は `speech::phrases::LatinPhrase` に依存（`LatinPhrase::COUNT` をフィールドサイズに使用）。
→ `LatinPhrase` が visual 内部で定義されているため、hw_core に移動するなら LatinPhrase も一緒に移動する必要がある。

**方針**: `LatinPhrase`（phrases.rs 内の enum + index メソッドのみ）と `FamiliarVoice` を **hw_core::familiar** に移動。phrases.rs の残り（フレーズテキスト定義）は hw_visual に残す。

## root に残留するファイル

以下の2ファイルは `app_contexts` への依存が深く、root に残す:

| ファイル | 理由 |
|:---|:---|
| `placement_ghost.rs` | BuildContext, CompanionPlacementState, WorldMapRead, RIVER_Y_MIN |
| `task_area_visual.rs` | TaskContext（アプリ固有リソース） |

→ hw_visual から `TaskAreaMaterial` を公開し、root 側のシステムが使う形にする。

**修正**: `task_area_visual.rs` は TaskAreaMaterial 定義（Shader マテリアル）とシステムが一体。マテリアル定義を hw_visual に、システム関数を root に分離する。

## GameAssets 問題の解決方針

GameAssets は 20 ファイルで使用され、移動先候補として以下のアプローチを検討:

**方針: 段階的に hw_visual 側を GameAssets 非依存にする**

各 visual システムの Query 引数に必要な `Handle<Image>` を渡す形に変更すると工数が膨大。
代わに **GameAssets を hw_core に移動する**。

GameAssets の依存:
- `bevy::prelude::*` (`Handle<Image>`, `Handle<Font>`, `Resource`) → hw_core は既に bevy に依存
- 型定義のみ（初期化ロジックは `src/plugins/startup/` にある）

**結論**: `GameAssets` struct 定義を **hw_core** に移動し、初期化（アセットロード）は root に残す。

## WorldMapRead の移動

`WorldMapRead` は `SystemParam` で hw_world の `WorldMap` をラップしている。
hw_world に移動可能。依存は `WorldMap` と bevy のみ。

## 事前作業まとめ

### 事前作業 1: hw_core への型移動

| 型 | 移動元 | 移動先 |
|:---|:---|:---|
| `GameAssets` (struct 定義のみ) | `src/assets.rs` | `hw_core::assets` |
| `AnimationState` | `src/entities/damned_soul/mod.rs` | `hw_core::soul` |
| `LatinPhrase` (enum + index) | `src/systems/visual/speech/phrases.rs` | `hw_core::familiar` |
| `FamiliarVoice` | `src/entities/familiar/voice.rs` | `hw_core::familiar` |

root 側には re-export を置く:
```rust
// src/assets.rs
pub use hw_core::assets::GameAssets;
// src/entities/damned_soul/mod.rs
pub use hw_core::soul::AnimationState;
```

### 事前作業 2: hw_world への型移動

| 型 | 移動元 | 移動先 |
|:---|:---|:---|
| `WorldMapRead` (SystemParam) | `src/world/map/access.rs` | `hw_world::map` |
| レイアウト定数 (RIVER_Y_MIN 等) | `src/world/map/layout.rs` | `hw_world::layout` |

## hw_visual クレート構成

```
crates/hw_visual/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── animations.rs          ← src/systems/utils/animations.rs
    ├── floating_text.rs       ← src/systems/utils/floating_text.rs
    ├── progress_bar.rs        ← src/systems/utils/progress_bar.rs
    ├── worker_icon.rs         ← src/systems/utils/worker_icon.rs
    ├── fade.rs                ← src/systems/visual/fade.rs
    ├── blueprint/             ← src/systems/visual/blueprint/
    │   ├── mod.rs
    │   ├── components.rs
    │   ├── effects.rs
    │   ├── material_display.rs
    │   ├── progress_bar.rs
    │   └── worker_indicator.rs
    ├── dream/                 ← src/systems/visual/dream/
    │   ├── mod.rs
    │   ├── components.rs
    │   ├── dream_bubble_material.rs
    │   ├── gain_visual.rs
    │   ├── particle.rs
    │   └── ui_particle/
    ├── gather/                ← src/systems/visual/gather/
    │   ├── mod.rs
    │   ├── components.rs
    │   ├── resource_highlight.rs
    │   └── worker_indicator.rs
    ├── haul/                  ← src/systems/visual/haul/
    │   ├── mod.rs
    │   ├── carrying_item.rs
    │   ├── components.rs
    │   ├── effects.rs
    │   └── wheelbarrow_follow.rs
    ├── speech/                ← src/systems/visual/speech/
    │   ├── mod.rs
    │   ├── animation.rs
    │   ├── components.rs
    │   ├── conversation/
    │   ├── cooldown.rs
    │   ├── emitter.rs
    │   ├── observers.rs
    │   ├── periodic.rs
    │   ├── phrases.rs (LatinPhrase enum除去後)
    │   ├── spawn.rs
    │   ├── typewriter.rs
    │   └── update.rs
    ├── soul.rs                ← src/systems/visual/soul.rs
    ├── mud_mixer.rs           ← src/systems/visual/mud_mixer.rs
    ├── tank.rs                ← src/systems/visual/tank.rs
    ├── wall_connection.rs     ← src/systems/visual/wall_connection.rs
    ├── wall_construction.rs   ← src/systems/visual/wall_construction.rs
    ├── floor_construction.rs  ← src/systems/visual/floor_construction.rs
    ├── plant_trees/           ← src/systems/visual/plant_trees/
    ├── site_yard_visual.rs    ← src/systems/visual/site_yard_visual.rs
    └── task_area_material.rs  ← task_area_visual.rs からマテリアル定義のみ
```

### root に残留

| ファイル | 理由 |
|:---|:---|
| `src/systems/visual/placement_ghost.rs` | app_contexts (BuildContext等) に依存 |
| `src/systems/visual/task_area_visual.rs` のシステム部分 | app_contexts (TaskContext) に依存 |

### Cargo.toml

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

## 実装手順

### Phase 1: 事前作業 — 型移動

1. `GameAssets` struct を `hw_core::assets` に移動、`src/assets.rs` は re-export
2. `AnimationState` を `hw_core::soul` に移動、`src/entities/damned_soul/mod.rs` は re-export
3. `LatinPhrase` enum を `hw_core::familiar` に移動
4. `FamiliarVoice` を `hw_core::familiar` に移動、`src/entities/familiar/voice.rs` は re-export
5. `WorldMapRead` を `hw_world::map` に移動、`src/world/map/access.rs` は re-export
6. レイアウト定数を `hw_world::layout` に移動
7. `cargo check` で検証

### Phase 2: hw_visual クレート作成

1. `crates/hw_visual/` 作成、Cargo.toml・lib.rs
2. workspace members に追加
3. `src/systems/utils/` の 4 ファイル + `fade.rs` を移動
4. `cargo check` で検証

### Phase 3: visual サブシステム移動

1. `speech/` を hw_visual に移動（phrases.rs から LatinPhrase enum 除去済み）
2. `dream/` を移動
3. `blueprint/` を移動
4. `haul/`, `gather/`, `plant_trees/` を移動
5. `soul.rs`, `mud_mixer.rs`, `tank.rs`, `wall_connection.rs` 等の単体ファイルを移動
6. `floor_construction.rs`, `wall_construction.rs` を移動
7. `site_yard_visual.rs` を移動
8. `task_area_visual.rs` からマテリアル定義を分離して hw_visual に移動
9. 各ステップ後に `cargo check`

### Phase 4: root 側のクリーンアップ

1. `src/systems/visual/mod.rs` を整理（残留ファイルのみ）
2. `src/plugins/visual.rs` のインポートを hw_visual に変更
3. `src/systems/utils/` ディレクトリ削除
4. 最終 `cargo check`

## 作業量見積もり

| Phase | 変更ファイル数 | 内容 |
|:---|---:|:---|
| Phase 1 (型移動) | ~15 | hw_core 4型追加 + hw_world 2型移動 + re-export |
| Phase 2 (utils移動) | ~20 | クレート新規 + インポート書き換え14箇所 |
| Phase 3 (visual移動) | ~60 | 54ファイル移動 + `crate::` → クレートパス変換 |
| Phase 4 (クリーンアップ) | ~5 | mod.rs・plugin 整理 |

## リスク評価

- **Phase 1**: 低リスク — re-export で後方互換維持
- **Phase 2**: 低リスク — `crate::` 依存ゼロのファイル群
- **Phase 3**: 中リスク — 大量のインポート書き換え。`crate::` → hw_* パスの変換漏れに注意
- **Phase 4**: 低リスク — 整理のみ

## 依存グラフ（完成後）

```
hw_core (GameAssets, AnimationState, LatinPhrase, FamiliarVoice 追加)
├── hw_jobs
├── hw_world (WorldMapRead, レイアウト定数 追加)
│   └── hw_spatial
├── hw_logistics
├── hw_ui
└── hw_visual ★ NEW
    ├── hw_core
    ├── hw_jobs
    ├── hw_logistics
    ├── hw_spatial
    ├── hw_world
    └── hw_ui
```
