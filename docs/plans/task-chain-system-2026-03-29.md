# タスクチェーンシステム実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `task-chain-system-plan-2026-03-29` |
| ステータス | `Draft` |
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

| 運搬タスク | 搬入先 | チェーン先 |
|:---|:---|:---|
| HaulToBlueprint（any素材） | Blueprint | Build |
| Haul（Bones） | FloorSite material_center | ReinforceFloorTile |
| Haul（StasisMud） | FloorSite material_center | PourFloorTile |
| Haul（Wood） | WallSite material_center | FrameWallTile |
| Haul（StasisMud） | WallSite material_center | CoatWall |

### 非対象（Out of Scope）

- Haul to Stockpile（チェーン先の作業が別場所）
- HaulWithWheelbarrow（終点が駐車場所であり作業場所ではない）
- HaulToMixer（素材変換機、作業タスクに直結しない）

## 3. 現状とギャップ

- **現状**: 運搬タスクは搬入完了後に `WorkingOn` を外して `AssignedTask::None` へ戻る。Familiar AI が翌フレームの Decide フェーズで別の Soul に作業タスクを割り当てる。
- **問題**: 搬入 Soul はすでに作業場所にいるにもかかわらず一旦離脱する。別 Soul の移動コストが発生し、タイムスロットの無駄がある。
- **本計画で埋めるギャップ**: 搬入完了直後の Execute フェーズ内でチェーン判定を行い、条件を満たせばそのままタスク移行する。

## 4. 実装方針（高レベル）

- **方針**: チェーンロジックを `chain.rs` として1箇所に集約する。個々のタスクハンドラ（haul_to_blueprint, dropping）は `chain.rs` の共通関数を呼ぶだけにする。
- **設計上の前提**:
  - Bevy の Perceive → Decide → Execute 実行順により、Execute 内のチェーンは同フレームの Decide より後に走るため二重割当は発生しない。
  - Blueprint への搬入は同期的（`bp.deliver_material()`）で即チェーン判定可。
  - FloorSite/WallSite への搬入は非同期（素材を ground drop し、翌フレームシステムでタイル状態遷移）。チェーンは `PickingUpX` フェーズへ遷移させて待機させる既存設計を活用する。
  - `FLOOR_BONES_PER_TILE=2` のため同一タイルに2体がチェーンしうる。`PickingUpBones` フェーズで競合を検出してアボートする。
- **Bevy 0.18 API での注意点**: `WorkingOn` Relationship の操作は Source 側（Soul）のみ行う。`remove::<WorkingOn>()` → `insert(WorkingOn(new_entity))` の順で置き換える。

## 5. マイルストーン

## M1: `StorageAccess` のタイル Query に Entity を追加

- **変更内容**: `StorageAccess.floor_tiles` と `.wall_tiles` の Query に `Entity` を追加し、`find_chain_opportunity` 内でタイルを Entity 付きで iterate できるようにする。既存呼び出し側を `(_, tile)` 形式に対応させる。
- **変更ファイル**:
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/access.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/reinforce_floor.rs`（`.get(tile_entity)` → `(_, tile)` 分解）
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/pour_floor.rs`（同上）
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/frame_wall.rs`（同上）
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/coat_wall.rs`（同上）
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul/dropping.rs`（`.iter()` → `(entity, tile)` 分解）
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading/capacity.rs`（`.iter()` 同上）
- **完了条件**:
  - [ ] `floor_tiles` / `wall_tiles` の型が `(Entity, &FloorTileBlueprint)` / `(Entity, &WallTileBlueprint)` になっている
  - [ ] 既存の全呼び出し側がコンパイルエラーなく動作する
- **検証**: `cargo check`

## M2: `chain.rs` 共通モジュールの作成

- **変更内容**: 以下2つの関数を持つ `chain.rs` を新規作成する。

  **`ChainOpportunity` enum**:
  ```
  Build { blueprint: Entity }
  ReinforceFloor { tile: Entity, site: Entity }
  PourFloor { tile: Entity, site: Entity }
  FrameWall { tile: Entity, site: Entity }
  CoatWall { tile: Entity, site: Entity }
  ```

  **`find_chain_opportunity(destination, resource_type, ctx) → Option<ChainOpportunity>`**:
  - destination entity が blueprint / floor_site / wall_site かを Query で判定
  - resource_type と照合してチェーン先を決定
  - タイル系はタイルを iterate し `parent_site` 一致・スロット空き・状態 `WaitingX` or `XReady` のものを1件返す
  - blueprint は `TaskSlots` / `TaskWorkers` を確認してスロット空きを検証

  **`execute_chain(opportunity, ctx, commands)`**:
  - `remove::<WorkingOn>()` → `insert(WorkingOn(task_entity))`
  - `ReserveSource { source: task_entity, amount: 1 }` をキュー
  - `*ctx.task = AssignedTask::...`（`PickingUpX` or `GoingToBlueprint` フェーズ）
  - path クリア・疲労微増（+0.05）

- **変更ファイル**:
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/chain.rs`（新規）
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/mod.rs`（`chain` モジュール追加）
- **完了条件**:
  - [ ] `chain.rs` が存在し `pub fn find_chain_opportunity` と `pub fn execute_chain` がエクスポートされている
  - [ ] `ChainOpportunity` の全バリアントが定義されている
- **検証**: `cargo check`

## M3: 呼び出し元へのチェーン組み込み

