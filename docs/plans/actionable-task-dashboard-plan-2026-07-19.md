# Track A3 アクション可能なタスクダッシュボード 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `actionable-task-dashboard-plan-2026-07-19` |
| ステータス | `In Progress`（コード実装完了、自動受入補強中） |
| 作成日 | `2026-07-19` |
| 最終更新日 | `2026-07-20` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track A3） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  - 現行タスクリストは Designation の件数、説明、優先度、担当数を表示できるが、未割当タスクが
    「なぜ進まないか」を説明できない。
  - Familiar AI は所有範囲、TaskArea、TaskSlots、建築フェーズ、資源・搬送元、予約、容量、到達可能性を
    すでに候補評価しているが、各 `None` / `false` で理由を捨てている。
  - 一覧からタスクへフォーカスはできる一方、絞り込み、安定した並べ替え、安全な優先度変更・キャンセルがない。
  - UI が独自に理由を推測したり、パネル表示中だけ候補探索や A* を再実行すると、AI 判断との不一致と
    規模依存の負荷増加を招く。
- 到達したい状態:
  - 既存の 0.5 秒周期の assignment producer が実際に観測した肯定結果・棄却理由・評価カバレッジと、
    その前提となる wheelbarrow arbitration の判定を、owner ごとの最新 cycle だけの有界な runtime snapshot として公開する。
  - タスク一覧は `Working` / `Blocked(reason)` / `PendingEvaluation` の粗い派生状態を表示し、
    評価されていないタスクを失敗扱いしない。
  - 種別、状態、優先度、担当有無で絞り込み、種別、状態、優先度、担当数で安定して並べ替えられる。
  - 選択行からフォーカス、固定 tier の優先度変更、所有者別の正式なキャンセル経路を利用できる。
  - blocker はライブ派生状態として表示し、A2 の通知履歴へ周期的に蓄積しない。操作結果だけ A2 通知を再利用する。
- 成功指標:
  - 資源・搬送元不足、担当可能な Familiar / Soul 不在、予約・容量の一時競合、依存タスク待ち、
    接続不能を、ログや追加探索なしで区別できる。
  - 1 つでも割り当て要求を実際に構築できれば、その cycle を blocker 確定に使わない。ただし submitted は
    assignment accepted の証拠ではないため、current `TaskWorkers` がなければ次の通常 cycle まで
    `PendingEvaluation` とする。
  - Top-K、優先候補への割り当て成功、対象 worker 未評価などで coverage が不足した場合は、
    理由数があっても `PendingEvaluation` とする。
  - ダッシュボードの表示・非表示、フィルタ、ソート操作による候補評価、source selector scan、
    connectivity 判定、runtime A* 回数の増加が 0 である。
  - 許可された優先度変更とキャンセル後も、`WorkingOn` / `TaskWorkers`、予約、要求、建設 site、
    WorldMap の既存不変条件が保たれる。

## 2. スコープ

### 対象（In Scope）

- producer 間で共有する診断契約と、producer 所有の latest-only snapshot:
  - `hw_jobs` 所有の粗い `TaskDiagnosticClass` / producer coverage / input stamp
  - `hw_familiar_ai` 内部用 `CandidateRejectReason` と `FamiliarTaskCandidateDiagnostics`
  - `hw_soul_ai` の別経路 `blueprint_auto_build_system` 用 `BlueprintAutoBuildDiagnostics`
  - `hw_logistics` の wheelbarrow arbitration が同じ走査内で作る `WheelbarrowArbitrationDiagnostics`
    （assignment producer 票ではなく、general delegation が参照する upstream 判定）
  - 1 task / applicable producer / applicable evaluator につき 1 terminal vote の固定長 counter、肯定証拠、coverage
  - cycle ID、applicable producer mask、入力 revision、world replacement reset
- 現行候補評価経路の typed result 化:
  - candidate collection / filter / score
  - worker distance / connectivity 判定
  - assignment policy / validator / builder
  - 「戻り値が成功なら `TaskAssignmentRequest` を実際に構築済み」という契約
- 表示用 `TaskStatusSummary` / `TaskBlockerReason` と `TaskEntry` 拡張。
- task-local、roster、TaskArea、availability / reservation、topology の revision による理由の失効。
- 種別、状態、優先度 tier、担当有無のフィルタと、種別、状態、優先度、担当数のソート。
- 既存の行クリックによるカメラフォーカスと InfoPanel pin の維持。
- 行とは sibling の action bar を使う優先度変更・キャンセル UI。
- 初版で操作可能な対象:
  - 保存可能な `PlayerIssuedDesignation`（新規 marker）を持つ `Chop` / `Mine`: priority / cancel
  - `Blueprint`: 専用 lifecycle による cancel のみ。priority は read-only
  - `ManualTransportRequest`: priority / cancel
  - Floor / Wall tile または関連 material request: parent site の site-wide cancellation のみ
- A2 の `UserFacingNotification` を使った、操作受理・拒否の bounded な `ToastOnly` 表示 adapter。
  action の typed result 自体は A3 所有とし、診断・操作成立を A2 の有無へ依存させない。
- load/reset、固定 tick、UI、lifecycle、性能回帰テスト。
- 実装完了時の task list、AI、event、architecture、invariant 文書同期。

### 非対象（Out of Scope）

- blocker の履歴、統計、セーブデータ化、通知履歴への周期発行。
- UI 専用の候補再評価、source scan、connectivity flood-fill、A*、全 Entity 走査。
- assigned Soul の実行フェーズ、進捗率、runtime path continuation の詳細表示。
- `PathSearchResult::Deferred` の監視 UI。初版の一時延期は候補評価中に実在する
  予約・容量・同 cycle 競合だけを表し、runtime A* の `Deferred` と同一視しない。
- Move task のキャンセル。`MovePlanned` と配置予約を含む専用 cleanup API がないため read-only とする。
- 自動生成 `TransportRequest` / `AutoGatherDesignation` / `GeneratePower` の優先度変更・恒久キャンセル。
- Blueprint の priority 変更。一般 delegator と `blueprint_auto_build_system` の 2 経路へ同じ順序契約を
  導入しない限り片方にしか効かないため、初版では read-only とする。
- `Priority` と `TransportPriority` の統合、producer 所有 priority model の再設計。
- 新しい永続 priority override component。初版は保存済みの既存 `Priority` だけを変更する。
  ただし安全な action provenance 用 `PlayerIssuedDesignation` marker は追加する。
- 一括選択、bulk priority / cancel、検索欄、キーボード shortcut、gamepad、ローカライズ基盤、最終アート。
- B2 の Familiar policy / WorkType 許可、Soul の直接操作、手動再割り当て。
- Track B〜D、C1 解体、C2/C3 セーブ機能、A2 通知センター自体の再設計。

## 3. 現状とギャップ

### 3.1 タスク一覧 UI

| 現行 | 使える基盤 | ギャップ |
| --- | --- | --- |
| `TaskEntry` は entity / description / priority / worker_count | dirty 時だけ snapshot と UI を更新する | status、reason、work type、capability が entry にない |
| `Vec<(WorkType, Vec<TaskEntry>)>` を WorkType 別表示 | `TaskListDirty` は state / list / summary を分離済み | 状態・優先度の global sort を表せない |
| 行全体が `Button` | click で camera focus と InfoPanel pin | 行内へ Button を入れると nested Button になる |
| 高優先度の色は `priority >= 5` | summary と row は同じ snapshot を使う | summary の high 判定は `> 0` で契約が不一致 |
| 変更時は子 UI を全再構築 | 無変更 frame は rebuild しない | cycle ID を entry equality に入れると 0.5 秒ごとに churn する |

本計画では snapshot を flat `Vec<TaskEntry>` へ寄せる。既定の `TaskSortKey::WorkType` のときだけ
renderer が隣接する同種 entry を group header 付きで表示し、それ以外の sort は flat list とする。
各行は常に WorkType icon を持つため、group header がない並びでも種別を識別できる。

### 3.2 Familiar 候補評価

- `collect_scored_candidates()` は Familiar ごとに空間 index、`ManagedTasks`、Yard-owned / Build の
  補助全件走査から候補を収集する。
- `candidate_snapshot()` は TransportRequest、owner / area、slot、建築材料、floor / wall state を
  `Option` で除外する。
- `score_candidate()` は GatherWater の受入容量などを `Option` で除外する。
- `assignment_loop` は worker 距離 60 tile、Top-K、connectivity cache、slot を判定し、
  高順位候補で割り当てが成立すれば残りを評価しない。
- `assign_task_to_worker()` と `policy/**` / `validator/**` は source、tool、予約、容量、需要を
  `bool` / `Option` で判定する。
- `assign_move()` のように上位が `true` を返しても、下位 builder が必要データ不足で
  request を発行せず return できる経路がある。現行 bool を肯定証拠としては使えない。
- 到達判定は `WalkabilityConnectivityCache` の Boolean であり、候補評価中に runtime A* は走らない。
  `PathSearchResult::Deferred` は割り当て後の Soul 実行側の別契約である。
- `Build` は一般 delegator とは別に `hw_soul_ai::blueprint_auto_build_system` からも
  `TaskAssignmentRequest` を発行する。1 producer だけの rejection で blocker を確定してはならない。

したがって、単に `None` を enum に変えるだけでは不十分である。各 task / cycle について、
「どの段階まで、対象となる Familiar / worker の何件を評価したか」と
「TaskAssignmentRequest を実際に構築できたか」を同時に記録する。

### 3.3 操作 owner

| task source | 現行 priority owner | 現行 cancel owner | 初版方針 |
| --- | --- | --- | --- |
| `PlayerIssuedDesignation` 付き Chop / Mine | `Priority` component | generic owner helper | priority / cancel 対応 |
| Blueprint | 2 assignment 経路で priority semantics 不一致 | 現行 helper + removed cleanup は関連 request / refund が不完全 | priority read-only、専用 cancel lifecycle を追加 |
| `ManualTransportRequest` | manual request entity | `transport_request_anchor_cleanup_system` に close 処理があるが UI 用 typed API は未公開 | lifecycle owner の typed close API を追加して priority / cancel 対応 |
| Floor / Wall tile / material request | construction phase producer | parent site cancellation system | priority read-only、site-wide cancel |
| Move | move finalization / reservation | 汎用 helper では cleanup 不足 | read-only |
| auto gather | auto-gather producer | 需要が残ると再生成される | read-only |
| auto TransportRequest | producer が `Priority` を upsert | despawn しても再生成される | read-only |
| GeneratePower | energy owner | 自動再生成経路 | read-only |

`Priority` は save schema に登録済みだが、自動 producer は upsert で値を再挿入する。
そのため UI は cached capability を表示に使っても、適用時に live component / marker を再検証し、
producer-owned task を変更しない。

Chop / Mine の手動発行元は現状 durable に識別できず、`AutoGatherDesignation` は save schema にない。
「auto marker がない」を許可条件にすると load 後の auto task を手動と誤認するため、deny-by-default とする。
area selection が発行した新規 Chop / Mine だけへ保存対象 `PlayerIssuedDesignation` を付け、legacy save で
marker のない designation と未知 producer は read-only にする。

### 3.4 lifecycle と reset

- task list snapshot と `TaskListDirty` は world replacement 時に root hook で reset される。
- 新しい diagnostics は simulation Entity を key にするため保存しない。各 producer crate は
  `reset_for_world_replace(&mut World)` を公開し、root `FamiliarAiPlugin` / `SoulAiPlugin` が
  `LoadResetRegistry` へ各 1 回登録する。`hw_logistics` も arbitration diagnostics 用 reset を公開し、root
  `LogicPlugin` が登録する。root revision bridge も同じ境界で Entity map を空にする。
- UI filter / sort、inline cancel confirmation も runtime state とし、`hw_ui::reset_for_world_replace()` で戻す。
- `Priority` と action provenance の `PlayerIssuedDesignation` は通常の save/load 往復対象になる。
  diagnostics、filter / sort、confirmation、requested marker は保存しない。

