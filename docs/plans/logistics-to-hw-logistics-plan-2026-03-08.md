# logistics 実行ロジックを hw_logistics へ移植する計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `logistics-to-hw-logistics-plan-2026-03-08` |
| ステータス | `In Progress (~70%)` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-09 (M1〜M6完了)` |
| 作成者 | `AI (Claude)` |

---

## 1. 目的

- **解決したい課題**: root の `src/systems/logistics/`（6,190行）が hw_logistics（359行）に対してアンバランス。logistics の実行ロジックが root に混在し変更コストが高い。
- **到達したい状態**: GameAssets を必要としない logistics ロジックが hw_logistics に集約される。root の logistics は薄い plugin 登録 shell + `initial_spawn.rs`（GameAssets 依存）のみ。
- **成功指標**: `cargo check` エラーゼロ。root の `src/systems/logistics/` が 300行以下。

---

## 2. スコープ

### 対象（In Scope）

- `transport_request/` 全体（lifecycle, arbitration/, producer/, wheelbarrow_completion, plugin 等）
- `floor_construction.rs`, `wall_construction.rs`（建設 tile 状態同期）
- `tile_index.rs`（TileSiteIndex）
- `item_lifetime.rs`（アイテム寿命管理システム関数）
- `provisional_wall.rs`
- `SharedResourceCache`（resource_sync から hw_logistics へ）

### 非対象（Out of Scope）

- `initial_spawn.rs`（GameAssets 依存のため root に残す）
- `ui.rs`（UI 表示のため root に残す）
- `zone.rs`, `water.rs`, `types.rs`, `ground_resources.rs`（**Phase 0 で完了済み**）

---

## 3. 現状

### Phase 0 で完了済み（8ファイルがシェル化済み）

root 側が `pub use hw_logistics::XXX::*;` の 1 行シェルになっているファイル:

| root ファイル | hw_logistics |
|:--|:--|
| `zone.rs` | `hw_logistics::zone` |
| `water.rs` | `hw_logistics::water` |
| `types.rs` | `hw_logistics::types` |
| `ground_resources.rs` | `hw_logistics::ground_resources` |
| `transport_request/components.rs` | `hw_logistics::transport_request::components` |
| `transport_request/kinds.rs` | `hw_logistics::transport_request::kinds` |
| `transport_request/state_machine.rs` | `hw_logistics::transport_request::state_machine` |
| `transport_request/metrics.rs` | `hw_logistics::transport_request::metrics` |

また `item_lifetime.rs` は `ItemDespawnTimer` 型のみ hw_logistics に移動済み。システム関数はまだ root に残っている。

### root-only 型ブロッカー（未解決）

| 型 | 現在地 | 移動先 | 影響ファイル数 |
|:--|:--|:--|:--|
| `FloorTileBlueprint` / `WallTileBlueprint` | `src/systems/jobs/*/components.rs` | `hw_jobs::construction` | 5 |
| `Yard`, `Site`, `PairedYard`, `PairedSite` | `src/systems/world/zones.rs` | `hw_world::zones` | 9 |
| `SharedResourceCache` | `src/systems/familiar_ai/perceive/resource_sync.rs` | `hw_logistics::resource_cache` | 2 |
| `MovePlanned` | `src/systems/soul_ai/execute/task_execution/move_plant.rs` | `hw_jobs::model` | 4 |
| `TaskArea` | `src/systems/command/mod.rs` | `hw_core::area` | 5 |
| `FloorConstructionSpatialGrid` | `src/systems/spatial/floor_construction.rs` | `hw_spatial`（Optional） | 2 |

### 依存確認（ブロッカー型の移動難易度）

| 型 | 自身の依存 | 移動難易度 | 備考 |
|:--|:--|:--|:--|
| `FloorTileBlueprint` | `FloorTileState`（hw_jobs::construction 済み）+ Entity/i32 | ✅ 低 | FloorConstructionSite とは**別型**。TaskArea 不要 |
| `WallTileBlueprint` | `WallTileState`（hw_jobs::construction 済み）+ Entity/i32 | ✅ 低 | FloorTileBlueprint と同様 |
| `Yard`, `Site` | `Vec2`, `WorldMap::world_to_grid` | ✅ 低 | **`WorldMap` は既に `hw_world::map` に存在**（root は re-export） |
| `PairedYard`, `PairedSite` | `Entity` のみ | ✅ 低 | |
| `SharedResourceCache` | `ResourceType`（hw_core）+ HashMap | ✅ 低 | |
| `MovePlanned` | `Entity` のみ | ✅ 低 | |
| `TaskArea` | `AreaBounds`（hw_core::area 既存） | ✅ 低 | |
| `FloorConstructionSpatialGrid` | `GridData`（hw_spatial::grid 既存） | ✅ 低 | Optional 対象 |

### hw_logistics Cargo.toml の現状

現在の依存: `bevy`, `hw_core` のみ。
移植後に必要: `hw_world`, `hw_jobs`, `hw_spatial` の追加が必要。

### GameSystemSet / FamiliarAiSystemSet / SoulAiSystemSet の所在

`plugin.rs` が参照するすべての SystemSet は **hw_core::system_sets** に存在する。
よって `plugin.rs` を含む transport_request 全体が hw_logistics に移動可能。

---

## 4. 実装方針

- **M1〜M6 は相互に独立**（依存関係なし）→ 任意の順序・並行実施が可能。
- **M7 は M1〜M6 がすべて完了してから着手**。
- 各ファイル移動後に `cargo check`。root 側は `pub use hw_logistics::XXX;` シェルで互換性を維持し、M8 でまとめて削除。
- **FloorConstructionSite / WallConstructionSite** は `TaskArea` に依存するため `FloorTileBlueprint` とは独立して hw_jobs に移動する（M5 の後）。これらは logistics ではなく jobs の関心であり、M8 クリーンアップの後に別タスクとして実施することもできる。

---

## 5. マイルストーン

### M1: FloorTileBlueprint, WallTileBlueprint → hw_jobs::construction ✅ **完了**

**変更内容**: `FloorTileBlueprint` 自体は `FloorTileState`（既に hw_jobs）と primitive 型にしか依存しない。`FloorConstructionSite`（`TaskArea` 依存）とは**別型**なので独立して移動できる。

**移動先**: `crates/hw_jobs/src/construction.rs`（既存ファイルに追記）

**シェル**:
- `src/systems/jobs/floor_construction/components.rs` → `pub use hw_jobs::construction::{FloorTileBlueprint, ...};`（`FloorConstructionSite` は root 残留）
- `src/systems/jobs/wall_construction/components.rs` → 同上（`WallConstructionSite` は root 残留）

**完了条件**:
- [x] `cargo check` が通る

---

### M2: Yard, Site, PairedYard, PairedSite → hw_world::zones ✅ **完了**

**変更内容**: `Yard::width_tiles()` / `height_tiles()` は `hw_world::coords::world_to_grid` を使用して移動。

**移動先**: `crates/hw_world/src/zones.rs`（新規作成）

**変更ファイル**:
- `crates/hw_world/src/zones.rs` (新規)
- `crates/hw_world/src/lib.rs` (`pub mod zones; pub use zones::{...}` 追加済み)
- `src/systems/world/zones.rs` → `pub use hw_world::zones::*;` の1行シェル

**完了条件**:
- [x] `cargo check` が通る

---

### M3: SharedResourceCache → hw_logistics::resource_cache ✅ **完了**

**移動元**: `src/systems/familiar_ai/perceive/resource_sync.rs` の `SharedResourceCache` struct と `impl`（`sync_reservations_system` 等のシステム関数は root に残留）

**移動先**: `crates/hw_logistics/src/resource_cache.rs`（新規作成）

**変更ファイル**:
- `crates/hw_logistics/src/resource_cache.rs` (新規)
- `crates/hw_logistics/src/lib.rs` (`pub mod resource_cache; pub use resource_cache::SharedResourceCache;` 追加済み)
- `src/systems/familiar_ai/perceive/resource_sync.rs` → `pub use hw_logistics::SharedResourceCache;` で再エクスポート（`apply_reservation_op` は root 残留）

**完了条件**:
- [x] `cargo check` が通る

---

### M4: MovePlanned → hw_jobs::model ✅ **完了**

**ブロッカー**: なし

**変更内容**: `MovePlanned { pub task_entity: Entity }` を hw_jobs::model に追加。

**変更ファイル**:
- `crates/hw_jobs/src/model.rs` (`MovePlanned` 定義を追加)
- `src/systems/soul_ai/execute/task_execution/move_plant.rs` → `pub use hw_jobs::MovePlanned;` に差し替え

**完了条件**:
- [x] `cargo check` が通る

---

### M5: TaskArea → hw_core::area ✅ **完了**

**ブロッカー**: なし（`AreaBounds` は hw_core::area に既存）

**変更内容**: `TaskArea` は `AreaBounds` のラッパー。hw_core に置くのが最も自然。

**移動元**: `src/systems/command/mod.rs` の `TaskArea` struct + impl のみ

**移動先**: `crates/hw_core/src/area.rs`（`AreaBounds` と同居）

**変更ファイル**:
- `crates/hw_core/src/area.rs` (`TaskArea` 定義を追加)
- `src/systems/command/mod.rs` → `pub use hw_core::area::TaskArea;` に差し替え

**M5 後に実施: FloorConstructionSite, WallConstructionSite → hw_jobs**

M5 完了後、`FloorConstructionSite`（`area_bounds: TaskArea` フィールド）と `WallConstructionSite` を hw_jobs::construction に移動できる。これは logistics 移植の直接的な前提条件ではないが、producer/blueprint.rs・producer/floor_construction.rs 等を hw_logistics に移すために必要。

**完了条件**:
- [x] `cargo check` が通る

---

### M6: hw_logistics の Cargo.toml に依存追加 ✅ **完了**

**ブロッカー**: なし

```toml
[dependencies]
bevy       = { workspace = true }
hw_core    = { path = "../hw_core" }
hw_world   = { path = "../hw_world" }   # 追加（Yard/WorldMap 参照のため）
hw_jobs    = { path = "../hw_jobs" }    # 追加（Blueprint/MovePlanned 等のため）
hw_spatial = { path = "../hw_spatial" } # 追加（StockpileSpatialGrid 等のため）
rand       = { workspace = true }       # 必要に応じて追加
```

**完了条件**:
- [x] `cargo check` が通る

**前提**: M1〜M6 完了

**移植対象と解放タイミング**:

| ファイル | 解放タイミング（必要な M）| 行数 |
|:--|:--|--:|
| `provisional_wall.rs` | M6 | 23 |
| `item_lifetime.rs`（システム関数） | M6 | 33 |
| `wheelbarrow_completion.rs` | M6（hw_world dep） | 56 |
| `transport_request/arbitration/types.rs` | M6 | 85 |
| `transport_request/arbitration/grants.rs` | M6 | 175 |
| `transport_request/arbitration/metrics_update.rs` | M6 | 34 |
| `transport_request/arbitration/lease_state.rs` | M6 | 98 |
| `transport_request/producer/upsert.rs` | M6 | 196 |
| `transport_request/producer/mixer_helpers/upsert.rs` | M6 | 152 |
| `tile_index.rs` | M1, M6 | 74 |
| `floor_construction.rs`（demand helper） | M1, M6 | 65 |
| `wall_construction.rs`（demand helper） | M1, M6 | 65 |
| `transport_request/lifecycle.rs` | M2, M6 | 99 |
| `transport_request/producer/task_area.rs` | M2, M6 | 362 |
| `transport_request/producer/consolidation.rs` | M2, M6 | 236 |
| `transport_request/producer/stockpile_group.rs` | M2, M6 | 278 |
| `transport_request/producer/mixer_helpers/types.rs` | M2, M6 | 14 |
| `transport_request/producer/mixer_helpers/collect.rs` | M2, M6 | 126 |
| `transport_request/producer/mixer_helpers/issue.rs` | M2, M6 | 192 |
| `transport_request/arbitration/collection.rs` | M3, M6 | 246 |
| `transport_request/arbitration/candidates.rs` | M3, M6 | 315 |
| `transport_request/arbitration/mod.rs` | M3, M6 | 305 |
| `transport_request/producer/mixer.rs` | M4, M6 | 115 |
| `transport_request/producer/tank_water_request.rs` | M4, M6 | 160 |
| `transport_request/producer/mixer_helpers/desired.rs` | M2, M3, M4, M6 | 125 |
| `transport_request/producer/mixer_helpers.rs` | M4, M6 | 19 |
| `transport_request/producer/bucket.rs` | M4, M5, M6 | 278 |
| `transport_request/producer/wheelbarrow.rs` | M5, M6 | 246 |
| `transport_request/producer/blueprint.rs` | M1, M2, M5, M6 | 235 |
| `transport_request/producer/provisional_wall.rs` | M2, M5, M6 | 294 |
| `transport_request/producer/mod.rs` | 全依存ファイル移植後 | 323 |
| `transport_request/mod.rs` | 全依存ファイル移植後 | 19 |
| `transport_request/plugin.rs` | 全システム関数移植後 | 114 |

**⚠️ 未解決（Optional M_extra が必要なファイル）**:

| ファイル | 追加ブロッカー | 行数 |
|:--|:--|--:|
| `transport_request/producer/floor_construction.rs` | `FloorConstructionSpatialGrid`（root の `src/systems/spatial/`）→ hw_spatial に移動が必要 | 237 |
| `transport_request/producer/wall_construction.rs` | 同上（WallConstructionSpatialGrid が未存在なら FloorConstruction と同様に対処） | 279 |

これら 2 ファイル（計 516 行）は Optional M_extra（FloorConstructionSpatialGrid → hw_spatial）が完了するまで root に残留することを許容する。

**移植手順**:
1. ファイルを上記表の「解放タイミング」順に移植
2. 各ファイル移植後に root 側を `pub use hw_logistics::XXX;` シェルに置換
3. `cargo check` 確認（ファイル 2〜3 件ごと）

**完了条件**:
- [ ] 表内の全ファイル（Optional 除く）が hw_logistics に移植済み
- [ ] 各ステップで `cargo check` が通る

---

### M8: root の logistics を shell 化

**変更内容**: M7 で移植済みのファイルから `pub use` シェルを削除し、root を整理する。

**残すもの（root）**:
- `initial_spawn.rs`（GameAssets 依存のためロジックごと残留）
- `mod.rs`（plugin 登録）
- `ui.rs`（UI 表示）
- `transport_request/plugin.rs` の Bevy Plugin impl（hw_logistics の関数を呼び出す形に）

**削除するもの（root）**:
- M7 で移植済みの全 `pub use` シェルファイル（内容が `pub use hw_logistics::...` 1行のもの）

**完了条件**:
- [ ] `cargo check` が通る
- [ ] `src/systems/logistics/` のファイル数が 10 以下（initial_spawn, mod, ui, transport_request/{plugin,mod} + 任意残留）

---

### Optional M_extra: FloorConstructionSpatialGrid → hw_spatial

`producer/floor_construction.rs`（237行）と `producer/wall_construction.rs`（279行）を hw_logistics に移すための前提。M7 の主要移植とは独立して実施できる。

**変更内容**:
- `src/systems/spatial/floor_construction.rs` の `FloorConstructionSpatialGrid` を `crates/hw_spatial/src/floor_construction.rs` に移動
- `crates/hw_spatial/src/lib.rs` に追加
- root の `src/systems/spatial/floor_construction.rs` → `pub use hw_spatial::FloorConstructionSpatialGrid;` シェルに

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| M7 で大量ファイルを一度に移動すると追跡困難 | 高 | ファイル 2〜3 件ずつ移動し都度 `cargo check` |
| `SharedResourceCache` 移動で familiar_ai の import が変わる | 低 | M3 の shell 化で互換性を維持 |
| `Yard::width_tiles()` の `WorldMap` 参照 | 低 | `hw_world::map::WorldMap::world_to_grid` or `hw_world::coords::world_to_grid` に書き換え |
| `FloorConstructionSite`/`WallConstructionSite` は M5 後まで root に残る | 低 | producer/floor_construction.rs 等の移植は M5 後に実施 |
| `plugin.rs` の `TransportRequestSet` が移動すると root から参照できなくなる | 低 | `pub use hw_logistics::transport_request::plugin::TransportRequestSet;` で re-export |

---

## 7. 検証計画

- 必須: 各マイルストーン完了後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- M8 完了後: `find src/systems/logistics -name "*.rs" | xargs wc -l` で 300行以下を確認
- ゲーム起動確認（物流・搬送が正常に機能すること）

---

## 8. ロールバック方針

- 各マイルストーンを個別コミット
- root 側の `pub use` を維持している間は API 互換性があるためロールバックはコミット revert のみ
- M7〜M8 はファイル単位で段階的にコミット

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `~70%`
- 完了済み: Phase 0（8ファイルシェル化）、AssignedTask 移植（別計画）、**M1〜M6**
- 未着手: M7・M8

### 次のAIが最初にやること

1. この計画書を読む
2. **M7 に着手**（M1〜M6 完了済み）
3. M7 移植表の「解放タイミング」列を参照し、ファイルを 2〜3 件ずつ移植する

### ブロッカー/注意点

- **M1〜M6 すべて完了済み**。M7 に着手可能。
- **M7 は表の「解放タイミング」列を必ず参照**。
- `initial_spawn.rs` は**絶対に hw_logistics に移さない**（GameAssets 依存）。
- `ui.rs` も**移さない**（UI 依存）。
- `FloorTileBlueprint` と `FloorConstructionSite` は別型。`FloorTileBlueprint` は TaskArea 不要（M1）、`FloorConstructionSite` は TaskArea 必要（M5 後）。
- `Yard::width_tiles()` の `WorldMap` は root の re-export ではなく `hw_world::map::WorldMap` または `hw_world::coords::world_to_grid` を直接使用すること。
- hw_logistics の `item_lifetime.rs` には `ItemDespawnTimer` 型のみ存在。システム関数（`despawn_expired_items_system`）は root に残っており、M7 で移植する。

### 参照必須ファイル

- `crates/hw_logistics/src/lib.rs`（移動先の起点）
- `src/systems/logistics/mod.rs`（全システム登録の起点）
- `src/systems/logistics/transport_request/plugin.rs`（plugin 登録の詳細）
- `crates/hw_core/src/area.rs`（TaskArea 定義）
- `crates/hw_world/src/zones.rs`（Yard/Site 定義）
- `src/systems/jobs/floor_construction/components.rs`（FloorTileBlueprint）
- `src/systems/familiar_ai/perceive/resource_sync.rs`（SharedResourceCache）

### 最終確認ログ

- 最終 `cargo check`: ✅ 2026-03-09（M1〜M6 完了状態）
- 未解決エラー: なし

### Definition of Done

- [ ] M1〜M8 完了
- [ ] `src/systems/logistics/` のファイル数が 10 以下
- [ ] hw_logistics の行数が 3,000行以上
- [ ] `cargo check` が成功

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI (Claude)` | 初版作成 |
| `2026-03-09` | `AI (Claude)` | 現状に合わせて全面改訂。Phase 0 完了を反映。WorldMap/FloorTileBlueprint の依存関係を修正。M1〜M6 が独立並行可能であることを明記。FloorConstructionSpatialGrid ブロッカーを新規追加。 |
| `2026-03-09` | `AI (Claude)` | M1〜M6 完了を反映。進捗 ~70% に更新。参照ファイルを crate 側に修正。 |
