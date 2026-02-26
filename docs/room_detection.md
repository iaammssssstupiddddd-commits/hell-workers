# Room 検出システム (Room Detection System)

このドキュメントでは、壁・扉・床で囲まれた空間を「Room」として自動検出するシステムについて説明します。

## 1. 概要

Room 検出システムは、完成した壁・扉・床で構成された密閉空間を `Room` エンティティとして自動認識します。
検出された Room は半透明オーバーレイで視覚的にフィードバックされ、将来の Room 系ゲームプレイ機能（温度・モラル・部屋品質バフ等）の基盤データを提供します。

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

| 型 | 説明 |
|:---|:---|
| `Room` | 検出された Room エンティティ。`tiles`, `wall_tiles`, `door_tiles`, `bounds`, `tile_count` を保持 |
| `RoomBounds` | Room の最小/最大グリッド座標（min_x, min_y, max_x, max_y） |
| `RoomOverlayTile` | 各床タイルに対応する半透明オーバーレイスプライト。`Room` エンティティの子として生成 |

### リソース

| 型 | 説明 |
|:---|:---|
| `RoomDetectionState` | dirty タイルセット・クールダウンタイマー・前フレームの `WorldMap.buildings` スナップショット |
| `RoomTileLookup` | `(i32, i32)` グリッド座標 → `Entity`（Room エンティティ）の逆引きマップ |
| `RoomValidationState` | 定期検証タイマー |

## 4. 検出アルゴリズム

### 4.1 入力データの構築（`build_detection_input`）

`Building + Transform` クエリを全走査し、以下の 3 セットを構築します。

```
floor_tiles      : BuildingType::Floor かつ world_map.buildings に未登録のタイル
solid_wall_tiles : BuildingType::Wall  かつ is_provisional == false
door_tiles       : BuildingType::Door
```

> **なぜ `world_map.buildings` をチェックするか**:  
> 完成 Floor タイルのグリッドに壁や別の建物が存在する場合（例: 壁を床の上に建てた位置）、その Floor エンティティは床として扱わず除外します。
> 完成 Floor タイル自体は `world_map.buildings` に登録されないため、内部床タイルは通常このチェックを通過します。

### 4.2 Flood-fill による Room 候補の抽出

1. 全 `floor_tiles` を未訪問セットとして初期化
2. 未訪問セットからシードを 1 つ取り出し、4 近傍 BFS を実施
3. 各タイルの近傍が「他の床 or 完成壁 or 扉 or マップ内」以外なら Room 不成立（`is_valid = false`）
4. `is_valid == true` かつ `boundary_doors.len() > 0` の場合のみ `RoomCandidate` を生成

### 4.3 Room エンティティの同期

```
既存 Room エンティティをすべて despawn（Bevy 0.18: 子の RoomOverlayTile も自動 despawn）
↓
新規 Room エンティティをスポーン（Transform::default() を必ず含める）
↓
RoomTileLookup を再構築
```

> **`Transform` が必須な理由**:  
> `Room` エンティティは `RoomOverlayTile` を `with_children` で子として保持します。Bevy 0.18 のトランスフォーム伝播は親の `GlobalTransform`（`Transform` から自動挿入）を必要とします。`Transform` を省略すると、すべての子オーバーレイタイルが `GlobalTransform::IDENTITY`（ワールド原点）で固定されてしまい、実際の部屋位置にオーバーレイが表示されません。

## 5. dirty タイル追跡

Room 再検出は「dirty タイルが存在する」かつ「クールダウンが完了した」場合にのみ実行されます。

### トリガー（`mark_room_dirty_from_building_changes_system`）

- `Added<Building>` / `Changed<Building>` / `Changed<Transform>` → 変化したタイル ± 1 近傍を dirty 化
- `Added<Door>` / `Changed<Door>` / `Changed<Transform>` → 同上

### トリガー（`mark_room_dirty_from_world_map_diff_system`）

