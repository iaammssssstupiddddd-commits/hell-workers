# デバッグ専用機能

開発・動作確認用のデバッグ機能一覧。実行時は通常プレイと同じバイナリで有効化できる。

---

## DevPanel（左上トグルパネル）

`DevPanel` は画面左上に表示される開発用ボタン・インジケーター群。
`crates/bevy_app/src/interface/ui/dev_panel.rs` を公開facadeとし、`dev_panel/` 配下のcomponents、spawn、actions、presentationが定義・表示・操作を分担する。

パネル内の表示順（上から）：

| 行 | 内容 | 更新方法 |
|---|---|---|
| `-` / `+` | パネル本文の最小化 / 復元ボタン | クリック |
| 3D: ON/OFF | Camera3d RTT 切り替えボタン | クリック |
| IBuild: ON/OFF | 壁即時完成トグルボタン | クリック |
| Mask / Light / Light2 / Terrain / Objs | 3D 固定費の切り分けボタン | クリック |
| ─ セパレーター ─ | | — |
| FPS: XX | フレームレート表示 | `update_fps_display_system`（1秒毎） |
| LOD:X rtt:XX.Xpx | 地形 LOD レベルと tile_rtt_px | `update_lod_indicator_system`（毎フレーム） |
| RTT:H Mask:ON Light:ON Light2:OFF Terrain:ON Objs:ON | RtT 品質と固定費トグル状態 | `update_render_perf_status_system`（変更時） |

最小化ボタンはパネル本文の外側に常時残る。`-` を押すと既存ボタン、インジケーター、
テキスト入力を含む本文を非表示にし、`+` を押すと同じentity群を復元する。
本文内のテキスト入力がフォーカス中なら、最小化時にフォーカスも解除する。
最小化中も FPS / LOD などの更新systemは動作し、復元時には最新値を表示する。
初期状態は展開で、最小化状態は設定ファイルへ永続化しない。

### FPS インジケーター

- `UiSlot::FpsText` entity として DevPanel 内に spawn し、`spawn_dev_panel_system` で `UiNodeRegistry` に登録する
- `update_fps_display_system`（`hw_ui` 側）が 1 秒間隔で平均 FPS を書き込む
- 以前は `top_right_slot` 内に独立 widget として配置していたが、DevPanel と重なって不可視になったため統合

### LOD インジケーター

- マーカー: `LodIndicatorText`
- 表示形式: `LOD:X rtt:YY.Ypx`（X = LOD レベル、YY.Y = `TerrainLodMetrics.tile_rtt_px`）
- `update_lod_indicator_system` が毎フレーム `TerrainLodState.level` と `TerrainLodMetrics.tile_rtt_px` を読んでテキストを更新する
- LOD 遷移の閾値確認（hysteresis デバッグ）に使用する

### 3D: ON / OFF ボタン

| 状態 | 色 | 説明 |
|---|---|---|
| ON（デフォルト） | 緑 | Camera3d RTT レンダリングを有効化 |
| OFF | 赤 | RTT を無効化（2D 表示のみ） |

- Resource: `Render3dVisible(pub bool)`（定義: `crates/bevy_app/src/lib.rs`、production Appへの初期登録: `plugins/game.rs`）
- マーカー: `ToggleRender3dButton`

### 3D 固定費比較操作

- `F4`: RtT 品質を `High -> Medium -> Low` で循環する
- `F6`: RtT 用 DirectionalLight を ON / OFF する
- `F7`: RtT terrain を ON / OFF する
- `F8`: RtT の main scene object（建築物・Soul・Familiar）を ON / OFF する
- 追加テスト用の2本目のRtT `DirectionalLight` はDevPanelの `Light2` buttonから切り替える。F5/F9はplayer Save/Load専用で、debug aliasを持たない
- 起動時に固定したい場合は `HW_DISABLE_RTT_DIRECTIONAL_LIGHT=1` / `HW_ENABLE_RTT_EXTRA_DIRECTIONAL_LIGHT=1` / `HW_DISABLE_RTT_TERRAIN=1` / `HW_DISABLE_RTT_SCENE_OBJECTS=1` を指定する
- F3/F4/F6/F7/F8/F12 は project-owned resolver が exact chord として解決し、Modal/Pause/TextInput 中は生成しない。各 consumer は既存 Resource mutation だけを担当する
- `F12`は`DebugVisible`を切り替える。trueの間、`hw_visual::soul::task_link_system`が
  `SoulTaskVisualState`のtask targetへGizmosの線と終点circleを描く。永続する`WorkLine` entity/componentは生成しない

### IBuild: ON / OFF ボタン（Instant Build）

