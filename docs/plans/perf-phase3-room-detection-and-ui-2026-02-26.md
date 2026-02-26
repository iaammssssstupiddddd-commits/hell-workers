# Phase 3: パフォーマンス改善 — Room 検出 HashMap clone 削除 + UI ViewModel dirty ゲート化

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `perf-phase3-room-detection-and-ui-2026-02-26` |
| ステータス | `Draft` |
| 作成日 | `2026-02-26` |
| 最終更新日 | `2026-02-26` |
| 作成者 | `Claude (AI Agent)` |
| 関連提案 | `docs/proposals/performance-bottlenecks-proposal-2026-02-26.md` |
| 関連Issue/PR | N/A |

---

## 1. 目的

- **解決したい課題**:
  - B2: `mark_room_dirty_from_world_map_diff_system` が WorldMap 変更のたびに `previous_world_buildings: HashMap<(i32, i32), Entity>` を `clone()` している
  - B5: `build_entity_list_view_model_system` が毎フレーム全件再構築している（`EntityListDirty` フラグが存在するにもかかわらず未活用）
- **到達したい状態**:
  - `previous_world_buildings` フィールドと `mark_room_dirty_from_world_map_diff_system` が削除され、Observer ベースの dirty marking に置き換わっている
  - `build_entity_list_view_model_system` が dirty フラグの有無を確認し、変更がない場合はスキップする
- **成功指標**: `cargo check` 成功。Building 追加/削除で Room 検出が正常動作。UI リストが Soul/Familiar 変化時に正しく更新される。

---

## 2. スコープ

### 対象（In Scope）

**B2: Room Detection**
- `src/systems/room/dirty_mark.rs` — `mark_room_dirty_from_world_map_diff_system` 削除、Observer 追加
- `src/systems/room/resources.rs` — `RoomDetectionState.previous_world_buildings` フィールド削除
- `src/systems/room/mod.rs` — システム/Observer 登録更新

**B5: UI View Model**
- `src/interface/ui/list/view_model.rs` — dirty ゲート追加
- `src/interface/ui/list/sync.rs` — `previous` 更新ロジック移動、`PartialEq` チェック維持
- `src/interface/ui/list/mod.rs` — `EntityListViewModel.previous` フィールドの扱い確認

### 非対象（Out of Scope）

- Room Detection のアルゴリズム自体の変更
- UI のビジュアル/レイアウト変更
- `sync_entity_list_value_rows_system` の変更

---

## 3. 現状とギャップ

**B2 現状**:
```rust
// dirty_mark.rs:36-69
pub fn mark_room_dirty_from_world_map_diff_system(world_map, mut detection_state) {
    if !world_map.is_changed() && !detection_state.previous_world_buildings.is_empty() {
        return;  // WorldMap 変更なし → スキップ（ガード済み）
    }
    // ... 差分比較 ...
    detection_state.previous_world_buildings = current.clone();  // ← HashMap 全クローン
}
```
- `world_map.is_changed()` ガードは存在するが、Building 追加/削除時は毎回 HashMap 全クローン発生

**B5 現状**:
```rust
// view_model.rs:137-193
pub fn build_entity_list_view_model_system(...) {
    view_model.previous = std::mem::take(&mut view_model.current);  // ← 毎フレーム
    // ... 全 Familiar + 全 Soul を再収集・ソート ... ← 毎フレーム
}
// sync.rs:85
if view_model.current == view_model.previous { return; }  // ← PartialEq による差分検出
```
- `EntityListDirty` リソースが存在し、Change Detection により適切にフラグが立つが、`build_entity_list_view_model_system` は dirty に関係なく毎フレーム実行される

---

## 4. 実装方針（高レベル）

**B2**: `previous_world_buildings` の差分比較を廃止し、Bevy Observer（`OnAdd<Building>`, `OnRemove<Building>` 等）で dirty marking を行う。Observer は ECS が自動でトリガーするため、HashMap 保存が不要になる。

