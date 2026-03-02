# Plant ホバーアクションボタン（プレースホルダー）実装計画

## Context
Plant建物（Tank / MudMixer）にホバーした際に「Move」ボタンを表示する機能の UI 部分のみをプレースホルダーとして実装する。クリック時は `info!` ログのみ。タスクシステムや PlayMode の変更は行わない。

## 方式: 永続エンティティ（Show/Hide）
- Startup時に非表示状態でスポーン（ツールチップと同じパターン）
- ホバー対象がPlant建物の場合に `Display::Flex` で表示、それ以外で `Display::None`
- 毎フレーム `world_to_viewport` で建物のスクリーン座標を計算し `left`/`top` を更新
- Spawn/Despawnを繰り返すよりシンプルで、ボタンホバー時のkeep-aliveも扱いやすい

## エッジケース: ボタン自体のホバー
カーソルが建物 → ボタンへ移動すると:
1. `UiInputBlocker` により `pointer_over_ui = true`
2. `HoveredEntity` が `None` になる
3. **対策**: ボタンの `Interaction` が `Hovered | Pressed` なら前のターゲットを保持

## 実装ステップ

### Step 1: コンポーネント追加 (`src/interface/ui/components.rs`)

- `HoverActionOverlay` マーカーコンポーネント追加
- `MenuAction::MovePlantBuilding(Entity)` バリアント追加

```rust
#[derive(Component)]
pub struct HoverActionOverlay {
    pub target: Option<Entity>,
}
```

### Step 2: スポーン処理追加 (`src/interface/ui/setup/panels.rs`)

`spawn_panels()` 内で `spawn_hover_tooltip()` の後に `spawn_hover_action_overlay()` を呼ぶ。
`overlay_parent` の子として、非表示のボタンをスポーン:

- `Button` + `Node { position_type: Absolute, display: None }` + `HoverActionOverlay`
- `MenuButton(MenuAction::MovePlantBuilding(Entity::PLACEHOLDER))` — ダミー値、毎フレーム更新
- `UiInputBlocker` + `ZIndex(30)`
- 子として `Text::new("Move")`

### Step 3: 更新システム (`src/interface/ui/interaction/hover_action.rs` 新規)

**`hover_action_button_system`** — Update で毎フレーム実行:

```
パラメータ:
  hovered: Res<HoveredEntity>
  q_buildings: Query<(&Building, &GlobalTransform)>
  q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>
  q_overlay: Query<(&mut HoverActionOverlay, &mut Node, &mut MenuButton, &Interaction)>

ロジック:
  1. hovered.0 が Some かつ Building.kind.category() == Plant → new_target = Some(entity)
  2. new_target が None かつ overlay.Interaction が Hovered/Pressed → effective_target = overlay.target (keep-alive)
  3. effective_target が None → display = None
  4. effective_target が Some →
     - MenuButton.0 = MovePlantBuilding(target)
     - world_to_viewport で建物位置のスクリーン座標取得
     - node.left/top を設定（建物の上方にオフセット）
     - display = Flex
```

### Step 4: アクション処理 (`src/interface/ui/interaction/menu_actions.rs`)

`handle_pressed_action` の match に追加:
```rust
MenuAction::MovePlantBuilding(entity) => {
    info!("[Placeholder] Move building requested for {:?}", entity);
}
```

### Step 5: モジュール登録

- `src/interface/ui/interaction/mod.rs`: `mod hover_action;` + `pub use`
- `src/interface/ui/plugins/core.rs`: チェインに `hover_action_button_system` を追加（`door_lock_action_system` の後）

## 変更ファイル一覧

| ファイル | 変更 |
|---|---|
| `src/interface/ui/components.rs` | `HoverActionOverlay` コンポーネント、`MenuAction::MovePlantBuilding` 追加 |
| `src/interface/ui/setup/panels.rs` | `spawn_hover_action_overlay()` 関数追加、`spawn_panels()` から呼出 |
| `src/interface/ui/interaction/hover_action.rs` | **新規** — `hover_action_button_system` |
| `src/interface/ui/interaction/mod.rs` | `mod hover_action;` + pub use 追加 |
| `src/interface/ui/interaction/menu_actions.rs` | `MovePlantBuilding` アーム追加 |
| `src/interface/ui/plugins/core.rs` | システム登録（チェイン内に追加） |

## 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo cargo check
```

ゲーム起動後:
1. Tank または MudMixer を建設
2. 完成した建物にカーソルを合わせる → 「Move」ボタンが建物上方に表示される
3. ボタンをクリック → コンソールに `[Placeholder] Move building requested for ...` が出力される
4. カーソルを外す → ボタンが消える
5. ツールチップは従来通り表示される（干渉なし）
