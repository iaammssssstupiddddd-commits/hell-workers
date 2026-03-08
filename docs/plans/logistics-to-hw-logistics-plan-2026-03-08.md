# logistics 実行ロジックを hw_logistics へ移植する計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `logistics-to-hw-logistics-plan-2026-03-08` |
| ステータス | `Draft` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI (Claude)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

---

## 1. 目的

- **解決したい課題**: root の `src/systems/logistics/`（6,153行 / 47ファイル）が hw_logistics（358行 / 11ファイル）に対してアンバランスに大きい。logistics の実行ロジック（transport_request 生成・仲裁・予約管理）が root の一部として混在しており、変更コストが高い。
- **到達したい状態**: GameAssets を必要としない logistics ロジックが hw_logistics に集約される。root の logistics は薄い plugin 登録 shell + initial_spawn（GameAssets 依存）のみになる。
- **成功指標**: `cargo check` エラーゼロ。root の `src/systems/logistics/` が 300行以下になる。

---

## 2. スコープ

### 対象（In Scope）

移植対象のモジュール群（GameAssets 非依存）:
- `transport_request/` 全体（lifecycle, state_machine, arbitration/, producer/, wheelbarrow_completion 等）
- `floor_construction.rs`, `wall_construction.rs`（建設 tile 状態同期）
- `ground_resources.rs`（地面アイテム管理）
- `item_lifetime.rs`（アイテム寿命管理）
- `water.rs`（水管理）
- `zone.rs`（ゾーン型）
- `tile_index.rs`（TileSiteIndex）
- `provisional_wall.rs`
- `types.rs`（ZoneType 等）
- `SharedResourceCache`（resource_sync から hw_logistics へ移動）

### 非対象（Out of Scope）

- `initial_spawn.rs`（GameAssets 依存のため root に残す）
- `ui.rs`（UI 表示のため hw_ui または root に残す）
- visual 系・GameAssets 依存コード

---

## 3. 現状とギャップ

### root-only 型ブロッカー

調査の結果、`src/systems/logistics/` の大多数のファイルが以下の **root-only 型** を参照していることが判明した。これらを先に crate 化しないと logistics が hw_logistics に移せない。

| root-only 型 | 現在地 | 参照ファイル数 | 移動先（提案） |
|:--|:--|:--|:--|
| `TaskArea` | `src/systems/command/mod.rs` | 11 | `hw_core::area` |
| `Yard`, `Site`, `PairedYard`, `PairedSite` | `src/systems/world/zones.rs` | 17 | `hw_world` |
| `FloorTileBlueprint` | `src/systems/jobs/floor_construction/components.rs` | 7 | `hw_jobs` |
| `WallTileBlueprint` | `src/systems/jobs/wall_construction/components.rs` | 4 | `hw_jobs` |
| `SharedResourceCache` | `src/systems/familiar_ai/perceive/resource_sync.rs` | 6 | `hw_logistics` |
| `MovePlanned` | `src/systems/soul_ai/execute/task_execution/move_plant.rs` | 4 | `hw_jobs` |

これらの prerequisite 移動 **すべてが完了して初めて** logistics 本体の移植が可能になる。

### 各 prerequisite の依存確認

| 型 | 自身の依存 | 移動難易度 |
|:--|:--|:--|
| `TaskArea` | `hw_core::area::AreaBounds` のみ | ✅ 低 |
| `Yard`, `Site` | `Vec2` のみ | ✅ 低 |
| `PairedYard`, `PairedSite` | `Entity` のみ | ✅ 低 |
| `FloorTileBlueprint` | `TaskArea` (→hw_core後), `FloorTileState` (→hw_jobs済み) | ✅ 低（TaskArea 移動後） |
| `WallTileBlueprint` | `TaskArea` (→hw_core後), `WallTileState` (→hw_jobs済み) | ✅ 低（TaskArea 移動後） |
| `SharedResourceCache` | `ResourceType` (hw_core), `HashMap<(Entity, ResourceType), usize>` | ✅ 低 |
| `MovePlanned` | `Entity` のみ | ✅ 低 |

