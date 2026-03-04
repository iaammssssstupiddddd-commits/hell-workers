# 実装計画: Site / Yard システム

> 提案書: `docs/proposals/site-yard-system.md`

## 概要

4 フェーズの段階的実装。各 Phase は独立して動作確認可能。

---

## Phase 1: データモデルと初期配置

### 目的
`Site` / `Yard` コンポーネントを追加し、ゲーム開始時に自動配置する。既存の挙動は変えない。

### 手順

#### 1-1. 定数追加
**対象**: `src/constants/` に新規ファイル `world_zones.rs` を作成（または既存ファイルに追記）

```rust
pub const SITE_WIDTH_TILES: f32 = 40.0;
pub const SITE_HEIGHT_TILES: f32 = 20.0;
pub const YARD_MIN_WIDTH_TILES: f32 = 20.0;
pub const YARD_MIN_HEIGHT_TILES: f32 = 20.0;
pub const YARD_INITIAL_WIDTH_TILES: f32 = 20.0;
pub const YARD_INITIAL_HEIGHT_TILES: f32 = 20.0;
```

#### 1-2. コンポーネント定義
**新規**: `src/systems/world/zones.rs`（ファイル新規作成）

```rust
#[derive(Component, Clone, Debug)]
pub struct Site { pub min: Vec2, pub max: Vec2 }

#[derive(Component, Clone, Debug)]
pub struct Yard { pub min: Vec2, pub max: Vec2 }

/// Site-Yard ペアリング（将来の複数セット対応のために用意）
#[derive(Component)] pub struct PairedYard(pub Entity);
#[derive(Component)] pub struct PairedSite(pub Entity);
```

両者に `contains(pos: Vec2) -> bool` などのメソッドを実装。`TaskArea` の既存メソッドを参考にする。

#### 1-3. 初期配置システム
**対象**: `src/systems/logistics/initial_spawn.rs`

`initial_resource_spawner` に続けて呼ばれる `spawn_site_and_yard` 関数を追加。
- Site: マップ中央付近に配置（x: map_center - 20..+20, y: map_center - 10..+10）
- Yard: Site に隣接（Site の右辺 + 1 タイルの位置から 20×20）
- `PairedYard` / `PairedSite` で相互参照

`AppPlugin` または `world_plugin` の起動時システムに登録する。

#### 1-4. BelongsTo の Yard 対応（準備のみ）
**確認**: `src/systems/logistics/types.rs:54` の `BelongsTo(pub Entity)` はそのまま。
Phase 2 で Yard entity を引数に渡す変更をするが、型定義自体は変更不要。

### 検証
- `cargo check` が通ること
- ゲーム起動時に `Site` / `Yard` エンティティが spawn されること（`F12` デバッグで gizmo 表示が理想）

---

## Phase 2: Stockpile グルーピングの Yard 化

### 目的
`DepositToStockpile` と `ConsolidateStockpile` の Stockpile 探索を Familiar 単位 → Yard 単位に変更。
`BelongsTo` を Yard entity に統一。

### 手順

#### 2-1. `StockpileGroup` の `owner` を Yard entity に変更
**対象**: `src/systems/logistics/transport_request/producer/stockpile_group.rs`

```rust
// 変更前
pub struct StockpileGroup {
    pub owner_familiar: Entity,
    ...
}

// 変更後
pub struct StockpileGroup {
    pub owner_yard: Entity,  // Familiar entity → Yard entity
    ...
}
```

#### 2-2. `build_stockpile_groups` シグネチャ変更
**対象**: `src/systems/logistics/transport_request/producer/stockpile_group.rs:52`

```rust
// 変更前
pub fn build_stockpile_groups(
    stockpile_grid: &StockpileSpatialGrid,
    active_familiars: &[(Entity, TaskArea)],
    q_stockpiles: &Query<...>,
) -> Vec<StockpileGroup>

// 変更後
pub fn build_stockpile_groups(
    stockpile_grid: &StockpileSpatialGrid,
    yards: &[(Entity, Yard)],  // Familiar + TaskArea → Yard に変更
    q_stockpiles: &Query<...>,
) -> Vec<StockpileGroup>
```

