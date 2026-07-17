# ゲーム不変条件 (Game Invariants)

AI エージェントが**絶対に破ってはいけないルール**を定義します。
コードを変更する際はこのファイルを基準に判断してください。

---

## 1. Soul（ワーカー）の不変条件

### I-S1: AssignedTask と状態の整合
`AssignedTask` が `None` の Soul は Idle 状態でなければならない。
`AssignedTask` が Some の Soul は通常 `WorkingOn` Relationship を持つ。ただしtargetのdespawnではBevyのRelationship cleanupが先に`WorkingOn`を除去し、次のtask executionまで`AssignedTask::Some + Without<WorkingOn>`が一時的に残り得る。
この一時状態はhandler/cleanupで解消するため、`task_execution_system`を`With<WorkingOn>`でfilterしてはならない。

### I-S2: CommandedBy は unassign_task が削除しない
`CommandedBy` の削除は `OnExhausted` / `OnStressBreakdown` Observer の責務。
`unassign_task` 内で `CommandedBy` を削除してはならない。
→ 詳細: [tasks.md §5](tasks.md)

### I-S3: OnTaskCompleted の発火条件
`task_execution_system` はハンドラが `TaskExecutionContext::complete_task` または
`complete_after_custom_cleanup` で正常終了を確定したフレームでのみ
`publish_task_completed` を呼ぶ。helper は domain `OnTaskCompleted` を trigger し、presentation
`TaskCompletedVisualMessage` を1回書き込む。
`abort_retryable` / `abort_closed` / `abort_retryable_after_custom_cleanup` では発火しない。
`AssignedTask` を `None` にするだけでは発火しない（中断と完了の区別）。
chain 経由（gather → haul 等）は root assignment を維持し、最後の segment の正常完了だけが
final current target/work type を持つ完了通知を1回発行する。

### I-S4: OnTaskAbandoned は通知専用
`OnTaskAbandoned` は presentation `Message` であり、受け取った MessageReader は通知（音声等）のみ行う。
クリーンアップは呼び出し元の `unassign_task`、または
`SoulTaskUnassignRequest` を適用する Soul AI Perceive 経路が既に完了している。
presentation system 内でタスク状態を変更してはならない。

### I-S5: Soul の突然 despawn
Soul が突然 despawn した場合、`OnTaskAbandoned` は発火しない。
`resource_sync` は `RemovedComponents<AssignedTask>` を全件消費して reservation dirty とし、次の Perceive で snapshot を再構築する。0.2秒 timer は安全監査であり、despawn 後の整合性回復を待たせるための遅延ではない。

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
最後の `WorkingOn` source が外れて `TaskWorkers` が消えた TransportRequest は、post-Soul-AI reconcile で同じ `Update` 内に `Pending` へ戻す。
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
直接 `AssignedTask` を `None` にリセットすると、`unassign_task` が行う `Release*` 要求の送信と task cleanup を飛ばす。signature 同期は次の Perceive/audit で snapshot を回復する安全網であり、中断経路の代替にしてはならない。

### I-L2: StasisMud / Sand の地面放置制限
StasisMud と Sand は地面にドロップされた状態で **5秒後に消滅** する。
以下のいずれかがあれば消滅しない:
`LoadedIn` / `StoredIn` / `DeliveringTo` / `StoredByMixer`

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
Bevy 0.19 の `RemovedComponents<T>` は removal message 用の `MessageCursor` を持つ。
`read()` は**実際に走査した iterator 要素まで**しか cursor を進めないため、`.next()` は1件だけ、
`.any()` は最初の match までしか消費しない。複数 reader を `||` に置くと、先行条件が true のとき
後続 reader は実行されない。未消費 message は次 frame へ持ち越され、buffer の更新後には失われ得る。

entity ID が不要な reader は `hw_core::ecs::drain_removed`、predicate が必要な reader は
`drain_removed_where` を使い、**全 reader を query の dirty 判定より先に**消費する。

```rust
use hw_core::ecs::{drain_removed, drain_removed_where};

// OK: 各 reader を最後まで消費してから結果を合成する
let removed_a = drain_removed(&mut removed_a);
let removed_b = drain_removed(&mut removed_b);
let removed_relevant = drain_removed_where(&mut removed_c, |entity| {
    q_targets.get(entity).is_ok()
});
let changed = query_dirty || removed_a || removed_b || removed_relevant;
```

reader の cursor を進めるだけの `read().next()`、predicate の `read().any(...)`、
および `read()` を含む短絡式を新規コードへ追加してはならない。

### I-U4: project shortcut と world-input capture の所有権を分離する

