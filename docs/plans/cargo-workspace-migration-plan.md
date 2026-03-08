# Cargo Workspace 移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `cargo-workspace-migration-plan-2026-03-07` |
| ステータス | `In Progress` |
| 作成日 | `2026-03-07` |
| 最終更新日 | `2026-03-08` |
| 作成者 | AI |
| 関連提案 | `docs/proposals/architecture-improvements-2026.md` |
| 関連Issue/PR | N/A |

## 1. 目的

- **解決したい課題**: `bevy_app` が依然として巨大な単一アプリケーションクレートであり、局所変更でも再コンパイル範囲が広い。
- **到達したい状態**: Cargo workspace を土台に、依存境界が明確な領域だけを段階的に別クレートへ切り出す。
- **成功指標**:
  - `cargo check --workspace` が常に通る
  - `cargo run` で起動できる
  - 切り出し前後で `cargo check` の所要時間を比較できる

## 2. 再評価結果

2026-03-08 時点で確認できた事実:

- workspace 自体はすでに導入済み
  - root `Cargo.toml` に `[workspace]` と `members = [".", "crates/*"]` がある
- 追加クレートとして `crates/hw_core/` と `crates/hw_world/` と `crates/hw_logistics/` と `crates/hw_jobs/` が存在する
- `cargo check --workspace` は成功している
- root crate は依然として以下を保持している
  - `src/events.rs`
  - `src/relationships.rs`
  - `src/world/`
  - `src/systems/jobs/`
  - `src/systems/logistics/`
- `hw_world` は world 全体ではなく、固定レイアウト定数・川/砂生成ロジック・pathfinding アルゴリズム・`TerrainType` を保持している
  - 追加で、ベース地形タイル生成・terrain border 判定・regrowth zone 定義/候補選定・spawn grid 選定補助も `hw_world` に保持している
  - 座標変換 (`world_to_grid` / `grid_to_world` / snap) と nearest query helper も `hw_world` に保持している
- `hw_core` には `events` のうち低結合な型群に加え、`WorkType` / `ResourceType` / `FamiliarAiState` / `AssignedTask` も移っている
- `hw_logistics` は base logistics component / helper / transport request model の最小単位を保持している
- `hw_jobs` は base job model と mud mixer model の最小単位を保持している
- `hw_components` は、現 checkout には存在しない

結論:

- 「workspace 導入」は成立している
- 「多クレート化」は `hw_core` と `hw_world` の最小単位抽出まで進んでいる
- 次に必要なのはエラー修正ではなく、**切り出し境界の再設計** である

## 3. 現在の到達点

### 完了済み

- root を workspace root として運用する構成
- `hw_core` クレートの追加
- `constants/` と `game_state.rs` の `hw_core` への移動
- `relationships.rs` の `hw_core` への移動
  - root の `src/relationships.rs` は互換維持のため re-export のみ保持
- `events.rs` のうち、`WorkType` / `ResourceType` / `AssignedTask` / `FamiliarAiState` に依存しない型群を `hw_core::events` へ移動
  - root の `src/events.rs` は高結合な型と re-export のみ保持
- `WorkType` を `hw_core::jobs` へ移動
  - root の `src/systems/jobs/mod.rs` は re-export のみ保持
- `ResourceType` を `hw_core::logistics` へ移動
  - root の `src/systems/logistics/types.rs` は re-export のみ保持
- `FamiliarAiState` を `hw_core::familiar` へ移動
  - root の `src/systems/familiar_ai/mod.rs` は re-export のみ保持
- `events.rs` のうち `WorkType` / `ResourceType` / `FamiliarAiState` 依存の型群も `hw_core::events` へ移動
  - root の `src/events.rs` は互換維持のため re-export のみ保持
- `AssignedTask` と関連 task execution data を `hw_core::assigned_task` へ移動
  - root の `src/systems/soul_ai/execute/task_execution/types.rs` は re-export のみ保持
- `WheelbarrowDestination` を `hw_core::logistics` へ移動
  - root の `src/systems/logistics/transport_request/components.rs` は re-export を保持
- `DoorState` を `hw_core::world` へ移動
  - `src/systems/jobs/door.rs` は re-export を保持
- `hw_world` クレートの追加
- `src/world/map/layout.rs` の fixed layout 定数を `hw_world::layout` へ移動
  - root の `src/world/map/layout.rs` は re-export のみ保持
- `src/world/river.rs` の川・砂タイル生成ロジックを `hw_world::river` へ移動
  - root の `src/world/river.rs` は re-export のみ保持
