# R09: 解除専用経路の最小Query context化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r09-task-unassign-context-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R09 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P2 / 小〜中 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 実装・必要な回帰・仕様／Help同期・品質gateを完了。専用native受入は不要。設計経緯としてarchiveし、現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: cleanup/pathfinding/observerが解除だけのために大きなTaskAssignmentQueriesを要求している状態を整理する。
- 到達したい状態・成功指標: 解除専用callerは既存TaskUnassignQueriesを使い、不要な割当writer/storage Queryと未使用re-exportがなく、解除挙動は不変。

## 2. スコープ

### 対象（In Scope）

- 全TaskAssignmentQueries参照の分類と解除caller移行
- 未使用context/trait/re-exportの撤去と初期化smoke

### 非対象（Out of Scope）

- unassignの予約解放・通知policy変更
- Familiar/Soulのcontext全体再設計、性能改善率の主張

### 主な変更対象（現行ファイル）

- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/cleanup.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/cleanup.rs)
- [crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs](../../../../crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs)
- [crates/hw_soul_ai/src/soul_ai/pathfinding/system/worker.rs](../../../../crates/hw_soul_ai/src/soul_ai/pathfinding/system/worker.rs)
- [crates/hw_soul_ai/src/soul_ai/pathfinding/fallback.rs](../../../../crates/hw_soul_ai/src/soul_ai/pathfinding/fallback.rs)
- [crates/bevy_app/src/entities/damned_soul/observers.rs](../../../../crates/bevy_app/src/entities/damned_soul/observers.rs)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

大きなTaskAssignmentQueriesと小さいTaskUnassignQueriesは同じTaskReservationAccessを実装する。cleanup、pathfinding fallback、root疲労/stress observerに解除だけの利用が残る。cleanup/observer専用testの十分なcoverageは未確認。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/hw_soul_ai/src/soul_ai/pathfinding/system/tests.rs](../../../../crates/hw_soul_ai/src/soul_ai/pathfinding/system/tests.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/external_task_terminal.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/external_task_terminal.rs)
- [crates/bevy_app/src/systems/familiar_ai/transport_assignment_tests.rs](../../../../crates/bevy_app/src/systems/familiar_ai/transport_assignment_tests.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- rust-analyzer参照検索を優先し、各callerが解除以外のQuery/MessageWriterを使っていないことを確認して置換する。既存unassign関数は変更しない。
- 全callerが移行した後だけ旧context・未使用trait・互換re-exportを削除する。pub APIの削除漏れはworkspace compileで検出する。
- SystemParamの取得資源が減ることと、Bevyのsystem/observer初期化を確認する。ParamSet aliasやlint抑制を追加しない。

### 具体設計と変更境界

全入口を既存 `TaskUnassignQueries` へ移す。`unassign_task` 自体と通知policyは変更しない。

| 移行する入口 | 変更と維持する処理順 |
| --- | --- |
| cleanup_commanded_souls_system | 解除→OnReleasedFromService→CommandedBy削除。 |
| PathfindingResources.assignment_queries | fieldをunassign_queriesへ改名する案。worker経由のfallback引数まで追従。 |
| process_worker_pathfinding → cleanup_unreachable_destination | Path消去/cooldown→必要時解除の順を維持。 |
| on_stress_breakdown | freeze挿入→解除→CommandedBy削除。 |
| on_exhausted | CommandedBy削除→解除→休息行動更新。 |

上記入口の `emit_abandoned=false` を維持する。全caller移行後、Soul側の `TaskAssignmentQueries`・`TaskAssignmentReadAccess` と専用alias/Deref/re-exportを参照ゼロ確認後に削除する。
**TaskReservationAccessはTaskQueries/TaskUnassignQueriesが使用するため残す。** Familiar側の同名ReadAccessや、実行contextのlive Queryまで削除しない。

### 実装単位と移行順

1. **R09-A / M1:** 上記5箇所の実参照と最小資源を固定し、system/observer実行testを用意。
2. **R09-B / M2:** cleanup→pathfinding受渡し→root observerの順に移す。各入口の通知と処理順を比較する。
3. **R09-C / M3:** Soul context/mod・task_execution/mod・root adapterのre-exportまで撤去。workspace compileで残存consumerを検出する。

### 依存関係と着手順

独立着手可能。R02はcontext/queries.rs、R07は解除callerに接触するため同時編集を避ける。R02の新しい必要Queryを誤って未使用として削除しない。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 参照と必要資源の確認

- 変更内容:
  - 全利用・re-exportを列挙し、解除専用/割当用を分類する。
  - cleanup/疲労/stress observerの既存testを調べ、欠ける初期化/実行smokeを限定追加する。
- 変更ファイル: R09-A: Soul cleanup/pathfinding tests、root damned_soul/observers.rs のobserver test。
- 完了条件:
  - [x] 全consumerと移行対象を特定し、各入口が必要とする資源を説明できる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: 最小contextへcaller移行

- 変更内容:
  - cleanup、pathfinding worker/fallback、root observerをTaskUnassignQueriesへ変更する。
  - 解除の引数/結果/通知とschedule順序は保持する。
- 変更ファイル: R09-B: execute/cleanup.rs、pathfinding/system.rs・system/worker.rs・fallback.rs、root entities/damned_soul/observers.rs。
- 完了条件:
  - [x] 既存回帰とsystem/observer初期化が成功し、不要writer/storage要求がない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: 未使用互換面を撤去

- 変更内容:
  - 参照がなくなった旧context/trait/re-exportだけを削除する。
  - crate READMEと仕様のcontext所有説明を同期する。
- 変更ファイル: R09-C: Soul task_execution/context/{queries,mod}.rs、task_execution/mod.rs、root systems/soul_ai/execute/task_execution/mod.rs と旧型専用trait/alias。
- 完了条件:
  - [x] workspace compile/Clippyが通り、使われない互換層が残らない。
  - [x] 削除候補に移行後の実consumerがないことを参照検索で確認した。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 解除結果・発火条件・通知を変えない内部依存整理として、実経路からNo impactを判定する。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/soul_ai.md](../../../../docs/soul_ai.md)
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
| 共有Queryを早く削除し別consumerを壊す | 定義→全参照→移行→削除の順とworkspace check。 |
| 最小contextでobserver資源が不足する | cleanup/疲労/stressの実行smokeを確認する。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| 到達不能pathfinding | 予約解放とtask終了が従来どおり。 |
| missing Familiar cleanup | 最小資源のAppで起動・解除が成立。 |
| 疲労/stress observer | 実行smokeでQuery conflict/資源不足なし、通知意味不変。 |
| wheelbarrow中断→再割当 | 運搬具claimが残らない。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R09-T1 `cleanup_works_without_assignment_messages_test` | WorldMap、SharedResourceCache、予約/解放Messageを持つ最小App。Familiar componentなしの指揮元にHaul Soulを所属させ、TaskAssignmentRequestを登録せずupdate。 | system初期化成功、task/WorkingOn/CommandedBy解除、予約解放1、service解放1。 |
| R09-T2 `release_observers_use_minimal_context_test` | 同じ最小資源でstress/exhaustedイベントを個別trigger。 | Query conflict/資源不足なし、stress freeze / exhausted休息状態、abandoned通知0。 |
| R09-T3 `pathfinding_fallback_preserves_release_order_test` | 到達不能workerとwheelbarrow使用workerを既存fallback fixtureで中断し再割当。 | Path/cooldownの既存結果、task解除、運搬具claimの残留なし。 |
| R09-T4 参照・公開面検査 | 旧型/aliasをworkspace検索し全targetをcompile。 | Soul旧context参照0、必要なTaskReservationAccessは残存。 |

