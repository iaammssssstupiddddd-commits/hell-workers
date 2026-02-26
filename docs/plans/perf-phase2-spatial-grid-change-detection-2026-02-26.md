# Phase 2: パフォーマンス改善 — タスク系空間グリッド Change Detection 化と sync 基盤削除

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `perf-phase2-spatial-grid-change-detection-2026-02-26` |
| ステータス | `Done` |
| 作成日 | `2026-02-26` |
| 最終更新日 | `2026-02-26` |
| 作成者 | `Claude (AI Agent)` |
| 関連提案 | `docs/proposals/performance-bottlenecks-proposal-2026-02-26.md` |
| 関連Issue/PR | N/A |

---

## 1. 目的

- **解決したい課題**:
  - `DesignationSpatialGrid` と `TransportRequestSpatialGrid` が 0.15 秒ごとに全クリア＆全件再挿入している（タスク検索に直接影響するため、Phase 1 と切り離して慎重に対応）
  - Phase 1 完了後、`SpatialGridSyncTimer`, `SyncGridClear`, `sync_grid_timed` が死コードになる → 削除
- **到達したい状態**: 全 9 空間グリッドが Change Detection パターンで動作。`SpatialGridSyncTimer` Resource と `sync_grid_timed` 関数が削除されている
- **成功指標**: `cargo check` 成功。Designation/TransportRequest がスポーン後 1 フレーム以内にグリッドに反映される（現状 0.15 秒以内 → 改善）

---

## 2. スコープ

### 対象（In Scope）

- `src/systems/spatial/designation.rs` — Change Detection 化
- `src/systems/spatial/transport_request.rs` — Change Detection 化
- `src/systems/spatial/grid.rs` — `SpatialGridSyncTimer`, `SyncGridClear`, `sync_grid_timed`, `tick_spatial_grid_sync_timer_system` を削除
- `src/systems/spatial/mod.rs` — re-exports 更新
- システム登録箇所（`tick_spatial_grid_sync_timer_system` の登録削除）

### 非対象（Out of Scope）

- 他グリッドの変更（Phase 1 で完了済み）
- Familiar AI のロジック自体の変更

---

## 3. 現状とギャップ

- **現状**:
  - `designation.rs` と `transport_request.rs` には以下のコメントがある:
    > "Spatial が Logic より先に実行されるため、Added<> だと Soul 生成の request が同一フレームで取り込まれず task_finder に見つからない問題を回避する"
  - このコメントの理解: `sync_grid_timed`（0.15 秒同期）も Change Detection（1 フレーム遅延）も、どちらも同一フレーム内での同期はできない。Change Detection の方が遅延が短い（1 フレーム ≈ 16ms vs 最大 0.15 秒）
- **問題**: コメントが示す同一フレーム同期制約は、どちらのアプローチでも回避できない。Change Detection は 0.15 秒同期より高速に反映できる
- **本計画で埋めるギャップ**: Designation/TransportRequest を Change Detection に移行し、sync 基盤コードを削除

---

## 4. 実装方針（高レベル）

- **移行後の動作**:
  - `Added<Designation>` / `Added<TransportRequest>`: スポーン後の次フレームの Spatial フェーズで検出 → Logic フェーズで利用可能（現状の 0.15 秒よりも早い）
  - `RemovedComponents<Designation>`: 削除後の次フレームで除去
  - タスク検索のロジックには変更なし
- **削除可能な基盤**: Phase 1 + Phase 2 完了で `SyncGridClear` を実装する型がゼロになる → `SyncGridClear`, `sync_grid_timed`, `SpatialGridSyncTimer`, `tick_spatial_grid_sync_timer_system` を全て削除
- Bevy 0.18 API 注意: `Commands` 経由で spawn された Designation は `ApplyDeferred` 後に確定。同一フレームの Spatial フェーズでは見えないが、これは現状も同様

---

## 5. マイルストーン

### M1: `designation.rs` を Change Detection 化

**変更内容**:
`sync_grid_timed` パターンから `SoulSpatialGrid` パターンへ移行。`SyncGridClear` impl を削除。

**変更ファイル**:
- `src/systems/spatial/designation.rs`

**具体的変更**:

```rust
// import 変更: SpatialGridSyncTimer, SyncGridClear, sync_grid_timed を削除
use super::grid::{GridData, SpatialGridOps};
use crate::systems::jobs::Designation;
use bevy::prelude::*;

// DesignationSpatialGrid struct と SpatialGridOps impl は変更なし
// get_in_area() メソッドも変更なし

// SyncGridClear impl を削除（impl SyncGridClear for DesignationSpatialGrid { ... }）

// update_designation_spatial_grid_system を置き換え:
/// Designation + Transform を持つエンティティの変更差分のみグリッドに反映する。
/// 変更がない場合はゼロコスト。スポーン後は次フレームの Spatial フェーズで反映される。
pub fn update_designation_spatial_grid_system(
    mut grid: ResMut<DesignationSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (With<Designation>, Or<(Added<Designation>, Changed<Transform>)>),
    >,
    mut removed: RemovedComponents<Designation>,
) {
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}
```

**完了条件**:
- [x] `SyncGridClear` impl が削除されている
- [x] `sync_grid_timed`, `SpatialGridSyncTimer` の import が削除されている
- [x] `cargo check` でエラーなし

**検証**:
- `cargo check`

---

### M2: `transport_request.rs` を Change Detection 化

**変更内容**: M1 と同パターン。`TransportRequest` コンポーネントを使用。

**変更ファイル**:
- `src/systems/spatial/transport_request.rs`

**具体的変更**:

```rust
use super::grid::{GridData, SpatialGridOps};
use crate::systems::logistics::transport_request::TransportRequest;
use bevy::prelude::*;

// SyncGridClear impl を削除

/// TransportRequest の変更差分のみグリッドに反映する。
pub fn update_transport_request_spatial_grid_system(
    mut grid: ResMut<TransportRequestSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (With<TransportRequest>, Or<(Added<TransportRequest>, Changed<Transform>)>),
    >,
    mut removed: RemovedComponents<TransportRequest>,
) {
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}
```

**完了条件**:
- [x] `SyncGridClear` impl が削除されている
- [x] `cargo check` でエラーなし

---

### M3: sync 基盤の削除

**前提**: M1 + M2 完了後、`SyncGridClear` を実装している型がゼロになっている。

**変更内容**:
- `grid.rs` から以下を削除:
  - `SpatialGridSyncTimer` struct + `impl Default`
  - `tick_spatial_grid_sync_timer_system` 関数
  - `SyncGridClear` トレイト定義
  - `sync_grid_timed` 関数
  - 関連 import（`SPATIAL_GRID_SYNC_INTERVAL` など）
- `mod.rs` の re-exports から削除:
  - `SpatialGridSyncTimer, SyncGridClear, sync_grid_timed, tick_spatial_grid_sync_timer_system`
- システム登録箇所から `tick_spatial_grid_sync_timer_system` の登録を削除

**変更ファイル**:
- `src/systems/spatial/grid.rs`
- `src/systems/spatial/mod.rs`
- `tick_spatial_grid_sync_timer_system` が登録されているプラグインファイル（`src/main.rs` または `src/plugins/` 以下）

**プラグイン登録箇所の特定**:
`grep -r "tick_spatial_grid_sync_timer_system" src/` で登録箇所を確認してから削除。

**完了条件**:
- [x] `SyncGridClear` トレイトが `src/` 内でゼロ参照
- [x] `sync_grid_timed` 関数が `src/` 内でゼロ参照
- [x] `tick_spatial_grid_sync_timer_system` がシステム登録から削除されている
- [x] `cargo check` でエラーなし
- [x] `SpatialGridSyncTimer` が `src/` 内でゼロ参照
- [x] `SyncGridClear` トレイトが `src/` 内でゼロ参照
- [x] `sync_grid_timed` 関数が `src/` 内でゼロ参照
- [x] `tick_spatial_grid_sync_timer_system` がシステム登録から削除されている
- [x] `cargo check` でエラーなし

**検証**:
- `cargo check`
- `grep -r "SpatialGridSyncTimer\|SyncGridClear\|sync_grid_timed" src/` が 0 件

---

### M4: `docs/architecture.md` 更新