**B5**:
1. `build_entity_list_view_model_system` に `Res<EntityListDirty>` を追加し、dirty でなければ early return
2. `sync_entity_list_from_view_model_system` の末尾（sync 実行後）に `view_model.previous = view_model.current.clone()` を追加
3. これにより「build がスキップされたフレームでは current == previous（前回 sync 後に更新済み）」が保証される

**Bevy 0.18 API 注意**:
- Observer に `OnAdd<T>`, `OnRemove<T>` を使用（Bevy 0.18 で利用可能）
- `app.observe(|trigger: Trigger<OnAdd, Building>, ...| { ... })` のシグネチャで登録

---

## 5. マイルストーン

### M1: B2 — `previous_world_buildings` 削除と Observer 移行

**変更内容**:

**(A) `resources.rs`**: `previous_world_buildings` フィールドを `RoomDetectionState` から削除

```rust
// Before:
pub struct RoomDetectionState {
    pub dirty_tiles: HashSet<(i32, i32)>,
    pub cooldown: Timer,
    pub previous_world_buildings: HashMap<(i32, i32), Entity>,  // ← 削除
}

// After:
pub struct RoomDetectionState {
    pub dirty_tiles: HashSet<(i32, i32)>,
    pub cooldown: Timer,
}
impl Default for RoomDetectionState {
    fn default() -> Self {
        Self {
            dirty_tiles: HashSet::new(),
            cooldown: Timer::from_seconds(ROOM_DETECTION_COOLDOWN_SECS, TimerMode::Repeating),
        }
    }
}
```
`HashMap` の import も不要になれば削除。

**(B) `dirty_mark.rs`**: `mark_room_dirty_from_world_map_diff_system` を削除し、Observer 関数を追加

```rust
use super::resources::RoomDetectionState;
use crate::systems::jobs::{Building, Door};
use crate::world::map::WorldMap;
use bevy::prelude::*;

// 既存システムはそのまま維持:
pub fn mark_room_dirty_from_building_changes_system(...) { /* 変更なし */ }

// 削除:
// pub fn mark_room_dirty_from_world_map_diff_system(...) { ... }

// 新規追加: Observer 関数
pub fn on_building_added(
    trigger: Trigger<OnAdd, Building>,
    q_transform: Query<&Transform>,
    mut detection_state: ResMut<RoomDetectionState>,
) {
    if let Ok(transform) = q_transform.get(trigger.target()) {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        detection_state.mark_dirty(grid);
    }
}

pub fn on_building_removed(
    trigger: Trigger<OnRemove, Building>,
    q_transform: Query<&Transform>,
    mut detection_state: ResMut<RoomDetectionState>,
) {
    if let Ok(transform) = q_transform.get(trigger.target()) {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        detection_state.mark_dirty(grid);
    }
}

pub fn on_door_added(
    trigger: Trigger<OnAdd, Door>,
    q_transform: Query<&Transform>,
    mut detection_state: ResMut<RoomDetectionState>,
) {
    if let Ok(transform) = q_transform.get(trigger.target()) {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        detection_state.mark_dirty(grid);
    }
}

pub fn on_door_removed(
    trigger: Trigger<OnRemove, Door>,
    q_transform: Query<&Transform>,
    mut detection_state: ResMut<RoomDetectionState>,
) {
    if let Ok(transform) = q_transform.get(trigger.target()) {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        detection_state.mark_dirty(grid);
    }
}
```

**注意**: `Trigger<OnRemove, Building>` では `trigger.target()` の Transform が削除直前（OnRemove は削除前に発火）であれば取得可能。Bevy 0.18 で `OnRemove` は Component が削除される直前に発火するため、Transform は存在する。

**(C) `mod.rs`（room）**: システム/Observer 登録の更新

```rust
// mark_room_dirty_from_world_map_diff_system の登録を削除
// Observer を追加:
app.observe(dirty_mark::on_building_added);
app.observe(dirty_mark::on_building_removed);
app.observe(dirty_mark::on_door_added);
app.observe(dirty_mark::on_door_removed);
```

**mod.rs の実際の登録箇所**: `grep -rn "mark_room_dirty_from_world_map_diff_system"` で確認してから削除。