- **変更内容**: M2 の共通関数を以下2箇所から呼ぶ。

  **`haul_to_blueprint.rs` Delivering フェーズ**:
  `bp.deliver_material()` 後、`ctx.inventory.0 = None` の前に挿入：
  ```
  if let Some(opp) = find_chain_opportunity(blueprint_entity, Some(resource_type), ctx) {
      ctx.inventory.0 = None;
      execute_chain(opp, ctx, commands);
      commands.entity(item_entity).despawn();
      return;
  }
  ```

  **`dropping.rs` floor_site / wall_site ブランチ**:
  item 設置コマンド発行後、既存の `WorkingOn` 削除・task clear の前に挿入：
  ```
  if let Some(opp) = find_chain_opportunity(stockpile, item_resource_type, ctx) {
      ctx.inventory.0 = None;
      execute_chain(opp, ctx, commands);
      return;  // WorkingOn削除・task clear は execute_chain が担う
  }
  ```

- **変更ファイル**:
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul_to_blueprint.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul/dropping.rs`
- **完了条件**:
  - [ ] 2箇所の call site に `find_chain_opportunity` / `execute_chain` 呼び出しが追加されている
  - [ ] チェーンしない従来パスが壊れていない
- **検証**: `cargo check`

## M4: `PickingUpX` フェーズの3ケース化

- **変更内容**: チェーンで `PickingUpX` フェーズに入った Soul が正しく振る舞うよう、各フェーズの条件分岐を拡張する。

  | タイル状態 | 挙動 | 変更有無 |
  |:---|:---|:---|
  | `WaitingX`（素材未着） | 何もせず待機 | 既存 |
  | `XReady`（素材到着済） | 次フェーズへ進む | 既存 |
  | それ以外（競合・スキップ済など） | `ReleaseSource` してアボート | **追加** |

  `CoatWall::PickingUpMud`（non-legacy）は現在 `CoatingReady` 以外で即キャンセルしているため、`WaitingMud` を待機ケースに追加する。

- **変更ファイル**:
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/reinforce_floor.rs`（`PickingUpBones` に abort ケース追加）
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/pour_floor.rs`（`PickingUpMud` に abort ケース追加）
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/frame_wall.rs`（`PickingUpWood` に abort ケース追加）
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/coat_wall.rs`（`PickingUpMud` non-legacy: WaitingMud 待機追加、他状態 abort）
- **完了条件**:
  - [ ] 各 `PickingUpX` フェーズが WaitingX / Ready / other の3ケースに対応している
  - [ ] abort ケースで `ReleaseSource` を発行している
- **検証**: `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `FLOOR_BONES_PER_TILE=2` で同一タイルに2体がチェーン | 両方が `Reinforcing` 状態になろうとする | M4 の abort ケースで `Reinforcing` 状態のタイルを検出し2体目が離脱 |
| `execute_chain` 後に destination が消失 | チェーン先タスクが開始直後に消える | 次フレームで各タスクハンドラが entity gone をハンドル（既存ロジック） |
| FloorSite/WallSite のスロット管理が不整合 | 予約リーク | `execute_chain` が `ReserveSource` を発行するため M2 時点で検証する |
| `StorageAccess` の Query 型変更により既存コードが破損 | コンパイルエラー | M1 を独立マイルストーンにして `cargo check` で確認してから次へ進む |

## 7. 検証計画

- **必須**: 各マイルストーン後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- **手動確認シナリオ**:
  - Blueprint に素材を搬入した Soul が Build フェーズへ直接移行することを確認
  - FloorSite に Bones を搬入した Soul が `PickingUpBones` で待機し、状態遷移後に Reinforcing へ進むことを確認
  - 同一タイルに2体 Bones 搬入時、2体目が正しくアボートすることを確認

## 8. ロールバック方針

- マイルストーン単位で独立しているため、問題のある M を git revert して前の状態に戻せる
- M1（StorageAccess 変更）は他 M の前提なので M1 ロールバック時は全て戻す

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手: M1 → M2 → M3 → M4 の順で実施

### 次のAIが最初にやること

1. `access.rs` の `floor_tiles` / `wall_tiles` 型を確認し M1 を実施
2. `cargo check` で M1 完了を確認
3. `chain.rs` 新規作成（M2）

### ブロッカー/注意点

- `floor_tiles` / `wall_tiles` の `.iter()` 呼び出しが `dropping.rs` と `haul_with_wheelbarrow/phases/unloading/capacity.rs` にある。M1 でこれらも `(entity, tile)` 形式に更新すること。
- `CoatWall::PickingUpMud` の non-legacy パスは現在 `CoatingReady` 以外で即キャンセルする実装。M4 で `WaitingMud` → 待機に変更する。legacy パス（`site == Entity::PLACEHOLDER`）は変更不要。
- `execute_chain` 内での `AssignedTask::CoatWall` 設定時、`wall` フィールドは `Entity::PLACEHOLDER` にする（`PickingUpMud` フェーズで `spawned_wall` を参照して確定させる既存設計に従う）。

### 参照必須ファイル

- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/access.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul/dropping.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul_to_blueprint.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/reinforce_floor.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/pour_floor.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/frame_wall.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/coat_wall.rs`
- `crates/hw_jobs/src/tasks/mod.rs`（AssignedTask バリアント確認用）

### 最終確認ログ

- 最終 `cargo check`: `未実施`
- 未解決エラー: なし（実装前）

### Definition of Done

- [ ] M1〜M4 が全て完了
- [ ] `cargo check` が成功（警告ゼロ）
- [ ] チェーンが発生するケース・発生しないケース両方で既存動作が壊れていない

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-29` | `Claude` | 初版作成 |
