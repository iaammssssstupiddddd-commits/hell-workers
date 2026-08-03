# Room 検出システム (Room Detection System)

このドキュメントでは、壁・扉・床で囲まれた空間を「Room」として自動検出するシステムについて説明します。

## 1. 概要

Room 検出システムは、完成した壁・扉・床で構成された密閉空間を `Room` エンティティとして自動認識します。
検出された Room は床を塗りつぶさず、床と外周壁の室内側に半透明の境界線を表示します。
Roomデータは将来の温度・モラル・部屋品質バフ等の基盤になります。

実装境界は次の 4 つの責務境界です。

- `crates/hw_world::room_detection`: pure core かつ ECS 型の所有者。入力分類、flood-fill、妥当性判定、`RoomBounds`、**`Room`/`RoomOverlayTile`（Component）**、**`RoomTileLookup`/`RoomDetectionState`/`RoomValidationState`（Resource）** を保持する。
- `crates/hw_world::room_systems`: ECS adapter 層。`detect_rooms_system` / `validate_rooms_system` / `mark_room_dirty_from_building_changes_system` / `on_building_added` / `on_building_removed` / `on_door_added` / `on_door_removed` / `sync_room_overlay_tiles_system` の実装本体をすべて所有する。`Building + Transform` クエリから明示的なroom-detection roleを収集して `Room` entity のスポーン/削除・`RoomTileLookup` 更新、dirty マーキング、オーバーレイ同期を行う。
- `crates/hw_world::world_replace`: world replacement時にruntime-onlyのRoom root／overlayを除去し、lookup・detection・validation resourceを初期化するidempotent resetを所有する。
- `crates/bevy_app/src/plugins/logic.rs` / `plugins/visual.rs`: `hw_world`のsystemとload reset hookを直接登録し、cross-domain orderingだけを所有する。旧`systems/room/*` shellは存在しない。

## 2. Room の成立条件

以下を **すべて** 満たす連続床タイルの集合が Room として認識されます。

| 条件 | 詳細 |
|:---|:---|
| 内部タイルがすべて完成床 | `BuildingType::Floor` かつ `is_provisional == false` |
| 外周がすべて壁または扉 | 完成 `BuildingType::Wall`（`is_provisional == false`）または `BuildingType::Door` |
| ドアが 1 つ以上存在 | 外周の中に `BuildingType::Door` が 1 個以上 |
| タイル数が上限以下 | `ROOM_MAX_TILES`（400）以下 |

> **壁の仮設状態について**: `is_provisional == true` の壁は境界として認めません。壁が完全完成（`CoatWall` 済み）してはじめて Room が成立します。

## 3. コンポーネントとリソース

### コンポーネント

| 型 | 定義クレート | 説明 |
|:---|:---|:---|
| `Room` | `hw_world` | 検出された Room エンティティ。`tiles`, `wall_tiles`, `door_tiles`, `bounds`, `tile_count` を保持 |
| `RoomBounds` | `hw_world` | Room の最小/最大グリッド座標（min_x, min_y, max_x, max_y） |
| `RoomOverlayTile` | `hw_world` | 床と外周壁の境界に置く細いline spriteのmarker。`Room` エンティティの子として生成 |

### リソース

| 型 | 定義クレート | 説明 |
|:---|:---|:---|
| `RoomDetectionState` | `hw_world` | dirty タイルセットとクールダウンタイマー |
| `RoomTileLookup` | `hw_world` | `(i32, i32)` グリッド座標 → `Entity`（Room エンティティ）の逆引きマップ |
| `RoomValidationState` | `hw_world` | 定期検証タイマー |

## 4. 検出アルゴリズム

### 4.1 入力データの構築（`hw_world::room_systems` → `build_detection_input`）

`hw_world` の `detect_rooms_system` / `validate_rooms_system` は `Building + Transform` クエリを全走査し、各建物を `RoomDetectionBuildingTile` に変換して `hw_world::room_detection::build_detection_input(...)` に渡します。

