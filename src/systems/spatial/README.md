# spatial — 空間グリッド更新（ルートクレート拡張）

## 役割

`hw_spatial` クレートに定義されたグリッド群のうち、**ルートクレート固有のグリッド**（GatheringSpot・FloorConstruction）の更新システムを実装する。
`hw_spatial` で定義されたグリッドの更新は `plugins/spatial.rs` で直接登録される。

## ファイル一覧

| ファイル | 内容 |
|---|---|
| `mod.rs` | `update_gathering_spot_spatial_grid_system`, `update_floor_construction_spatial_grid_system` の公開 |
| `gathering.rs` | `GatheringSpatialGrid` の毎フレーム更新 |
| `floor_construction.rs` | `FloorConstructionSpatialGrid` の毎フレーム更新 |
| `blueprint.rs` | ブループリットグリッド補助 |
| `designation.rs` | 指定グリッド補助 |
| `familiar.rs` | Familiar グリッド補助 |
| `resource.rs` | リソースグリッド補助 |
| `soul.rs` | Soul グリッド補助 |
| `stockpile.rs` | ストックパイルグリッド補助 |
| `transport_request.rs` | 輸送要求グリッド補助 |
| `grid.rs` | グリッド共通ユーティリティ |

## 全グリッド更新の登録場所

| グリッド | 登録場所 |
|---|---|
| `SoulSpatialGrid` | `plugins/spatial.rs` (`hw_spatial` 提供) |
| `FamiliarSpatialGrid` | `plugins/spatial.rs` |
| `ResourceSpatialGrid` | `plugins/spatial.rs` |
| `DesignationSpatialGrid` | `plugins/spatial.rs` |
| `GatheringSpatialGrid` | `plugins/spatial.rs` (このディレクトリ提供) |
| `BlueprintSpatialGrid` | `plugins/spatial.rs` |
| `FloorConstructionSpatialGrid` | `plugins/spatial.rs` (このディレクトリ提供) |
| `StockpileSpatialGrid` | `plugins/spatial.rs` |
| `TransportRequestSpatialGrid` | `plugins/spatial.rs` |

---

## hw_spatial との境界

**分割基準**: グリッドに格納するコンポーネント型が hw_* クレートで定義されているかどうか。

| グリッド | 型の定義場所 | グリッドの定義場所 |
|---|---|---|
| `SoulSpatialGrid` 等 7 グリッド | `hw_core` / `hw_jobs` / `hw_logistics` | `hw_spatial` |
| `GatheringSpatialGrid` | `src/` (`GatheringSpot` はルート型) | このディレクトリ |
| `FloorConstructionSpatialGrid` | `src/` (`FloorConstructionSite` はルート型) | このディレクトリ |

新しいグリッドを追加する場合:
- コンポーネント型が hw_* にある → `hw_spatial` に追加し `plugins/spatial.rs` で登録
- コンポーネント型が src/ にある → このディレクトリに追加し `plugins/spatial.rs` で登録
