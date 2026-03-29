# タスクチェーンシステム実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `task-chain-system-plan-2026-03-29` |
| ステータス | `Completed` |
| 作成日 | `2026-03-29` |
| 最終更新日 | `2026-03-29` |
| 作成者 | `Claude` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- **解決したい課題**: 運搬タスクを完了した Soul が一旦アイドルに戻り、別の Soul が同じ場所で作業タスクを開始するという非合理な分断が発生している。搬入直後に同じ Soul が作業に移行できれば効率が上がる。
- **到達したい状態**: 「運搬完了地点 = 次の作業開始地点」であるすべてのケースで、同一 Soul がチェーン移行できる。
- **成功指標**: Blueprint/FloorSite/WallSite へ搬入した Soul が、スロット空きがある場合に翌フレームを待たず作業フェーズへ遷移する。

## 2. スコープ

### 対象（In Scope）

| 運搬タスク | 搬入先 | チェーン先タスク | チェーン開始フェーズ |
|:---|:---|:---|:---|
| HaulToBlueprint（any素材） | Blueprint | Build | `BuildPhase::GoingToBlueprint` |
| Haul（Bone） | FloorSite material_center | ReinforceFloorTile | `ReinforceFloorPhase::PickingUpBones` |
| Haul（StasisMud） | FloorSite material_center | PourFloorTile | `PourFloorPhase::PickingUpMud` |
| Haul（Wood） | WallSite material_center | FrameWallTile | `FrameWallPhase::PickingUpWood` |
| Haul（StasisMud） | WallSite material_center | CoatWall | `CoatWallPhase::PickingUpMud` |

### 非対象（Out of Scope）

- Haul to Stockpile（チェーン先の作業が別場所）
- HaulWithWheelbarrow（終点が駐車場所であり作業場所ではない）
- HaulToMixer（素材変換機、作業タスクに直結しない）

## 3. 現状とギャップ

- **現状**: 運搬タスクは搬入完了後に `WorkingOn` を外して `AssignedTask::None` へ戻る。Familiar AI が翌フレームの Decide フェーズで別の Soul に作業タスクを割り当てる。
  - `haul_to_blueprint.rs` Delivering フェーズ終端（`~L234-239`）: `ctx.inventory.0 = None` → `remove::<WorkingOn>()` → `clear_task_and_path` → `despawn(item)` の順でクリア
  - `dropping.rs` 終端（`L310-312`）: `ctx.inventory.0 = None` → `remove::<WorkingOn>()` → `clear_task_and_path` の共通クリーンアップ
- **問題**: 搬入 Soul はすでに作業場所にいるにもかかわらず一旦離脱する。別 Soul の移動コストが発生し、タイムスロットの無駄がある。
- **本計画で埋めるギャップ**: 搬入完了直後の Execute フェーズ内でチェーン判定を行い、条件を満たせばそのままタスク移行する。

## 4. 実装方針（高レベル）

- **方針**: チェーンロジックを `chain.rs` として1箇所に集約する。個々のタスクハンドラ（haul_to_blueprint, dropping）は `chain.rs` の共通関数を呼ぶだけにする。
- **設計上の前提**:
  - Bevy の Perceive → Decide → Execute 実行順により、Execute 内のチェーンは同フレームの Decide より後に走るため二重割当は発生しない。
  - Blueprint への搬入は同期的（`bp.deliver_material()`）で即チェーン判定可。`bp.materials_complete()` が true になった場合のみ Build チェーンを発火する。
  - FloorSite/WallSite への搬入は非同期（素材を ground drop し、翌フレームシステムでタイル状態遷移）。チェーンは `PickingUpX` フェーズへ遷移させて待機させる既存設計を活用する。
  - `FLOOR_BONES_PER_TILE=2` のため同一タイルに2体がチェーンしうる。`PickingUpBones` フェーズで `TaskWorkers` を見て競合を検出しアボートする。
- **Bevy 0.18 API での注意点**: `WorkingOn` Relationship の操作は Source 側（Soul）のみ行う。`remove::<WorkingOn>()` → `insert(WorkingOn(new_entity))` の順で置き換える。

## 5. マイルストーン

---

## M1: `StorageAccess` のタイル Query を `(Entity, &TileBlueprint, Option<&TaskWorkers>)` に拡張

### 変更の目的

`find_chain_opportunity`（M2）がタイルを Entity 付きで iterate し、かつ各タイルのスロット競合（`TaskWorkers`）を確認できるようにする。

### 具体的な型変更

> **⚠️ 修正（レビュー反映）**: `TaskExecutionContext` が使う `TaskQueries.storage` は `MutStorageAccess`（queries.rs:178）であり `StorageAccess` ではない。変更対象を `MutStorageAccess` に修正する。