### hw_logistics の追加依存

移植後、hw_logistics は以下の dep を新たに必要とする：

```toml
[dependencies]
hw_world   = { path = "../hw_world" }   # 追加（Yard/Site 参照のため）
hw_jobs    = { path = "../hw_jobs" }    # 追加（FloorTileBlueprint 等のため）
hw_spatial = { path = "../hw_spatial" } # 追加（StockpileSpatialGrid 等のため）
rand       = { workspace = true }       # 確認要
```

---

## 4. 実装方針

- **前提**: 本計画は prerequisite 型の移動（M1〜M6）と logistics 本体移植（M7〜M8）の 2 フェーズに分かれる。
- **原則**: 各マイルストーンは独立してコミット可能。移動後は root 側で `pub use hw_XXX::...` を挟み互換性を維持してから、最終的にそれらを削除する。
- **Bevy 0.18 注意点**: `FloorTileBlueprint` 等 Component が移動しても `#[reflect]` 登録は plugin 登録箇所（root）で行う。

---

## 5. マイルストーン

### M1: TaskArea を hw_core へ移動

**変更内容**: `TaskArea` は `AreaBounds` のラッパーに過ぎず、hw_core に置くのが最も自然。

**移動元**: `src/systems/command/mod.rs`（`TaskArea` struct + impl のみ）

**移動先**: `crates/hw_core/src/area.rs`（`AreaBounds` と同居）

**変更ファイル**:
- `crates/hw_core/src/area.rs` (`TaskArea` 定義を追加)
- `src/systems/command/mod.rs` (`TaskArea` 定義を削除し `pub use hw_core::area::TaskArea;` に差し替え)

**完了条件**:
- [ ] `cargo check` が通る
- [ ] `src/systems/command/mod.rs` に `TaskArea` の定義がない

---

### M2: Yard, Site, PairedYard, PairedSite を hw_world へ移動

**変更内容**: zone 型は world 概念であり hw_world が適切な場所。

**移動元**: `src/systems/world/zones.rs`（4 struct の定義部分）

**移動先**: `crates/hw_world/src/zones.rs`（新規作成または既存ファイルに追加）

**変更ファイル**:
- `crates/hw_world/src/zones.rs` (追加または新規作成)
- `crates/hw_world/src/lib.rs` (`pub mod zones; pub use zones::*;` 追加)
- `src/systems/world/zones.rs` (定義を削除し `pub use hw_world::zones::*;` に差し替え)

**注意点**: hw_world の依存 crate（hw_spatial 等）が `Yard`/`Site` を必要とするか確認する。

**完了条件**:
- [ ] `cargo check` が通る

---

### M3: FloorTileBlueprint, WallTileBlueprint を hw_jobs へ移動

**前提**: M1 完了（TaskArea が hw_core にある）

**変更内容**: `FloorTileBlueprint` は `FloorTileState`（hw_jobs 済み）と `TaskArea`（M1 後 hw_core）にのみ依存しており、hw_jobs に置くのが自然。`WallTileBlueprint` も同様。

**移動元**:
- `src/systems/jobs/floor_construction/components.rs` の `FloorConstructionSite`, `FloorTileBlueprint`
- `src/systems/jobs/wall_construction/components.rs` の `WallConstructionSite`, `WallTileBlueprint`

**移動先**:
- `crates/hw_jobs/src/construction.rs`（既存ファイルへ追加）

**変更ファイル**:
- `crates/hw_jobs/src/construction.rs` (FloorTileBlueprint, WallTileBlueprint 定義を追加)
- `src/systems/jobs/floor_construction/components.rs` (定義を削除し `pub use hw_jobs::construction::*` に差し替え)
- `src/systems/jobs/wall_construction/components.rs` (同上)

**完了条件**:
- [ ] `cargo check` が通る

---

### M4: SharedResourceCache を hw_logistics へ移動