## 4. 実装方針（高レベル）

### 4.1 責務境界

```text
hw_logistics wheelbarrow arbitration（dirty / fallback rebuild 時だけ）
  -> WheelbarrowArbitrationDiagnostics
       └-> general delegation が lease 不在の upstream 理由として read-only 参照

normal producer cycle（dashboard open state 非依存）
  hw_familiar_ai general delegation
    -> FamiliarTaskCandidateDiagnostics
  hw_soul_ai blueprint auto build
    -> BlueprintAutoBuildDiagnostics
  いずれも TaskAssignmentRequest がゲーム処理の正本
                    │ read-only merge
                    v
bevy_app task-list adapter
  existing Designation scan + applicable producer coverage + TaskWorkers + capability query
       -> TaskStatusSummary / TaskEntry
                    │
                    v
hw_ui dashboard
  filter / sort / render / focus / action UiIntent
                    │
                    v
bevy_app action adapter
  live capability 再検証 -> existing owner cleanup / Priority mutation
                    └-> A2 UserFacingNotification（操作結果だけ）
```

- `hw_familiar_ai` は UI 型、文字列、notification、filter state を知らない。
- `hw_ui` は `hw_familiar_ai` に依存せず、表示用の粗い状態と action intent だけを所有する。
- root `bevy_app` は producer / owner crate をつなぎ、game-specific component から description / capability を導出する。
- `WheelbarrowArbitrationDiagnostics` は applicable producer を増やさない。general delegation が同じ
  request の lease 判定を行うときだけ、revision が一致する upstream evidence として利用する。
- submitted は producer が request を writer へ渡せた証拠に限定し、assignment accepted / worker relation の
  代用にしない。root は current `TaskWorkers` がなければ submitted task を `PendingEvaluation` とし、
  downstream rejection / same-frame abandon を UI 側で推測しない。
- UI から AI への診断要求、dashboard-open flag、再評価 Message は作らない。
- M1〜M3 は A2 非依存とする。M4 も owner action の typed result を先に確定し、既存 A2 facade への
  変換は root presentation adapter に閉じ込める。A2 を外しても action の受理・拒否契約は変えない。

### 4.2 型と所有権

| 型 / 責務 | owner | 契約 |
| --- | --- | --- |
| `CandidateRejectReason` | `hw_familiar_ai::task_management::diagnostics` | 一般 delegator 内部の詳細理由。`Copy` enum、文字列なし |
| `TaskDiagnosticClass` / producer record | `hw_jobs::diagnostics` | peer producer と root adapter が共有する 5 分類、coverage、input stamp |
| `TaskDiagnosticCounters` | 各 producer | `[u16; COUNT]`、producer × applicable evaluator ごとに 1 terminal vote、saturating increment |
| `FamiliarTaskCandidateDiagnostics` | `hw_familiar_ai` | 一般 delegator が既存 candidate universe で観測した task だけ。latest cycle |
| `BlueprintAutoBuildDiagnostics` | `hw_soul_ai` | `Build` の別 producer snapshot。latest cycle |
| `WheelbarrowArbitrationOutcome` / `WheelbarrowArbitrationDiagnostics` | `hw_logistics::transport_request::arbitration` | rebuild header（generation / any vehicle exists / available / leased）と、既存走査が触れた request ごとの lease / terminal reason を latest-only publish。assignment producer 票ではない |
| `TaskDiagnosticInputRevisions` | shared value は `hw_jobs`、更新 bridge は root | task Entity 別 + roster / availability / topology 世代 |
| `TaskBlockerReason` | `hw_ui::panels::task_list` | UI 安全な粗い理由。文字列化は renderer / presenter だけ |
| `TaskStatusSummary` | `hw_ui::panels::task_list` | `Working` / `Blocked` / `PendingEvaluation`。submitted 単独の `Ready` は持たない |
| `TaskPriorityTier` | `hw_ui::panels::task_list` | `Normal(0..=4)` / `High(5..=9)` / `Critical(10+)` の唯一の分類 |
| `TaskActionCapabilities` | `hw_ui` 表示型、root 導出 | focus / priority / cancel kind。適用権限の正本にはしない |
| `TaskDashboardViewState` | `hw_ui` | filter / sort / direction。保存しない |
| task action `UiIntent` | `hw_ui::intents` | entity と typed operation だけ。component mutation はしない |
| action capability resolver / apply | root `bevy_app` | live state を再検証し、owner API へ接続する |

diagnostics の cycle ID、入力 revision、観測数は `TaskEntry::PartialEq` へ含めない。
adapter が最終的に導出した status / reason / capability /表示値だけを比較し、表示内容が同じ cycle では
UI を再構築しない。

### 4.3 診断 cycle、coverage、肯定証拠

各 producer は既存の 0.5 秒 gate が true の cycle だけ自身の diagnostics を開始・確定する。

1. cycle 開始時に新しい accumulator と現在の input revisions を用意する。
2. Familiar ごとの既存 candidate universe 走査を完了したかを evaluator header に記録し、
   重複排除後かつ `candidate_snapshot()` / filter より前の実 candidate entity 集合への membership を
   task record へ追加する。この集合には TaskArea / Yard の spatial lookup、`ManagedTasks` だけでなく、
   `include_in_global_designation_scan()` が加える global `Build` と Yard-owned designation も含める。
   由来を scope 名から再推測せず、この実集合に含まれる場合だけ `Applicable`、同じ input revision で完走した
   universe に含まれない Familiar は `NotApplicable` とする。universe 走査未完了、header 不在、revision 不一致は
   applicability 自体が unknown である。
   ただし全 evaluator の universe から未観測の root task に `NotApplicable` record を新規作成せず、record absent として
   Pending に保つ。少なくとも 1 evaluator の membership がある task に限り、他 evaluator の完走済み非所属を分母外とする。
   AI 側へ active Designation の追加全件 seed scan は入れない。
3. score、worker 距離、Top-K、connectivity、policy / validator の各段階を、`Applicable` な
   `(task, producer, familiar)` ごとの producer-local reducer へ集める。source 候補や worker ごとの branch hit を
   代表理由の生票にせず、`NotApplicable` evaluator は分母にも reason vote にも含めない。
4. producer-local reducer は、(a) request submitted が 1 件でもあれば肯定、(b) 必要な worker / source の
   未評価が 1 件でもあれば coverage partial、(c) eligible worker 0 なら `NoEligibleFamiliar`、(d) それ以外の
   terminal rejection は理由の出現有無と固定順位だけで 1 票へ縮約する。同じ理由の worker / source 件数は票数にしない。
5. `TaskAssignmentRequest` を実際に writer へ渡せた builder だけが producer accumulator の `submitted_count` を増やす。
   これは「その評価時点で request を構築・送信できた」証拠であり、consumer accepted の証拠にはしない。
6. 上位 task の成功で下位 task を試さなかった場合、下位 task の coverage は partial のままにする。
7. applicable producer と、task / producer ごとの applicable evaluator 集合の必要段階を完了した task だけ
   代表 blocker を確定する。
8. cycle 終了時に accumulator を一括で publish し、前 cycle の map を置き換える。履歴は保持しない。

`TaskDiagnosticCoverage` は最低限次を区別する。

- producer cycle header の eligible Familiar / worker 数と、各 evaluator の universe 完走状態。
  zero-roster でも header は publish する。
- task / producer ごとの evaluator applicability (`Applicable` / `NotApplicable` / unknown)。
  未観測を rejection にせず、完走済み universe の非所属だけを `NotApplicable` とする。
- static filter / score を完了した applicable evaluator 数。
- worker-stage の対象数と評価済み数。
- Top-K / 先行成功 / idle worker 不足により未評価が残ったか。
- assignment request の `submitted_count` があるか。

published snapshot は task ごとの applicable / evaluated / terminal vote / submitted の固定幅 count だけを保持し、
`HashSet<(task, evaluator)>` を cycle 間で残さない。candidate universe は Familiar 内で既に重複排除されるため、
membership 時に applicable count を 1 回だけ増やせる。`NotApplicable` は完走済み evaluator 数との差として扱い、
latest snapshot のメモリを Familiar 数との積へ広げない。

判定規則は次で固定する。

1. `TaskWorkers.len() > 0` なら UI adapter は diagnostics より優先して `Working` とする。
2. applicable producer のどれかに current revision の `submitted_count > 0` があっても、current `TaskWorkers` が
   なければ `PendingEvaluation` とし、blocker を表示しない。submitted は accepted を意味せず、downstream rejection、
   same-frame assignment completion / abandon、relationship 適用前を UI adapter が区別しない。次の通常 producer cycle で
   再評価し、安定した肯定状態は `TaskWorkers` による `Working` だけとする。
3. 肯定証拠がなく、全 applicable producer と各 task の applicable evaluator 集合の coverage が complete のときだけ
   `Blocked(representative)` とする。producer snapshot がない場合も complete ではない。
4. coverage partial、input revision 不一致、初回 cycle 前、新規 task、spatial index 未反映、
   applicability unknown、既存 candidate universe で全 evaluator から未観測の task は
   `PendingEvaluation` とする。
5. task 行の universe は root task-list adapter がすでに行う Designation scan とする。AI map に record がなくても、
   全 applicable producer の最新 cycle header で eligible roster 0 を確認できた場合だけ、
   全該当行を `NoEligibleFamiliar` とする。header 不在または revision 不一致なら Pending のままにする。
6. task 消滅または `Designation` 削除時は行自体を除き、古い reason を別 map へ残さない。

### 4.4 理由分類と代表理由

| `TaskDiagnosticClass` | 内部原因の例 | 表示の意味 |
| --- | --- | --- |
| `NoEligibleFamiliar` | applicable evaluator 内の対象 worker 0、全 worker が条件外 | 担当できる使い魔・作業員がいない |
| `MissingResourceOrSource` | 資材、搬送元、bucket / wheelbarrow、受入先の実容量がない | 必要な資源・搬送元・道具がない |
| `Unreachable` | 既存 connectivity cache が対象 worker から false | 現在の地形接続では到達できない |
| `TemporaryContention` | source / destination が予約済み、同 cycle reservation shadow、容量が搬入予約で埋まる | 一時競合の解消待ち |
| `DependencyWaiting` | Blueprint 材料未完了、floor / wall phase 未準備 | 上流要求・建設フェーズ待ち |

- worker 距離 60 tile 超過だけを `Unreachable` と呼ばない。別 applicable evaluator / worker が未評価なら
  coverage partial、全 applicable evaluator の eligible worker が距離条件外なら `NoEligibleFamiliar` とする。
- runtime A* budget の `PathSearchResult::Deferred` はこの分類へ入力しない。
- stale entity、cycle 中の component 消滅、内部 query 不一致は player blocker にせず、
  task が残る場合は `PendingEvaluation`、消えた場合は snapshot から除く。診断詳細は debug log / test へ残す。
- Haul 系の必須 `TransportRequest` 欠落は待てば直る依存状態ではなく invariant 違反である。
  `MalformedTask` 相当の内部診断 / test failure とし、プレイヤー表示は `PendingEvaluation` にする。
