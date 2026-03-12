# spatial — root adapter for `hw_spatial`

## 役割

空間グリッド本体は `hw_spatial` が所有する。
このディレクトリは root crate 側のコンポーネント型を `hw_spatial` の汎用 update system に束ねる adapter と、既存 import path を維持する re-export shell を置く。

## ファイル一覧

| ファイル | 内容 |
|---|---|
| `mod.rs` | root 互換パスとして各 grid / update system を再公開 |
| `blueprint.rs` | root の `Blueprint` 型に対して `hw_spatial` の汎用 update system を束縛 |
| `designation.rs` | root の `Designation` 型に対する adapter |
| `familiar.rs` | root の `Familiar` 型に対する adapter |
| `resource.rs` | root の `ResourceItem` 型に対する adapter |
| `soul.rs` | root の `DamnedSoul` 型に対する adapter |
| `stockpile.rs` | root の `Stockpile` 型に対する adapter |
| `transport_request.rs` | root の `TransportRequest` 型に対する adapter |
| `floor_construction.rs` | `hw_spatial::floor_construction` の thin shell |
| `gathering.rs` | `hw_spatial::gathering` の thin shell |
| `grid.rs` | `GridData` / `SpatialGridOps` の re-export |

## 全グリッド更新の登録場所

| グリッド | 登録場所 |
|---|---|
| `SoulSpatialGrid` | `plugins/spatial.rs` (`hw_spatial` の汎用 system を root 型で束縛) |
| `FamiliarSpatialGrid` | `plugins/spatial.rs` |
| `ResourceSpatialGrid` | `plugins/spatial.rs` |
| `DesignationSpatialGrid` | `plugins/spatial.rs` |
| `GatheringSpotSpatialGrid` | `plugins/spatial.rs` (`hw_spatial` 提供) |
| `BlueprintSpatialGrid` | `plugins/spatial.rs` |
| `FloorConstructionSpatialGrid` | `plugins/spatial.rs` (`hw_spatial` 提供) |
| `StockpileSpatialGrid` | `plugins/spatial.rs` |
| `TransportRequestSpatialGrid` | `plugins/spatial.rs` |

---

## hw_spatial との境界

`hw_spatial` は grid resource 本体と汎用 update system を所有する。
root `src/systems/spatial/` は、root 側で定義されたコンポーネント型をその汎用 system に渡す adapter と、後方互換の公開パスだけを持つ。

| 役割 | 所有先 |
|---|---|
| `GridData<T>`, `SpatialGridOps`, 全 grid resource | `hw_spatial` |
| `FloorConstructionSpatialGrid`, `GatheringSpotSpatialGrid` の定義と update system | `hw_spatial` |
| root 型 (`Blueprint`, `Designation`, `DamnedSoul` など) への generic binding | `src/systems/spatial/` |
| 互換 re-export パス | `src/systems/spatial/` |

新しいグリッドを追加する場合:
- grid resource 本体と update system は `hw_spatial` に追加する
- root 固有コンポーネント型を generic system に束ねる必要がある場合だけ、このディレクトリに薄い adapter を追加する