**`crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/access.rs`（`MutStorageAccess` 構造体）**

| フィールド | 変更前 | 変更後 |
|:---|:---|:---|
| `floor_tiles`（L221） | `Query<'w, 's, &'static mut hw_jobs::construction::FloorTileBlueprint>` | `Query<'w, 's, (Entity, &'static mut hw_jobs::construction::FloorTileBlueprint, Option<&'static hw_core::relationships::TaskWorkers>)>` |
| `wall_tiles`（L231） | `Query<'w, 's, &'static mut hw_jobs::construction::WallTileBlueprint>` | `Query<'w, 's, (Entity, &'static mut hw_jobs::construction::WallTileBlueprint, Option<&'static hw_core::relationships::TaskWorkers>)>` |

> **Blueprint の TaskSlots/TaskWorkers は変更不要**: `DesignationAccess.designations`（`DesignationsAccessQuery`）が既に `Option<&TaskSlots>` / `Option<&TaskWorkers>` を持っている。`find_chain_opportunity` でのスロット空き確認は `ctx.queries.designation.designations.get(destination)` を使うことで `MutStorageAccess.blueprints` を変更せずに済む（→ M2 参照）。

### `.get()` / `.get_mut()` 呼び出し側の修正（戻り値の分解パターン変更）

> **注意**: これらのファイルは `TaskExecutionContext`（= `MutStorageAccess`）を使うため、`.get()` と `.get_mut()` が混在する。タプル変更は同じパターンで対応する。

| ファイル | 行 | 変更前 | 変更後 |
|:---|:---|:---|:---|
| `reinforce_floor.rs` | L69 | `let Ok(tile_blueprint) = ctx.queries.storage.floor_tiles.get(tile_entity)` | `let Ok((_, tile_blueprint, _)) = ctx.queries.storage.floor_tiles.get(tile_entity)` |
| `pour_floor.rs` | L67 | `let Ok(tile_blueprint) = ctx.queries.storage.floor_tiles.get(tile_entity)` | `let Ok((_, tile_blueprint, _)) = ctx.queries.storage.floor_tiles.get(tile_entity)` |
| `frame_wall.rs` | L53 | `let Ok(tile_blueprint) = ctx.queries.storage.wall_tiles.get(tile_entity)` | `let Ok((_, tile_blueprint, _)) = ctx.queries.storage.wall_tiles.get(tile_entity)` |
| `coat_wall.rs` | L210 | `let Ok(tile_blueprint) = ctx.queries.storage.wall_tiles.get(tile_entity)` | `let Ok((_, tile_blueprint, _)) = ctx.queries.storage.wall_tiles.get(tile_entity)` |

### `.iter()` 呼び出し側の修正（外部関数へのイテレータ渡し）

`floor_site_tile_demand` / `wall_site_tile_demand`（`hw_logistics` crate 内）のシグネチャは `impl Iterator<Item = &'a FloorTileBlueprint>` のため、変更不要。  
呼び出し側で `.map(|(_, t, _)| t)` を追加して型を合わせる。

| ファイル | 行 | 変更前 | 変更後 |
|:---|:---|:---|:---|
| `dropping.rs` | L25-29 | `ctx.queries.storage.floor_tiles.iter()` | `ctx.queries.storage.floor_tiles.iter().map(\|(_, t, _)\| t)` |
| `dropping.rs` | L50-54 | `ctx.queries.storage.wall_tiles.iter()` | `ctx.queries.storage.wall_tiles.iter().map(\|(_, t, _)\| t)` |
| `capacity.rs` | L18 | `ctx.queries.storage.floor_tiles.iter()` | `ctx.queries.storage.floor_tiles.iter().map(\|(_, t, _)\| t)` |
| `capacity.rs` | L42 | `ctx.queries.storage.wall_tiles.iter()` | `ctx.queries.storage.wall_tiles.iter().map(\|(_, t, _)\| t)` |

### blueprints `.get()` 呼び出し側の修正

> **⚠️ 修正（レビュー反映）**: M1 で `MutStorageAccess.blueprints` を変更しないため、既存の `.get()`/`.get_mut()` 呼び出しのタプルパターン変更は**不要**。
>
> `ctx.queries.storage.blueprints` を参照している箇所は以下の2ファイルのみ（実際に grep して確認）：
> - `haul_to_blueprint.rs`：`let q_blueprints = &mut ctx.queries.storage.blueprints;`（L27）を介した `.get()`（L120）/ `.get_mut()`（L179）
> - `haul_with_wheelbarrow/going_to_destination.rs`：直接の `.get()` 呼び出し（L76）
>
> これらは `(&Transform, &mut Blueprint, Option<&Designation>)` のまま変更なしでよい。

### 完了条件