- `TaskSlots` が満員かつ worker が存在する場合は `Working` であり blocker ではない。
- wheelbarrow lease 不在だけから理由を推測しない。general delegation は同じ request に対応する
  `WheelbarrowArbitrationDiagnostics` が current availability / arbitration revision のときだけ、
  no vehicle / no source / capacity / batch wait / arbitration contention を terminal reason へ写像する。
  snapshot 不在、revision 不一致、仲裁非対象、仲裁 rebuild 未完了は `PendingEvaluation` または他の applicable path の
  coverage とし、Familiar 側で同じ source / wheelbarrow 全件走査を再実行しない。
  owner 内部の typed outcome は最低限、`LeaseGranted`、`NotApplicable`、`NoAvailableWheelbarrow`、
  `NoSourceItems`、`SourceReserved`、`NoDestinationCapacity`、`CapacityReserved`、`DemandGone`、`PreferredBatchWaiting`、
  `ArbitrationContention`、`StaleInput` を区別する。world に Wheelbarrow 自体が 0、no source、実容量 0 は
  `MissingResourceOrSource`、Wheelbarrow は存在するが available 0、source / capacity 予約、dedup、他 request との競争、
  期限付き batch wait は `TemporaryContention`、
  `DemandGone` / `NotApplicable` / `StaleInput` は reason 票にせず task 消滅または coverage / revision 失効へ反映する。
  available wheelbarrow 0 の fast path は request 全件を診断のためだけに走査せず、current rebuild header の
  `any_vehicle_exists`、`available_vehicle_count`、既存 lease state 由来 `leased_vehicle_count` を
  wheelbarrow-required task の共通 upstream evidence とする。`any_vehicle_exists` は `Without<PushedBy>` の
  arbitration available query から導出せず、既存の全 Wheelbarrow query の `is_empty()` または owner-maintained count を使う。
  per-request record は arbitration が本来評価した request だけを持ち、未走査 request を
  rejection record で seed しない。
- 仲裁適用判定は Designation の `WorkType` で近似せず `TransportRequest.kind + resource_type` を正本とする。
  `DeliverToBlueprint` の `Haul` と `DeliverToMixerSolid` の `HaulToMixer` も typed outcome を参照する。
- 未予約候補が `hard_min` 未満でも、同じ実検索範囲の予約済み候補を含めれば必要数を満たす場合は
  `NoSourceItems` でなく `SourceReserved` とする。

Familiar-local reducer は worker / source ごとの理由件数を比較しない。まず `submitted` を優先し、次に未評価の有無を
確認する。complete rejection の場合だけ、出現した reason class の集合から下記と同じ固定順位で 1 class を選ぶ。
これにより 1 Familiar に所属する Soul 数、worker の sort / spawn 順、source 候補数を変えてもその Familiar の票は変わらない。

全 rejection の件数が 0 の場合は blocker を捏造せず `PendingEvaluation` とする。
worker / source 候補の詳細理由はまず producer-local / Familiar-local terminal outcome へ縮約し、
各 applicable producer の各 Familiar が 1 票だけを投じる。producer 間を merge した代表理由はこの
正規化済み票の「件数が多い順」、同数なら次の固定順位で選ぶ。

1. `MissingResourceOrSource`
2. `NoEligibleFamiliar`
3. `Unreachable`
4. `DependencyWaiting`
5. `TemporaryContention`

HashSet の走査順、Familiar / Soul の spawn 順、Top-K 内の偶然の順番を代表理由へ使わない。

### 4.5 input revision と stale reason の失効

単一の global dirty bit では、頻繁に動く資源 1 件で全 task が永久に `PendingEvaluation` になり得る。
そのため revision を reason dependency ごとの 4 domain に分ける。

| revision domain | 主な入力 | 依存する reason |
| --- | --- | --- |
| `task` | Entity ごとの Designation、TaskSlots、TaskWorkers、Blueprint / tile / TransportRequest / demand | 全状態、特に DependencyWaiting |
| `roster` | Familiar、Commanding、TaskArea、ManagedTasks、FamiliarOperation、Soul eligibility | NoEligibleFamiliar |
| `availability` | ResourceSpatialGrid、SharedResourceCache、reservation / IncomingDeliveries、Inventory / StoredItems / LoadedItems、tool / vehicle state | MissingResourceOrSource / TemporaryContention |
| `topology` | `WorldMap.obstacle_version` | Unreachable |

各 diagnostic record は、実際に判断へ使った domain の mask と revision snapshot だけを持つ。
現在 revision と不一致なら、その reason は次の既存 delegation cycle まで `PendingEvaluation` とする。
revision 変更を理由に timer を早回しせず、ゲームの割り当て頻度と候補評価回数を変えない。

- `DependencyWaiting` は Blueprint / Floor / Wall phase の task-local 状態だけに依存し、global availability 変更では
  失効させない。producer cycle header の roster stamp は reason mask と別に evaluator coverage 全体を検証する。
- Soul eligibility は `AssignedTask`、`CommandedBy`、休息 / breakdown、Familiar fatigue threshold、GeneratePower の
  Dream threshold を境界値として追跡する。idle timer や同じ可否側に留まる疲労変化では roster を進めない。
- `task` は単一 global 世代ではなく Entity ごとの bounded revision map とし、task removal 時に掃除する。
- dependency owner は上位 crate を参照しない。`hw_spatial` / `hw_logistics` は自身の semantic generation を
  公開し、root bridge がそれらと inventory / storage / delivery / tool / vehicle relationship の実差分を
  `TaskDiagnosticInputRevisions` へ写像する。
  producer は cycle 開始時に read-only snapshot を取る。
- wheelbarrow arbitration は `should_rebuild == true` の既存走査で outcome map と arbitration generation を同時に
  置き換え、rebuild しない frame は直前 snapshot を保持する。request removal / world replacement では該当 Entity を
  除去し、availability revision と snapshot stamp が一致しない record を general delegation が利用しない。
- revision は `Res::is_changed()` の値をそのまま世代にしない。owner の mutation point で
  「検索結果・予約数・容量判定が実際に変わった」ときだけ明示的な semantic generation を進める。
  特に `SharedResourceCache::begin_frame()` は毎 frame `ResMut` を通るため、それだけで availability を
  進めると全該当行が永久に Pending になる。既存 reservation signature の差分、resource grid の
  add / move / visibility / remove、relationship の実差分を使い、generation 算出用の全件 hash scan は追加しない。
- resource / reservation は全 task の文字列を再生成せず、availability-dependent record だけを失効させる。
- 初版の availability は domain-global generation とし、その domain mask を持つ task は安全側に一括失効して
  最大 0.5 秒 Pending になり得ることを許容する。resource type / source / destination 別 dirty key は
  実測で常時 Pending が発生した場合の M3 最適化とし、正しさより先に複雑化しない。
- topology は `WorldMap::is_changed()` ではなく最終 walkability の正本 `obstacle_version` を比較する。
- pause 中は `Time<Real>` で AI 診断を別実行しない。入力が変われば古い理由を隠して判定待ちとする。
- cycle publish 後、最終 status / reason が変わらなければ `TaskListDirty::mark_list()` を呼ばない。

### 4.6 dashboard の filter / sort / layout

`TaskDashboardViewState` は次を持つ。

- `work_type`: All または 1 `WorkType`
- `status`: All / Working / Blocked / Pending
- `priority`: All / Normal / High / Critical
- `workers`: All / Assigned / Unassigned
- `sort_key`: WorkType / Status / Priority / WorkerCount
- `direction`: Ascending / Descending

filter を適用してから sort し、最後の tie-break は Entity の index、generation の順とする。
default は `WorkType Ascending`。WorkType sort のときだけ group header を描画し、他の sort では flat list にする。
priority tier helper は row color、summary high count、filter、sort、priority action で共用し、
現行の `> 0` と `>= 5` の不一致を解消する。

狭い左パネル内で nested Button を作らないため、構造を次にする。

```text
TaskListBody
  Toolbar row(s): filter / sort controls
  Task row wrapper
    Focus button: icon / description / priority / workers / status
    Action bar sibling（pin された 1 行だけ）
      Priority - / Priority + / Cancel or Cancel site
```

- focus button の既存 camera / pin 動作を維持する。
- action button 押下で focus button の `Interaction::Pressed` を発火させない。
- destructive cancel は inline 2-step confirmation とし、別行選択、filter 変更、panel close で解除する。
- toolbar / row / action の発行側は既存 `ForegroundUiGate` を正本とし、Modal / Pause capture 中は発行しない。
  apply 側の MessageReader は `run_if` で止めず常に drain し、paused / captured ならその場で typed rejection にする。
- `world_input_capture_started` で inline confirmation を clear し、capture 解除後の 1 click を
  confirmation 2 回目として扱わない。
- blocker body は短い固定文言にし、内部 Entity ID、debug reason、raw error を表示しない。
- cycle ID や last-checked time は表示せず、同じ内容の 0.5 秒 rebuild を避ける。

### 4.7 優先度変更とキャンセル

優先度は arbitrary `u32` 入力にせず、既存の意味と表示を揃えた 3 tier を循環・上下移動する。

- Normal: `0`
- High: `5`
- Critical: `10`

現在値が中間値の場合は現在 tier の境界を基準に次 tier へ移す。UI は `Priority` component だけを変更し、
`TransportPriority` を暗黙に同期しない。producer-owned task はボタンを表示しない。
これは A3 の UI tier であり、別 enum の `TransportPriority::Critical = 30` と同じ数値契約ではない。

task action は `UiIntent` の specialized variant とする。既存 `handle_ui_intent` は exhaustive な専用分岐で
task variant を mutation せず通過させ、別 `apply_task_action_intents_system` の独立 MessageReader が
同じ Interface frame に読む。この system を `handle_ui_intent` の後へ置き、同 frame の Pause intent が
先に `Time<Virtual>` へ反映された状態で再検証する。
適用時は次を再検証する。

- Entity と `Designation` が現存する。
- cached entry と現在の WorkType / marker / owner kind が一致する。
- `TaskActionCapabilities` を live component から再導出できる。
- virtual time が進行中で、foreground capture がない。pause 中は action を発行しない。
- priority は保存済み `PlayerIssuedDesignation` 付き Chop / Mine または live `ManualTransportRequest` で、
  producer が次 frame に上書きする task ではない。Blueprint は priority read-only。
- cancel は `GenericDesignation` / `Blueprint` / `ManualTransportRequest` /
  `FloorSite(parent)` / `WallSite(parent)` のいずれかに解決できる。

cancel apply は UI renderer から component を個別に剥がさず、source-kind ごとの typed owner adapter を通す。

- manual Chop / Mine は `PlayerIssuedDesignation`、正確な WorkType、Tree / Rock、producer marker 不在を
  positive allow-list として再検証する。generic owner helper は `SoulTaskUnassignRequest` を発行し、
  `(Designation, TaskSlots, ManagedBy, Priority, PlayerIssuedDesignation)` を一括で外す。
- `ManualTransportRequest` は marker と fixed source を再検証し、pinned source cleanup、worker unassign、
  request despawn を `hw_logistics::transport_request::lifecycle` が公開する 1 typed cancel API / result で行う。
  root は request component を個別 remove せず、この owner API の成功 / stale / unsupported を action outcome へ写像する。
  anchor cleanup も同じ close primitive を再利用し、UI cancel と自然消滅で pinned source、lease、demand、designation の
  除去集合が分岐しない。bool 引数だけの generic helper を公開 capability にしない。
- Blueprint は現行 `cancel_single_designation()` で即 despawn せず、`BlueprintCancelRequested`（新規）を付ける。
  owner cancellation system は全 Soul の `AssignedTask` payload を走査し、
  `Build(data.blueprint)` / `HaulToBlueprint(data.blueprint)` /
  `HaulWithWheelbarrow(data.destination == WheelbarrowDestination::Blueprint(blueprint))` を直接列挙して unassign する。
  `TaskWorkers` と関連 `TargetBlueprint` request worker は索引・整合性検査には使えるが、cleanup correctness の
  唯一の根拠にしない。request / reservation、pending companion、building / stockpile の WorldMap 登録を閉じ、
  記録済み搬入資材は flexible ledger を含め blueprint 位置へ 1 回だけ refund してから despawn する。
- Floor / Wall は `FloorTileBlueprint::parent_site` / `WallTileBlueprint::parent_site`、または material request の
  保存対象 `TransportRequest.kind + anchor`（`DeliverToFloorConstruction` / `DeliverToWallConstruction` + site Entity）を
  正本として解決する。`TargetFloorConstructionSite` / `TargetWallConstructionSite` は load 後に存在することを
  前提にせず、fast path と整合性検査だけに使う。tile / request entity へ generic cancel をせず、live な parent site を
  再検証して `FloorConstructionCancelRequested` / `WallConstructionCancelRequested` を付与する。
  UI label も `Cancel site` とし、1 行だけの削除に見せない。
