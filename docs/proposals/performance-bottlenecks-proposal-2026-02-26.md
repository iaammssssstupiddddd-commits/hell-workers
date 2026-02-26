# パフォーマンスボトルネック改善提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `performance-bottlenecks-proposal-2026-02-26` |
| ステータス | `Draft` |
| 作成日 | `2026-02-26` |
| 最終更新日 | `2026-02-26` |
| 作成者 | `Claude (AI Agent)` |
| 関連計画 | `TBD` |
| 関連Issue/PR | N/A |

---

## 1. 背景と問題

- **現状**: Soul 数が増加するにつれてフレームレートが低下する傾向がある。Space/Spatial グリッドの同期、Room 検出、Soul AI の決定処理など複数の領域で毎フレーム・定期的な全件処理が行われている。
- **問題**: 以下の 6 領域でホットスポットが確認された（コード静的解析による）：
  1. Spatial Grid の毎 0.15 秒全クリア＆再挿入（8 グリッド独立）
  2. Room 検出での `HashMap::clone()` 毎フレーム実行
  3. Soul Idle Behavior の毎フレーム `HashMap` 割り当て
  4. Familiar AI タスク検索の全件スキャン＋ Reachability A* 多重呼び出し
  5. UI Entity List の毎フレーム全件再構築 + `O(N*M)` PartialEq 比較
  6. `get_nearby_in_radius` の毎呼び出し `Vec` 動的割り当て
- **なぜ今やるか**: Soul 数が数十人規模になると複合的に影響が顕在化する。早期に対処することでスケーラビリティを確保できる。

---

## 2. 目的（Goals）

- Soul 数 100+ 時の主要ロジックフレームコストを削減する
- メモリ割り当て頻度（毎フレーム `Vec`/`HashMap` 生成）を抑制する
- 各改善を独立したプランとして段階的に実施できるよう整理する

## 3. 非目的（Non-Goals）

- GPU/レンダリング側の最適化（対象外）
- A* アルゴリズム自体の置き換え（アルゴリズム変更は別提案）
- セーブデータ構造の変更

---

## 4. 提案内容（概要）

- **一言要約**: 毎フレームの全件処理・動的割り当てを差分更新・プール再利用・Change Detection に置き換える。
- **主要な変更点**: 優先度別に 6 項目（下記§5 で詳述）
- **期待される効果**: Soul 100 人スケールで Logic フェーズのフレームコストを 30〜50% 削減（推定）

---

## 5. 詳細設計

### 5.1 ボトルネック一覧と優先度

| 優先度 | ID | 領域 | 推定影響 |
|:---:|:---:|:---|:---:|
| P1 | B1 | Spatial Grid フル再構築（毎 0.15 秒 × 8 グリッド） | 高 |
| P1 | B2 | Room Detection `HashMap::clone()` 毎フレーム | 高 |
| P2 | B3 | Familiar AI タスク検索 O(n) + Reachability A* 多重 | 高 |
| P2 | B4 | Soul Idle Behavior 毎フレーム `HashMap` 割り当て | 中～高 |
| P3 | B5 | UI View Model 毎フレーム全件再構築 + `PartialEq` | 中～高 |
| P3 | B6 | `get_nearby_in_radius` 毎呼び出し `Vec` 割り当て | 中 |

---

### 5.2 B1: Spatial Grid フル再構築

**現状コード** (`src/systems/spatial/grid.rs:43-60`):

```rust
pub fn sync_grid_timed<G, I>(sync_timer: &mut SpatialGridSyncTimer, grid: &mut G, entities: I) {
    if sync_timer.first_run_done && !sync_timer.timer.just_finished() {
        return;
    }
    grid.clear_and_sync(entities);  // 毎 0.15 秒に HashMap 2 つを全クリア＆全件挿入
}
```

**問題点**:
- 8 種類のグリッド（Designation, TransportRequest, Resource, Stockpile, Soul\*, Familiar, Blueprint, GatheringSpot, FloorConstruction）が毎 0.15 秒にクリア＆全件再挿入
- `SoulSpatialGrid` は既に Change Detection ベース（効率的）だが、他 7 種は全件スキャン
- `SpatialGridSyncTimer` が各グリッドごとに独立して存在する可能性あり（確認要）

