# R06: Stockpile受理判定のsnapshotと予約入力の集約 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r06-stockpile-inbound-snapshot-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R06 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P2 / 中 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 実装・必要な回帰・仕様／Help同期・品質gateを完了。専用native受入は不要。設計経緯としてarchiveし、現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: 共通evaluatorへ渡す内容・予約数の構築がproducer/arbitration/AIへ分散している状態を整理する。
- 到達したい状態・成功指標: 同じ状態とphaseでは全adapterが同じpolicy入力を作り、NewInboundとCommittedInboundの自己予約・policy変更時の扱いを維持する。

## 2. スコープ

### 対象（In Scope）

- hw_logisticsの内容/資源別予約snapshotと入力constructor
- producer/arbitration/Familiar/Soul adapterの段階移行

### 非対象（Out of Scope）

- evaluatorのpolicy変更、長寿命cache、grant/unloadの再検証削除
- source claimの同frame排他（R02の責務）

### 主な変更対象（現行ファイル）

- [crates/hw_logistics/src/stockpile_policy.rs](../../../../crates/hw_logistics/src/stockpile_policy.rs)
- [crates/hw_logistics/src/lib.rs](../../../../crates/hw_logistics/src/lib.rs)
- [crates/hw_logistics/src/transport_request/producer/task_area.rs](../../../../crates/hw_logistics/src/transport_request/producer/task_area.rs)
- [crates/hw_logistics/src/transport_request/producer/consolidation.rs](../../../../crates/hw_logistics/src/transport_request/producer/consolidation.rs)
- [crates/hw_logistics/src/transport_request/arbitration/candidates.rs](../../../../crates/hw_logistics/src/transport_request/arbitration/candidates.rs)
- [crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/capacity_helpers.rs](../../../../crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/capacity_helpers.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/stockpile_policy.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/stockpile_policy.rs)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

