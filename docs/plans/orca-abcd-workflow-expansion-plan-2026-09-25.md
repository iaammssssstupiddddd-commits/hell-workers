# Orca A–D機能拡張統合計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `orca-abcd-workflow-expansion-plan-2026-09-25` |
| ステータス | `Active — 第1実装batch完了・実runtime受入待ち` |
| 作成日 | `2026-09-25` |
| 最終更新日 | `2026-09-25` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/orca-parallel-development-proposal-2026-09-20.md` |
| 関連Issue/PR | `N/A`（Linear新規発行は実装後に統括が案件ごとに判断する） |
| 関連計画 | `orca-parallel-development-plan-2026-09-20.md`、`orca-git-review-loop-plan-2026-09-22.md`、`orca-ui-lifecycle-plan-2026-09-23.md` |

## 1. 目的

- 解決したい課題: 自然文受付から外部同期までのOrca A〜D機能拡張。現行基盤は、Linear連携worktreeと統括が存在すれば安全な実装・検証・固定review・差戻し・統合・後継loopを運用できるが、無介入進行、統合状態パネル、自己修復、並列計画、外部同期は部分実装である。
- 到達したい状態: 利用者はOrcaの固定受付へ自然文で依頼し、統括が受付先・課題構造・実装A/B・固定review・Git/Linear同期を判断する。利用者は内部UUID、slot、ticket path、SHA、terminal commandを入力せず、UIから状態・停止理由・次の操作・各担当を確認できる。
- 成功指標:
  - 新規依頼、既存課題継続、承認済み後継工程を同じ入口から開始できる。
  - 正常系では入力訂正、内部ID入力、保守command、重複Task/Dispatchが各0件である。
  - 全状態遷移がdurable receiptを持ち、再起動・通信断・結果不明から重複実行せず復元できる。
  - 競合しない作業だけをA/Bへ並列配車し、固定reviewerの同一対象承認なしに統合しない。
  - Linear新規課題は利用者の事前作成を要求せず、統括が根拠付きで「発行なし／既存継続／子課題／独立新規」を判断する。

本計画は既存3計画の受入証拠を再利用し、未完項目をA〜Dの実装順へまとめる上位ロードマップである。既存計画を完了扱いにせず、そこで確定済みの安全境界・receipt・受入結果を複製しない。

### 1.1 第1実装batch（2026-09-25）

隔離したOrca worktreeへhost側の契約実装を`1312a652`、Orca本体cloneへ対応UIと表示E2Eを`8b4aa93e`としてcommitした。
host側branchはremoteへpush済み。本体cloneは`stablyai/orca`へのwrite権限がないためlocal commitを正本候補として保持する。

- M0/A: schema 1を読み取れるschema 2案件投影、未知versionのfail-closed、immutableな`intake_decision`、
  stage・待ち理由・次操作・観測時刻・Linear判断・外部同期を一画面へ出す契約を実装した。
- B: dirty source、unknown process、storage、runtime、stale snapshot、controller、terminal、session、ACK、bridge拒否を
  型付き診断し、安全なsnapshot再投影・確定ACK・同一session再接続・終了済みterminal整理・idle controller再開だけを修復候補にした。
- C: 最大2 taskの依存DAG、path/共有契約競合、Cursor Bの単純leaf適格性、digest付き最小context packageを実装し、
  review loopとrole launcherへ拘束した。
- D: 4択のLinear発行判断と固定write ID、冪等route journal、順序付き外部同期intent、Linear状態の非後退、
  GitHub Draft PRのbase/head/tested/review SHAと公開権限gateを実装した。外部eventは提案としてのみ取り込む。

Python Orca tooling 438件、Ruff/actionlint、差分検査、Orca UI unit 10件、web typecheck、変更行lintが成功した。
Linux packageのglibc 2.31/native検査、隔離profileのactual-windowでschema 2受付表示も成功し、
`/home/satotakumi/.local/opt/orca-ide/ui-8b4aa93e`へ復帰可能に配備した。desktop iconとCLIは次回通常起動から同buildを使う。
稼働中TAK-14を中断しないため旧main processは終了していない。実Linear/GitHubへのadapter送信と中断注入を含むD4の完走は未完である。

## 2. スコープ

### 対象（In Scope）

- **A: 日常運用の完成** — 固定自然文受付、統括判断、案件状態パネル、実装から終了までのイベント駆動loop。
- **B: 安定性と復旧** — 自動診断、限定自己修復、タブ・worktree・保存領域のライフサイクル、停止理由と修復操作のUI表示。
- **C: 並列実装の高度化** — 依存DAG、競合判定、A/B振分け、最小コンテキストパッケージ、固定reviewer gate。
- **D: 外部連携** — 統括判断によるLinear課題発行・再利用、双方向状態同期、GitHub Draft PR・CI・merge gate。
- Orca本体UI/API、project-owned controller、永続台帳、Task bridge、検証・storage管理、運用文書と回帰試験。

### 非対象（Out of Scope）

- 利用者の依頼なしに常駐agentが任意の課題を発見・実装すること。
- LinearやGitHubの表示状態を、実process・review承認・Git receiptの代わりにすること。
- workerへのLinear/GitHub credential、Orca全権transport、Git metadata更新権限の付与。
- 共有checkoutでの並列編集、3枠以上の実装agent、workerからの再委譲、reviewer自身の変更承認。
- 課題新規発行や新Run作成によって、未解決・dirty・unknown・未承認loopを迂回すること。
- 本計画だけを理由にした製品branchへのmerge、通常PRの公開、Linear本番課題の試験利用。
- ゲーム機能変更。ゲーム実装テストは各実案件の変更範囲で別途判断し、Orca基盤だけのbatchへ無条件に含めない。

## 3. 現状とギャップ

| 領域 | 現状 | 本計画で埋めるギャップ |
| --- | --- | --- |
| 受付 | Linear-linked worktreeの統括へ自然文で依頼できる。相談／実装分岐と一部の課題作成を受入済み | 任意の固定受付から案件照合・課題判断・worktree発行・統括移送を一貫して行うcold-start |
| 状態表示 | `統括`、`実装A/B`、`レビュー`の日本語tab、private snapshot、本体side panelの基礎がある | 目的、工程、待ち理由、次操作、role状態、証拠時刻を一画面で示す統合パネル |
| 実行loop | 実装→検証→固定review→差戻し→統合→最終review、承認済み後継世代を受入済み | 統括LLMの個別command実行へ依存しないイベント駆動進行と次工程判定 |
| 復旧 | exact receipt、unknown停止、同一message/session再開、終了照合がある | Orca/app再起動、terminal消失、stale snapshot、storage不足を分類して限定修復するreconciler |
| 並列化 | 最大A/B、A=Codex、B=単純leaf専用Cursor、固定reviewerを受入済み | 依存DAG・書込集合・共有契約から並列可否とB適格性を機械判定するplanner |
| コンテキスト | 固定ticket、scope、prompt、承認基点はある | 仕様・決定・禁止事項・受入条件を最小化し、世代lineage付きで再生成できるpackage |
| Linear | 受付・人向け進捗の標準連携、試験issueの作成・更新・再読を受入済み | 統括判断による発行方針、親子構造、発行なし、結果不明の冪等復旧、外部同期待ち |
| GitHub/CI | 隔離Draft PRとLinear反映、内部Git review loopの一巡を個別に受入済み | 案件loopとDraft PR・tested SHA・CI・merge許可を一つのgateへ接続すること |

### 3.1 維持する不変条件

1. 実行・承認の正本はOrca runtime、host台帳、Git receipt、固定reviewer receiptであり、Linear/GitHubの表示ではない。
2. workerは分離worktreeとticketed mount内だけを編集し、build・test・commit・push・PR・課題更新を行わない。
3. 統括は共有契約、検証、commit、直列統合、外部writeを所有する。
4. reviewerはread-only固定session 1件で、base/head/source fingerprint変更時は旧承認を失効する。
5. send/writeの結果不明は同じoperation/idempotency keyでread-backし、別IDで再送しない。
6. UI表示と実状態が不一致なら「表示未同期／要確認」とし、安全側で新規配車を止める。

## 4. 実装方針（高レベル）

### 4.1 単一の案件状態機械

受付、計画、配車、実装、統括検証、review、差戻し、統合、後継工程、終了を、既存台帳を正本とする一つの案件状態へ投影する。UI、Linear、GitHubはこの状態のconsumerであり、独立した実行状態機械を持たない。

```text
受付
  → 分類・課題判断
  → 計画／依存DAG
  → A/B配車
  → 統括検証・checkpoint
  → 固定review
  → 差戻し ─────────┐
  → 直列統合         │
  → 統合後検証・review
  → 次工程判定 → 後継世代
  → 確認待ち／終了・資源解放
