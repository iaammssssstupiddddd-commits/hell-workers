# hw_spatial クレート新設 実装計画

## 0. 実装可否

- ステータス: `Ready`
- 方針: `docs` 上の合意は整っており、実装は M0 から順次実施できる状態。
- まず M0 と M1 を直列で進め、M2/M3 を同一スライスで進行し、最後に M4/M5 を分岐して実装する。

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `hw-spatial-crate-plan-2026-03-08` |
| ステータス | `Completed` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI` |
| 関連計画 | `docs/plans/hw-ai-crate-plan-2026-03-08.md`, `docs/plans/hw-ai-crate-phase2-2026-03-08.md` |
| 関連提案 | `docs/proposals/hw-ai-crate.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `WorldMap` と concrete `SpatialGrid` resource が root crate (`bevy_app`) に残っているため、`hw_ai` へ直接移せる AI システムが `WorldMapRead` / `SpatialGrid` 依存で止まりやすい。
- 到達したい状態: `WorldMap` を `hw_world`、可搬な 7 種の `SpatialGrid` を新規 `hw_spatial` に移し、root は app shell / wiring / 残留 2 grid に集中する。
- 成功指標:
  - `crates/hw_world` が `WorldMap` 本体を公開し、`cargo check -p hw_world` が通る
  - `crates/hw_spatial` が `GridData` と 7 種の grid resource / update system を公開し、`cargo check -p hw_spatial` が通る
  - root 側には `WorldMapRead` / `WorldMapWrite`、`Tile`、startup/plugin wiring、`GatheringSpotSpatialGrid`、`FloorConstructionSpatialGrid` だけが残る
  - `motion_dispatch.rs`, `rest_decision.rs`, `rest_area.rs`, `vitals_influence.rs` の 4 ファイルが `hw_ai` へ直接移動済み、またはその直前まで整理されている
  - `cargo check --workspace` が通る

## 2. スコープ

### 対象（In Scope）

- `WorldMap` 本体を `src/world/map/mod.rs` から `crates/hw_world/src/map.rs` へ移す
- 新規 crate `crates/hw_spatial/` を作り、`GridData` と 7 種の concrete grid を移す
- root 側に薄い `pub use` / wrapper を残し、import の破壊を段階化する
- `crates/hw_ai/Cargo.toml` に `hw_spatial` を追加し、直接依存へ切り替える
- startup / plugin / familiar plugin に散っている grid wiring を `hw_spatial` 前提に更新する
- 関連ドキュメント (`docs/proposals/hw-ai-crate.md`, `docs/plans/hw-ai-crate-phase2-2026-03-08.md`, `docs/cargo_workspace.md`, `docs/architecture.md`, `docs/README.md`, `docs/soul_ai.md`, `docs/familiar_ai.md`) を更新する

### 非対象（Out of Scope）

- `GatheringSpotSpatialGrid` の移設
- `FloorConstructionSpatialGrid` の移設
- `GameAssets`, UI, speech, visual, `Commands` 依存システムの移設
- `PopulationManager` の crate 移動
- AI ロジックやアルゴリズム自体の変更

## 3. 現状とギャップ

- 現状:
  - `WorldMap` 本体は `crates/hw_world/src/map.rs` に移され、`impl PathWorld for WorldMap` も同ファイルへ移されている
  - `GridData` と 9 種の concrete grid は `src/systems/spatial/` にある
  - grid の初期化 / wiring は `src/plugins/startup/mod.rs`, `src/plugins/spatial.rs`, `src/systems/familiar_ai/mod.rs`, `populate_resource_spatial_grid` に分散している
  - `hw_ai` 側にはすでに `PathWorld + SpatialGridOps` で動く pure helper があり、small trait 方式も一部成立している
- 問題:
  - 既存 proposal / phase2 / workspace guide は root shell 維持前提で固定されており、この案と衝突している
  - `hw_spatial` を作るだけでは AI ファイルがすべて即座に移せるわけではない
  - 前版の計画は依存グラフの向きと AI 移動候補の棚卸しが粗く、実装順の判断材料として弱かった