project-owned の edge-triggered keyboard shortcut は `bevy_app::input_actions` の resolver だけが raw
`ButtonInput<KeyCode>` を読み、consumer は frame-local semantic action を読む。新しい shortcut は binding
table、context/compatibility、owner classification test を同時に更新する。

`UiInputState.pointer_over_ui` は通常 UI hover、`world_input_captured` は Modal/Pause ownership であり、
同じ field に畳まない。world pointer/camera consumer は `world_input_blocked()` を使うが、UI 自身は hover で
停止せず capture 中だけ foreground ancestry に従う。overlay open request の受理 frame から pending capture と
`InputFocus` clear を成立させ、capture root の表示更新を待ってはならない。

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

### I-P1: spawn 時コンポーネントは allow-list、shell、または rehydrate helper に必ず登録
セーブ対象エンティティ（Soul / Familiar / Building 等）へ spawn 時に付与するコンポーネントを追加したら、
永続化すべき simulation 状態なら `systems/save/schema.rs` の型分類へ、
通常の実行時状態なら対応する `attach_*_shell`（spawn とロード後 rehydrate の共用関数）へ追加する。
source-aware obstacle provenance / navigation cache は `rehydrate_obstacle_runtime` の durable source matrix から
再構築する。どの経路にも登録しないと**ロード後にだけ**そのコンポーネントが欠落するサイレントバグになる。
`Blueprint` と floor / wall construction の visual mirror は durable component から完成形を作る rehydrate helperへ
追加し、virtual time pause 中に停止する Logic の差分同期だけへ委ねない。
詳細: [docs/save_load.md](save_load.md)

### I-P2: タプルキーのコレクションを保存対象型に持ち込まない
`HashMap<(i32,i32), _>` / `HashSet<(i32,i32)>` を含む型を allow-list に入れると、
ロード時に `DynamicMap::insert_boxed` がタプルの `reflect_hash`（bevy_reflect 0.19 未実装）を
要求して panic する。enum キーは `enum_hash` があるため可。どうしても必要な場合は
`WorldMap` と同様に serde derive + `#[reflect(Serialize, Deserialize)]` で型全体を
serde 経路にする（`crates/hw_world/src/map/mod.rs` 参照）。

### I-P3: loadのpreflight成功はlive apply成功を保証しない
loadは、header/seed/schema検証、staging `World`への静的preflight、rehydrate prerequisite検証を
**live persisted entityのdespawn前**に完了する。staging成功はReflect registry / component/resource
contractだけを保証し、live適用のtransaction保証ではない。live apply開始後に`Result`エラーが発生したら、
apply時の`EntityHashMap`に記録されたpartial entityを掃除し、同一schemaのrollback snapshotを復元して
通常loadと同じreset/finalize（cache reset、runtime正規化、rehydrate）を通す。raw Entity IDやRON bytesではなく、
Entity remap後のpersistent graphが回復対象である。詳細: [docs/save_load.md](save_load.md)

### I-P4: world replacementはLastで完結し、旧Entity参照を次frameへ渡さない
save/loadのapplyは`Last::SaveLoadApplySet`だけが実行し、`Update`のInput/Interfaceは
`SaveLoadState`への要求発行だけを行う。replace開始後はroot/plugin hookでmessage、selection、UI、visual、
entity-bearing cacheをclearし、old persisted entityをdespawnして`flush()`する。

Bevy 0.19の`RemovedComponents`は二重bufferであるため、new worldのwrite前に
`World::clear_trackers()`を2回呼んでold removalを破棄する。write後に手動clearしてはいけない。
loaded componentの`Added`/`Changed`は次frameの差分rebuildに残す。system-localにEntityを保持する場合は
`WorldEpoch`不一致時に最初の利用前にclearし、scratch bufferが毎回clearされる場合だけ例外とする。
`GatheringSpot`と`ParticipatingIn` / `GatheringParticipants`はruntime-onlyであり、新規saveへ含めず、
legacy bodyからはschema検証前に除去する。replace hookは旧spotとlinked visualを同時にdespawnする。

### I-P5: construction runtime cacheはload中に再構築し、WorldMapを再予約しない
Floor / Wall construction の `TileSiteIndex`、工程counter、Curing中の`CuringFootprint`は保存しない
runtime stateである。loadのexclusive rehydrateはSpatial/Logicの再開前に、tileからindex、工程rankから
counter、Curing siteからfootprintの順で同期再構築する。保存済み`WorldMap`はoccupancyの正本なので、
この再構築でobstacle/occupancyをreserveしてはならない。

---

## 8. 経路探索 (Pathfinding) の不変条件

