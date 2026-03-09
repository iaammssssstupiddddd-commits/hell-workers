# hw_logistics — 輸送・リソース・ストックパイル管理

## 役割

リソース種別の定義、地上アイテムのライフサイクル、ストックパイル管理、輸送要求システムを提供するクレート。
**輸送の実行（AI 行動）は `hw_ai` に実装**されており、このクレートはデータ型と要求定義のみを担う。

## 主要モジュール

| ファイル | 内容 |
|---|---|
| `types.rs` | `ResourceType` enum (Wood, Rock, Bone, Sand, StasisMud, Water, ...) |
| `zone.rs` | ゾーン管理・エリア制御 |
| `water.rs` | 水システムコンポーネント・ロジック |
| `ground_resources.rs` | 地上アイテム（木・岩等）コンポーネント |
| `item_lifetime.rs` | アイテム消滅タイマー（StasisMud/Sand は 5 秒後消滅） |
| `transport_request/` | 輸送要求エンティティ（下表参照） |

## transport_request/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | `TransportRequest` エンティティ型 |
| `kinds.rs` | `TransportRequestKind` enum（輸送種別） |
| `components.rs` | `TransportRequest`, `TransportDemand` コンポーネント |
| `metrics.rs` | 在庫メトリクス・需要計算 |
| `state_machine.rs` | `TransportRequestState` (Pending/Claimed) 遷移 |

## TransportRequestKind

```rust
DepositToStockpile       // ストックパイルへ格納
DeliverToBlueprint       // 建設ブループリントへ配達
DeliverToMixerSolid      // ミキサーへ固体素材配達
DeliverToFloorConstruction
DeliverToWallConstruction
DeliverToProvisionalWall
DeliverWaterToMixer
GatherWaterToTank
ReturnBucket
BatchWheelbarrow
ConsolidateStockpile
```

## アイテムライフサイクル

```
地上スポーン
  → [予約済み / 輸送中 / 格納済み] → 通常維持
  → [予約なし・格納なし・輸送中でない] → 5秒後に自動消滅
```

## 依存クレート

- `hw_core` のみ（軽量な純粋データクレート）

---

## src/ との境界

hw_logistics は**型定義と純粋なアイテムライフサイクルのみ**を提供する。
リソース生成・輸送実行・ゾーン管理は `src/systems/logistics/` に実装する。

| hw_logistics に置くもの | src/systems/logistics/ に置くもの |
|---|---|
| `ResourceType`, `ResourceItem`, `Inventory` 等の型 | 初期リソーススポーン (`initial_spawn.rs`) |
| `TransportRequestKind` enum | 輸送要求プロデューサー全般 (`producer/`) |
| `TransportRequest`, `TransportDemand` コンポーネント型 | 輸送要求の調停システム (`arbitration/`) |
| `item_lifetime` タイマーシステム（純粋） | `TileSiteIndex`（WorldMap 座標系依存） |
| `TransportRequestState` 状態遷移型 | ゾーン管理・水システム実装 |

src/ 側では hw_logistics の型を透過的に re-export している:
```rust
// src/systems/logistics/mod.rs
pub use hw_logistics::types::*;           // ResourceType, ResourceItem, ...
pub use hw_logistics::transport_request::components::*;
```