- [ ] `floor_tiles` / `wall_tiles` の型が `(Entity, &TileBlueprint, Option<&TaskWorkers>)` になっている
- [ ] `blueprints` の型に `Option<&TaskSlots>` / `Option<&TaskWorkers>` が追加されている
- [ ] `cargo check` がクリーン（警告ゼロ）

---

## M2: `chain.rs` 共通モジュールの作成

### ファイル

- **新規**: `crates/hw_soul_ai/src/soul_ai/execute/task_execution/chain.rs`
- **更新**: `crates/hw_soul_ai/src/soul_ai/execute/task_execution/mod.rs`（`pub(super) mod chain;` 追加）

### `ChainOpportunity` 定義

```rust
/// チェーン移行の種別と対象エンティティを表す
pub(super) enum ChainOpportunity {
    /// Blueprint の全素材が揃い Build スロットに空きがある
    Build { blueprint: Entity },
    /// FloorSite に Bone が搬入済みで WaitingBones タイルがある
    ReinforceFloor { tile: Entity, site: Entity },
    /// FloorSite に StasisMud が搬入済みで WaitingMud タイルがある
    PourFloor { tile: Entity, site: Entity },
    /// WallSite に Wood が搬入済みで WaitingWood タイルがある
    FrameWall { tile: Entity, site: Entity },
    /// WallSite に StasisMud が搬入済みで WaitingMud タイルがある（spawned_wall 確定済み）
    CoatWall { tile: Entity, site: Entity, wall: Entity },
}
```

### `find_chain_opportunity` 関数シグネチャ

```rust
pub(super) fn find_chain_opportunity(
    destination: Entity,
    resource_type: ResourceType,
    /// haul_to_blueprint.rs 専用: 呼び出し元が bp.materials_complete() を計算済みの場合に渡す。
    /// None の場合（dropping.rs 等）は Blueprint チェックをスキップする。
    materials_complete: Option<bool>,
    ctx: &TaskExecutionContext,
) -> Option<ChainOpportunity>
```

> **⚠️ 修正（レビュー反映）**: 引数 `materials_complete: Option<bool>` を追加。
>
> **背景**: `haul_to_blueprint.rs` は `let q_blueprints = &mut ctx.queries.storage.blueprints;` を関数スコープ全体で保持する。そのため `find_chain_opportunity(ctx)` を呼ぶ時点でも `q_blueprints` の可変借用が生きており、`ctx` を再借用しようとすると Rust の Borrow Checker に弾かれる。
>
> **回避策**: 呼び出し元（`haul_to_blueprint.rs`）で `bp` が生きている間に `bp.materials_complete()` の値を変数に取り出し、`bp` のスコープを閉じた後で `find_chain_opportunity` を呼ぶ。`materials_complete` の値は引数として渡す。
>
> `TaskSlots`/`TaskWorkers` の確認は `ctx.queries.designation.designations.get(destination)` で行う（`DesignationAccess.designations` は既に `Option<&TaskSlots>` / `Option<&TaskWorkers>` を持つため `MutStorageAccess.blueprints` を変更不要）。

### `find_chain_opportunity` の内部ロジック

```
1. materials_complete == Some(true) の場合のみ Blueprint チェックを実施:
   → destination が Blueprint かどうかを ctx.queries.designation.designations.get(destination) で確認
     (Designation を持つエンティティのみヒットするため、非 Blueprint なら Err になり自動スキップ)
     Ok((_, _, _, _, task_slots_opt, task_workers_opt, _, _)) の場合:
       a. スロット空き確認:
          let max = task_slots_opt.map_or(1, |s| s.max);
          let used = task_workers_opt.map_or(0, |w| w.len());
          if used >= max as usize { return None; }
       b. return Some(ChainOpportunity::Build { blueprint: destination })

2. destination が FloorConstructionSite かチェック: ctx.queries.storage.floor_sites.get(destination)
   → Ok の場合:
     a. resource_type に応じてタイル状態を検索:
        ResourceType::Bone:
          ctx.queries.storage.floor_tiles.iter()
            .find(|(_, tile, workers)|
              tile.parent_site == destination
              && tile.state == FloorTileState::WaitingBones
              && workers.map_or(true, |w| w.is_empty()))
          → Some((tile_entity, _, _)) → return Some(ChainOpportunity::ReinforceFloor { tile: tile_entity, site: destination })
        ResourceType::StasisMud:
          同様に FloorTileState::WaitingMud を検索
          → return Some(ChainOpportunity::PourFloor { ... })
        _ → return None

3. destination が WallConstructionSite かチェック: ctx.queries.storage.wall_sites.get(destination)
   → Ok の場合:
     a. resource_type に応じてタイル状態を検索:
        ResourceType::Wood:
          WallTileState::WaitingWood を検索
          → return Some(ChainOpportunity::FrameWall { tile, site: destination })
        ResourceType::StasisMud:
          WallTileState::WaitingMud かつ tile.spawned_wall.is_some() を検索
          → let wall = tile.spawned_wall.unwrap()
          → return Some(ChainOpportunity::CoatWall { tile, site: destination, wall })
        _ → return None

4. return None（非対象の destination）
```

