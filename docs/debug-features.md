# デバッグ専用機能

開発・動作確認用のデバッグ機能一覧。実行時は通常プレイと同じバイナリで有効化できる。

---

## DevPanel（左上トグルパネル）

`DevPanel` は画面左上に常時表示される開発用ボタン・インジケーター群。
`crates/bevy_app/src/interface/ui/dev_panel.rs` で定義・管理する。

パネル内の表示順（上から）：

| 行 | 内容 | 更新方法 |
|---|---|---|
| 3D: ON/OFF | Camera3d RTT 切り替えボタン | クリック |
| IBuild: ON/OFF | 壁即時完成トグルボタン | クリック |
| Mask / Light / Terrain / Objs | 3D 固定費の切り分けボタン | クリック |
| ─ セパレーター ─ | | — |
| FPS: XX | フレームレート表示 | `update_fps_display_system`（1秒毎） |
| LOD:X rtt:XX.Xpx | 地形 LOD レベルと tile_rtt_px | `update_lod_indicator_system`（毎フレーム） |
| RTT:H Mask:ON Light:ON Terrain:ON Objs:ON | RtT 品質と固定費トグル状態 | `update_render_perf_status_system`（変更時） |

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

- Resource: `Render3dVisible(pub bool)`（`crates/bevy_app/src/main.rs`）
- マーカー: `ToggleRender3dButton`

### 3D 固定費比較キー

- `F4`: RtT 品質を `High -> Medium -> Low` で循環する
- `F5`: Soul mask RtT pass を ON / OFF する
- `F6`: RtT 用 DirectionalLight を ON / OFF する
- `F7`: RtT terrain を ON / OFF する
- `F8`: RtT の main scene object（建築物・Soul・Familiar）を ON / OFF する
- 起動時に固定したい場合は `HW_DISABLE_SOUL_MASK=1` / `HW_DISABLE_RTT_DIRECTIONAL_LIGHT=1` / `HW_DISABLE_RTT_TERRAIN=1` / `HW_DISABLE_RTT_SCENE_OBJECTS=1` を指定する

### IBuild: ON / OFF ボタン（Instant Build）

| 状態 | 色 | 説明 |
|---|---|---|
| OFF（デフォルト） | 暗グレー | 通常の建築フロー（ワーカーが作業） |
| ON | 暗橙 | 壁を配置した次フレームに即時完成させる |

- Resource: `DebugInstantBuild(pub bool)`（`crates/bevy_app/src/main.rs`）
- マーカー: `InstantBuildButton`

---

## DebugInstantBuild（壁即時完成）

### 概要

`IBuild: ON` の状態で壁を配置すると、ワーカーによるフレーミング・コーティング工程を
スキップして次フレームに完成済み壁が表示される。  
Camera3d 角度の目視確認など、壁の 3D ビジュアルをすぐに確認したいときに使用する。

### 動作

- 配置直後に `WallTileBlueprint` を `WallTileState::Complete` に強制設定する
- `WallConstructionSite.phase` を `Coating` に強制移行する
- タイルに `spawned_wall` がない場合（フレーミング前）は完成済み `Building(Wall)` と
  `Building3dVisual(wall_material)` を直接 spawn する
- `spawned_wall` が既に存在する場合（仮設壁が立っている）は `ProvisionalWall` を除去して
  `is_provisional = false` に昇格させる
- `wall_construction_completion_system` が同フレーム内で site・tile の cleanup を行う

### 床なし配置バイパス

`IBuild: ON` のとき、壁配置時の「完成済み床の上にしか配置できない」制約を無視する。
占有・歩行可能チェック（`NotWalkable` / `OccupiedByBuilding` / `OccupiedByStockpile`）は引き続き有効。

### 実装箇所

| ファイル | 役割 |
|---|---|
| `crates/bevy_app/src/main.rs` | `DebugInstantBuild` resource 定義・`init_resource` |
| `crates/bevy_app/src/interface/ui/dev_panel.rs` | `InstantBuildButton` spawn・toggle・visual 更新システム |
| `crates/bevy_app/src/plugins/interface.rs` | ボタン systems を `Interface` セットに登録 |
| `crates/bevy_app/src/plugins/interface_debug.rs` | `debug_instant_complete_walls_system` |
| `crates/bevy_app/src/plugins/logic.rs` | wall construction グループ（Group D）の `wall_framed_tile_spawn_system` 直前に挿入 |
| `crates/bevy_app/src/interface/selection/floor_place/validation.rs` | `validate_wall_tile_no_floor_check`（floor 制約バイパス用） |
| `crates/bevy_app/src/interface/selection/floor_place/wall_apply.rs` | `bypass_floor_check` フラグで分岐 |
| `crates/bevy_app/src/interface/selection/floor_place/input.rs` | `handle_release` にフラグを伝搬 |
| `crates/bevy_app/src/interface/selection/floor_place/mod.rs` | `floor_placement_system` で `DebugInstantBuild` を読み込み |

---

## キーボードショートカット（デバッグスポーン）

`DebugVisible` resource が `true` のときのみ有効。
`crates/bevy_app/src/plugins/interface_debug.rs` の `debug_spawn_system` で処理する。

| キー | 動作 |
|---|---|
| `P` | カーソル位置に `DamnedSoul` をスポーン |
| `O` | カーソル位置に `Familiar (Imp)` をスポーン |

- Resource: `DebugVisible(pub bool)`（`crates/bevy_app/src/main.rs`）
- デフォルト: `false`（パネルや UI ボタンからの有効化は未実装）

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

## 関連ファイル

- `crates/bevy_app/src/main.rs` — デバッグ resource 定義
- `crates/bevy_app/src/interface/ui/dev_panel.rs` — DevPanel UI・FPS/LOD インジケーター
- `crates/bevy_app/src/systems/visual/terrain_lod.rs` — `TerrainLodMetrics` / `TerrainLodState` / `LodLevel`
- `crates/hw_ui/src/interaction/status_display/runtime.rs` — `update_fps_display_system`
- `crates/bevy_app/src/plugins/interface_debug.rs` — デバッグシステム本体
- `crates/bevy_app/src/plugins/interface.rs` — Interface セット登録
- `crates/bevy_app/src/plugins/logic.rs` — Logic セット登録