**改善案 (案A: Change Detection 統一)**:
- `Designation`/`Blueprint`/`GatheringSpot`/`FloorConstruction` は `Added<T>`/`Changed<Transform>`/`RemovedComponents<T>` を使用して差分更新
- `SoulSpatialGrid` と同じパターンを全グリッドに適用

```rust
// 変更例: designation spatial grid
pub fn update_designation_spatial_grid_system(
    mut grid: ResMut<DesignationSpatialGrid>,
    added: Query<(Entity, &Transform), Added<Designation>>,
    changed: Query<(Entity, &Transform), (With<Designation>, Changed<Transform>)>,
    mut removed: RemovedComponents<Designation>,
) {
    for entity in removed.read() { grid.remove(entity); }
    for (e, t) in added.iter().chain(changed.iter()) {
        grid.update(e, t.translation.truncate());
    }
}
```

**改善案 (案B: 同期タイマー一括化)**:
- 現在の 0.15 秒タイマーを全グリッドで共有する単一の `SpatialGridSyncTimer` にまとめる
- 同一フレームで 8 グリッドが同時に sync されるよう整列させる

**推奨**: 案A（Change Detection 統一）。新規割り当てなしに差分更新できる。静的エンティティ（Designation 等）には特に効果的。

**変更対象**:
- `src/systems/spatial/designation.rs`
- `src/systems/spatial/transport_request.rs`
- `src/systems/spatial/resource.rs`
- `src/systems/spatial/stockpile.rs`
- `src/systems/spatial/familiar.rs`
- `src/systems/spatial/blueprint.rs`
- `src/systems/spatial/gathering.rs`
- `src/systems/spatial/floor_construction.rs`

---

### 5.3 B2: Room Detection `HashMap::clone()` 毎フレーム

**現状コード** (`src/systems/room/dirty_mark.rs:62-68`):

```rust
pub fn mark_room_dirty_from_world_map_diff_system(...) {
    // ... 差分比較のループ ...
    detection_state.previous_world_buildings = current.clone();  // 毎フレーム HashMap 全クローン
}
```

**問題点**:
- `previous_world_buildings: HashMap<(i32, i32), Entity>` を毎フレーム `clone()` して保存
- 500 ビルディングがあれば毎フレーム 500 エントリのコピー
- このシステムはビルディング変更があるかに関係なく毎フレーム実行される

**改善案**:
- `WorldMap::buildings` を `Res<WorldMap>` の `Changed` フィルタで監視し、変更がある場合のみ差分比較を実行
- または `previous_world_buildings` を廃止し、`Added<Building>`/`RemovedComponents<Building>` Observer で dirty マークを直接行う（HashMap 保存不要）

```rust
// 改善後: Observer ベース
fn on_building_added(trigger: Trigger<OnAdd, Building>, mut state: ResMut<RoomDetectionState>, q: Query<&Transform>) {
    if let Ok(t) = q.get(trigger.entity()) {
        state.mark_dirty(WorldMap::world_to_grid(t.translation.truncate()));
    }
}

fn on_building_removed(trigger: Trigger<OnRemove, Building>, ...) { ... }
```

**変更対象**:
- `src/systems/room/dirty_mark.rs`
- `src/systems/room/resources.rs`（`previous_world_buildings` フィールド削除）

---

### 5.4 B3: Familiar AI タスク検索 + Reachability

**現状コード** (`src/systems/familiar_ai/.../assignment_loop.rs:57-73`):

```rust
fn reachable_with_cache(worker_grid, candidate, world_map, pf_context, cache) -> bool {
    // フレーム内キャッシュは存在するが、次フレームで全破棄
    let key = (worker_grid, candidate.target_grid);
    if let Some(r) = cache.get(&key) { return *r; }
    let reachable = evaluate_reachability(...);  // A* を最大 2 回呼び出し
    cache.insert(key, reachable);
    reachable
}
```