内部の `get_in_area(area.min, area.max)` を `get_in_area(yard.min, yard.max)` に変更。

#### 2-3. `task_area_auto_haul_system` を Yard 単位に変更
**対象**: `src/systems/logistics/transport_request/producer/task_area.rs:182`

```rust
// 変更前 Query
q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>

// 変更後 Query（Yard をクエリする）
q_yards: Query<(Entity, &Yard)>
q_familiars: Query<(Entity, &ActiveCommand)>
```

`issued_by` を Yard entity に変更。`TransportRequest.issued_by` は Yard entity を指す。

#### 2-4. `stockpile_consolidation_producer_system` を Yard 単位に変更
**対象**: `src/systems/logistics/transport_request/producer/consolidation.rs:34`

`build_stockpile_groups` の引数変更に合わせて、`q_familiars` を `q_yards` に変更。

#### 2-5. `BelongsTo` の付与を Yard entity に変更
**対象**: Stockpile zone 配置時

Stockpile ゾーン配置のシステムで `BelongsTo(familiar_entity)` を `BelongsTo(yard_entity)` に変更。
- 現在 BelongsTo を付与している箇所を `grep -r "BelongsTo"` で洗い出し、
  Stockpile 系のものを Yard entity に変更する

#### 2-6. `task_area_auto_haul_system` のソース収集範囲の更新
**対象**: `src/systems/logistics/transport_request/producer/task_area.rs`

`find_nearest_group_for_item_indexed` がアイテムを各グループに紐付ける際の距離計算で
Yard の境界を基準にするよう変更（現状は TaskArea の外周距離で比較している）。

### 検証
- Stockpile を Yard 内に配置 → `DepositToStockpile` が 1 件のみ発行（Familiar 数に比例しない）
- 複数 Familiar の Soul が同一 Stockpile に搬入できる
- `TransportRequestMetrics` ログで `task_area_groups` 数が Familiar 数ではなく Yard 数であることを確認

---

## Phase 3: 資材探索フローの変更

### 目的
`blueprint_auto_haul_system` / `blueprint_auto_gather_system` / `mud_mixer_auto_haul_system` の
ソース探索を「TaskArea → +10/+30/+60 マージン → 全域」から「TaskArea → Yard → 全域」に変更。

### 手順

#### 3-1. `blueprint_auto_haul_system` のソース探索変更
**対象**: `src/systems/logistics/transport_request/producer/blueprint.rs:27`

```
変更前: find_owner_familiar(bp_pos, &active_familiars) → TaskArea から探す
変更後: find_owner_yard(bp_pos, &yards) OR TaskArea に位置するものは Familiar の Yard を参照

Blueprint は Site 内にある前提なので、Site から Yard を逆引きして Yard 内のソースを探す。
```

`find_owner_familiar` ヘルパー: `mod.rs:27` の引数を `(pos, familiars_with_task_areas, yards)` に拡張するか、
別ヘルパー `find_yard_for_site_pos(pos, sites, yards)` を追加する。

ソース探索の段階:
1. `StockpileSpatialGrid.get_in_area(task_area.min, task_area.max)` — TaskArea 内 Stockpile
2. `StockpileSpatialGrid.get_in_area(yard.min, yard.max)` — Yard 内 Stockpile
3. マップ全体の地面アイテム（現行フォールバック）

#### 3-2. `blueprint_auto_gather_system` の段階探索変更
**対象**: `src/systems/familiar_ai/decide/auto_gather_for_blueprint/`

現在の探索段階（Stage 0: TaskArea → Stage 1: +10 → Stage 2: +30 → Stage 3: +60 → Stage 4: 全域）を:

- Stage 0: TaskArea 内
- Stage 1: Yard 内
- Stage 2: マップ全体（到達可能）

に変更。各 Stage の「境界を計算する関数」を変更する。

#### 3-3. `mud_mixer_auto_haul_system` の探索変更
**対象**: `src/systems/logistics/transport_request/producer/mixer.rs:162-195`

```rust
// 変更前
let area_filter = task_area.contains_with_margin(pos, 10.0) && !other_area_contains...

// 変更後
let area_filter = yard.contains(pos)
```

Mixer は Yard 内にある前提なので、SandPile / Rock の探索も Yard 内から優先する。

#### 3-4. `task_finder` の Yard 内 TransportRequest 探索追加
**対象**: `src/systems/familiar_ai/decide/task_management/task_finder/filter.rs:23`

```rust
// 変更前
pub(super) fn collect_candidate_entities(
    task_area_opt: Option<&TaskArea>,
    ...
)

// 変更後
pub(super) fn collect_candidate_entities(
    task_area_opt: Option<&TaskArea>,
    yard_opt: Option<&Yard>,  // 追加
    ...
)
```

内部で `transport_request_grid.get_in_area(yard.min, yard.max)` を追加して
Yard 内の TransportRequest を候補に含める。

`FamiliarTaskAssignmentQueries` に Yard クエリを追加する必要がある。

### 検証
- Blueprint 配置 → Yard の Stockpile から資材が調達される
- Mixer が Yard 内にある場合、マージン計算なしで Sand/Rock が搬入される
- `blueprint_auto_gather_system` が TaskArea → Yard → 全域 の順で Tree/Rock を探す

---

## Phase 4: 配置制約と UI / ビジュアル

### 目的
- Structure 建築の Site 外配置禁止
- Plant/Temporary/Stockpile の Yard 外配置禁止
- Site/Yard の境界線表示
- TaskArea 編集の Site 内制約
- Yard 拡張 UI

### 手順

#### 4-1. 建築配置検証に Site/Yard 制約を追加
**対象**: `src/interface/selection/building_place/placement.rs:37-59`

`can_place` 判定に以下を追加:
```rust
// Structure 系（Wall, Floor, Bridge, Door）は Site 内のみ
if building_type.category() == BuildingCategory::Structure {
    if !site.contains(world_pos) { return false; }
}

// Plant + Temporary + Stockpile は Yard 内のみ
if matches!(building_type.category(), BuildingCategory::Plant | BuildingCategory::Temporary) {
    if !yard.contains(world_pos) { return false; }
}
```

`placement.rs` の `place_building_blueprint` は `site: &Site, yard: &Yard` を追加引数にするか、
`WorldMap` や `Res` 経由で参照できるようにする。

ゴースト表示の赤判定も同様に更新（placement ghost の色付けロジック）。

#### 4-2. Stockpile ゾーン配置の Yard 制約
**対象**: Stockpile zone 配置システム（`src/interface/selection/` 内）

Stockpile のドラッグ配置時に `yard.contains(pos)` チェックを追加。
Yard 外では配置ゴーストを赤表示。

#### 4-3. TaskArea 編集の Site 内制約
**対象**: `src/systems/command/area_selection/apply.rs`

TaskArea を適用する際に、`area.min >= site.min` かつ `area.max <= site.max` を強制。
はみ出す場合は Site の境界でクランプする。

#### 4-4. Site / Yard のビジュアル表示
**対象**: 新規ファイル `src/systems/visual/site_yard_visual.rs`（または既存の `task_area_visual.rs` を参考）

- 現在の `shaders/task_area.wgsl` を流用して Site / Yard 用シェーダーを作成
- Site: 石壁をイメージした灰色・茶色の境界線
- Yard: 設備エリアをイメージした青緑の境界線
- 表示システムを `VisualPlugin` に登録

#### 4-5. Yard 拡張 UI
**対象**: `src/interface/ui/` か `src/systems/command/` の新規モジュール

