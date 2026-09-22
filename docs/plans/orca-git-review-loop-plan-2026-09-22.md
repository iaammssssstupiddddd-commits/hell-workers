# Orca Git基点レビュー反復計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `orca-git-review-loop-plan-2026-09-22` |
| ステータス | `In Progress（統合・最終review差戻しのfixture接続、実runtime・公開未完）` |
| 作成日 | `2026-09-22` |
| 最終更新日 | `2026-09-22` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/orca-parallel-development-proposal-2026-09-20.md` |
| 関連Issue/PR | 外部連携の隔離試験: `TAK-7` / [PR #26](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/26)（merged、製品master未変更） |

## 1. 目的

- 解決したい課題: Git commit SHAへ固定された実装・review・差戻し・再実装ループが未接続である。
  - 現在のOrca基盤には実装担当・固定reviewer・同一session再開・承認失効の部品はあるが、
    `実装 → 統括検証 → review → 差戻し → 同じ実装担当で修正 → 再review → 統合`を
    Git上の一意な対象へ結び付けて完遂する制御がない。
  - Linear受付とOrca内部状態だけでは、どの修正世代が審査・承認・統合されたかを外部から再現しにくい。
- 到達したい状態:
  - Git branch / commit SHA / PRを変更内容の正本とし、Orca統括が実行ループを制御する。
  - LinearはGitHub標準連携で紐づいたPRと外部review・merge等の状態を表示し、利用者向けの
    依頼・優先順位・進捗表示を担う。local branch作成や内部reviewは自動通知される前提にしない。
  - 差戻し時は同じ実装担当sessionと同じworker branchを再開し、前回承認を失効させて新しいSHAを再reviewする。
- 成功指標:
  - 利用者はOrcaの`統括`へ自然文で依頼するだけで、branch/worktree/commit/PR/review世代を入力しない。
  - reviewで必須指摘が出たfixtureを、同一workerと固定reviewerで1回以上修正・再reviewし、承認された
    exact SHAだけを課題用branchへ統合できる。
  - process中断後も永続台帳とGitから一意に再開でき、重複commit・重複PR・別SHAの誤承認を起こさない。
  - LinearとGitHubの表示不整合があっても実装を再実行せず、Git/Orcaの事実を照合して同期だけを復旧できる。

## 2. スコープ

### 対象（In Scope）

- 1 Linear課題につき1本の課題用integration branchと1つの統括worktree。
- 実装A/Bごとの分離worker branch/worktree。worker自身のcommit/pushは禁止し、統括だけがcommitする。
- 修正世代ごとの不変checkpoint commit、固定reviewer verdict、検証証拠、承認失効。
- review差戻しから同じworker sessionへのfollow-up、再検証、同じ固定reviewer sessionでの再review。
- 承認済みworker branchの課題用branchへの直列統合と、統合後headの再検証・最終review。
- 課題用branch一本のDraft PR、GitHub Actions結果、Linear GitHub標準連携による進捗可視化。
- Orca再起動、terminal終了、GitHub/Linear通信失敗、結果不明時のfail-closed復旧。
- Orca UI上の`統括`、`実装A（Codex）`、`実装B（Cursor）`、`レビュー（固定Codex）`表示。

### 非対象（Out of Scope）

- Linear Webhookを実装/reviewループの正本またはschedulerにすること。
- Linear Agent Session APIへの移行。Developer Previewのため、本計画の必須依存にしない。
- workerごとのPR、workerによるcommit/push/merge、reviewerによる修正。
- 自動merge、force-push、承認後のhistory rewrite、無制限の自動再試行。
- 共有file/API/save/renderer/infrastructure作業をA/Bへ並列委譲すること。
- GitHub以外のGit provider対応。
- ゲーム機能、Rust/Bevy実装、native/GPU受入。本計画の受入には非ゲームfixtureを使う。

## 3. 現状とギャップ

### 現状

- 専用candidate `518102b337f67eaf30a72e990a177b3739dce91f`に以下がある。
  - Linear-linked worktreeの可視統括と、別目的課題からの課題/worktree自動引継ぎ。
  - Codex A、軽量Cursor B、固定read-only Codex reviewerの専用launcherとTask bridge。
  - workerの同一task/session再開、固定reviewer session拘束、source fingerprint、承認後変更の失効。
  - 最大2 worker、reviewer 1、重い実行1のhost資源制御。
- GitHub ActionsはPRを入口に変更範囲別CIを実行し、同一base/head/tested SHAを検証できる。
- repository ruleは目的別branch、PRによるCI、統括所有の検証/commit/直列統合を要求する。

### 問題

- `scripts/orca_dispatch.py`は単一roleを配車するだけで、worker完了後の検証、review ticket、採否、
  commit、Linear更新を行わない。
- 第3batchまでに永続反復driverと共有Run/inbox/回答を接続したが、実UI/providerの監督・反復受入は未完。
- 第4batchで承認済みA/Bの直列merge・combined-head再検証/review・単担当への最終差戻しを接続した。
  競合/検証失敗の修正、複数scopeやbase更新を要する修正の認可、実runtime受入は残る。
- Linear GitHub連携はTAK-7 / PR #26で自動link、ready→In Progress、merge→Doneを確認した。
  全team設定・target別rule・外部review eventは未確認。
- 外部連携単独の一巡は完了したが、Orca自動controllerから駆動した一巡ではない。

### 本計画で埋めるギャップ

- Git SHAをreview・検証・統合判断の共通IDにする永続controllerを追加する。
- review verdictを型付き記録にし、`changes_requested`だけを同じworkerへの限定follow-upへ変換する。
- 課題用Draft PR一本へ承認済み成果を集約し、GitHub ActionsとLinear GitHub連携を外部可視化に使う。
- 外部イベントの欠損・重複・遅延を内部実装の再実行へ結び付けない照合境界を設ける。

### 2026-09-22 計画レビューの指摘と反映

以下はcandidate `518102b3`の実装との照合結果。部品の存在をループの動作保証と取り違えない。

| 優先度 | 確認した欠落・矛盾 | 修正先 |
| --- | --- | --- |
| P1 | `orca_roles.validate_ticket`はHEAD＝ticket baseを要求し、`orca_role_state.admit`は同sessionのticket/subject変更を拒否する。統括commit後はそのまま再開できない | M1/M2: 固定assignmentと世代認可を分離し、統括commitの遷移証明でのみ更新 |
| P1 | `orca_dispatch.start`にはworkerの世代別resume経路がなく、同ticket dispatchの再利用だけでは差戻しを配車できない | M3: generation別dispatchと明示follow-up、遅延完了通知の拒否 |
| P1 | `orca_roles.verify_review`は固定reviewerの最新ticket/historyを参照する。Aの後にBを審査するとA承認の照合先が失われる | M3/M4: 審査完了時にsubject別の不変receiptを確定 |
| P1 | `orca_ui_coordinator.launch`は引継ぎmarker/processを監視するが、worker完了から統括の次turnを駆動する永続loopがない | M1/M3/M7: event inbox、単一owner、待機・再開・終了の実装と実runtime受入 |
| P1 | 統合後review・CI・GitHub差戻しから修正担当へ戻る経路がない | M4/M5: 指摘の所有者解決、同担当再開、再統合・再review |
| P2 | local branch作成とLinear更新、内部verdictとGitHub reviewを同一視している | M0/M5/M6: 通知条件・権限・表示範囲を分離 |
| P2 | CIのPR headとtested merge SHA、運用基盤の配備先とPR baseが曖昧 | M0/M5: 各refの契約と配備・受入手順を追加 |

計画レビュー後、下記の第1実装batchでcheckpointと再開認可・審査receiptを追加した。
現行runtimeの反復loop全体が対応済みという意味ではない。

### 第1実装batch（2026-09-22）

- codeはcandidate `518102b3`からの未commit差分。`orca_git_checkpoint.py`とそのtestを追加し、
  `orca_role_state.py` / `orca_roles.py` / `orca_dispatch.py`を接続した。
- M1の世代認可、M2の検証証拠・checkpoint・中断復旧、M3の世代別resumeとsubject別review receiptを実装。
  old schemaを破壊的に移行せず、既存のexact-ticket経路を維持した上でprivate receiptによる遷移を追加する。
- fixtureでA/Bのsame-session再開、別session/改変ticket/外部編集の拒否、ref更新後の復旧、
  固定reviewerのA→B審査後のA承認照合を検査した。実agentを新規起動した受入ではない。
- M0のread-only実測: Orca `1.4.205`はready、repo base refは既存基盤branch。
  GitHub default branchは`master`でbranch protectionなし。team `TAK`にはIn Progress / In Review / Doneが存在。
  repository hooks一覧は空だが、これだけでGitHub App integrationの有無を判断しない。
  Linear GitHub App設定・automation rule・独立reviewer identityは未確認。
- 新たな成立条件: 現行の`run_provider()`は対話CLIの終了をwaitする。worker_done後のTUI待機から
  正常な終了証拠を得てlockを解放する経路もM3のdriverに含める。通知受信だけでprocess終了を記録してはならない。
  `--exit-on-settlement`のopt-in経路を追加し、confirmed settlement＋exact terminal idleを確認して
  自分のprovider子だけを終了・waitする。actual exit codeとTask outcomeを分けて保存する。
  未受入のため既定では有効にしない。event inbox/統括起床、検証失敗の修正、統合以後は未実装。

### GitHub→Linear隔離試験（2026-09-22）

- ユーザーの明示許可後、新規TAK-7、専用source/target branch、Draft PR #26を作成・pushし、
  ready化と試験branch間mergeを実施した。master、TAK-5/TAK-6、candidate実装は変更しない。
- 手動Linear status変更・PR添付なしで、自動link、Backlog→In Progress→Doneをread-backした。
  今回のtargetではDraft解除はIn Reviewではない。内部reviewをnative approvalへ読み替えない。
- fixtureのみのcontracts CIが成功。全体loopの実runtime受入やcandidateのCI成功として流用しない。
- exact SHA、run、activity ID、保持用途は[隔離試験記録](../development-infra/orca-github-linear-acceptance-2026-09-22.md)に集約。
- M0/M6の外部正常系だけが進んだ。M1/M3の自動driver、M4/M5の実装、M7の配備は引き続き未完。

### 第2実装batch（2026-09-22）

- `orca_review_loop.py`と24件のfixture testを追加し、可視統括launcherの寿命へ単一host driverを接続した。
  既存の統括process・配備refは変更しない。内部登録された1〜2 laneだけをatomic台帳で進める。
- confirmed settlementと実provider終了をrole/workspace lock下で照合し、公式APIのrelease結果を再読してから
  検証・checkpoint・固定reviewを順に進める。不正verdictもrelease後に停止する。
- 検証失敗はdirty source/失敗evidenceを固定した世代認可で同workerへ戻し、commitしない。
  review差戻しは指摘だけを同sessionへ渡す。最大3回・同一未解消指摘・未知結果で停止する。
- 非ゲームfixtureでは実Git commitと模擬providerで、差戻し→同worker修正→固定reviewer再審査→承認を一巡した。
  複製driver、未知配車、途中検証、取消後commit、承認後変更、曖昧review、重実行busyの拒否も検査した。
- production差分は実Help判断待ちで停止する。事前のHelp理由を実レビューの代用にしない。
  `paused`解除やruntime変更後の自動引継ぎは未実装。未解決loopを別依頼登録で回避できない。
- 残件: 共有Runとdurable inbox/質問/escalation、統括起床、実provider終了→同session再開、
  A/Bの実runtime反復、combined-head統合/review、GitHub adapter、基盤配備。
  世代ごとの別Run/terminalをまだ使うため、実用可能・自動完遂とは報告しない。
- 検証中のinstalled Cursor更新（2026.09.18-9a7762b）で既存互換testが6 subcase失敗した。
  新版bundleの設定/session実装を監査し、旧版/新版の明示mappingを追加した。権限assertionは維持し、
  未知versionを黙認しない。offline互換test 3件が成功。新版の実provider/hooks受入はM7に残す。

### 第3実装batch（2026-09-22）

- `orca_loop_mail.py`を追加し、1依頼1 Run、固定consumer generation、FIFO Deliveryと回答intentを永続化した。
  配車先Runを新設せず全世代で再利用する。既存の他Runを新規作成で上書きせず、consumer交代も拒否する。
- hostだけがconsuming checkを所有する。質問・例外は可視統括のread-only `watch`へ返し、
  内部`decide`で回答/判断する。利用者のUUID入力やbusy TUIへのキー注入は追加しない。
- 質問をaskのconfirmed receipt、完了等をsend receiptに照合する。質問の`dispatch:`アドレス、task/thread、
  回答先とconsumer世代も確認する。未信頼本文をcommandや権限に読み替えない。
- 全件処理前のACK、release前の完了ACK、完了通知欠損時のglobal承認を拒否する。
  作成/回答/ACK不明結果は自動再送しない。schema 1はread-only互換で、曖昧なRun移行を拒否する。
- 実Git＋模擬通信の通しfixtureで1 Run・4 Dispatch・4完了Delivery ACK・差戻し1回を確認した。
  質問/完了混在、未確定ask、重複回答、consumer交代、他Run/送信元/本文の混入を追加検証する。
- 未完: UI統括の実watch/回答、実providerのsettlement後終了→同session再開、paused/再起動復旧、
  A/B統合・combined-head review・GitHub adapter・新規tree配備。既存Orca状態へのmutationは行っていない。

### 第4実装batch（2026-09-22）

- `orca_git_integrate.py`を追加し、登録済みの課題用clean targetへ、全worker承認・完了ACK後の直列mergeを接続。
  全treeの事前計算、親SHAを保持したmerge commit、prepared intent、非resetのcheckout/CAS ref更新を使う。
  競合時はtargetを変更しない。反映途中の中断はexact前後状態だけを受け入れ、外部編集を上書きしない。
  Git 2.55.0のhelpと公式[merge-tree](https://git-scm.com/docs/git-merge-tree) /
  [read-tree](https://git-scm.com/docs/git-read-tree)仕様を確認し、推測したforce/reset経路を使わない。
- 統合後は新しいvalidation evidenceと固定reviewerの別generationを必須とし、同Runで最終reviewする。
  最終release・完了ACK前にはglobal承認しない。取消・stale承認・ignored衝突・検証によるsource変更で停止する。
- combined findingsはUI統括の`watch`へ返し、統括がhead/元slot/scope内の根拠を内部`route`で指定する。
  同workerの既存scope・同sessionへ指摘だけを戻し、再review・再統合・combined再検証/reviewを進める。
  初期worker baseは維持し、統合SHAはread-only contextとして渡す。scope/base変更の包括的許可にはしない。
- 既存reviewer leaseのbusyで承認済みlane照合が待機しても、inbox/質問回答を止めない修正を追加。
- 実Git＋模擬providerで最終差戻しの一巡、2 lane merge、競合時無変更、中断復旧等を検証する。
  競合・統合後validation失敗・複数scope/base更新の修正、GitHub adapter、実runtime、基盤配備は未完。

## 4. 実装方針（高レベル）

### 責務分離

| 正本・処理 | 所有者 | 備考 |
| --- | --- | --- |
| 依頼、優先順位、利用者向け進捗 | Linear | Gitの事実を表示。中止等は統括が再照合して扱い、statusだけで配車しない |
| branch、commit、PR、merge結果 | Git / GitHub | 変更内容と外部CIの不変証拠 |
| task世代、session、資源lock、差戻し制御 | Orca統括台帳 | Linear statusやterminal titleから推測しない |
| 編集 | 実装A/B | 指定scopeのみ。commit/push/test/reviewは禁止 |
| checkpoint commit、検証、統合、push | 統括 | dirty範囲と対象SHAを毎回照合 |
| verdict | 固定reviewer | source read-only。同じsessionで全世代を審査 |

### Gitモデル

```text
Linear issue
  └─ issue integration branch / coordinator worktree
       ├─ worker-a branch / isolated worktree
       └─ worker-b branch / isolated worktree