**問題点**:
- Reachability キャッシュがフレーム内のみ有効（次フレームで再計算）
- タスク委譲インターバル（0.3 秒程度）があっても、インターバル明けに全ワーカー × top-K 候補で再 A*
- `filter.rs` の `collect_candidate_entities` は `HashSet → Vec` 変換を毎回行う

**改善案**:
- `ReachabilityCache` を `Local` から `Resource` に昇格し、ワールドマップ変更時のみ無効化
- Candidate フィルタリング結果を `FAMILIAR_TASK_DELEGATION_INTERVAL` の間キャッシュ
- `HashSet → Vec` 変換を廃止し、直接 `SmallVec` または事前確保 `Vec` を使用

```rust
// Reachability を Resource 化（WorldMap 変更時にクリア）
#[derive(Resource, Default)]
struct ReachabilityCache(HashMap<ReachabilityCacheKey, bool>);

// WorldMap 変更時にキャッシュクリア
fn clear_reachability_cache_on_world_change(
    world_map: Res<WorldMap>,
    mut cache: ResMut<ReachabilityCache>,
) {
    if world_map.is_changed() {
        cache.0.clear();
    }
}
```

**変更対象**:
- `src/systems/familiar_ai/decide/task_management/task_finder/filter.rs`
- `src/systems/familiar_ai/decide/task_management/delegation/assignment_loop.rs`

---

### 5.5 B4: Soul Idle Behavior 毎フレーム `HashMap` 割り当て

**現状コード** (`src/systems/soul_ai/decide/idle_behavior/mod.rs:41`):

```rust
pub fn idle_behavior_decision_system(...) {
    let mut pending_rest_reservations: HashMap<Entity, usize> = HashMap::new();  // 毎フレーム生成

    for (...) in query.iter_mut() {  // 100+ Souls
        // ...
    }
}
```

**問題点**:
- `pending_rest_reservations: HashMap` が毎フレーム確保・解放
- 100+ Souls ループで HashMap への挿入/参照が繰り返される

**改善案**:
- `Local<HashMap<Entity, usize>>` に変換し、ループ前に `clear()` のみ実行（再アロケーション不要）
- または RestArea 数（通常 5〜10 個）に合わせた小サイズの固定配列（`SmallVec<[(Entity, usize); 8]>`）に置換

```rust
pub fn idle_behavior_decision_system(
    // ...
    mut local_rest_reservations: Local<HashMap<Entity, usize>>,
) {
    local_rest_reservations.clear();  // アロケーションなし
    // ...
}
```

**変更対象**:
- `src/systems/soul_ai/decide/idle_behavior/mod.rs`

---

### 5.6 B5: UI Entity List 毎フレーム全件再構築

**現状コード** (`src/interface/ui/list/view_model.rs`):

```rust
pub fn build_entity_list_view_model_system(...) {
    // 毎フレーム全 Familiar + 全 Soul を再収集・ソート
    view_model.previous = std::mem::take(&mut view_model.current);
    let mut familiars = Vec::new();
    // ...
    familiars.sort_by_key(|vm| vm.entity.index());
    // ...
}
```

**問題点**:
- `EntityListDirty` による Change Detection は実装済みだが、View Model 構築は毎フレーム実行
- `familiars` と `unassigned` が毎フレーム `Vec::new()` で確保
- `view_model.current == view_model.previous` の `PartialEq` 比較が全 String フィールドを含む

**改善案**:
- `build_entity_list_view_model_system` を `dirty` フラグが立っている場合のみ実行するよう条件追加
- `Vec` を `Local` の再利用バッファに変更（`clear()` のみ）
- `PartialEq` 比較を廃止し、`dirty` フラグのみで sync 判定

```rust
pub fn build_entity_list_view_model_system(
    dirty: Res<EntityListDirty>,
    // ...
) {
    if !dirty.needs_rebuild() {
        return;  // 変更がなければスキップ
    }
    // ...
}
```

**変更対象**:
- `src/interface/ui/list/view_model.rs`
- `src/interface/ui/list/sync.rs`

---

### 5.7 B6: `get_nearby_in_radius` の毎呼び出し `Vec` 割り当て