> **Note**: `ResourceType::StasisMud` が FloorSite と WallSite 両方にマッチしうるが、`destination` は高々1種のサイト型しか持たないためコンフリクトなし。

### `execute_chain` 関数シグネチャ

```rust
pub(super) fn execute_chain(
    opportunity: ChainOpportunity,
    ctx: &mut TaskExecutionContext,
    commands: &mut Commands,
)
```

### `execute_chain` の内部ロジック（バリアント別）

**`Build { blueprint }`**:
```rust
// HaulToBlueprint 時点の WorkingOn は blueprint を指している想定だが
// 安全のため remove→insert で確実に更新する
commands.entity(ctx.soul_entity).remove::<WorkingOn>();
commands.entity(ctx.soul_entity).insert(WorkingOn(blueprint));
*ctx.task = AssignedTask::Build(BuildData {
    blueprint,
    phase: BuildPhase::GoingToBlueprint,
});
ctx.path.waypoints.clear();
ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
// Build の予約は WorkingOn Relationship で完結するため ReserveSource 不要
```

**`ReinforceFloor { tile, site }`**:
```rust
commands.entity(ctx.soul_entity).remove::<WorkingOn>();
commands.entity(ctx.soul_entity).insert(WorkingOn(tile));
*ctx.task = AssignedTask::ReinforceFloorTile(ReinforceFloorTileData {
    tile,
    site,
    phase: ReinforceFloorPhase::PickingUpBones,
});
ctx.path.waypoints.clear();
ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
// ReserveSource は WorkingOn Relationship で完結するため不要（auto_build.rs 確認済み）
```

**`PourFloor { tile, site }`**: ReinforceFloor と同様、フェーズを `PourFloorPhase::PickingUpMud` に変更。

**`FrameWall { tile, site }`**: ReinforceFloor と同様、フェーズを `FrameWallPhase::PickingUpWood` に変更。

**`CoatWall { tile, site, wall }`**:
```rust
commands.entity(ctx.soul_entity).remove::<WorkingOn>();
commands.entity(ctx.soul_entity).insert(WorkingOn(tile));
*ctx.task = AssignedTask::CoatWall(CoatWallData {
    tile,
    site,
    wall,   // find_chain_opportunity で spawned_wall.unwrap() 済みの値
    phase: CoatWallPhase::PickingUpMud,
});
ctx.path.waypoints.clear();
ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
// ReserveSource は WorkingOn Relationship で完結するため不要（auto_build.rs 確認済み）
```

> **注意**: CoatWall の `wall` フィールドに `Entity::PLACEHOLDER` を使う必要はない。`find_chain_opportunity` は `spawned_wall.is_some()` のタイルのみ返すため、`wall` は確定済みの provisional wall entity。

### 完了条件

- [ ] `chain.rs` に `ChainOpportunity`、`find_chain_opportunity`、`execute_chain` が定義されている
- [ ] `mod.rs` に `pub(super) mod chain;` が追加されている
- [ ] `cargo check` がクリーン

---

## M3: 呼び出し元へのチェーン組み込み

### `haul_to_blueprint.rs` — Delivering フェーズへの組み込み

**挿入位置**: `bp.deliver_material(resource_type, 1)` 呼び出し後、Blueprint のスコープ（`if let Ok(...)` ブロック）を**閉じてから**チェーン判定する。

> **⚠️ 修正（レビュー反映）**: `haul_to_blueprint.rs` は `let q_blueprints = &mut ctx.queries.storage.blueprints;` でクエリを関数スコープ全体で保持する。`bp.get_mut()` が生きている間に `find_chain_opportunity(ctx)` を呼ぶと Borrow Checker に弾かれる。**`bp` が有効な間に `materials_complete` を変数に取り出し、`bp` のスコープを閉じてから** チェーン判定を行うこと。