**変更ファイル**:
- `src/systems/room/resources.rs`
- `src/systems/room/dirty_mark.rs`
- `src/systems/room/mod.rs`（または登録箇所のプラグインファイル）

**完了条件**:
- [ ] `previous_world_buildings` フィールドが削除されている
- [ ] `mark_room_dirty_from_world_map_diff_system` が削除されている
- [ ] 4 つの Observer 関数が追加されている
- [ ] Observer が `app.observe(...)` で登録されている
- [ ] `cargo check` でエラーなし

**検証**:
- `cargo check`
- 手動確認: Building を追加/削除後に Room 検出が正常動作する

---

### M2: B5 — UI View Model dirty ゲート化

**変更内容**:

**設計の根拠**:
- `detect_entity_list_changes` が dirty フラグを立てる
- `build_entity_list_view_model_system` が dirty 確認 → スキップ or ビルド
- `sync_entity_list_from_view_model_system` が dirty クリア + `previous = current` 更新

これにより: dirty でない場合 → build スキップ → current == previous（前回 sync 後に一致させた） → sync スキップ。

**(A) `view_model.rs`**: dirty ゲートを追加

```rust
pub fn build_entity_list_view_model_system(
    dirty: Res<EntityListDirty>,  // ← 追加
    mut view_model: ResMut<EntityListViewModel>,
    // ... 既存パラメータ ...
) {
    // dirty でなければ再構築不要
    if !dirty.needs_structure_sync() && !dirty.needs_value_sync_only() {
        return;
    }

    view_model.previous = std::mem::take(&mut view_model.current);
    // ... 以降は変更なし ...
}
```

**(B) `sync.rs`**: sync 完了後に `previous = current.clone()` を追加

`sync_entity_list_from_view_model_system` の末尾に追加:

```rust
pub fn sync_entity_list_from_view_model_system(
    mut commands: Commands,
    // ... 既存パラメータ ...
    mut view_model: ResMut<EntityListViewModel>,  // mut に変更（既に mut の場合は変更不要）
    mut dirty: ResMut<super::dirty::EntityListDirty>,
    // ...
) {
    dirty.clear_all();

    if view_model.current == view_model.previous {
        return;
    }

    // ... 既存の sync_familiar_sections / sync_unassigned_souls 呼び出し ...

    // 追加: sync 完了後、previous を current に合わせる
    // これにより次フレームで build がスキップされた場合でも current == previous が保証される
    view_model.previous = view_model.current.clone();
}
```

**変更ファイル**:
- `src/interface/ui/list/view_model.rs`
- `src/interface/ui/list/sync.rs`

**完了条件**:
- [ ] `build_entity_list_view_model_system` に `Res<EntityListDirty>` が追加されている
- [ ] dirty でない場合に early return している
- [ ] `sync_entity_list_from_view_model_system` の末尾に `view_model.previous = view_model.current.clone()` がある
- [ ] `EntityListDirty::needs_structure_sync()` / `needs_value_sync_only()` が既存 API で使用されている
- [ ] `cargo check` でエラーなし

**検証**:
- `cargo check`
- 手動確認:
  - Soul がスポーンした時にリストに表示される
  - Soul のタスクが変化した時にリストのアイコンが更新される
  - Familiar が Soul に命令を出した時にリスト構造が更新される
  - 何も変化しない場合（idle 状態）はリストが更新されない（`build_entity_list_view_model_system` がスキップされる）

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `OnRemove<Building>` 時に Transform が既に削除されていると grid 座標が取得できない | 中 | Bevy 0.18 では OnRemove は Component 削除直前に発火するため Transform は存在するはず。確認: `OnRemove` の発火タイミングのドキュメント確認 |
| Building 削除時に dirty marking が漏れると Room が消えない | 高 | Observer 登録後に Building 削除テストを実施。フォールバック: 既存の `mark_room_dirty_from_building_changes_system` は維持されているため、Transform Changed 経由でも検出される |
| `build_entity_list_view_model_system` の dirty ゲートで更新が漏れる | 高 | `detect_entity_list_changes` が全必要な Component の Added/Changed/Removed を網羅しているか確認（change_detection.rs の 12 クエリを全チェック）。漏れがある場合は `mark_values()` or `mark_structure()` を追加 |
| `view_model.previous = view_model.current.clone()` が重い | 低（Familiar × Soul 数分の String clone） | 既存の `view_model.previous = std::mem::take(&mut view_model.current)` も同コスト。毎フレームではなく sync 時のみになるため問題なし |