- Floor / Wall owner cleanup も marker-only query にせず、同じ `TransportRequest.kind + anchor` で関連 request を列挙する。
  save/load fixture では target marker がなくても request row から site-wide cancel と cleanup が成立しなければならない。
- Floor / Wall cancellation は `TileSiteIndex` miss / 0 tile で永久に marker を残さない。明示 action 時だけ
  authoritative tile query へ fallback するか retry 状態を保持し、cleanup 完了 / 再試行を test で固定する。
- Move / auto producer / unsupported target は no-op と typed failure を返す。

apply は root-owned `TaskActionOutcome` Message を常に発行する。操作成功は `Priority changed` /
`Cancellation requested` の `ToastOnly`、stale / unsupported / paused / captured は必要な場合だけ Warning とする。
A2 adapter は `NotificationSystemSet::Adapt` で outcome を変換し、dedupe key は
action kind + target Entity identity + outcome class とする。別 task や成功 / 拒否を合流させず、blocker 状態自体は通知にしない。

### 4.8 system order と world replacement

```text
Update::FamiliarAiSystemSet::Perceive
  reservation sync -> external semantic generation bridge

Update::TransportRequestSet::Arbitrate（Familiar Update 後 / Decide 前）
  wheelbarrow arbitration rebuild
    -> lease Commands + latest WheelbarrowArbitrationDiagnostics publish
    -> ApplyDeferred / explicit edge before FamiliarTaskDecisionSet::Delegation

Update::FamiliarAiSystemSet::Decide（既存 0.5 秒 gate）
  FamiliarTaskDecisionSet（export された named chain）
    StateDecision -> StateFlush
      -> BlueprintAutoGather -> AutoGatherFlush
      -> TaskRevisionSync（root bridge の final sync）
      -> Delegation
          begin diagnostics cycle
            -> candidate/filter/score
            -> worker/connectivity
            -> policy/validator/builder
          publish latest diagnostics
      -> Encouragement

Update::SoulAiSystemSet::Decide（既存 blueprint 0.5 秒 gate）
  cycle 開始時の revision snapshot
  blueprint auto-build + independent producer diagnostics

次 frame PreUpdate
  task-list Changed / Removed + diagnostics content change
    -> build TaskEntry / filter / sort

Update::GameSystemSet::Interface
  task-list render / local controls
    -> `ui_interaction_system` + `ForegroundUiGate` -> UiIntent
    -> `handle_ui_intent`（Pause 等を先に反映）
    -> `apply_task_action_intents_system`（独立 reader を常に drain、live gate 再検証）
    -> TaskActionOutcome
    -> NotificationSystemSet::Adapt -> Reduce -> Present
```

`FamiliarTaskDecisionSet` は `hw_familiar_ai` が定義・export し、leaf plugin が上記 set を `.chain()` する。
既存の anonymous `ApplyDeferred` と root system の `.after(auto_gather).before(delegation)` だけに依存しない。
root の task-local revision final sync は `TaskRevisionSync` set に登録し、`AutoGatherFlush` が Commands を反映した後、
`Delegation` が input stamp を取得する前に必ず 1 回実行する。production test は auto-gather が同 frame に変更した
task component を final sync と delegation の双方が観測することを固定する。

`apply_task_action_intents_system` は `.after(handle_ui_intent).before(NotificationSystemSet::Adapt)` を明示し、
同 frame の outcome を Adapt が読み、Reduce / Present まで到達させる。Interface 内の tuple 登録順には依存しない。

cancel intent は Interface で受理されるため、その frame の Logic はすでに完了している。
manual / Blueprint cancel の marker と `SoulTaskUnassignRequest`、Floor / Wall の requested marker は次 frameへ到達する。
generic Message は既存 Perceive-before-Execute 契約を使う。一方、Blueprint / Floor / Wall owner cancellation は
直接 worker cleanup も行うため、新しい `TaskOwnerCancellationSet` を `GameSystemSet::Logic` 内で
`FamiliarAiSystemSet::Perceive` より前へ明示し、その直後かつ Familiar Perceive より前に `ApplyDeferred` を置く。
これにより Familiar pipeline 全体に加え `TransportRequestSet::Perceive` と `SoulAiSystemSet::Perceive` よりも先に、
despawn / marker / relationship 変更を可視化する。production schedule、
request 再生成、load 直後の index rebuild、0 tile を模した統合テストで completion / cancellation の二重終端を防ぐ。

world replacement では次を reset する。

- `FamiliarTaskCandidateDiagnostics` / `BlueprintAutoBuildDiagnostics` / `WheelbarrowArbitrationDiagnostics`
- diagnostic cycle / input revisions / test-only counters
- `TaskListState` / `TaskListDirty`
- `TaskDashboardViewState`
- inline cancel confirmation と task row / action bar Entity
- `UiIntent` / `TaskActionOutcome` / notification の未読 Message

load 後は task list が `PendingEvaluation` から開始し、Spatial / Logic が再開した最初の通常 cycle で理由を再構築する。
保存済みの旧 world Entity を diagnostics や confirmation に残さない。
`hw_ui::reset_for_world_replace()` は `TaskListBody` の static root を残し、その直下の全 dynamic child を
収集・despawnする。既存 `TaskListItem` だけの収集に依存せず、header / empty row / wrapper / action bar を残さない。

### 4.9 Bevy 0.19 API での注意点

- `UiIntent` / `TaskActionOutcome` / `UserFacingNotification` / `SoulTaskUnassignRequest` は Bevy 0.19 の `Message` とし、
  owner plugin の `add_message::<T>()` を維持する。
- root-owned `TaskActionOutcome` は `plugins/messages.rs::root_message_types!` へ追加し、登録と world replacement
  clear の inventory を一致させる。`UiIntent` は既存 `hw_ui` reset を使う。
- Bevy Relationship の target (`TaskWorkers`) は手動変更せず、`WorkingOn` source cleanup を既存 owner に任せる。
- `Changed<T>` / `RemovedComponents<T>` reader は全件 drain し、診断 input revision と task-list dirty の
  reader を共有したつもりで片方を省略しない。
- `ResMut` は revision / content が実際に変わるときだけ dereference-mutate し、無変更 frame の
  change detection を汚さない。
- UI row 内へ `Button` descendant を置かず、focus row と action bar を sibling にする。
- `WorldMap.obstacle_version` を connectivity validity の正本とし、UI 用 A* を追加しない。
- `Time<Virtual>` pause 中に診断 timerを `Time<Real>` で進めない。
- 新しい system / set の順序は `.chain()` / `.before()` / `.after()` で明示し、tuple の暗黙順序へ依存しない。
- API の実装時は Bevy 0.19 のローカル source または docs.rs で Message / Change Detection / UI interaction を確認する。

## 5. マイルストーン

## M0: 診断契約・分岐台帳・性能基準の確定

- 変更内容:
  - `task_finder`、`assignment_loop`、`task_assigner`、`policy/**`、`validator/**`、`builders/**` の
    全 `None` / `false` / early return を列挙する。
  - wheelbarrow arbitration の eligibility / candidate / grant early return を同じ台帳へ追加する。
  - WorkType ごとの assignment producer を列挙し、特に `Build` の general delegation /
    `blueprint_auto_build_system` を applicable producer mask として固定する。
  - 各分岐を detailed reason、non-diagnostic stale、success-submitted のいずれかへ分類する。
  - task / producer ごとの evaluator applicability、Familiar-local reducer、applicable evaluator 1 terminal vote、
    zero-roster header、missing producer = Pending の pure contract を固定する。
  - submitted は blocker を確定させない一方、`TaskWorkers` なしでは Working にせず Pending とする pure contract を固定する。
  - WorkType / source provenance ごとの priority / cancel capability matrix を deny-by-default の
    テストデータとして固定する。
  - coverage、representative reason、input revision、priority tier の pure contract test を先に作る。
  - 同一 fixture の candidate / source scan / connectivity / runtime A* counter baseline を記録する。
- 変更ファイル:
  - `docs/plans/actionable-task-dashboard-plan-2026-07-19.md`
  - `crates/hw_jobs/src/diagnostics.rs`（共有型の新規候補）
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/`
  - `crates/hw_soul_ai/src/soul_ai/decide/work/auto_build.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/`
  - `crates/bevy_app/src/interface/ui/panels/task_list/`
  - profiling counter の既存定義・test support（必要箇所のみ）
- 完了条件:
  - [ ] 現行の全 early return がレビュー済み台帳の分類を持つ。M3 完了後は typed result の
    exhaustive match と WorkType fixture により、新しい未分類分岐を compile / test で検知できる。
  - [ ] coverage partial を blocker にしない pure test がある。
  - [ ] 離れた 2 つの TaskArea の非 global task では他方の Familiar を `NotApplicable` として分母から外し、
    同じ配置の global Build / Yard-owned task は `Applicable` のまま、universe 走査未完了は unknown / Pending とする
    pure test がある。
  - [ ] 同じ Familiar の Soul 数 / source 候補数、worker の spawn / sort 順を変えても、local reducer の
    terminal outcome と producer merge 後の代表理由が変わらない。
  - [ ] applicable producer が 1 つでも未評価なら blocker を確定しない。
  - [ ] 現行 bool の「true だが request 未発行」経路を再現する test がある。
  - [ ] submitted + TaskWorkers 0、submitted + same-frame relationship removal、TaskWorkers > 0 をそれぞれ
    Pending / Pending / Working とする contract test がある。
  - [ ] wheelbarrow arbitration の no vehicle / 全台 PushedBy / no source / capacity / reservation / batch wait /
    grant competition が header / typed outcome へ分類され、lease 不在や available query だけから理由を推測する
    分岐が台帳上 0 件である。
  - [ ] capability matrix が provenance 不明、Blueprint priority、Move、auto request / gather を read-only とする。
  - [ ] hidden dashboard baseline の schema / fixture / counter が記録されている。
- 検証:
  - `cargo test -p hw_familiar_ai task_management`
  - `cargo test -p bevy_app task_list`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- ロールバック境界:
  - production behavior を変えない contract / test milestone として単独で戻せる。

## M1: latest-only 診断コアと shallow reason

- 変更内容:
  - shared class / producer mask / input stamp を `hw_jobs`、fixed vote counter / coverage / latest snapshot を
    `hw_familiar_ai` に追加する。
  - candidate universe、filter、score を typed result 化する。
  - evaluator header と、global Build / Yard-owned 補助 scan を含む filter 前の実 universe membership から
    task / producer ごとの applicable evaluator 集合を作り、`NotApplicable` と本来の評価未完了を分離する。
  - cycle header を task record が 0 件でも publish し、roster 0 と未観測 task を区別する。
  - worker / source branch を件数で数えず、submitted -> partial -> zero worker -> fixed reason precedence の順で
    Familiar-local 1 terminal outcome へ縮約する。
  - roster 0、owner / TaskArea、slot、Blueprint 材料、floor / wall state、
    GatherWater capacity の shallow reason を記録する。
  - 必須 TransportRequest 欠落は blocker でなく invariant violation + Pending とする。
  - Top-K / short-circuit / worker 未評価を coverage partial として保持する。
  - task Entity 別 revision、owner crate の semantic generation、root bridge と
    `reset_for_world_replace()` を接続する。
  - reservation sync 後に external generation、blueprint auto-gather の ApplyDeferred 後かつ
    delegation cycle 開始直前に task-local revision を最終同期する。`hw_familiar_ai` が export する
    `FamiliarTaskDecisionSet` の `AutoGatherFlush -> TaskRevisionSync -> Delegation` chain を正本とする。
  - general delegation の共通 submit helper でのみ producer accumulator の `submitted_count` を増やし、
    view model は `TaskWorkers` がない submitted task を Pending とする。
  - `Build` は別 producer が未統合の M1 では、unassigned の blocker を確定せず Pending とする。
  - diagnostics がない構成でも Familiar assignment の成否を変えないことをテストする。
- 変更ファイル:
  - `crates/hw_jobs/src/diagnostics.rs`（新規候補）
  - `crates/hw_jobs/src/lib.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/diagnostics.rs`（新規候補）
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/mod.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/filter.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/score.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/builders/submit.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/resources.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/mod.rs`
  - `crates/hw_familiar_ai/src/lib.rs`
  - `crates/hw_spatial/src/grid.rs` / `resource.rs`（semantic generation）
  - `crates/hw_logistics/src/resource_cache.rs`（semantic generation）
  - `crates/bevy_app/src/systems/familiar_ai/perceive/resource_sync.rs`
  - `crates/bevy_app/src/systems/familiar_ai/mod.rs`
  - `crates/bevy_app/src/interface/ui/panels/task_list/view_model.rs`
