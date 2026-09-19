# R02: 採集後チェーンの受理・予約・segment適用の統一 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r02-gather-haul-chain-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R02 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P1 / 大 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 実装・必要な回帰・仕様／Help同期・品質gateを完了。専用native受入は不要。設計経緯としてarchiveし、現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: 採集後の連続搬送が通常割当と異なるsource claim・予約・搬入先受理を使う入口差を解消する。
- 到達したい状態・成功指標: 同frameのsource競合を一意に処理し、搬入先policyを開始時と荷下ろし時に検証し、chain全体のassignment identityと最終segmentの完了通知を維持する。

## 2. スコープ

### 対象（In Scope）

- Gather Doneの候補選定とsegment遷移
- source/receiver/mixer予約、DeliveringTo、同frame claimの共有処理
- 通常割当との競合・拒否・中断・最終完了のECSテスト

### 非対象（Out of Scope）

- FamiliarとSoul間の新しいCargo依存
- 全assignmentを単一Messageへ統合すること
- 全物流producerの同時移行、AI優先度・経路探索の再設計

### 主な変更対象（現行ファイル）

- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/chain.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/chain.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/gather.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/gather.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_assignment_apply.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_assignment_apply.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/execution.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/execution.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution_system.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution_system.rs)（同cycle共有のclaimを各worker contextへ渡す入口）
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/stockpile_policy.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/stockpile_policy.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul/dropping.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul/dropping.rs)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

