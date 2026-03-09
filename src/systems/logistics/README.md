# logistics — ロジスティクス shell + GameAssets 依存システム

## 役割

物流ロジックの大部分は `hw_logistics` クレートに移植済み。
このディレクトリは以下のみを担う:

- `initial_spawn.rs` — GameAssets 依存の初期リソーススポーン
- `ui.rs` — ロジスティクス UI ヘルパー
- `transport_request/plugin.rs` — `FloorWallTransportPlugin`（Optional M_extra 解消まで root 残留）
- `transport_request/producer/floor_construction.rs` / `wall_construction.rs` — `FloorConstructionSpatialGrid` 依存のため hw_logistics に未移植（Optional M_extra）
- hw_logistics 公開 API の re-export 層

## 主要ファイル

| ファイル | 内容 |
|---|---|
| `mod.rs` | hw_logistics モジュールの re-export + `initial_spawn`, `ui` 公開 |
| `initial_spawn.rs` | 初期リソースエンティティのスポーン（GameAssets 依存） |
| `ui.rs` | ロジスティクス UI ヘルパー |

## transport_request/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | hw_logistics の型を re-export + `plugin`, `producer` を公開 |
| `plugin.rs` | `FloorWallTransportPlugin` — floor/wall construction producer 登録（Optional M_extra 解消まで root 残留）。`TransportRequestPlugin` / `TransportRequestSet` は hw_logistics から re-export |

## transport_request/producer/ ディレクトリ

Optional M_extra（`FloorConstructionSpatialGrid` → hw_spatial）が完了するまで残留するファイル。

| ファイル | 内容 |
|---|---|
| `mod.rs` | floor/wall construction を宣言、hw_logistics から共通ヘルパーを `pub(crate) use` |
| `floor_construction.rs` | `floor_construction_auto_haul_system` 等（`FloorConstructionSpatialGrid` 依存） |
| `floor_construction/designation.rs` | 床建設サイトのデジグネーションヘルパー |
| `wall_construction.rs` | `wall_construction_auto_haul_system` 等（`FloorConstructionSpatialGrid` 依存） |

---

## hw_logistics との境界

このディレクトリが保持するもの（残留理由）:

| ファイル | 残留理由 |
|---|---|
| `initial_spawn.rs` | `GameAssets` リソース（テクスチャ等）に依存 |
| `ui.rs` | UI レンダリングに依存 |
| `plugin.rs`（FloorWallTransportPlugin） | Optional M_extra 完了まで一時残留 |
| `producer/floor_construction.rs` | `FloorConstructionSpatialGrid`（root の spatial/ に残留）を参照 |
| `producer/wall_construction.rs` | 同上 |

hw_logistics に移植済み（re-export 経由で公開）:

- 全 transport request producer（`blueprint`, `bucket`, `consolidation`, `mixer`, `provisional_wall`, `stockpile_group`, `tank_water_request`, `task_area`, `upsert`, `wheelbarrow`）
- 手押し車仲裁システム（`arbitration/`）
- `TransportRequestPlugin`, `TransportRequestSet`
- 建設系需要計算ヘルパー（`floor_construction.rs`, `wall_construction.rs`, `tile_index.rs`）
- アイテムライフサイクル管理（`item_lifetime.rs`）