- 完了条件:
  - [ ] counters は producer × applicable evaluator 1 vote の固定長・saturating で、未観測 task を seed せず stale record を保持しない。
  - [ ] 1 Familiar 内の複数 worker / source outcome は出現有無 + 固定順位で 1 票になり、件数・spawn 順で変わらない。
  - [ ] submitted が 1 件あればその producer cycle を Blocked にせず、`TaskWorkers` がなければ Pending、
    request 未提出かつ全 producer terminal rejection + complete coverage だけが Blocked になる。
  - [ ] 離れた 2 つの TaskArea の非 global task が互いの Familiar を待たずに確定し、global Build / Yard-owned task は
    両方の evaluator を分母に含め、applicable evaluator の未評価だけが coverage partial / Pending になる。
  - [ ] root の既存 Designation universe と cycle header から zero roster を表示でき、record absent、new task、
    spatial index 未反映、dirty input、short-circuit task は Pending になる。
  - [ ] `SharedResourceCache::begin_frame()` だけで availability generation が進まない。
  - [ ] auto-gather Commands を `AutoGatherFlush` が反映した後に final revision sync、delegation の順で観測し、
    root system と anonymous `ApplyDeferred` の偶然の実行順へ依存しない。
  - [ ] submitted 後に assignment が同 frame で解除された fixture でも、`TaskWorkers` 0 なら Pending となる。
  - [ ] task deletion / Designation removal / producer cycle replacement / world replacement で diagnostics に
    stale Entity が残らない。
  - [ ] runtime A* を呼ばず、既存 connectivity call 数を変えない。
- 検証:
  - `cargo test -p hw_familiar_ai diagnostics`
  - `cargo test -p hw_familiar_ai task_finder`
  - `cargo test -p bevy_app world_replace`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- ロールバック境界:
  - UI / action を持たない runtime observation として単独で戻せる。

## M2: shallow 診断で成立する read-only dashboard

- 変更内容:
  - `TaskStatusSummary`、`TaskBlockerReason`、priority tier、flat `TaskEntry` を追加する。
  - `TaskEntry` は WorkType を明示的に持ち、root adapter が既存 Designation universe、worker、
    producer coverage、input revision から status を導出する。
  - M1 で未対応の deep reason と別 producer の coverage は安全側に Pending とする。
  - filter / sort state、toolbar、status line、stable ordering を実装する。
  - default WorkType grouping と他 key の flat list を実装する。
  - existing focus / pin を維持し、cycle content 不変時の rebuild を抑止する。
  - `TaskListBody` 直下の dynamic child と view state を world replacement で同期 reset する。
  - hidden / visible / filter変更で AI counter が同一である統合テストを追加する。
- 変更ファイル:
  - `crates/hw_ui/src/panels/task_list/types.rs`
  - `crates/hw_ui/src/panels/task_list/render.rs`
  - `crates/hw_ui/src/panels/task_list/interaction.rs`
  - `crates/hw_ui/src/panels/task_list/mod.rs`
  - `crates/hw_ui/src/components.rs`
  - `crates/hw_ui/src/lib.rs`
  - `crates/bevy_app/src/interface/ui/panels/task_list/view_model.rs`
  - `crates/bevy_app/src/interface/ui/panels/task_list/dirty.rs`
  - `crates/bevy_app/src/interface/ui/panels/task_list/presenter.rs`
  - `crates/bevy_app/src/interface/ui/panels/task_list/update.rs`
  - `crates/bevy_app/src/interface/ui/plugins/info_panel.rs`
- 完了条件:
  - [ ] Working / 5 blocker / Pending の formatter が空文字にならない。
  - [ ] submitted のみを accepted / Working と誤認せず、current `TaskWorkers` がなければ Pending となる。
  - [ ] filter 4 種、sort 4 key、direction、Entity tie-break を pure test で網羅する。
  - [ ] priority tier が row color、summary、filter、sort で一致する。
  - [ ] cycle / revision 数値だけの更新で子 UI を rebuild しない。
  - [ ] action 未実装の M2 でも row focus / pin と Modal / Pause capture が退行しない。
  - [ ] Build の別 producer 未評価や深い policy 未評価を Blocked にせず Pending とする。
  - [ ] reset 境界内で header / empty row / wrapper を除去し、旧 Entity を次 Update まで残さない。
  - [ ] dashboard hidden / visible / filter変更で候補評価・source scan・connectivity・A* counter が完全一致する。
- 検証:
  - `cargo test -p hw_ui task_list`
  - `cargo test -p bevy_app task_list`
  - `cargo test -p bevy_app --features profiling task_dashboard`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- ロールバック境界:
  - M1 diagnostics を残し、read-only UI adapter / widget だけを戻せる。

## M3: deep reason、arbitration evidence、全 producer coverage

- 変更内容:
  - M3a で basic / floor / water、M3b で haul / wheelbarrow / source selector の順に
    `bool` / `Option` を typed attempt result へ移行する。
  - source 不在、tool 不在、destination capacity、demand 消滅、reservation conflict を分類する。
  - wheelbarrow arbitration の既存 rebuild pass で request ごとの typed outcome と availability / arbitration stamp を
    latest-only publish する。vehicle 0 は header だけで表し、Familiar 側は lease 不在時に current snapshot を参照して
    同じ request / source / vehicle scan を複製しない。
  - source / worker の branch hit を Familiar-local reducer で `(task, producer, familiar)` terminal outcome へ縮約する。
  - builder success を `TaskAssignmentRequest` submitted と一致させ、Move template 欠落等の
    false positive をなくす。
  - `blueprint_auto_build_system` に独立した latest-only producer diagnostics を追加し、root が
    general delegation と applicable producer coverage を merge する。auto-build も writer への実 submitted だけを
    producer 肯定証拠とし、`TaskWorkers` がなければ view model は Pending とする。
  - worker distance、connectivity false、未評価 worker の coverage を区別する。
  - reservation 由来 `TemporaryContention` と connectivity `Unreachable` を混同しない。
- 変更ファイル:
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_assigner.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/builders/`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/diagnostics.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/`
  - `crates/hw_logistics/src/transport_request/plugin.rs`
  - `crates/hw_logistics/src/transport_request/mod.rs`
  - `crates/hw_logistics/src/lib.rs`
  - `crates/hw_soul_ai/src/soul_ai/decide/work/auto_build.rs`
  - `crates/hw_soul_ai/src/soul_ai/decide/work/diagnostics.rs`（新規候補）
  - `crates/hw_soul_ai/src/soul_ai/mod.rs`
  - `crates/hw_soul_ai/src/lib.rs`
  - `crates/bevy_app/src/systems/soul_ai/mod.rs`
  - `crates/bevy_app/src/plugins/logic.rs`
  - `crates/bevy_app/src/interface/ui/panels/task_list/view_model.rs`
- 完了条件:
  - [ ] 対象 WorkType の attempt が submitted または typed terminal rejection を返す。
  - [ ] source / tool / capacity / reservation / demand の代表 fixture が表示分類へ到達する。
  - [ ] no wheelbarrow / 全台 PushedBy・使用中は current rebuild header で Missing / TemporaryContention を区別し、
    no source / source reserved、実容量 0、incoming reservation、
    dedup / grant competition、preferred batch wait は arbitration owner の typed outcome になり、
    lease 不在 1 bit へ潰れない。
  - [ ] available wheelbarrow 0 の診断のために request / source / vehicle 全件 scan を追加せず、既存の全車 query の
    `is_empty()` または owner count を使い、未走査 request record を seed しない。
  - [ ] arbitration rebuild がない frame は直前 snapshot を保持し、request removal / world replacement / revision mismatch は
    stale reason を利用せず Pending になる。
  - [ ] ある Familiar / producer の rejection 後に別 Familiar / producer が submitted なら、TaskWorkers 0 は Pending、
    TaskWorkers > 0 は Working となり blocker を表示しない。
  - [ ] lower-ranked、candidate universe 外、producer 未評価 task は reason vote があっても Pending のままである。
  - [ ] TaskArea / Yard / `ManagedTasks` の異なる Familiar 間で、実 candidate 集合にない `NotApplicable` は
    vote / coverage の分母に入らない。一方、global Build / Yard-owned membership は分母に入り、
    applicable evaluator の short-circuit だけが Pending を維持する。
  - [ ] Soul / source 候補数を変えても producer × applicable evaluator 1 vote の代表理由が安定する。
  - [ ] worker spawn / sort 順を変えても同じ Familiar-local terminal outcome になり、raw rejection 件数を票にしない。
  - [ ] general / blueprint producer の submitted 後に worker assignment が成立しない、または同 frame に解除された場合、
    `TaskWorkers` 0 を Working / Blocked と誤表示せず Pending にする。
  - [ ] `PathSearchResult::Deferred` を `Unreachable` へ写像するコードがない。
  - [ ] world replacement で Blueprint producer record / cycle header と wheelbarrow arbitration record / header の
    旧 Entity が 0 件になる。
  - [ ] 列挙済みの Move false-positive 修正を除き、assignment request、reservation shadow、candidate /
    connectivity count が移行前と一致する。意図した差分は専用 fixture で固定する。
- 検証:
  - `cargo test -p hw_familiar_ai task_management`
  - `cargo test -p hw_familiar_ai source_selector`
  - `cargo test -p hw_logistics wheelbarrow_arbitration`
  - `cargo test -p hw_soul_ai auto_build`
  - `cargo test -p hw_familiar_ai --features profiling delegation`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- ロールバック境界:
  - M1/M2 の shallow dashboard を残し、M3a / M3b（wheelbarrow arbitration snapshot を含む）または
    blueprint producer ごとに typed migration を戻せる。

## M4: capability 付き priority / cancel action

- 変更内容:
  - `TaskActionCapabilities`、priority / cancel intent、inline confirmation を追加する。
  - 保存対象 `PlayerIssuedDesignation` を manual Chop / Mine の発行時だけ付与する。
  - root capability resolver を positive source-kind allow-list として実装し、render と apply で共用する。
  - manual Chop / Mine と `ManualTransportRequest` だけ priority を 0 / 5 / 10 tier で変更する。
  - manual generic cancel と、`hw_logistics::transport_request::lifecycle` 所有の ManualTransportRequest typed close API、
    Blueprint 専用 cancel lifecycle、
    Floor / Wall の保存済み request kind + anchor 経路へ接続する。
  - Blueprint priority、Move、provenance 不明、auto gather / request、GeneratePower は disabled かつ apply 時も拒否する。
  - `TaskActionOutcome` を A2 `ToastOnly` notification へ `NotificationSystemSet::Adapt` で変換する。
  - task action reader は capture / pause 中も drain し、confirmation は capture 開始時に clear する。
  - cancel request と次 frame owner cleanup の production order、index miss fallback、load 境界をテストする。