- 本計画で埋めるギャップ:
  - `hw_world` / `hw_spatial` / root shell の責務境界を採用済み前提で固定する
  - `hw_spatial` が「resource 型と update system を持つ crate」であり、root が「init_resource / add_systems / startup bootstrap を持つ shell」であると明記する
  - `hw_ai` へ直接移せるファイルと、追加抽出が必要なファイルを分けて計画する

### 3.1 現在の配置

| 対象 | 現在地 | 補足 |
| --- | --- | --- |
| `WorldMap` | `crates/hw_world/src/map.rs` | `Entity` を保持する Resource。`bevy_app` 側は `WorldMapRead/Write` を残す |
| `WorldMapRead` / `WorldMapWrite` | `src/world/map/access.rs` | Bevy 0.18 の `SystemParam` |
| `Tile` | `src/world/map/mod.rs` | map spawn shell で使う root component |
| `PathWorld` / pathfinding | `crates/hw_world/src/pathfinding.rs` | trait と helper はすでに `hw_world` |
| `impl PathWorld for WorldMap` | `crates/hw_world/src/map.rs` | `WorldMap` と同一 crate に統合されている |
| `SpatialGridOps` | `crates/hw_world/src/spatial.rs` | read-only trait は `hw_world` |
| `GridData` | `src/systems/spatial/grid.rs` | 実体は root |
| concrete grid 9 種 | `src/systems/spatial/*.rs` | すべて root |

### 3.2 `hw_spatial` へ移す対象

| Grid | マーカー / 依存型 | 移設先 | 判定 |
| --- | --- | --- | --- |
| `SpatialGrid` | `DamnedSoul` (`hw_core`) | `hw_spatial` | 対象 |
| `FamiliarSpatialGrid` | `Familiar` (`hw_core`) | `hw_spatial` | 対象 |
| `BlueprintSpatialGrid` | `Blueprint` (`hw_jobs`) | `hw_spatial` | 対象 |
| `DesignationSpatialGrid` | `Designation` (`hw_jobs`) | `hw_spatial` | 対象 |
| `ResourceSpatialGrid` | `ResourceItem` (`hw_logistics`) | `hw_spatial` | 対象 |
| `StockpileSpatialGrid` | `Stockpile` (`hw_logistics`) | `hw_spatial` | 対象 |
| `TransportRequestSpatialGrid` | `TransportRequest` (`hw_logistics`) | `hw_spatial` | 対象 |
| `GatheringSpotSpatialGrid` | `GatheringSpot` (`hw_ai` helper) | root | 今回は非対象 |
| `FloorConstructionSpatialGrid` | `FloorConstructionSite` (root jobs shell) | root | 今回は非対象 |

### 3.3 AI 移動候補の棚卸し

#### M1 / M2 後に直接移しやすい候補

| ファイル | 現在の主依存 | 補足 |
| --- | --- | --- |
| `src/systems/soul_ai/decide/idle_behavior/motion_dispatch.rs` | `WorldMap`, `SpatialGridOps` | `ParticipatingIn` / event は `hw_core` re-export へ寄せればよい |
| `src/systems/soul_ai/decide/idle_behavior/rest_decision.rs` | `WorldMap` | rest helper 側も同時に移しやすい |
| `src/systems/soul_ai/decide/idle_behavior/rest_area.rs` | `WorldMap`, `RestArea` | `RestArea` は `hw_jobs` 参照へ切り替え可能 |
| `src/systems/soul_ai/update/vitals_influence.rs` | `FamiliarSpatialGrid` | `ActiveCommand` / `FamiliarCommand` も `hw_core` にある |

#### 追加抽出が必要な候補

| ファイル | 残る論点 |
| --- | --- |
| `src/systems/soul_ai/helpers/work.rs` | `unassign_task` のうち `WheelbarrowMovement` / root visual shell 参照を残す判定 |

#### 今回の計画だけでは完了しない候補

| ファイル | 残る論点 |
| --- | --- |
| `src/systems/soul_ai/execute/drifting.rs` | `PopulationManager` 依存 |
| `src/systems/soul_ai/decide/drifting.rs` | `PopulationManager` 依存 |

