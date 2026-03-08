# Cargo Workspace Guide

本プロジェクトの Cargo workspace 構成と、コードをどの crate に置くべきかの判断基準をまとめたガイドです。

## 1. 目的

- root crate (`bevy_app`) を Bevy の app shell に寄せる
- 純粋ロジックや共有 model を責務ごとに別 crate へ置く
- `cargo check --workspace` を常に green に保ちながら段階的に分割する

## 2. 現在の workspace 構成

`Cargo.toml` の workspace member は以下です。

```text
.
crates/hw_core
crates/hw_world
crates/hw_logistics
crates/hw_jobs
crates/hw_ai
crates/hw_spatial
crates/hw_ui
```

依存の向きは次を基本とします。

```text
hw_core
  ├─ hw_world
  ├─ hw_logistics
  ├─ hw_jobs
  ├─ hw_spatial
  ├─ hw_ai
  └─ bevy_app

hw_world
  └─ bevy_app

hw_logistics
  └─ bevy_app

hw_jobs
  └─ bevy_app

hw_spatial (hw_core + hw_world + hw_logistics + hw_jobs)
  └─ bevy_app

hw_ai (hw_core + hw_jobs + hw_logistics + hw_world + hw_spatial)
  └─ bevy_app

hw_ui
  ├─ hw_core
  ├─ hw_jobs
  ├─ hw_logistics
  └─ bevy_app
```

重要な原則:

- leaf crate から root crate (`bevy_app`) へ逆依存しない
- `hw_components` のような雑多な共通箱は作らない
- 型定義とその主要 `impl` は同じ crate に置く

### `hw_ai`

役割:

- Root crate に依存しない AI コアロジック
- Soul AI および Familiar AI の純粋なシステム実装
- hw_core / hw_jobs / hw_logistics / hw_world を組み合わせた AI ドメインロジック

代表例:

- `SoulAiCorePlugin` — Soul AI の Update/Execute/Decide ヘルパーフェーズコアシステム
- `FamiliarAiCorePlugin` — Familiar AI の Perceive/Decide/Execute フェーズコアシステム
- `soul_ai::update::*` — 疲労・バイタル・夢・集会・休憩所の更新システム
- `soul_ai::execute::designation_apply` — Designation 要求適用
- `soul_ai::execute::gathering_apply` — 集会管理要求適用（Merge / Dissolve / Recruit / Leave）
- `soul_ai::decide::idle_behavior::transitions` — IdleBehavior 遷移判定ヘルパー（次の行動選択・持続時間計算）
- `soul_ai::decide::idle_behavior::task_override` — タスク割り当て時の集会・休憩解除ヘルパー
- `soul_ai::decide::idle_behavior::exhausted_gathering` — 疲労集会（ExhaustedGathering）状態処理ヘルパー
- `soul_ai::helpers::gathering` — 集会スポット型定義・ヘルパー
- `soul_ai::helpers::gathering_positions` — 集会周辺ランダム位置生成・overlap 回避（`PathWorld + SpatialGridOps` 経由）
- `soul_ai::helpers::gathering_motion` — 集会中移動先選定（Wandering / Still retreat）
- `soul_ai::helpers::work::is_soul_available_for_work` — 作業可否判定ヘルパー
- `soul_ai::decide::escaping` / `soul_ai::perceive::escaping` — 逃走判断ロジック
- `soul_ai::decide::gathering_mgmt` — 集会管理要求生成
- `familiar_ai::perceive::state_detection` — 使い魔 AI 状態遷移検知
- `familiar_ai::decide::following` — 使い魔追尾システム（hw_core 型のみ依存）
- `familiar_ai::execute::state_apply` — `FamiliarStateRequest` 適用
- `familiar_ai::execute::state_log` — 状態遷移ログ出力

ここに置かないもの:

- `GameAssets` 依存の sprite spawn
- `WorldMap` resource / `WorldMapRead` SystemParam を直接参照するシステム
- `SpatialGrid` concrete resource を直接参照しない（`hw_spatial` / root wrapper を経由）
- UI システム
- `Commands` で複雑な Entity 生成を行うもの
- `unassign_task`（`helpers/work.rs`）は `WheelbarrowMovement` / `Visibility` / `Transform` など root 依存が強いため core 化対象外

## 3. 各 crate の責務

### `bevy_app`

役割:

- Bevy plugin / system 登録
- `Commands`, `Res`, `Query` を使う app shell
- Sprite spawn, UI, ECS wiring
- crate 間の接着層

ここに残すもの:

- plugin 定義
- startup / visual / UI system
- ECS resource と shell system
- root 側の互換 re-export 層

### `hw_core`

役割:

- ドメイン横断で使う基礎型
- 安定した enum / message / relationship / constants

代表例:

- `constants`
- `game_state`
- `relationships`
- `events`
- `AssignedTask`
- `WorkType`
- `ResourceType`
- `DoorState`

### `hw_world`

役割:

- world の純粋ロジック
- pathfinding, terrain, map helper, 座標変換
- AI helper が使用する read-only 空間トレイト

代表例:

- terrain / river / mapgen / borders / regrowth
- spawn grid helper
- `world_to_grid`, `grid_to_world`
- nearest walkable / river query
- `PathWorld` trait — `is_walkable` など通行判定 API（`WorldMap` の impl は root）
- `SpatialGridOps` trait — `get_nearby_in_radius` など空間グリッド read-only API（concrete resource の本体は `hw_spatial`）

ここに置かないもの:

- `Commands` を使う sprite spawn
- `GameAssets` 依存の texture 選択
- `WorldMap` resource そのもの
- `SpatialGrid` resource 実体と update system（7 種 concrete）は `hw_spatial` が保持

### `hw_spatial`

役割:

- SpatialGrid の concrete resource / update 系（7 種）
- `GridData` と空間検索ヘルパの共通化
- 2D 空間スナップショットの初期化時の query 補助

ここに置くもの:

- `SpatialGrid`, `FamiliarSpatialGrid`, `BlueprintSpatialGrid`, `DesignationSpatialGrid`, `ResourceSpatialGrid`, `StockpileSpatialGrid`, `TransportRequestSpatialGrid`

ここに置かないもの:

- `GatheringSpotSpatialGrid`, `FloorConstructionSpatialGrid`
- root `WorldMap` shell、`WorldMapRead/Write`、startup/wiring

### `hw_logistics`

役割:

- 物流の共有 model / helper
- transport request の共有状態

代表例:

- `ResourceItem`, `Wheelbarrow`, `Stockpile`
- water helper
- ground resource helper
- `TransportRequest*`
- transport metrics / state sync

ここに置かないもの:

- producer plugin
- request lifecycle shell
- app 固有の orchestration

### `hw_jobs`

役割:

- jobs の共有 model
- building / blueprint / designation 系の基礎型

代表例:

- `BuildingType`, `Building`, `Blueprint`
- `Designation`, `Priority`, `TaskSlots`
- `MudMixerStorage`

ここに置かないもの:

- floor / wall construction system
- building completion shell
- door system

## 4. どこに置くかの判断基準

### `hw_core` に置く

- 複数ドメインから参照される
- Bevy app shell から独立している
- 安定した基礎型として使いたい

### `hw_world` に置く

- world/map/pathfinding の純粋ロジック
- `WorldMap` を trait や引数で抽象化できる
- `Commands` や asset に依存しない

### `hw_logistics` に置く

- transport / stockpile / resource 搬送の共有型
- producer 間で共通に使う helper
- app shell がなくても意味がある

### `hw_jobs` に置く

- building / designation / blueprint の基礎 model
- 複数 system から広く参照される component
- construction shell ではなく shared state として再利用される

### root (`bevy_app`) に残す

- Bevy system registration が主責務
- `Commands` / asset / UI / plugin order に強く依存する
- app shell としての意味が大きい

## 5. compatibility layer の扱い

分割後すぐに import path を全面変更しない場合、root 側に互換 re-export を置いてよいです。

例:

- `src/systems/jobs/mod.rs` -> `pub use hw_jobs::model::*;`
- `src/systems/logistics/types.rs` -> `pub use hw_logistics::types::*;`
- `src/world/river.rs` -> `pub use hw_world::river::*;`

ルール:

- root wrapper は薄く保つ
- wrapper に独自ロジックを足し始めたら責務を見直す
- 参照がなくなった re-export は削除する

## 6. `WorldMap` の境界

`WorldMap` は root crate に残す resource です。

`WorldMap` の責務:

- terrain / tile entity / building / stockpile / obstacle の状態保持
- occupancy / footprint / door / stockpile の更新 API
- Bevy resource としての公開面

`hw_world` 側へ寄せる責務:

- 座標変換
- pathfinding
- terrain 判定
- nearest walkable / river helper
- mapgen / border / regrowth の純粋ロジック

`src/world/map/spawn.rs`, `src/world/map/terrain_border.rs`, `src/world/regrowth.rs` は app shell です。これらは `GameAssets`, `Commands`, `Resource` を扱い、純粋ロジックは `hw_world` から呼び出します。

## 7. crate を増やすときの手順

1. `crates/<name>/Cargo.toml` と `src/lib.rs` を作る
2. root `Cargo.toml` に path dependency を追加する
3. shared model / helper から移す
4. root 側を re-export または import 修正でつなぐ
5. `cargo check --workspace` を通す
6. docs の責務表と `docs/README.md` を更新する

## 8. 検証コマンド

全体確認:

```bash
cargo check --workspace
```

timing 記録:

```bash
cargo check --workspace --timings
```

root app 起動:

```bash
cargo run
```

個別 crate の確認:

```bash
cargo check -p hw_core
cargo check -p hw_world
cargo check -p hw_logistics
cargo check -p hw_jobs
cargo check -p hw_ai
```

## 9. やらないこと

- `jobs` / `logistics` / `world` / `UI` を一度に全部分割する
- 広すぎる共通 crate に型をまとめて押し込む
- root wrapper に再びロジックを戻す
- `cargo check` を通さずに crate 分割を進める