**変更後（構造の概略）**:
```rust
HaulToBpPhase::Delivering => {
    // ① resource_type と materials_complete を先に取り出す
    let res_item_opt = q_targets.get(item_entity).ok()
        .and_then(|(_, _, _, _, ri, _, _)| ri.map(|r| r.0));
    let Some(resource_type) = res_item_opt else { return; };

    let materials_complete = {
        let Ok((_, mut bp, _)) = q_blueprints.get_mut(blueprint_entity) else {
            cancel::cancel_haul_to_blueprint(ctx, item_entity, blueprint_entity, commands);
            return;
        };
        if bp.remaining_material_amount(resource_type) == 0 {
            cancel::cancel_haul_to_blueprint(ctx, item_entity, blueprint_entity, commands);
            return;
        }
        bp.deliver_material(resource_type, 1);
        // ... 既存の logging / ManagedBy 削除 / Priority 付与処理（変更なし） ...
        bp.materials_complete()
        // ↑ ここで bp のスコープが終わり、q_blueprints の可変借用が解放される
    };

    // ② bp スコープを抜けた後でチェーン判定（ctx を安全に渡せる）
    if let Some(opp) = chain::find_chain_opportunity(
        blueprint_entity, resource_type, Some(materials_complete), ctx
    ) {
        ctx.inventory.0 = None;
        chain::execute_chain(opp, ctx, commands);
        commands.entity(item_entity).despawn();
        reservation::release_destination(ctx, blueprint_entity); // 予約解放は必須
        ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
        return;
    }

    // ③ 非チェーン: 既存フロー（変更なし）
    ctx.inventory.0 = None;
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();
    clear_task_and_path(ctx.task, ctx.path);
    commands.entity(item_entity).despawn();
    ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
    reservation::release_destination(ctx, blueprint_entity);
}
```

> **注意**: 実際の変更は既存コードの構造を確認しながら行うこと。上記は意図の概略であり、既存の `if let Ok` ブロック構造を内側のスコープに変換する必要がある。

### `dropping.rs` — floor_site / wall_site ブランチへの組み込み

**挿入位置**: 各ブランチ内でアイテム設置コマンドを全発行した直後、かつ次のブランチや `L310-312` への fall-through の直前。

**floor_site ブランチ（`~L248` 直後）**:
```rust
// ★ チェーン判定（DropしたリソースタイプでWaitingBonesまたはWaitingMudのタイルを探す）
// dropping.rs では Blueprint の materials_complete は関係しないため None を渡す
if let Some(resource_type) = item_resource_type {
    if let Some(opp) = chain::find_chain_opportunity(stockpile, resource_type, None, ctx) {
        ctx.inventory.0 = None;
        chain::execute_chain(opp, ctx, commands);
        return; // L310-312 の共通クリーンアップをスキップ（execute_chain が WorkingOn 変更を担う）
    }
}
```

**wall_site ブランチ（`~L273` 直後）**: 同様のコードを挿入。`stockpile` は wall_site entity。

**L310-312（共通クリーンアップ）は変更しない**:
```rust
// チェーンしなかった場合のみここに到達する（変更なし）
ctx.inventory.0 = None;
commands.entity(ctx.soul_entity).remove::<WorkingOn>();
clear_task_and_path(ctx.task, ctx.path);
```

### 完了条件

- [ ] `haul_to_blueprint.rs` Delivering フェーズに `find_chain_opportunity` / `execute_chain` 呼び出しが追加されている
- [ ] `dropping.rs` floor_site / wall_site ブランチそれぞれに `find_chain_opportunity` / `execute_chain` 呼び出しが追加されている
- [ ] `release_destination` が haul_to_blueprint のチェーンパスで呼ばれている
- [ ] 非チェーンパス（従来フロー）が壊れていない
- [ ] `cargo check` がクリーン

---

## M4: `PickingUpX` フェーズの3ケース化

チェーンで `PickingUpX` に入った Soul が競合状態に陥った場合、正しく離脱できるよう各フェーズを拡張する。

### 現状と変更後の対応表

| ファイル | フェーズ | 現状の挙動 | 追加する挙動 |
|:---|:---|:---|:---|
| `reinforce_floor.rs` L68 | `PickingUpBones` | tile gone → abort<br>`ReinforcingReady` → GoingToTile<br>それ以外 → **何もしない（待機）** | それ以外に `WaitingBones` **なし**（他 Soul が先に着手 = `Reinforcing` 等）→ `ReleaseSource` してアボート |
| `pour_floor.rs` L66 | `PickingUpMud` | tile gone → abort<br>`PouringReady` → GoingToTile<br>それ以外 → 何もしない | 同様: `WaitingMud` でない かつ `PouringReady` でもない → アボート |
| `frame_wall.rs` L52 | `PickingUpWood` | tile gone → abort<br>`FramingReady` → GoingToTile<br>それ以外 → 何もしない | 同様: `WaitingWood` でない かつ `FramingReady` でもない → アボート |
| `coat_wall.rs` L209 | `PickingUpMud`（non-legacy） | tile gone → cancel<br>`spawned_wall` None → cancel<br>`CoatingReady` でない → **即キャンセル** | `WaitingMud` → 待機に変更<br>`CoatingReady` → GoingToTile（既存）<br>それ以外 → cancel（既存） |

### `reinforce_floor.rs` の変更（代表例）