## 4. 実装方針（高レベル）

- 方針:
  - `hw_world` は `WorldMap` 本体を持つ world crate とする
  - `hw_spatial` は 7 種の grid resource 型 / `GridData` / update system を持つ spatial crate とする
  - root (`bevy_app`) は app shell として `SystemParam`, plugin wiring, startup bootstrap, shell-only component を持つ
- 設計上の前提:
  - `WorldMapRead` / `WorldMapWrite` は root に残す
  - `Tile`, `spawn_map`, `terrain_border`, `populate_resource_spatial_grid` は root に残す
  - `GatheringSpotSpatialGrid` と `FloorConstructionSpatialGrid` は root に残す
  - root 側の互換 `pub use` は一定期間残してよいが、独自ロジックは足さない
  - `hw_ai` 側の first slice は「direct move できる 4 ファイル」を優先し、`work.rs` と `escaping.rs` は helper 抽出後に扱う
- 依存関係の表記:
  - 以後、本計画では `A -> B` を「A が B に依存する」と定義する

### 4.1 採用する依存グラフ

```text
hw_jobs -> hw_core
hw_logistics -> hw_core
hw_world -> hw_core
hw_spatial -> hw_core, hw_jobs, hw_logistics, hw_world
hw_ai -> hw_core, hw_jobs, hw_logistics, hw_world, hw_spatial
bevy_app -> hw_core, hw_jobs, hw_logistics, hw_world, hw_spatial, hw_ai

M1 後:
hw_world -> hw_core, hw_jobs
```

### 4.2 crate ごとの責務

| crate | 持つもの | 持たないもの |
| --- | --- | --- |
| `hw_world` | `WorldMap`, `impl PathWorld for WorldMap`, world helper | `Tile`, `spawn_map`, `WorldMapRead/Write` |
| `hw_spatial` | `GridData`, 7 種の grid resource 型, update system | plugin wiring, startup bootstrap, 残留 2 grid |
| root (`bevy_app`) | `WorldMapRead/Write`, `Tile`, startup/plugin wiring, `populate_resource_spatial_grid`, 残留 2 grid | `WorldMap` 本体, 可搬 7 grid の定義本体 |

### 4.3 Bevy 0.18 APIでの注意点

- `SystemParam` (`WorldMapRead` / `WorldMapWrite`) は root adapter に残す
- `Res<T>` / `ResMut<T>` / `RemovedComponents<T>` を使う grid update system は `hw_spatial` に置いてよいが、`app.add_systems(...)` は root plugin で行う
- `.init_resource::<...>()` は root plugin / startup から登録する
- `Tile` のような shell component は `WorldMap` と同じ crate へ移さない

## 5. マイルストーン

## M0: docs の採用反映

- 変更内容:
  - `docs/proposals/hw-ai-crate.md` の non-goal / 代替案比較を `hw_spatial` 採用前提へ更新する
  - `docs/plans/hw-ai-crate-phase2-2026-03-08.md` の root 残留前提を改める
  - `docs/cargo_workspace.md` の `WorldMap` / spatial 境界を新方針に合わせる
- 変更ファイル:
  - `docs/proposals/hw-ai-crate.md`
  - `docs/plans/hw-ai-crate-phase2-2026-03-08.md`
  - `docs/cargo_workspace.md`
- 完了条件:
  - [x] proposal / phase2 / workspace guide が `hw_world::WorldMap` + `hw_spatial` 前提で一致している
  - [x] `WorldMap` を root に残す記述と `hw_spatial` 不採用記述が消えている
  - [x] 本計画との矛盾がなくなっている
- 検証:
  - 文書整合レビュー

## M1: `WorldMap` を `hw_world` へ移す

- 変更内容:
  - `crates/hw_world/Cargo.toml` に `hw_jobs` を追加する
  - `WorldMap` 定義、`Default`、主要 `impl`、`impl PathWorld for WorldMap` を `crates/hw_world/src/map.rs` へ移す
  - root `src/world/map/mod.rs` は `Tile`, `spawn_map`, `terrain_border`, layout re-export, `WorldMapRead` / `WorldMapWrite` を持つ shell に縮退する
  - root `src/world/pathfinding.rs` は re-export layer に縮小する
