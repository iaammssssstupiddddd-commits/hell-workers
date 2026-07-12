# ゲーム不変条件 (Game Invariants)

AI エージェントが**絶対に破ってはいけないルール**を定義します。
コードを変更する際はこのファイルを基準に判断してください。

---

## 1. Soul（ワーカー）の不変条件

### I-S1: AssignedTask と状態の整合
`AssignedTask` が `None` の Soul は Idle 状態でなければならない。
`AssignedTask` が Some の場合、`WorkingOn` Relationship が存在しなければならない。

### I-S2: CommandedBy は unassign_task が削除しない
`CommandedBy` の削除は `OnExhausted` / `OnStressBreakdown` Observer の責務。
`unassign_task` 内で `CommandedBy` を削除してはならない。
→ 詳細: [tasks.md §5](tasks.md)

### I-S3: OnTaskCompleted の発火条件
`task_execution_system` はハンドラが `TaskExecutionContext::complete_task` を呼び、`TaskEndDisposition::Completed` になったフレームでのみ `OnTaskCompleted` を trigger する。
`abort_retryable` / `abort_closed` / `clear_soul_assignment(Aborted*)` では発火しない。
`AssignedTask` を `None` にするだけでは発火しない（中断と完了の区別）。
chain 経由（gather → haul 等）で `AssignedTask::None` を経由しない完了は、従来どおり報酬なし。

### I-S4: OnTaskAbandoned は通知専用
`OnTaskAbandoned` を受け取った Observer は通知（音声等）のみ行う。
クリーンアップは呼び出し元の `unassign_task` が既に完了している。
Observer 内でタスク状態を変更してはならない。

### I-S5: Soul の突然 despawn
Soul が突然 despawn した場合、`OnTaskAbandoned` は発火しない。
予約の整合性回復は `resource_sync` の次回再構築（0.2秒以内）に委ねる。

---

## 2. Familiar（使い魔）の不変条件

### I-F1: Familiar は直接作業しない
Familiar は Soul への指揮・タスク割り当てのみを行う。
Familiar エンティティ自身が `WorkingOn` を持ってはならない。

### I-F2: リクルート閾値 < リリース閾値
リクルート直後にリリースされないよう、リクルート閾値はリリース閾値より低く設定されている。
この大小関係を逆転させてはならない。

---

## 3. タスクシステムの不変条件

### I-T1: Haul 系 WorkType には TransportRequest が必須
⚠️ **サイレント失敗**: 以下の WorkType は `TransportRequest` コンポーネントがないと
`task_finder` のフィルタで無音スキップされる（エラーもログも出ない）:
`Haul` / `HaulToMixer` / `GatherWater` / `HaulWaterToMixer` / `WheelbarrowHaul`

### I-T2: Bevy Relationship の Target は手動で書かない
Bevy 0.19 の Relationship は Source 側を操作すれば Target 側が自動更新される。
Target 側（`TaskWorkers`, `ManagedTasks`, `HeldBy`, `StoredItems`, `IncomingDeliveries`, `Commanding`）を手動で書いてはならない。
→ 詳細: [tasks.md §2.1](tasks.md)

### I-T3: タスクの二重割当禁止
1つの task エンティティに対して `TaskWorkers.len() < TaskSlots.max`（デフォルト 1）を超える割り当てをしてはならない。
`SharedResourceCache` による排他制御を迂回してはならない。

### I-T4: Designation 削除 = タスク消滅
`Designation` を削除するとタスクが消滅する。
`unassign_task` は Designation を削除しない（再試行を許可するため）。
Designation を削除するのはタスクを完全に取り消す場合のみ。

### I-T5: DesignationSpatialGrid の反映タイミング
`DesignationSpatialGrid` / `TransportRequestSpatialGrid` は Change Detection で動作し、
スポーン後の**次フレーム**で反映される。
スポーン直後のフレームではタスクが発見されない可能性がある（これは仕様）。

---

## 4. 物流（Logistics）の不変条件

### I-L1: SharedResourceCache 予約の解放責務
`unassign_task` は `SharedResourceCache` の予約を解放する責務を持つ。
タスクを中断・放棄する全経路で `unassign_task` を呼ぶこと。
直接 `AssignedTask` を `None` にリセットすると予約がリークする。

### I-L2: StasisMud / Sand の地面放置制限
StasisMud と Sand は地面にドロップされた状態で **5秒後に消滅** する。
以下のいずれかがあれば消滅しない:
`ReservedForTask` / `LoadedIn` / `StoredIn` / `DeliveringTo`

### I-L3: 容量判定の方法
Stockpile の容量判定は `StoredItems.len() + IncomingDeliveries.len() < capacity` で行う。
`StoredItems.len() < capacity` だけで判定してはならない（配送中分が無視される）。

### I-L4: タンク・ミキサー内の水アイテムには Transform が必須
`pouring.rs` でタンクまたはミキサー内にスポーンする水アイテムには必ず `Transform::default()` を付与すること。
`filling.rs` および `refine.rs` の `resource_items` クエリは `&Transform` を要求しており、Transform がなければ
ストア済みの水が一切検出されず、搬入→中断のループが発生する（アイテムは `Visibility::Hidden` のため
Transform があっても描画・空間グリッドには影響しない）。