**変更前（L68-91）**:
```rust
ReinforceFloorPhase::PickingUpBones => {
    let Ok(tile_blueprint) = ctx.queries.storage.floor_tiles.get(tile_entity)
    else {
        clear_task_and_path(ctx.task, ctx.path);
        commands.entity(ctx.soul_entity).remove::<WorkingOn>();
        return;
    };
    if matches!(tile_blueprint.state, FloorTileState::ReinforcingReady) {
        *ctx.task = AssignedTask::ReinforceFloorTile(ReinforceFloorTileData {
            tile: tile_entity, site: site_entity,
            phase: ReinforceFloorPhase::GoingToTile,
        });
        ctx.path.waypoints.clear();
    }
    // else: WaitingBones → 何もせず次フレームも PickingUpBones のまま待機
}
```

**変更後**:
```rust
ReinforceFloorPhase::PickingUpBones => {
    let Ok((_, tile_blueprint, _)) = ctx.queries.storage.floor_tiles.get(tile_entity)
    else {
        clear_task_and_path(ctx.task, ctx.path);
        commands.entity(ctx.soul_entity).remove::<WorkingOn>();
        return;
    };
    match tile_blueprint.state {
        FloorTileState::WaitingBones => {
            // 素材未着。次フレームも PickingUpBones のまま待機（何もしない）
        }
        FloorTileState::ReinforcingReady => {
            *ctx.task = AssignedTask::ReinforceFloorTile(ReinforceFloorTileData {
                tile: tile_entity, site: site_entity,
                phase: ReinforceFloorPhase::GoingToTile,
            });
            ctx.path.waypoints.clear();
        }
        _ => {
            // 競合: 他の Soul が Reinforcing に入った等 → タイルスロットを解放してアボート
            // ★ ReleaseSource の具体的なキュー方法は既存タスクキャンセル処理を参照
            clear_task_and_path(ctx.task, ctx.path);
            commands.entity(ctx.soul_entity).remove::<WorkingOn>();
        }
    }
}
```

> `PourFloorTile`・`FrameWallTile` も同パターンで変更（`WaitingMud` / `WaitingWood` → 待機、それ以外 → アボート）。

### `coat_wall.rs` non-legacy PickingUpMud の変更（L209-230）

**変更前**:
```rust
CoatWallPhase::PickingUpMud => {
    let Ok((_, tile_blueprint, _)) = ctx.queries.storage.wall_tiles.get(tile_entity) else {
        cancel_coat_wall_task(...); return;
    };
    let Some(actual_wall) = tile_blueprint.spawned_wall else {
        cancel_coat_wall_task(...); return;
    };
    if !matches!(tile_blueprint.state, WallTileState::CoatingReady) {
        cancel_coat_wall_task(..., "tile not ready"); return;  // WaitingMud もここでキャンセル ← 問題
    }
    // CoatingReady → GoingToTile へ進む
```

**変更後**:
```rust
CoatWallPhase::PickingUpMud => {
    let Ok((_, tile_blueprint, _)) = ctx.queries.storage.wall_tiles.get(tile_entity) else {
        cancel_coat_wall_task(...); return;
    };
    let Some(actual_wall) = tile_blueprint.spawned_wall else {
        cancel_coat_wall_task(...); return;
    };
    match tile_blueprint.state {
        WallTileState::WaitingMud => {
            // 素材未着。次フレームも PickingUpMud のまま待機（何もしない）
        }
        WallTileState::CoatingReady => {
            *ctx.task = AssignedTask::CoatWall(CoatWallData {
                tile: tile_entity, site: site_entity,
                wall: actual_wall,
                phase: CoatWallPhase::GoingToTile,
            });
            ctx.path.waypoints.clear();
        }
        _ => {
            cancel_coat_wall_task(ctx, tile_entity, commands, "tile state unexpected");
        }
    }
}
```

### 完了条件

