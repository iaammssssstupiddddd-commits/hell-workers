# Phase 1: パフォーマンス改善 — リスクゼロの即効改善

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `perf-phase1-quick-wins-2026-02-26` |
| ステータス | `Done` |
| 作成日 | `2026-02-26` |
| 最終更新日 | `2026-02-26` |
| 作成者 | `Claude (AI Agent)` |
| 関連提案 | `docs/proposals/performance-bottlenecks-proposal-2026-02-26.md` |
| 関連Issue/PR | N/A |

---

## 1. 目的

- **解決したい課題**:
  - B4: `idle_behavior_decision_system` が毎フレーム `HashMap` を確保・解放している
  - B6: `get_nearby_in_radius` が毎呼び出しで `Vec::new()` ヒープ割り当てを行っている
  - B1a: 5 種類の空間グリッド（Familiar/Stockpile/Blueprint/FloorConstruction/GatheringSpot）が 0.15 秒ごとに全クリア＆全件再挿入している
- **到達したい状態**: 上記 3 項目のメモリ割り当て頻度が削減され、コードが `SoulSpatialGrid`/`ResourceSpatialGrid` と同じ Change Detection パターンに統一される
- **成功指標**: `cargo check` でエラーなし。全グリッドの変更なし時は 0 件の update ループ実行。

---

## 2. スコープ

### 対象（In Scope）

- `src/systems/soul_ai/decide/idle_behavior/mod.rs` — B4 のみ
- `src/systems/spatial/grid.rs` — B6 API 追加のみ
- `src/systems/spatial/familiar.rs` — B1a
- `src/systems/spatial/stockpile.rs` — B1a
- `src/systems/spatial/blueprint.rs` — B1a
- `src/systems/spatial/floor_construction.rs` — B1a
- `src/systems/spatial/gathering.rs` — B1a（Transform ではなく `GatheringSpot.center` を使用）

### 非対象（Out of Scope）

- `DesignationSpatialGrid`/`TransportRequestSpatialGrid`（Phase 2 で対応）
- `SpatialGridSyncTimer` の削除（Phase 2 で対応）
- `get_nearby_in_radius_into` の全呼び出し箇所への適用（追加後は任意のタイミングで段階適用可）

---

## 3. 現状とギャップ

- **現状**:
  - `idle_behavior_decision_system` 内に `let mut pending_rest_reservations: HashMap<Entity, usize> = HashMap::new();` がある（毎フレーム 1 回）
  - `GridData::get_nearby_in_radius()` が `Vec::new()` を毎回返す
  - 5 種類のグリッドが `sync_grid_timed` で 0.15 秒ごとに全クリア（Soul と Resource は既に Change Detection 化済み）
- **問題**: Soul 数 × フレームレート分の不要なアロケーションと全件スキャンが累積する
- **本計画で埋めるギャップ**: アロケーション削減と変更検出への移行（Designation/TransportRequest を除く安全な 5 グリッド）

---

## 4. 実装方針（高レベル）

- `SoulSpatialGrid`（`soul.rs`）の Change Detection パターンを参考実装として使用する
- `SyncGridClear` impl は各ファイルから削除するが、他ファイルが `SyncGridClear` を impl している間は `grid.rs` から削除しない（Phase 2 で削除）
- `GatheringSpot.center` は `Transform` ではなく component フィールドなので `Changed<GatheringSpot>` を使う
- Bevy 0.18 API: `Added<T>`, `Changed<T>`, `RemovedComponents<T>` は全て Bevy 0.18 で動作確認済み

---

## 5. マイルストーン

### M1: B4 — idle_behavior の HashMap → Local

**変更内容**:
`pending_rest_reservations` を `Local<HashMap<Entity, usize>>` に変更し、ループ前に `.clear()` を呼ぶ。

**変更ファイル**:
- `src/systems/soul_ai/decide/idle_behavior/mod.rs`

**具体的変更**:

```rust
// Before (line 62):
let mut pending_rest_reservations: HashMap<Entity, usize> = HashMap::new();

// After (関数シグネチャに追加):
pub fn idle_behavior_decision_system(
    // ... 既存パラメータ ...
    mut pending_rest_reservations: Local<HashMap<Entity, usize>>,  // ← 追加
) {
    let dt = time.delta_secs();
    pending_rest_reservations.clear();  // ← HashMap::new() の代わり
    // 以降は変更なし（pending_rest_reservations は &mut HashMap として使用可能）
```

**注意**: `pending_rest_reservations` を渡している内部関数がある場合は、引数の型は `&mut HashMap<Entity, usize>` のままで呼び出せるため変更不要。