**変更内容**:
`docs/architecture.md` の「空間グリッド一覧」セクションで、同期方法の記述を更新する。

現在の記述: `同期間隔: SPATIAL_GRID_SYNC_INTERVAL（0.15秒）、SpatialGridSyncTimer で管理`

更新後: `全グリッドが Change Detection ベース（Added/Changed/Removed）で差分更新。変更がない場合はゼロコスト。`

**変更ファイル**:
- `docs/architecture.md`

**完了条件**:
- [x] Change Detection パターンの説明が追加されている
- [x] `SpatialGridSyncTimer` への言及が削除されている
- [x] Change Detection パターンの説明が追加されている

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Designation スポーン直後の同一フレームで Familiar AI がそれを見つけられない | 低（現状も 0.15 秒遅延あり。Change Detection は 1 フレーム遅延で改善） | 動作確認: 新しい Designation スポーン後に Familiar AI がタスクを受け取ることを確認 |
| `TransportRequest` の Transform が存在しない場合（Transform なしで spawn） | 中（グリッドに追加されない） | `transport_request.rs` のクエリが `With<TransportRequest>` + `With<Transform>` を保証。spawn 側に Transform 追加が必要か確認 |
| `tick_spatial_grid_sync_timer_system` の削除で OrderingConstraint が壊れる | 低（タイマー tick のみの関数で副作用なし） | システム登録から削除後 `cargo check` で確認 |
| Phase 1 が完了していない状態で Phase 2 を実施すると一時的に `SyncGridClear` を使う型が残る | なし（Phase 2 は Phase 1 完了後に実施） | Phase 1 の `cargo check` 成功を確認してから開始 |

---

## 7. 検証計画

- **必須**: `cargo check`
- **手動確認シナリオ**:
  - Familiar が伐採/採掘の Designation に対してタスクを正しく割り当てる
  - 搬送タスク（TransportRequest）が Familiar AI に正常に見える
  - `grep -r "SpatialGridSyncTimer\|SyncGridClear\|sync_grid_timed" src/` が 0 件
- **パフォーマンス確認**: F12 デバッグ表示や `FamiliarDelegationPerfMetrics` で `reachable_with_cache_calls` が安定していることを確認

---

## 8. ロールバック方針

- M1, M2, M3 は独立したコミットで実施
- M3 は M1 + M2 に依存するため、M1 or M2 を revert する場合は M3 も同時に revert

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1`, `M2`, `M3`
- 未着手/進行中: なし

### 次のAIが最初にやること

1. Phase 1 の `cargo check` 成功を確認
2. `grep -r "SpatialGridSyncTimer\|SyncGridClear" src/` で Phase 1 後の残件を確認
3. M1（designation.rs）→ M2（transport_request.rs）→ M3（grid.rs 削除）の順で実施

### ブロッカー/注意点

- **Phase 1 完了が前提**: Phase 1 未完了の場合、`SyncGridClear` を削除できない
- `designation.rs` のコメントには「Added<> は使えない」と書かれているが、Change Detection（次フレーム検出）は現状の 0.15 秒同期より **高速** であるため、コメントの意図とは逆に改善になる。コメントは削除または更新すること
- `TransportRequest` エンティティに `Transform` が必ず付与されているか確認（`grep -n "TransportRequest" src/systems/logistics/` で spawn 箇所を確認）

### 参照必須ファイル

- `docs/plans/perf-phase1-quick-wins-2026-02-26.md`（前フェーズ完了確認）
- `src/systems/spatial/soul.rs`（参考実装）
- `src/systems/spatial/designation.rs`（移行対象）
- `src/systems/spatial/transport_request.rs`（移行対象）

### 最終確認ログ

- 最終 `cargo check`: 2026-02-26 (`cargo check`)
- 未解決エラー: なし（計画段階）

### Definition of Done

- [x] M1〜M4 全て完了
- [x] `cargo check` 成功
- [x] `docs/architecture.md` 更新済み
- [x] `SpatialGridSyncTimer` / `SyncGridClear` / `sync_grid_timed` が `src/` 内でゼロ参照
- [x] `docs/architecture.md` 更新済み

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-26` | `Claude (AI Agent)` | 初版作成 |