- [ ] `reinforce_floor.rs` の `PickingUpBones`: WaitingBones / ReinforcingReady / other の3ケース
- [ ] `pour_floor.rs` の `PickingUpMud`: WaitingMud / PouringReady / other の3ケース
- [ ] `frame_wall.rs` の `PickingUpWood`: WaitingWood / FramingReady / other の3ケース
- [ ] `coat_wall.rs` の `PickingUpMud`（non-legacy）: WaitingMud 待機ケースが追加されている
- [ ] `cargo check` がクリーン

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `FLOOR_BONES_PER_TILE=2` で同一タイルに2体がチェーン | 両方が `Reinforcing` 状態になろうとする | M4 の abort ケースで `Reinforcing` 状態のタイルを検出し2体目が離脱 |
| `execute_chain` 後に destination が消失 | チェーン先タスクが開始直後に消える | 次フレームで各タスクハンドラが entity gone をハンドル（既存ロジック） |
| ~~`blueprints` Query 拡張で既存 `.get()` 呼び出しが壊れる~~（修正済み） | ~~コンパイルエラー~~ | **`MutStorageAccess.blueprints` は変更しないため、このリスクは解消** |
| `StorageAccess` ではなく `MutStorageAccess` を変更するため `.iter()` の戻り値型が変わる | コンパイルエラー | M1 で `.iter().map(\|(_, t, _)\| t)` を追加してから `cargo check` を通す |
| `haul_to_blueprint.rs` の Delivering フェーズで Borrow Checker エラー | コンパイルエラー | M3 で `bp.materials_complete()` を先にスコープ内で取り出し、ブロックを閉じてから `find_chain_opportunity` を呼ぶ |
| CoatWall チェーン時に `spawned_wall` が None | パニックまたはロジックエラー | `find_chain_opportunity` で `spawned_wall.is_some()` をガード条件に含める |
| ~~ReserveSource 不足でスロットカウントがリーク~~（確認済み） | ~~過剰割り当て~~ | **Build・Floor・Wall 作業タスクの割り当ては `WorkingOn` Relationship のみで追跡（`auto_build.rs` 確認済み）。`ReserveSource` は輸送系の素材予約専用のため、チェーン移行時に発行不要** |
| Build チェーン後に `release_destination` 漏れ | 輸送予約がリーク | M3 で early return 前に必ず `release_destination` を呼ぶこと（現実装は noop だが将来のため整合性を維持する） |

## 7. 検証計画

- **必須**: 各マイルストーン後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- **手動確認シナリオ**:
  1. Blueprint に最後の素材を搬入した Soul が、翌フレームを待たず Build の `GoingToBlueprint` フェーズへ移行する
  2. Blueprint に素材を搬入したが Build スロットが埋まっている場合、チェーンせず通常の None 状態に戻る
  3. FloorSite に Bone を搬入した Soul が `PickingUpBones` フェーズで待機し、素材到着後に `GoingToTile` → `Reinforcing` へ進む
  4. 同一タイルに2体 Bone 搬入時、2体目が `Reinforcing` 状態を検出してアボートし離脱する
  5. WallSite に StasisMud を搬入した Soul が `CoatWall::PickingUpMud` で `WaitingMud` を待機し、`CoatingReady` になったら GoingToTile へ進む

## 8. ロールバック方針

- マイルストーン単位で独立しているため、問題のある M を git revert して前の状態に戻せる
- M1（StorageAccess 変更）は他 M の前提なので M1 ロールバック時は全て戻す

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手: M1 → M2 → M3 → M4 の順で実施

### 次のAIが最初にやること

1. `access.rs` を開き、**`MutStorageAccess`**（L221, L231）の `floor_tiles`・`wall_tiles` の型を変更する（`StorageAccess` L155/L165 は変更しない）
2. `dropping.rs` / `capacity.rs` の `.iter()` 呼び出しに `.map(|(_, t, _)| t)` を追加する
3. `reinforce_floor.rs`(L69) / `pour_floor.rs`(L67) / `frame_wall.rs`(L53) / `coat_wall.rs`(L210) の `.get()` 戻り値を `(_, tile_blueprint, _)` に変更する
4. `cargo check` で M1 完了を確認する
5. `chain.rs` 新規作成（M2）：`find_chain_opportunity` の第3引数は `materials_complete: Option<bool>`、Blueprint スロット確認は `ctx.queries.designation.designations.get()` を使う
6. `haul_to_blueprint.rs` の Delivering フェーズを `bp` スコープを閉じてからチェーンを呼ぶ構造に変更し、`find_chain_opportunity` 呼び出しを追加（M3）
7. `dropping.rs` に `find_chain_opportunity(..., None, ctx)` 呼び出しを追加（M3）
8. `PickingUpX` フェーズを3ケース化（M4）

### ブロッカー/注意点