task_area/consolidationの独自snapshot、arbitrationのlive集計、Familiarのcapacity input、Soulのowned予約正規化が並立する。pure evaluator自体は既に共通化済み。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/hw_logistics/src/stockpile_policy.rs](../../../../crates/hw_logistics/src/stockpile_policy.rs)
- [crates/hw_logistics/src/transport_request/arbitration/system.rs](../../../../crates/hw_logistics/src/transport_request/arbitration/system.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/stockpile_policy.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/stockpile_policy.rs)
- [crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/resolver.rs](../../../../crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/resolver.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- hw_logisticsにdomain値型とpure constructorを置き、Query shape/取得時点はcallerに残す。内容、matching/other/unknown予約、cycle shadow、自己所有予約を区別する。
- NewInboundとCommittedInboundを別constructorまたは明示型で表し、予約済み搬入のgrandfatheringを維持する。既存evaluatorを基準に、旧/new adapterの同じ状態での結果を比較する。
- ResourceItem欠落も物理予約として数える。特殊storageの専用規則とowner照合を維持する。R02のsource claimをこのsnapshotの責務へ混ぜない。

### 具体設計と変更境界

`hw_logistics::stockpile_policy` に次の値型を置く案とし、既存evaluatorの入力へ変換する。Queryとphaseの実行時点は移さない。

| 型（案） | fieldと集計規則 |
| --- | --- |
| StockpileContentsSnapshot | policy / capacity / stored_amount / stored_resource。実在庫のみを表す。 |
| InboundReservationSnapshot | incoming_reserved / incoming_other_resource / owned_reservation / cycle_reserved / cycle_other_resource。durable予約と同cycle増分を区別する。 |
| Query adapter | IncomingDeliveriesの全件をtotalへ数え、other=total−known_matchingとする。ResourceItem不明はotherにも含める。 |
| owned算出 | 評価対象item集合と**そのdestination**のIncomingDeliveriesの積集合。別destinationや単に同資源の予約を自己予約にしない。 |

以下は既存evaluatorの容量計算を固定する式で、新しい評価algorithmではない。差は全て飽和減算とする。
`P=capacity−stored`、`T=clamped_target−stored`、`R=incoming+cycle` と置く。

- NewInbound: acceptance・stored/予約資源の一致を通した後、許容量は `min(requested, P−R, T−R)`。
- CommittedInbound: stored資源と物理容量は検査し、許容量は `min(requested, P−((incoming−owned)+cycle))`。acceptance/target変更だけで既存予約を拒否しない。
- mixed batch: `K=min(owned, requested)` を先にCommittedInboundとして評価。実際の許可分をstoredへ足し、incomingからKを引いたsnapshotで残りをNewInbound評価する。各phaseの既存拒否理由を保持する。

snapshotは判定値のみで、予約を増減しない。Tank/Bucket等の特殊storage、owner照合、grant直前/荷下ろし時のlive再検証はcallerに残す。

### 実装単位と移行順

1. **R06-A / M1:** 集計とmixed batchの固定fixtureを作り、現在のadapter結果を記録。
2. **R06-B / M2:** 値型とconstructorを追加し、logisticsのproducer/arbitrationから移行。
3. **R06-C / M3:** Familiar validator→Soul実行の順に移行。R02-Aは独立完了可能とし、R02-Bが後から共有型を利用する。

### 依存関係と着手順

R02に依存しない。推奨はR02限定修正→R06→R02の共有型への追従整理。ただしR06単独着手も可能。R05とはproducer2ファイル、R02/R09とはSoul context周辺の編集を直列化する。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 集計の意味を固定

- 変更内容:
  - 各adapterのstored/total/matching/other/own/cycle予約を表にし、既存evaluator fixtureを基準として再現する。
  - policy変更とCommittedInboundの期待値を先に固定する。
- 変更ファイル: R06-A: hw_logistics/src/stockpile_policy.rs とSoul stockpile_policy.rs の既存test、Query集計fixture。
- 完了条件:
  - [x] 集計差の意図が明確で、単なる数値一致に潰していない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: logisticsへ値型とconstructorを追加

- 変更内容:
  - snapshotとNewInbound/CommittedInbound入口を導入し、未知資材・異種・自己予約のpure testを作る。
  - 旧adapterとの比較で意味が同じことを確認する。
- 変更ファイル: R06-B: hw_logistics/src/stockpile_policy.rs、producer/task_area.rs・consolidation.rs、arbitration/candidates.rs。
- 完了条件:
  - [x] 共通入力型が既存policyを変更せず表現できる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: 取得時点を維持してcaller移行

- 変更内容:
  - producer/arbitration、Familiar、Soulの順に移し、各phaseのlive Queryとcycle shadowを保持する。
  - 未使用となった独自snapshot/集計helperを除去し、R02が採用済みならchainも同じAPIへ追従する。
- 変更ファイル: R06-C: hw_familiar_ai validator/capacity_helpers.rs、hw_soul_ai task_execution/stockpile_policy.rs と各搬送caller。
- 完了条件:
  - [x] 全既存callerでgrant/unload直前再検証と予約意味が不変。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 入力構造だけの変更で受理条件が不変か実経路で確認する。差があればNo impactとせずpolicy変更として扱う。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/logistics.md](../../../../docs/logistics.md)
  - [docs/invariants.md](../../../../docs/invariants.md)
  - [docs/tasks.md](../../../../docs/tasks.md)
  - [docs/cargo_workspace.md](../../../../docs/cargo_workspace.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功し、rust-analyzerの取得可能な診断にerrorがない。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| 共通化でlive再検証がなくなる | 値型だけを共通化しQuery時点をcallerに残す。 |
| 自己予約を二重加算/無条件に控除する | 所有者付きCommittedInboundとして検証。 |
| R02と循環する実装依存 | 既存callerを基準に完了可能としchain採用は一方向のconsumer。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| 通常/予約済み搬入でpolicy変更 | 新規は拒否、既存予約の許可は現行仕様どおり。 |
| 未知ResourceItem・異種予約・自己予約 | 物理capacityを過大に見積もらない。 |
| mixed batch・重なるYard | 同一セルをcycle内で過剰予約しない。 |
| target変更・owner不一致・特殊storage | phaseごとの再検証と専用規則を維持。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R06-T1 `inbound_snapshot_preserves_capacity_test` | capacity10 / stored2 / target8、同資源incoming2、shadow1、requested5。 | NewInbound許可3、物理remaining8、target remaining6。 |
| R06-T2 `unknown_incoming_reserves_space_test` | T1にResourceItem不明のIncomingDeliveryを1個追加。 | incoming=3、other=1、ReservedResourceMismatchで新規拒否。 |
| R06-T3 `mixed_batch_keeps_only_committed_allowance_test` | capacity10/stored2、storedと搬入は同資源、incoming=owned=2、other=cycle=cycle_other=0、acceptance停止・target0、requested3。 | committed2 / new0。storedを9へ変えてphysical_remaining=1にした場合はcommitted1 / new0。 |
| R06-T4 `owned_reservation_is_destination_scoped_test` | 同じ資源の2 destinationを用意し、評価対象itemの予約先を片方だけにする。 | 他方でowned=0。旧adapterと新snapshotの評価結果が一致。 |

固定表はallowedだけでなくrejection/state/remainingも比較する。長寿命cache、source claim、性能改善の主張をこのtestへ混ぜない。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p hw_logistics stockpile
python3 scripts/dev.py cargo -- test -p hw_logistics transport_request::producer
python3 scripts/dev.py cargo -- test -p hw_logistics transport_request::arbitration
python3 scripts/dev.py cargo -- test -p hw_familiar_ai validator
python3 scripts/dev.py cargo -- test -p hw_soul_ai stockpile_policy
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

振る舞い同値はpure/ECS回帰で判定する。snapshot集約だけを理由に新規native/performance matrixを要求しない。実機確認が必要な挙動差が判明した場合は別caseを追加する。

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

adapter群ごとの移行を戻せる構成にする。pure型追加は利用がなくなれば削除し、旧policy結果を保つ。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 現在地

- 2026-09-19完了。共有snapshot移行と既存回帰が成功。残り物理1枠のcommitted/new分割と、実2destinationのIncomingDeliveries Queryによる自己予約判定を追加して成功。
- `python3 scripts/dev.py verify`が成功。通常／profilingのworkspace test、通常Clippy警告0、feature別compile、Python、Help、文書・storage gateを確認した。
- Helpは全体差分でUpdate required。Move失敗時の保持、タスク表示、建設工程の3 entryとexact snapshotを更新し、実際のplayer-visible経路と差分をレビュー済み。
- 本計画単体の専用native job／binary copy／worktreeは作成していない。通常primary cacheは継続開発用に保持する。R13/R14用candidateと未終了native確認は各計画が所有し、本計画の完了と分離する。

### 次のAIが最初にやること

現行の関連仕様と実装を参照する。本書の設計案・着手手順を未実装の作業として再実行しない。
新しい変更ではその差分に応じた回帰、Help判断、storage確認を行う。

### 最終確認ログ

| 検査 | 結果 |
| --- | --- |
| focused tests / 新規回帰 | 共有snapshot移行と既存回帰が成功。残り物理1枠のcommitted/new分割と、実2destinationのIncomingDeliveries Queryによる自己予約判定を追加して成功。 |
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
| 2026-09-18 | Codex | R06の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: phase別snapshot、自己予約、unknown予約、live再検証とR02からの独立性を確認。追加修正不要。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: R06-T1〜T4のtotal/matching/unknown/owned/shadowを数値表にし、既存adapterの戻り値を固定してから共有値型を追加する。 実装は未着手。 |
| 2026-09-19 | Codex | 実装・回帰・Help／仕様同期と品質gateを完了しarchive。 |