- `src/world/pathfinding.rs` のアルゴリズムを `hw_world::pathfinding` へ移動
  - `WorldMap` 依存は `PathWorld` trait で抽象化
  - root の `src/world/pathfinding.rs` は `WorldMap` 実装と互換ラッパーのみ保持
  - target 到達可否の共通判定 helper も `hw_world::pathfinding` へ集約
- `TerrainType` を `hw_world::terrain` へ移動
  - root の `src/world/map/mod.rs` は re-export を保持
- ベース地形タイル生成を `hw_world::mapgen` へ移動
  - root の `src/world/map/spawn.rs` は Bevy sprite spawn と `WorldMap` 反映のみ保持
- terrain border の隣接判定ロジックを `hw_world::borders` へ移動
  - root の `src/world/map/terrain_border.rs` は texture 解決と sprite spawn のみ保持
- regrowth の zone 定義と候補位置選定を `hw_world::regrowth` へ移動
  - root の `src/world/regrowth.rs` は Bevy resource と sprite spawn のみ保持
- spawn 用の近傍歩行可能マス探索と矩形内 walkable grid 選定を `hw_world::spawn` へ移動
  - root の `src/entities/spawn_position.rs` は re-export のみ保持
- `WorldMap` に `buildings` / `stockpiles` / `bridged_tiles` の操作メソッドを追加
  - `buildings` / `stockpiles` は read/write ともに高頻度経路をメソッド経由へ移行済み
- `WorldMap` に `tiles` / `tile_entities` / `obstacles` 用 accessor を追加
  - 現時点で `world_map.tiles` / `world_map.tile_entities` / `world_map.obstacles` の直接参照は `src/` から除去済み
- `task_execution` / spawn helper の `WorldMap` 依存を `&Res<WorldMap>` から `&WorldMap` へ縮小
  - `src/` の helper API では Bevy resource 型への依存を除去済み
- `WorldMapRead` system param を追加し、一部の read-only system を raw `Res<WorldMap>` から移行
  - `assign_task` / `drifting` / `movement` / `pathfinding` / `area_selection` / `placement_ghost` / `room validation` / `gathering separation`
- `WorldMapRead` の適用を `startup` / `spawn` / `observer` / `idle behavior` / `escaping` / `task execution` / `room detection` / `wall connection` / `regrowth` / `terrain border` / `building move preview` / `auto gather` / `mixer producer` まで拡大
- `WorldMapWrite` system param を追加し、write 系 system の raw `ResMut<WorldMap>` を移行
  - `src/` から raw `Res<WorldMap>` / `ResMut<WorldMap>` は除去済み
- `WorldMap` に occupancy 更新 API を追加
  - `set_building_occupancy` / `clear_building_occupancy` / `clear_building_occupancy_if_owned`
  - `register_bridge_tile` / `register_door` / `sync_door_passability`
  - placement / construction / completion / move 系の map 更新で再利用
- `WorldMap` に batch / stockpile 更新 API を追加
  - `add_grid_obstacle` / `remove_grid_obstacle` / `add_grid_obstacles`
  - `register_stockpile_tile` / `move_stockpile_tile`
  - obstacle sync / initial spawn / dream planting / zone placement / move 系で再利用
- `WorldMap` に cleanup / batch release API を追加
  - `release_building_grid_if_owned` / `release_building_grid_if_matches` / `release_building_grids_if_owned`
  - `take_stockpile_tiles`
  - wall completion / wall cancellation / move cleanup / zone removal で再利用
- `WorldMap` に building-type aware な footprint API を追加
  - `reserve_building_footprint`
  - `register_completed_building_footprint`
  - bridge / door / 通常建物の差分を `WorldMap` 側へ集約
  - placement / completion / startup spawn / provisional wall spawn で再利用
- `WorldMap` に construction footprint 用の batch API を追加
  - `reserve_building_footprint_tiles`
  - `clear_building_footprint`
  - `release_building_footprint_if_owned`
  - `release_building_footprint_if_matches`
  - wall / floor / move 系の construction shell で再利用
- `WorldMap` の pure coordinate / nearest query helper を `hw_world` へ移動
  - `world_to_grid` / `grid_to_world` / `snap_to_grid_center` / `snap_to_grid_edge`
  - `find_nearest_walkable_grid` / `find_nearest_river_grid`
  - `WorldMap` 側は互換 wrapper を残しつつ、責務を resource state と occupancy / terrain access に寄せた