- 変更ファイル:
  - `crates/hw_jobs/src/model.rs` または source marker 専用 module
  - `crates/hw_jobs/src/lib.rs`
  - `crates/hw_ui/src/intents.rs`
  - `crates/hw_ui/src/components.rs`
  - `crates/hw_ui/src/lib.rs`
  - `crates/hw_ui/src/panels/menu.rs` または `crates/hw_ui/src/panels/task_list/interaction.rs`
  - `crates/hw_ui/src/panels/task_list/`
  - `crates/bevy_app/src/interface/ui/panels/task_list/actions.rs`（新規候補）
  - `crates/bevy_app/src/interface/ui/interaction/intent_handler.rs`
  - `crates/bevy_app/src/interface/ui/interaction/menu_actions.rs`
  - `crates/bevy_app/src/interface/ui/plugins/core.rs`
  - `crates/bevy_app/src/interface/ui/plugins/notifications.rs`
  - `crates/bevy_app/src/interface/ui/notifications.rs`
  - `crates/bevy_app/src/plugins/messages.rs`
  - `crates/bevy_app/src/systems/save/schema.rs`
  - `crates/bevy_app/src/systems/command/area_selection/apply.rs`
  - `crates/bevy_app/src/systems/command/area_selection/cancel.rs`
  - `crates/bevy_app/src/systems/command/area_selection/cleanup.rs`
  - `crates/hw_logistics/src/transport_request/lifecycle.rs`
  - `crates/hw_logistics/src/transport_request/mod.rs`
  - `crates/hw_logistics/src/lib.rs`
  - `crates/bevy_app/src/systems/jobs/blueprint_cancellation.rs`（新規候補）
  - `crates/bevy_app/src/systems/jobs/floor_construction/cancellation.rs`
  - `crates/bevy_app/src/systems/jobs/wall_construction/cancellation.rs`
  - `crates/bevy_app/src/plugins/logic.rs`
- 完了条件:
  - [ ] nested Button がなく、action click で focus click が重複発火しない。
  - [ ] stale Entity / Designation、capability 変更、pause / capture を apply 時に drain + 安全に拒否し、
    unpause 後の遅延適用と confirmation 持越しがない。
  - [ ] 新規 manual Chop / Mine だけ provenance marker を保存し、legacy / load 後 auto / unknown task は read-only。
  - [ ] priority は許可 task だけ変更し、Blueprint には表示せず、save/load 後も既存 `Priority` 値を保持する。
  - [ ] generic cancel は `Priority` / provenance を残さず、manual request は lifecycle owner の typed API を通って
    worker / reservation / pinned source を残さない。anchor cleanup と UI cancel が同じ close primitive / 除去集合を使う。
  - [ ] Blueprint cancel は全 Soul の `AssignedTask` payload から Build / HaulToBlueprint /
    Blueprint 宛 wheelbarrow worker を列挙し、`TaskWorkers` 欠落時も `TargetBlueprint` request、予約、pending companion、
    building / stockpile WorldMap 登録を残さず、搬入資材を exactly once refund する。
  - [ ] Floor / Wall tile と関連 material request の双方から site-wide cancel でき、資材返却 / request / tile /
    WorldMap 契約を通る。save/load 後に target marker がなくても `TransportRequest.kind + anchor` から解決でき、
    stale index / 0 tile でも永久 pending にならない。
  - [ ] Move / auto producer task を誤って変更・despawn できない。
  - [ ] owner cancellation set + ApplyDeferred が production schedule で Familiar Perceive より前に実行され、
    TransportRequest Perceive / Decide と Soul Perceive も cancel 済み状態だけを読み、対象 request を同 frame に再生成しない。
  - [ ] blocker 更新は notification を発行せず、操作 1 回が bounded notification 1 件以下になる。
    別 Entity、成功 / 拒否は dedupe されず、同 frame に Adapt -> Reduce -> Present される。
- 検証:
  - `cargo test -p hw_ui task_list`
  - `cargo test -p bevy_app task_dashboard_action`
  - `cargo test -p hw_logistics transport_request_lifecycle`
  - `cargo test -p bevy_app construction_cancellation`
  - `cargo test -p bevy_app systems::save`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- ロールバック境界:
  - read-only M2/M3 を残し、action intent / adapter / provenance / owner cancellation / controls を一括で戻せる。

## M5: reset、統合受入、恒久文書同期

- 変更内容:
  - diagnostics、UI view、confirmation、Message の world replacement reset を統合確認する。
  - 同一 schema / fixture で dashboard hidden / visible の counter と frame cost を比較する。
  - task、AI、UI、Message、load reset、invariant、wheelbarrow arbitration / TransportRequest lifecycle、
    Blueprint / Floor / Wall cancellation、crate owner 文書を同期する。
  - 関連提案の A3 実装状態を更新し、手動受入後に本計画を archive する。