| 状態 | 色 | 説明 |
|---|---|---|
| OFF（デフォルト） | 暗グレー | 通常の建築フロー（ワーカーが作業） |
| ON | 暗橙 | 壁の配置確定時に完成Wallを直接生成する |

- Resource: `DebugInstantBuild(pub bool)`（定義: `crates/bevy_app/src/lib.rs`、production Appへの初期登録: `plugins/game.rs`）
- マーカー: `InstantBuildButton`

---

## DebugInstantBuild（壁即時完成）

### 概要

`IBuild: ON` の状態で壁を配置すると、ワーカーによるフレーミング・コーティング工程を
スキップして配置transaction内で完成済み壁を生成する。仮想時間がpause中でも、配置入力が
許可されている状態なら即時完成はvirtual-time Logic gateに依存しない。
Camera3d 角度の目視確認など、壁の 3D ビジュアルをすぐに確認したいときに使用する。

### 動作

- ONで新しく配置するWallは`WallConstructionSite` / `WallTileBlueprint`を作らず、productionの
  `spawn_wall_shell`を通して完成済み`Building(Wall)`と`Building3dVisual`を直接spawnする
- 同じ配置transactionで完成bounceを開始し、`WorldMap`の各タイルownerを完成Wallへ設定する
- ONへ切り替える前から存在する建設siteは従来のfallbackを使う。タイルを`Complete`、siteを
  `Coating`へ強制し、仮設Wallを昇格して通常completion systemでcleanupする

### 床なし配置バイパス

`IBuild: ON` のとき、壁配置時の「完成済み床の上にしか配置できない」制約を無視する。
占有・歩行可能チェック（`NotWalkable` / `OccupiedByBuilding` / `OccupiedByStockpile`）は引き続き有効。

### 実装箇所

| ファイル | 役割 |
|---|---|
| `crates/bevy_app/src/lib.rs` | `DebugInstantBuild` resource 定義 |
| `crates/bevy_app/src/plugins/game.rs` | `DebugInstantBuild` の `init_resource` |
| `crates/bevy_app/src/interface/ui/dev_panel.rs` / `dev_panel/` | `InstantBuildButton` のcomponent、spawn、toggle、visual更新システム |
| `crates/bevy_app/src/plugins/interface.rs` | ボタン systems を `Interface` セットに登録 |
| `crates/bevy_app/src/plugins/interface_debug.rs` | `debug_instant_complete_walls_system` |
| `crates/bevy_app/src/plugins/logic.rs` | wall construction グループ（Group D）の `wall_framed_tile_spawn_system` 直前に挿入 |
| `crates/bevy_app/src/interface/selection/floor_place/validation.rs` | `validate_wall_tile_no_floor_check`（floor 制約バイパス用） |
| `crates/bevy_app/src/interface/selection/floor_place/wall_apply.rs` | ON時にproduction Wall shellを直接commit、OFF時は通常site/tileを生成 |
| `crates/bevy_app/src/interface/selection/floor_place/input.rs` | `handle_release` にIBuild状態と3D handleを伝搬 |
| `crates/bevy_app/src/interface/selection/floor_place/mod.rs` | `floor_placement_system` で `DebugInstantBuild` を読み込み |

---

## キーボードショートカット（デバッグスポーン）

`DebugVisible` resource が `true` のときのみ有効。
`crates/bevy_app/src/plugins/interface_debug.rs` の `debug_spawn_system` で処理する。

| キー | 動作 |
|---|---|
| `P` | カーソル位置に `DamnedSoul` をスポーン |
| `O` | カーソル位置に `Familiar (Imp)` をスポーン |

- Resource: `DebugVisible(pub bool)`（定義: `crates/bevy_app/src/lib.rs`、production Appへの初期登録: `plugins/game.rs`）
- デフォルト: `false`（パネルや UI ボタンからの有効化は未実装）
- P/O の可否は resolver frame 開始時の `DebugVisible` で確定する。F12 と同時入力した場合も、後段の F12 consumer が値を変更してから P/O を再判定しない

---

## ワールド生成 seed（本番 startup 経路）

MS-WFC-4 以降、`Startup` の `setup()` が `prepare_generated_world_layout_resource()` で
`hw_world::generate_world_layout(master_seed)` を実行し、`GeneratedWorldLayoutResource` を挿入する。
`PostStartup` の `spawn_map_timed` と `initial_resource_spawner_timed` が**同じ layout**を参照し、
3D 地形・初期木/岩・初期木材・Site/Yard・猫車置き場・regrowth 初期化まで一貫する。

