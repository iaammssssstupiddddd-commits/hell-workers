# hw_ui — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- ゲーム UI（パネル・リスト・ダイアログ・ツールチップ・サブメニュー）のセットアップと入力処理
- `UiAssets` trait の定義（`GameAssets: UiAssets` の実装は bevy_app 側）
- `HwUiPlugin` によるシステム登録（ゲーム固有の ECS クエリを持たない UI システムのみ）
- `UiIntent` メッセージ型の定義（ユーザー操作意図の型安全な表現）

## 禁止事項（AI がやってはいけないこと）

- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約）
- **`DamnedSoul` / `Familiar` / `AssignedTask` 等ゲームエンティティを直接クエリするシステムをここに書かない**（ゲーム固有 ViewModel 構築は bevy_app 側）
- **`Res<GameAssets>` を引数に取るシステムをここに書かない**（Bevy は `Res<dyn Trait>` 不可。GameAssets への依存は bevy_app 側で解決）
- **ゲーム状態遷移 (`PlayMode`) の変更をここに書かない**（ルートクレートの責務）
- **`#[allow(dead_code)]` を使用しない**
- **Bevy 0.14 以前の API を推測で使わない**

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- leaf crate：`bevy`, `hw_core`, `hw_jobs`, `hw_logistics` に依存
- `bevy_app` への逆依存は **完全禁止**
- UI コンポーネントを定義するが、ゲームロジックを持たない
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
bevy         ✓
hw_core      ✓
hw_jobs      ✓
hw_logistics ✓

# 禁止
bevy_app       ✗
hw_soul_ai     ✗
hw_familiar_ai ✗
hw_spatial     ✗
hw_visual      ✗
hw_world       ✗
hw_energy      ✗
```

## plugin / system 登録責務

- **`HwUiPlugin`** がゲーム固有クエリを持たない UI システムのみを登録する
- ゲーム固有クエリを必要とする UI システム（entity_list 更新等）は `bevy_app/plugins/input.rs` 等が登録する

## UiAssets 抽象化パターン

新しいアセット（フォント・アイコン等）が必要になった場合:
1. `setup/mod.rs` の `UiAssets` trait にメソッドを追加
2. `bevy_app/src/entities/game_assets.rs` の `impl UiAssets for GameAssets` に実装を追加
3. このクレートに `GameAssets` への直接依存は追加しない

## テキスト入力・スクロール UI の方針（Bevy 0.19 標準 widget 利用）

- スクロール可能な UI コンテナには自前実装を追加せず、`bevy::ui_widgets::{ScrollArea, Scrollbar, ScrollbarThumb, ControlOrientation}` を使う。
  - `ScrollArea` は `#[require(ScrollPosition)]` 付きなので `ScrollPosition` の手動 insert は不要。
  - `UiWidgetsPlugins`（`ScrollAreaPlugin` / `ScrollbarPlugin` / `EditableTextInputPlugin` 等）は `bevy` の `ui` feature 経由で `DefaultPlugins` に自動登録済み。個別に plugin 登録しない。
  - 参考実装: `crates/hw_ui/src/setup/entity_list.rs` の未所属 Soul リスト（`UnassignedSoulContent`）。
- テキスト入力は自前 widget を作らず `bevy::text::EditableText` + `crates/hw_ui/src/widgets/text_field.rs` の `spawn_text_field` を使う。
  - `TextFieldRole` は **root ではなく `EditableText` エンティティ**に付与（observer フィルタ用）
  - Enter/Escape は `crates/hw_ui/src/interaction/text_field.rs` の observer で処理（`EditableText` に `ValueChange` はない）
  - `String` を含む確定イベント（リネーム等）は `TextInputIntent`（non-`Copy`）で送る。`UiIntent` / `MenuAction` は `Copy` 維持
  - フォーカス中のゲーム keybind 抑止は `UiInputState::text_input_focused` / `text_input_consumed_keyboard` + `text_input_blocks_keybinds()`
  - `text_input_consumed_keyboard` のリセットは `InputFocusSystems::Dispatch` より前、Enter/Escape の適用は dispatch 後。Escape でフォーカス解除した同フレームもゲーム側 keybind に伝播させない
  - 検索などのライブ同期は `EditableTextSystems` 後に `EditableText` 値を読む。Escape クリア時は state だけでなく `EditableText` 本体も空にする
  - クリップボード連携は workspace `bevy` features の `"system_clipboard"` で有効化（`EditableTextInputPlugin` 内蔵の Ctrl+C/V 等）