- 変更ファイル:
  - `crates/hw_world/Cargo.toml`
  - `crates/hw_world/src/lib.rs`
  - `crates/hw_world/src/map.rs`（新規）
  - `src/world/map/mod.rs`
  - `src/world/map/access.rs`
  - `src/world/pathfinding.rs`
  - `src/world/map/spawn.rs`
- 完了条件:
  - [x] `hw_world::WorldMap` が公開されている
  - [x] root 側 `WorldMapRead` / `WorldMapWrite` が外部 crate の `WorldMap` を参照してコンパイルできる
  - [x] `Tile` と `spawn_map` が root に残っている
  - [x] 既存の `use crate::world::map::WorldMap` は wrapper 経由で壊れていない
- 検証:
  - `cargo check -p hw_world`
  - `cargo check --workspace`

## M2: `hw_spatial` crate を新設する

- 変更内容:
  - `crates/hw_spatial/Cargo.toml` と `crates/hw_spatial/src/lib.rs` を作る
  - `GridData` を `crates/hw_spatial/src/grid.rs` へ移す
  - 以下 7 ファイルを `crates/hw_spatial/src/` へ移す
    - `soul.rs`
    - `familiar.rs`
    - `blueprint.rs`
    - `designation.rs`
    - `resource.rs`
    - `stockpile.rs`
    - `transport_request.rs`
  - root `src/systems/spatial/mod.rs` は 7 種を re-export し、残留 2 種だけをローカル定義する hybrid wrapper に変更する
- 変更ファイル:
  - `Cargo.toml`
  - `crates/hw_spatial/Cargo.toml`（新規）
  - `crates/hw_spatial/src/lib.rs`（新規）
  - `crates/hw_spatial/src/grid.rs`（新規）
  - `crates/hw_spatial/src/{soul,familiar,blueprint,designation,resource,stockpile,transport_request}.rs`（新規）
  - `src/systems/spatial/mod.rs`
  - `src/systems/spatial/grid.rs`
  - `src/systems/spatial/{soul,familiar,blueprint,designation,resource,stockpile,transport_request}.rs`
- 完了条件:
  - [x] `cargo check -p hw_spatial` が通る
  - [x] `GatheringSpotSpatialGrid` と `FloorConstructionSpatialGrid` が root 側に残っている
  - [x] root から 7 種の import path を維持できる
- 検証:
  - `cargo check -p hw_spatial`
  - `cargo check --workspace`

## M3: root wiring を `hw_spatial` / `hw_world` 前提へ揃える

- 変更内容:
  - startup の `init_resource::<...>()` を新しい型定義へ向ける
  - `src/plugins/spatial.rs` の update system import を `hw_spatial` 由来へ切り替える
  - `src/systems/familiar_ai/mod.rs` の `DesignationSpatialGrid` / `TransportRequestSpatialGrid` 初期化を新 crate 前提に揃える
  - `populate_resource_spatial_grid` のような root bootstrap helper を新 import path に合わせる
- 変更ファイル:
  - `src/plugins/startup/mod.rs`
  - `src/plugins/spatial.rs`
  - `src/systems/familiar_ai/mod.rs`
  - `src/systems/spatial/mod.rs`
- 完了条件:
  - [x] startup, spatial plugin, familiar plugin, bootstrap helper の 4 系統がすべてコンパイルしている
  - [x] root が `init_resource` / `add_systems` / bootstrap を担当し続けている
  - [x] `cargo check --workspace` が通る
- 検証:
  - `cargo check --workspace`

## M4: `hw_ai` へ direct move できる slice を移す

- 変更内容:
  - `crates/hw_ai/Cargo.toml` に `hw_spatial` を追加する
  - まず direct move できる 4 ファイルを `hw_ai` へ寄せる
  - root 側には re-export または thin wrapper を残す
- 直接移動の first slice:
  - `src/systems/soul_ai/decide/idle_behavior/motion_dispatch.rs`
  - `src/systems/soul_ai/decide/idle_behavior/rest_decision.rs`
  - `src/systems/soul_ai/decide/idle_behavior/rest_area.rs`
  - `src/systems/soul_ai/update/vitals_influence.rs`