chainはVisibility/StoredIn/LoadedInを確認する一方、予約・TaskWorkers・DeliveringTo・手動固定sourceを候補条件として読まない。gatherはpayloadとWorkingOnを直接更新する。Stockpile fallbackもacceptance/targetを評価しない。荷下ろしでlive再検証されるため、禁止資材が格納されるとは断定しない。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/hw_soul_ai/src/soul_ai/execute/task_execution_system/tests/completion.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution_system/tests/completion.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution_system/tests/stockpile_policy.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution_system/tests/stockpile_policy.rs)
- [crates/bevy_app/src/systems/familiar_ai/transport_assignment_tests.rs](../../../../crates/bevy_app/src/systems/familiar_ai/transport_assignment_tests.rs)
- [crates/hw_jobs/src/lifecycle/tests/mod.rs](../../../../crates/hw_jobs/src/lifecycle/tests/mod.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- 初回TaskAssignmentRequestはbusy worker拒否/identity生成を持つため、そのままchainを流さない。SegmentTransition用の前提検査と適用入口をSoul側に持ち、claim/予約/deliveryの共通部分だけを通常applyと共有する。
- 共有pure判定の候補ownerはhw_jobs（task意味）とhw_logistics（受理/予約）。実際のCargo graphを確認して配置し、hw_jobs→hw_logisticsやAI相互依存は追加しない。
- source・receiver・予約差分を副作用なしで検証し、受理できたsegmentだけsource claimと資材別receiver容量を一体で確定する。deferred Commandsだけに排他を任せず、後続Soulはlive予約と同cycle確定差分の双方を見る。通常割当で確保済みの容量を二重計上しない。
- 拒否したsegmentのclaim・予約・WorkingOn・DeliveringTo・identityは更新しない。その後の採集完了処理は既存方針へ戻す。cycle共有状態はworkerごとに初期化せず、WorldEpoch/次cycleへ残留させない。
- receiver選択時に既存Stockpile evaluatorを使い、その外側にあるsource/receiverのownership適合とpending解体ownerへの開始拒否も引き継ぐ。直接のpendingだけでなくBelongsTo等のownerを検査する。荷下ろし時のlive再検証を残し、開始後の消失・拒否は共通terminalへ戻す。
- assignment_entityを維持し、既存のActiveTaskIdentity::transition_to(target, work_type)相当でcurrent target・current_work_type・Detached→Attachedを更新する。採集→搬送では元assignmentの完了イベントを途中発火させない。

### 具体設計と変更境界

新規名は案。Soul側の `chain.rs::prepare_gather_haul_segment` は副作用なしで候補を選び、payload・source/receiver・資材・必要予約を返す。`gather.rs::commit_gather_haul_segment` は受理済み結果だけを反映する。

`ChainAdmissionShadow` はsource集合とreceiverごとの総数/資材別増分を持つ。`task_execution_system` のSoul loop直前に空で一度作り、execution context経由で共有し、system終了時に破棄する。live cacheの予約をshadowへ複製しない。通常割当が反映済みである既存scheduleを前提として、判定はlive＋今回の差分を使う。

| 段階 | 実行内容と失敗時の扱い |
| --- | --- |
| 候補抽出 | pin・既存claim・ownership・pending ownerで不適格を除いてから最寄りsourceを選ぶ。Soul巡回順は現状維持し、新しいEntity sortを導入しない。 |
| prepare | sourceとreceiverの両方を検査。Stockpileは既存evaluator、Mixerは資材別残量を使う。拒否はclaimを残さず既存Gather完了処理へ戻す。 |
| commit | 全条件成立後、shadow確定→予約Message→DeliveringTo/WorkingOn→identity transition→payload更新の順。prepare後に失敗し得る検査を残さない。 |
| 次cycle / 中断 | liveへ適用済みの予約を数え、前cycle shadowは使わない。受理後の取消/消失は通常Haulのterminalへ委ねる。 |

通常assignment applyとはdelivery/予約付与等の小さいprimitiveだけを共有する。root assignment生成・busy判定・元assignment完了通知はsegment適用へ持ち込まない。

### 実装単位と移行順

1. **R02-A / M1〜M2:** 競合fixture→空shadow導入→sourceとreceiverの同時claimを既存evaluatorで完結させる。R06完了を待たない。
2. **R02-B / M3:** prepare/commitと通常applyの共通primitiveを整理する。R06採用後のsnapshot利用は別差分にし、R02-Aの受入を再利用できる形にする。

### 依存関係と着手順

必須の先行計画なし。R06の共通入力型が先に完了していれば利用し、未完なら現行logistics evaluatorで限定修正を完結させる。R06/R07の全面完了を待たない。R06のSoul adapter移行とはstockpile_policy.rsを同時編集しない。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 競合・拒否・identityを再現

- 変更内容:
  - 2 Soul同時Doneと通常割当とのsource競合を構成し、現行の予約/relationship差を観測する。
  - 禁止Stockpile、target到達、固定source、Mixer容量、最終完了の期待値をケース表にする。
  - 別sourceによる同receiver容量競合、異種資材、拒否後のsource再利用、owner/pending拒否を構成する。
- 変更ファイル: R02-A: task_execution_system/tests のGather/搬送fixture、chain.rs の候補受理条件。
- 完了条件:
  - [x] 入口差と実行結果を区別した回帰例があり、禁止資材の格納を未確認のまま主張しない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: 受理と同frame claimを揃える

- 変更内容:
  - sourceとreceiverに既存の受理判定を適用し、source claimと資材別receiver容量を同cycle内で一体として確定する。
  - chain不成立は既存の採集完了/次の通常割当に戻し、新たな無期限待ちを作らない。
- 変更ファイル: R02-A: task_execution_system.rs、context/execution.rs、chain.rs、gather.rs。loop共有shadowと原子的な受理を導入。
- 完了条件:
  - [x] 2 Soul/通常割当との競合でsource所有が一意、receiver容量を過剰確保せず、拒否先への新segmentが開始されない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: segment適用の所有者を統一

- 変更内容:
  - 通常applyとsegment遷移で予約/deliveryの共通primitiveを使い、違いは明示入力として残す。
  - 開始・成功・失敗・取消・World replacementでclaim/予約が閉じることを検査する。
- 変更ファイル: R02-B: chain.rs / gather.rs と task_assignment_apply.rs の共通primitive。必要時に hw_logistics のR06値型へ追従。
- 完了条件:
  - [x] identity維持、予約exact-once、最終segmentのみ完了通知が成立する。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 採集後の搬送条件と失敗結果がplayerへどう伝わるか確認し、必要なら採集/物流のHelpを同じbatchで更新する。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/tasks.md](../../../../docs/tasks.md)
  - [docs/logistics.md](../../../../docs/logistics.md)
  - [docs/invariants.md](../../../../docs/invariants.md)
  - [docs/soul_ai.md](../../../../docs/soul_ai.md)
  - [docs/events.md](../../../../docs/events.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功し、rust-analyzerの取得可能な診断にerrorがない。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| 初回割当とchainを混同しidentityが作り直される | segment専用入口を置き既存completion testを継承。 |
| deferred可視性で同sourceへ複数claim | cycle内の即時claimで仲裁し、失敗/epoch切替を検査。 |
| R06との依存循環や二重集計 | R02は既存evaluatorで完結可能とし、型移行は後から一方向に行う。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| 2 Soul同時Gather Done/通常割当競合 | 同じsourceの新規claimが一意。既存Soul巡回順を維持し、Entity sortは追加しない。 |
| 別sourceが残り1枠のStockpile/Mixerを競合 | 資材別receiver容量も同cycleに確保し、受理は1件だけ。通常割当で埋まった枠へchainを開始しない。 |
| 空StockpileへWood/Rockが同cycleに競合 | 確定済みの資材種を後続判定へ反映し、異種の同時搬入を開始しない。 |
| receiver拒否後に別の有効segmentを評価 | 拒否されたsource/receiver claimは残らず、sourceを再利用できる。 |
| 手動固定source・他worker予約済み | chain候補から除外。 |
| source/Stockpileのowner不適合・source/receiverまたはそのownerが解体pending | evaluator外の既存受理条件で拒否し、segmentの予約・relationship・identityを変更しない。 |
| Stockpile拒否/target到達・異種予約 | 未予約分は現policyに従い、拒否条件では開始しない。自己予約済み搬入は通常搬送と同じCommittedInboundとしてacceptance/target変更から保護し、荷下ろし時の物理容量は再検証する。 |
| Mixer満杯・source/receiver消失 | 必要な予約を漏れなく解放し、誤完了しない。 |
| 複数segment完了・途中取消 | root identity維持、重複通知・予約二重解放なし。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R02-T1 `gather_chain_claims_capacity_once_test` | Detached identity・WorkingOnなしのGather Done Soulを2体、itemを2個、残り1枠のStockpileを用意して1 update。 | Haul / DeliveringTo / ReserveSourceが各1件。勝者のassignment_entity不変、current target/work type更新、勝者の途中完了通知0。敗者はGatherを正常完了し、敗者の完了通知は1件。 |
| R02-T2 `gather_chain_mixer_capacity_is_shared_test` | Mine/Rockの2候補とRock在庫がMUD_MIXER_CAPACITY−1のMixer。MixerをanchorとするDeliverToMixerSolid/RockのTransportRequest、remaining()>0のTransportDemand、Pending stateを置き、WheelbarrowLeaseなし。 | 受理1件。同じsource競合ではsource claimも1件。 |
| R02-T3 `gather_chain_rejection_leaves_no_claim_test` | pin・owner不適合・pending owner・受入停止を各1条件だけ変え、prepare単体で拒否を検査。 | 予約/relationship/identity/shadow変更0。有効条件に戻したprepareが同sourceをclaimできる。system全体では拒否後のGather完了とidentity除去を別assert。 |
| R02-T4 `gather_chain_counts_live_reservations_once_test` | 容量2にlive予約1＋新規候補1、容量1にlive予約1＋新規候補1の2例。予約適用systemを通す。旧segment中断・解放後の次cycleにも同sourceを評価。 | 容量2では新規1件を受理、満杯例では0件。liveの二重計上による過剰拒否なし、次cycleに古いsource claimが残らない。 |
| R02-T5 `gather_chain_resource_and_terminal_contract_test` | 空StockpileにWood/Rockが競合。受理後にpolicy変更/取消を行う。 | 同cycleの異種受理なし。自己予約はCommittedInboundとして扱い、terminal解放は一度だけ。 |

T1/T2の勝者entity自体は固定しない。既存巡回順を維持しながら「同時受理が1件」という排他契約を検査する。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p hw_soul_ai task_execution
python3 scripts/dev.py cargo -- test -p hw_soul_ai task_assignment_apply
python3 scripts/dev.py cargo -- test -p hw_familiar_ai task_management
python3 scripts/dev.py cargo -- test -p hw_logistics stockpile
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

予約・同frame競合はheadless ECSテストを主判定とする。通常プレイで採集→搬送→荷下ろしを補助確認する場合も、専用caseとnative Skillのlauncherを使う。性能改善を主張しないためCapture/Memory比較をこの修正の既定条件にはしない。

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

候補受理/claimの修正とapply primitiveの抽出を分離する。共有化を戻す場合も拒否・競合回帰を維持し、旧予約欠落を再導入しない。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 現在地

- 2026-09-19完了。7件のGatherチェーン回帰と本番登録順の回帰が成功。受理後の取消→次cycle再取得、policy変更後の実pickup→搬入、元assignmentの完了通知1件も確認。
- `python3 scripts/dev.py verify`が成功。通常／profilingのworkspace test、通常Clippy警告0、feature別compile、Python、Help、文書・storage gateを確認した。
- Helpは全体差分でUpdate required。Move失敗時の保持、タスク表示、建設工程の3 entryとexact snapshotを更新し、実際のplayer-visible経路と差分をレビュー済み。
- 本計画単体の専用native job／binary copy／worktreeは作成していない。通常primary cacheは継続開発用に保持する。R13/R14用candidateと未終了native確認は各計画が所有し、本計画の完了と分離する。

### 次のAIが最初にやること

現行の関連仕様と実装を参照する。本書の設計案・着手手順を未実装の作業として再実行しない。
新しい変更ではその差分に応じた回帰、Help判断、storage確認を行う。

### 最終確認ログ

| 検査 | 結果 |
| --- | --- |
| focused tests / 新規回帰 | 7件のGatherチェーン回帰と本番登録順の回帰が成功。受理後の取消→次cycle再取得、policy変更後の実pickup→搬入、元assignmentの完了通知1件も確認。 |
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
| 2026-09-18 | Codex | R02の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: source/receiver容量の一体claim、owner/pending条件、同cycle回帰と共有contextの対象を補足。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: R02-T1/T2で2 Soul・2 source・残り1枠を再現し、通常割当flush→task実行→予約適用の順を既存Appで固定する。 実装は未着手。 |
| 2026-09-19 | Codex | 実装・回帰・Help／仕様同期と品質gateを完了しarchive。 |