- **変更対象は `MutStorageAccess`（L221, L231）**。`StorageAccess`（L155, L165）と `FamiliarStorageAccess`（L94, L95）は変更しない。
- `floor_tiles`/`wall_tiles` の `.iter()` 呼び出しは `dropping.rs` (L26, L51) と `capacity.rs` (L18, L42) の4箇所。外部関数 `floor_site_tile_demand`/`wall_site_tile_demand` のシグネチャは変更不要（`.map()` で吸収）。
- **`MutStorageAccess.blueprints` は変更しない**。Blueprint のスロット確認（TaskSlots/TaskWorkers）は `ctx.queries.designation.designations.get(blueprint_entity)` で行う。`blueprints.get()` の呼び出しパターンは変更不要。
- **Borrow Checker 対策**: `haul_to_blueprint.rs` は `let q_blueprints = &mut ctx.queries.storage.blueprints;` を関数スコープ全体で持つ。`bp` が生きている間に `find_chain_opportunity(ctx)` を呼べない。必ず `bp.materials_complete()` を変数に取り出し `if let Ok(...)` ブロックを閉じてから `find_chain_opportunity` を呼ぶこと。
- `CoatWall::PickingUpMud` の non-legacy パスは現在 `CoatingReady` 以外で即キャンセルする実装（L218-220）。M4 で `WaitingMud` → 待機ケースを追加する。legacy パス（`site == Entity::PLACEHOLDER`）は変更不要。
- `execute_chain` 内での `AssignedTask::CoatWall` の `wall` フィールドは `find_chain_opportunity` から受け取った `wall: Entity`（= `spawned_wall.unwrap()`）を使う。`Entity::PLACEHOLDER` は使わない。
- M3 の `haul_to_blueprint.rs` では `release_destination(ctx, blueprint_entity)` を early return 前に必ず呼ぶこと（現在の実装は noop だが整合性のため）。
- **ReserveSource は不要**: Build・ReinforceFloor・PourFloor・FrameWall・CoatWall のタスク割り当てはすべて `WorkingOn` Relationship のみで追跡（`auto_build.rs` 確認済み）。チェーン移行時も `ReserveSource` は発行しない。

### 参照必須ファイル

| ファイル | 目的 |
|:---|:---|
| `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/access.rs` | M1 型変更の対象（`MutStorageAccess` L221/L231） |
| `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul/dropping.rs` | M1 iter 修正、M3 チェーン挿入 |
| `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading/capacity.rs` | M1 iter 修正 |
| `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul_to_blueprint.rs` | M3 チェーン挿入（bp スコープ分離が必要） |
| `crates/hw_soul_ai/src/soul_ai/execute/task_execution/reinforce_floor.rs` | M1 get 修正、M4 フェーズ拡張 |
| `crates/hw_soul_ai/src/soul_ai/execute/task_execution/pour_floor.rs` | M1 get 修正、M4 フェーズ拡張 |
| `crates/hw_soul_ai/src/soul_ai/execute/task_execution/frame_wall.rs` | M1 get 修正、M4 フェーズ拡張 |
| `crates/hw_soul_ai/src/soul_ai/execute/task_execution/coat_wall.rs` | M1 get 修正、M4 フェーズ拡張 |
| `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs` | TaskQueries/TaskAssignmentQueries の storage 型確認用 |
| `crates/hw_jobs/src/tasks/mod.rs` | AssignedTask バリアント確認 |
| `crates/hw_jobs/src/construction.rs` | FloorTileState / WallTileState enum 定義 |

### 型・定数クイックリファレンス

```
FloorTileState: WaitingBones / ReinforcingReady / Reinforcing{progress} / ReinforcedComplete / WaitingMud / PouringReady / Pouring{progress} / Complete
WallTileState:  WaitingWood / FramingReady / Framing{progress} / FramedProvisional / WaitingMud / CoatingReady / Coating{progress} / Complete

AssignedTask variants: None / Gather / Haul / HaulToBlueprint / Build / ... / ReinforceFloorTile / PourFloorTile / FrameWallTile / CoatWall / ...

BuildData          { blueprint: Entity, phase: BuildPhase }
BuildPhase         { GoingToBlueprint (default) / Building{progress} / Done }
ReinforceFloorTileData { tile: Entity, site: Entity, phase: ReinforceFloorPhase }
PourFloorTileData      { tile: Entity, site: Entity, phase: PourFloorPhase }
FrameWallTileData      { tile: Entity, site: Entity, phase: FrameWallPhase }
CoatWallData           { tile: Entity, site: Entity, wall: Entity, phase: CoatWallPhase }

ResourceReservationOp::ReserveSource  { source: Entity, amount: usize }
ResourceReservationOp::ReleaseSource  { source: Entity, amount: usize }
```

### 最終確認ログ

- 最終 `cargo check`: `未実施`
- 未解決エラー: なし（実装前）

### Definition of Done

- [ ] M1〜M4 が全て完了
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が成功（警告ゼロ）
- [ ] チェーンが発生するケース・発生しないケース両方で既存動作が壊れていない

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-29` | `Claude` | 初版作成 |
| `2026-03-29` | `Claude` | コードベース調査に基づき具体化（型・行番号・コードスニペット・クイックリファレンス追加） |
| `2026-03-29` | `Claude` | レビュー反映：M1変更対象を`StorageAccess`→`MutStorageAccess`(L221/L231)に修正、blueprints変更を廃止し`designation.designations`活用に変更、M3借用競合を修正（bp スコープ分離）、ReserveSource不要を確定 |
