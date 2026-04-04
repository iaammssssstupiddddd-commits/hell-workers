# デバッグ専用機能

開発・動作確認用のデバッグ機能一覧。実行時は通常プレイと同じバイナリで有効化できる。

---

## DevPanel（左上トグルパネル）

`DevPanel` は画面左上に常時表示される開発用ボタン群。
`crates/bevy_app/src/interface/ui/dev_panel.rs` で定義・管理する。

### 3D: ON / OFF ボタン

| 状態 | 色 | 説明 |
|---|---|---|
| ON（デフォルト） | 緑 | Camera3d RTT レンダリングを有効化 |
| OFF | 赤 | RTT を無効化（2D 表示のみ） |

- Resource: `Render3dVisible(pub bool)`（`crates/bevy_app/src/main.rs`）
- マーカー: `ToggleRender3dButton`

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

## WFC 川プレビュー seed

`crates/bevy_app/src/world/map/spawn.rs` は、MS-WFC-4 の本統合前に
`hw_world::generate_world_layout()` の結果（**WFC 生成の `terrain_tiles`**）を一時的に描画する。

- 用途: seed 付きワールド生成（川マスク + WFC 地形）の見た目確認
- 適用範囲: **地形描画のみ**
  - 初期木・岩・Site/Yard・猫車置き場などの startup 統合はまだ旧経路のまま
- 環境変数: `HELL_WORKERS_WORLDGEN_SEED=<u64>`
  - 指定時: その seed で描画
  - 未指定時: 起動ごとにランダム seed を生成
- 起動ログ: `BEVY_STARTUP: Map spawned ... preview worldgen seed=<seed>`
- 生成ログ（`hw_world`）: validate 失敗で次 attempt に進むとき `[WFC validate] attempt=...` が `eprintln!` される。`debug` / テストビルドでは採用レイアウトに対し `[WFC debug] ...` で `debug_validate` の警告が出る（fallback 時は `FallbackReached` 等）

この経路はデバッグ用の暫定接続であり、最終的な startup 統合は
`docs/plans/3d-rtt/wfc-ms4-startup-integration.md` の対象。

---

## 関連ファイル

- `crates/bevy_app/src/main.rs` — デバッグ resource 定義
- `crates/bevy_app/src/interface/ui/dev_panel.rs` — DevPanel UI
- `crates/bevy_app/src/plugins/interface_debug.rs` — デバッグシステム本体
- `crates/bevy_app/src/plugins/interface.rs` — Interface セット登録
- `crates/bevy_app/src/plugins/logic.rs` — Logic セット登録