worker edit
  → coordinator scope check + selected validation
  → immutable checkpoint commit
  → fixed reviewer reviews exact SHA
       ├─ changes_requested → same worker branch/session → successor commit
       └─ approved → serial integration into issue branch
  → one Draft PR once the first approved integration checkpoint exists
  → combined-head validation + final review (same PR is updated)
  → GitHub Actions / external review
  → merge (automatic mergeはしない)
```

- 課題用branchはLinear/Orcaが提示するissue ID入りbranch名を優先する。fallbackでもissue identifierを必ず含める。
- PRタイトルまたは本文にissue identifierを含め、必要ならLinearのcontributing magic wordを使う。
  `Fixes`等の完了語はmergeで課題を完了させる意図がある場合だけ使用する。
- worker branchは課題用branchの固定baseから作る。workerはGit metadataをread-onlyとし、統括が
  scope検査と必要検証後にcheckpoint commitを作る。
- review対象commitはamend/rebase/force-pushしない。差戻し修正は後続commitとgenerationを追加する。
- 単一laneは承認済みworker headへfast-forward可能。複数laneは統括が課題用branchへ直列mergeし、
  merge後の新しいcombined SHAを最終review対象にする。cherry-pick等でSHAが変わった場合も再reviewする。
- Draft PRは課題用branch一本だけ。worker branchは通常pushしない。外部復旧に必要な場合も統括が明示判断する。
- 最初の承認済み統合checkpointからDraft PRを作れるようにし、最終reviewまで外部表示を待たせない。
  未承認worker変更は公開用integration branchへ混入させない。PR作成前の内部反復はOrca UIで表示する。

### 状態機械

```text
worker: planned → dispatching → implementing → validating → checkpointed → reviewing
                                ↑                │                          │
                                └─ fix_required ─┘                          │
                                └─ changes_requested ←──────────────────────┘
        reviewing → approved → integrating