### I-L5: BucketTransportData.amount は GoingToDestination 移行前に確定させること
GoingToBucket フェーズで `routing::transition_to_destination` を呼び出す時点では、
`BucketTransportData.amount` が実際に運搬するバケツ容量（`BUCKET_CAPACITY`）に設定済みでなければならない。
`amount == 0` のまま GoingToDestination に遷移すると、`going_to_destination.rs` が空バケツと判断して
タンクへの再充填ループを引き起こす。

---

## 5. UI / Visual の不変条件

### I-U1: UI は simulation state を直接変更しない
UI システムはシミュレーション状態（Soul のバイタル、タスク状態等）を直接変更してはならない。
変更はイベント（Request 系）または Command を通じて行う。
→ 詳細: [events.md](events.md)

### I-U2: システムセット実行順の遵守
`Input → Spatial → Logic → Actor → Visual → Interface` の順序は固定。
Visual / Interface フェーズから Logic フェーズのリソースに書き込んではならない。

### I-U3: RemovedComponents は毎フレーム全リーダーを消費すること
`RemovedComponents<T>` は Bevy の `EventReader` で実装されており、イベントは**2フレーム**で期限切れになる。
複数の `RemovedComponents` リーダーを `||` 短絡評価で並べると、
最初の条件が成立した時点で残りのリーダーが消費されず、イベントが期限切れで消失する。

```rust
// NG: 短絡評価で後続リーダーが消費されないことがある
let changed = removed_a.read().next().is_some() || removed_b.read().next().is_some();

// OK: |= で全リーダーを必ず消費する
let mut changed = false;
changed |= removed_a.read().next().is_some();
changed |= removed_b.read().next().is_some();
```

---

## 6. ECS アーキテクチャの不変条件

### I-A1: leaf crate への逆依存禁止
`hw_*` crate は `bevy_app` に依存してはならない（依存グラフが循環する）。
`bevy_app` が `hw_*` に依存するのは正当。逆は禁止。

### I-A2: leaf crate は root へ逆依存しない
`hw_*` crate は `bevy_app` に依存してはならない。Bevy の `Commands`・`Query`・`Res` を使う
system / Observer は、所有する leaf crate に置いてよい。

root (`bevy_app`) は、window / asset / UI adapter / production plugin wiring と、root 固有 Resource を
必要とする ECS 接続を担当する。leaf の system を root 側へ戻して依存方向や登録責務を曖昧にしてはならない。

---

## 7. セーブ/ロードの不変条件

### I-P1: spawn 時コンポーネントは allow-list か shell のどちらかに必ず登録
セーブ対象エンティティ（Soul / Familiar / Building 等）へ spawn 時に付与するコンポーネントを追加したら、
永続化すべき simulation 状態なら `systems/save/saving.rs` の allow-list + `register.rs` へ、
実行時状態なら対応する `attach_*_shell`（spawn とロード後 rehydrate の共用関数）へ追加する。
どちらにも登録しないと**ロード後にだけ**そのコンポーネントが欠落するサイレントバグになる。
詳細: [docs/save_load.md](save_load.md)

### I-P2: タプルキーのコレクションを保存対象型に持ち込まない
`HashMap<(i32,i32), _>` / `HashSet<(i32,i32)>` を含む型を allow-list に入れると、
ロード時に `DynamicMap::insert_boxed` がタプルの `reflect_hash`（bevy_reflect 0.19 未実装）を
要求して panic する。enum キーは `enum_hash` があるため可。どうしても必要な場合は
`WorldMap` と同様に serde derive + `#[reflect(Serialize, Deserialize)]` で型全体を
serde 経路にする（`crates/hw_world/src/map/mod.rs` 参照）。

---

## 8. 経路探索 (Pathfinding) の不変条件

### I-PF1: 歩行可否を変える WorldMap 変更は必ず obstacle_version を bump する
⚠️ **サイレント失敗**: `WorldMap` は歩行可否の世代番号 `obstacle_version` を持つ。
Soul のパス再利用（`pathfinding/reuse.rs`）と `pathfinding_system` の per-tick スキップ
（`can_skip_pathfinding_tick`）は、「`Path.validated_obstacle_version == WorldMap.obstacle_version`
なら経路上の障害物再検証を省略する」ことで成立している。

したがって **`is_walkable` の入力（`obstacles` / `door_states` / `bridged_tiles` / 地形 `tiles`）を
変える全ての mutation は `bump_obstacle_version()` を通過しなければならない**。
既存の全経路（`add_obstacle` / `add_grid_obstacle(s)` / `set_building_occupanc*` /
`clear_building_*` / `add_door` / `remove_door` / `set_door_state` / `sync_door_passability` /
`register_door` / `set_terrain_at_idx` / `add_bridged_tile`）は満たしている。

新しく歩行可否を変える setter を追加して bump を忘れると、**壁・扉を建てても Soul が
古いパスを再利用して障害物に突っ込む**（エラーもログも出ない）。特に walkable → blocked
方向（障害物追加・扉ロック）の漏れが危険。`bump_obstacle_version` は wrapping で単調増加し、
値そのものに意味はない（一致=無変更の検知にのみ使う）。
