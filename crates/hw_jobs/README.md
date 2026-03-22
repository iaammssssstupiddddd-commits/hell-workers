# hw_jobs — タスク・建設状態と visual mirror 同期

## 役割

Soul が実行するタスクの種類・進捗状態、および建物の建設フェーズ状態機械を定義するクレート。
加えて、`hw_core::visual_mirror` へ状態を写す軽量な sync system / observer を持つ。
**AI ロジックは含まない**。タスク model・建設 state・visual mirror 同期に責務を限定する。

## 主要モジュール

| ファイル | 内容 |
|---|---|
| `tasks/` | `AssignedTask` と各タスク種別の進捗型（gather / haul / build / refine / wheelbarrow / bucket transport など） |
| `construction.rs` | 床・壁の建設フェーズ状態機械、タイル Blueprint コンポーネント |
| `model.rs` | `BuildingType`、`MovePlanned`、`Door` / `DoorCloseTimer`、`remove_tile_task_components` |
| `mud_mixer.rs` | 泥ミキサーのワークフロー状態 |
| `events.rs` | タスク完了イベント等 |
| `lifecycle.rs` | タスク予約ライフサイクル helper (`collect_active_reservation_ops`, `collect_release_reservation_ops`) |
| `visual_sync/` | `GatherHighlightMarker` / `RestAreaVisual` / `BuildingVisualState` / `MudMixerVisualState` などの visual mirror 同期関数群 |

## AssignedTask

Soul の現在タスクを表す enum（struct variant 形式）。

主要バリアント: `None`, `Chop`, `Mine`, `Haul`, `Build`, ...

新しいタスクを追加する場合は必ず **struct variant** として定義すること。

## 建設フェーズ状態機械

### 床 (FloorConstructionPhase)
```
ReinforceReady → Reinforcing → PouredReady → Poured
```

### 壁 (WallConstructionPhase)
```
Ready → Framed → ProvisionalReady → CoatedReady → Coated
```

## BuildingType

```rust
Wall, Door, Floor, Tank, MudMixer, RestArea, Bridge, SandPile, BonePile, WheelbarrowParking
```

`required_materials()` で各建物タイプに必要な `ResourceType → 数量` の HashMap を返す。

## 依存クレート

- `hw_core` のみ（軽量な jobs/visual mirror crate）

---

## src/ との境界

hw_jobs は**共有 model / state と visual mirror 同期関数**を提供する。
建設フェーズを実際に進める system や、plugin への登録責務は root app shell 側に残す。

| hw_jobs に置くもの | src/systems/jobs/ に置くもの |
|---|---|
| `AssignedTask` enum（バリアント定義） | タスクハンドラ（`gather.rs`, `build.rs` 等） |
| `FloorConstructionPhase` / `WallConstructionPhase` 状態機械型 | `floor_construction_phase_transition_system` 等の実システム |
| `BuildingType` と `required_materials()` | 建物完成後処理・ワールドマップ更新 |
| `Door`, `DoorCloseTimer` | ドア自動開閉 system (`hw_world`) と UI からの状態変更適用 |
| `MudMixerInputSlot` / `MudMixerOutputSlot` 型 | 泥ミキサーのフロー制御システム |
| タスク予約ライフサイクル helper (`lifecycle.rs`) | 予約再構築を呼ぶゲーム側システム |
| `remove_tile_task_components` | 建設フェーズ遷移の apply system |
| 建設状態コンポーネント（`FloorTileState`, `WallTileState` 等） | コンポーネントの Bevy 登録・Observer 配線 |
| `FloorTileBlueprint`, `WallTileBlueprint`, `FloorConstructionSite`, `WallConstructionSite` | これらを進行させる build / logistics / visual system |
| `TargetFloorConstructionSite`, `TargetWallConstructionSite` | — |
| `FloorConstructionCancelRequested`, `WallConstructionCancelRequested` | — |
| `visual_sync::{observers,sync}` の関数本体 | `bevy_app/src/plugins/logic.rs` での `add_systems` / `add_observer` 登録 |

新しいタスクバリアントを追加する場合:
1. `tasks/` 配下の適切なモジュールと `tasks/mod.rs` に struct variant を追加（hw_jobs）
2. `src/systems/soul_ai/execute/task_execution/` にハンドラを実装（src/）