issue:  integrated → combined_validation → final_review → awaiting_ci → merge_ready
                        │                     │               │              │
                        └──── correction_required ────────────┘              │
                                  ↓                                         │
                         owner worker resumes → re-integrate                 │
        merge_ready → completed (actual merge confirmed, not merely ready)

不明な結果 → blocked_unknown / 反復上限・判断待ち → paused / 中止確認 → cancelled
```

- 台帳はissue、固定assignment、lane、generation、worker/reviewer session、scope digest、base/candidate/integration SHA、
  tree/index/source fingerprint、validation evidence、subject別review receipt、指摘の所有者、PR URL/number、
  CIのbase/head/tested SHA・run/attempt、外部event cursor、同期watermark、再試行数を保持する。
- 各遷移は前状態とexact identityを検査し、atomic保存する。同一event/generationの再実行は同じ結果を返す。
- `changes_requested`は構造化された必須指摘と受入条件だけを同じworker sessionへ渡す。元依頼全体を再注入しない。
- 検証失敗もscope内なら同じ担当へ戻す。承認対象のsource/index/evidenceが変わればその承認を失効する。
  固定reviewerが次の対象を審査する通常のhistory追加だけで、確定済みの他subject承認を失効させない。
- combined review、外部review、CI失敗は統括が指摘をtaskへ分類し、同じ担当へ戻す。CI再実行だけで済む通信障害とcode修正を区別する。
  修正担当が統合後contextを必要とする場合は、統括所有のbase更新receiptを記録してから再開し、以前の承認は流用しない。
- 修正は初期値3回までの有界budgetで実行し、同一指摘の無進展・scope拡張・担当不明では統括判断へ戻す。
  通常の差戻しごとに利用者へ再入力を求めない。利用者には仕様・権限など統括が解決できない判断のみ問い合わせる。

### checkpoint後も同じ担当を再開できる認可

- 固定assignment（issue/task/lane/worktree/session/scope）と、世代ticket（base/head/指摘/dispatch capability）を分ける。
  現行のticket digest/subjectを無条件で書き換えたり、HEAD・fingerprint検査を削除したりしない。
- worker終了と編集lock解放を確認後、統括が検証した変更だけをcommitし、before/after HEAD・tree・index、
  validation、操作owner、generationをcheckpoint receiptへ記録する。この遷移証明だけで次世代ticketを発行する。
- commit前にintentを永続化し、commit成功後・台帳保存前に停止してもparent/tree/operation IDをread-backして一意復旧する。
  証明に合わないGit操作、途中の外部編集、別sessionへのfallbackは停止する。
- review完了時にsubject SHA・evidence digest・verdict・session・当該turnの記録を不変receiptへ確定してから次の審査へ進む。
  receiptはhost内の来歴検査であり、独立したGitHubアカウントの承認を偽装するものではない。

### 統括loopの駆動と寿命

- 可視`統括`起動時にcontrollerを一つだけ確保し、issueごとのowner leaseと永続event inbox/cursorを持つ。
  worker完了・review完了・有界GitHub pollの通知を照合し、資源lock取得後に次の遷移を実行する。
- 実装時のOrca CLI/イベント仕様に基づき、待機中の統括sessionへ構造化follow-upを届ける。
  busyなTUIへキー入力を差し込まず、連続通知はqueue化する。対応APIがない場合は同sessionの明示resume方式を実装・受入する。
- タブを開いただけで永続稼働とは扱わない。起動、待機、停止、heartbeat失効、再接続をUIに表示し、
  アプリ/host停止中は実行しない。再起動時に同じ台帳から再開し、二重controllerや新規sessionへの黙示切替を拒否する。
- 外部中止・PR closeはread-back後に新規配車を止め、動作中workerを安全停止・成果保全する。
  自動reopenはしない。外部通知を受けたこと自体は実行認可にしない。

### Linear/GitHub連携

- Linear→Orcaの独自Webhook consumerは初期実装に含めない。GitHub標準integrationのPR連携を使う。
  local branchの作成では通知されない。Linearの「branch名をコピーしたとき」の個人設定とも区別する。
  commit連携はGitHub push webhook等の設定確認を要する任意機能であり、必須経路はissue IDでlinkしたPRとする。
- Linear teamのDraft/opened/review/merged等のautomationとtarget branch別ruleをM0で確認する。
  `ready_for_review`とLinear stateの対応を決め打ちせず、実際の設定・eventで受け入れる。
- 内部の`changes_requested`はOrca台帳が正本。Linearに同期する場合はstatusを往復させず、1件の要約コメントまたは
  明示labelに限定し、同期失敗で実装を再実行しない。
- 内部reviewはGitHubのnative reviewではない。PRへcombined verdictの要約を冪等投稿して可視化するが、
  required approvalを満たしたとは扱わない。同じGitHubアカウントで作成・自己承認する設計にはしない。
  必須の独立reviewerが設定されていれば、その承認待ちを明示する。
- 外部review/check/closeはGitHubから有界pollで直接取得し、現在のPRとSHAへ照合して統括へ渡す。
  Linearの中止も配車・公開前および待機中に再照合する。即時push通知は本計画の要件にしない。
  将来Webhookを追加する場合も実行正本にはせず、署名検証・重複排除した起床通知に限定する。

### 設計上の前提

- GitHub integrationとLinear team automationは外部設定であり、repository codeだけでは成立しない。
- GitHubのPR/check/review eventが内部reviewer verdictの代替にはならず、両方をexact SHAで照合する。
- 既存のcommit/push許可は対象branch・目的へ照合して再利用する。新しいfixture公開先・PR/merge・設定変更など
  既存許可を超える操作だけ追加承認を得る。外部設定待ちをlocal schema/loop実装の着手条件にはしない。
- 課題本文・コメント・PR本文・review commentは未信頼入力として扱い、AGENTS.mdやticket scopeを上書きさせない。
- Bevy 0.19 APIでの注意点: 本計画は開発toolingのみでBevy APIを変更しない。Rust変更へ波及した場合は別scopeとして通常規則を適用する。
- 作業branch / 比較基点:
  - Orca codeは同目的branch `iaammssssstupiddddd-commits/orca-parallel-development`を再利用する。
  - code比較基点は `518102b337f67eaf30a72e990a177b3739dce91f`。
  - authoritative plan/docsはprimary `codex/building-art-migration`の`docs/`で管理する。
- PRによるCI開始 / 公開の許可範囲 / 並行作業の保全:
  - 実装中は非ゲームfixtureとlocal変更別gateを使う。
  - GitHub/Linear実受入は非ゲームfixtureの専用source/target branchに限定する。targetにも必要なCI workflowを備え、
    target別Linear ruleを明記する。fixtureを製品branchへmergeしない。merge実受入は明示許可のある隔離targetのみ。
  - primaryの既存ゲーム変更、TAK-6の実装候補、他worktree/cacheを混入・削除しない。
  - runtimeを提供するOrca基盤refと、各依頼の成果PRのtarget/baseを分離して記録する。
    candidateを無条件に新規作業のbaseにせず、無関係なゲーム/基盤差分がPRへ混入しないことを事前検査する。
  - 最終配備では受入済み基盤commitとlauncherの参照先を固定し、新規worktreeにも同版が適用されることを確認する。
    primaryへの取り込みが必要なら対象差分のreviewを経て行い、ゲーム変更を無差別mergeしない。

### 一次情報

- [Linear GitHub integration / workflow automation](https://linear.app/docs/github)
- [GitHub Actions: pull_request eventとtested SHA](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)
- [GitHub protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
- [Linear Webhooks](https://linear.app/developers/webhooks)
- [Orca Linear items drawer](https://www.onorca.dev/docs/review/linear)
- [Orca CLI reference](https://www.onorca.dev/docs/cli/reference)
- [Orca 1.4.205 Run FIFO Delivery/ACK](https://github.com/stablyai/orca/blob/v1.4.205/src/main/runtime/rpc/methods/orchestration/messaging/check-run.ts)
- [Orca 1.4.205 question/answerのDispatch・consumer拘束](https://github.com/stablyai/orca/blob/v1.4.205/src/main/runtime/orchestration/db/questions/question-threads.ts)

## 5. マイルストーン

### M0: 外部Git連携と契約を確定する

- 変更内容:
  - GitHub repositoryとLinear teamの標準integration、branch/PR linking、status automation、review表示をread-only確認する。
  - teamのstarted/review/completed state、branch naming、branch protection、必須GitHub checkを記録する。
  - PR作成者・外部reviewerのGitHub identityと、内部verdictの表示手段を確認する。
  - runtime基盤ref、成果PR target/base、fixture target、launcher配備先を分離し、混入差分を検査する。
  - primaryとcandidateのagent rule差を確認し、適用する編集委譲・権限規則を確定する。
    本レビューではworkerを起動しない。運用時も適用規則が編集委譲を禁止したままなら配車しない。
  - integrationが未設定なら、必要権限と設定手順だけを提示し、許可なしに変更しない。
- 変更ファイル:
  - `docs/development-infra/orca-development.md`
  - 本計画
- 完了条件:
  - [ ] GitHub→Linearで利用可能なeventと不足設定が推測なしで記録されている。
  - [ ] issue branch、worker branch、Draft PR、mergeの命名・所有権が確定している。
  - [ ] internal reviewとGitHub review/checkの採否条件が区別されている。
  - [ ] 独立reviewerが必要な設定で、内部verdictだけをrequired approvalに数えていない。
  - [ ] 配備ref・作業base・fixture公開範囲が確定し、既存ゲーム変更を混入させない。
- 検証:
  - read-only API/UI確認。外部設定mutation、push、PR作成は行わない。

### M1: Git identityと永続ループ台帳を実装する

- 変更内容:
  - issue/task/lane/generationとGit base/candidate/integration SHAを拘束するschemaとatomic state transitionを追加する。
  - current dirty/index/HEAD/upstream、worktree identity、branch ownerを照合し、別作業やhistory rewriteを拒否する。
  - legacy role stateを移行またはread-only互換で取り込み、曖昧な状態は`blocked_unknown`へ置く。
  - 固定assignmentと世代ticketを分離し、統括checkpoint/base更新receiptに限ってHEAD・ticketの変更を認める。
  - event inbox、owner lease、cursor、retry budgetを永続化し、可視統括の起動/終了へcontrollerを接続する。
- 変更ファイル:
  - candidate `scripts/orca_review_loop.py`（第2batchで追加）
  - candidate `scripts/orca_loop_mail.py`（第3batchで追加）
  - candidate `scripts/orca_role_state.py`
  - candidate `scripts/orca_roles.py`
  - candidate `scripts/orca_ui_coordinator.py`
  - candidate `scripts/host_coordination.py`
  - candidate `scripts/tests/test_orca_review_loop.py`（第2batchで追加）
- 完了条件:
  - [ ] 全状態遷移が前状態、generation、session、worktree、SHAを検査する。
  - [ ] 同じ入力の再実行が重複task/commitを作らない。
  - [ ] unknown、破損state、branch移動、dirty範囲外変更でfail-closedする。
  - [ ] controllerは一つだけ稼働し、idle/busy/停止/再接続を区別して通知を保持する。
  - [ ] 既存schemaは黙って書き換えず、backup付きmigrationまたは停止理由を返す。
- 検証:
  - fixture repositoryによる状態遷移・再起動・重複・破損・競合test。

### M2: 統括所有のcheckpoint commitを導入する

実装状況: local checkpoint helperとfixture検証を追加。第2batchでdriverからの呼出しを接続。実runtime受入は未完。

- 変更内容:
  - worker完了後にticket scope、tracked/untracked、symlink/hardlink、Git metadata、base ancestryを検査する。
  - 統括が必要なfocused validationとHelp影響判断を完了した後だけ、不変checkpoint commitを作る。
  - commit message/trailerへissue、task、generation、validation/Help判断を記録し、workerのGit writeを引き続き拒否する。
  - worker終了を確認してからcommit intentを保存し、成功結果不明でも同じ操作を重複実行しない。
  - commit後のreceiptと次世代ticketを同sessionへ接続する。検証失敗はcommitせず修正へ戻す。
- 変更ファイル:
  - candidate `scripts/orca_review_loop.py`
  - candidate `scripts/orca_git_checkpoint.py`
  - candidate `scripts/orca_roles.py`
  - candidate `scripts/tests/fixtures/orca_edit_acceptance/`
  - candidate `scripts/tests/test_orca_review_loop.py`
- 完了条件:
  - [x] workerはcommit/pushできず、統括だけがexact scopeをcommitできる（既存mount境界＋helper）。
  - [x] commit前後のtree/index/HEADと台帳が一致する（fixture）。
  - [x] amend/rebase/force-pushを使用せず、修正世代はsuccessor commitになる。
  - [x] 統括commit後も既存guardを迂回せず同worker sessionの再開認可を通過する（fixture、実provider未受入）。
  - [x] commit成功直後のcontroller停止から、commitを増やさず復旧できる（fault injection）。
- 検証:
  - A/B fixtureの正常系、scope外、dirty混入、競合、commit失敗、終了結果不明test。

### M3: 型付きreview verdictと差戻しループを接続する

- 変更内容:
  - reviewer入力をexact base/candidate SHA、diff、scope、validation evidenceへ固定する。
  - verdictを`approved`または`changes_requested`、blocking findings、再確認条件としてschema検証する。
  - 差戻し時は同じworker branch/sessionへ指摘だけをfollow-upし、修正後に新checkpointと同じ固定reviewerを再開する。
  - 承認後変更、別reviewer、新session fallback、空の指摘、自己修正を拒否する。
  - generation別dispatch capabilityとworker resume/follow-upを追加し、旧世代の完了通知を拒否する。
  - reviewerが次subjectへ進む前に不変receiptを確定し、最新session stateだけに承認照合を依存させない。
  - worker/reviewer終了eventを統括の次turnへ接続し、自然文受付後に利用者の「続けて」を要求しない。
  - accepted worker_doneと対話CLIの終了を分離し、同担当sessionを保存してprocess終了を確認してから
    source/role lockを解放する。timeout/idleのみで終了成功にせず、実際に次世代まで進むことを受け入れる。
- 変更ファイル:
  - candidate `scripts/orca_review_loop.py`
  - candidate `scripts/orca_dispatch.py`
  - candidate `scripts/orca_roles.py`
  - candidate `scripts/orca_task_bridge.py`
  - candidate `scripts/orca_role_state.py`
  - candidate `scripts/orca_ui_coordinator.py`
  - candidate `scripts/tests/test_orca_review_loop.py`
- 完了条件:
  - [x] `実装 → 検証 → changes_requested → 同じ実装担当 → 再検証 → 同じreviewer → approved`がfixtureで一巡する（実Git・模擬provider）。
  - [ ] 各世代のSHA、指摘、session、検証証拠が台帳から追跡できる。
  - [ ] 同じ未解決指摘の反復、scope拡張、結果不明は自動継続せず統括へ停止理由を返す。
  - [ ] A審査→B審査の後もAの不変承認を検証でき、改変したreceiptや別SHAは拒否する。
  - [ ] idleな統括が完了通知から再開し、修正・再reviewまで利用者の追加送信なしで進む。
- 検証:
  - Codex A正常系を先行し、Cursor Bはsimple leaf fixtureだけで同じ状態遷移を確認する。

### M4: A/B成果を課題用branchへ直列統合する

実装状況: 第4batchで正常統合・最終review・既存1担当への最終差戻しを接続（実Git/模擬provider）。
競合/検証失敗の修正、複数scope/base更新の認可、実runtime受入は未完。

- 変更内容:
  - 承認済みSHAだけを統合対象にし、A/Bを一件ずつ課題用branchへ取り込む。
  - 競合時は自動解決せず、統括所有の新しい修正・review世代として扱う。
  - combined headで変更別検証と固定reviewer最終確認を行い、worker承認をcombined headの承認へ流用しない。
  - 統合後指摘を元taskへ返し、必要なcontext/base更新を統括が認可して同担当で修正する。
    担当不明・共有scope競合は統括へ戻し、Bへ複雑な統合作業を押し付けない。
- 変更ファイル:
  - candidate `scripts/orca_git_integrate.py`
  - candidate `scripts/tests/test_orca_git_integrate.py`
  - candidate `scripts/orca_review_loop.py`
  - candidate `scripts/tests/test_orca_review_loop.py`
- 完了条件:
  - [x] A/B独立変更の直列統合とcombined-head reviewが成功する（別々のfixture。2 lane実provider一巡はM7）。
  - [ ] 競合、base更新、承認後変更、片lane未承認で統合が停止する（競合/stale/dirty/取消はfixture確認、残る全matrixを継続）。
  - [x] 統合後のcommit graphから各worker generationを追跡できる（2-parent mergeのfixture）。
  - [x] combined reviewの差戻しが担当修正→再統合→combined再検証/reviewまで一巡する（既存単担当scope・模擬provider）。
  - [ ] 統合後validation失敗/競合の修正、複数scopeやworker base更新を要する指摘を認可付きで扱える。
- 検証:
  - 2 lane fixture、競合fixture、stale approval、別base、dirty integration branch test。

### M5: 課題用Draft PRとGitHub CIを接続する

- 変更内容:
  - 課題用branchをpushし、issue identifier入りのDraft PRを一意に作成または再利用する。
  - PR URL/number、head/base SHA、GitHub Actions run/check suiteを台帳へ記録する。
  - 最初の承認済み統合checkpointでDraft PRを作り、以後は同じPRへ後続commitを反映する。
  - `ci_scope.py` / `ci_result.py`を再利用し、PR headとGitHub Actionsのtested merge SHAを区別する。
    base/head/tested SHA、workflow/run/attempt、選択群と必須jobを照合し、base更新でもCI証拠を失効させる。
  - `ready_for_review`、CI成功、head不変、固定reviewer最終承認を別々に検査する。
  - push/PR作成結果不明時はread-backし、重複PRや再pushで迂回しない。
  - PRのreview/check/closeを有界pollし、指摘のSHA・未解決thread・最新状態を照合する。
    CI失敗/外部changes requestedを統括が修正taskへ返し、同じPRで再検証・再reviewする。
  - 内部combined verdictはSHA付き要約としてPRへ冪等反映する。native reviewの代用にはしない。
- 変更ファイル:
  - candidate `scripts/orca_git_publish.py`（新規予定）
  - candidate `scripts/orca_review_loop.py`
  - candidate `scripts/tests/test_orca_git_publish.py`（新規予定）
  - `.github/workflows/ci.yml`（必要なevent/check契約だけ。不要なら変更しない）
- 完了条件:
  - [ ] 同じissue branch/targetの稼働PRが一つで、base/head/tested SHA更新を正しく新しいCI対象として扱う。
  - [ ] CIの別SHA、cancelled/skipped必須job、古いreview、head更新前の対象に対する遅延成功通知を採用しない。
  - [ ] mergeは自動実行せず、統括が全gateを提示して許可された操作だけ行う。
  - [ ] Draft解除とmergeは別操作で、completedは実mergeのread-backと対象commit照合後だけ成立する。
  - [ ] 外部指摘/CI失敗→同担当修正→同PR更新→再review/CIが一巡し、取消は新規配車を止める。
- 検証:
  - mock GitHub API/CLIの全異常系後、許可された専用fixture Draft PRで実受入する。

### M6: Linear GitHub標準連携を受け入れる

実測: TAK-7 / PR #26で標準PR link、Draft解除、merge反映を受入済み。
これはcontroller接続前の隔離試験で、外部review event・同期異常系・UIからの全体自動進行は未受入。

- 変更内容:
  - issue identifierでPRがLinear課題へlinkされ、設定されたPR/review/merge状態が反映されることを確認する。
    local branch作成や内部reviewの自動表示は受入要件に数えず、Orca UIとPR要約で補う。
  - Linear status automationとOrca内部状態を照合するが、Linear表示を内部実行成功へ読み替えない。
  - 必要なら統括が開始/差戻し/完了の要約を一回だけ投稿する。実装世代ごとの実況コメントは作らない。
- 変更ファイル:
  - `docs/development-infra/orca-development.md`
  - `docs/orca-quickstart.md`
  - candidate `scripts/orca_git_publish.py`（必要なread-back/syncのみ）
- 完了条件:
  - [ ] Draft PR、設定済み外部review/merge状態が同じLinear課題に対応し、CI詳細はlinked PRから辿れる。
  - [ ] Linear/GitHub同期失敗がworker再実行や重複commit/PRを起こさない。
  - [ ] 利用者はOrca UIとLinear課題から進捗を確認でき、内部ID/command入力を要求されない。
- 検証:
  - 許可された専用Linear課題・Draft PRで実eventを確認する。Webhook consumerは作成しない。

### M7: 中断復旧と実runtime受入を完了する

- 変更内容:
  - 各状態でOrca/terminal/controller再起動、GitHub/Linear通信断、古い通知、結果不明を注入する。
  - 台帳・Git・Orca・GitHub・Linearのread-backから一意な場合だけ再開する。
  - UI上で統括/A/B/reviewer tabと差戻し世代を確認し、不要terminal/worktree/fixture PRを整理する。
  - 受入済み基盤refをlauncherへ配備し、新規依頼のworktreeでも同じcontroller/guardが使われることを確認する。
    旧sessionは移行可否を明示し、動作中に黙って入れ替えない。
- 変更ファイル:
  - candidate `scripts/tests/test_orca_review_loop.py`
  - candidate `scripts/tests/test_orca_git_publish.py`
  - `docs/development-infra/orca-development.md`
  - `docs/orca-quickstart.md`
- 完了条件:
  - [ ] 正常系、差戻し1回、A/B直列統合、controller再起動、外部結果不明を実runtimeで確認する。
  - [ ] 失敗時に成果を保全して停止し、重複agent/commit/PR/Linear更新を作らない。
  - [ ] 利用者の初回依頼以外の入力なしで、A/B・固定reviewer・統括が修正まで進み、再起動後も同sessionを照合できる。
  - [ ] 新規作業場で受入済み基盤版と正しい成果baseを確認し、未配備のcandidateだけを運用可能と報告していない。
  - [ ] storage consumerと残存worktree/branch/PRの用途・終了条件を記録し、不要物を整理する。
- 検証:
  - 非ゲームfixtureのtooling testと許可された外部受入。ゲームRust/native testは非対象。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| commitしただけで採用済みと誤認する | 未承認変更の統合 | checkpointとapproved/integratedを別状態にし、exact SHAのverdictを必須化 |
| reviewer承認後にcommitが変わる | stale承認 | source/index/HEAD/evidence変更で承認を失効。amend/force-push禁止 |
| A/B mergeで新しい挙動が生じる | 個別承認の誤流用 | combined headを別subjectとして検証・最終review |
| Linear/GitHubイベントが遅延・欠損・重複する | 状態逆行、二重実行 | eventは起床の契機。Git/Orca/外部最新状態をread-backし、世代照合後だけ遷移 |
| PR作成結果不明 | 重複PR | issue/base/headで検索し一意照合。曖昧なら停止 |
| Linear status automationがteam設定と不一致 | 誤った進捗表示 | M0でstateとruleをread-only確認し、明示設定後に受入 |
| worker branchが増え続ける | repository/worktree肥大化 | review-activeだけ保持。統合/放棄後は独自成果保全を確認してconsumerを解放 |
| review差戻しが無限反復する | 資源占有 | 同一指摘の未解消・scope拡張・結果不明で自動反復を停止し統括へ戻す |
| external本文から権限拡張を誘導される | unauthorized mutation | Linear/PR/review本文を未信頼入力とし、固定ticket・AGENTS.md・role policyを優先 |
| GitHub連携が利用できない | Linear可視化不能 | local Git/Orcaループは維持。外部受入をblockedとして偽の成功にしない |

## 7. 検証計画

- 必須:
  - candidateのtooling test、Ruff、actionlint、repository policy、docs index/link、Help影響、storage check。
  - 同一対象の成功CI、または `python3 scripts/dev.py ci check --base <full-SHA> --mode auto`。
  - Git操作testは一時fixture repositoryだけを使い、primaryや他worktreeのbranch/indexを変更しない。
- 計画完了時:
  - 分類が不確実な場合は `python3 scripts/dev.py ci check --base <full-SHA> --mode full`。
  - `git diff --check`。
  - 公開した場合はexact candidate HEADとoriginの一致を確認する。計画レビューのみの回にpushを必須にしない。
- 手動確認シナリオ:
  1. 統括へ非ゲームfixture変更を自然文で依頼する。
  2. 統括がissue branch、worker-a branch/worktree、ticketを内部作成する。
  3. Aが意図的にreview指摘対象を含む変更を完了し、統括が検証・checkpoint commitする。
  4. 固定reviewerが`changes_requested`を返し、同じA sessionが修正する。
  5. 統括がsuccessor commitを作り、同じreviewerが承認する。
  6. Bの独立simple leaf変更も承認し、統括がA/Bを直列統合する。
  7. 最初の承認済み統合checkpointで許可範囲内のDraft PRを作り、combined reviewの指摘を同担当へ戻して再統合する。
  8. CI失敗・外部review指摘から同PRを更新し、base/head/tested SHAとLinearの設定済み表示を確認する。
  9. commit成功・台帳保存前や通知待機中にcontrollerを再起動し、重複なしで同じgenerationから再開する。
  10. 取消・上限到達・失われたsessionで停止理由と成果保全を確認する。外部mergeは隔離targetへ許可がある場合だけ行う。
  11. 基盤配備後の新規依頼で、同版のlauncher/guardと正しい作業baseを確認する。
- パフォーマンス確認:
  - ゲーム性能は非対象。状態台帳・Git照合・API read-backの有界性だけをtestし、busy loopや無制限pollを禁止する。

### 検証データ管理（各バッチの開始前・報告前に更新）

- 正本: `docs/development-infra/validation-storage-workflow.md`
- primaryの `python3 scripts/dev.py validation` でretain/checkを行う。
- batch ID / 判断対象 / 責任者 / 全consumer:
  - 実装開始時に`orca-git-review-loop-<milestone>`単位で登録する。
- worktree・clone・branch / subject・asset view / job roots:
  - candidate worktreeを同一feedback対象として維持し、Git fixtureは一時directoryへ作る。
- 開始時bytes / 検証結果 / 採用成果物と最終結果の正本:
  - 各milestone開始時に実測し、採用codeはcandidate branch、仕様はprimary docsへ集約する。
- 削除済みpath / 前後bytes / filesystem空き差:
  - milestone終了時に一時Git fixture、不要terminal、終了済みworktreeをexact pathで整理して記録する。
- 残存path / bytes / owner / consumer / 次の作業 / release_when:
  - candidateとreview-active cacheだけを次milestone consumerへ引き継ぐ。
- 修正対応 / 整理状態:
  - 差戻し中は同じcandidate/cacheを保持し、最終承認または放棄後に解放する。

## 8. ロールバック方針

- milestoneごとに独立commitし、状態schemaを変更するmilestoneには旧schema read-only互換または明示migrationを付ける。
- M1〜M4はfeature flag相当のlauncher入口で旧単一dispatchから分離し、未受入時は新規loop投入を停止できるようにする。
- M5/M6の外部連携を無効化してもlocal Git/Orca台帳から成果を失わない。
- push済みbranch/PRを削除・force-pushで戻さない。必要ならPRをcloseし、理由をLinearへ一回だけ記録する。
- rollback後もreview-active worker branch、唯一のcommit、会話sessionを削除せず、採用・放棄判断後に整理する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 2026-09-22追記: ユーザーのcommit指示で累積codeを`535d4d869a0ca5ca0ff3ed666f980647b15f7e9b`へ保存。
  pushなし。既定base refは同branchなので新規treeへ含まれるが、稼働processの変更や運用受入済みの主張はしない。
  次はユーザーが指定した「保存領域欠損3件の修正」を、可視統括→実装→固定reviewの試運転に使う。
  判定根拠を調べる前の台帳解除・空directory作成は禁止。ゲーム/Rust/native検証は引き続き対象外。
- 進捗: `第4実装batch。M4の直列統合・combined-head検証/review・単担当への最終差戻しをfixture接続。実runtime・公開は未完。`
- 完了済みマイルストーン: M2のlocal helper/fixture項目。自動接続と実受入は残る。
- 未着手/進行中: M0/M6は隔離fixtureの外部正常系を確認。M1/M3/M4は一部実装。M5/M7は未着手。

### 次のAIが最初にやること

1. primaryの本計画、`docs/development-infra/orca-development.md`、validation storage workflowを読む。
2. candidate `518102b3`上の未commit差分を保全し、追加helperとtestsを読む。今回の変更を未知のdirtyとして消さない。
3. M1/M3のUI統括watch/回答とworker_done後の実process終了・同session再開を受け入れる。
   編集agentの実起動は適用AGENTS.mdの許可範囲を再確認し、禁止状態なら迂回しない。
   M0の未確認外部設定は公開段階までに確定し、独立したlocal実装を止めない。
4. M4の残る競合/統合後validation失敗/base更新認可とM5のGitHub adapterを接続する。
   最終reviewの単担当差戻しは実装済みなので作り直さず、統合receipt/履歴と固定sessionを再利用する。

### ブロッカー/注意点

- Linear GitHub標準PR連携は実測済み。全automation設定・別targetのrule・外部reviewは未確認。
- 許可されたTAK-7 / PR #26の隔離試験は完了。実装candidateのpublish/mergeや本番自動進行は未実施。
- Cursor installed版は2026.09.18-9a7762bへ変わった。設定/session offline互換は確認したが、旧版の実Task受入を新版へ流用しない。
- primaryには別目的のゲーム変更があり得る。candidateへ無差別mergeせず、目的別base/headを固定する。
- 既存のゲーム実装候補は本計画のfixtureや受入対象にしない。

### 参照必須ファイル

- `docs/development-infra/orca-development.md`
- `docs/orca-quickstart.md`
- `docs/development-infra/validation-storage-workflow.md`
- candidate `scripts/orca_ui_coordinator.py`
- candidate `scripts/orca_dispatch.py`
- candidate `scripts/orca_roles.py`
- candidate `scripts/orca_role_state.py`
- candidate `scripts/orca_task_bridge.py`
- candidate `scripts/orca_review_loop.py`
- candidate `scripts/orca_loop_mail.py`
- candidate `scripts/orca_git_checkpoint.py`
- candidate `scripts/orca_git_integrate.py`
- `.github/workflows/ci.yml`

### 最終確認ログ

- 最終 `python3 scripts/dev.py check`: `N/A（Python開発tooling・文書のみ。変更別contracts/toolingを使用）`
- 最終 `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`: `N/A（Rust非対象）`
- 最終 `python3 scripts/dev.py cargo -- test --workspace`: `N/A（Rust非対象）`
- CI run URL / base・head・tested SHA / mode・選択群・結果:
  - fixtureのみ: [run 35736783946](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35736783946)、attempt=1、auto/contractsのみ成功。
    base=`b68bafd7358580ff8ad77962fa02987a215b5b60`、head=`cb402021ff02526def4c5c561c89db052ff31e29`、
    tested=`38f8ac18584b44bcbe9a0a66a56dc788516a1be5`。candidate code / primary dirty docsには流用しない。
- ローカル代替command / 比較基点・dirty範囲・source fingerprint / 結果:
  - `2026-09-22 計画レビュー / base=e947155c91f3bf8c87c85f0d882296fe7e6ac581 / dirty=本計画・親計画・docs/plans/README.md・docs/README.md。`
  - `python3 scripts/dev.py docs --write / --check`、docs linkと`git diff --check`はpass。
  - `python3 scripts/dev.py ci check --base e947155c91f3bf8c87c85f0d882296fe7e6ac581 --mode auto`はdocumentationとしてcontractsのみ選択し、既存storage台帳不整合で停止。全gate成功とは扱わない。
  - `2026-09-22 第1実装batch / candidate base=518102b337f67eaf30a72e990a177b3739dce91f / dirty=checkpoint helper・role state/launcher・dispatcherと対応tests。`
  - candidateの変更別gateはcontracts/toolingを選択し、contractsの既存storage欠損3件で停止。
    `python3 scripts/dev.py quality --group tooling`を独立実行し、Ruff/actionlint、Python 450件、
    Blender用Python tooling 164件、perf self-testがpass。agent rule、repo hygiene、crate dependencyとdiff検査もpass。
    この独立実行でstorage失敗を相殺しない。ゲームbuild/test/nativeは実行しない。
  - 第1batch candidate source fingerprint: `38b5e4a0df9d9ed768f1bb061ffb1c43b427c772626da60a3ae9fa4f337c3350`。
    Help No impact: 開発用hostのcheckpoint・再開認可・review証拠・終了処理だけの変更で、
    player input/game state/runtime assetsと`build_help_panel_content`の静的provider経路は不変。
    実agent/新規worktree/PRの受入は未実施。今回の実装差分は未commit・未push。
  - `2026-09-22 第2実装batch / candidate base=518102b337f67eaf30a72e990a177b3739dce91f`。
    dirtyは第1batchにloop/UI接続・検証失敗認可・対応tests・Cursor版別offline互換testを追加。
    最終source fingerprint: `b4dcc76463f9e0d449f875b026eba32d134fe6d755e9cae7c5f20fe45c176fb9`。
    最終`quality --group tooling`はRuff/actionlint、Python 474件、Blender tooling 164件、perf self-test成功。
    loop追加24件、Cursor互換3件を個別にも確認。docs索引/link、agent rules、repo hygiene、Help、diff検査成功。
    途中のCursor更新由来6 subcase失敗は上記のoffline再監査/mapping更新後に解消した。
    最終変更別gateはcandidate=contracts/tooling、primary docs=contractsを選択し、双方とも既存storage欠損3件で停止。
    tooling単独成功を全gate成功へ読み替えない。ゲーム/Rust/native/model起動は行っていない。
  - Help実レビューはNo impact。producerは開発用host driver・private台帳・provider互換testであり、
    playerの入力/状態/表示/runtime assetsへ流入しない。`build_help_panel_content`は不変の静的manifest/providerを消費する。
    primaryの並行production変更は本判断へ混ぜない。Help source/snapshotの空変更は行わない。
  - candidate保持: `/home/satotakumi/orca/workspaces/hell-workers/orca-parallel-development`、
    allocated `27,609,673,728 bytes`。owner=Orca統括、consumer=反復driverの実runtime/統合受入、
    next_action=共有Run/inbox接続と実provider同session再開、release_when=採用/放棄と最後のreview consumer終了。
    test fixtureは各TemporaryDirectoryで除去。初回test隔離漏れの偽bridge記録3file（924 bytes）は内容/所有を照合し、
    exact path `task-bridges/4ab8974e-a391-4a70-92ab-e57d33734a74`だけ削除済み。testのstate_rootを隔離して再発を防止した。
    実agent履歴・稼働resourceは削除していない。実装差分は未commit・未push・未配備。
- Help実レビュー / native受入要否・結果 / primary storage確認: `Help No impact（開発tooling/文書のみ）、native非対象。storageは今回と無関係な使用中登録3件の実path消失を検出してfail。`
  - 第3batch最終対象: candidate base=`518102b337f67eaf30a72e990a177b3739dce91f`、
    source fingerprint=`8d79f1f77a91e1df1d0e6eb9077ce9624a3098d5041dbda15e9c66814f83a393`。
    累積dirtyへ`orca_loop_mail.py`と14 tests、dispatcherの共有Run、loop/UIのwatch/decideと対応testsを追加。
    `quality --group tooling`はRuff/actionlint・Python 491件・Blender tooling 164件・perf self-testが成功。
    index/link、Help、agent rules、repo hygiene、diff検査も成功。ゲーム/Rust/native検証は実行しない。
  - 変更別gateはcandidateでcontracts/tooling、primary docsでcontractsを選択し、いずれも既存storage欠損3件で停止。
    Help No impactの根拠は、producer/consumerとも開発用host・private台帳・UI統括で完結し、
    player input/game state/runtime assetsおよび静的Help manifest/provider経路を変更しないこと。
  - 同candidate保持の最新allocated bytes=`27,609,772,032`。owner=Orca統括、consumer=実UI/provider反復・統合受入、
    next_action=実watch/回答と同session再開の受入、release_when=採用/放棄と最後のreview consumer終了。
    temporary Git/mail fixturesは各testで自動除去。新しい恒久job、実Run/Task/agent、公開PRは作成していない。
    code/docsは未commit・未push・未配備のまま保全した。
  - 第4batch最終対象: candidate base=`518102b337f67eaf30a72e990a177b3739dce91f`、
    source fingerprint=`ae2b07f8bf0c14d499852bc8d5c74ebb1879f85c3c211a3301cd945fddce79aa`。
    累積dirtyへ`orca_git_integrate.py`と15 tests、loopの統合/最終差戻し、UI promptを追加。
    最終`quality --group tooling`はRuff/actionlint、Python 506件、Blender tooling 164件、perf self-test成功。
    先行tooling検証後に修正を追加したため、最終sourceで全toolingを再実行した。ゲーム/Rust/native/model起動なし。
    docs索引/link、agent rules、repo hygiene、diff検査も成功。
    candidate同baseのHelp gateはproduction差分なし。実経路判断もNo impactであり、host統合・指摘配車・private記録は
    player入力/ゲーム状態/runtime assets/静的Help providerへ流入しない。Help source/snapshotは変更しない。
  - 第4batch変更別gateはcandidate=contracts/tooling、primary docs=contractsを選択し、既存storage欠損3件で停止。
    tooling単独成功を全gate成功へ読み替えない。今回の差分は未commit・未push・未配備。
    同candidateのallocated bytes=`27,609,886,720`、前回から`+114,688 bytes`。
    owner=Orca統括、consumer=実UI/provider反復・統合受入、next_action=統合残件/GitHub adapterと実runtime受入、
    release_when=採用/放棄と最後のreview consumer終了。TemporaryDirectoryのfixtureはtest終了時に自動除去。
    新規の恒久worktree/jobや実Orca Run/Task、PRは作成しない。別作業のmissing resourceは変更しない。
- 未解決エラー: `実UI/provider受入、統合の競合/検証失敗/base更新修正、公開/配備は未完。Linearの全automation設定・別targetのrule・native review eventは未確認。既存storage台帳の使用中登録3件で実pathが存在しない。消失原因・現consumerの要否は未確認であり、本作業では削除・復元・台帳解放を行わない。`

### 保存領域欠損を使う実runtime試運転（2026-09-22）

- ユーザーが統括→実装→固定reviewの実運用試験として指示。専用TAK-8と
  `tak-8-storage-recovery-trial` worktreeを作成し、review holdを登録した。
- 基盤は`535d4d86`にcommit済み。起動時に共有統括台帳の別repo記録を自身の記録として検査する不具合を検出し、
  `b9b63c50`で共通record検証と自身のrepo admissionを分離。既存記録は削除しない。
  回帰2件を含むPython 508件、Blender tooling 164件、Ruff/actionlint、perf self-testが成功。
- 試運転worktreeを同commitへfast-forwardし、可視`統括`の起動・TAK-8取込・acknowledgeを確認した。
  実装/reviewの完遂はまだ未確認。AはCodex、Bは単純な独立処理に限るCursor、reviewは固定Codexを維持する。
- 欠損は`formwork-release-candidate`、`refactor-r01-r14-candidate`、`tak6-orca-implementation-worktree`。
  変更別gateはこの3件で停止したまま。根拠なしrelease、空directoryの偽復元、検査無効化は禁止。
  正本台帳の是正は調査・固定review後に統括が行う。ゲーム/Rust/nativeテスト、push/PR/製品mergeは行わない。
- 実runtimeの結果: 可視統括が分離`tak-8-storage-recovery-test-a`、編集ticket、共有Run
  `run_d9a13f938019`を作成したが、worker起動前の既存role-state不整合で停止。
  `worker-a.json`は`tasks={}`、`last.phase=unknown`、exit=-9でattempt identityを欠く。
  ファイル更新日時は試運転前の2026-09-22 01:10:21 UTC。類似のinterrupt test fixtureは存在するが、
  当該fileの生成元・それ以前のbindingを証明できていないため削除/初期化しない。
  launcherは`unknown role attempt`で拒否し、driverは`KeyError: attempt_id`でthread終了した。
  RunのTask/Dispatchは0、reclaimable workerも0。実装・checkpoint・固定review・統合は一つも完遂していない。
- 起動元で追加修正: 不完全attempt identityを型付き拒否、配車前role-state検査、予期しないdriver例外の
  failed記録を追加。回復前の試運転subjectは変更せず、基盤candidateで検証する。
  `164ebb92`へcommit済み。追加3 regressionを含むPython 511件・Blender tooling 164件、
  Ruff/actionlint、perf self-test成功。candidate変更別gate（contracts/tooling）とprimary docs gate（contracts）は
  欠損3件で停止し、全gate成功ではない。Help No impact: 開発hostの認可/記録経路のみで、player input、
  game state、runtime assets、静的Help manifest/providerへ流入しない。ゲーム/Rust/native試験なし。
  現Runの再開・不完全な既存bindingの復旧には、証拠を保持した明示的な照合経路が別途必要。
  未配車のterminalを成功/settled扱いせず、別Runや新sessionで迂回しない。
- 保存領域調査: 型枠は`8d3edab93d9cd45eadf00fbe1d6c8b4ff157c6ed`、R01–R14は
  `a0487a35c09ed444dc588902ed198c9532f4c685`のGit objectが残る。
  TAK-6はBacklogと既存実装branchが残る。元のbuild cacheや未記録成果の完全復元は未証明。
  consumer終了の根拠はなく、固定review前なので3件の復元・releaseは未実施。
- 保持するtrial（28,991,488 bytes）とworker（28,585,984 bytes）はどちらもowner=
  `orca-storage-recovery-trial`、consumer=`tak8-orca-runtime-storage-repair`でprimary台帳登録済み。
  next_action=同じ試運転の状態照合と安全な復旧、release_when=試運転/修正の承認または明示終了。
  新規登録以外に既存holdを変更せず、成果物・cache・worktreeの削除なし。

### Definition of Done（全体計画）

- [ ] M0〜M7が完了し、Git SHA基点の実装/review差戻しループを実runtimeで受け入れた。
- [ ] 影響ドキュメントが更新済み。
- [ ] 同一base/head/tested SHA・選択群の成功CIとURL、またはdirtyを含むローカル変更別検証を記録した。
- [ ] 検証後の追加差分がなく、必要群のskip/cancel/欠損を成功扱いしていない。
- [ ] Help実レビュー、storage整理をCIとは別に確認した。native受入はゲーム/native変更がない限り非対象。
- [ ] external integrationの実受入でPRと設定済み外部review/merge状態が同じLinear課題へ対応付き、CI詳細を辿れる。
- [ ] 差戻し、再実装、再review、combined-head review、中断復旧の各証拠がexact SHA/session/generationへ結び付く。
- [ ] 統括への初回依頼後、手動の続行指示なしで反復する。新規worktreeへの受入済み基盤配備も確認した。
- [ ] 最終close時に本計画のconsumerは0。共有残存は別consumer・担当者・bytes・終了条件を引継ぎ済み。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-22 | Codex | Gitを変更正本、Orcaを実行制御、Linear GitHub標準連携を外部可視化とする初版。checkpoint commit、固定review差戻し、A/B直列統合、Draft PR、外部同期復旧をM0〜M7へ分割 |
| 2026-09-22 | Codex | コード照合レビュー。commit後の同session認可、subject別review receipt、統括のevent駆動、統合後/外部差戻し、CIのbase/head/tested SHA、Linear通知境界、基盤配備と受入条件を修正。実装は未着手 |
| 2026-09-22 | Codex | 第1実装batch: 統括checkpoint・世代認可・固定review receipt・世代別dispatch resumeを追加。worker_done後のTUI終了確認もdriver要件に追加。実運用loopと外部受入・配備は未完 |
| 2026-09-22 | Codex | 許可されたTAK-7 / PR #26の隔離作成・push・mergeを実施。Linearの自動link、ready→In Progress、merge→Doneとfixture限定CIを確認。全体loopの完了とは分離して記録 |
| 2026-09-22 | Codex | 第2実装batch: 可視統括host driver・永続反復台帳・失敗検証の同session認可を接続。実Git/模擬providerの差戻し一巡を確認。共有Run/inbox・実runtime・統合/公開/配備は未完 |
| 2026-09-22 | Codex | 第3実装batch: 共有Run、FIFO通知、回答/ACKの送信前intent、UI統括watch/decideを接続。1 Runで4 Dispatchの反復と全完了ACKをfixture確認。実UI/provider受入・統合/公開/配備は未完 |
| 2026-09-22 | Codex | 第4実装batch: 承認済み直列merge・combined検証/固定review・単担当の最終差戻しを接続。実Gitで再統合まで一巡。競合等の修正・実runtime・公開/配備は未完 |