**変更内容**: `SharedResourceCache` は resource reservation（物流予約）の状態管理であり、hw_logistics が保持するのが概念的に正しい。

**移動元**: `src/systems/familiar_ai/perceive/resource_sync.rs`

**移動先**: `crates/hw_logistics/src/resource_cache.rs`（新規作成）

**変更ファイル**:
- `crates/hw_logistics/src/resource_cache.rs` (SharedResourceCache 定義)
- `crates/hw_logistics/src/lib.rs` (`pub mod resource_cache; pub use resource_cache::*;` 追加)
- `src/systems/familiar_ai/perceive/resource_sync.rs` (定義を削除し `pub use hw_logistics::SharedResourceCache;` に差し替え)

**注意点**: `familiar_ai` は hw_logistics に既に依存しているため、依存追加は不要。

**完了条件**:
- [ ] `cargo check` が通る

---

### M5: MovePlanned を hw_jobs へ移動

**変更内容**: `MovePlanned` は `{ pub task_entity: Entity }` のみのマーカーコンポーネント。soul_ai のタスク実行中に使われるジョブ状態であり hw_jobs が適切。

**移動元**: `src/systems/soul_ai/execute/task_execution/move_plant.rs` の `MovePlanned` 定義

**移動先**: `crates/hw_jobs/src/model.rs` または `crates/hw_jobs/src/move_task.rs`

**変更ファイル**:
- `crates/hw_jobs/src/model.rs` (`MovePlanned` 定義を追加)
- `src/systems/soul_ai/execute/task_execution/move_plant.rs` (定義を削除し `pub use hw_jobs::MovePlanned;` に差し替え)

**完了条件**:
- [ ] `cargo check` が通る

---

### M6: hw_logistics の Cargo.toml に依存追加

**変更内容**: M7 の logistics 本体移植に先立ち、hw_logistics が必要とする dep を追加する。

**変更ファイル**:
- `crates/hw_logistics/Cargo.toml`

```toml
[dependencies]
bevy      = { workspace = true }
rand      = { workspace = true }
hw_core   = { path = "../hw_core" }
hw_world  = { path = "../hw_world" }  # 追加
hw_jobs   = { path = "../hw_jobs" }   # 追加
hw_spatial = { path = "../hw_spatial" } # 追加（要確認）
```

**完了条件**:
- [ ] `cargo check` が通る（hw_logistics が空のまま依存追加されても問題ない）

---

### M7: logistics 実行ロジックの移植（本体）

**前提**: M1〜M6 完了

**変更内容**: `src/systems/logistics/` の主要ロジックを hw_logistics へ移植。ファイル単位で移動し、都度 `cargo check`。

**移植順序**（依存の少ないものから）:

1. `types.rs`（ZoneType 等）→ `hw_logistics/src/types.rs`
2. `zone.rs` → `hw_logistics/src/zone.rs`
3. `ground_resources.rs` → `hw_logistics/src/ground_resources.rs`（hw_logistics に既存ファイルと統合）
4. `item_lifetime.rs` → `hw_logistics/src/item_lifetime.rs`
5. `water.rs` → `hw_logistics/src/water.rs`
6. `tile_index.rs`（TileSiteIndex）→ `hw_logistics/src/tile_index.rs`
7. `provisional_wall.rs` → `hw_logistics/src/provisional_wall.rs`
8. `transport_request/` 全体 → `hw_logistics/src/transport_request/`
9. `floor_construction.rs` → `hw_logistics/src/floor_construction.rs`
10. `wall_construction.rs` → `hw_logistics/src/wall_construction.rs`

各ファイルは移動後、root 側に `pub use hw_logistics::XXX;` の薄いシェルを残し互換性を維持する。

**完了条件**:
- [ ] 各ファイル移植後に `cargo check` が通る

---

### M8: root の logistics を shell 化

**変更内容**: root の `src/systems/logistics/` を薄い plugin 登録と `initial_spawn.rs` のみに整理する。