最小Appは「Queryを減らした」という実装形ではなく、割当用Message資源なしで解除経路が起動・実行できることを検証する。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p hw_soul_ai pathfinding::system::tests
python3 scripts/dev.py cargo -- test -p hw_soul_ai external_task_terminal::tests
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 initial_wheelbarrows_can_be_reassigned_after_transport_interruption
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

既存回帰と必要な初期化smokeで確認する。構造変更だけのために実画面・GPU・性能matrixを追加しない。

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

caller群ごとに戻せる差分にし、context撤去は最後に行う。予約・終端logicの変更を混ぜない。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 現在地

- 2026-09-19完了。TaskUnassignQueriesへ解除callerを移行。割当Messageのない最小AppとObserver回帰が成功。
- `python3 scripts/dev.py verify`が成功。通常／profilingのworkspace test、通常Clippy警告0、feature別compile、Python、Help、文書・storage gateを確認した。
- Helpは全体差分でUpdate required。Move失敗時の保持、タスク表示、建設工程の3 entryとexact snapshotを更新し、実際のplayer-visible経路と差分をレビュー済み。
- 本計画単体の専用native job／binary copy／worktreeは作成していない。通常primary cacheは継続開発用に保持する。R13/R14用candidateと未終了native確認は各計画が所有し、本計画の完了と分離する。

### 次のAIが最初にやること

現行の関連仕様と実装を参照する。本書の設計案・着手手順を未実装の作業として再実行しない。
新しい変更ではその差分に応じた回帰、Help判断、storage確認を行う。

### 最終確認ログ

| 検査 | 結果 |
| --- | --- |
| focused tests / 新規回帰 | TaskUnassignQueriesへ解除callerを移行。割当Messageのない最小AppとObserver回帰が成功。 |
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
| 2026-09-18 | Codex | R09の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: consumer調査→移行→未参照撤去の順序とsystem/observer smokeを確認。追加修正不要。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: TaskAssignmentRequestを登録しない最小AppでR09-T1/T2を作り、各callerの解除前後の通知とCommandedBy操作順を固定する。 実装は未着手。 |
| 2026-09-19 | Codex | 実装・回帰・Help／仕様同期と品質gateを完了しarchive。 |