adapterは`BuildingType::room_detection_role(is_provisional)`で各建物を分類し、core側で以下の3セットと
floor invalidator集合を構築します。経路探索や`WorldMap.buildings`の配置占有をRoom判定へ流用しません。

```
floor_tiles      : RoomDetectionRole::Floor
solid_wall_tiles : RoomDetectionRole::SolidBoundary
door_tiles       : RoomDetectionRole::DoorBoundary
floor_invalidators: SolidBoundary / DoorBoundary / FloorInvalidator
```

`BuildingType::Floor`はFloor、完成WallはSolidBoundary、DoorはDoorBoundary、仮設WallとBridgeは
FloorInvalidatorになる。Tank / MudMixer / RestArea / SandPile / BonePile / WheelbarrowParking / SoulSpa /
OutdoorLampはInteriorFixtureであり、同じセルを配置占有したり移動を塞いだりしても、その下の完成Floorを
Room内部から除外しない。この分類をpathfindingの`blocks_movement()`から推測しない。

### 4.2 Flood-fill による Room 候補の抽出

1. 全 `floor_tiles` を未訪問セットとして初期化
2. 未訪問セットからシードを 1 つ取り出し、4 近傍 BFS を実施
3. 各タイルの近傍が「他の床 or 完成壁 or 扉 or マップ内」以外なら Room 不成立（`is_valid = false`）
4. `is_valid == true` かつ `boundary_doors.len() > 0` の場合のみ `DetectedRoom` を生成

### 4.3 Room エンティティの同期

```
既存 Room エンティティをすべて despawn（Bevy 0.19: 子の RoomOverlayTile も自動 despawn）
↓
`DetectedRoom` を `Room` component に変換して新規 Room エンティティをスポーン（Transform::default() を必ず含める）
↓
RoomTileLookup を再構築
```

> **`Transform` が必須な理由**:  
> `Room` エンティティは `RoomOverlayTile` を `with_children` で子として保持します。Bevy 0.19 のトランスフォーム伝播は親の `GlobalTransform`（`Transform` から自動挿入）を必要とします。`Transform` を省略すると、すべての子オーバーレイタイルが `GlobalTransform::IDENTITY`（ワールド原点）で固定されてしまい、実際の部屋位置にオーバーレイが表示されません。

## 5. dirty タイル追跡

Room 再検出は「dirty タイルが存在する」かつ「クールダウンが完了した」場合にのみ実行されます。

### トリガー（`mark_room_dirty_from_building_changes_system`）

- `Added<Building>` / `Changed<Building>` / `Changed<Transform>` → 変化したタイル ± 1 近傍を dirty 化
- `Added<Door>` / `Changed<Door>` / `Changed<Transform>` → 同上

### トリガー（Observer: `on_building_*` / `on_door_*`）

- `Add` / `Remove` Observer が Building / Door の追加・削除タイルを dirty 化する
- 削除系の変化は `On<Remove, Building>` / `On<Remove, Door>` で補足する

## 6. 定期検証（`validate_rooms_system`）

2 秒ごとに既存の `Room` エンティティを再評価します。

- 現在の建物状態に対して `hw_world::room_detection::room_is_valid_against_input(&room.tiles, ...)` を実行
- 不正な Room は despawn → dirty マーキング → 再検出へ戻す
- 正常な Room の `RoomTileLookup` を再構築

## 7. 視覚境界線（`sync_room_overlay_tiles_system`）

`Added<Room>` または `Changed<Room>` で起動し、各床タイルの4近傍に外周壁がある辺だけ
`RoomOverlayTile` line spriteを生成します。隣接2辺のcornerは線を延長して隙間を埋めます。