- 変更ファイル:
  - `docs/task_list_ui.md`
  - `docs/tasks.md`
  - `docs/familiar_ai.md`
  - `docs/logistics.md`
  - `docs/building.md`
  - `docs/events.md`
  - `docs/architecture.md`
  - `docs/invariants.md`
  - `docs/cargo_workspace.md`
  - `docs/save_load.md`
  - `docs/notifications.md`
  - `crates/hw_jobs/README.md`
  - `crates/hw_familiar_ai/README.md`
  - `crates/hw_soul_ai/README.md`
  - `crates/hw_logistics/README.md`
  - `crates/hw_spatial/README.md`
  - `crates/hw_ui/README.md`
  - `crates/bevy_app/src/interface/README.md`
  - `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
  - `docs/plans/README.md`
- 完了条件:
  - [ ] owner、reason、coverage、revision、action capability、reset、system order が code / docs で一致する。
  - [ ] load 後に旧 Entity の diagnostics / selection / confirmation が残らない。
  - [ ] `PlayerIssuedDesignation` / priority の save 往復と、diagnostics の非永続・再評価を同時に確認できる。
  - [ ] UiIntent / action outcome の未読 buffer と全 task-list dynamic child が reset 境界内で空になる。
  - [ ] 提案 A3 の 3 受入条件を自動テストと手動シナリオへ対応付けられる。
  - [ ] docs index が最新で、完了計画が archive されている。
  - [ ] full quality gate が成功する。
- 検証:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `python3 scripts/dev.py verify`
  - `git diff --check`
- ロールバック境界:
  - code milestone と対応する durable docs を同じ単位で戻す。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 理由が記録されなかった task を blocker 扱いする | 高順位 task の短絡で下位 task を誤診する | coverage を別に持ち、complete 以外は Pending にする |
| 全 Familiar または scope 名だけを task coverage の分母にする | 通常 task は別 TaskArea を待ち、global Build は必要 evaluator を落とす | global 補助 scan を含む実 universe membership から task / producer 別集合を作る |
| 1 producer の rejection で blocker を決める | Build の別割当経路が実行可能なのに誤診する | applicable producer mask を持ち、全 producer complete だけ Blocked |
| branch hit の生件数を比較する | worker / source 数が多い理由へ代表が偏る | producer × applicable evaluator 1 terminal vote へ正規化する |
| Familiar-local reducer が未定義 | worker の spawn / sort 順や Soul 数で 1 票の理由が変わる | submitted -> partial -> zero worker -> reason presence + fixed precedence を固定する |
| wheelbarrow lease / available query だけを見る | 全台 PushedBy を不存在扱いし、no source、batch wait、競合も誤分類する | arbitration owner が全車存在 header + 既存 rebuild pass の typed latest snapshot を公開する |
| bool success を assignable と見なす | request 未発行なのに blocker を隠す | writer へ渡した builder submitted だけを producer 側の肯定証拠にする |
| submitted を accepted と見なす | downstream rejection / same-frame abandon 後に false Working を表示する | stable positive は current `TaskWorkers` だけ。submitted + worker 0 は Pending とする |
| 全 policy を一度に typed 化する | 巨大 diff で assignment 挙動が変わる | M1 shallow → M2 UI → M3a basic/floor/water → M3b haul の順で移行する |
| 詳細 reason が UI 文言へ漏れる | 内部実装と表示が密結合する | root adapter で 5 class へ縮約し、文字列は hw_ui だけで作る |
| runtime Deferred を Unreachable と扱う | 一時的な探索予算不足を永久障害と誤表示する | candidate reservation deferred と runtime path deferred を別契約にする |
| global dirty で全 task が常時 Pending になる | dashboard が役に立たない | domain 別 revision + dependency mask で必要な reason だけ失効する |
| revision の依存方向が逆転する | lower crate が root/AI型を参照する | owner generation 公開 + root bridge + producer read-only snapshot |
| root sync を anonymous ApplyDeferred の前後指定だけで置く | auto-gather の同 frame 変更を古い revision で評価する | leaf-owned named set chain に flush / sync / delegation seam を作る |
| UI 表示で候補探索を再実行する | 規模依存の CPU 増加と AI/UI 不一致 | diagnostics は normal cycle の副産物、UI は Resource read-only とする |
| cycle ごとに UI を rebuild する | hover / confirmation 消失、Entity churn | cycle ID を equality から外し、表示内容差分だけ dirty にする |
| nested Button | action と row focus の二重発火 | focus row と action bar を sibling にする |
| producer-owned Priority を変更する | 次 frame に上書きされ、表示と実態がずれる | capability matrix と live revalidation で read-only にする |
| 非永続 auto marker の欠落を manual と誤認する | load 後の auto task を破壊する | 保存済み manual provenance の positive allow-list、未知は read-only |
| generic cancel を全 task に使う | Move reservation、construction site、auto request が壊れる | owner 別 cancel enum と既存専用 cleanup を使う |
| root が ManualTransportRequest component を個別 remove する | anchor cleanup と UI cancel の lease / pinned source 除去がずれる | `hw_logistics` lifecycle の typed close primitive を両経路で共用する |
| Blueprint の即時 despawn | 関連 haul、予約、資材、WorldMap 登録が残る | requested marker + formal owner cancellation + refund test |
| Floor / Wall の 1 tile cancel に見える | site 全体を意図せず削除する | label を `Cancel site`、2-step confirmation にする |
| stale Entity action | load / despawn 後に別 Entity を操作する | live Entity + component + source kind を再検証し、reset する |
| pause 中に reader を止める | unpause 後に action が遅延適用される | reader は常時 drain、live gate で即時拒否、confirmation clear |
| diagnostic counter が増え続ける | 長時間プレイでメモリ増加 | latest cycle map 置換、fixed reason array、active task key だけ保持する |

## 7. 検証計画

### 7.1 判定原則

- 値、状態遷移、Entity cleanup、並び順、保存値、counter は再現可能なので、自動テストを受入の正本にする。
- 実機確認へ残すのは、実 renderer の可読性、実 pointer hit-test / overlay の重なり、実時間・実メモリ計測だけとする。
- 「自動テスト追加待ち」を手動確認で代替しない。該当テストが通るまで未受入のままとする。
- unit / headless integration で保証済みの項目を、実機チェックリストへ重複掲載しない。
- performance 比較は同じ binary、feature、seed、fixture、fixed tick 数、counter schema だけを使う。

### 7.2 自動化済み

| ID | 保証する契約 | 主な回帰テスト |
| --- | --- | --- |
| A01 | Working / Blocked / Pending の派生、submitted かつ worker なしは Pending、正確な文言、3種類の semantic color token | `workers_override_stale_or_blocked_diagnostics`、`submitted_without_current_workers_remains_pending`、`complete_terminal_rejection_is_blocked`、`every_status_has_the_exact_dashboard_label`、`task_statuses_use_distinct_semantic_theme_colors` |
| A02 | 4 filter、4 sort key、2 direction、Entity tie-break、control の一巡 | `every_filter_dimension_selects_the_expected_entries`、`every_sort_key_and_direction_has_a_deterministic_order`、`descending_sort_keeps_entity_tie_break_deterministic`、`dashboard_controls_cycle_back_to_their_defaults` |
| A03 | row 押下時の camera / InfoPanel pin、action 押下による余分な focus の不在、capture 中の row / toolbar 押下と confirmation を持ち越さない | `row_press_focuses_camera_and_pins_the_target`、`action_button_press_does_not_trigger_row_focus`、`captured_row_press_is_drained_without_delayed_focus`、`captured_toolbar_press_is_not_applied_after_capture_ends`、`capture_start_clears_pending_cancellation_confirmation` |
| A04 | manual Chop / Mine、ManualTransportRequest、Blueprint、Floor / Wall と read-only 対象の capability allow-list、適用時 live revalidation、Pause / capture 中の button ingress / intent drain | `task_dashboard_action_capability_allow_list_rejects_unmarked_designations`、`task_dashboard_action_capabilities_match_the_owner_matrix`、`task_dashboard_action_applies_priority_only_after_live_revalidation`、`task_dashboard_captured_or_paused_action_press_leaves_no_intent_or_confirmation` |
| A05 | latest-only diagnostics、固定幅 reason counter、partial coverage、producer mask、reason 別 revision domain | `counters_saturate_and_use_fixed_representative_tie_break`、`partial_or_submitted_coverage_is_not_a_complete_rejection`、`build_requires_both_assignment_producers`、`only_used_revision_domains_invalidate_a_record`、各 `publish_*` test |
| A06 | manual transport owner close、Blueprint / Floor / Wall の基礎 cleanup、cancel と task execution の順序 | `transport_request_lifecycle_manual_close_unpins_and_requests_worker_cleanup`、`cancellation_uses_payload_and_refunds_delivered_materials_once`、Floor / Wall の `construction_cancellation_*`、`blueprint_cancel_unassigns_before_same_update_task_execution` |
| A07 | world replacement で old Entity、confirmation、dynamic row、Message、diagnostics を破棄 | `world_replace_reset_clears_entity_bearing_ui_state`、`world_replace_reset_drops_entity_revisions_and_snapshots`、各 diagnostics reset / latest-only test |

### 7.3 自動テスト移管状況（旧手動シナリオから移管）

以下はすべて自動化対象であり、ユーザーの実機確認項目にはしない。旧手動シナリオ由来の決定的確認は
T01〜T10 まで自動化済みで、残る T11 も手動確認へ戻さず perf harness を整備してから受け入れる。

| ID | 旧シナリオ | 追加する決定的 fixture / assertion | 状態 |
| --- | --- | --- | --- |
| T01 | 2 | 離れた 2 TaskArea で非 global、global Build、Yard-owned の task 別 `applicable_evaluators` と `NotApplicable` を同一 cycle で検証 | [x] `diagnostic_membership_uses_each_real_candidate_universe` |
| T02 | 3 | 実評価経路で Blueprint 依存待ち、bucket / wheelbarrow 不足と車両競合を作り、5分類から最終 `TaskStatusSummary` への写像を検証 | [x] `diagnostic_membership_uses_each_real_candidate_universe`、`gather_water_without_a_bucket_is_missing_resource_not_contention`、`missing_wheelbarrow_inputs_are_distinct_from_temporary_contention`、`terminal_diagnostic_classes_map_to_dashboard_blockers` |
| T03 | 4 | locked door / wall の connectivity false と、距離超過だけでは `Unreachable` にしないことを delegation diagnostics まで統合検証 | [x] connectivity cache test 群 + `distance_limit_and_connectivity_use_distinct_rejection_classes` |
| T04 | 5 | source / destination 予約あり cycle で `TemporaryContention`、解放後の次 cycle で理由が消える遷移を検証 | [x] arbitration reservation test 群 + `released_arbitration_contention_clears_on_the_next_evidence` |
| T05 | 8 | manual Chop / Mine / ManualTransport の 0 / 5 / 10 遷移、sort / summary、`Priority` と provenance の save/load 往復を検証 | [x] shared tier / capability / sort / summary test と `root_marker_matrix_collects_extracts_and_round_trips_durable_entities` |
| T06 | 9 | UI intent から generic / manual cancel owner まで通し、Priority、provenance、worker、予約、pinned source、row の消滅を検証 | [x] `task_dashboard_cancel_intents_route_through_task_owners` + `transport_request_lifecycle_manual_close_unpins_and_requests_worker_cleanup` |
| T07 | 10 | Blueprint cancel fixture に `HaulToBlueprint`、Blueprint 宛 wheelbarrow、TargetBlueprint / anchor request と予約を追加して全 cleanup を検証 | [x] 拡張済み `cancellation_uses_payload_and_refunds_delivered_materials_once` + `user_unassign_cleans_assignment_before_abandonment_notification` |
| T08 | 11 | Floor / Wall の tile 行 / request 行起点、worker、0 tile、全 material、spawned wall entity を表駆動で検証 | [x] capability owner matrix + Floor / Wall の `construction_cancellation_*` test 群 |
| T09 | 13 | `TaskActionButton` の実 ingress を Pause / Modal 中の `Interaction::Pressed` から通し、intent / 2-step confirmation が発行・遅延適用されないことを検証 | [x] A03 / A04 の capture test 群 |
| T10 | 14 | load reset 後の最初の producer cycle から新 `TaskListState` / dynamic row が再構築されることを検証 | [x] `task_dashboard_rebuilds_from_the_first_post_load_producer_cycle` + A07 reset test 群 |
| T11 | 性能 | perf fixture に dashboard hidden / visible / active-filter mode と不足 counter を追加し、同一 fixed tick で AI work counter が完全一致することを検証 | [ ] |
| T12 | 性能 | active task 数を段階的に増やし、各 latest-only map が active task 数以下、record が固定幅で evaluator 行列を保持しないことを検証 | [x] `published_map_scales_with_current_tasks_not_evaluator_history`、`published_task_records_are_fixed_width_and_heap_free`、各 producer の latest-only test |

T11 では既存の source selector、`reachable_with_cache_calls`、runtime A* / deferred counter を再利用する。
candidate snapshot / score attempt と wheelbarrow arbitration rebuild / bucket build / Top-K scan は現状 counter がないため、
profiling counter と capture schema を先に追加する。UI system 自体の時間は AI work counter と分離する。

### 7.4 実機でのみ確認する項目

現在ユーザーへ依頼できる実機確認は R01 / R02 の 2 件だけである。R03 は性能計測基盤 T11 の完成後にだけ
実施する将来の計測であり、現時点の実機確認リストには含めない。

#### R01 表示の可読性

1. Working / Blocked / Evaluating の 3 行と、Normal / High / Critical の行を同時に表示する。
2. デフォルト UI scale と通常のゲーム背景で、文言、状態色、priority 色が互いに判別できることを目視する。
3. 文字切れ、重なり、action bar による行幅崩れがないことを確認する。

自動テストはラベルと theme token の対応までを保証し、最終的な視認性だけをここで確認する。

#### R02 実 pointer hit-test と overlay

1. row 本体をクリックし、camera と InfoPanel がその対象へ移ることを確認する。
2. 同じ行の priority / cancel button をクリックし、camera / InfoPanel が別途移動しないことを確認する。
3. Pause menu と Modal をそれぞれ開いた状態で、背後の toolbar / row / action をクリックする。
4. 背後操作が発火せず、閉じた直後にも押下や cancel confirmation が遅延発火しないことを確認する。

headless test は system routing と capture 後の持越し防止までを保証し、実 pointer の z-order / hit-test だけをここで確認する。

#### R03 実 renderer / allocator の性能

T11 の dashboard mode 付き perf harness 完成後にだけ実施する。完成前は再現可能な正式手順がないため、受入済みにしない。

1. 同一 binary / seed / fixture / measure 秒数で hidden / visible / active-filter を取得する。
2. UI system CPU、frame-time、allocation / peak memory を比較し、既存 perf policy の回帰判定を通す。
3. task 数を増やした capture で実割当量が線形範囲に収まり、task × evaluator 行列相当の増え方をしないことを確認する。

### 7.5 旧手動シナリオの移管表

| 旧番号 | 新しい正本 |
| --- | --- |
| 1 | A01 + R01 |
| 2〜5 | T01〜T04 |
| 6 | A02 |
| 7 | A03 + R02 |
| 8 | A04 + T05 + R01 |
| 9〜11 | T06〜T08 |
| 12 | A04 |
| 13 | A03 / A04 + T09 + R02 |
| 14 | A07 + T10 |

### 7.6 必須コマンド

- 重点テスト:
  - `cargo test -p hw_familiar_ai task_management`
  - `cargo test -p hw_ui task_list`
  - `cargo test -p bevy_app@0.1.0 task_dashboard`
  - 追加した T01〜T12 の個別 test command
- 計画完了時:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `python3 scripts/dev.py verify`
  - `git diff --check`

## 8. ロールバック方針

- M1、M2、M3、M4 はそれぞれ別 commit 境界にできる。M0 は contract/test、M5 は対応 docs とする。
- M2 を戻しても M1 diagnostics は UI 非依存 Resource として残せる。不要なら M1 と続けて戻す。
- M3 を戻す場合、該当 WorkType / producer の typed result と診断 mapping を同時に戻し、
  bool adapter と typed adapter を二重に残さない。M2 の shallow dashboard は維持できる。
- M4 を戻す場合、`UiIntent` variant、provenance schema、owner cancellation、action bar、root adapter、通知 test を一括で戻し、
  read-only dashboard を維持する。
- M4 は `PlayerIssuedDesignation` を save schema に追加する。rollback / older build 互換を実行前に確認し、
  component だけを残した中途半端な rollback をしない。marker がない task は常に read-only fallback とする。
- cancel owner API 自体の cleanup 改修が必要になった場合は、その owner の test / docs と同じ commit に分離する。
- 実際の rollback 前には repository の Git Revert Policy に従い、`git log --oneline -5` と
  対象 `git diff HEAD -- <file>`、並行作業の有無を確認する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `95%`（M0〜M4 と M5 のコード・恒久文書、旧手動シナリオの自動化まで完了）
- 完了済みマイルストーン: `M0`〜`M4`
- 残作業: T11 の再現可能な性能計測基盤、R01 / R02 の最小実機受入、T11 完成後の R03、受入後の本計画 archive

### 次のAIが最初にやること

1. §7.4 の R01 / R02 だけを実機確認する。決定的な状態・cleanup・counter は手動確認へ戻さない。
2. T11 の perf harness と不足 counter を整備し、同一 fixture の自動比較が通ってから R03 を計測する。
3. 問題がなければ本計画を `docs/plans/archive/` へ移し、`python3 scripts/dev.py docs --write` と
   `python3 scripts/dev.py verify` を再実行する。
4. current worktree の `docs/plans/implementation-spec-alignment-plan-2026-07-20.md` は別作業なので、
   A3 archive / commit 時に破棄・混入させない。

### ブロッカー/注意点

- 現在の worktree には A3 と無関係な `implementation-spec-alignment` 計画がある。破棄・巻き込みをしない。
- 「reason がない」は blocker ではない。coverage incomplete は必ず `PendingEvaluation`。
- task record がないことを rejection としない。zero roster だけは cycle header + root Designation universe から導出する。
- coverage の分母は全 Familiar でも scope 名の再計算でもない。global Build / Yard-owned 補助 scan を含む、
  重複排除後・filter 前の実 candidate universe membership から task / producer ごとの applicable evaluator 集合を作る。
  実集合にない `NotApplicable` Familiar だけを分母と vote から外す。
- raw branch count を代表理由に使わず、producer × applicable evaluator 1 terminal vote にする。
- Familiar-local reducer は submitted -> partial -> zero worker -> reason presence + fixed precedence の順とし、
  worker / source 件数や spawn / sort 順を使わない。
- Build は general / blueprint-auto-build の全 applicable producer coverage が揃うまで Pending。
- writer へ渡した `TaskAssignmentRequest` submitted だけを producer 側の肯定証拠とし、現行 `bool == true` を信用しない。
  submitted は accepted ではなく、current `TaskWorkers` がなければ Pending とする。
- wheelbarrow lease 不在だけを reason にしない。`hw_logistics` arbitration の current latest-only outcome を参照し、
  Familiar 側に source / vehicle scan を複製しない。全台 PushedBy / 使用中を不存在と誤認しないよう、物理的な
  Wheelbarrow の存在は `Without<PushedBy>` available query ではなく全車 query / owner count を正本にする。
- 仲裁 outcome は `WheelbarrowHaul` という WorkType 名で接続せず、`TransportRequest` の適用条件で `Haul` /
  `HaulToMixer` request に接続する。予約混在で `hard_min` を割る場合は `SourceReserved` とする。
- candidate connectivity は Boolean cache、runtime `PathSearchResult::Deferred` は別契約。
- UI を開いたことを AI へ通知しない。diagnostics は常に normal assignment producer cycle の副産物。
- auto producer は Priority を上書きするため初版 action 対象外。
- manual Chop / Mine は保存済み positive provenance がある場合だけ操作可能。auto marker 不在を許可条件にしない。
- Blueprint priority は read-only。cancel は generic despawn でなく関連 haul / refund / WorldMap を閉じる owner lifecycle。
- Blueprint cancel は `TaskWorkers` だけを信じず、全 Soul の `AssignedTask` payload から Build / HaulToBlueprint /
  Blueprint 宛 wheelbarrow task を列挙する。
- Move cancel は予約 cleanup が不足するため初版対象外。
- Floor / Wall cancel は tile / material request のどちらからでも parent site 全体へ解決する。material request は
  非永続 target marker でなく保存済み `TransportRequest.kind + anchor` を正本とし、owner set + ApplyDeferred を
  Familiar Perceive より前に置く。
- task list row は Button なので action control を descendant にしない。
- cycle ID / revision を `TaskEntry` equality に含めない。
- diagnostics と UI confirmation は Entity を持つ runtime state。world replacement reset が必須。
- blocker を A2 通知へ流さず、action outcome だけを bounded toast にする。
- capture / pause 中も action reader を止めず drain し、confirmation を capture 開始時に clear する。
- auto-gather 後の task revision sync は leaf-owned named set の `AutoGatherFlush -> TaskRevisionSync -> Delegation` で固定する。
- ManualTransportRequest cancel は `hw_logistics::transport_request::lifecycle` の typed close primitive を通し、
  root が component cleanup を複製しない。
- Bevy API は 0.19 source で確認し、Message / Event、Change Detection、Relationship の旧版例を使わない。
- 他セッションの変更を破棄しない。rollback 時は AGENTS.md の Git Revert Policy を守る。

### 参照必須ファイル

- `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
- `docs/task_list_ui.md`
- `docs/tasks.md`
- `docs/familiar_ai.md`
- `docs/logistics.md`
- `docs/building.md`
- `docs/invariants.md`
- `docs/architecture.md`
- `docs/events.md`
- `docs/save_load.md`
- `docs/notifications.md`
- `crates/hw_ui/src/lib.rs`
- `crates/hw_ui/src/panels/task_list/types.rs`
- `crates/hw_ui/src/panels/task_list/render.rs`
- `crates/hw_ui/src/panels/task_list/interaction.rs`
- `crates/hw_ui/src/intents.rs`
- `crates/bevy_app/src/interface/ui/panels/task_list/view_model.rs`
- `crates/bevy_app/src/interface/ui/panels/task_list/dirty.rs`
- `crates/bevy_app/src/interface/ui/plugins/info_panel.rs`
- `crates/bevy_app/src/interface/ui/plugins/notifications.rs`
- `crates/bevy_app/src/interface/ui/notifications.rs`
- `crates/bevy_app/src/plugins/messages.rs`
- `crates/bevy_app/src/plugins/logic.rs`
- `crates/bevy_app/src/systems/command/area_selection/apply.rs`
- `crates/bevy_app/src/systems/command/area_selection/cancel.rs`
- `crates/bevy_app/src/systems/command/area_selection/cleanup.rs`
- `crates/bevy_app/src/systems/familiar_ai/perceive/resource_sync.rs`
- `crates/bevy_app/src/systems/save/schema.rs`
- `crates/bevy_app/src/systems/jobs/floor_construction/cancellation.rs`
- `crates/bevy_app/src/systems/jobs/wall_construction/cancellation.rs`
- `crates/hw_soul_ai/src/soul_ai/decide/work/auto_build.rs`
- `crates/hw_logistics/src/resource_cache.rs`
- `crates/hw_logistics/src/transport_request/arbitration/`
- `crates/hw_logistics/src/transport_request/lifecycle.rs`
- `crates/hw_spatial/src/resource.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
- `crates/hw_familiar_ai/src/familiar_ai/mod.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/filter.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/score.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_assigner.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/builders/`

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-07-20` / pass（`python3 scripts/dev.py verify`）
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `2026-07-20` / pass、0 warning
- 最終 `cargo test --workspace`: `2026-07-20` / pass（Bevy app 204 tests、workspace 全 crate / doctest）
- 最終 `python3 scripts/dev.py docs --check`: `2026-07-20` / pass（8 current plans、47 archived plans）
- 最終 `python3 scripts/dev.py verify`: `2026-07-20` / pass
- 未解決エラー: なし

