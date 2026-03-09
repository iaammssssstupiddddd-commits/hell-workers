# hw_jobs — タスク・建設データ定義

## 役割

Soul が実行するタスクの種類・進捗状態、および建物の建設フェーズ状態機械を定義するクレート。
**AI ロジックは含まない**。データ型と状態遷移の定義のみ。

## 主要モジュール

| ファイル | 内容 |
|---|---|
| `assigned_task.rs` | `AssignedTask` enum — Soul に割り当てられたタスクと進捗 |
| `construction.rs` | 床・壁の建設フェーズ状態機械、タイル Blueprint コンポーネント |
| `model.rs` | `BuildingType` enum と必要素材マッピング |
| `mud_mixer.rs` | 泥ミキサーのワークフロー状態 |
| `events.rs` | タスク完了イベント等 |

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

- `hw_core` のみ（軽量な純粋データクレート）

---

## src/ との境界

hw_jobs は**型・状態機械定義のみ**を提供する。
建設フェーズを実際に進めるシステムは `src/systems/jobs/` に実装する。

| hw_jobs に置くもの | src/systems/jobs/ に置くもの |
|---|---|
| `AssignedTask` enum（バリアント定義） | タスクハンドラ（`gather.rs`, `build.rs` 等） |
| `FloorConstructionPhase` / `WallConstructionPhase` 状態機械型 | `floor_construction_phase_transition_system` 等の実システム |
| `BuildingType` と `required_materials()` | 建物完成後処理・ワールドマップ更新 |
| `MudMixerInputSlot` / `MudMixerOutputSlot` 型 | 泥ミキサーのフロー制御システム |
| 建設状態コンポーネント（`FloorTileState`, `WallTileState` 等） | コンポーネントの Bevy 登録・Observer 配線 |
| `FloorTileBlueprint`, `WallTileBlueprint`（タイル Blueprint） | `FloorConstructionSite`, `WallConstructionSite`（`TaskArea` 依存のため root 残留） |
| `TargetFloorConstructionSite`, `TargetWallConstructionSite` | — |
| `FloorConstructionCancelRequested`, `WallConstructionCancelRequested` | — |

新しいタスクバリアントを追加する場合:
1. `assigned_task.rs` に struct variant を追加（hw_jobs）
2. `src/systems/soul_ai/execute/task_execution/` にハンドラを実装（src/）
