# R05: TransportRequest producerの重複選択と更新処理の共通化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r05-transport-request-reconcile-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R05 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P2 / 中 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 実装・必要な回帰・仕様／Help同期・品質gateを完了。専用native受入は不要。設計経緯としてarchiveし、現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: producerごとに異なる重複request選択、worker保持、spawn/updateを揃えて追従漏れを減らす。
- 到達したい状態・成功指標: 同じidentityのrequestでcanonical選択が安定し、実行中workerを守り、非canonicalへ新規割当せず、同値reconcileではChangedが発生しない。

## 2. スコープ

### 対象（In Scope）

- producer/upsertとBlueprint/Tank/Bucket/Mixer/Wheelbarrow/Stockpile、Floor/Wall/ProvisionalWall/SoulSpaのreconcile
- identity・需要0・slot上限・priority・追加Target componentの契約表

### 非対象（Out of Scope）

- 需要計算、資源優先度、運搬経路の全面変更
- R06のsnapshot型統合、全producerを一つの巨大systemへ統合

### 主な変更対象（現行ファイル）

- [crates/hw_logistics/src/transport_request/producer/mod.rs](../../../../crates/hw_logistics/src/transport_request/producer/mod.rs)
- [crates/hw_logistics/src/transport_request/producer/bucket.rs](../../../../crates/hw_logistics/src/transport_request/producer/bucket.rs)
- [crates/hw_logistics/src/transport_request/producer/wheelbarrow.rs](../../../../crates/hw_logistics/src/transport_request/producer/wheelbarrow.rs)
- [crates/hw_logistics/src/transport_request/producer/floor_construction.rs](../../../../crates/hw_logistics/src/transport_request/producer/floor_construction.rs)
- [crates/hw_logistics/src/transport_request/producer/wall_construction.rs](../../../../crates/hw_logistics/src/transport_request/producer/wall_construction.rs)
- [crates/hw_logistics/src/transport_request/producer/provisional_wall.rs](../../../../crates/hw_logistics/src/transport_request/producer/provisional_wall.rs)
- [crates/bevy_app/src/systems/jobs/soul_spa_construction/auto_haul.rs](../../../../crates/bevy_app/src/systems/jobs/soul_spa_construction/auto_haul.rs)
- [crates/hw_logistics/src/transport_request/producer/upsert.rs](../../../../crates/hw_logistics/src/transport_request/producer/upsert.rs)
- [crates/hw_logistics/src/transport_request/producer/blueprint.rs](../../../../crates/hw_logistics/src/transport_request/producer/blueprint.rs)
- [crates/hw_logistics/src/transport_request/producer/tank_water_request.rs](../../../../crates/hw_logistics/src/transport_request/producer/tank_water_request.rs)
- [crates/hw_logistics/src/transport_request/producer/mixer_helpers/upsert.rs](../../../../crates/hw_logistics/src/transport_request/producer/mixer_helpers/upsert.rs)
- [crates/hw_logistics/src/transport_request/producer/task_area.rs](../../../../crates/hw_logistics/src/transport_request/producer/task_area.rs)
- [crates/hw_logistics/src/transport_request/producer/consolidation.rs](../../../../crates/hw_logistics/src/transport_request/producer/consolidation.rs)
- [crates/hw_logistics/src/transport_request/lifecycle.rs](../../../../crates/hw_logistics/src/transport_request/lifecycle.rs)と[plugin.rs](../../../../crates/hw_logistics/src/transport_request/plugin.rs)（終端・schedule回帰の確認入口。既存所有と順序を維持）

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