### I-PF1: obstacle_version は最終 is_walkable topology の世代である
⚠️ **サイレント失敗**: `WorldMap` は最終的な `is_walkable` の世代番号
`obstacle_version` を持つ。
Soul のパス再利用（`pathfinding/reuse.rs`）と `pathfinding_system` の per-tick スキップ
（`can_skip_pathfinding_tick`）は、「`Path.validated_obstacle_version == WorldMap.obstacle_version`
なら経路上の障害物再検証を省略する」ことで成立している。

したがって **`is_walkable` の結果を変える mutation だけが
`bump_obstacle_version()` を通過しなければならない**。raw `obstacles` の bit が変わっても、
扉・橋・地形の最終判定で歩行可否が変わらない場合は bump しない。
`Open` と `Closed` の Door はどちらも歩行可能で、cost だけを変えるため世代は不変とする。
`Locked` への遷移と解除、Door の追加/削除を含め、terrain/bridge/障害物によって最終 walkability が
変わる場合だけ世代を進める。`replace_obstacle_bitmap` は全セルの最終 walkability を比較し、差分がある場合でも
1回だけ世代を進める。load 時は `replace_navigation_caches` が raw blocker / Door / Bridge cache を変更する前後の
topology を比較し、同じく最大1回だけ世代を進める。

新しく walkability を変える setter を追加して bump を忘れると、**壁・扉を建てても Soul が
古いパスを再利用して障害物に突っ込む**（エラーもログも出ない）。特に walkable → blocked
方向（障害物追加・扉ロック）の漏れが危険。`bump_obstacle_version` は wrapping で単調増加し、
値そのものに意味はない（一致=無変更の検知にのみ使う）。

`WalkabilityConnectivityCache` も同じ世代を正本として、Boolean 到達判定用の連結成分を保持する。
Open/Closed の Door cost 変更で cache を破棄してはならず、Locked・地形・橋・障害物など最終
walkability を変える mutation だけが次回問い合わせ時の再構築を要求する。flood-fill の斜め移動は
A* と同じ corner-cutting helper を使う。cache は保存しない runtime state なので、load 時は新旧
`WorldMap` が偶然同じ `obstacle_version` を持っていても stale component を使わないよう、
`reset_runtime_caches` で必ず default に戻す。

### I-PF2: runtime A* budget の `Deferred` は到達不能ではない

`RuntimePathSearchBudget` は `PreUpdate` でframeごとにresetし、world replacementでもdefaultへ戻す。
budgeted facadeは実際にcore A*を開始する直前にだけ1枠をclaimする。direct探索と隣接goal fallbackは別々のcore A*なので、それぞれ個別にclaimする。範囲外endpointやpolicy上許可されないblocked goalはcore A*を起動しないため枠を消費しない。

`hw_world` 外の runtime waypoint 生成は必ず budgeted facade を通す。raw の
`find_path` / `find_path_to_adjacent` / `find_path_to_boundary` 等は `pub(crate)` とし、
`hw_world` 内の mapgen validation と unit test だけが unbudgeted core を使う。

枠がない場合の `PathSearchResult::Deferred` は `Unreachable` と同一視してはならない。Actor再探索では`Deferred`時に`PathCooldown`、`Destination`、`Path`、`AssignedTask`、reservation、task dispositionを変更せず、同じ探索段階から再試行する。task handler と bucket routing でも phase、assignment、reservation、`Destination`、`Path` を維持し、direct 探索が失敗して adjacent 探索で defer した場合は adjacent から再開する。escapeの経路距離判定では`EscapeRequest`を出さず、`Escaping`、`Destination`、既存`Path`と評価済み候補を次の行動tickまで維持する。一方、すべての試行が実行されて`Unreachable`となったときだけ従来の到達不能cleanupまたは`ReachSafety`を許可する。

escapeはLogic/DecideでActorより先に最大2枠を使う。Execute の task handler / bucket routing は累積4枠まで、続く Actor の `ActiveTask` 再探索は累積6枠まで、idle/rest は累積8枠まで引き上げる。これにより Execute が全枠を使い切らず、Actor 側の task replan に2枠を残す。Actor は `RuntimePathWorkQueue` の `ActiveTask` / `IdleOrRest` class 別 FIFO へ、目的地・task・idle state の変更、cooldown 終了、topology version 変更を投入し、topology 変更時以外に全 Soul を二重走査しない。task handler と escape は最後に core A* を claim した Entity の次から round-robin する。これらの queue、continuation、cursor はすべて `EpochLocal` で保持し、`WorldEpoch` 変更時に旧 world の Entity/request を破棄する。
