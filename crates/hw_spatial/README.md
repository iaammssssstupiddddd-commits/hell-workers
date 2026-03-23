# hw_spatial — 空間インデックス・グリッド検索

## 役割

ゲームワールドをグリッドに分割し、エンティティの近傍検索・位置クエリを O(1) ∼ O(n_cell) で実現する空間インデックスクレート。
`GameSystemSet::Spatial` フェーズで全グリッドを更新し、`Logic` フェーズの AI がクエリを発行する。

## グリッド一覧

| グリッド | ファイル | 用途 |
|---|---|---|
| `DesignationSpatialGrid` | `designation.rs` | 未割当タスク検索（Familiar タスク探索用） |
| `TransportRequestSpatialGrid` | `transport_request.rs` | 輸送要求の近傍検索 |
| `ResourceSpatialGrid` | `resource.rs` | 地上アイテム位置検索 |
| `StockpileSpatialGrid` | `stockpile.rs` | ストックパイル位置検索 |
| `SpatialGrid` | `soul.rs` | Soul 位置検索（経路探索・分離行動用）。`SoulSpatialGrid` という型名はない |
| `FamiliarSpatialGrid` | `familiar.rs` | Familiar 位置検索 |
| `BlueprintSpatialGrid` | `blueprint.rs` | 建設ブループリント位置検索 |
| `GatheringSpotSpatialGrid` | `gathering.rs` | 集会スポット位置検索 |
| `FloorConstructionSpatialGrid` | `floor_construction.rs` | 床建設サイト位置検索 |

## 主要型

| ファイル | 内容 |
|---|---|
| `grid.rs` | `GridData<T>` — ボクセルグリッド実装, `SpatialGridOps` トレイト |
| `lib.rs` | 全グリッドの pub re-export |

### SpatialGridOps トレイト

```rust
trait SpatialGridOps {
    fn update(&mut self, ...);       // グリッドセルへの挿入
    fn query_radius(&self, ...);     // 半径内エンティティ取得
    fn query_nearest(&self, ...);    // 最近傍エンティティ取得
}
```

## 更新タイミング

- `Spatial` フェーズ（毎フレーム）で Bevy の **Change Detection**（Added / Changed / RemovedComponents）を利用して差分更新。
- AI の `Perceive` フェーズは必ず `Spatial` フェーズの後に実行されるため、常に最新のグリッド状態を参照できる。

## 依存クレート

- `hw_core`, `hw_jobs`, `hw_logistics`, `hw_world`

---

## bevy_app との境界

hw_spatial は grid resource 本体と汎用 update system を所有する。`crates/bevy_app/src/systems/spatial/` は削除済みで、**`crates/bevy_app/src/plugins/spatial.rs`** の `SpatialPlugin` が `hw_spatial` と `hw_logistics`（`ResourceItem` / `Stockpile` 向け更新の一部）をまとめて `GameSystemSet::Spatial` に登録する。

| 置き場所 | 内容 |
|---|---|
| `hw_spatial` | 各 `*SpatialGrid` / `SpatialGrid`、`GridData`、`SpatialGridOps`、コンポーネント束ねの generic update system |
| `hw_logistics` | `ResourceSpatialGrid` / `StockpileSpatialGrid` 向けのコンポーネント特化ラッパ（例: `update_resource_spatial_grid_system_resource_item`） |
| `bevy_app::plugins::spatial` | 上記システムの登録のみ（adapter 用の `systems/spatial` モジュールはない） |