旧process_duplicate_keyはqueryで先に見たrequestを採用する。Stockpile系には実行中優先・stable IDの選定と非canonicalの新規枠を閉じる実装がある。共通spawn helperは全producerのpriority/Target構成を表現できない。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/hw_logistics/src/transport_request/producer/upsert.rs](../../../../crates/hw_logistics/src/transport_request/producer/upsert.rs)
- [crates/hw_logistics/src/transport_request/producer/task_area.rs](../../../../crates/hw_logistics/src/transport_request/producer/task_area.rs)
- [crates/hw_logistics/src/transport_request/producer/consolidation.rs](../../../../crates/hw_logistics/src/transport_request/producer/consolidation.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- identity key、manual除外、需要0時の休止/despawn、desired_slots、Target、priority tierをkind別表で固定し、意図した差をpolicyとして残す。
- canonical選択の基準は実行中優先＋Entity index/generation昇順に揃え、旧挙動との差をテストで明示する。worker付きduplicateは既存workerを残して新規slotだけ閉じる。
- 需要計算とidentity構成はproducerに残し、共通reconcilerへdesired specと追加component群を渡す。Commands経由の同値書込み回避を維持する。
- 有効なanchorを持つ需要0/duplicateと、anchor/issuer消失による終端を分ける。後者は既存MaintainのcloseとSoulTaskUnassignRequestへ委譲し、producerでworker保持や直接解除へ置き換えない。
- Decide→Arbitrate→Execute、Actor後のReconcile→Maintain、および既存のCommands/Relationship flush順序を維持する。

### 具体設計と変更境界

`producer/upsert.rs` に純粋な `select_canonical(candidates)` と `plan_reconcile(current, desired, policy) -> ReconcileAction` を置く案とし、Commands適用を分ける。canonicalは既存Stockpileと同じ「workerあり優先、同条件ならEntity index/generation昇順」にする。旧producerの先着選択からの変更を回帰で明示する。

slot入力は `TotalSlots(n)` と `AdditionalSlots(n)`（案）を区別する。SemanticRequestSpecのdesired_slotsは総枠を直渡し、Stockpileのnew_assignableだけは既存worker数を加える。SoulSpaのremaining算出など各producerの現行式を保存し、一律のworkers加算やidentity全体の新しい上限式へ置換しない。

表はproducerがreconcileへ到達した場合のpolicyであり、既存の早期return条件は維持する。fixtureは各producerの実行条件も明示し、SoulSpaのowner候補消失を単純な需要0入力として扱わない。

以下は**canonicalの需要消失時**に維持するpolicy。募集除去は既存のDesignation/TaskSlots等の除去範囲に従う。非canonicalは別規則で、workerなしを削除、workerありを既存worker数へcapし新規枠を閉じる。

| kind | workerなし | workerあり |
| --- | --- | --- |
| Blueprint / Tank | 募集除去、Demand維持 | 現状維持 |
| Mixer | inactiveは削除、activeは募集除去・Demand維持 | 現状維持 |
| ReturnBucket | 募集除去、desired_slots=0 | 募集除去、desired_slots=0、inflight保持 |
| ReturnWheelbarrow | 削除 | 募集除去、desired_slots=0 |
| Floor / Wall / ProvisionalWall / SoulSpa | 募集除去、desired_slots=0 | 募集除去、desired_slots=0、既存worker保持 |
| Deposit / Consolidate | 募集除去、desired_slots=0 | 既存worker数へcap |
| 旧BatchWheelbarrow | 削除 | 現状維持 |

TransportDemand.desired_slots=0かつworkerなしの終端化は後続Maintainが担う。Blueprint/Tank/active Mixerの需要消失は既存の正のDemandを維持するため、募集停止後もMaintainで削除しない。旧BatchWheelbarrowには生成・再有効化経路がなく、残存worker保護とworkerなし削除だけを検査する。anchor/issuer消失はこの表と分け、既存close→SoulTaskUnassignRequestを通す。
identity/需要計算/manual除外/Target/priorityの組立はproducerに残す。追加componentを任意closureで書き換える汎用frameworkは作らない。

### 実装単位と移行順

1. **R05-A / M1:** 上表とslot式を固定し、Decide直後/Actor後Maintain後の二地点で期待値を記録。
2. **R05-B / M2:** selector・slot入力・action適用をStockpileの既存helperから抽出。worker付きduplicateの保護を先に通す。
3. **R05-C / M3:** Blueprint/Tank/Mixer→返却系→Floor/Wall/ProvisionalWall/SoulSpa/root auto_haulの順に移行。各群を独立して戻せる差分にする。

### 依存関係と着手順

R06は必須先行ではない。task_area.rs/consolidation.rsを共用するため、R05のproducer群移行とR06のsnapshot移行は同時編集しない。R02のclaimは別責務として維持。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: kind別契約表と回帰

- 変更内容:
  - 全producerのidentity/worker/需要0/Target/priorityを列挙し、旧方式とStockpile方式の差を明示する。
  - duplicateの走査順を変えるテストと、同値2回目更新のテストを既存upsertテストへ追加する。
- 変更ファイル: R05-A: producer/upsert.rs と各producerの既存test、root construction/SoulSpa producer test。slot式・需要消失表を固定。
- 完了条件:
  - [x] 統一する差と維持するkind固有差を区別できる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: canonical選択とspecの共有

- 変更内容:
  - canonical選択・非canonicalのslot制御を共通関数へ切り出す。
  - priorityと追加component群を表せるspawn/update specを設計し、同値書込み抑止を維持する。
- 変更ファイル: R05-B: hw_logistics/src/transport_request/producer/upsert.rs のselector/spec/actionとStockpile利用側。
- 完了条件:
  - [x] requestごとのslot式、非canonicalの新規枠0、既存worker保持が共通contract testで成立。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: producerを群ごとに移行

- 変更内容:
  - Blueprint/Tank、Bucket/Mixer、Wheelbarrow/Stockpileを順に移行し、各群で既存testを通す。
  - sync_construction_requestsとそのFloor/Wall/SoulSpa caller、ProvisionalWallも移行する。SoulSpaにはreconcile専用testを追加し、既存delivery/cancellation testだけを根拠にしない。
  - 残った旧重複処理とspawnの重複を撤去し、arbiter/state machineとの整合を検査する。
  - active worker付きanchor/issuer消失を既存scheduleで確認し、Soul owner経由の予約・delivery解放とproducerの再生成防止を検査する。
- 変更ファイル: R05-C: producer/{blueprint,tank_water_request,mixer,bucket,wheelbarrow,mod}.rs、mixer_helpers/upsert.rs、Stockpile/floor_construction/wall_construction/provisional_wall producer、root soul_spa_construction/auto_haul.rs。
- 完了条件:
  - [x] 全producerで共通契約が成立し、kind固有需要policyが保たれる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 物流の開始/停滞/再開結果が変わる差は実経路で確認する。同値更新整理だけのbatchとcanonical挙動変更のbatchを混同しない。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/logistics.md](../../../../docs/logistics.md)
  - [docs/tasks.md](../../../../docs/tasks.md)
  - [docs/invariants.md](../../../../docs/invariants.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功し、rust-analyzerの取得可能な診断にerrorがない。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| desired_slotsを追加可能数と誤解する | TotalSlotsはrequestの総枠として直渡し、AdditionalSlotsのみ既存worker数を加算。identity全体の上限式は変更しない。 |
| kind固有の休止/despawnを共通化で失う | policy値を明示してkindごとの回帰を残す。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| duplicate順序反転・workerあり/なし | canonicalが同一。非canonicalへ新規割当なし。 |
| 需要0/duplicate/再有効化 | kind別の休止・新規slot制限を保ち、有効な実行中workerを保持する。workerなしの終了条件も既存policyに従う。 |
| anchor/issuer消失 | 既存Maintainのclose ownerへ渡してrequestを閉じ、Soul owner経由でworker・予約・deliveryを解放する。producerのreconcileは終端処理を置換しない。 |
| desired_slots・manual・priority・Target | 上限の意味とproducer固有componentを維持。 |
| 2回同値reconcile | Changed・不要spawn/despawnなし。 |
| Floor/Wall/ProvisionalWall/SoulSpaの需要更新 | 共通helper経由/独自producer双方でcanonical・slot・追加Target契約が維持される。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R05-T1 `canonical_request_is_order_independent_test` | 同identityのrequest2個を作り、一方/双方にWorkingOn workerを付ける。candidate順を反転する。 | 同じcanonical、非canonical新規枠0、既存worker/予約保持。workerなしduplicateだけ削除。 |
| R05-T2 `slot_spec_preserves_producer_arithmetic_test` | workers=2でTotalSlots(3) / AdditionalSlots(3)を適用。SoulSpaは現行remainingのfixtureを別に固定。 | 総枠はそれぞれ3 / 5。同一式へ正規化して意図せず増枠しない。 |
| R05-T3 `zero_demand_policy_survives_production_maintain_and_reactivation_test` | 現役kindで需要あり→消失→再有効化。旧Batchは残存requestを再実行。各段階でCommands/relationshipをflush。 | Decide直後は表どおり。Demand=0・worker0、およびanchor/issuer消失は既存Maintainのclose owner経由で終端化。正のDemandを維持する種別は同一requestを保持。worker付き需要消失/duplicateは有効な既存workerを保持。 |
| R05-T4 `reconcile_equal_spec_does_not_change_components_test` | 同じspecを2回適用し、間にchange tickを進める。 | 関連componentのChangedなし、spawn/despawnなし。priority/manual/Target保持。 |

SoulSpaは既存delivery/cancel testとは別にproducerの重複・slot・需要消失fixtureを追加する。T1の順序反転はQueryの偶然の順番に依存させずpure selectorの入力列で保証する。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p hw_logistics transport_request::producer
python3 scripts/dev.py cargo -- test -p hw_logistics transport_request::state_machine
python3 scripts/dev.py cargo -- test -p hw_logistics transport_request::arbitration
python3 scripts/dev.py cargo -- test -p hw_logistics transport_request::lifecycle
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 soul_spa_construction
```

test filterは実装時に実際の出力件数を確認し、0件実行を成功根拠にしない。
既存filterに入らない新testは正確なmodule/test名をここへ追加する。

### 完了gate

```bash
python3 scripts/dev.py docs --write
python3 scripts/dev.py docs --check
python3 scripts/check_help_impact.py
python3 scripts/dev.py check
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
python3 scripts/dev.py verify
python3 scripts/dev.py validation check
git diff --check
```

`verify`がworkspace testを含むため、成功後に理由なく同じ全testを繰り返さない。
上のコマンドは実装完了時の要件であり、計画作成時の実行済み記録ではない。
Help No impactのローカルoverrideはSkillに従い具体的理由を設定し、将来の差分を事前承認しない。
Help更新時はmanifest/provider/coverageと生成したexact snapshotを同じbatchに含め、全差分を読む。

### 実画面・性能確認

ECS contract testを必須とし、画面変更のない内部整理にGPU測定を追加しない。canonical選択変更で実運搬の差を確認する場合は固定scenarioを追加し、性能を主張する場合のみ別計測を計画する。

実機確認を実施する場合は[Native Acceptance Skill](../../../../.cursor/skills/hell-workers-run-native-acceptance/SKILL.md)を読み直す。
primary `dev.py validation plan --spec ... -- <既存helperのplan>`へ登録し、返されたno-prompt kitty launcherだけを使う。
fixtureが対象を覆わなければ先に専用caseと独立verifierを用意する。汎用smokeの成功を個別要件の証明にしない。
Capture/Memoryを含むrecipeは逐次実行し、read-only observer・source/asset/binary一致・timeoutを検証する。
通常の修正feedbackでは同じdev cacheを再利用し、headless・feedback・正式受入・性能結果を区別する。

### 検証データ管理（各バッチの開始前・報告前に更新）

正本は[検証データ管理](../../../development-infra/validation-storage-workflow.md)。実行開始/再開時に必ず読み直す。
通常quality gateは既存primary cacheで行い、専用native/performance出力を作る場合にretain/plan/executeを登録する。

| 項目 | 完了時点 |
| --- | --- |
| batch / owner | 専用native batch不要。主担当Codexがprimaryで実装・通常品質gateを実行 |
| workspace / subject | primaryの作業差分。基準HEADはメタ情報の調査基準を参照 |
| artifact / 開始・終了bytes | 本計画専用のartifact・binary copy・worktreeなし |
| 採用成果と最終結果 | codeと関連仕様はprimary。本書§9に最終検証結果を記録 |
| 削除path / 容量差 | 本計画固有の削除なし、専用出力0 bytes |
| 継続保持 | primary通常Cargo cacheは継続開発用。R13/R14用candidateは各計画・primary保持台帳に用途を記録 |
| 整理状態 | 本計画の受入を完了。未終了の別計画を理由に専用jobを保持しない |

成功は元依存が残る間に独立verifyしてsealする。失敗/中断も理由・未検証範囲をsealし、不要jobを整理してfinalize/checkを通す。
job終了とcandidate/cache解放は分ける。review-activeの無応答・中間passを終了と解釈しない。
保持にはowner・consumer・bytes・次作業・release_whenが必要で、親track未完だけを保持理由にしない。
全jobのcapsule保存、一律日数/容量上限を新設しない。primaryの通常Cargo cacheは別の保守寿命を持つ。

## 8. ロールバック方針

producer群ごとに旧callerへ戻せる差分にする。canonical規則の挙動変更とspawn共通化を分離し、worker保護を消さない。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 現在地

- 2026-09-19完了。本番TransportRequestPluginの需要消失→Maintain→再開matrix、inactive Mixer・旧Batch、Actor後のworker/owner消失が成功。SoulSpa専用3件で重複・inflight差引・TotalSlots・需要消失・ownerなしearly returnを確認。
- `python3 scripts/dev.py verify`が成功。通常／profilingのworkspace test、通常Clippy警告0、feature別compile、Python、Help、文書・storage gateを確認した。
- Helpは全体差分でUpdate required。Move失敗時の保持、タスク表示、建設工程の3 entryとexact snapshotを更新し、実際のplayer-visible経路と差分をレビュー済み。
- 本計画単体の専用native job／binary copy／worktreeは作成していない。通常primary cacheは継続開発用に保持する。R13/R14用candidateと未終了native確認は各計画が所有し、本計画の完了と分離する。

### 次のAIが最初にやること

現行の関連仕様と実装を参照する。本書の設計案・着手手順を未実装の作業として再実行しない。
新しい変更ではその差分に応じた回帰、Help判断、storage確認を行う。

### 最終確認ログ

| 検査 | 結果 |
| --- | --- |
| focused tests / 新規回帰 | 本番TransportRequestPluginの需要消失→Maintain→再開matrix、inactive Mixer・旧Batch、Actor後のworker/owner消失が成功。SoulSpa専用3件で重複・inflight差引・TotalSlots・需要消失・ownerなしearly returnを確認。 |
| check / rust-analyzer | workspace compile成功。workspace rust-analyzer診断はMCPのnull応答により取得不可で、compileとClippyで代替確認 |
| Clippy / verify | 通常Clippy警告0、verify成功 |
| Help判断 / 仕様同期 | Update requiredとして3 entry・exact snapshot・関連仕様を同期 |
| native / GPU / 性能 | 本計画では不要。性能改善の定量主張なし |
| validation storage | primary `validation check`成功。専用出力なし、他計画の用途を持つcacheを保持 |
| 未解決エラー | なし。上記MCPの取得制約は残る |

### Definition of Done

- [x] 実装と必要な回帰、仕様・Help同期を完了
- [x] workspace compile・Clippy警告0・verify・storage checkが成功
- [x] 本計画固有の不要job／専用環境なし。関連計画の現用途を持つcacheを保持
- [x] primaryのarchiveへ移動し、計画・提案索引を再生成

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-18 | Codex | R05の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: 需要0/duplicateとanchor/issuer消失を分離し、終端所有・schedule/flush・lifecycle回帰を補足。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: SemanticRequestSpecとStockpileRequestSpecのslot入力を比較し、workers=2のR05-T2と全kindの需要0表を現行testへ固定する。 実装は未着手。 |
| 2026-09-19 | Codex | 実装・回帰・Help／仕様同期と品質gateを完了しarchive。 |