**残すもの（root）**:
- `initial_spawn.rs`（GameAssets 依存のためロジックごと残留）
- `mod.rs`（plugin 登録 + hw_logistics の re-export）

**削除するもの（root）**:
- M7 で移植済みの全ファイル（`pub use` を除いて空になったもの）

**完了条件**:
- [ ] `cargo check` が通る
- [ ] `src/systems/logistics/` のファイル数が 5 以下

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
|:--|:--|:--|
| `SharedResourceCache` を hw_logistics に移すと familiar_ai → hw_logistics の依存方向が変わる | 低（既に familiar_ai は hw_logistics に依存） | Cargo.toml の dep を確認してから移動 |
| M7 で大量ファイルを一度に移動すると追跡が困難 | 高 | ファイル 1〜3 件ずつ移動し都度 `cargo check` |
| `FloorTileBlueprint` が `TaskArea` を field に持つため、hw_jobs が hw_core に依存することになる | 低（hw_jobs は既に hw_core 依存） | 確認のみ |
| hw_spatial 依存の有無が未確認 | 中 | M6 で hw_spatial を追加し、M7 で使用箇所を確認してから判断 |
| root の `FamiliarAiSystemSet`/`SoulAiSystemSet` 参照がまだ残っている | 中 | hw_logistics 移植後も system set 登録は root plugin に残す（型参照は hw_core::system_sets 経由） |

---

## 7. 検証計画

- 必須: 各マイルストーン完了後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- M8 完了後: `find src/systems/logistics -name "*.rs" | wc -l` が 5 以下であることを確認
- ゲーム起動時の動作確認（物流・搬送が正常に機能すること）

---

## 8. ロールバック方針

- 各マイルストーンを個別コミット
- root 側の `pub use` を維持している間は API 互換性があるためロールバックはコミット revert のみ
- M7〜M8 はファイル単位で段階的にコミットしてロールバック粒度を細かく保つ

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手: M1〜M8

### 次のAIが最初にやること

1. この計画書を読む
2. M1（TaskArea を hw_core へ）から着手
3. `src/systems/command/mod.rs` と `crates/hw_core/src/area.rs` を確認して TaskArea の移動内容を把握

### ブロッカー/注意点

- **M1〜M6 は M7 の前提条件**。M7 の logistics 移植を M1〜M6 より前に始めてはいけない。
- `initial_spawn.rs` は **絶対に hw_logistics に移さない**（GameAssets 依存のため）。
- `ui.rs` も **移さない**（UI 依存）。
- `SharedResourceCache` を hw_logistics に移す際、`familiar_ai/perceive/resource_sync.rs` の他のコード（`sync_reservations_system` 等）は root に残る。SharedResourceCache の **型定義のみ** を移動する。
- hw_logistics の既存ファイル（`ground_resources.rs`, `water.rs` 等）が root の同名ファイルと **内容が重複していないか** M7 着手前に確認すること。

### 参照必須ファイル

- `src/systems/logistics/mod.rs`（全システム登録の起点）
- `src/systems/logistics/transport_request/plugin.rs`（plugin 登録の詳細）
- `crates/hw_logistics/src/lib.rs`（移動先の起点）
- `src/systems/command/mod.rs`（TaskArea 定義）
- `src/systems/world/zones.rs`（Yard/Site 定義）
- `src/systems/jobs/floor_construction/components.rs`（FloorTileBlueprint）
- `src/systems/familiar_ai/perceive/resource_sync.rs`（SharedResourceCache）

### 最終確認ログ

- 最終 `cargo check`: `N/A`（未着手）
- 未解決エラー: N/A

### Definition of Done

- [ ] M1〜M8 完了
- [ ] `src/systems/logistics/` のファイル数が 5 以下
- [ ] hw_logistics の行数が 5,000行以上
- [ ] `cargo check` が成功
- [ ] `docs/architecture.md` の hw_logistics セクションを更新

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI (Claude)` | 初版作成 |