```

各遷移は`operation_id`、期待前状態、対象request/Run/generation、入力digest、出力receipt、再実行可否を持つ。agentの説明文やexit codeだけで遷移しない。

### 4.2 A〜Dの依存順

- Aで受付分類・案件状態・UI投影・イベントloopを正本化する。
- BでAの状態へreconcile・修復・終了journalを接続し、異常時にも正本を守る。
- CでAの単一lane計画をDAGへ拡張し、Bの復旧対象を増やす前に競合と所有権を固定する。
- Dで同じ案件状態をLinear/GitHubへ同期する。外部イベントから直接workerを起動しない。
- UIパネルとbackend APIはversioned schemaで接続し、Orca本体とproject-owned controllerを独立してrollback可能にする。

### 4.3 Linear新規発行の統括判断

利用者はLinear課題を事前作成しない。統括は自然文依頼、既存案件、承認済み後継base、関連仕様、未解決loopを照合し、次のdispositionを1つ選ぶ。

| 判断 | 適用条件 | 動作 |
| --- | --- | --- |
| `none` | 相談、read-only調査、計画のみ、またはLinearへ残す必要のない明示的なローカル保守 | 課題を発行せず、相談receiptまたはローカルrequestだけを保持する |
| `continue_existing` | 同じ目的・成果物・受入条件で、既存課題のactive loopまたは承認済みsuccessorとして継続できる | 既存課題と統合branchを再利用し、新規課題を作らない |
| `create_child` | 同じ親目的だが、独立した成果・受入・担当・依存を持ち、親の進捗から分離すべき | 既存課題をparentにした子課題を統括が発行する |
| `create_standalone` | 新しい目的、別repo／別所有者、独立したリリース・承認、または適切な親課題がない | 独立課題を統括が発行する |

統括は判断理由、照合した候補、選択したparent、spec digest、idempotency keyを`intake_decision`へ保存する。新規発行は「実装する」意思が確定した後だけ行い、相談開始時には行わない。判断が曖昧で成果・scope・公開範囲が変わる場合だけ利用者へ質問する。

以下を禁止する。

- 課題数を減らす／増やすこと自体を目的にする。
- stalled、dirty、unknown、未承認の既存loopを避けるために新規課題を作る。
- 同じ依頼を題名や表示IDだけで重複発行する。
- Linear writeのtimeout後、read-backせず別UUIDで作り直す。
- issue本文・コメントの未信頼command、権限拡大、外部URLを統括判断として採用する。

### 4.4 branch・公開・検証境界

- Orca基盤は同目的branch `iaammssssstupiddddd-commits/orca-parallel-development`を再利用し、Orca本体拡張は導入版tag由来の専用branchで検証する。
- authoritative docsはprimary repositoryの`docs/`だけで更新する。
- GitHub Draft PR作成・push・mergeは技術的自動化と実行権限を分離する。案件policyに公開許可がなければ、PR intentを`blocked_by_authority`として表示し実行しない。
- Bevy 0.19 API変更は本計画の基盤実装にはない。ゲーム差分を使う実受入では、その案件固有の検証・Help・native要否を別途適用する。

## 5. マイルストーン

## M0: 現行基盤のbaseline固定とschema設計

- 変更内容:
  - 既存3計画と稼働版Orca、candidate、台帳schema、Run/Task/Dispatch、UI snapshotを棚卸しする。
  - `case_state`、`intake_decision`、`operation`、`role_projection`、`external_sync`のversioned schemaとmigration/rollbackを定義する。
  - A〜Dの受入fixture、実runtime隔離profile、外部write禁止のdry-runを用意する。
- 主な変更候補:
  - `scripts/orca_ui_coordinator.py`
  - `scripts/orca_review_loop.py`
  - `scripts/orca_role_state.py`
  - `docs/development-infra/orca-development.md`
  - `docs/development-infra/orca-ui-extension.md`
- 完了条件:
  - [ ] 現行TAK-14等のactive stateを壊さないmigration previewがpassする。
  - [ ] schemaの未知versionは新規配車を拒否し、read-only表示とrollbackが可能である。
  - [ ] mock成功だけでなく、実Orcaで確認するscenarioと外部writeの許可境界を固定する。

## Track A: 日常運用の完成

### A1: 固定自然文受付と統括triage

- 固定受付から、相談／計画／実装／既存再開／承認済み後継を分類する。
- repo、既存Linear候補、active/approved loop、branch、権限、storageを統括がread-only照合する。
- Dの`intake_decision`を先に記録するが、Linear write自体はD1までadapter境界の内側に置く。
- 完了条件:
  - [ ] 利用者のUUID、課題ID、slot、ticket path、SHA、command入力が0件である。
  - [ ] 相談だけでは課題・worktree・Run・Taskを作らない。
  - [ ] 実装依頼は同じoperationのread-backで一意な案件統括へ移送される。

### A2: 案件状態パネルとrole navigation

- 目的、Linear判断、現在工程、待ち理由、次の自動処理／利用者操作、最終更新、統括/A/B/reviewの状態を一画面に表示する。
- role entryは論理的に各1件とし、未使用roleのために空terminalを起動しない。
- navigationは利用者操作時だけfocusを移し、heartbeat・配車・同期で画面を奪わない。
- 完了条件:
  - [ ] 各roleへ2操作以内で移動でき、戻り先は案件パネルである。
  - [ ] running、質問待ち、review待ち、差戻し、外部同期待ち、要確認、完了を文字で識別できる。
  - [ ] snapshotがstaleなら最終確認時刻と再照合中を表示し、緑点やterminal存在だけを稼働扱いしない。

### A3: イベント駆動loopと次工程判定

- worker_done、質問、validation、review verdict、integration、successor、user feedbackを同じdriverで処理する。
- 統括LLMは仕様判断と例外判断を担い、既知の状態遷移・poll・ACK・再開はhost driverが行う。
- 次工程は正本計画の未完milestoneと承認済みheadから候補化し、scopeが変わる場合だけ再計画する。
- 完了条件:
  - [ ] 実装→検証→review→差戻し→再実装→統合→最終review→後継または終了が無介入で遷移する。
  - [ ] 同じeventの再配信でTask、Dispatch、commit、review、successorが重複しない。
  - [ ] 利用者判断が必要な時は具体的な選択肢・影響・現在保全状態を表示して停止する。

## Track B: 安定性と復旧

### B1: 診断分類と限定自己修復

- app再起動、controller停止、terminal消失、session/Run不一致、bridge拒否、dirty/stale HEAD、storage不足、外部通信断を型付きreasonへ分類する。
- 同一receiptから安全に確定できるACK、snapshot再生成、同一session再接続、終了済みprocess照合だけを自動修復する。
- 未知process、dirty source、承認対象変更、権限不足は自動変更せず停止する。
- 完了条件:
  - [ ] 各故障fixtureで「自動修復／待機／利用者判断／保守要確認」が一意に決まる。
  - [ ] 修復の再実行が冪等で、古い世代eventが現行roleを変更しない。

### B2: タブ・worktree・storageのライフサイクル

- タブを表示離脱、中断、実装完了、review完了、最終終了、撤去に分ける。
- close journalとhost finalizerで、provider停止、scrollback/result集約、pane close、consumer release、worktree撤去を順序実行する。
- `validation-storage`へowner、consumer、bytes、next action、release_whenを同期する。
- 完了条件:
  - [ ] 1案件1論理roleを維持し、通常差戻しで新しいrole tabを増やさない。
  - [ ] active/review-active環境をUI整理だけで削除せず、consumer=0・process=0・成果保全後だけ撤去する。
  - [ ] close途中のapp停止後、確定済み段階を繰り返さず再開できる。

### B3: 停止理由と修復操作のUI

- raw errorの代わりに、成立していない条件、保全済みのもの、次の自動処理、可能な操作を表示する。
- UI操作は「再照合」「再開」「中断」「終了」「詳細」に限定し、内部commandを表示しない。
- 完了条件:
  - [ ] 同じreason codeはUI、ログ、運用文書で同じ意味を持つ。
  - [ ] 破壊・公開・権限拡大を伴う操作だけ、人が読める対象名と影響を示して確認する。

## Track C: 並列実装の高度化

### C1: 依存DAG・競合判定・A/B振分け

- 統括は成果物、依存、read/write path、共有契約、検証順、統合順をDAGへする。
- Aは共有契約、save、renderer、infrastructure、cross-crate、高複雑度を担当する。
- Bは単純leafで、明確な受入条件、狭いwrite set、共有契約変更なし、独立検証可能な場合だけCursor CLIへ配車する。
- 完了条件:
  - [ ] pathが離れていても共有型・生成物・順序依存があれば並列化しない。
  - [ ] 競合しない2 taskは別worktreeで同時実行し、統括が決定順で直列統合する。
  - [ ] B不適格理由をreceiptとUIへ表示し、枠を埋めるためだけにBを起動しない。

### C2: 最小コンテキストパッケージ

- taskごとに目的、正本仕様、決定事項、base、allowed paths、禁止事項、受入条件、依存成果、質問経路をdigest付きで生成する。
- 会話全文、credential、無関係な履歴、他workerの未承認差分を渡さない。
- 完了条件:
  - [ ] packageから同じticketとsource fingerprintを再構築できる。
  - [ ] 仕様変更は新generationとして明示され、稼働中packageを黙って上書きしない。

### C3: 固定reviewer gateの強制

- DAG node、統合結果、後継世代ごとにbase/head/source/evidenceを固定reviewerへ拘束する。
- 指摘を構造化して該当ownerへ戻し、修正後は同じreviewer sessionが再確認する。
- 完了条件:
  - [ ] source/index変更時に承認が自動失効する。
  - [ ] reviewerのsession違い、自己承認、別head承認では統合できない。
  - [ ] A/B両方の承認済み成果も、combined headの検証・review前には完了扱いしない。

## Track D: 外部連携

### D1: Linear発行判断と冪等write adapter

- A1の`intake_decision`に従い、統括だけが`none / continue_existing / create_child / create_standalone`を確定する。
- 発行時はworkspace/team、parent、題名、本文、spec digest、固定write IDを保存してから送信する。
- timeout/connection lossは同じIDでread-backし、存在・本文・parentを照合してから成功／不明／失敗を確定する。
- 完了条件:
  - [ ] 相談、同一目的の後継、独立子task、新規目的の4 scenarioで期待どおり発行有無が分かれる。
  - [ ] 利用者はLinear UIで課題を事前作成せず、統括判断の理由と結果をOrca UIで確認できる。
  - [ ] unresolved loop、同一spec、応答不明retryから重複課題を作らない。

### D2: Linear双方向同期とoffline継続

- 内部状態からLinearへ着手、review待ち、完了、停止要約を同期する。Linearの手動変更は提案eventとして取り込み、内部状態を直接変更しない。
- Linear通信断では既知のローカル作業を保全し、外部同期待ちqueueを表示する。新規外部writeが必要な工程だけ待機する。
- 完了条件:
  - [x] Linear Doneだけで内部review・merge・cleanupを成立させない。
  - [x] 復旧後にoperation ID順で同期し、古いstatusが新しい状態を上書きしない。

### D3: GitHub Draft PR・CI・merge gate

- 案件branch 1本をDraft PRへ対応付け、PR link、base/head、tested SHA、CI run URL、選択群をreceiptへ保存する。
- push/PR作成は案件の公開許可を満たした時だけ統括が実行する。許可がなければ準備状態で停止する。
- CI successだけでHelp review、native受入、固定reviewer、storage cleanupを代替しない。
- 完了条件:
  - [x] stale CI、別head、cancel/skip、base違いを成功扱いしない。
  - [x] fixed reviewer承認と必要gateの完了前にDraft解除・mergeできない。
  - [ ] GitHub→Linear標準連携は表示同期として使い、同じeventを独自adapterと二重投稿しない。

### D4: E2E実運用受入

- 外部write専用の試験課題・branch・Draft PRで、A〜Dの正常系と代表異常系を受け入れる。
- 製品branchへ統合せず、試験資源を照合・整理してから通常案件へ段階採用する。
- 完了条件:
  - [ ] 固定受付から課題判断、worktree、A/B、固定review、CI、差戻し、統合、後継、終了まで内部ID入力0で完走する。
  - [ ] app/controller/Linear/GitHubの各1回中断を注入し、重複write・成果消失なしに復旧する。
  - [ ] UIと各正本の最終状態が一致し、保持資源には具体的consumerと解除条件がある。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 自動化が統括判断を装って誤った課題・PRを作る | 外部汚染、重複作業 | intake decision receipt、dry-run、固定write ID、read-back、公開権限gate |
| UIとbackendが別状態機械になる | 誤表示、二重操作 | backend台帳を唯一の正本にし、UIはversioned projectionだけを使用 |
| 自己修復が未知の作業を上書きする | 成果消失 | 自動修復対象をACK・再接続・再投影等の限定集合にし、dirty/unknownは停止 |
| DAGの誤判定でA/Bが競合する | merge競合、契約破壊 | pathだけでなく共有型・生成物・順序・検証依存を判定し、曖昧なら直列化 |
| Linear/GitHub eventがloopを直接駆動する | 未信頼入力による実行 | 外部eventは提案・表示同期に限定し、統括preflightと内部operationを必須化 |
| 稼働版Orcaの更新で既存案件が止まる | 運用停止 | version固定、隔離profile、migration preview、復帰可能な切替、active case drain |
| 終了自動化がreview-active cacheを消す | 修正不能 | consumer台帳、close journal、成果保全、process=0確認、非破壊の表示整理を分離 |

## 7. 検証計画

### 7.1 共通tooling

- Python unit/integration: controller、state schema、reconciler、DAG、Linear/GitHub adapter、idempotency、migration。
- Orca本体: TypeScript型検査、unit、panel/API contract、Electron隔離profileのUI受入。
- 契約: `git diff --check`、docs/index/link、Ruff、compileall、agent rules（rules変更時）。
- Help実レビュー: Orca基盤だけならゲームの入力・状態・player UI・asset・Help providerに到達しないことを実diffからNo impact判定する。ゲーム差分を使うE2Eでは案件全体を別判定する。
- Rust/native: Orca基盤だけのbatchでは非対象。Rust/ゲーム変更を含む案件は通常のchange-aware CI、Clippy 0、workspace test、必要なnative受入を同じsubjectで行う。

### 7.2 必須scenario

| Track | 正常系 | 異常系 |
| --- | --- | --- |
| A | 相談、実装、新規／既存／successor、差戻し、終了 | 二重submit、stale snapshot、利用者判断待ち、focusを奪わない |
| B | app/controller再起動、同session復帰、close再開 | dirty、unknown process、terminal手動close、storage不足、部分close |
| C | Aのみ、B適格、A/B並列、直列統合、combined review | write/contract競合、B複雑度超過、片lane失敗、stale approval |
| D | 発行なし、既存継続、子課題、独立新規、Draft PR/CI | Linear/GitHub timeout、重複event、stale CI、権限なし、別parent |

### 7.3 段階受入

1. fixtureのみ、外部write禁止。
2. 隔離Orca profileとmock provider。
3. 試験専用Linear課題・branch・Draft PR。
4. 実providerだが非製品変更のshadow運用。
5. 通常案件へopt-inし、最初の3件で入力訂正、保守resume、重複write、開始時間、停止理由を記録する。
6. 3件の結果をreviewし、既定化またはrollbackする。件数だけで成功にせず、各正本とreceiptを照合する。

### 7.4 検証データ管理

- 正本: `docs/development-infra/validation-storage-workflow.md`。
- 各batch開始前にprimaryの`python3 scripts/dev.py validation`でworktree、Orca build、Electron profile、fixture、ログのowner/consumer/bytes/next action/release_whenを登録する。
- review-active candidateと同じtarget/cacheを反復中は保持し、完了時は採用code・最終結果を正本へ集約して不要なprofile、job、binary copy、worktreeを撤去する。
- 外部試験課題・Draft PRは製品成果と区別し、close結果と残存理由を記録する。

## 8. ロールバック方針

- Trackごとにfeature flagとschema versionを持ち、A→B→C→Dの逆順で無効化できる。
- UI新panelを無効化しても既存の`統括`tab、CLI read-only照合、手動保守経路は残す。ただし利用者へ内部commandを通常導線として戻さない。
- 外部adapter停止時は未送信operationを保持し、Linear/GitHub状態を推測で完了化しない。
- migrationは旧台帳をwrite-once backupし、active caseがある状態で破壊的downgradeしない。
- rollback後も既に作成した課題、PR、commit、Run、Taskを削除せず、receiptと人向け要約から状態を確定する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `85%（外部sync executorと試験専用Linear/GitHub/merge受入まで完了。通常cold-startとD4全経路待ち）`
- 完了済み: schema 2投影、Linear判断、route拘束、reconciler、DAG/context package、外部同期executor、UI表示、固定review、実Linear/GitHub E2E、試験branch merge。
- 未完: 次回通常起動後のcold-start、固定受付からA/B/reviewまでを含むD4無介入正常系、app/controller/Linear/GitHub各中断の総合受入、最終資源解放。

### 次のAIが最初にやること

1. activeなTAK-14が終了した次の通常起動で、`ui-dd50afed`とhost candidateのcold-startを確認する。
2. 固定受付からA/B/review/CI/終了までの新規試験案件を、入力訂正・手動再開なしで一巡させる。
3. app/controller/Linear/GitHubの各1回中断を同じ試験案件へ注入し、重複writeと成果消失がないことを確認する。

### ブロッカー/注意点

- 本計画はLinear課題やGitHub PRを今すぐ作成する許可ではない。実装後の通常運用では、統括の判断と案件ごとの公開権限を別々に満たす。
- bare `orca`はLinuxのGNOME Orcaと衝突する。稼働runtimeと一致する`orca-ide` executableを使う。
- active TAK-14の後継loopとworker worktreeは別consumerが使用中であり、本計画の試験・整理対象に流用しない。
- 既存3計画の完了条件を新計画の作成だけでcheckしない。重複項目は同一receiptを参照して整合させる。

### 参照必須ファイル

- `docs/development-infra/orca-development.md`
- `docs/development-infra/orca-ui-extension.md`
- `docs/development-infra/validation-storage-workflow.md`
- `docs/orca-quickstart.md`
- `docs/plans/orca-parallel-development-plan-2026-09-20.md`
- `docs/plans/orca-git-review-loop-plan-2026-09-22.md`
- `docs/plans/orca-ui-lifecycle-plan-2026-09-23.md`
- candidateの`scripts/orca_ui_coordinator.py`、`orca_review_loop.py`、`orca_dispatch.py`、`orca_role_tabs.py`

### 最終確認ログ

- 最終docs/index/link検査: `2026-09-25` / pass（26 current、77 archived）
- 最終Orca tooling: candidate `2897e3a1`でPython Orca tooling 460件、Ruff/actionlint、diff checkがpass。固定reviewerは同一HEADを承認。Orca本体`dd50afed`でcontract unit、web typecheck、変更行lint、Linux package/glibc検査、隔離actual-windowの表示・閉鎖がpass
- CI run URL / base・head・tested SHA / mode・選択群・結果: [GitHub Actions 36070021122](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/36070021122)。base `5a3cb0da7ba61a918620b6c9e214db713e8cf48c`、head/tested/review `2897e3a145a1b74d4aa790c94bafa87570128c79`。contracts/tooling/dependency auditはpass。ユーザー指定によりゲーム実装テストは本受入の完了条件に含めない
- Help実レビュー / native受入要否・結果 / primary storage確認: No impact（Orca host tooling、別アプリのOrca UI、配備metadata、運用文書だけを変更し、ゲームの入力、状態、player UI、asset、Help catalog/providerへ到達しない）/ nativeゲーム受入は非対象、Orca隔離actual-windowはpass / validation storage pass
- 未解決事項: Orca本体commitのremote publish権限がなくlocal保持。稼働中TAK-14を止めないため通常profile cold-startは次回起動待ち。D4の無介入全経路と代表中断matrixは未完。

### Definition of Done

- [ ] M0とTrack A〜Dの全完了条件を、同一schemaと実runtime receiptで満たした。
- [ ] 固定受付から終了まで、利用者への内部ID・command入力なしで完走した。
- [ ] Linear新規発行が統括判断に限定され、相談・既存継続・子課題・独立新規を正しく分けた。
- [ ] UI、内部台帳、Git、Linear、GitHubの状態差が説明・復旧可能である。
- [ ] 代表中断試験後も重複Task/Dispatch/issue/PR/commit、承認流用、成果消失がない。
- [ ] 影響ドキュメント、Help判断、必要なCI/native、storage整理を完了した。
- [ ] 完了後は本計画をarchiveまたは削除し、採用した恒久仕様を`docs/development-infra/`と運用ガイドへ集約した。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-09-25` | `Codex` | A〜Dの統合ロードマップを作成。Linear新規発行を利用者の事前操作ではなく、統括の根拠付き4択判断へ固定 |
| `2026-09-25` | `Codex` | 第1実装batchを反映。schema 2、受付判断、reconciler、DAG/context package、外部同期intent、対応UIを実装し、実runtime/外部provider受入を残件として明記 |
| `2026-09-25` | `Codex` | schema 2表示E2EとLinux packageを追加し、隔離actual-windowを受入。新buildを復帰可能に配備し、稼働中案件を止めず次回通常起動へ設定 |
| `2026-09-25` | `Codex` | external sync executorを実装。TAK-15とDraft PR #27でLinear status/comment、誤状態write拒否、SHA/権限gate、重複抑止、試験専用branch mergeを受入。host `2897e3a1`、UI `dd50afed`を固定review済み |