### Definition of Done

- [ ] M0〜M5 がすべて完了
- [ ] 提案 A3 の 3 受入条件を自動テストと R01〜R03 の最小実機確認で満たす
- [ ] coverage 不足を blocker とする false positive が 0 件
- [ ] 非 global task で別 TaskArea の `NotApplicable` evaluator を待つ永久 Pending と、global Build / Yard-owned の
  evaluator を分母から落とす false blocker が 0 件
- [ ] applicable producer 未評価と raw branch count による false blocker が 0 件
- [ ] worker / source 件数・spawn 順に依存する Familiar-local vote、lease 不在 1 bit / 全台 PushedBy による誤分類、
  submitted を accepted と扱う false Working / Blocked が 0 件
- [ ] dashboard 操作による候補評価・source scan・connectivity・A* 増加が 0
- [ ] provenance 不明 / auto task を priority・cancel できず、許可 task の owner cleanup が完全
- [x] 影響ドキュメントが更新済み
- [x] `python3 scripts/dev.py docs --check` が成功
- [x] `cargo check --workspace` が成功
- [x] `cargo clippy --workspace --all-targets -- -D warnings` が成功
- [x] `cargo test --workspace` が成功
- [x] `python3 scripts/dev.py verify` が成功
- [ ] 完了した本計画が archive され、索引が最新

## 10. 受入条件トレーサビリティ

| 提案の受入条件 | 設計 / 実装 | 自動検証 | 手動確認 |
| --- | --- | --- | --- |
| 停滞 task に安定理由または判定待ち | M1/M3 coverage + fixed votes、M2 status adapter | A01 / A05 + T01〜T04 | R01 は可読性のみ |
| dashboard で候補評価・経路探索を増やさない | normal cycle の副産物、UI read-only | A02 / A03 + T11 / T12 | R02 の hit-test、R03 の実時間 / 実メモリ |
| priority / cancel 後も不変条件維持 | M4 capability + live revalidation + owner cleanup | A04 / A06 / A07 + T05〜T10 | R01 の表示、R02 の pointer routing |

## 11. 計画レビュー基準

実装開始前と各 milestone 完了時に、少なくとも次を再確認する。

- [ ] 現在の early return 台帳に未分類の `None` / `false` がない。
- [ ] coverage incomplete / zero rejection を Blocked にしていない。
- [ ] 全 applicable producer が complete でない task を Blocked にしていない。
- [ ] scope 名から coverage 分母を推測せず、global Build / Yard-owned を含む実 candidate 集合から
  task / producer ごとの applicable evaluator と `NotApplicable` を区別している。
- [ ] raw worker / source branch count でなく producer × applicable evaluator 1 terminal vote を使っている。
- [ ] Familiar-local reducer が submitted -> partial -> zero worker -> reason presence + fixed precedence を実装し、
  worker / source 件数・spawn / sort 順に依存していない。
- [ ] submitted でない bool success を肯定証拠にしていない。
- [ ] submitted を accepted と扱わず、current `TaskWorkers` なしを Pending にしている。
- [ ] wheelbarrow lease 不在を直接分類せず、current arbitration snapshot の typed outcome と全車存在 header を使い、
  全台 PushedBy / 使用中を Missing と誤分類していない。
- [ ] `Deferred` と `Unreachable`、距離制限と connectivity false を混同していない。
- [ ] dashboard open / filter state が AI system parameter に入っていない。
- [ ] hot path に String、reason Vec、履歴 queue、UI Query、追加 A* がない。
- [ ] volatile input 1 件で全 task を無条件に永久 Pending にしていない。
- [ ] semantic generation は owner crate 公開値 + root bridge で更新し、依存方向を逆転していない。
- [ ] task-local revision sync が named `AutoGatherFlush -> TaskRevisionSync -> Delegation` seam に入り、
  anonymous `ApplyDeferred` との偶然の順序に依存していない。
- [ ] cycle ID / revision だけで UI rebuild していない。
- [ ] row focus Button の descendant に action Button を置いていない。
- [ ] capability を cached UI 値だけで信頼していない。
- [ ] manual Chop / Mine は durable positive provenance だけを許可し、marker 不在を manual と推定していない。
- [ ] Blueprint priority を許可せず、cancel は全 Soul の direct `AssignedTask` payload、related request、refund、
  WorldMap を含む owner lifecycle を通る。
- [ ] Move / auto producer へ generic cancel / priority mutation を適用していない。
- [ ] ManualTransportRequest cancel と anchor cleanup が `hw_logistics` owner の同じ typed close primitive を使っている。
- [ ] Floor / Wall は site-wide cancellation と表示し、load 後も request kind + anchor から site を解決している。
- [ ] owner cancellation set + ApplyDeferred を Familiar Perceive より前に固定している。
- [ ] Relationship target を直接書き換えていない。
- [ ] world replacement 後に旧 Entity 参照が残らない。
- [ ] capture / pause 中も action Message を drain し、confirmation を持ち越していない。
- [ ] blocker state を A2 notification history へ周期送信していない。
- [ ] current code / tests / durable docs が同じ owner と system order を説明している。

## 12. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-19` | `Codex` | 初版作成。現行 UI / candidate / action owner を監査し、coverage、latest-only diagnostics、safe capability 境界を定義 |
| `2026-07-19` | `Codex` | 自己レビュー。producer coverage、正規化 vote、semantic revision owner、durable provenance、Blueprint / construction cancellation、UI/通知/reset順序を修正 |
| `2026-07-19` | `Codex` | 最終レビュー。global scan を含む task 別 applicable evaluator、Familiar Perceive 前の取消順序、load 後の construction request 識別、Blueprint 間接 worker cleanup を修正 |
| `2026-07-20` | `Codex` | 自己レビュー指摘を反映。wheelbarrow arbitration diagnostics、Familiar-local reducer、submitted/accepted 分離、named schedule seam、ManualTransport owner API、building/logistics 文書範囲を追加 |
| `2026-07-20` | `Codex` | M0〜M4を実装。最終コードレビューで typed attempt の深部伝播、reason別revision domain、managed Build producer、pending Stockpile cleanup、manual/auto provenance、availability検知、revision leak、予約済みTop-Kを修正し、full verifyを完了 |
| `2026-07-20` | `Codex` | 最終読み取りレビューを反映。Haul / HaulToMixer への仲裁診断接続、DependencyWaiting の task-only revision、Soul eligibility 境界、予約混在 batch の SourceReserved 判定を修正し、full verify を再完了 |
| `2026-07-20` | `Codex` | 受入項目を再監査。deterministic な旧手動シナリオを自動化済み / テスト追加待ちへ分離し、実機確認を可読性・pointer hit-test・実性能に限定。状態表示、全filter/sort、focus/capture、capability表の回帰テストを追加 |
| `2026-07-20` | `Codex` | 旧手動シナリオ由来の決定的確認を T01〜T10 の回帰テストへ移管。実機受入を現在実施可能な R01 / R02 と、perf harness 完成後の R03 に分離し、Bevy app 204 tests を含む full verify を完了 |
