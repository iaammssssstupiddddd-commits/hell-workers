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
| `SoulSpatialGrid` | `soul.rs` | Soul 位置検索（経路探索・分離行動用） |
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

## src/ との境界

hw_spatial は grid resource 本体と汎用 update system を所有する。
root `src/systems/spatial/` は root 側コンポーネント型をその generic system に束ねる adapter と、既存 import path を保つ re-export shell だけを持つ。

| hw_spatial に置くもの | src/systems/spatial/ に置くもの |
|---|---|
| `SoulSpatialGrid` から `FloorConstructionSpatialGrid` まで全 grid resource | root 型に対する generic update system の binding wrapper |
| 全 grid の update system 本体 | 互換 re-export パス |
| `GridData<T>`, `SpatialGridOps` トレイト | `plugins/spatial.rs` から参照する薄い adapter |