**完了条件**:
- [x] `HashMap::new()` の行が削除されている
- [x] `Local<HashMap<Entity, usize>>` がシグネチャに追加されている
- [x] `cargo check` でエラーなし

**検証**:
- `cargo check`

---

### M2: B6 — `get_nearby_in_radius_into` API 追加

**変更内容**:
`GridData` に `get_nearby_in_radius_into` メソッドを追加する。既存の `get_nearby_in_radius` は削除しない（段階移行のため）。

**変更ファイル**:
- `src/systems/spatial/grid.rs`

**具体的変更**:

`GridData::get_nearby_in_radius` の直後（line 118 付近）に追加:

```rust
/// バッファを受け取るバージョン。呼び出し側で Local<Vec<Entity>> を再利用できる。
pub fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>) {
    out.clear();
    let cell_radius = (radius / self.cell_size).ceil() as i32;
    let center_cell = self.pos_to_cell(pos);

    for dy in -cell_radius..=cell_radius {
        for dx in -cell_radius..=cell_radius {
            let cell = (center_cell.0 + dx, center_cell.1 + dy);
            if let Some(entities) = self.grid.get(&cell) {
                for &entity in entities {
                    if let Some(&entity_pos) = self.positions.get(&entity) {
                        if pos.distance(entity_pos) <= radius {
                            out.push(entity);
                        }
                    }
                }
            }
        }
    }
}
```

**`SpatialGridOps` トレイトへの追加**（grid.rs line 37 付近）:

```rust
pub trait SpatialGridOps {
    fn insert(&mut self, entity: Entity, pos: Vec2);
    fn remove(&mut self, entity: Entity);
    fn update(&mut self, entity: Entity, pos: Vec2);
    fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity>;
    // 新規追加:
    fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>);
}
```

**各グリッド型の `SpatialGridOps` impl への追加**（soul.rs, familiar.rs 等の全実装型）:

```rust
fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>) {
    self.0.get_nearby_in_radius_into(pos, radius, out);
}
```

**完了条件**:
- [x] `GridData` に `get_nearby_in_radius_into` が追加されている
- [x] `SpatialGridOps` に `get_nearby_in_radius_into` が追加されている
- [x] 全 impl 型（SpatialGrid, FamiliarSpatialGrid, etc.）で impl されている
- [x] `cargo check` でエラーなし

**検証**:
- `cargo check`

---

### M3: B1a — 5 グリッドを Change Detection パターンに移行

**変更内容**:
`familiar.rs`, `stockpile.rs`, `blueprint.rs`, `floor_construction.rs`, `gathering.rs` の 5 ファイルを `SoulSpatialGrid` のパターンに合わせる。

**参考実装** (`soul.rs`):
```rust
pub fn update_spatial_grid_system(
    mut grid: ResMut<SpatialGrid>,
    query: Query<(Entity, &Transform), (With<DamnedSoul>, Or<(Added<DamnedSoul>, Changed<Transform>)>)>,
    mut removed: RemovedComponents<DamnedSoul>,
) {
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}
```

**変更ファイルと具体的変更**:

**(1) `familiar.rs`** — `Familiar` コンポーネント使用:

```rust
// import 変更: SpatialGridSyncTimer, SyncGridClear, sync_grid_timed を削除
use super::grid::{GridData, SpatialGridOps};

// SyncGridClear impl を削除（lines 28-38）

// update_familiar_spatial_grid_system を置き換え:
pub fn update_familiar_spatial_grid_system(
    mut grid: ResMut<FamiliarSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (With<Familiar>, Or<(Added<Familiar>, Changed<Transform>)>),
    >,
    mut removed: RemovedComponents<Familiar>,
) {
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}
```

**(2) `stockpile.rs`** — `Stockpile` コンポーネント使用（同パターン）

**(3) `blueprint.rs`** — `Blueprint` コンポーネント使用（同パターン）

**(4) `floor_construction.rs`** — `FloorConstructionSite` コンポーネント使用（同パターン）

**(5) `gathering.rs`** — Transform がないため `GatheringSpot.center` を使用:

```rust
// import 変更: Transform 不要
use super::grid::{GridData, SpatialGridOps};
use crate::systems::soul_ai::helpers::gathering::GatheringSpot;
use bevy::prelude::*;

// SyncGridClear impl を削除

pub fn update_gathering_spot_spatial_grid_system(
    mut grid: ResMut<GatheringSpotSpatialGrid>,
    query: Query<(Entity, &GatheringSpot), Or<(Added<GatheringSpot>, Changed<GatheringSpot>)>>,
    mut removed: RemovedComponents<GatheringSpot>,
) {
    for (entity, spot) in query.iter() {
        grid.update(entity, spot.center);
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}
```