- 環境変数: `HELL_WORKERS_WORLDGEN_SEED=<u64>`
  - 指定時: その seed でワールド生成
  - 未指定時: 起動ごとにランダム seed
- 地形スポーン後のログ例:
  `BEVY_STARTUP: Map spawned (100x100 tiles, worldgen seed=<u64>, attempt=<u32>, fallback=<bool>)`
- より前段のログ例（layout 準備時）:
  `BEVY_STARTUP: Prepared worldgen layout (seed=..., attempt=..., fallback=...)`
- 生成ログ（`hw_world`）: validate 失敗で次 attempt に進むとき `[WFC validate] attempt=...` が `eprintln!` される。`debug` / テストビルドでは採用レイアウトに対し `[WFC debug] ...` で `debug_validate` の警告が出る（fallback 時は `FallbackReached` 等）

詳細な起動順序と責務は `docs/plans/3d-rtt/archived/wfc-ms4-startup-integration.md` を参照。

---

## 解体 actual-window受入ドライバ

`NativeDeconstructionAcceptancePlugin`は通常起動では無効なopt-in driverであり、
`hell-workers-run-native-acceptance` Skillの`plan-deconstruction`からだけ起動する。
V1〜V5でOrdersからの解体指定、各建物種とSoul Spa、salvage表示、save/load後のstale request拒否、
Help capture、Task Dashboardのpriority変更と2段階cancelをproduction経路で検査する。最終frameでは
load後に残したWallを選択・pinし、typed `InfoPanelNodes`のroot authorityが一つだけであること、
`Building: Wall` header、空のplain-building summary、非対象sectionの非表示を実UI上で検査してから撮影する。

save/load証跡はartifact内の専用`runtime/saves/world.scn.ron`へ隔離する。driverは現行の
`SaveStorageRoot + SaveSlotId::LegacyDefault`を同じpathへ束縛し、save requestを一度だけ発行する。
Task Dashboardの2段階cancel中は`Time<Virtual>`をpauseせず相対速度だけ0にして、UIとLogic ownerを
動かしたままcommit claimとの競合を防ぐ。外側validatorはV1〜V5、WorldEpoch、save hash、
実window screenshot、renderer adapter/backendを独立に再検証する。

2026-08-28のrefactor closureではIntel Arc / Vulkan / X11でV1〜V5と最終Info PanelがPASSし、
`target/native-acceptance/building-deconstruction-20260828T141118Z-37ff852a/`へ
2560×1440 PNG、503,876 byteのsave、source fingerprint付きのfail-closed artifactを保存した。

検査対象と結果schemaの正本は
`crates/bevy_app/src/systems/jobs/deconstruction/native_acceptance.rs`である。

---

## セーブ／ロード actual-window受入ドライバ

`NativeSaveLoadAcceptancePlugin`は通常バイナリに含まれる開発専用のopt-in driverであり、
`HW_NATIVE_SAVE_LOAD_ACCEPTANCE_ARTIFACT`が未設定ならpluginを追加せず、通常起動へ影響しない。
実機確認はrepositoryの`hell-workers-run-native-acceptance` Skillが定めるno-prompt `kitty` launcher、
fresh artifact、source fingerprint、bounded monitor契約から起動する。

- `HW_NATIVE_SAVE_LOAD_ACCEPTANCE_ARTIFACT`: 既存の空ディレクトリを絶対pathで指定する。
- `HW_NATIVE_SAVE_LOAD_ACCEPTANCE_RUNTIME_ROOT`: artifact外のfresh disk-backed runtime rootを絶対pathで指定する。
- `HW_NATIVE_SAVE_LOAD_ACCEPTANCE_RUN_ID`: path separatorや改行を含まないfresh run IDを指定する。
- save-catalog profileは`HW_WINDOW_BACKEND=x11`を必須にする。headless / Waylandは、ゲーム自身の
  client windowへ画像を束縛できないため受入対象にしない。performance scenarioとの併用も拒否する。
- `driver-result.json`、capture marker、screenshot、runtimeのmanual save/settingsが既に存在するjobはstaleとして起動前に拒否する。
- driverはruntime rootの`saves/`と`settings/`をStartup前に注入し、実ユーザーの同名directoryを読書きしない。
  実windowと永続worldの成立後にvirtual timeをpauseし、productionの`UiIntent → catalog/modal → input capture → Last dispatcher`
  を通す。V1〜V5は、initial/manual slot save-load、occupied slot overwrite、status表示とinvalid-body recovery、
  confirm/escape ownership、rollback/recovery-only fail-closedと明示resumeをそれぞれ検査する。