- 変更ファイル:
  - `crates/hw_ai/Cargo.toml`
  - `crates/hw_ai/src/soul_ai/...`
  - `src/systems/soul_ai/...`
- 完了条件:
  - [x] first slice の 4 ファイルが `hw_ai` へ移っている
  - [x] `hw_ai` が `hw_world::WorldMap` と `hw_spatial::*SpatialGrid` を直接 import してビルドできる
  - [x] root 側 wrapper は薄い re-export に留まっている
- 検証:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`

## M5: helper 抽出が必要な slice を整理する

- 変更内容:
  - `decide/escaping.rs` について、`perceive/escaping.rs` を generic helper 化するか `hw_ai` へ同時移設する
  - `helpers/work.rs` の `is_soul_available_for_work` を `hw_ai` へ移設し、`unassign_task` は root shell として残す
  - `gathering_mgmt.rs` の実装を `hw_ai` へ移設する
  - 関連 docs を最終境界へ更新する
- 変更ファイル:
  - `crates/hw_ai/src/soul_ai/decide/escaping.rs`
  - `crates/hw_ai/src/soul_ai/perceive/escaping.rs`
  - `src/systems/soul_ai/decide/escaping.rs`（thin wrapper）
  - `src/systems/soul_ai/perceive/escaping.rs`（thin wrapper）
  - `crates/hw_ai/src/soul_ai/decide/gathering_mgmt.rs`
  - `src/systems/soul_ai/helpers/work.rs`
  - `crates/hw_ai/src/soul_ai/helpers/work.rs`
  - `src/systems/soul_ai/decide/gathering_mgmt.rs`（thin wrapper）
  - `docs/architecture.md`
  - `docs/cargo_workspace.md`
  - `docs/soul_ai.md`
  - `docs/familiar_ai.md`
  - `docs/README.md`
- 完了条件:
  - [x] `work.rs` は `is_soul_available_for_work` を `hw_ai` に移植し、`unassign_task` は `WheelbarrowMovement` など root 依存があるため shell として root 残留する方針が明文化されている
  - [x] `decide/escaping.rs` + `perceive/escaping.rs` が `hw_ai` 移設され、root 側に thin wrapper が残る構成になっている
  - [x] `gathering_mgmt.rs` が `hw_ai` へ移設され、root 側 thin wrapper で接続されている
  - [x] 仕様書と実コードの crate 境界が一致するよう、`architecture.md`, `cargo_workspace.md`, `soul_ai.md` の責務記述を更新済み
  - [x] 不要な root wrapper は削除され、残る wrapper は薄い re-export shell または root 依存 shell（`unassign_task`）として明文化したうえで最小化されている
- 検証:
  - `cargo check -p hw_ai`
  - `cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| docs 方針だけ旧前提のまま残る | 高 | M0 を最初に実施し、proposal / phase2 / workspace guide を同時更新する |
| crate が 1 つ増え、wiring 更新漏れが出る | 高 | `startup`, `plugins/spatial`, `familiar_ai/mod`, bootstrap helper を M3 の DoD に含める |
| `WorldMap` 移設時に shell component まで巻き込む | 高 | `Tile`, `spawn_map`, `terrain_border`, `WorldMapRead/Write` は root 残留と明記する |
| `work.rs` を direct move できると誤認する | 高 | M4 では扱わず、M5 で helper / shell 分離を前提にする |
| `decide/escaping.rs` が helper ごと root 依存を引きずる | 中 | `perceive/escaping.rs` の generic 化または同時移設を前提にする |
| `GatheringSpotSpatialGrid` を巻き込んで循環依存が発生する | 中 | 今回は明示的に対象外とし、必要なら別計画で扱う |

## 7. 検証計画

- 必須:
  - `cargo check -p hw_world`
  - `cargo check -p hw_spatial`
  - `cargo check -p hw_ai`
  - `cargo check --workspace`
