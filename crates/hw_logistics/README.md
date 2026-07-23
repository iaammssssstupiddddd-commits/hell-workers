# hw_logistics — 物流実行ロジック・輸送要求システム

## 役割

リソース種別の定義、地上アイテムのライフサイクル、ストックパイル管理、および**輸送要求の生成（producer）・仲裁（arbitration）・プラグイン登録**を提供するクレート。
輸送の実行（AI 行動フェーズでのソース解決・搬送）は `hw_familiar_ai`・`hw_soul_ai` に実装されている。

## 主要モジュール

| ファイル | 内容 |
|---|---|
| `types.rs` | `ResourceType` enum, `ResourceItem`, `Inventory`, `Wheelbarrow` 等 |
| `zone.rs` | ゾーン管理・エリア制御、通常セルの永続 `StockpilePolicy` と共通 patch 型 |
| `stockpile_policy.rs` | 搬入・確定済み搬入・搬出を予約込みで判定する副作用のない policy evaluator と、通常 Stockpile の owner 互換契約 |
| `stockpile_policy_change.rs` | 単一セル・範囲編集で共有する typed request / outcome と、managed cell 境界を再検証する policy 適用システム |
| `water.rs` | 水システムコンポーネント・ロジック |
| `ground_resources.rs` | 地上アイテム（木・岩等）コンポーネント |
| `item_lifetime.rs` | `despawn_expired_items_system` — アイテム消滅タイマー管理 |
| `provisional_wall.rs` | 仮壁ライフサイクル管理ヘルパー |
| `floor_construction.rs` | 床建設サイトへの需要計算・資材消費ヘルパー |
| `wall_construction.rs` | 壁建設サイトへの需要計算・資材消費ヘルパー |
| `tile_index.rs` | `TileSiteIndex` — タイル座標 → サイトエンティティ高速逆引き |
| `construction_phase_transition.rs` | `TileSiteIndex`で当該siteだけを検証するfloor/wall phase transitionとprofiling metrics |
| `resource_cache.rs` | `SharedResourceCache` — タスク間リソース予約キャッシュ。`begin_frame` は frame delta のみを clear し、`replace_reservation_snapshot` は予約 snapshot のみを置換する |
| `construction_helpers.rs` | `ResourceItemVisualHandles`, `spawn_refund_items` — 建設キャンセル返却 helper |
| `plugin.rs` | `LogisticsPlugin` — `apply_reservation_requests_system` のプラグイン登録 |
| `manual_haul_selector.rs` | 手動運搬選定ロジック。managed cell は共通 `NewInbound` evaluator と reservation shadow、特殊 bucket storage は既存専用規則を使用 |
| `spatial_sync.rs` | `ResourceSpatialGrid`・`StockpileSpatialGrid`・`TransportRequestSpatialGrid` 更新システム |
| `visual_sync.rs` | `WheelbarrowMarker`・`InventoryItemVisual` 等の visual mirror 同期 Observer |
| `transport_request/` | 輸送要求の完全なライフサイクル（下表参照） |

## transport_request/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | 公開 API 集約 |
| `components.rs` | `TransportRequest`, `TransportDemand`, policy-driven request の runtime-only `ReceiverPolicyTier` コンポーネント |
| `kinds.rs` | `TransportRequestKind` enum（輸送種別） |
| `lifecycle.rs` | `transport_request_anchor_cleanup_system` と UI/anchor共用の `close_manual_transport_request` typed owner API |
| `metrics.rs` | `TransportRequestMetrics`, 需要計算システム |
| `state_machine.rs` | `TransportRequestState` (Pending/Claimed) 遷移 |
| `wheelbarrow_completion.rs` | 手押し車輸送完了判定ヘルパー |
| `plugin.rs` | `TransportRequestPlugin`, `TransportRequestSet` |
| `arbitration/` | 手押し車仲裁システム（下表参照） |
| `producer/` | 輸送要求の自動生成システム群（下表参照） |

### arbitration/