- 外側monitorはrun ID付き`capture-ready.txt`を受け、起動したCargo process treeのPIDを`_NET_WM_PID`で
  照合した**唯一の X11 client window**だけを`import -window <id>`で撮影する。root desktop / 全画面撮影へは
  fallbackしないため、別windowやoverlayのmarkerをゲームUI証跡として受理しない。size・dimensions・SHA-256値に加え、
  fixed `capture_scope=x11-client-window`、window ID、window PIDを`capture.done.txt`へ記録する。driverはrun IDと
  filename、PNG header、640×360以上、16 MiB以下、ack一致、marker、scope/window ID/PIDを検証してから、その値を
  create-newの`driver-result.json`へ記録してPASSを確定する。`capture-ready.txt`とackはいずれもcreate-newの
  atomic publishであり、monitorの`xprop`/`import`呼出は5秒でfail-closedに終了する。ackを受理する直前まで
  Save catalog自身が唯一のforeground capture rootであることをdriverが再確認するため、markerだけが残った画面を
  UI証跡としてPASSにできない。
- driver全体は180秒でfail-closedに終了する。physical F5/F9/Escのdesktop入力注入は行わず、resolver mappingは
  unit/integration、actual windowではproduction intent/capture/modal stateを証明する。失敗artifactは上書き・
  自動削除せず診断用に保持する。

検査対象と結果schemaの正本は`crates/bevy_app/src/systems/save/native_acceptance.rs`、
一般のsave/load契約は[save_load.md](save_load.md)を参照する。

## Track A2 通知 actual-window受入ドライバ

`NativeNotificationAcceptancePlugin`は配置不能理由と通知UIをactual windowで検査する開発専用の
opt-in driverである。`HW_NATIVE_NOTIFICATION_ACCEPTANCE_ARTIFACT`が未設定の通常起動ではpluginを
追加しない。起動は`hell-workers-run-native-acceptance` Skillの`plan-notifications`だけを入口にする。

- artifact、artifact外のfresh runtime root、run IDをそれぞれ
  `HW_NATIVE_NOTIFICATION_ACCEPTANCE_ARTIFACT`、`HW_NATIVE_NOTIFICATION_ACCEPTANCE_RUNTIME_ROOT`、
  `HW_NATIVE_NOTIFICATION_ACCEPTANCE_RUN_ID`へ束縛する。
- runtimeの`saves/`と`settings/`をStartup前に注入し、実ユーザーの永続データを読書きしない。
- productionの配置tooltipと`SaveLoadOutcome → UserFacingNotification → reducer → presenter`を通し、
  typed rejection、target/result表示、同一失敗のrepeat集約、toast 3/history 64の有界契約を検査する。
- `Time<Virtual>`をpauseしたまま`Time<Real>`でtoastがexpireすること、toast surfaceが入力透過で、
  再表示したhistory panelが`UiInputBlocker` / `FocusPolicy::Block`を持つことを検査する。
- 最終tooltip、toast、history panel、PASS bannerをprimary windowのBevy screenshotとして保存し、
  PNG構造、640×360以上、16 MiB以下、renderer adapter/backend/display handle、exact artifact setを
  driverと外側verifierが独立に検証する。headless、performance scenario、他native profileとの併用は拒否する。

検査対象とresult schemaの正本は
`crates/bevy_app/src/interface/ui/notifications/native_acceptance.rs`である。失敗jobは上書き・自動削除せず、
fresh jobだけを再実行する。

---

## 関連ファイル

- `crates/bevy_app/src/lib.rs` — デバッグ resource 定義
- `crates/bevy_app/src/plugins/game.rs` — デバッグ resource 初期登録とproduction App構成
- `crates/bevy_app/src/interface/ui/dev_panel.rs` / `dev_panel/` — DevPanel facadeとUI・FPS/LODインジケーター実装
- `crates/bevy_app/src/systems/visual/terrain_lod.rs` — `TerrainLodMetrics` / `TerrainLodState` / `LodLevel`
- `crates/hw_ui/src/interaction/status_display/runtime.rs` — `update_fps_display_system`
- `crates/bevy_app/src/plugins/interface_debug.rs` — デバッグシステム本体
- `crates/bevy_app/src/plugins/interface.rs` — Interface セット登録
- `crates/bevy_app/src/plugins/logic.rs` — Logic セット登録
- `crates/bevy_app/src/systems/jobs/deconstruction/native_acceptance.rs` — opt-in解体受入driver
- `crates/bevy_app/src/systems/save/native_acceptance.rs` — opt-in actual-window save/load受入driver
- `crates/bevy_app/src/interface/ui/notifications/native_acceptance.rs` — opt-in Track A2 placement/notification受入driver