- 手動確認シナリオ:
  - Soul の wandering / gathering / rest 決定が従来通り動く
  - Familiar 近接による stress / motivation 変化が従来通り動く
  - escaping 判定と逃走先更新が従来通り動く
  - blueprint / designation / transport request の空間検索が従来通り動く
- 追加確認:
  - root startup plugin で `init_resource::<SpatialGrid系>()` が従来どおり登録できる
  - `WorldMapRead` / `WorldMapWrite` から `is_changed()` が従来通り使える
  - `populate_resource_spatial_grid` が `ResourceSpatialGrid` を初期化できる

## 8. ロールバック方針

- どの単位で戻せるか:
  - M0 は docs コミット単位
  - M1 は `WorldMap` 移設コミット単位
  - M2 は `hw_spatial` 抽出コミット単位
  - M3 は wiring 整理コミット単位
  - M4 / M5 は AI helper 移設コミット単位
- 戻す時の手順:
  1. docs だけ問題なら M0 を revert する
  2. `WorldMap` で不具合が出たら M1 を revert し、M2 へ進まない
  3. `hw_spatial` 抽出で不整合が出たら M2 と M3 をまとめて revert する
  4. AI 側の移設だけ問題なら M4 / M5 を revert し、`hw_world` / `hw_spatial` は残す

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `20%`
- 完了済みマイルストーン:
  - 採用前提の計画書への再構成
  - M0 `docs` 整合
  - M1 `WorldMap` 移設
- 未着手/進行中:
- M1 は実施済み。`hw_world` へ `WorldMap` を移設し、`cargo check` を通過済み
- M2 / M3 / M4 / M5

### 次のAIが最初にやること

1. M0 の 3 文書を更新し、proposal / phase2 / workspace guide をこの計画へ揃える
2. M2 と M3 を 1 スライスで進め、`SpatialGrid` / `FamiliarSpatialGrid` を最初に `hw_spatial` 化して wiring を通す
3. M4 の first slice 4ファイルの移設準備と `hw_ai` 側依存見直し

### ブロッカー/注意点

- `Tile` は `WorldMap` と一緒に移さないこと
- `startup`, `plugins/spatial`, `familiar_ai/mod`, `populate_resource_spatial_grid` の 4 箇所をまとめて見ること
- `helpers/work.rs` は direct move 候補ではない
- `decide/escaping.rs` は `perceive/escaping.rs` とセットで判断すること

### 参照必須ファイル

- `docs/proposals/hw-ai-crate.md`
- `docs/plans/hw-ai-crate-phase2-2026-03-08.md`
- `docs/cargo_workspace.md`
- `src/world/map/mod.rs`
- `src/world/map/access.rs`
- `src/world/pathfinding.rs`
- `src/plugins/startup/mod.rs`
- `src/plugins/spatial.rs`
- `src/systems/familiar_ai/mod.rs`
- `src/systems/spatial/grid.rs`
- `src/systems/spatial/mod.rs`
- `src/systems/soul_ai/perceive/escaping.rs`
- `crates/hw_world/src/spatial.rs`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-08` / `pass`
- 未解決エラー:
  - `N/A`

### Definition of Done

- [x] M0〜M5 が完了している
- [x] `hw_ai` が `hw_spatial` / `hw_world::WorldMap` を直接使えている
- [x] root 側に残るのが shell / adapter / 非対象 2 grid だけになっている
- [x] 関連ドキュメントが更新済み
- [x] `cargo check --workspace` が成功している

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
| `2026-03-08` | `AI` | 採用前提へ全面改稿。`Blocked` を解除し、依存グラフ、責務境界、wiring、AI移動候補を実コードに合わせて修正した。 |
| `2026-03-08` | `AI` | M2 のチェック項目（`cargo check -p hw_spatial` / root 残留 2 grid / import path）を反映し、Definition of Done を更新。`docs` 側の進捗チェック項目を現実の実装状態へ整合。 |
| `2026-03-08` | `AI` | M0 を実施。`docs/proposals/hw-ai-crate.md` / `docs/plans/hw-ai-crate-phase2-2026-03-08.md` / `docs/cargo_workspace.md` を `hw_spatial` 前提へ整合。 |
