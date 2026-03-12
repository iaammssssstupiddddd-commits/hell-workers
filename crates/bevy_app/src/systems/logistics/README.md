# logistics — root shell + app-specific logistics

## 役割

物流ロジック本体は `hw_logistics` クレートが所有する。
このディレクトリは root crate 固有の依存を持つ処理と、既存 import path を維持する thin shell だけを担う。

- `initial_spawn/` — `GameAssets` 依存の初期リソーススポーン（ディレクトリ）
- `ui.rs` — ロジスティクス UI ヘルパー
- `transport_request/` — `hw_logistics` 実装への thin shell / re-export
- `mod.rs` — `hw_logistics` 公開 API の再公開と互換パス維持

## 主要ファイル

| ファイル | 内容 |
|---|---|
| `mod.rs` | `hw_logistics` の re-export + `initial_spawn`, `ui` 公開 |
| `initial_spawn/mod.rs` | facade: `initial_resource_spawner` のみ（~60行） |
| `initial_spawn/layout.rs` | pure 計算: Bevy/Commands 依存なし（Site/Yard・Parking 配置）|
| `initial_spawn/terrain_resources.rs` | Tree / Rock / Wood spawn 実装 |
| `initial_spawn/facilities.rs` | Site / Yard / WheelbarrowParking spawn 実装 |
| `initial_spawn/report.rs` | `InitialSpawnReport` によるログ集約 |
| `ui.rs` | ロジスティクス UI ヘルパー |

## initial_spawn/ モジュール構成

```
initial_spawn/
├── mod.rs               ← facade: スポーン順序のみを制御
├── layout.rs            ← pure 計算（Bevy Commands 非依存）
│   ├── compute_site_yard_layout() → Result<SiteYardLayout, SiteYardLayoutError>
│   └── compute_parking_layout(base, &WorldMap) → Option<ParkingLayout>
├── terrain_resources.rs ← Tree・Rock・Wood spawn
│   ├── spawn_trees / spawn_rocks（共通 spawn_obstacle_batch helper 利用）
│   └── spawn_initial_wood
├── facilities.rs        ← Site/Yard・WheelbarrowParking spawn
│   ├── spawn_site_and_yard(&mut Commands, &SiteYardLayout)
│   └── spawn_wheelbarrow_parking(&mut Commands, &GameAssets, &mut WorldMap, &ParkingLayout)
└── report.rs            ← InitialSpawnReport（ログ集約）
```

**スポーン順序（`mod.rs` で固定）:**
1. 地形障害物（Tree / Rock）— `add_grid_obstacle` 呼び出しを伴う
2. 拾得可能アイテム（Wood）
3. 施設（WheelbarrowParking / Site / Yard）— 障害物スポーン後に `register_completed_building_footprint`

**`layout.rs` の境界:** `compute_site_yard_layout` は定数のみ参照する pure 関数。`compute_parking_layout` は `&WorldMap` の読み取り専用参照のみを受け取り、`Commands` には依存しない。

## transport_request/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | `hw_logistics::transport_request` の型を re-export + `plugin`, `producer` を公開 |
| `plugin.rs` | `TransportRequestPlugin` / `TransportRequestSet` の thin shell |

## transport_request/producer/ ディレクトリ

floor / wall construction producer 実装本体は `hw_logistics` に移り、このディレクトリには互換 re-export だけが残る。

| ファイル | 内容 |
|---|---|
| `mod.rs` | thin shell module 宣言 |
| `floor_construction.rs` | `hw_logistics::transport_request::producer::floor_construction` の re-export |
| `wall_construction.rs` | `hw_logistics::transport_request::producer::wall_construction` の re-export |

---

## hw_logistics との境界

このディレクトリが保持するもの:

| ファイル | 残留理由 |
|---|---|
| `initial_spawn/` | `GameAssets` リソース（テクスチャ等）に依存 |
| `ui.rs` | UI レンダリングに依存 |
| `transport_request/plugin.rs` | 後方互換の import path を維持する thin shell |
| `transport_request/producer/*.rs` | 後方互換の import path を維持する thin shell |

hw_logistics に移植済み（re-export 経由で公開）:

- 全 transport request producer（`blueprint`, `bucket`, `consolidation`, `mixer`, `provisional_wall`, `stockpile_group`, `tank_water_request`, `task_area`, `upsert`, `wheelbarrow`）
- floor / wall construction producer（`floor_construction`, `wall_construction`）
- 手押し車仲裁システム（`arbitration/`）
- `TransportRequestPlugin`, `TransportRequestSet`
- 建設系需要計算ヘルパー（`floor_construction.rs`, `wall_construction.rs`, `tile_index.rs`）
- アイテムライフサイクル管理（`item_lifetime.rs`）