- `src/world/map/spawn.rs` / `src/world/map/terrain_border.rs` / `src/world/regrowth.rs` は `hw_world` の pure API を直接使う shell に整理
  - `WorldMap` に残るのは Bevy resource と map state 更新責務
- root crate から `hw_core` の参照
- `hw_logistics` クレートの追加
- `src/systems/logistics/types.rs` の基礎コンポーネントを `hw_logistics::types` へ移動
  - `ResourceItem` / `ReservedForTask` / `BelongsTo` / `BucketStorage`
  - `Wheelbarrow` / `WheelbarrowParking` / `PendingBelongsToBlueprint` / `Inventory`
  - root の `src/systems/logistics/types.rs` は re-export のみ保持
- `src/systems/logistics/zone.rs` の `ZoneType` / `Stockpile` を `hw_logistics::zone` へ移動
  - root の `src/systems/logistics/zone.rs` は re-export のみ保持
- `src/systems/logistics/water.rs` の tank water helper を `hw_logistics::water` へ移動
  - root の `src/systems/logistics/water.rs` は re-export のみ保持
- `src/systems/logistics/ground_resources.rs` の純粋 helper を `hw_logistics::ground_resources` へ移動
  - root の `src/systems/logistics/ground_resources.rs` は re-export のみ保持
- `src/systems/logistics/item_lifetime.rs` の `ItemDespawnTimer` を `hw_logistics::item_lifetime` へ移動
  - root の `src/systems/logistics/item_lifetime.rs` は system shell のみ保持
- `src/systems/logistics/transport_request/` の shared model を `hw_logistics::transport_request` へ移動
  - `TransportRequestKind`
  - `TransportPriority` / `TransportRequest` / `TransportDemand` / `TransportPolicy`
  - `ManualTransportRequest` / `TransportRequestFixedSource` / `ManualHaulPinnedSource`
  - `WheelbarrowLease` / `WheelbarrowPendingSince` / `TransportRequestState`
  - root 側の `components.rs` / `kinds.rs` は re-export のみ保持
- `TransportRequestMetrics` と `transport_request_state_sync_system` を `hw_logistics::transport_request` へ移動
  - root 側の `metrics.rs` / `state_machine.rs` は re-export のみ保持
- `hw_jobs` クレートの追加
- `src/systems/jobs/mod.rs` の base job model を `hw_jobs::model` へ移動
  - `BuildingType` / `BuildingCategory` / `Building`
  - `FlexibleMaterialRequirement` / `Blueprint`
  - `ProvisionalWall` / `RestArea`
  - `SandPile` / `BonePile` / `Tree` / `TreeVariant` / `Rock`
  - `TargetBlueprint` / `ObstaclePosition`
  - `Designation` / `Priority` / `TaskSlots`
  - `IssuedBy` alias と `WorkType` の再公開も `hw_jobs::model` 側へ集約
  - root の `src/systems/jobs/mod.rs` は app shell と re-export のみ保持
- `src/systems/jobs/mud_mixer.rs` の model を `hw_jobs::mud_mixer` へ移動
  - `MudMixerStorage` / `TargetMixer` / `StoredByMixer`
  - root の `src/systems/jobs/mud_mixer.rs` は re-export のみ保持
- root crate から `hw_world` の参照
- root crate から `hw_jobs` の参照
- root crate から `hw_logistics` の参照
- build timing を記録
  - `2026-03-08`: `cargo check --workspace --timings` が `2.59s`
  - report: `target/cargo-timings/cargo-timing-20260308T034413.390702268Z.html`

### 未完了

- build timing の比較表の完成
  - post-migration timing は記録済み
  - historical baseline は未記録のため、比較対象をどう固定するかだけ残っている

## 4. スコープ

### 対象（In Scope）

- workspace 構成を壊さずに維持すること
- `hw_core` の責務を明確化すること
- 依存境界が比較的明確な領域を小さく切り出すこと
- 各段階でビルド時間を測定すること

### 非対象（Out of Scope）

- 一度に広範囲を多クレート化すること
- `hw_components` のような広すぎる共通クレートを先に作ること
- バルクスクリプトで import を大量書き換えすること
- ゲームロジックの仕様変更

## 5. 設計原則

1. **green を維持する**
   - 各段階で `cargo check --workspace` を通した状態を保つ。
2. **計測してから切る**
   - 「何となく分ける」のではなく、変更頻度と依存関係を見て切り出す。
3. **責務ごとに切る**
   - `hw_components` のような雑多な箱を作らず、`world` `logistics` のように責務単位で分ける。
