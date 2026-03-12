# アーキテクチャ維持前提の追加クレート化計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `architecture-safe-crate-extraction-plan-2026-03-12` |
| ステータス | `In Progress` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `AI (Codex)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- **解決したい課題**: workspace 方針では root crate を app shell に寄せるとしている一方、実コード上はすでに blocker が解消済みなのに root に残っている実装がある。
- **到達したい状態**: `bevy_app` には `GameAssets`・`app_contexts`・`NextState<PlayMode>`・`WorldMapWrite` を伴う adapter だけを残し、純ロジック寄りの残存実装を既存 crate (`hw_spatial`, `hw_logistics`, `hw_visual`) に移す。
- **成功指標**:
  - `FloorConstructionSpatialGrid` の定義・system が `hw_spatial` に移る
  - floor / wall の construction transport producer と plugin が `hw_logistics` に移る
  - floor / wall の construction visual system が `hw_visual` に移る
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功する

## 2. スコープ

### 対象（In Scope）

- `src/systems/spatial/floor_construction.rs` → `crates/hw_spatial/src/floor_construction.rs` への移設（M1）
- `src/systems/logistics/transport_request/plugin.rs` と `producer/{floor_construction,wall_construction}.rs` → `hw_logistics` への移設（M2）
- `src/systems/visual/{floor_construction,wall_construction}.rs` → `hw_visual` への移設（M3）
- crate 境界の変更に伴う `docs/cargo_workspace.md`, `docs/architecture.md`, `src/systems/*/README.md` の同期

### 非対象（Out of Scope）

- `src/app_contexts.rs` の分離
- `src/interface/selection/*` と `src/interface/ui/interaction/intent_handler.rs` の crate 化
- `src/systems/visual/placement_ghost.rs` と `src/systems/visual/task_area_visual.rs` の移設
- `GameAssets` の所有先変更
- 新規 crate の追加

## 3. 現状とギャップ

### 現在のクレート依存グラフ

```
bevy_app (root)
  ├── hw_core
  ├── hw_world
  ├── hw_jobs       ← FloorConstructionSite, WallConstructionSite, FloorTileBlueprint, WallTileBlueprint
  ├── hw_logistics  ← TransportRequest, Stockpile, floor/wall demand helpers
  ├── hw_spatial    ← SpatialGrid, Blueprint/Soul/Familiar/Stockpile/TransportRequest grids (※floor_construction は未登録)
  ├── hw_visual     ← HwVisualPlugin, handles, blueprint visual (※floor/wall construction visual は未移設)
  ├── hw_ui
  └── hw_ai
```

```
hw_spatial  →  hw_core, hw_jobs, hw_world
hw_logistics →  hw_core, hw_world, hw_jobs, hw_spatial, rand
hw_visual    →  hw_core, hw_jobs, hw_logistics, hw_spatial, hw_world, hw_ui, rand
```

### 現在の問題箇所

| ファイル | 問題 | 移設先 |
| --- | --- | --- |
| `src/systems/spatial/floor_construction.rs` | `FloorConstructionSpatialGrid` と update system が root に残留 | `crates/hw_spatial/src/floor_construction.rs` |
| `src/systems/logistics/transport_request/plugin.rs` | `FloorWallTransportPlugin` が root に残留 | `crates/hw_logistics/src/transport_request/plugin.rs` |
| `src/systems/logistics/transport_request/producer/floor_construction.rs` | 3 systems が root に残留 (`floor_construction_auto_haul_system`, `floor_material_delivery_sync_system`, `floor_tile_designation_system`) | `crates/hw_logistics/src/transport_request/producer/` |
| `src/systems/logistics/transport_request/producer/floor_construction/designation.rs` | floor tile designation system のサブモジュールが root に残留 | 同上 |
| `src/systems/logistics/transport_request/producer/wall_construction.rs` | 3 systems が root に残留（wall 版） | 同上 |
| `src/systems/visual/floor_construction.rs` | 4 systems が root に残留（visual） | `crates/hw_visual/src/floor_construction.rs` |
| `src/systems/visual/wall_construction.rs` | 3 systems が root に残留（visual） | `crates/hw_visual/src/wall_construction.rs` |

### すでに解消済みの前提条件

- `FloorConstructionSite` / `WallConstructionSite` / `FloorTileBlueprint` / `WallTileBlueprint` は `hw_jobs::construction` に存在 ✅
- `floor_site_tile_demand`, `wall_site_tile_demand` などの需要計算 helper は `hw_logistics` に存在 ✅
- `hw_visual` はすでに `hw_jobs`, `hw_logistics`, `hw_spatial` に依存済みのため、floor/wall visual 移設で新規依存追加は不要 ✅
- `hw_logistics` はすでに `hw_spatial` に依存済みのため、M1 完了後すぐに M2 に着手可能 ✅

### docs とのズレ

- `docs/cargo_workspace.md` に「floor/wall construction は M_extra（追加移設候補）」と記載されているが、実際は blocker が解消済みで移設可能な状態。M3 完了後に記述を更新する。

## 4. 実装方針（高レベル）

- **方針**:
  1. `FloorConstructionSpatialGrid` を `hw_spatial` へ移す（M1）―― M2 の依存前提
  2. construction transport producer 群を `hw_logistics` へ移す（M2）――  `FloorConstructionSpatialGrid` が hw_spatial にある前提
  3. construction visual を `hw_visual` へ移す（M3）―― GameAssets 非依存を事前確認
  4. backlog を評価・docs 更新（M4）
- **設計上の前提**:
  - root 側が保持すべきもの: `GameAssets`, `BuildContext`, `ZoneContext`, `TaskContext`, `NextState<PlayMode>`, `WorldMapWrite`, camera / UI input
  - `hw_*` crate 側には root への逆依存を持ち込まない
  - 型定義と主要 `impl` は同一 crate に置く
- **Bevy 0.18 での注意点**:
  - system の二重登録を避け、所有 crate の Plugin を唯一の登録元にする
  - `Resource` / `Component` の移設時は `定義 → plugin wiring → 使用側` の順に更新する
  - `RemovedComponents`, `ChildOf`, `Message` の schedule 順序は既存 chain を壊さない
  - `TransportRequestSet::Decide` / `GameSystemSet::Logic` などの SystemSet は hw_logistics / root 双方で参照するため import path に注意する

## 5. マイルストーン

### M1: `FloorConstructionSpatialGrid` を `hw_spatial` へ移す

**前提チェック（着手前に確認）**

- `hw_spatial/src/lib.rs` に `floor_construction` が pub mod として未登録であることを確認
- `src/systems/spatial/floor_construction.rs` に `FloorConstructionSpatialGrid` と `update_floor_construction_spatial_grid_system` が存在することを確認
- `src/plugins/spatial.rs` が `update_floor_construction_spatial_grid_system` を root モジュールから直接 import していることを確認

**変更内容**

1. `crates/hw_spatial/src/floor_construction.rs` を新規作成し、`FloorConstructionSpatialGrid` 定義・`SpatialGridOps` impl・`update_floor_construction_spatial_grid_system` をそのままコピー
2. `crates/hw_spatial/src/lib.rs` に `pub mod floor_construction;` と `pub use floor_construction::*;` を追加
3. `src/systems/spatial/floor_construction.rs` を薄い re-export に変更（`pub use hw_spatial::floor_construction::*;`）または削除し、`src/systems/spatial/mod.rs` の参照を更新
4. `src/plugins/spatial.rs` の import を `hw_spatial::` 経由に変更

**変更ファイル**

| ファイル | 変更種別 |
| --- | --- |
| `crates/hw_spatial/src/floor_construction.rs` | 新規作成 |
| `crates/hw_spatial/src/lib.rs` | `pub mod floor_construction` 追加 |
| `src/systems/spatial/floor_construction.rs` | re-export に縮退 or 削除 |
| `src/systems/spatial/mod.rs` | 参照更新 |
| `src/plugins/spatial.rs` | import 変更 |
| `src/systems/spatial/README.md` | 現状反映 |
| `docs/architecture.md` | spatial grid 節の更新 |
| `docs/cargo_workspace.md` | hw_spatial 担当範囲の更新 |

**完了条件**

- `FloorConstructionSpatialGrid` の型定義が root から消える
- `SpatialPlugin` が `hw_spatial::floor_construction::*` のみを参照する
- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功する

---

### M2: floor / wall construction transport producer を `hw_logistics` へ移す

**前提チェック（着手前に確認）**

- M1 完了済みであること（`FloorConstructionSpatialGrid` が `hw_spatial` に存在する）
- `src/systems/logistics/transport_request/producer/floor_construction.rs` に `floor_construction_auto_haul_system`, `floor_material_delivery_sync_system`, `floor_tile_designation_system` が存在することを確認
- `src/systems/logistics/transport_request/producer/floor_construction/designation.rs` の有無を確認し、移設対象に含める
- `FloorWallTransportPlugin` が `src/systems/logistics/transport_request/plugin.rs` にのみ定義されていることを確認
- `hw_logistics::transport_request::plugin.rs` が既存 plugin 定義（`TransportRequestPlugin`）と干渉しないことを確認

**変更内容**

1. `crates/hw_logistics/src/transport_request/producer/floor_construction.rs` を新規作成し、root 側の 3 system と `designation.rs` サブモジュールを移植
2. `crates/hw_logistics/src/transport_request/producer/wall_construction.rs` を新規作成し、wall 版 3 system を移植
3. `crates/hw_logistics/src/transport_request/producer/mod.rs` に `pub mod floor_construction; pub mod wall_construction;` を追加
4. `crates/hw_logistics/src/transport_request/plugin.rs` に `FloorWallTransportPlugin` 定義を追加（または既存 `TransportRequestPlugin::build` 内に統合）
5. root 側 `src/systems/logistics/transport_request/plugin.rs` を re-export shell に変更
6. root 側 producer ファイル群を削除または re-export に縮退

**変更ファイル**

| ファイル | 変更種別 |
| --- | --- |
| `crates/hw_logistics/src/transport_request/producer/floor_construction.rs` | 新規作成 |
| `crates/hw_logistics/src/transport_request/producer/floor_construction/designation.rs` | 新規作成（サブモジュールがある場合） |
| `crates/hw_logistics/src/transport_request/producer/wall_construction.rs` | 新規作成 |
| `crates/hw_logistics/src/transport_request/producer/mod.rs` | mod 追加 |
| `crates/hw_logistics/src/transport_request/plugin.rs` | FloorWallTransportPlugin 追加 |
| `src/systems/logistics/transport_request/plugin.rs` | re-export に縮退 |
| `src/systems/logistics/transport_request/producer/{floor,wall}_construction.rs` | 削除 or re-export |
| `src/systems/logistics/README.md` | 現状反映 |
| `docs/architecture.md` | logistics 節の更新 |
| `docs/cargo_workspace.md` | hw_logistics 担当範囲の更新 |
| `docs/logistics.md` | transport producer 節の更新 |

**完了条件**

- `FloorWallTransportPlugin` の定義が `hw_logistics` に移る
- root 側の producer 実装本体が消える
- `TransportRequestSet::Decide` / `GameSystemSet::Logic` の schedule が継続して機能する
- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功する

---

### M3: floor / wall construction visual を `hw_visual` へ移す

**前提チェック（着手前に確認）**

- `src/systems/visual/floor_construction.rs` の各 system（`update_floor_tile_visuals_system`, `manage_floor_curing_progress_bars_system`, `update_floor_curing_progress_bars_system`, `sync_floor_tile_bone_visuals_system`）が `GameAssets` を直接参照していないことを確認。参照している場合は M3 をブロックし、`GameAssets` の移設または依存除去を先行タスクとして立てる。
- `src/systems/visual/wall_construction.rs` の各 system についても同様に確認
- `hw_visual` の既存 handle（`WallVisualHandles`, `BuildingAnimHandles` など）が bone / wood sprite を保持しているか確認し、不足なら hw_visual 側で handle を追加定義する
- `src/plugins/visual.rs` が floor/wall visual system を `.chain()` で登録している箇所を特定し、`HwVisualPlugin` 側の ordering に影響しないことを確認

**変更内容**

1. `crates/hw_visual/src/floor_construction.rs` を新規作成し、4 system を移植
2. `crates/hw_visual/src/wall_construction.rs` を新規作成し、3 system を移植
3. `crates/hw_visual/src/lib.rs` の `HwVisualPlugin::build` に両モジュールの system 登録を追加（`GameSystemSet::Visual` set に帰属させる）
4. `src/plugins/visual.rs` から floor/wall construction system の登録を削除。残すのは `placement_ghost`, `task_area_visual`, `DebugVisible` gating など root 固有のものだけ
5. `src/systems/visual/floor_construction.rs` と `wall_construction.rs` を削除または re-export に縮退

**変更ファイル**

| ファイル | 変更種別 |
| --- | --- |
| `crates/hw_visual/src/floor_construction.rs` | 新規作成 |
| `crates/hw_visual/src/wall_construction.rs` | 新規作成 |
| `crates/hw_visual/src/lib.rs` | mod 追加 + system 登録 |
| `src/systems/visual/floor_construction.rs` | 削除 or re-export に縮退 |
| `src/systems/visual/wall_construction.rs` | 削除 or re-export に縮退 |
| `src/plugins/visual.rs` | 登録削除（root 固有のみ残す） |
| `src/systems/visual/README.md` | 現状反映 |
| `docs/architecture.md` | visual 節の更新 |
| `docs/cargo_workspace.md` | hw_visual 担当範囲の更新 |

**完了条件**

- construction visual system が `HwVisualPlugin` 配下で登録される
- root 側 visual モジュールが shell に縮退する
- curing / progress bar / bone visual が正常に表示されることを手動確認
- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功する

---

### M4: 二次候補の評価と backlog 化

**変更内容**

- `Room` model / overlay、`GameTime` 分離、`menu_actions` のような小粒候補を再評価し、次計画に切り出す
- 今回の main path からは外すが、`root adapter` と `shared model` の線引きをドキュメント化する
- `docs/cargo_workspace.md` の「M_extra」節を実態に合わせて更新する

**変更ファイル**

| ファイル | 変更種別 |
| --- | --- |
| `docs/cargo_workspace.md` | M_extra 節の更新 |
| `docs/architecture.md` | root 残留方針の更新 |

**完了条件**

- 今回やらない候補と理由が docs 上で明文化される

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `hw_spatial` へ construction grid を移す際に type path が大量に変わる | 中 | M1 は re-export を一時的に残し、import 差し替えを段階化する |
| transport producer 移設で system 登録元が二重化する | 高 | plugin 所有先を `hw_logistics` に一本化し、root 側は re-export のみ残す |
| visual 移設で `VisualPlugin` の `.chain()` 順序が壊れる | 中 | `HwVisualPlugin` の登録順を維持し、root 側は app-context 固有 system だけを残す |
| `sync_floor_tile_bone_visuals_system` が `GameAssets` を参照している | 高 | M3 前提チェックで確認。依存していた場合は handle を hw_visual 側に追加定義するか、M3 をブロックして先行タスクを立てる |
| M2 で `TransportRequestSet::Decide` の import path が変わりコンパイルエラーになる | 中 | hw_logistics の `transport_request/plugin.rs` で SystemSet の use 宣言を明示する |
| docs が古い前提のまま残る | 中 | 各マイルストーン完了時に `docs/cargo_workspace.md`, `docs/architecture.md`, README を同時更新する |

## 7. 検証計画

- **必須（各マイルストーン完了後）**:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- **手動確認シナリオ（M3 完了後）**:
  - floor / wall construction を含むシーンで request 発行と tile designation が継続動作する
  - construction visual が表示され、curing / progress bar / bone visual が崩れない
- **パフォーマンス確認（必要時）**:
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`
  - transport producer の実行時間に明確な悪化がないことを確認する

## 8. ロールバック方針

- M1, M2, M3 をそれぞれ独立コミットにする
- 直前 milestone 単位で `git revert` する。docs も同じコミットで巻き戻す
- re-export shell を残す期間は API path を維持し、途中ロールバックを容易にする

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M4 未着手

### 次のAIが最初にやること

1. `src/systems/spatial/floor_construction.rs` と `crates/hw_spatial/src/lib.rs` を読んで M1 の前提チェックを実行する
2. M1 を実施: `crates/hw_spatial/src/floor_construction.rs` を新規作成 → lib.rs に登録 → root 側を re-export/削除 → `src/plugins/spatial.rs` の import を更新
3. `cargo check --workspace` で M1 を確認し、M2 へ進む
4. M2 前に `src/systems/logistics/transport_request/producer/floor_construction.rs` の system 一覧と `GameAssets` 依存の有無を確認する
5. M3 前に `src/systems/visual/floor_construction.rs` の `sync_floor_tile_bone_visuals_system` が `GameAssets` を使っているか確認する（最重要ブロッカー確認）

### 主要な型・関数の対応表

| 型 / 関数 | 現在地 | 移設先 |
| --- | --- | --- |
| `FloorConstructionSpatialGrid` | `src/systems/spatial/floor_construction.rs` | `crates/hw_spatial/src/floor_construction.rs` |
| `update_floor_construction_spatial_grid_system` | 同上 | 同上 |
| `FloorWallTransportPlugin` | `src/systems/logistics/transport_request/plugin.rs` | `crates/hw_logistics/src/transport_request/plugin.rs` |
| `floor_construction_auto_haul_system` | `src/systems/logistics/transport_request/producer/floor_construction.rs` | `crates/hw_logistics/src/transport_request/producer/floor_construction.rs` |
| `floor_material_delivery_sync_system` | 同上 | 同上 |
| `floor_tile_designation_system` | `src/systems/logistics/transport_request/producer/floor_construction/designation.rs` | 同上 |
| `wall_construction_auto_haul_system` | `src/systems/logistics/transport_request/producer/wall_construction.rs` | `crates/hw_logistics/src/transport_request/producer/wall_construction.rs` |
| `update_floor_tile_visuals_system` | `src/systems/visual/floor_construction.rs` | `crates/hw_visual/src/floor_construction.rs` |
| `manage_floor_curing_progress_bars_system` | 同上 | 同上 |
| `update_floor_curing_progress_bars_system` | 同上 | 同上 |
| `sync_floor_tile_bone_visuals_system` | 同上 | 同上（GameAssets 依存要確認） |
| `update_wall_tile_visuals_system` | `src/systems/visual/wall_construction.rs` | `crates/hw_visual/src/wall_construction.rs` |
| `manage_wall_progress_bars_system` | 同上 | 同上 |
| `update_wall_progress_bars_system` | 同上 | 同上 |

### ブロッカー/注意点

- `app_contexts`、`GameAssets`、`NextState<PlayMode>`、`WorldMapWrite` を伴う層は今回の対象外
- `placement_ghost` と `task_area_visual` を一緒に動かそうとすると計画が膨らむので切り離す
- `sync_floor_tile_bone_visuals_system` が `GameAssets` に依存している場合は M3 をブロックし、先行タスクとして handle の hw_visual 化を別計画に立てること
- `TransportRequestSet::Decide` の SystemSet import は hw_logistics 側で解決する（root の `GameSystemSet` への逆依存に注意）
- docs に残っている「root 残留前提」は実コードとズレている箇所があるため、実装後は必ず更新する

### 参照必須ファイル（着手前に必ず読む）

- `docs/DEVELOPMENT.md`
- `docs/cargo_workspace.md`
- `docs/architecture.md`
- `crates/hw_jobs/src/construction.rs` — 型定義の確認
- `crates/hw_spatial/src/lib.rs` — 現在の pub mod 一覧
- `src/systems/spatial/floor_construction.rs` — 移設対象の確認
- `src/plugins/spatial.rs` — import 変更箇所の確認
- `src/systems/logistics/transport_request/plugin.rs` — FloorWallTransportPlugin の確認
- `src/systems/logistics/transport_request/producer/` — producer ファイル一覧の確認
- `src/plugins/visual.rs` — visual system 登録箇所の確認
- `src/systems/visual/floor_construction.rs` — GameAssets 依存の有無を確認

### 最終確認ログ

- 最終 `cargo check`: `2026-03-12` / `pass`
- 未解決エラー: なし

### Definition of Done

- [ ] M1〜M3 が完了している
- [ ] 関連 docs が更新済み
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `AI (Codex)` | 初版作成 |
| `2026-03-12` | `AI (GitHub Copilot)` | コードベース調査に基づき全面ブラッシュアップ：クレート依存グラフ追加、現在の問題箇所の表を追加、各 M に前提チェック・実装ステップ・変更ファイル表を追加、主要型・関数の対応表を追加、リスク追加（GameAssets 依存・SystemSet import）、cargo コマンドを CARGO_HOME 付きに統一 |