---

## 7. 検証計画

- **必須**: `cargo check`
- **手動確認シナリオ**:

**B2 検証**:
1. ゲーム起動後、Building を配置 → Room 検出が動作することを確認
2. Building を削除 → Room が消えることを確認
3. Door を追加 → Room の開閉判定が正常に動作することを確認

**B5 検証**:
1. Soul が何もしていない状態でリストが静止していることを確認（毎フレーム更新されていない）
2. Soul に新しいタスクが割り当てられた時にリストアイコンが変わることを確認
3. Soul がスポーン/デスポーンした時にリストの行が追加/削除されることを確認
4. Familiar が Soul に命令を出した時に構造が変わることを確認

---

## 8. ロールバック方針

- M1 と M2 は独立したコミット
- M1 を revert する場合は Observer 登録も含めて revert（`on_building_added` 等の Observer 4 つ + `app.observe(...)` 4 行）
- M2 を revert する場合は `view_model.rs` と `sync.rs` の 2 ファイルを revert

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（Phase 2 完了後に着手）
- 完了済みマイルストーン: なし
- 未着手/進行中: Phase 2 完了後に M1 から開始

### 次のAIが最初にやること

1. `grep -rn "mark_room_dirty_from_world_map_diff_system" src/` で登録箇所を特定
2. `grep -rn "previous_world_buildings" src/` でフィールド参照箇所を確認
3. M1（Room Detection）から着手。Observer のシグネチャは Bevy 0.18 に合わせて確認
4. M2（UI View Model）を実施。sync.rs の `view_model` が `ResMut` か確認してから変更

### ブロッカー/注意点

- **Observer の `trigger.target()` シグネチャ**: Bevy 0.18 では `Trigger<OnAdd, T>` の `trigger.entity()` または `trigger.target()` でエンティティ取得。正しいメソッド名は Bevy ドキュメントで確認すること（`~/.cargo/registry/src/` の `bevy_ecs/src/observer/` 参照）
- **`OnRemove` のタイミング**: Component が削除される直前に発火するため、同一フレーム内で Transform は存在するはず。ただし、`despawn_recursive` で親と子が同時に削除される場合は順序に注意
- **B5 の `dirty.clear_all()` の呼び出し位置**: `sync_entity_list_from_view_model_system` の先頭（line 83）で呼ばれる。`build_entity_list_view_model_system` のガード確認時点ではまだ dirty が true のため、順序は正しい（build → sync の順）
- `EntityListDirty::needs_structure_sync()` と `needs_value_sync_only()` の意味を確認してから使用（dirty.rs を読む）

### 参照必須ファイル

- `src/systems/room/dirty_mark.rs`（変更対象）
- `src/systems/room/resources.rs`（変更対象）
- `src/interface/ui/list/dirty.rs`（`EntityListDirty` の API 定義）
- `src/interface/ui/list/change_detection.rs`（detect システムで mark している全箇所）
- `src/interface/ui/list/view_model.rs`（変更対象）
- `src/interface/ui/list/sync.rs`（変更対象）

### 最終確認ログ

- 最終 `cargo check`: 未実施
- 未解決エラー: なし（計画段階）

### Definition of Done

- [ ] M1, M2 全て完了
- [ ] `cargo check` 成功
- [ ] `previous_world_buildings` が `src/` 内でゼロ参照
- [ ] `mark_room_dirty_from_world_map_diff_system` が `src/` 内でゼロ参照
- [ ] Room 検出の手動確認シナリオが全てパス
- [ ] UI リストの手動確認シナリオが全てパス

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-26` | `Claude (AI Agent)` | 初版作成 |