4. **型と impl を同じクレートに置く**
   - `E0116` を避けるため、型定義とその主要 `impl` は同じクレートに置く。
5. **手動で進める**
   - バルク置換スクリプトは使わず、ファイル単位で import と依存を直す。

## 6. 推奨依存グラフ

現時点の推奨は以下。これは最終形ではなく、段階的切り出しの目安とする。

```text
hw_core
  ├─ hw_world
  ├─ hw_jobs
  ├─ hw_logistics
  └─ bevy_app

hw_world
  └─ bevy_app

hw_jobs
  └─ bevy_app

hw_logistics
  └─ bevy_app
```

補足:

- `hw_components` は現時点では作らない
- `events` は依存先が広いため、これ以上の分割は早期対象にしない
- `relationships` は `hw_core` へ移設済み
- `hw_world` は `layout` / `river` / `pathfinding` / `TerrainType` / `mapgen` / `borders` / `regrowth` / `spawn` / `coords` / `query` を保持し、`WorldMap` 本体は root resource として残す
- `hw_jobs` はまず base job model と mud mixer model を保持し、door / building completion / floor construction / wall construction の app shell は root に残す
- `hw_logistics` はまず base component / shared helper / transport request model / metrics / state sync を保持し、producer / plugin / lifecycle shell は root に残す

## 7. フェーズ計画

### P0: ベースライン固定

目的:

- 現在の workspace 状態を正確に固定する

作業:

- `cargo check --workspace` を実行
- 必要なら `cargo run` を実行して起動確認
- `cargo check --workspace --timings` などで baseline を記録
- `hw_core` から未使用 import や不要依存があれば整理

完了条件:

- `cargo check --workspace` が通る
- baseline 計測結果を記録できる

### P1: `hw_core` の責務整理

目的:

- `hw_core` に置くべきものと置かないものを明確にする

候補:

- 維持: `constants`, `game_state`
- 維持: `relationships`
- 移動済み: `events` の低結合型群、`WorkType`、`ResourceType`、`FamiliarAiState`、`AssignedTask`
- 保留: historical timing 比較の固定方法

作業:

- `relationships.rs` の参照元・依存先を棚卸し
- `hw_core` に移しても逆依存が発生しないか確認
- 移動する場合は root 側を `pub use` または import 修正で置き換える

完了条件:

- `hw_core` の責務が文書化されている
- `cargo check --workspace` が通る

### P2: `hw_world` 切り出し可否の判定

目的:

- world 系コードが独立クレートとして成立するか確認する

主対象:

- `src/world/`

補助対象:

- world 専用の定数、地形、地図生成、再成長、river など

注意:

- `spawn_terrain_borders` はすでに `src/world/map/terrain_border.rs` にあるため、古い計画書の前提を使わない
- `assets` や render 寄りの依存が強い場合は、`hw_world` への全移動ではなく「純粋な world data / map logic」のみ先に切る
- 現時点で先に切り出した最小単位は `layout` / `river` / `pathfinding` / `TerrainType`
- `pathfinding.rs` は `PathWorld` trait で切り出し済み
- `DoorState` 依存は解消済み
- `buildings` / `stockpiles` は read/write ともに accessor 経由へ移行済み
- `tiles` / `tile_entities` / `obstacles` の直接アクセスも accessor 経由へ移行済み
- `WorldMap` の pure helper は `hw_world` へ寄せ終えており、root 側に残るのは resource state と Bevy app shell

完了条件:

- `hw_world` を切り出すか、見送るかを判断できる
- 切り出す場合は最小構成で `cargo check --workspace` が通る

### P3: `hw_logistics` 切り出し可否の判定

目的:

- logistics 系コードを shared crate と app shell に分けられるか確認する

主対象:

- `src/systems/logistics/types.rs`
- `src/systems/logistics/zone.rs`
- `src/systems/logistics/water.rs`
- `src/systems/logistics/ground_resources.rs`
- `src/systems/logistics/item_lifetime.rs`
- `src/systems/logistics/transport_request/`

注意:

- `jobs` と相互依存しやすいため、先に型依存を整理する
- `ResourceType` のような基礎型は、必要であれば `hw_core` に残すか専用クレートを再検討する
- `BuildingType -> ResourceType` のような依存を見ずに切り出しを始めない

完了条件:

- 依存の方向が整理されている
- shared model / helper を `hw_logistics` に寄せた上で `cargo check --workspace` が通る

## 8. 実行順

推奨順序:

