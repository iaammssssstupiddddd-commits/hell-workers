# room — 部屋検出システム

## 役割

壁・ドアで囲まれた閉じた領域を「部屋」として検出し、バリデーションとビジュアル同期を行うシステム群。
建物の変化（壁追加・ドア設置等）を検知して部屋を再計算する。

## ファイル一覧

| ファイル | 内容 |
|---|---|
| `mod.rs` | 公開 API |
| `components.rs` | `Room`, `RoomBounds`, `RoomOverlayTile` コンポーネント |
| `detection.rs` | `detect_rooms_system` — 閉囲域の検出アルゴリズム |
| `dirty_mark.rs` | 建物変化イベントで「ダーティ」フラグを立てる Observer群 |
| `validation.rs` | `validate_rooms_system` — 部屋の有効性チェック |
| `visual.rs` | `sync_room_overlay_tiles_system` — 部屋オーバーレイの同期 |
| `resources.rs` | `RoomDetectionState`, `RoomTileLookup`, `RoomValidationState` リソース |

## 更新トリガー

以下の Observer が `dirty_mark.rs` に登録されており、変化時に再検出をスケジュールする:

- `on_building_added` / `on_building_removed`
- `on_door_added` / `on_door_removed`
- `mark_room_dirty_from_building_changes_system`