**現状コード** (`src/systems/spatial/grid.rs:98-118`):

```rust
pub fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
    let mut results = Vec::new();  // 毎呼び出しで Vec 生成
    // ...
    results
}
```

**問題点**:
- Familiar AI の gather/transport 検索で頻繁に呼び出される
- 返却 `Vec` がすぐに消費されるにもかかわらず毎回ヒープ割り当て

**改善案**:
- コールバック版 API を追加し、呼び出し側でバッファを再利用できるようにする

```rust
// 既存 API は互換性のため残す
pub fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> { ... }

// 新 API: 呼び出し側バッファに書き込む
pub fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>) {
    out.clear();
    // ... 既存ロジックを out.push(...) に変更
}
```

**変更対象**:
- `src/systems/spatial/grid.rs`
- 各呼び出し箇所（`familiar_ai` 内）

---

### 5.8 変更対象ファイル一覧

| ファイル | 対応ボトルネック |
|:---|:---:|
| `src/systems/spatial/designation.rs` | B1 |
| `src/systems/spatial/transport_request.rs` | B1 |
| `src/systems/spatial/resource.rs` | B1 |
| `src/systems/spatial/stockpile.rs` | B1 |
| `src/systems/spatial/familiar.rs` | B1 |
| `src/systems/spatial/blueprint.rs` | B1 |
| `src/systems/spatial/gathering.rs` | B1 |
| `src/systems/spatial/floor_construction.rs` | B1 |
| `src/systems/spatial/grid.rs` | B6 |
| `src/systems/room/dirty_mark.rs` | B2 |
| `src/systems/room/resources.rs` | B2 |
| `src/systems/familiar_ai/decide/task_management/task_finder/filter.rs` | B3 |
| `src/systems/familiar_ai/decide/task_management/delegation/assignment_loop.rs` | B3 |
| `src/systems/soul_ai/decide/idle_behavior/mod.rs` | B4 |
| `src/interface/ui/list/view_model.rs` | B5 |
| `src/interface/ui/list/sync.rs` | B5 |

---

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| Spatial Grid の全件同期を維持（現状） | 不採用 | スケール時に O(n) コストが複数グリッド分累積 |
| Spatial Grid を ECS Relationship に置換 | 保留 | 設計変更が大きく、別提案として扱う |
| Room Detection をイベントドリブン化（Observer） | 採用推奨 | `previous_world_buildings` clone を完全廃止できる |
| Reachability をグローバルキャッシュ化 | 採用推奨 | 0.3 秒以内のワールド変更は稀なため有効 |
| UI View Model をリアクティブ diff に置換 | 採用推奨 | `dirty` フラグが既に存在するため低コストに実現可能 |
| A* をナビゲーションメッシュに置換 | 将来検討 | 変更コストが大きいため別提案 |

---

## 7. 影響範囲

- **ゲーム挙動**: 差分更新化により古いフレームのグリッド状態が参照される可能性あり（Change Detection のフレーム遅延に注意）
- **パフォーマンス**: Logic フェーズのフレームコストを削減。特に Soul 数 50+ 時に効果
- **UI/UX**: View Model スキップ条件が正確に実装されていれば変化なし
- **セーブ互換**: データ構造変更なし（`previous_world_buildings` 削除は Resource 変更だがセーブ対象外）
- **既存ドキュメント更新**: `docs/architecture.md` の空間グリッド一覧・同期方法を更新

---

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Change Detection ベースのグリッドでフレーム遅延が発生し UI や AI が古い情報を参照 | 中 | 初回フレームは強制同期、その後は差分のみ（現 `SoulSpatialGrid` のパターンを踏襲） |
| Reachability キャッシュが WorldMap 変更後に古い結果を返す | 高 | `world_map.is_changed()` 時にキャッシュクリア。Observer で `OnTileChanged` イベント発行も検討 |
| View Model スキップ条件の実装漏れでリスト更新が止まる | 中 | `dirty` フラグの mark 箇所を網羅的にテスト（spawn/despawn/assign/unassign を確認） |
| Observer ベース Room Dirty Mark が非同期イベントで抜け漏れ | 低 | 既存のタイマーによる定期全確認をフォールバックとして残す |

