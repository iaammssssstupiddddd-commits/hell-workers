# R07: タスク参照entityとowner取消判定の共有 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r07-task-entity-references-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R07 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P2 / 中 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 実装・必要な回帰・仕様／Help同期・品質gateを完了。専用native受入は不要。設計経緯としてarchiveし、現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: payloadのsecondary参照やWorkingOnの有無により取消対象workerの抽出が経路ごとに異なる状態を減らす。
- 到達したい状態・成功指標: 取消/割当拒否に必要なentity roleを網羅し、siteへのchain搬送も適切に閉じ、refund・予約解放・通知を一度だけ行う。

## 2. スコープ

### 対象（In Scope）

- AssignedTaskの参照判定と必要ならrole付き列挙
- 床/壁取消、pending owner割当拒否、external terminalとの整合

### 非対象（Out of Scope）

- exclusive World terminalとCommands systemの全面統合
- payload/Reflect保存名変更、全取消workflowの再設計

### 主な変更対象（現行ファイル）

- [crates/hw_jobs/src/tasks/mod.rs](../../../../crates/hw_jobs/src/tasks/mod.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_assignment_apply.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_assignment_apply.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/external_task_terminal.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/external_task_terminal.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/external_task_terminal/preflight.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/external_task_terminal/preflight.rs)
- [crates/bevy_app/src/systems/jobs/floor_construction/cancellation.rs](../../../../crates/bevy_app/src/systems/jobs/floor_construction/cancellation.rs)
- [crates/bevy_app/src/systems/jobs/wall_construction/cancellation.rs](../../../../crates/bevy_app/src/systems/jobs/wall_construction/cancellation.rs)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