| ファイル | 内容 |
|---|---|
| `mod.rs` | `WheelbarrowArbitrationRuntime` / `WheelbarrowArbitrationDirtyParams` と `wheelbarrow_arbitration_system` の公開 root shell |
| `candidates.rs` | 仲裁候補エントリの収集と、receiver tier / live reservation を含む単一セル policy 評価 |
| `collection.rs` | バッチ候補評価。同 owner source を優先し、不在時だけ owner 未設定の地面資材へフォールバック |
| `grants.rs` | grant 直前 policy / 実 item-owner 再検証、資源別 cycle shadow、batch clamp、`WheelbarrowLease` 付与 |
| `lease_state.rs` | lease ライフサイクル管理。`Designation` 不在または `Demand=0` の Pending request から lease / pending timer を即時解放 |
| `metrics_update.rs` | 仲裁メトリクス更新 |
| `system.rs` | `wheelbarrow_arbitration_system` の実装本体 |
| `types.rs` | 仲裁内部型 (`WheelbarrowCandidate` 等) |
| `diagnostics.rs` | latest-only `WheelbarrowArbitrationDiagnostics`、物理/available車両header、request別typed outcome |

### producer/

| ファイル | 内容 |
|---|---|
| `mod.rs` | 共通ヘルパー (`collect_all_area_owners`, `find_owner`, `sync_construction_requests` 等) |
| `blueprint.rs` | `blueprint_auto_haul_system` |
| `bucket.rs` | `bucket_auto_haul_system` |
| `consolidation.rs` | `stockpile_consolidation_producer_system`。receiver=`NewInbound`、donor=`NewOutbound`、draining override、receiver tier、committed worker 保持を統合 |
| `mixer.rs` | `mud_mixer_auto_haul_system` |
| `mixer_helpers/` | mixer 用サブヘルパー群 (`collect`, `desired`, `issue`, `types`, `upsert`) |
| `provisional_wall.rs` | `provisional_wall_auto_haul_system`, `provisional_wall_designation_system` |
| `floor_construction.rs` | `floor_construction_auto_haul_system`, `floor_material_delivery_sync_system`, `floor_tile_designation_system` |
| `stockpile_group.rs` | `StockpileGroup` — Yard 単位ストックパイルグルーピング |
| `active_unit_cache.rs` | Familiar / Yard と、`With<StockpilePolicy>` membership だけを保持する構造 group cache。live policy / stored / incoming は保持しない |
| `tank_water_request.rs` | `tank_water_request_system` |
| `task_area.rs` | `task_area_auto_haul_system`。tier 別 request の決定的生成と semantic-diff upsert |
| `upsert.rs` | request の upsert/cleanup 共通ヘルパー |
| `wall_construction.rs` | `wall_construction_auto_haul_system`, `wall_material_delivery_sync_system`, `wall_tile_designation_system` |
| `wheelbarrow.rs` | `wheelbarrow_auto_haul_system` |

`DeliverToSoulSpa`は有効な`TransportRequestKind`だが、producerだけはroot固有のSoul Spa建設siteと
energy orderingへ接続するため`bevy_app/src/systems/jobs/soul_spa_construction/auto_haul.rs`が所有する。

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
| manual request close primitive / arbitration diagnostics | UI actionのroot adapter（owner APIを呼ぶだけ） |
| `TransportRequestPlugin` + `TransportRequestSet` | `transport_request/` の thin shell / re-export |
| 建設系需要計算ヘルパー | 後方互換の import path を保つ root shell |
| `ResourceItemVisualHandles` と `spawn_refund_items` | `GameAssets` から handle Resource を注入する startup |
| `TileSiteIndex` 型定義・更新システム | — |
| `construction_phase_transition`（index-backed floor/wall adapter、`ConstructionPerfMetrics`） | transitionのproduction登録、cancel/completion、asset依存spawn |
| `SharedResourceCache` | `sync_reservations_system`（ゲーム固有クエリ） |
| Stockpile policy のデータ型・純粋 evaluator・typed change handler | policy の保存移行、UI adapter、ゲーム固有の producer / execution 接続 |

手押し車仲裁は source reservation を近傍 Top-K の投入前に除外する。実検索範囲内で予約を含めれば `hard_min` を
満たせる場合は、全件予約・一部予約のどちらも `SourceReserved`、予約を含めても不足する場合は `NoSourceItems` とし、
予約済み item を lease へ含めない。Stockpile destination は候補化時と grant 直前に共通 evaluator で再評価し、
group 合計ではなく実際に選ぶ単一セルの available amount へ batch を制限する。mixed-owner group のセル再選択では、
実 lease item と destination cell の owner 互換性も維持する。