---

## 9. 検証計画

- `cargo check` でコンパイルエラーなし
- 手動確認シナリオ:
  - Soul 100 人をスポーンして全員がタスクを受け取ることを確認
  - 建築を追加/削除してルーム検出が正常に動作することを確認
  - UI リストに Soul/Familiar の変化がリアルタイムで反映されることを確認
  - パスファインディングが全 Soul に対して正常に機能することを確認
- 計測/ログ確認:
  - `REACHABLE_WITH_CACHE_CALLS` アトミックカウンターで Reachability 呼び出し回数の削減を確認
  - Bevy `#[cfg(feature = "trace")]` または `bevy_mod_debugdump` でシステム実行時間を計測

---

## 10. ロールアウト/ロールバック

- **導入手順**: B1〜B6 を独立した PR/プランに分割し、優先度順に実施
  - P1 (B1, B2) → P2 (B3, B4) → P3 (B5, B6) の順
- **段階導入**: 各 Bx を独立して適用可能（依存関係なし）
- **問題発生時の戻し方**: 各 Bx が独立したコミットであれば `git revert` で個別に戻せる

---

## 11. 未解決事項（Open Questions）

- [ ] `SpatialGridSyncTimer` が各グリッドで共有されているか個別か確認（B1 の実装方法に影響）
- [ ] `WorldMap::buildings` の変更検出が Bevy の `Changed<T>` で機能するか確認（Resource の内部変更は `is_changed()` を手動で呼ぶ必要がある場合も）
- [ ] Room Detection の 0.5 秒クールダウンは Observer 化後も維持するか（連続建築時の throttle として有効）
- [ ] `get_nearby_in_radius_into` の採用時、既存の呼び出し箇所が `Local` バッファを保持できるか確認

---

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（調査・提案のみ、実装未着手）
- 直近で完了したこと: コード静的解析によるボトルネック特定と提案書作成
- 現在のブランチ/前提: `master` ブランチ、`cargo check` 済みの状態から実装開始

### 次のAIが最初にやること

1. `docs/proposals/performance-bottlenecks-proposal-2026-02-26.md` の「未解決事項」を確認し、`SpatialGridSyncTimer` の共有状況を `src/systems/spatial/mod.rs` で調査
2. P1 (B1: Spatial Grid Change Detection 化) から着手する場合は `docs/plans/spatial-grid-change-detection-YYYY-MM-DD.md` を作成
3. 実装後は必ず `cargo check` でコンパイルエラーなしを確認

### ブロッカー/注意点

- `SoulSpatialGrid` の Change Detection 実装 (`src/systems/spatial/soul.rs`) が参考実装として使用可能
- グリッド変更は Familiar AI のタスク検索に直接影響するため、タスク割り当て動作のリグレッションに注意
- `WorldMap` は `Resource` であり、内部 `HashMap` の変更では `Changed<WorldMap>` がトリガーされない可能性がある（`set_changed()` を手動で呼ぶか `RemovedComponents` Observer が必要）

### 参照必須ファイル

- `docs/architecture.md` § 空間グリッド一覧
- `src/systems/spatial/soul.rs`（Change Detection ベース実装の参考）
- `src/systems/spatial/grid.rs`（`SyncGridClear` トレイト定義）
- `src/systems/room/dirty_mark.rs`（B2 の現状実装）
- `src/systems/familiar_ai/decide/task_management/delegation/assignment_loop.rs`（B3 のキャッシュロジック）
- `src/interface/ui/list/view_model.rs` と `dirty.rs`（B5 の dirty フラグ設計）

### 完了条件（Definition of Done）

- [x] 提案内容がレビュー可能な粒度で記述されている
- [x] リスク・影響範囲・検証計画が埋まっている
- [ ] 実装へ進む場合の `docs/plans/...` が明記されている（実装 PR 時に追加）

---

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-26` | `Claude (AI Agent)` | 初版作成（コード静的解析によるボトルネック特定） |