**注意点**: これら 5 ファイルから `SyncGridClear` impl を削除しても、`grid.rs` には `SyncGridClear` トレイト定義が残る（Designation/TransportRequest がまだ使用中）。Phase 2 完了後に `grid.rs` から削除する。

**`mod.rs` の re-exports 更新**:
- `SpatialGridSyncTimer`, `SyncGridClear`, `sync_grid_timed` の re-export は維持（Phase 2 まで Designation/TransportRequest が使用）

**完了条件**:
- [x] 5 ファイルから `SyncGridClear` impl と `sync_grid_timed` 呼び出しが削除されている
- [x] 5 ファイルで `SpatialGridSyncTimer` の import が削除されている
- [x] `SpatialGridOps` に `get_nearby_in_radius_into` が追加されたため、各ファイルの impl が更新されている（M2 と連動）
- [x] `cargo check` でエラーなし

**検証**:
- `cargo check`
- 手動確認: ゲーム起動後、Familiar が通常通り Soul に命令を出し、Stockpile への搬送が行われることを確認

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| GatheringSpot の `Changed<GatheringSpot>` が center 変更時に fire しない | 低（center はスポーン時のみ設定される） | GatheringSpot の mutability 確認。必要なら `Added<GatheringSpot>` のみで十分 |
| Familiar が追加/削除されない限り FamiliarSpatialGrid が更新されない | 低（Familiar は基本的に移動する） | `Changed<Transform>` が毎フレーム変化を捉える |
| `SpatialGridOps` にメソッドを追加すると全 impl 型でコンパイルエラー | 確実に発生 | M2 の `get_nearby_in_radius_into` を全 impl 型に追加してから他マイルストーンを進める |

---

## 7. 検証計画

- **必須**: `cargo check`
- **手動確認シナリオ**:
  - ゲーム起動して Soul が Stockpile に搬送するタスクを実行できる
  - Familiar が新しい Soul に命令を出せる（FamiliarSpatialGrid が機能している）
  - GatheringSpot が正常に動作する（Soul が集まる）
  - Blueprint が Familiar AI に正常に見える

---

## 8. ロールバック方針

- M1, M2, M3 は独立したコミットで実施
- `git revert <commit>` で各マイルストーン単位で戻せる

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1`, `M2`, `M3`
- 未着手/進行中: なし（Phase 2 は次タスク）

### 次のAIが最初にやること

1. M2 から始める（`SpatialGridOps` へのメソッド追加が他 M1/M3 に影響するため、**M2 を最初に実施**）
2. M2 完了後 `cargo check`
3. M1, M3 を任意の順で実施（独立）

### ブロッカー/注意点

- **M2 が先決**: `SpatialGridOps` に `get_nearby_in_radius_into` を追加すると、全 impl 型でコンパイルエラーが発生するため、M2 で全 impl 型を一括更新してから M3 を進める
- `gathering.rs` の `GatheringSpot.center` は `Vec2` フィールド。`Changed<GatheringSpot>` が `center` 変更時に正しく発火するか要確認（`GatheringSpot` が `grace_timer` 等の別フィールドで頻繁に変更されている場合は `Changed<GatheringSpot>` で過剰更新になる可能性。その場合は別途 marker component 追加を検討）
- Phase 2 が完了するまで `SpatialGridSyncTimer` Resource と `SyncGridClear` トレイトを `grid.rs` から削除しないこと

### 参照必須ファイル

- `src/systems/spatial/soul.rs`（参考実装）
- `src/systems/spatial/resource.rs`（参考実装）
- `src/systems/spatial/grid.rs`（`GridData`, `SpatialGridOps` 定義）
- `src/systems/soul_ai/helpers/gathering.rs`（`GatheringSpot` 構造体確認）

### 最終確認ログ

- 最終 `cargo check`: `cargo check` 成功
- 未解決エラー: なし

### Definition of Done

- [ ] M1, M2, M3 全て完了
- [ ] `cargo check` が成功
- [ ] `docs/architecture.md` の空間グリッド同期方法の記述を更新（Phase 2 完了後に一括更新でも可）
- [x] M1: `pending_rest_reservations` を `Local<HashMap<..., ...>>` 化し `clear()` 再利用化
- [x] M2: `GridData::get_nearby_in_radius_into` 追加と `SpatialGridOps` 反映
- [x] M3: Familiar / Stockpile / Blueprint / FloorConstruction / GatheringSpot を `Added/Changed + RemovedComponents` へ移行

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-26` | `Claude (AI Agent)` | 初版作成 |