- `WorldMap.buildings` 差分（エンティティの追加・削除・置換）を前フレームスナップショットと比較し、変化したグリッドを dirty 化
- 削除系の変化（壁破壊・サイト cancel）をここで補足する

## 6. 定期検証（`validate_rooms_system`）

2 秒ごとに既存の `Room` エンティティを再評価します。

- 現在の建物状態に対して `room_is_valid_against_input()` を実行
- 不正な Room は despawn → dirty マーキング → 再検出へ戻す
- 正常な Room の `RoomTileLookup` を再構築

## 7. 視覚オーバーレイ（`sync_room_overlay_tiles_system`）

`Added<Room>` または `Changed<Room>` で起動し、Room の各床タイルに対して `RoomOverlayTile` スプライトを生成します。

- `Z_ROOM_OVERLAY`（= 0.08）レイヤーに描画（床より上、拾得アイテムより下）
- 色: `ROOM_OVERLAY_COLOR`（半透明）
- Bevy 0.18 では親 Room を `try_despawn()` するだけで子 RoomOverlayTile も自動 despawn されます

## 8. システム実行順序

```
GameSystemSet::Logic（Logic ループ内）
 └─ mark_room_dirty_from_building_changes_system
     → mark_room_dirty_from_world_map_diff_system
         → validate_rooms_system
             → detect_rooms_system
（room systems は dream_tree_planting_system の後に実行）

GameSystemSet::Visual（Visual ループ内）
 └─ sync_room_overlay_tiles_system
```

## 9. 実装上の注意点

### `Room` エンティティには必ず `Transform::default()` を付与すること

`RoomOverlayTile` は `Room` の子エンティティです。Bevy 0.18 のトランスフォーム伝播（`propagate_parent_transforms`）は、親の `GlobalTransform` が存在しない場合に子をスキップします。`Transform` が欠けていると全オーバーレイタイルがワールド原点 (0, 0) に描画されます。

### `WorldMap.buildings` と床タイルの関係

- 完成した `BuildingType::Floor` エンティティは `world_map.buildings` に **登録されません**
- 床タイルを建設中の `FloorConstructionSite` や、その上に建てられた壁などが `world_map.buildings` に登録されます
- 床建設が完了またはキャンセルされた際は `world_map.buildings.remove(&(gx, gy))` を呼んで stale エントリを消去してください

### 仮設壁は Room の境界として認めない

`is_provisional == true` の壁は `solid_wall_tiles` に含まれません。Flood-fill 中にその位置を踏むと `is_valid = false` になり Room 不成立となります。

## 10. 定数（`src/constants/building.rs`）

| 定数 | 値 | 説明 |
|:---|:---|:---|
| `ROOM_MAX_TILES` | 400 | Room として認められる最大タイル数 |
| `ROOM_DETECTION_COOLDOWN_SECS` | 0.5 | dirty 収集後に再検出を実行する最小間隔（秒） |
| `ROOM_VALIDATION_INTERVAL_SECS` | 2.0 | 既存 Room を再検証する周期（秒） |

## 11. 関連ファイル

| ファイル | 役割 |
|:---|:---|
| `src/systems/room/detection.rs` | `build_detection_input`・Flood-fill・`detect_rooms_system` |
| `src/systems/room/dirty_mark.rs` | Building/Door 変化と WorldMap 差分からの dirty マーキング |
| `src/systems/room/validation.rs` | 定期検証システム |
| `src/systems/room/visual.rs` | `RoomOverlayTile` 同期システム |
| `src/systems/room/components.rs` | `Room`, `RoomBounds`, `RoomOverlayTile` 定義 |
| `src/systems/room/resources.rs` | `RoomDetectionState`, `RoomTileLookup`, `RoomValidationState` 定義 |
| `src/plugins/logic.rs` | Room 検出システムの登録 |
| `src/plugins/visual.rs` | Room ビジュアルシステムの登録 |
| `src/constants/building.rs` | Room 関連定数 |
