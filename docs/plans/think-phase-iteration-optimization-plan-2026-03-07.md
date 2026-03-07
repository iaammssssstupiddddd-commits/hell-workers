# Think フェーズのイテレーション最適化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `think-phase-iteration-optimization-plan-2026-03-07` |
| ステータス | `Draft` |
| 作成日 | `2026-03-07` |
| 最終更新日 | `2026-03-07` |
| 作成者 | `AI (Copilot)` |
| 関連提案 | `docs/proposals/think-phase-iteration-optimization-proposal-2026-03-07.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  - `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs` の `compute_remaining_floor_bones` / `compute_remaining_floor_mud` が、毎回 `queries.storage.floor_tiles.iter()` を全件走査して `parent_site == site_entity` を絞り込んでいる。
  - 同じファイルの `compute_remaining_wall_wood` / `compute_remaining_wall_mud` も、毎回 `queries.storage.wall_tiles.iter()` を全件走査している。
  - `count_matching_incoming_deliveries` は `queries.reservation.incoming_deliveries_query.get(target)` で取得した delivery 群に対して、アイテムごとに `queries.reservation.resources.get(item)` を呼んで ResourceType を判定している。
  - これらの処理は Familiar ごとの委譲ループではなく、`src/systems/familiar_ai/decide/task_delegation.rs` の `familiar_task_delegation_system` が 0.5 秒ごとに回す Think フェーズ内で繰り返されるため、サイト数・タイル数・搬送中アイテム数が増えるほど CPU コストが増える。
- 到達したい状態:
  - 床/壁サイト需要の基礎計算が「全タイル Query 走査」ではなく「そのサイト配下の tile entity 一覧」だけを走査する実装になる。
  - `IncomingDeliveries` の resource type 判定が need 計算のたびに繰り返されず、委譲サイクル先頭で 1 回だけ集計される。
  - `ReservationShadow` を含む既存の need 算出ロジックと割り当て結果を変えずに、読み取りコストだけ削減する。
- 成功指標:
  - `floor_site_tile_demand` / `wall_site_tile_demand` 呼び出し元で全件 iterator を渡さなくなる。
  - `count_matching_incoming_deliveries` が削除されるか、少なくとも need 計算のホットパスから外れる。
  - `familiar_task_delegation_system` 内で snapshot 構築が 1 回だけ行われ、その参照が Familiar 全体で共有される。
  - 実装後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が成功する。

## 2. スコープ

### 対象（In Scope）

- `src/systems/logistics/` 配下に、サイトごとの tile entity 一覧を保持する Resource と同期システムを追加する。
- `src/plugins/spatial.rs` に、その同期システムを `GameSystemSet::Spatial` で登録する。
- `src/systems/familiar_ai/decide/task_delegation.rs` で、委譲サイクル先頭に `IncomingDeliveries` 集計スナップショットを構築する。
- `src/systems/familiar_ai/decide/familiar_processor.rs` と `src/systems/familiar_ai/decide/task_management/delegation/*.rs` を通して、snapshot と tile index を demand 計算へ渡す。
- `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs` の need 計算を index / snapshot ベースに置き換える。
- `docs/architecture.md` へ新規 Resource と実行フェーズ配置を追記する。

### 非対象（Out of Scope）

- `src/systems/soul_ai/execute/task_execution/haul/dropping.rs` や `haul_with_wheelbarrow/phases/unloading.rs` の最適化。
- `TransportRequest` の意味論変更や `ReservationShadow` の仕様変更。
- `SharedResourceCache`、`DesignationSpatialGrid`、`TransportRequestSpatialGrid` の更新方式変更。
- ベンチマーク基盤の新設や自動 perf テスト追加。

## 3. 現状とギャップ

### 3.1 現状のホットパス

1. `familiar_task_delegation_system` が `ReservationShadow::default()` を作成し、Familiar ごとに `process_task_delegation_and_movement` を呼ぶ。
2. `process_task_delegation_and_movement` から `TaskManager::delegate_task` が呼ばれ、`collect_scored_candidates` と assignment loop を経由して haul 系 policy に入る。
3. haul 系 policy から `policy/haul/demand.rs` の need 計算が呼ばれる。
4. need 計算内で、床/壁 construction site のたびに全 tile query を走査し、incoming のたびに resource query を引いている。

### 3.2 いま実際に触るファイル

| 役割 | ファイル | 現在の具体的な責務 |
| --- | --- | --- |
| 委譲サイクル起点 | `src/systems/familiar_ai/decide/task_delegation.rs` | timer gate 判定、`ReservationShadow` 作成、Familiar ループ |
| Delegation context | `src/systems/familiar_ai/decide/familiar_processor.rs` | `FamiliarDelegationContext` を組み立てて `TaskManager` へ渡す |
| Task delegation | `src/systems/familiar_ai/decide/task_management/delegation/mod.rs` / `assignment_loop.rs` | worker 候補選定と task assignment 実行 |
| 残需要計算 | `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs` | floor/wall/blueprint/stockpile/provisional wall の need 算出 |
| 床需要ヘルパー | `src/systems/logistics/floor_construction.rs` | `floor_site_tile_demand` が全 floor tile iterator を前提にしている |
| 壁需要ヘルパー | `src/systems/logistics/wall_construction.rs` | `wall_site_tile_demand` が全 wall tile iterator を前提にしている |
| tile spawn | `src/interface/selection/floor_place/floor_apply.rs` / `wall_apply.rs` | `FloorTileBlueprint::new` / `WallTileBlueprint::new` で tile を spawn |
| tile despawn | `src/systems/jobs/floor_construction/completion.rs` / `cancellation.rs`、`src/systems/jobs/wall_construction/completion.rs` / `cancellation.rs` | 完了/キャンセル時に tile と site を despawn |
| Spatial 系登録 | `src/plugins/spatial.rs` | `GameSystemSet::Spatial` に差分同期システムを登録 |

### 3.3 ギャップ

- 床/壁 tile の親 site 情報は `FloorTileBlueprint.parent_site` / `WallTileBlueprint.parent_site` として持っているが、逆引き index がない。
- incoming item は `IncomingDeliveries(Vec<Entity>)` としてぶら下がっているが、「destination ごとの resource type 集計」がない。
- その結果、need 計算のたびに「どの tile がこの site 配下か」「どの incoming item がこの resource type か」を毎回再計算している。

## 4. 実装方針（高レベル）

### 4.1 改善A: TileSiteIndex を導入する

- 新規ファイル候補: `src/systems/logistics/tile_index.rs`
- 追加する Resource:

```rust
#[derive(Resource, Default)]
pub struct TileSiteIndex {
    pub floor_tiles_by_site: HashMap<Entity, Vec<Entity>>,
    pub wall_tiles_by_site: HashMap<Entity, Vec<Entity>>,
}
```

- この Resource は「site entity -> tile entity 配列」だけを持つ。tile の state や delivered amount は引き続き既存 Query から読む。
- `GameSystemSet::Spatial` で毎フレーム同期し、need 計算は `queries.storage.floor_tiles.get(tile_entity)` / `queries.storage.wall_tiles.get(tile_entity)` で個別 tile を読む。

### 4.2 改善B: IncomingDeliverySnapshot を導入する

- 追加先候補: `src/systems/familiar_ai/decide/task_management/mod.rs`
- 追加する一時構造体:

```rust
pub struct IncomingDeliverySnapshot {
    pub by_destination_total: HashMap<Entity, u32>,
    pub by_destination_resource: HashMap<(Entity, ResourceType), u32>,
}
```

- これは Resource にしない。`familiar_task_delegation_system` の 1 回の実行中だけ使うローカル値にする。
- 構築時だけ `queries.reservation.resources.get(item)` を使い、need 計算側は `HashMap` 参照だけにする。

### 4.3 データの通し方

- `familiar_task_delegation_system` で `ReservationShadow` と並んで `IncomingDeliverySnapshot` を 1 回作る。
- `FamiliarDelegationContext` に `incoming_snapshot: &IncomingDeliverySnapshot` と `tile_site_index: &TileSiteIndex` を追加する。
- `TaskManager::delegate_task` と `try_assign_for_workers` と `assign_task_to_worker` の引数に同じ参照を追加し、最終的に `policy/haul/demand.rs` まで渡す。
- 需要計算は Query 自体を持ち続けるが、ホットパスでは
  - site 配下 tile 列挙: `TileSiteIndex`
  - incoming 件数参照: `IncomingDeliverySnapshot`
  を使う。

## 5. マイルストーン

## M1: Tile lifecycle と need 計算の接続点を固定する

- 変更内容:
  - `demand.rs` で index 化対象になる関数を 4 つに限定する:
    - `compute_remaining_floor_bones`
    - `compute_remaining_floor_mud`
    - `compute_remaining_wall_wood`
    - `compute_remaining_wall_mud`
  - tile spawn / despawn 契機を明示する:
    - spawn: `src/interface/selection/floor_place/floor_apply.rs:72-96`
    - spawn: `src/interface/selection/floor_place/wall_apply.rs:75-99`
    - despawn: `src/systems/jobs/floor_construction/completion.rs:120-160`
    - despawn: `src/systems/jobs/floor_construction/cancellation.rs:195-205`
    - despawn: `src/systems/jobs/wall_construction/completion.rs:35-64`
    - despawn: `src/systems/jobs/wall_construction/cancellation.rs:184-206`
  - snapshot 構築位置を `familiar_task_delegation_system` の `allow_task_delegation` 判定直後、Familiar ループ直前に固定する。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/task_delegation.rs`
  - `src/systems/familiar_ai/decide/familiar_processor.rs`
  - `src/systems/familiar_ai/decide/task_management/delegation/mod.rs`
  - `src/systems/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
  - `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs`
- 完了条件:
  - [ ] どの関数が index / snapshot を受け取るかシグネチャ単位で決まっている
  - [ ] tile spawn/despawn の入口がファイル単位で列挙されている
  - [ ] 追加する参照が Decide フェーズ内に閉じる設計になっている
- 検証:
  - `rust-analyzer` か `rg` で参照経路を再確認

## M2: TileSiteIndex を実装し SpatialPlugin に登録する

- 変更内容:
  - `src/systems/logistics/tile_index.rs` を新規作成する。
  - `src/systems/logistics/mod.rs` に `mod tile_index;` と `pub use tile_index::*;` を追加する。
  - `src/plugins/spatial.rs` に以下の import / system 登録を追加する:
    - `sync_floor_tile_site_index_system`
    - `sync_wall_tile_site_index_system`
    - 必要なら `cleanup_tile_site_index_on_site_removal_system`
  - 登録位置は `update_floor_construction_spatial_grid_system` の直後に置き、同じ `GameSystemSet::Spatial` に入れる。
- 具体的な実装ルール:
  - `Added<FloorTileBlueprint>` を読んで `floor_tiles_by_site[parent_site].push(entity)` する。
  - `Added<WallTileBlueprint>` を読んで `wall_tiles_by_site[parent_site].push(entity)` する。
  - `RemovedComponents<FloorTileBlueprint>` / `RemovedComponents<WallTileBlueprint>` を読んで、該当 tile entity を全 site vec から除去する。
  - vec が空になった site entry は `HashMap::remove` で片付ける。
  - 初版では O(number_of_sites_for_removal) の retain 実装でよい。site 数は tile 総数より小さいため、現在の全 tile 走査より十分安い。
- need 計算側の変更:
  - `src/systems/logistics/floor_construction.rs` の `floor_site_tile_demand` は
    - `impl Iterator<Item = &FloorTileBlueprint>` を受け取る形のままにせず、
    - `tile_entities: &[Entity]` と `q_tiles: &Query<...>` を受け取る helper に置き換えるか、
    - `demand.rs` 側で `tile_entities.iter().filter_map(|entity| q_tiles.get(*entity).ok())` を組み立てて渡す。
  - `src/systems/logistics/wall_construction.rs` も同様に置き換える。
- 変更ファイル:
  - `src/systems/logistics/tile_index.rs`（新規）
  - `src/systems/logistics/mod.rs`
  - `src/plugins/spatial.rs`
  - `src/systems/logistics/floor_construction.rs`
  - `src/systems/logistics/wall_construction.rs`
  - `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs`
- 完了条件:
  - [ ] `TileSiteIndex` が `app.init_resource::<TileSiteIndex>()` で初期化される
  - [ ] floor/wall tile の追加と削除で index が追随する
  - [ ] `compute_remaining_floor_*` / `compute_remaining_wall_*` が全件 query iterator を使わない
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - floor site / wall site を 1 つずつ作成し、site ごとの tile 件数を debug 出力で spot check

## M3: IncomingDeliverySnapshot を委譲サイクル先頭で構築する

- 変更内容:
  - `src/systems/familiar_ai/decide/task_management/mod.rs` に `IncomingDeliverySnapshot` と `impl IncomingDeliverySnapshot { fn build(...) -> Self }` を追加する。
  - `build(...)` の入力は次の 2 Query に限定する:
    - `&task_queries.reservation.incoming_deliveries_query`
    - `&task_queries.reservation.resources`
  - `src/systems/familiar_ai/decide/task_delegation.rs` の `let mut reservation_shadow = ...` の直後で snapshot を構築し、Familiar ループ内で共有する。
- 集計アルゴリズム:
  1. `incoming_deliveries_query.iter()` で `(destination_entity, &IncomingDeliveries)` を全 destination 分だけ走査する。
  2. `incoming.len()` を `by_destination_total[destination_entity]` に加算する。
  3. 各 `item` について `resources.get(item)` に成功したら `by_destination_resource[(destination_entity, resource_type)] += 1` する。
  4. `resources.get(item)` に失敗した item は既存ロジック同様にカウントしない。
- need 計算側の変更:
  - `count_exact_incoming_deliveries` を snapshot 参照版へ置き換える。
  - flexible material 用の `count_matching_incoming_deliveries` は
    - `accepted_types.iter().map(|t| snapshot.count_exact(target, *t)).sum()`
    に置き換える。
  - stockpile capacity 計算で使う total incoming も `incoming_deliveries_query.get(stockpile_entity).map(|incoming| incoming.len())` ではなく `snapshot.total_for(stockpile_entity)` を使う。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/task_management/mod.rs`
  - `src/systems/familiar_ai/decide/task_delegation.rs`
  - `src/systems/familiar_ai/decide/familiar_processor.rs`
  - `src/systems/familiar_ai/decide/task_management/delegation/mod.rs`
  - `src/systems/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
  - `src/systems/familiar_ai/decide/task_management/task_assigner.rs`
  - `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs`
- 完了条件:
  - [ ] snapshot 構築が `familiar_task_delegation_system` の 1 回の呼び出しにつき 1 回だけ
  - [ ] `demand.rs` で `queries.reservation.resources.get(item)` を呼ばなくなる
  - [ ] flexible material と exact match の両方が snapshot 経由になる
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - blueprint / stockpile / floor site / wall site / provisional wall の need 計算で回帰がないか手動確認

## M4: need 計算の引数整理と docs 更新を行う

- 変更内容:
  - `demand.rs` の関数群が `queries`, `shadow`, `tile_site_index`, `incoming_snapshot` を何度も受け取るなら、以下のような束ね構造を追加して引数爆発を防ぐ。

```rust
pub struct DemandReadContext<'a, 'w, 's> {
    pub queries: &'a FamiliarTaskAssignmentQueries<'w, 's>,
    pub shadow: &'a ReservationShadow,
    pub incoming_snapshot: &'a IncomingDeliverySnapshot,
    pub tile_site_index: &'a TileSiteIndex,
}
```

  - `docs/architecture.md` に `TileSiteIndex` を「Spatial セットで同期され、Think フェーズの construction need 計算で参照される Resource」として追記する。
  - 必要なら `docs/logistics.md` に floor/wall construction demand が site index を使うことを追記する。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs`
  - `docs/architecture.md`
  - 必要なら `docs/logistics.md`
- 完了条件:
  - [ ] need 計算の引数が実装可能な範囲に整理されている
  - [ ] ドキュメントが新しい Resource と実行順を説明している
- 検証:
  - docs 差分を人間が読んで実装位置を追えること

## 6. 実装詳細メモ

### 6.1 TileSiteIndex の同期戦略

- 初版では「追加は Added で追記、削除は RemovedComponents で全 vec から retain 除去」で十分。
- tile の `parent_site` は spawn 時に設定され、その後のゲーム内で site を付け替える実装は現在見当たらないため、`Changed<FloorTileBlueprint>` / `Changed<WallTileBlueprint>` を必須にしなくてよい。
- ただし将来 `parent_site` の付け替えが入る場合に備え、`debug_assert!` で `q_tiles.get(tile_entity)` から読める `parent_site` が index の site と一致しているかを spot check できる形にしておく。

### 6.2 IncomingDeliverySnapshot の API 例

```rust
impl IncomingDeliverySnapshot {
    pub fn total_for(&self, destination: Entity) -> u32 { ... }
    pub fn exact_for(&self, destination: Entity, resource_type: ResourceType) -> u32 { ... }
}
```

- flexible blueprint 需要では `accepted_types` を for loop で回して合計すればよい。predicate ベース API を残す必要はない。
- `by_destination_total` は stockpile capacity の `stored + incoming + shadow` 計算にそのまま使える。

### 6.3 既存関数の具体的な置換対象

- `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs`
  - `compute_remaining_floor_bones`
  - `compute_remaining_floor_mud`
  - `compute_remaining_wall_wood`
  - `compute_remaining_wall_mud`
  - `compute_remaining_stockpile_capacity`
  - `count_exact_incoming_deliveries`
  - `count_matching_incoming_deliveries`（削除候補）
- `src/systems/logistics/floor_construction.rs`
  - `floor_site_tile_demand`
- `src/systems/logistics/wall_construction.rs`
  - `wall_site_tile_demand`

### 6.4 この計画で触らない箇所

- `src/systems/soul_ai/execute/task_execution/haul/dropping.rs`
- `src/systems/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading.rs`
- `src/systems/logistics/transport_request/arbitration/*`

上記は今回の snapshot と似た問題を持つ可能性はあるが、Think フェーズ最適化とは別チケットに分離する。

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| tile index の remove 漏れ | 需要を過大計上する | completion/cancellation の両方で tile despawn が起きるファイルを M1 で固定し、RemovedComponents ベースでクリーンアップする |
| snapshot を Familiar ごとに構築してしまう | 期待した改善が出ない | `task_delegation.rs` の Familiar ループ外で build する実装を計画段階で固定する |
| 引数追加が広がりすぎる | 実装が散らかる | `DemandReadContext` を導入して `queries + shadow + index + snapshot` を束ねる |
| stockpile capacity だけ旧ロジックが残る | 一部ホットパスが残る | M3 の完了条件に `compute_remaining_stockpile_capacity` の snapshot 化を含める |
| `TileSiteIndex` を Logic セットへ置いてしまう | Think フェーズ開始時に未同期になる | `src/plugins/spatial.rs` の `GameSystemSet::Spatial` に限定して登録する |

## 8. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動確認シナリオ:
  1. 床サイトを 1 つ作成し、骨待ち tile 数に応じて `compute_remaining_floor_bones` が正しい値を返す。
  2. 壁サイトを 1 つ作成し、木材待ち tile 数に応じて `compute_remaining_wall_wood` が正しい値を返す。
  3. 途中搬送中の資源を site / blueprint / stockpile に向けて発生させ、need が incoming 分だけ減ることを確認する。
  4. site をキャンセルして tile が despawn したあと、index が空になることを確認する。
  5. site 完了で tile が despawn したあと、残 need が 0 のまま崩れないことを確認する。
- パフォーマンス確認:
  - `familiar_task_delegation_system` 冒頭と末尾の `Instant::now()` 差分は既に `perf_metrics.latest_elapsed_ms` に入っているため、変更前後でその値を比較する。
  - 高負荷シナリオでは「複数 floor/wall site を同時に置く」「搬送中 item を増やす」の 2 条件を同時に作る。

## 9. ロールバック方針

- 戻し方は 2 段階に分ける:
  1. snapshot 側だけ戻す  
     - `IncomingDeliverySnapshot` の導入差分を戻し、`demand.rs` を旧 `incoming_deliveries_query + resources.get(item)` 実装へ戻す。
  2. tile index 側も戻す  
     - `TileSiteIndex` と `SpatialPlugin` への登録を外し、`floor_site_tile_demand` / `wall_site_tile_demand` を全件 iterator 前提へ戻す。
- どちらも独立 revert できるよう、M2 と M3 を別コミットに分ける前提で進める。

## 10. 実施順序

1. `task_delegation.rs` / `familiar_processor.rs` / `task_management/delegation/*` の引数経路を確認する
2. `TileSiteIndex` を追加して `SpatialPlugin` に登録する
3. floor/wall need 計算を index ベースに置き換える
4. `IncomingDeliverySnapshot` を追加する
5. blueprint / stockpile / floor / wall / provisional wall need 計算を snapshot ベースに置き換える
6. `docs/architecture.md` を更新する
7. `cargo check` と手動確認を行う

## 11. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン:
  - なし
- 未着手/進行中:
  - M1〜M4 全て未着手

### 次のAIが最初にやること

1. `src/systems/familiar_ai/decide/task_delegation.rs` を開き、`ReservationShadow` を作っている位置の直後に snapshot を差し込む前提で引数経路を整理する。
2. `src/plugins/spatial.rs` を開き、`update_floor_construction_spatial_grid_system` の近くに `TileSiteIndex` 同期システムを追加する前提で順序を決める。
3. `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs` から着手し、floor/wall need 計算が全 tile iterator に依存している箇所を index 引数に置き換える。

### ブロッカー/注意点

- `TaskManager::delegate_task` から `policy/haul/demand.rs` までの呼び出し経路が長いので、先に `DemandReadContext` などの束ね方を決めてから変更する。
- `TileSiteIndex` を Resource にする以上、初期化忘れがあると起動時 panic になる。必ず plugin 側で `init_resource` する。
- `RemovedComponents<FloorTileBlueprint>` / `RemovedComponents<WallTileBlueprint>` は 1 フレーム遅れで読む前提なので、同一フレームでの Added/Removed 競合をログで確認できるようにしておくと安全。

### 参照必須ファイル

- `docs/proposals/think-phase-iteration-optimization-proposal-2026-03-07.md`
- `src/systems/familiar_ai/decide/task_delegation.rs`
- `src/systems/familiar_ai/decide/familiar_processor.rs`
- `src/systems/familiar_ai/decide/task_management/delegation/mod.rs`
- `src/systems/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
- `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs`
- `src/systems/logistics/floor_construction.rs`
- `src/systems/logistics/wall_construction.rs`
- `src/plugins/spatial.rs`
- `src/interface/selection/floor_place/floor_apply.rs`
- `src/interface/selection/floor_place/wall_apply.rs`
- `src/systems/jobs/floor_construction/completion.rs`
- `src/systems/jobs/floor_construction/cancellation.rs`
- `src/systems/jobs/wall_construction/completion.rs`
- `src/systems/jobs/wall_construction/cancellation.rs`

### 最終確認ログ

- 最終 `cargo check`: `N/A`（docs-only planning task）
- 未解決エラー:
  - なし

### Definition of Done

- [ ] `TileSiteIndex` が `GameSystemSet::Spatial` で同期される
- [ ] floor/wall need 計算が全 tile query 走査をやめている
- [ ] `IncomingDeliverySnapshot` が `familiar_task_delegation_system` の 1 回の呼び出しで 1 回だけ構築される
- [ ] `demand.rs` が incoming item ごとの ECS `get()` をホットパスで呼ばない
- [ ] `docs/architecture.md` に新規 Resource と実行順が追記される
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が成功する

## 12. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-07` | `AI (Copilot)` | 抽象表現を削り、対象関数・対象ファイル・追加データ構造・システム登録位置・検証手順を具体化 |