1. P0: baseline 固定
2. P1: `hw_core` の責務整理
3. P2: `hw_world` の可否判定と最小切り出し
4. P3: `hw_logistics` の可否判定と最小切り出し
5. P4: `hw_jobs` の最小切り出し

やらない順序:

- `jobs` / `logistics` / `AI` / `UI` を同時に分割する
- `hw_components` を先に作って型をまとめて押し込む

## 9. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 広すぎる共通クレートを作る | 高 | ドメイン単位で切る。`hw_components` は保留 |
| 型と impl が別クレートに分かれる | 高 | 型定義と主要 `impl` を同じクレートに置く |
| `jobs` と `logistics` の相互依存 | 高 | 先に依存棚卸しを行い、切り出し可否を判定する |
| バルク置換で import が壊れる | 高 | 手動でファイル単位に修正する |
| 分割したのにビルドが速くならない | 中 | 各段階で timings を記録して効果を測定する |

## 10. 検証計画

- 毎段階必須:
  - `cargo check --workspace`
- 節目で実施:
  - `cargo run`
- 効果測定:
  - `cargo check --workspace --timings`
  - 必要なら追加で手動計測を記録

## 11. ロールバック方針

- 各フェーズは小さなコミット単位で進める
- 問題が出たら直前コミットへ戻せる状態を保つ
- 無関係な未コミット変更を巻き込む破壊的な全消去コマンドは使わない

## 12. 次の担当者が最初にやること

1. `cargo check --workspace` を実行して current green を再確認する
2. floor / wall construction site を root shell のまま固定するか、追加で `hw_jobs` へ寄せる価値があるかだけ確認する
3. `cargo run` による起動確認を取る
4. historical baseline が必要なら timing 比較対象の commit を固定して再計測する

## 13. Definition of Done

- [ ] workspace 構成が維持されている
- [ ] 切り出し対象ごとに `cargo check --workspace` が通っている
- [ ] `cargo run` で起動できる
- [ ] before / after のビルド時間比較が残っている
- [ ] 次にどのドメインを切るべきかが文書で説明できる

## 14. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-07` | AI | 初版作成 |
| `2026-03-08` | AI | 現 checkout を基準に全面再評価。存在しないクレート前提の記述を削除し、段階的移行計画へ修正 |
| `2026-03-08` | AI | `relationships.rs` を `hw_core` へ移設し、root 側は re-export 互換レイヤーへ変更 |
| `2026-03-08` | AI | `hw_world` を追加し、`layout` と `river` の純粋ロジックを移設 |
| `2026-03-08` | AI | `pathfinding` を `hw_world` へ移設し、`PathWorld` trait と root 互換ラッパーを追加 |
| `2026-03-08` | AI | `DoorState` を `hw_core` へ、`TerrainType` を `hw_world` へ移設 |
| `2026-03-08` | AI | `WorldMap` の building/stockpile/bridge 操作メソッドを追加し、高頻度 write path を移行 |
| `2026-03-08` | AI | `WorldMap` の building/stockpile 直接参照を accessor 化し、高頻度 read path も移行 |
| `2026-03-08` | AI | `WorldMapRead` system param を追加し、read-only system 境界の `Res<WorldMap>` を一部置換 |
| `2026-03-08` | AI | `WorldMapRead` / `WorldMapWrite` を `src/` 全体へ展開し、raw `Res<WorldMap>` / `ResMut<WorldMap>` を除去 |
| `2026-03-08` | AI | `WorldMap` に occupancy 更新 API を追加し、placement / construction / move / door 更新へ適用 |
| `2026-03-08` | AI | `WorldMap` に obstacle / stockpile の batch 更新 API を追加し、spawn / placement / move / cleanup へ適用 |
| `2026-03-08` | AI | `WorldMap` に cleanup / batch release API を追加し、conditional な construction cleanup と zone removal を簡素化 |
| `2026-03-08` | AI | `WorldMap` に building-type aware な footprint API を追加し、placement / completion / startup spawn の world 更新規約を集約 |
| `2026-03-08` | AI | `WorldMap` に construction footprint batch API を追加し、wall / floor / move の world 更新を footprint 単位へ整理 |
| `2026-03-08` | AI | `hw_world` に座標変換と nearest query helper を移し、world shell を pure API 直接利用へ整理 |
| `2026-03-08` | AI | `hw_logistics` を追加し、base logistics component / helper / transport request model / metrics / state sync を移設 |
| `2026-03-08` | AI | `cargo check --workspace --timings` を実行し、post-migration timing report を記録 |
| `2026-03-08` | AI | `hw_jobs` を追加し、base job model と mud mixer model を移設 |