hw_jobs::references_entityは全payload edgeを列挙するが、assignment applyと床/壁取消には独自判定がある。WorkingOn(item)と搬入先siteのようなsecondary参照が取消時に抽出される条件が揃わない。後段消失処理で回復する可能性は別に評価する。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/bevy_app/src/systems/jobs/floor_construction/cancellation.rs](../../../../crates/bevy_app/src/systems/jobs/floor_construction/cancellation.rs)
- [crates/bevy_app/src/systems/jobs/wall_construction/cancellation.rs](../../../../crates/bevy_app/src/systems/jobs/wall_construction/cancellation.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/external_task_terminal.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/external_task_terminal.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- source/destination/site/tile/order/task_entity/carrierを役割付き表で整理し、取消対象として必要なroleだけを選ぶ。MovePlant.task_entityとbuilding、Deconstruct.orderとtargetは同一視しない。
- 床/壁取消で既存references_entityが意味を満たす部分から採用し、その後必要な重複に限ってhw_jobsへrole付き列挙を追加する。
- payload edge列挙とtask shell全体の参照判定を区別する。external terminal/owner preflightはpayloadに加えてActiveTaskIdentity.assignment_entity・current_target_entity・WorkingOnの参照を保持する。shell参照を初回pending-owner検査へ一律追加せず、MovePlant.task_entityの除外とDeconstructの専用受理分岐を維持する。
- 現行のemit_abandoned=true、refund、owner一致、exact identityを保持する。external terminalのpreflight契約を参考にするが、通知policyやexclusive World境界を勝手に統合しない。

### 具体設計と変更境界

| 参照の種類 | 所有者・採用する判定 |
| --- | --- |
| payload | hw_jobsの既存 `AssignedTask::references_entity` を入口として維持。重複が残る場合だけ `visit_entity_references(role, entity)`（案）から導出する。 |
| task shell | Soul側の `task_shell_references_entity`（案）でpayload OR identity.assignment_entity OR identity.current_target_entity OR WorkingOnを判定。external terminalのpreflightと適用時で共有する。 |
| 床/壁取消 | related_targetsに対するpayload全参照＋WorkingOnでworkerを抽出。shellのroot assignmentを無条件に追加しない。 |
| 初回pending-owner検査 | physical roleを選択。MovePlant.task_entityをbuilding扱いせず、Deconstructの専用受理検証を維持する。 |

床/壁取消はrelated_targets構築→worker抽出→既存 `release_matching_workers` →refund→owner削除の順を保持する。
予約/relationshipの解除、abandoned通知、refundは既存ownerへ残す。pure参照APIへCommandsや取消policyを持ち込まない。
external terminalの全員preflight→一括適用の境界と、床/壁のCommands経由cleanupは独立のままにする。

### 実装単位と移行順

1. **R07-A / M1:** secondary destinationとidentity-only参照の取消fixtureを追加し、role別に取消する/しないを固定。
2. **R07-B / M2:** 床/壁取消を既存references_entityへ移す。通知/refundの順序は変更しない。
3. **R07-C / M3:** external terminalのshell判定を共有し、なお残るrole列挙だけhw_jobsへ集約する。単純な参照統一で足りればvisitorは追加しない。

### 依存関係と着手順

pure参照整理は独立可能。R02採用後はchain取消の統合ケースを再確認するがR02を先行必須にはしない。R03とは床/壁ownerの前提、R09とは解除contextの変更を逐次整合する。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 参照roleと取消回帰を固定

- 変更内容:
  - 全taskの参照表を作り、WorkingOn欠落とsecondary destinationの取消ケースを構成する。
  - 現在のabandoned通知、refund、reservationの期待値を記録する。
  - payload/WorkingOnに対象がなく、identityだけがroot assignmentまたはcurrent targetを参照するowner cleanupを追加する。
- 変更ファイル: R07-A: hw_jobs/src/tasks/mod.rs の参照test、root construction_cancellation.rs test、Soul external_task_terminalのtest。
- 完了条件:
  - [x] 抽出漏れと意図的なrole除外を区別できる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: 床/壁取消を既存共有判定へ移行

- 変更内容:
  - 既存references_entityを使えるcallerを更新し、payload primary/WorkingOnだけの判定を整理する。
  - worker終了とowner取消を同じframeで検証する。
- 変更ファイル: R07-B: root floor_construction/cancellation.rs、wall_construction/cancellation.rs、共通construction_cancellation.rs。
- 完了条件:
  - [x] chain搬入先取消で必要なworkerが抽出され、二重cleanupがない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: 必要なrole列挙を共有

- 変更内容:
  - 重複が残る場合だけhw_jobsのpure edge列挙からreferences_entity/pending owner判定を導出する。
  - task_assignment_applyとexternal terminalはentity参照判定を共有し、各入口固有の受理/拒否条件を保持する。初回phase検証と実行中identity/batch検証を混同せず、過剰取消がないことを確認する。
  - external terminalのpreflightと適用時再検査の双方でpayload/identity/WorkingOnの参照集合を維持する。共有payload visitorへの置換でshell参照を削らない。
- 変更ファイル: R07-C: Soul external_task_terminal.rs / preflight.rs、task_assignment_apply.rs。必要時のみ hw_jobs/src/tasks 配下にrole visitorを追加。
- 完了条件:
  - [x] 全variantの必要roleを網羅し、保存型や通知policyを変えていない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 取消対象・通知・返還のplayer-visible意味を照合し、変化した場合はHelpと建築仕様を更新する。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/tasks.md](../../../../docs/tasks.md)
  - [docs/invariants.md](../../../../docs/invariants.md)
  - [docs/building.md](../../../../docs/building.md)
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
| 全edgeを同じroleとして過剰取消する | role表とMovePlant/Deconstruct専用例で固定。 |
| terminal統合でabandonedやrefundが変わる | 既存通知policyとexact-onceを維持し統合範囲を限定。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| source/destination/site/tile/order/carrier別 | 必要roleを漏らさず、無関係roleを過剰取消しない。 |
| chain搬送中の床/壁取消 | WorkingOnがitemでも搬入先取消に追随。 |
| WorkingOn欠落・owner消失・pending解体 | 安全に抽出/拒否。 |
| payload/WorkingOnに対象がなくidentityだけが参照 | root assignmentまたはcurrent targetによるowner cleanup対象を漏らさず、preflightと適用時の再検査が一致。 |
| 取消再実行・完了競合 | refund/解放/abandoned通知は契約どおり一度。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R07-T1 `construction_cancellation_releases_secondary_destination_test` | 既存construction cancellation AppへHaul(item, stockpile=site)とWorkingOn(item)を置き、siteを取消。 | task/delivery解除、ReleaseSourceとabandoned各1、既存refund量を維持。WorkingOnなしでも搬入先参照から抽出。 |
| R07-T2 `external_terminal_accepts_identity_only_reference_test` | external terminalのtest_worldでpayload/WorkingOnにないownerをidentityだけへ設定。 | preflight抽出と適用時の再検査が同じ対象を採用する。 |
| R07-T3 `external_terminal_rejects_changed_identity_atomically_test` | T2のpreflight後、batch内1 workerのidentityを変更して適用。 | batch全員のtask/予約/通知が適用前と同じ。部分解除なし。 |
| R07-T4 `cancellation_roles_do_not_alias_orders_test` | MovePlant.task_entityとbuilding、Deconstruct.orderとtargetを別entityにし、取消を2回試す。 | 対象roleだけ取消、補助entityの混同なし、refund/通知/解放の二重実行なし。 |

