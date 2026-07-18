# ゲーム設定（GameSettings）

`GameSettings` Resource と `settings/settings.ron` による永続化で、UI スケール・カメラ・デフォルト速度・デバッグ表示を管理する。

## 保存先

- パス: 実行ディレクトリ直下 `settings/settings.ron`
- 書込: 設定モーダルを Close した時、`AppExit` 検知時
- 読込: 起動時 `Startup`（`SettingsPlugin`）

## 設定項目

| フィールド | UI ラベル | 反映先 |
| --- | --- | --- |
| `ui_scale` | UI Scale | `UiScale` |
| `camera_pan_speed` | Camera Pan Speed | `PanCamera.pan_speed`（`MainCamera`） |
| `camera_mouse_pan_enabled` | Mouse Drag Pan | `PanCamera.mouse_pan_settings.enabled` |
| `default_time_speed` | Default Game Speed | **起動時のみ** `Time<Virtual>` |
| `debug_gizmos_enabled` | Debug Gizmos | `DebugVisible` + `GizmoConfigStore`（F12 と同期） |
| `fps_display_enabled` | Show FPS | DevPanel 内 `UiSlot::FpsText` の `Visibility` |

## 重要な制約

- **`apply_settings_system` は `Time<Virtual>` を触らない**（D9）。`default_time_speed` は Startup の `load_settings_system` で一度だけ適用する。設定 UI の Default Speed ボタンは次回起動用の値のみ更新し、現在の pause/速度は変えない。
- **`PanCamera.enabled`** は通常 UI hover、Modal/Pause capture、text input focus、task area の左ドラッグ中に使う一時 guard 専用。area gesture は押下から release frame まで claim を維持し、残る capture / text focus / pointer claim がなくなった次 frame に既存設定どおり復帰する。永続化するのは `mouse_pan_settings.enabled` と `pan_speed` のみ。
- **crate 境界**: `GameSettings` 型は `hw_core`。ロード/保存/intent 処理は `bevy_app`。UI spawn は `hw_ui`（`SettingsPanelInitial` DTO 経由）。
- **widget の見た目同期**: headless widget のため見た目は自前。スライダー thumb は `sync_settings_slider_thumbs_system`、チェックマークは `sync_settings_checkmarks_system`（`Checked` の有無 → `Display`、いずれも `hw_ui/src/interaction/settings.rs`）が毎フレーム同期する。F12 は `debug_toggle_system` が `GameSettings` に加えて Debug Gizmos チェックボックスの `Checked` も直接更新する。
- **`settings/` は gitignore 済み**（ユーザーローカルファイル。`saves/` と同扱い）。

## 設定画面の開き方

- ボトムバー **Settings** ボタン
- ポーズメニュー（Save/Load 下）の **Settings** 行
- Esc: 最前面 overlay を `LoadConfirm → Settings → Pause → OperationDialog` の優先順で 1 つだけ閉じる。Settings close 自体は背景 active mode を cancel しない

## UI スケール

- 主経路: `UiScale`（スライダー 0.85〜1.25）
- 主要パネル文字: `FontSize::Rem(px / 20.0)`（`RemSize` デフォルト 20.0 固定）
- `RemSize` 同期は第 2 段階（目視 QA 後に必要なら `apply.rs` へ追加）

## 関連コード

- `crates/hw_core/src/settings.rs` — `GameSettings` 型
- `crates/bevy_app/src/systems/settings/` — 永続化・反映・observer
- `crates/hw_ui/src/setup/settings_panel.rs` — 設定 UI（BSN ルート + Slider/Checkbox）

## 項目追加手順

`hw_ui/_rules.md` の設定項目追加チェックリストを参照。