- `Z_ROOM_OVERLAY`レイヤーに描画
- 色と太さ: `ROOM_BORDER_COLOR` / `ROOM_BORDER_THICKNESS`
- Room内部の床面全体を塗るspriteは生成しない
- Bevy 0.19 では親 Room を `try_despawn()` するだけで子 RoomOverlayTile も自動 despawn されます

## 8. システム実行順序

```
GameSystemSet::Logic（Logic ループ内）
 └─ mark_room_dirty_from_building_changes_system
     → validate_rooms_system
         → detect_rooms_system
（Building / Door の Add / Remove は Observer が dirty 化）
（room systems は dream_tree_planting_system の後に実行）

GameSystemSet::Visual（Visual ループ内）
 └─ sync_room_overlay_tiles_system
```

### world replacement

`Room`、`RoomOverlayTile`、`RoomTileLookup`、検出・検証timerは保存しないruntime stateである。
normal load、rollback、recovery-only replaceはpersisted entityを書き換える前に同じ
`hw_world::reset_for_world_replace` hookを実行し、Room rootと独立して残ったoverlayをどちらもdespawnして
3 resourceをdefaultへ戻す。新worldのWall / Door / Floorを入力に、次のLogicでRoomとlookupを
再検出する。旧worldのRoom Entity IDをload後へ持ち越さない。

## 9. 実装上の注意点

### `Room` エンティティには必ず `Transform::default()` を付与すること

`RoomOverlayTile` は `Room` の子エンティティです。Bevy 0.19 のトランスフォーム伝播（`propagate_parent_transforms`）は、親の `GlobalTransform` が存在しない場合に子をスキップします。`Transform` が欠けていると全オーバーレイタイルがワールド原点 (0, 0) に描画されます。

### 配置占有とRoom roleを混同しない

- 完成した`BuildingType::Floor`エンティティは`WorldMap.buildings`へ登録しない。
- 完成Wall / Door、仮設Wall、Bridgeは同じセルのFloorをRoom内部から除外する。
- Plant / TemporaryのInteriorFixtureは`WorldMap.buildings`のownerになっても完成Floorを有効なRoom tileとして残す。
- 新しい`BuildingType`を追加したら`room_detection_role`へ明示分類し、InteriorFixture上の床とboundary重複の回帰を追加する。

### 仮設壁は Room の境界として認めない

`is_provisional == true` の壁は `solid_wall_tiles` に含まれません。Flood-fill 中にその位置を踏むと `is_valid = false` になり Room 不成立となります。

## 10. 定数（`crates/hw_core/src/constants/building.rs`）

| 定数 | 値 | 説明 |
|:---|:---|:---|
| `ROOM_MAX_TILES` | 400 | Room として認められる最大タイル数 |
| `ROOM_DETECTION_COOLDOWN_SECS` | 0.5 | dirty 収集後に再検出を実行する最小間隔（秒） |
| `ROOM_VALIDATION_INTERVAL_SECS` | 2.0 | 既存 Room を再検証する周期（秒） |

## 11. 関連ファイル

| ファイル | 役割 |
|:---|:---|
| `crates/hw_world/src/room_detection.rs` | room detection core。`build_detection_input`・Flood-fill・validator・`RoomBounds` |
| `crates/hw_world/src/room_systems.rs` | ECS adapter 層。`detect_rooms_system` / `validate_rooms_system` / `mark_room_dirty_from_building_changes_system` / dirty mark Observer 群 / `sync_room_overlay_tiles_system` の実装本体 |
| `crates/hw_world/src/world_replace.rs` | load / rollback / recovery-only共通のRoom runtime reset |
| `crates/hw_jobs/src/model.rs` | `BuildingType::room_detection_role`と`RoomDetectionRole`の分類正本 |
| `crates/bevy_app/src/plugins/logic.rs` | Room 検出システムの登録 |
| `crates/bevy_app/src/plugins/visual.rs` | Room ビジュアルシステムの登録 |
| `crates/hw_core/src/constants/building.rs` | Room 関連定数 |
