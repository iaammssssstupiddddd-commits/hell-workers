# logistics — リソース管理・輸送システム

## 役割

ゲーム内リソース（木・岩・砂・水等）のライフサイクル管理、ストックパイル、輸送要求の生成・仲裁・実行を担うシステム群。
型定義は `hw_logistics` クレートにあり、このディレクトリは Bevy システムとして動作する**実装**を担う。

## 主要ファイル

| ファイル | 内容 |
|---|---|
| `mod.rs` | 全モジュールの公開 API |
| `types.rs` | `ResourceType`, `ResourceItem`, `Stockpile` 等（`hw_logistics` から re-export 含む） |
| `zone.rs` | `ZoneType`（Stockpile/Yard）・ゾーン管理システム |
| `initial_spawn.rs` | 初期リソースエンティティのスポーン |
| `ground_resources.rs` | 地上アイテムの追跡・更新システム |
| `item_lifetime.rs` | `despawn_expired_items_system` — 時間切れアイテムの消滅 |
| `provisional_wall.rs` | 仮壁のライフサイクル管理 |
| `floor_construction.rs` | 床建設サイトのロジスティクス |
| `wall_construction.rs` | 壁建設サイトのロジスティクス |
| `tile_index.rs` | `TileSiteIndex` — タイル→サイト高速ルックアップ |
| `water.rs` | 水システム（川・バケツ・タンク） |
| `ui.rs` | ロジスティクス UI ヘルパー |

## transport_request/ ディレクトリ

輸送要求の完全なライフサイクルを管理するサブシステム。

| ファイル/ディレクトリ | 内容 |
|---|---|
| `plugin.rs` | `TransportRequestPlugin`, `TransportRequestSet` |
| `components.rs` | `TransportRequest`, `TransportDemand` コンポーネント |
| `kinds.rs` | `TransportRequestKind` enum（輸送種別） |
| `lifecycle.rs` | `transport_request_anchor_cleanup_system` |
| `metrics.rs` | `TransportRequestMetrics`, 需要計算 |
| `state_machine.rs` | `TransportRequestState` (Pending/Claimed) 遷移 |
| `wheelbarrow_completion.rs` | 手押し車輸送完了処理 |
| `arbitration/` | 輸送要求の調停・優先度決定 |
| `producer/` | 輸送要求の自動生成（ストックパイル・建設・ミキサー等） |

### TransportRequestSet 実行順序

```
Perceive → Decide → Arbitrate → Execute → Maintain
```

Familiar `Update` と `Decide` の間、および Soul `Execute` の後に配置される。

## 輸送要求プロデューサー一覧（producer/）

`blueprint.rs` / `bucket.rs` / `consolidation.rs` / `floor_construction.rs` / `mixer.rs` / `mixer_helpers.rs` / `provisional_wall.rs` / `stockpile_group.rs` / `tank_water_request.rs` / `task_area.rs` / `wall_construction.rs` / `wheelbarrow.rs` / `upsert.rs`

---

## hw_logistics との境界

ロジスティクスは `hw_logistics` クレートと `src/systems/logistics` に分割されている。

### hw_logistics に置かれているもの（型定義のみ）

| 型 | 内容 |
|---|---|
| `ResourceType` | リソース種別 enum |
| `ResourceItem(ResourceType)` | 地上アイテムコンポーネント |
| `Inventory(Option<Entity>)` | Soul 所持品スロット |
| `Wheelbarrow { capacity }` | 手押し車コンポーネント |
| `ReservedForTask` | タスク予約マーカー |
| `TransportRequest` (型定義) | 輸送要求の基本型 |
| `TransportRequestKind` | 輸送種別 enum |
| `item_lifetime` システム | アイテム消滅タイマー（純粋） |
| `SharedResourceCache`（予約キャッシュ型） | `sync_reservations_system` 等（ゲーム固有クエリを持つ） |

### src/ に置かれているもの（ゲーム固有システム）

| モジュール | 理由 |
|---|---|
| `transport_request/plugin.rs` | `TransportRequestPlugin` — ゲーム固有システム登録 |
| `transport_request/arbitration/` | `WorldMap` 経路コスト・空間グリッドを参照した調停 |
| `transport_request/producer/` | 各建設・ストックパイル状態に応じた要求生成 |
| `tile_index.rs` | `TileSiteIndex` — タイル座標 → サイトエンティティの高速逆引き |
| `initial_spawn.rs` | 初期リソースエンティティのスポーン |
| `floor_construction.rs` / `wall_construction.rs` | 建設サイトとリソースの関連付け |
| `zone.rs` | `Stockpile` / `Yard` ゾーン管理システム |
| `water.rs` | 水・バケツ・タンクの状態管理 |

### 典型的な境界

```rust
// src/systems/logistics/mod.rs
// hw_logistics の型を透過的に re-export
pub use hw_logistics::types::*;  // ResourceType, ResourceItem, Inventory, Wheelbarrow...
pub use hw_logistics::transport_request::components::*;

// src/ 固有の型は直接定義
pub struct TileSiteIndex { ... }  // WorldMap 座標系に依存
```
