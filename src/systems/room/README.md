# room — 部屋検出システム

## 役割

壁・ドアで囲まれた閉じた領域を「部屋」として検出し、バリデーションとビジュアル同期を行うシステム群。
建物の変化（壁追加・ドア設置等）を検知して部屋を再計算する。

検出アルゴリズム本体は `crates/hw_world::room_detection` にあり、このディレクトリは ECS adapter / dirty tracking / visual の root shell を担当する。

## ファイル一覧

| ファイル | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `components.rs` | `Room`, `RoomOverlayTile` コンポーネント。`RoomBounds` は `hw_world` から re-export |
| `detection.rs` | `detect_rooms_system` — `Building` Query を `RoomDetectionBuildingTile` に変換し、`DetectedRoom` を ECS に反映する adapter |
| `dirty_mark.rs` | 建物変化イベントで「ダーティ」フラグを立てる Observer群 |
| `validation.rs` | `validate_rooms_system` — 既存 `Room` を `hw_world` validator で再評価する adapter |
| `visual.rs` | `sync_room_overlay_tiles_system` — 部屋オーバーレイの同期 |
| `resources.rs` | `RoomDetectionState`, `RoomTileLookup`, `RoomValidationState` リソース |

## 更新トリガー

以下の Observer が `dirty_mark.rs` に登録されており、変化時に再検出をスケジュールする:

- `on_building_added` / `on_building_removed`
- `on_door_added` / `on_door_removed`
- `mark_room_dirty_from_building_changes_system`

## 境界メモ

- `Room` entity の spawn/despawn と `RoomTileLookup` 再構築は root 側責務。
- `Transform::default()` を Room 親に付ける契約も root 側で維持する。
- `build_detection_input` / flood-fill / `room_is_valid_against_input` は `hw_world::room_detection` 側で管理する。
