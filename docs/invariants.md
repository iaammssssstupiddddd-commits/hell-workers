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
`AssignedTask` が `Some → None` に変わった瞬間に `OnTaskCompleted` が発火する。
`None` の状態で直接 `AssignedTask` を書き換えてはならない（Observer が誤発火する）。

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
Bevy 0.18 の Relationship は Source 側を操作すれば Target 側が自動更新される。
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

---

## 6. ECS アーキテクチャの不変条件

### I-A1: leaf crate への逆依存禁止
`hw_*` crate は `bevy_app` に依存してはならない（依存グラフが循環する）。
`bevy_app` が `hw_*` に依存するのは正当。逆は禁止。

### I-A2: ECS（Commands/Query）を leaf crate に持ち込まない
`hw_familiar_ai`・`hw_soul_ai`・`hw_jobs` 等の leaf crate に Bevy の `Commands`・`Query` を直接持ち込まない。
ECS 接続は `bevy_app/src/systems/` 層が担当する。