- スクロール入力ブロック（`UiInputBlocker` + `RelativeCursorPosition`）はスクロール実装方式に関わらず、pointer-over 判定用として維持する。

### text_field 追加チェックリスト

1. `widgets/text_field.rs` の `TextFieldRole` に用途を追加（必要なら）
2. `spawn_text_field` / `spawn_text_field_on_entity` で imperative spawn（BSN は root のみ可）
3. `interaction/text_field.rs` に Enter/Escape / ライブ sync を追加
4. ゲーム状態を変える確定処理は `TextInputIntent` または `bevy_app` handler へ委譲（hw_ui から `SoulIdentity` 等をクエリしない）
5. `ButtonInput<KeyCode>` 系ショートカットに `text_input_blocks_keybinds` ガードを追加

## 設定画面 UI（Slider / Checkbox / BSN）

- 設定 UI は `crates/hw_ui/src/setup/settings_panel.rs`。ルート entity は `bsn! { SettingsPanel }` + 子は imperative spawn（`FontSource` / `MenuButton` は BSN 制約上 imperative）。
- Slider / Checkbox は `bevy::ui_widgets` headless widget。`SettingsPlugin` で `slider_self_update` / `checkbox_self_update` を **各 1 回** `add_observer` する。spawn 時 `.observe(...)` は使わない。
- `ValueChange` → `UiIntent` の observer（bevy_app）は **`SettingsSliderMarker` / `SettingsCheckboxMarker` で必ずフィルタ**する。
- `GameSettings` への write は bevy_app の intent handler のみ。hw_ui から `ResMut<GameSettings>` 禁止。
- 詳細: [docs/settings.md](../../docs/settings.md)

### 設定項目追加チェックリスト

1. `hw_core::GameSettings` にフィールド + `Default`
2. `bevy_app/systems/settings/persistence.rs` の `GameSettingsFile` + `From`
3. `apply_settings_system` に反映（`Time<Virtual>` は触らない）
4. `settings_panel.rs` に UI 行
5. `SettingsField` + marker
6. `handlers/settings.rs` + `intents.rs`
7. 手動 QA + RON 再起動確認

## BuildingType メニュー追加時のルール

新 `BuildingType` を建設メニューに追加する場合:
1. `setup/submenus.rs` の該当カテゴリの `architect_building_specs()` に `MenuEntrySpec` を追加
2. `panels/task_list/presenter.rs` に表示ラベルを追加

## Pause メニュー / 確認ダイアログ追加時のルール

セーブ/ロード等、ゲーム状態を変える UI ボタンは以下の流れに従う:

1. **`hw_ui/src/intents.rs`**: `UiIntent` バリアントを追加（hw_ui はゲーム状態を直接書かない）
2. **`hw_ui/src/components.rs`**: パネル marker（例: `PauseMenu`, `LoadConfirmDialog`）を追加
3. **`hw_ui/src/setup/`**: `MenuButton(MenuAction::…)` 付きボタンを spawn（`dialogs.rs` / `pause_menu.rs` 参照）
4. **`bevy_app/.../menu_actions.rs`**: `MenuAction` → `UiIntent` 変換
5. **`bevy_app/.../handlers/`**: intent 実処理（例: `save_game.rs` → `SaveLoadState`）
6. **`bevy_app/.../intent_handler.rs`**: 新 intent の match 分岐

`bsn!` マクロは **ルート entity のみ採用**（`settings_panel.rs` の「設定画面 UI」節参照）。子ツリーやボタンは **`commands.spawn` + `MenuButton`** で統一する（`FontSource` / `MenuButton` は BSN 制約上 imperative spawn が必要）。

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/building.md](../../docs/building.md)（BuildingType メニュー追加時）
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md)（Cargo.toml 変更時）
- `crates/hw_ui/_rules.md`（このファイル）

## 検証方法

```bash
# コンパイル確認（必須）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

## 参照ドキュメント

- [docs/building.md](../../docs/building.md): BuildingType 一覧・カテゴリ
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
