# ゲーム設定（GameSettings）

`GameSettings` Resource と `settings/settings.ron` による永続化で、UI スケール・カメラ・デフォルト速度・電力配電・オートセーブ・デバッグ表示を管理する。

## 保存先

- パス: 実行ディレクトリ直下 `settings/settings.ron`
- 書込: 設定モーダルを Close した時、`AppExit` 検知時
- 読込: 起動時 `Startup`（`SettingsPlugin`）
- native acceptance / performance はStartup前に独立した`SettingsStorageRoot`を注入する。通常の
  `settings/settings.ron`と同じAPIを使うが、ユーザー設定を読書きしない。

設定画面を閉じた際の書込失敗は、セッションへの適用と次回起動用の保存を区別して重要通知を出す。
設定画面にも未保存状態を表示し、通知履歴の「現在の設定を再保存」または設定画面を閉じる操作で再試行する。
再試行は現在の `GameSettings` を読み、失敗時のコピーを復元しない。保存試行ticketを照合し、
成功済み・新しい失敗に置き換わった古い通知の操作は受理しない。再保存成功時には成功通知を出す。
通知の操作ボタンは履歴だけに表示し、toastは引き続き入力透過。終了時の保存失敗は既存のログ出力を維持する。

## 設定項目

| フィールド | UI ラベル | 反映先 |
| --- | --- | --- |
| `ui_scale` | UI倍率（即時反映） | `UiScale` |
| `camera_pan_speed` | カメラ移動速度（即時反映） | `PanCamera.pan_speed`（`MainCamera`） |
| `camera_mouse_pan_enabled` | Mouse Drag Pan | project-owned world pointer gesture（`PanCamera.mouse_pan_settings.enabled`は常時false） |
| `default_time_speed` | 起動時のゲーム速度（次回起動時に反映） | **起動時のみ** `Time<Virtual>` |
| `debug_gizmos_enabled` | Debug Gizmos | `DebugVisible` + 開発パネルと診断用 `GizmoConfigStore`（F12 と同期）。プレイヤー用地図レイヤーは独立 |
| `fps_display_enabled` | Show FPS | 左上の時計に隣接する `UiSlot::FpsText` の `Visibility`。開発パネルの表示と独立 |
| `power_priority_enabled` | Power priority allocation | `true`: priority strict prefix / `false`: Legacy all-or-none |
| `autosave_enabled` | Autosave | 初期値 `false`（性能 gate とは独立した product 判断） |
| `autosave_interval_minutes` | 自動保存間隔 | `5` / `10` / `20` / `30`（active play の実時間） |
| `notification_duration_seconds` | 通知の表示時間 | 4 / 8 / 12秒。既定4秒。新しい通知から反映 |
| `autosave_generations` | 自動保存の保持数 | `1..=5`（縮小しても既存 file は削除せず load-only 保持） |

スライダーの下に適用済みの値を常時表示する。UI倍率は%、カメラ速度はワールド単位/秒、
自動保存間隔は分、保持数は世代。表示は `GameSettings` から取得し、自動保存は実処理と同じ
正規化値を使う。通知表示時間は秒で表示する。UI倍率とカメラ速度は即時反映、起動時速度は次回起動用。
自動保存間隔は次の待ち時間判定、保持数は次の保存先選択へ反映される。

## 重要な制約

- **`apply_settings_system` は `Time<Virtual>` を触らない**（D9）。`default_time_speed` は Startup の `load_settings_system` で一度だけ適用する。設定 UI の Default Speed ボタンは次回起動用の値のみ更新し、現在の pause/速度は変えない。
- **`PanCamera.enabled`** は通常 UI hover、Modal/Pause capture、text input focus、task area の左ドラッグ中に使う一時 guard 専用。area gesture は押下から release frame まで claim を維持し、残る capture / text focus / pointer claim がなくなった次 frame に既存設定どおり復帰する。永続化するのは `mouse_pan_settings.enabled` と `pan_speed` のみ。
- **crate 境界**: `GameSettings` 型は `hw_core`。ロード/保存/intent 処理は `bevy_app`。UI spawn は `hw_ui`（`SettingsPanelInitial` DTO 経由）。
- **旧settings互換**: `power_priority_enabled` / `autosave_*` に field-level `serde` default を置く。通知時間がない旧RONも4秒で読める。旧RONのUI scale、camera、debug等の値を保持したまま新fieldだけを補完し、file全体をdefaultへ戻すmigrationにはしない。
- **電力wake-up**: `sync_power_allocation_mode_from_settings_system`はresourceの実値が変わった時だけenergy allocationをdirtyにする。UI scale等の無関係な設定変更では配電を再実行しない。
- **widget の見た目同期**: headless widget のため見た目は自前。スライダー thumb は `sync_settings_slider_thumbs_system`、チェックマークは `sync_settings_checkmarks_system`（`Checked` の有無 → `Display`、いずれも `hw_ui/src/interaction/settings.rs`）が毎フレーム同期する。F12 は `debug_toggle_system` が `GameSettings` に加えて Debug Gizmos チェックボックスの `Checked` も直接更新する。
- **`settings/` は gitignore 済み**（ユーザーローカルファイル。`saves/` と同扱い）。
- **計測隔離**: save-transactionのruntime rootはsave rootとsettings rootをまとめて隔離する。artifactや
  実ユーザーの`settings/`を計測fixtureの保存先として再利用しない。

## 設定画面の開き方

- 右上の **メニュー** → **Settings** 行（Save/Load 下）
- Esc: 最前面 overlay を `LoadConfirm → Save/Load catalog → Settings → Pause → OperationDialog` の優先順で
  1つだけ閉じる。Load confirmのEscは親catalogへ戻る。Recovery Load catalogは`RecoveryFailed`中に閉じず、
  Settings close自体は背景 active modeをcancelしない。

## UI スケール

- 設定画面はviewport中央の幅380px（最大92%）、最大高さ88%のshellとする。
  見出しとCloseを固定し、項目本文を標準`ScrollArea`/`Scrollbar`で縦スクロールする。
  パネル面は不透明とし、背後に開いたメニューの文字が設定項目へ透けないようにする。
- 主経路: `UiScale`（スライダー 0.85〜1.25）
- 主要パネル文字: `FontSize::Rem(px / 20.0)`（`RemSize` デフォルト 20.0 固定）
- `RemSize` 同期は第 2 段階（目視 QA 後に必要なら `apply.rs` へ追加）

## 関連コード

- `crates/hw_core/src/settings.rs` — `GameSettings` 型
- `crates/bevy_app/src/systems/settings/` — 永続化・反映・observer
- `crates/hw_ui/src/setup/settings_panel.rs` — 設定 UI（BSN ルート + Slider/Checkbox）

## 項目追加手順

`hw_ui/_rules.md` の設定項目追加チェックリストを参照。
