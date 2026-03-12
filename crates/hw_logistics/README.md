# hw_logistics — 物流実行ロジック・輸送要求システム

## 役割

リソース種別の定義、地上アイテムのライフサイクル、ストックパイル管理、および**輸送要求の生成（producer）・仲裁（arbitration）・プラグイン登録**を提供するクレート。
輸送の実行（AI 行動フェーズでのソース解決・搬送）は `hw_ai` に実装されている。

## 主要モジュール

| ファイル | 内容 |
|---|---|
| `types.rs` | `ResourceType` enum, `ResourceItem`, `Inventory`, `Wheelbarrow` 等 |
| `zone.rs` | ゾーン管理・エリア制御 |
| `water.rs` | 水システムコンポーネント・ロジック |
| `ground_resources.rs` | 地上アイテム（木・岩等）コンポーネント |
| `item_lifetime.rs` | `despawn_expired_items_system` — アイテム消滅タイマー管理 |
| `provisional_wall.rs` | 仮壁ライフサイクル管理ヘルパー |
| `floor_construction.rs` | 床建設サイトへの需要計算・資材消費ヘルパー |
| `wall_construction.rs` | 壁建設サイトへの需要計算・資材消費ヘルパー |
| `tile_index.rs` | `TileSiteIndex` — タイル座標 → サイトエンティティ高速逆引き |
| `resource_cache.rs` | `SharedResourceCache` — タスク間リソース予約キャッシュ |
| `transport_request/` | 輸送要求の完全なライフサイクル（下表参照） |

## transport_request/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | 公開 API 集約 |
| `components.rs` | `TransportRequest`, `TransportDemand` コンポーネント |
| `kinds.rs` | `TransportRequestKind` enum（輸送種別） |
| `lifecycle.rs` | `transport_request_anchor_cleanup_system` |
| `metrics.rs` | `TransportRequestMetrics`, 需要計算システム |
| `state_machine.rs` | `TransportRequestState` (Pending/Claimed) 遷移 |
| `wheelbarrow_completion.rs` | 手押し車輸送完了判定ヘルパー |
| `plugin.rs` | `TransportRequestPlugin`, `TransportRequestSet` |
| `arbitration/` | 手押し車仲裁システム（下表参照） |
| `producer/` | 輸送要求の自動生成システム群（下表参照） |

### arbitration/

| ファイル | 内容 |
|---|---|
| `mod.rs` | `wheelbarrow_arbitration_system` |
| `candidates.rs` | 仲裁候補エントリの収集 |
| `collection.rs` | バッチ候補評価 |
| `grants.rs` | `WheelbarrowLease` の付与・検証 |
| `lease_state.rs` | lease ライフサイクル管理 |
| `metrics_update.rs` | 仲裁メトリクス更新 |
| `types.rs` | 仲裁内部型 (`WheelbarrowCandidate` 等) |

### producer/

| ファイル | 内容 |
|---|---|
| `mod.rs` | 共通ヘルパー (`collect_all_area_owners`, `find_owner`, `sync_construction_requests` 等) |
| `blueprint.rs` | `blueprint_auto_haul_system` |
| `bucket.rs` | `bucket_auto_haul_system` |
| `consolidation.rs` | `stockpile_consolidation_producer_system` |
| `mixer.rs` | `mud_mixer_auto_haul_system` |
| `mixer_helpers/` | mixer 用サブヘルパー群 (`collect`, `desired`, `issue`, `types`, `upsert`) |
| `provisional_wall.rs` | `provisional_wall_auto_haul_system`, `provisional_wall_designation_system` |
| `floor_construction.rs` | `floor_construction_auto_haul_system`, `floor_material_delivery_sync_system`, `floor_tile_designation_system` |
| `stockpile_group.rs` | `StockpileGroup` — Yard 単位ストックパイルグルーピング |
| `tank_water_request.rs` | `tank_water_request_system` |
| `task_area.rs` | `task_area_auto_haul_system` |
| `upsert.rs` | request の upsert/cleanup 共通ヘルパー |
| `wall_construction.rs` | `wall_construction_auto_haul_system`, `wall_material_delivery_sync_system`, `wall_tile_designation_system` |
| `wheelbarrow.rs` | `wheelbarrow_auto_haul_system` |

## 依存クレート

- `bevy` (ECS)
- `hw_core` (AreaBounds, ResourceType, relationships 等)
- `hw_world` (Yard/Site/zones)
- `hw_jobs` (Blueprint/FloorTileBlueprint 等)
- `hw_spatial` (StockpileSpatialGrid 等)
- `rand`

---

## src/ との境界

| hw_logistics に置くもの | src/systems/logistics/ に置くもの |
|---|---|
| 輸送要求の全 producer システム | `initial_spawn.rs`（GameAssets 依存） |
| 手押し車仲裁システム | `ui.rs`（UI ロジスティクス表示） |
| `TransportRequestPlugin` + `TransportRequestSet` | `transport_request/` の thin shell / re-export |
| 建設系需要計算ヘルパー | 後方互換の import path を保つ root shell |
| `TileSiteIndex` 型定義・更新システム | — |
| `SharedResourceCache` | `sync_reservations_system`（ゲーム固有クエリ） |