T2のassignment-only例はcurrent targetとWorkingOnの一致を維持する。current-target-only例はWorkingOnなし、payloadとidentity.current_work_typeを一致させ、別のIdentityMismatch guardへ入らないfixtureにする。各例で適用結果Appliedまでassertする。
T4の期待値は各入口のrole表に従う。全roleを一律に「参照していれば取消」とするtestにしない。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p hw_jobs
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 construction_cancellation_
python3 scripts/dev.py cargo -- test -p hw_soul_ai external_task_terminal::tests
python3 scripts/dev.py cargo -- test -p hw_soul_ai task_assignment_apply::tests
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

取消の正しさはECS transaction testを必須とする。playerの取消入力まで変更する場合だけ、その入力を含むnative専用caseを追加する。

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

callerの参照判定変更と新edge APIを分離する。role抽象化を戻しても、必要secondary参照の回帰修正は保持する。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 現在地

- 2026-09-19完了。全AssignedTaskの参照roleを共有。MovePlantのorder/building分離取消、残るworkerの保護、再取消時の通知不増、およびDeconstructの再取消を確認。
- `python3 scripts/dev.py verify`が成功。通常／profilingのworkspace test、通常Clippy警告0、feature別compile、Python、Help、文書・storage gateを確認した。
- Helpは全体差分でUpdate required。Move失敗時の保持、タスク表示、建設工程の3 entryとexact snapshotを更新し、実際のplayer-visible経路と差分をレビュー済み。
- 本計画単体の専用native job／binary copy／worktreeは作成していない。通常primary cacheは継続開発用に保持する。R13/R14用candidateと未終了native確認は各計画が所有し、本計画の完了と分離する。

### 次のAIが最初にやること

現行の関連仕様と実装を参照する。本書の設計案・着手手順を未実装の作業として再実行しない。
新しい変更ではその差分に応じた回帰、Help判断、storage確認を行う。

### 最終確認ログ

| 検査 | 結果 |
| --- | --- |
| focused tests / 新規回帰 | 全AssignedTaskの参照roleを共有。MovePlantのorder/building分離取消、残るworkerの保護、再取消時の通知不増、およびDeconstructの再取消を確認。 |
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
| 2026-09-18 | Codex | R07の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: payload列挙とidentity/WorkingOnを含むshell参照を分離し、preflightの対象漏れ防止を補足。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: HaulのWorkingOn=item・搬入先=siteを持つR07-T1と、identity-onlyのR07-T2を既存Appに追加し、取消入口ごとの抽出範囲を固定する。 実装は未着手。 |
| 2026-09-19 | Codex | 実装・回帰・Help／仕様同期と品質gateを完了しarchive。 |