- `Zones` メニューに「Yard を拡張」操作を追加
- ドラッグで Yard の矩形を拡大（最小 20×20 の制約を維持）
- Site との重複チェック（重複する変更は拒否）

### 検証
- Structure 建築を Site 外にドラッグ → ゴーストが赤 + 配置拒否
- MudMixer を Yard 外に配置 → ゴーストが赤 + 配置拒否
- TaskArea を Site 外にドラッグ → Site 境界でクランプされる
- Site / Yard の境界線が画面上に表示される
- Yard をドラッグ拡張できる

---

## ファイル変更一覧

| ファイル | 変更種別 | Phase | 概要 |
|:---|:---|:---|:---|
| `src/constants/world_zones.rs` | 新規 | 1 | SITE_WIDTH/HEIGHT, YARD_MIN_* 定数 |
| `src/systems/world/zones.rs` | 新規 | 1 | Site, Yard, PairedYard, PairedSite コンポーネント |
| `src/systems/logistics/initial_spawn.rs` | 追記 | 1 | spawn_site_and_yard 関数追加 |
| `src/systems/logistics/transport_request/producer/stockpile_group.rs` | 変更 | 2 | build_stockpile_groups の引数を Yard に変更 |
| `src/systems/logistics/transport_request/producer/task_area.rs` | 変更 | 2 | クエリを Yard ベースに変更、issued_by を Yard entity に |
| `src/systems/logistics/transport_request/producer/consolidation.rs` | 変更 | 2 | Yard ベースのグルーピングに変更 |
| `src/systems/logistics/transport_request/producer/mod.rs` | 変更 | 2-3 | find_owner_familiar の更新 or 新 Yard ヘルパー追加 |
| `src/systems/logistics/transport_request/producer/blueprint.rs` | 変更 | 3 | ソース探索を TaskArea → Yard → 全域に変更 |
| `src/systems/logistics/transport_request/producer/mixer.rs` | 変更 | 3 | TaskArea マージン → Yard 内探索に変更 |
| `src/systems/familiar_ai/decide/auto_gather_for_blueprint/` | 変更 | 3 | 探索段階を TaskArea → Yard → 全域に再編 |
| `src/systems/familiar_ai/decide/task_management/task_finder/filter.rs` | 変更 | 3 | Yard 内 TransportRequest 候補の追加 |
| `src/interface/selection/building_place/placement.rs` | 変更 | 4 | Site/Yard 配置制約を can_place に追加 |
| `src/systems/command/area_selection/apply.rs` | 変更 | 4 | TaskArea を Site 内にクランプ |
| `src/systems/visual/site_yard_visual.rs` | 新規 | 4 | Site/Yard 境界線ビジュアル |
| `src/interface/ui/` (Zones メニュー) | 変更 | 4 | Yard 拡張 UI の追加 |

---

## 注意点 / ブロッカー

- **`BelongsTo` の影響範囲**: `BelongsTo` を Yard entity に変更すると、Tank → Bucket → BucketStorage の所有チェーンが影響を受ける。`src/systems/logistics/transport_request/producer/bucket.rs` と `tank.rs` で `issued_by` / `BelongsTo` の照合ロジックを確認し、Yard entity に対応させること
- **`find_owner_familiar` の置き換え**: `mod.rs:27` の `find_owner_familiar` は TaskArea ベースで Familiar を探す関数。Phase 2 以降は `find_owner_yard` に置き換えるか、両方を共存させる移行期間を設ける
- **`task_finder` の Familiar と Yard の紐付け**: Phase 3 で `filter.rs` に `yard_opt` を追加する際、Familiar → 所属 Site → paired Yard の逆引きが必要。`FamiliarTaskAssignmentQueries` に Yard クエリを追加する
- **`Query<&Site>` / `Query<&Yard>` の使用**: `Single<>` を使わず常にイテレーション。初期実装は 1 件だが将来の複数セット対応のため
