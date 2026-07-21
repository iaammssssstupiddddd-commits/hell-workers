# hw_spatial — 空間インデックス・グリッド検索

## 役割

ゲームワールドをグリッドに分割し、エンティティの近傍検索・位置クエリを O(1) ∼ O(n_cell) で実現する空間インデックスクレート。
`GameSystemSet::Spatial` フェーズで全グリッドを更新し、`Logic` フェーズの AI がクエリを発行する。

## グリッド一覧

| グリッド | ファイル | 用途 / 更新 policy |
|---|---|---|
| `DesignationSpatialGrid` | `designation.rs` | 未割当タスク検索。標準 Transform policy |
| `TransportRequestSpatialGrid` | `transport_request.rs` | 輸送要求の近傍検索。標準 Transform policy |
| `ResourceSpatialGrid` | `resource.rs` | 地上アイテム位置検索。Visibility を考慮する専用 policy |
| `StockpileSpatialGrid` | `stockpile.rs` | ストックパイル位置検索。標準 Transform policy |
| `SpatialGrid` | `soul.rs` | Soul 位置検索。標準 Transform policy。`SoulSpatialGrid` という型名はない |
| `FamiliarSpatialGrid` | `familiar.rs` | Familiar 位置検索。標準 Transform policy |
| `BlueprintSpatialGrid` | `blueprint.rs` | 建設ブループリント位置検索。標準 Transform policy |
| `GatheringSpotSpatialGrid` | `gathering.rs` | 集会スポット位置検索。`GatheringSpot.center` の Added-only policy |
| `FloorConstructionSpatialGrid` | `floor_construction.rs` | 床建設サイト位置検索。標準 Transform policy |

`door_proximity.rs`は`SpatialGrid`から近傍Soulだけを抽出するdoor auto-open/close adapterと
`DoorPerfMetrics`を所有する。1候補の開閉ruleと`DoorState`適用は`hw_world`、production登録はrootが所有する。

## 主要型

| ファイル | 内容 |
|---|---|
| `grid.rs` | `GridData`、`SpatialIndex<Tag>`、crate 所有 ZST tag、共通 Transform updater |
| 各 grid module | 公開 alias と concrete component wrapper。Resource / Gathering は専用 policy を所有 |
| `lib.rs` | 全 alias、tag、共通 API の pub re-export |

### SpatialGridOps トレイト

traitの正本は`hw_world/src/spatial.rs`です。`hw_spatial::grid`はこれをre-exportし、
concreteな`SpatialIndex<Tag>`実装を一つだけ提供します。

```rust
trait SpatialGridOps {
    fn insert(&mut self, entity: Entity, pos: Vec2);
    fn remove(&mut self, entity: Entity);
    fn update(&mut self, entity: Entity, pos: Vec2);
    fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity>;
    fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>);
}
```

標準 7 系統は `SpatialIndex<Tag>` に対する唯一の `SpatialGridOps` 実装と
`update_transform_spatial_index_system::<Tag, Tracked>` を共有する。tag は必ず
`hw_spatial` が所有し、downstream domain component を tag にしない。
custom cell size または内部 grid の検査・構成が必要な場合は、tuple field に依存せず
`SpatialIndex::new(GridData)`、`data`、`data_mut`、`into_data` を使う。
`generation()` は membership または記録位置が実際に変化したときだけ進む semantic generation であり、
task diagnostics の availability revision bridge が stale reason の失効に使う。読み取りや同値 update では進めない。

## 更新タイミング

- 標準 7 系統は `Spatial` フェーズ（毎フレーム）で `Added<Tracked>` / `Changed<Transform>` / `RemovedComponents<Tracked>` を利用して差分更新。
- `ResourceSpatialGrid` は `Visibility::Hidden` を除外し、Visibility が外れた item を可視として再登録する。
- `GatheringSpotSpatialGrid` は spawn 時の `GatheringSpot.center` だけを読み、`grace_timer` 等による Changed を無視する。
- AI の `Perceive` フェーズは必ず `Spatial` フェーズの後に実行されるため、常に最新のグリッド状態を参照できる。

## 依存クレート

- `hw_core`, `hw_jobs`, `hw_world`

---

## bevy_app との境界

hw_spatial は grid resource 本体と汎用 update system を所有する。`crates/bevy_app/src/systems/spatial/` は削除済みで、**`crates/bevy_app/src/plugins/spatial.rs`** の `SpatialPlugin` が `hw_spatial` と `hw_logistics`（`ResourceItem` / `Stockpile` 向け更新の一部）をまとめて `GameSystemSet::Spatial` に登録する。

| 置き場所 | 内容 |
|---|---|
| `hw_world` | downstream AI helperも参照できる`SpatialGridOps` trait契約 |
| `hw_spatial` | `SpatialIndex<Tag>`、`GridData`、traitのconcrete impl、crate所有tag、標準Transform updater、Resource/Gathering専用policy、door proximity adapter |
| `hw_logistics` | `ResourceItem` / `Stockpile` / `TransportRequest` 向けの component 特化ラッパ（例: `update_resource_spatial_grid_system_resource_item`） |
| `bevy_app::plugins::spatial` | 上記システムの登録のみ（adapter 用の `systems/spatial` モジュールはない） |
