# 通常Orcaへの復帰・独自統括の撤去と外付け化計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `orca-normalization-and-coordinator-extraction-plan-2026-10-03` |
| ステータス | Feedback — A通常版復帰の技術受入完了。B構想は完全別プロジェクトOrca Conductorへ移管済み。通常運用の応答まで保全subjectを維持 |
| 作成日 / 最終更新日 | 2026-10-03 |
| 作成者 / 実施責任者 | Codex main agent |
| 関連提案 | [外付け構想の別プロジェクト移管案内](../development-infra/orca-conductor-migration.md)。設計正本はhell-workers外 |
| 関連Issue | 既存TAK-14 / request `54cf1dd7-7485-5f49-a8e5-827773355096`。課題取消・新Runなし |
| 方針の根拠 | 利用者の「統括機能を全て削除して通常のOrcaに戻す。構想はplugin/workflowへ」 |
| 時間・工数上限 | **設けない**。以前の復旧120分・rollback60分は本計画へ継承しない |

## 1. 目的

**独自統括をOrca本体・通常起動・hell-workersの必須運用から撤去し、通常Orcaだけで作業できる状態へ戻す。**
現在の統括を完成・復旧させる計画ではない。過去runtimeやDBへ巻き戻す計画でもない。
利用者が明示的に選んだ場合だけ使える外付け機能として、良い要件と失敗から得た制約を残す。

成果を二つに分ける。

1. **A：通常Orca復帰** — 独自UI・controller・Driver・自動配車/同期・必須hookがなく、既存成果と会話を保全して通常の操作が成立する。
2. **B：構想の保存と外付け設計** — 完全別プロジェクトOrca Conductorへ移管済み。設計・今後の実装は本repositoryの対象外。Bの実装完成をAの完了条件にしない。

計画策定後、利用者の「進めてください」により実行開始。工程別の実測記録は末尾へ追記する。
旧統括の自動復旧・hardening・限定rollbackの未完工程は本方針で置換し、旧指示を再開しない。

## 2. スコープ

### 撤去するもの

| 層 | 撤去対象 | 撤去方法 |
| --- | --- | --- |
| Orca本体 | hell-workers向け統括パネル、`supervision.*` RPC、mailbox/controller起動、独自状態投影と埋込接点 | 原則、独自差分を含まない検証済み通常版へ交換。共有ファイル全体を削除しない |
| 起動・設定 | `launch-supervised`の環境注入、desktop起動先、repoの統括default tab/hook、独自base-ref固定 | GUI・正式CLI・PATH CLIを同じ通常配布物へ揃え、独自hookだけ除去 |
| project-owned実行系 | intake/route/role dispatch、Driver、host broker、request/loop自動処理、独自外部同期・復旧helper | 実行接続を廃止し、通常開発対象から除去。原本は保全後に非実行の保存対象へ移す |
| 必須運用 | 統括/A/B/reviewerタブ常設、旧launcher限定、旧loop経由のみ許す規則とその検査 | 通常の単独agent運用を既定に改訂。任意workflowの条件とは分離 |
| 監視 | 旧統括を再起動・修復・通知再送するautomation/prompt | 実施時に正式な管理APIで廃止または新目的へ改訂。古いheartbeatから復活しない状態にする |

### 保持するもの

- Orca標準のTasks、Linear連携、worktree、terminal、provider、browser、plugin機構、native orchestrationとRun履歴。
- 通常Orca自身のdaemon監督・安全機構。名前に`supervision`や`orchestration`があるだけで消さない。
- ゲームのコード・asset・制作原本・全未commit/untracked成果、独自commit、既存会話、必要な担当tabと固定reviewerの履歴。
- `dev.py`、Cargo/host資源guard、`validation_storage.py`、Help/native Skills、Clippy警告0等のゲーム品質規則。
  `dev.py validation`の「coordinator」は検証管理であり、撤去する独自Orca統括ではない。
- request/loop/Run、immutable input、fence、transfer intent、WAL、ACK、unknown receiptの原本。撤去を成功・承認・Doneへ読み替えない。

### 非対象

- ゲーム編集・build/test・製品受入、push/merge/公開、新規Run、課題取消、認証情報出力。
- 全checkoutのreset/clean、原本DBのdowngrade、`user_version`やtriggerの手直し、旧台帳への成功/取消/ACKの捏造。
- plugin/workflowの本格実装・自動再開・GitHub/Linear自動完了。これらはA完了後の別実装計画で扱う。
- 稼働中の他案件の中断、PTY/会話の一括削除、原本を失う整理。

## 3. 現状とギャップ

### 今回の読取調査で確認した境界

- 本体fork：`/home/satotakumi/tools/orca-ui-lifecycle`。独自統括はcommit済み差分にも存在する。
  dirty差分だけ消しても通常版には戻らない。
- local上流tag `v1.4.205`は`11aba8bdc5e492d3ba01fc7fe333495ace74128f`。
  shallow checkoutであり、これを「現在の最新通常版」とは断定しない。
- 当該tagはorchestration schema41 / daemon protocol36。現候補sourceはschema43 / protocol37。
  独自v42/v43のretention triggerもあり、**旧版が起動しただけではDB互換を証明できない**。
- control workspace：`/home/satotakumi/orca/workspaces/hell-workers/orca-hardening-next`。
  `scripts/orca*.py`89本・29,770行、`test_orca*.py`100本（名称ベースの概数）。primary `scripts/`には`orca*.py`はない。
  この行数は工数や完了率ではなく、手作業で一つずつrevertしない判断の根拠である。
- `launch-supervised`、desktopファイル、control `orca.yaml`の`default-entry`が独自実行系へ接続する。
  GUI、canonical CLI shim、`~/.local/bin/orca`は現状同じpackageを選んでいない。
- `supervision.read`でもcontrollerを起動し、controllerはSyncWorkerと相談/配車を開始する。
  監視automationのPAUSEDや表示の閲覧だけでは、独自effect停止にならない。
- 旧pauseは未解決loopを拒否する。**「旧loopをapprovedにするまで撤去不可」は採用しない。**

### 引き継ぐ歴史（現在稼働の証明ではない）

13:54〜13:58 UTCの前計画ではapp3049512、runtime `2d5830cc-61d0-4f3f-9a17-00349be14e52`、
Run `run_3236488f0dff` / generation2、保存Codex session `01a0d377-feef-7520-bef5-50565ef282cb`、
統括terminal `term_756e4b0f1b47a24bb2baa44b01480999`が照合された。Driver failed、製品工程は未再開。
実施時はprocess birth、所有、pending operationと現在terminalを改めて最小限照合する。
前計画では保全copyもlive切替も未実施だった。保存済みback-upがある前提にしない。

一次根拠：本体`src/main/supervision/supervision-controller-process.ts`、
`src/main/runtime/rpc/methods/supervision.ts`、`src/main/runtime/orchestration/db/schema/migrate.ts`、
`src/main/daemon/daemon-protocol-version.ts`、control `scripts/orca_supervision.py`、
`scripts/orca_request_runtime.py`、`orca.yaml`、`scripts/check_agent_rules.py`。

## 4. 実装方針・作業総量

### 方式を変える

現在のforkへ小さな復旧部品を足し続けたり、89本のhelperを一つずつ通常状態へ修正したりしない。
**独自成果を保全 → cleanな通常版を隔離受入 → 独自系を保全停止 → 通常版へ切替 → 必須接続を撤去**を一続きで扱う。
上流を確認した配布物を優先し、source buildが必要なら別のclean checkoutを使う。dirtyな現forkをresetしない。
通常版に独自helper、runtime pin、独自Run consumer移行を再導入しない。

移行専用処理が不可欠なら、新効果の抑止・対象processの保全終了・結果観測だけに限定する。
汎用retirement/succession/enable frameworkを完成させることは目標にしない。
一時処理は通常版の常駐依存にせず、用途終了後の撤去まで同じ変更単位に含める。

### 見積もりと進捗の扱い

- 総量は下記M1〜M6の**6つの受入単位**。Bのコード実装は含まない。
- 調査済み範囲は本体・設定・control・規則の4層。未確定なのは通常配布物の選択、保存データ互換、稼働processの停止/継続方法。
- 時間・人分の上限は設定しない。現時点で根拠ある総人時/終了時刻は未確定。
  M1で対象tupleと移行分岐を固定し、工程ごとの作業量・不確実性と予測を記録する。
- 工程数の割合を工数達成率に読み替えない。合格工程、現在の具体作業、残件、実測人分を報告する。
- 同じ仮説で効果が出なければ一度で打ち切り、方式を見直す。新部品追加を進捗としない。
  新しい方式も含め、live前に正常系と失敗時退避を隔離環境で通す。

### 所有・branch・公開

mainが編集・切替・文書・統合を担当。並行調査はread-onlyのみ。reviewは固定のread-only担当がexact subjectを検査する。
既存reviewerの会話/作業場を保持し、旧統括の再生成をreviewの前提にしない。
独立変更は`codex/`専用branch/clean candidateで扱う。primaryのゲームbranchと現control/forkのdirty成果はそのまま保持する。
比較基点は通常版のfull SHA、撤去前HEAD、dirty/untracked manifest。commit/PR/pushの実施は計画策定に含まない。
Bevy/Rust/API変更はない。ゲーム用Cargo検証をOrca移行の代用にしない。

## 5. マイルストーン

| 工程 | 成果物 | 合格条件 |
| --- | --- | --- |
| M1 対象と保全・移行方式を固定 | 撤去manifest、独自成果/状態の初期保全と更新追跡、通常版tuple、互換分岐と作業見積もり | 全入口・所有・原本・unknown・残す標準機能が特定され、初期copyの復元/読取試験が成立 |
| M2 通常版を隔離受入 | clean通常候補、非ゲームfixtureの証拠、固定review | 独自統括なしで通常操作が成立し、採用する保存/daemon移行経路と失敗時退避が検証済み |
| M3 独自実行系を保全停止 | 新規effect停止、in-flight結果照合、exact旧processの終了/継続記録、更新を含む最終checkpoint | 承認/Doneを捏造せず停止でき、他案件と必要sessionを侵害せず最終移行subjectを照合済み |
| M4 通常経路へ切替 | GUI/CLI/設定/hook/規則の整合した切替 | current実体・loaded build・profile・daemon接続・会話/tab対応を実見。旧統括は再起動しない |
| M5 active機能と古い指示を撤去 | 通常開発対象からの独自code/tests除去、文書/規則/監視の更新、concept保存 | active依存0、独自成果は非実行位置に保全、通常機能とゲーム品質gateは残る |
| M6 通常運用の実確認・close | 再起動を含む最終受入、同会話の通常turn、保全/整理結果 | 通常操作と具体的次工程が成立し、独自spawn・同期・自動replayが0。A完了を報告 |

### M1：実施前に固定するもの

1. 各repoの`git log --oneline -5`、diff全行、未追跡/ignoredの唯一成果と他sessionの所有を確認する。
   commit、dirty、実行設定、state、会話、必要tabsをmanifestに分ける。全巨大cacheの複製はしない。
2. SQLite正規backup等でDB/WAL整合を保全する。appだけでなくDBを開くdaemonも確認する。
   原本はその場に保持し、隔離copyで整合・必要record/会話参照を検査する。credentialは出力しない。
   M1は初期保全と更新追跡の開始であり、DB backupだけで別JSON/WAL/provider会話も同時点整合したとは扱わない。
   live切替にはM3の最終checkpointが必要。
3. 正式な通常配布物のversion、source/full SHAまたは配布hash、CLI、schema、daemon protocol、profileをtupleとして選ぶ。
   過去custom package09を「通常版」として採用しない。新しい通常版の同じschema番号も、実table/trigger/API互換の証明にはならない。
4. 全default tab/hook、startup/service、環境注入、settings override、CLI入口、旧automationを確定する。
   primary外の設定変更は実施時のsandbox承認経路を使い、制限を迂回しない。

### M2：profile/daemon移行の選択

**同profile・同daemon継続を優先するが、証明できないdowngradeはしない。**

| 分岐 | 採用条件 | 不成立時 |
| --- | --- | --- |
| 同profile継続 | 隔離copyの読取/書込/再起動で全必要データとunknownが保持され、標準候補が現daemon/PTYへ支持された接続を行える | 次の分岐へ。DB version偽装やdaemon強制置換はしない |
| 通常版専用profile | 標準の移行/再登録手段でrepo・worktree・会話・必要tab対応を保持でき、旧profile/Run原本を非実行履歴として読取可能に残せる | 実切替を行わず、欠ける保存契約を明示する |

後者では旧Runを別Runへ再作成・自動replayしない。同じCodex保存会話はproviderの正式resume経路で扱い、
旧processのpositive exit後にだけ新processを起動する。新incarnationが必要なら対応を記録し、同一PTY維持と偽らない。
稼働中の他案件PTYは継続または自然終了を待つ。移植不能という理由でまとめて終了しない。
新profileを使うだけで全履歴が移行したことにはしない。読取・再開・tab復元の試験を分ける。

### M3：旧統括の状態に依存しない保全停止

- 旧loopのapproval/成功・Run完了・Linear Doneは不要。未完は未完のまま保存する。
- 新規受付/dispatch/外部sync/自動resumeを先に抑止し、進行中effectのreceiptと外部readbackを照合する。
  結果不明はunknownのまま保持し、本文再送・再取消・再実行しない。
  旧監視automationの無効化もこの段階で正式APIとreadbackにより確認し、M5で廃止/指示整理を完了する。
- 支持されたservice/通常終了経路を一次sourceとfixtureで確認する。必要な移行専用抑止は別の廃止操作として設計/検証し、旧guardを緩和しない。
  廃止記録は「成功した旧workflow」ではなく「新効果を行わず撤去のため停止した」と区別する。
- 対象はactual owner、process birth、FD/ancestry等で限定する。app終了、controller終了、provider終了、daemon/PTY保持を別々に観測する。
  PIDだけのkill、absenceを終了扱い、偽exit0/ACKは使わない。
- 旧runtime pinの復旧や新consumer successionを通常Orcaへ戻す前提にしない。
  停止できないprocessがあれば、その対象の移行だけ止め、通常候補・文書整理など無関係な準備を進める。
- 対象writerの停止/静止確認後、M1以降の更新を含む最終backupまたは支持された差分移行を行う。
  DB・custom JSON/WAL・会話・tab参照の整合を検査し、M2試験subjectとの差異に必要な再検証/reviewを完了してからM4へ進む。
  他案件writerを止められない場合は支持されたonline移行または旧profile保持の分岐を使い、直接DB/WALコピーで済ませない。

### M4/M5：切替と削除の対象

- desktop、`current`、canonical shim、PATH CLIを通常候補へ揃える。独自環境を注入しない支持された通常起動を使う。
  旧「必ずlaunch-supervised」規則は今回撤去する契約であり、通常化後の起動を禁止する根拠にしない。
  版確認は標準が提供するidentityと実process executable/package hashで行う。
  独自`loadedRelease`/`release.loaded-identity.v1`を必須にせず、不足する証拠を得るため旧patchを再導入しない。
- `orca.yaml`/local hookの統括default-entryだけを撤去。通常shell/provider、一般setup診断等は必要性を確認して残す。
- primaryの`AGENTS.md`、`CLAUDE.md`、`GEMINI.md`、他active rule、`README.md`、`docs/DEVELOPMENT.md`、
  `docs/orca-quickstart.md`、`docs/development-infra/orca-development.md`と`check_agent_rules.py`/関連testsを同時改訂する。
  shared-checkout並行編集禁止、main所有のbuild/commit、資源guardは維持する。通常UIで起動できることを編集権限拡大と混同しない。
- active codeからsupervision専用moduleとimport、RPC登録/generated catalog、UI/i18n、test依存を除去する。
  sharedファイルはcustom hunkだけ対象。通常版採用ならclean sourceへ独自moduleを持ち込まない。
- guarded-create、Run CAS、draft取消、loaded identity等の独自基盤は通常版に残す前提にせず、独立改善候補として保存する。
  Linux修正等の別目的成果も保全し、統括機能として誤って廃棄しない。
- controlの実装/未commit snapshotは保全manifestのhash照合後に非実行の保存先へ切り離す。
  名称globで89本を一括削除しない。通常作業branch/設定/packageから到達できないこととsource撤去を両方確認する。
- 旧復旧計画/運用書は歴史・conceptへ分類し、active入口から外す。旧automationの廃止は正式APIで行い、設定ファイルを直書きしない。

## 6. リスクと対策

| リスク | 対策・合格条件 |
| --- | --- |
| schema43→41/protocol37→36の盲目的downgrade | 実データ/daemon互換の隔離試験。非互換なら通常専用profileを採用し、旧原本は保存 |
| 旧controllerが読取やdefault hookで復活 | 全起動入口の閉包を確認。fresh起動/閲覧/再起動で独自process・effectが0 |
| DBを使用中に単独copyして履歴を壊す | 正規backupと所有確認。copy整合検査、原本上書きなし |
| live PTYと会話保存を同一視する | process/PTYと会話の移行を別試験。必要な同session resume前に旧provider終了を確認 |
| 全削除でゲーム/標準Orca/他案件を巻き込む | path+所有+上流diffのmanifest。名前ではなく依存経路で分類 |
| 外付け化が復帰を遅らせる | Aを先にclose。Bは文書のみ保存し、別計画なしにコードを増やさない |
| 停止が再び無限の基盤開発になる | 一時処理は廃止操作の閉包だけ。同profile維持に固執せずM2分岐を使う。方式変更を隠さない |

## 7. 検証計画

### 通常Orcaの受入

- 採用分岐は公式AppImage。公開digestと実hash、package closure、下記の限定smokeで受入する。
  source buildを採用する分岐だけtype/lint/unit/integration/E2Eを公式手順で要求する。
  今回は上流source build/full suite・新worktree作成を実行せず、本受入外とする。
- 標準Tasks/Linearの閲覧、repo/worktree選択、terminal表示/入力、provider起動/同会話resume、browser、通常停止/再起動。
  外部writeを必要とするテストは隔離fixtureへ限定し、実TAK-14/公開先へ書かない。
- 基本入力の受付だけでなく、新turnとその中の具体的な非ゲーム操作を実見する。
  同会話の正式resume時は新しい限定指示で通常単独agentの作業へ切り替える。
  保存会話中の旧統括prompt・bootstrap・loop・自動復旧を再開しないことをprovider出力と具体操作で確認する。
- 実施した通常操作、表示poll、再起動、repo登録fixtureで独自統括tab/Driver/controller/dispatch/syncが起動しない。
  primaryが受入済みagent起点。旧19worktreeは参照/成果保全だけを確認し、旧AGENTS/hookは凍結原本として保持する。
  local-onlyはhook自動実行を閉じるがagent指示は無効化しない。旧checkoutでのagent開始前には個別に通常規則へ切り替え、
  必要なsubject再reviewを行う。全旧checkoutの規則整合・手動旧helper実行の禁止をOSで強制したとは主張しない。
- 旧unknown操作を再送しない、旧Runを変更/再作成しない、ゲーム/dirty/source/会話を失わない。
- fixed read-only reviewはbaseline+candidate+manifest+設定tuple+実移行方式を対象にする。変更後は承認無効。
  旧loopの固定review承認を通常化の承認へ流用しない。

### Repository gateとHelp

- 変更群はtooling/docs/config。primary change-aware coordinatorで対象を決定し、同subjectの検証を記録する。
  Rustが非対象ならゲームCargo/native受入を実行しない。必要群の欠損を成功扱いしない。
- 規則変更後は`python3 scripts/check_agent_rules.py`と関連tooling tests、索引変更後は`python3 scripts/dev.py docs --write`、両索引check、`git diff --check`。
- 実際の機能撤去後にHelp影響Skillを使う。想定No impactは開発環境のみの変更だが、ゲーム操作/条件/文言が不変であることを実差分から判定する。
- 既存のvendor link欠損等は別の未解決gateとして報告し、全体greenと偽らない。

### 検証データ管理

正本：[validation storage workflow](../development-infra/validation-storage-workflow.md)。開始/resumeごとに読む。
primaryの`python3 scripts/dev.py validation`で候補/backup/fixtureのowner・全consumer・実測bytes・next_action・release_whenを登録する。
副作用なしの計画策定では新backup/job/packageを作らない。実施時はseal/finalize/checkを通す。
旧source/会話/stateの唯一成果はconcept/移行の正本として保持し、不要な巨大build/rawまで全量永久保存しない。
review-active cacheや他consumerを削除せず、不要copyだけ所有確認後に整理し、path/bytes/空き差を報告する。

## 8. 失敗時の退避・ロールバック

- live前：現選択/原本を維持し、隔離候補を修正する。停止の原因を旧統括修復へ戻さない。
- 切替後：通常版専用profileなら旧profileを上書きしない。同profile書込があれば現在DBを保存し、古いcopyを戻さない。
- 復帰先は検証済み**非自動の退避経路**。旧launch-supervised/旧controllerを無条件に再有効化しない。
  配布物を元へ戻す必要がある場合も独自effectを停止した設定との互換を先に検証する。
- 逆切替が安全に証明できなければ成果を保全し、その対象だけ停止する。複数回の盲目的再起動で結果不明を増やさない。
- irreversibleなデータ移植/唯一原本削除/公開/他案件停止が必要なら、初めて理由を示して利用者判断を求める。
  通常の技術選択や既存許可内の撤去に、内部ID選択・不要な再承認を求めない。

## 9. AI引継ぎメモ・Definition of Done

### 現在地

- 通常版1.4.205を専用profileへ切替する分岐を採用。旧DB43/protocol37は移植せず、原本とonline backupを歴史として保持する。
- M1〜M6の通常版復帰の技術受入は完了。同保存会話の標準新turn/実操作と最終再起動readbackを確認。B構想は独立repositoryへ移管し、コード実装は未着手。本計画では実装しない。
- 旧監視automationは正式APIで削除済み。必須規則と現行ガイドは通常運用へ変更済み。新Run・ゲーム変更・外部公開なし。
- 現行ガイドへ通常運用契約と受入結果を集約した。利用者の通常運用フィードバック中は保全subjectを維持し、最終受入後に本temporary planをarchiveする。
- 参照：[構想の移管案内](../development-infra/orca-conductor-migration.md)、[前rollbackの不成立記録](orca-safe-rollback-plan-2026-10-03.md)、
  [旧hardeningの履歴](orca-system-hardening-plan-2026-09-29.md)、storage workflow、CLIのversion-matched guide。

### Aの完了条件

- [x] 採用通常packageとprimary起点の独自統括UI/RPC・常駐実行系・自動hook・必須運用・旧監視接続を撤去済み。旧参考checkoutのagent開始は本受入外。
- [x] GUI/CLI/通常配布物/profile/daemonのtupleを実見し、標準操作が成立する。
- [x] 会話・必要tabs・成果・dirty/commit・Run/unknown履歴が保全され、必要な同会話の通常turnと具体操作を確認。
- [x] 再起動・閲覧・通常作業開始で独自process/spawn/sync/replayが0。単なるfeature非表示ではない。
- [x] primaryのactive docs/rulesと採用通常packageが通常運用と整合。旧凍結checkoutには旧規則を保持し、未切替のagent起点にはしない。標準Orcaとゲーム品質契約は保持。
- [x] exact source/設定の固定review、必要検証、Help実判断、storage整理を記録。未検証/既存不合格を明記。
- [x] 独自構想と唯一source成果を非実行位置に保存。任意workflow/pluginがなくても通常Orcaを使える。

### 計画策定時点の検証ログ（現在の切替状況ではない）

- 2026-10-03：read-only計画reviewの指摘（最終checkpoint、旧指示を継続しない同会話resume、標準identity使用）を反映。
  これは計画のレビューであり、実施時のfixed source review・互換受入・適用承認ではない。
- `python3 scripts/update_docs_index.py --check`：plans/proposals両方pass。`git diff --check`：pass。
- `python3 scripts/dev.py validation check`：pass、187 batches、legacy0、allocated 675649445888 bytes。
  新backup/job/package・削除なし。計画文書はprimary正本へ保存し、検証出力を蓄積していない。
- `python3 scripts/dev.py docs --write`：両索引は更新済み。全体docs契約は既存`node_modules/debug`の3リンクと
  `node_modules/nan`の1リンク欠損でfail。今回追加した計画/提案のリンク欠損ではなく、vendorを修正/削除していない。
- `python3 scripts/dev.py ci check --base 80df44c471a964907fb0fe8b4d0c634d11f33f13 --mode auto`：
  既存dirty `validation_storage.py`とuntracked package/node_modulesを含むためcontracts/tooling/deps/rustを選択。
  contracts内のstorage・agent rules・Help（no production changes）・hygiene・crate graph・両索引はpass。
  同じvendor link欠損でcontractsがfailし後続群は未実行。全体gate合格とは報告しない。
- Rust/Clippy/game test/native受入：未実行。今回コード/ゲーム機能の変更はない。
- Orca切替受入：未実行。計画完成を実装・復帰完了とは報告しない。

## 10. 更新履歴

### 実施記録（2026-10-03 UTC）

- 14:33〜14:36：canonical CLIのhost側statusはapp/runtime not_running。
  hostでdaemon1459822、旧launcher2087516、provider2103719/2103720、Codex2103721、shell1565694の生存を確認。
  sandbox内のPID一覧はhostを表さないため終了証拠として使わない。標準UI停止を全writer静止と扱わない。
- 通常候補：公式release `stablyai/orca` / `v1.4.205`、asset `orca-linux.AppImage`、197744574 bytes。
  GitHub release公開digestと既存AppImageの実測SHA256が一致：
  `7bede254c95ad7237098890bbeca53e667a044997f0314bb67dbc4b932094bcf`。
  既存squashfs-rootの`orca-ide`は独自launcherへのsymlinkだったため使わず、AppImageから専用領域へ再展開。
  新展開の実binaryでCLI version1.4.205とhelpの実行を確認。最新版とは主張しない。
- 14:37:57：primary storageへ`orca-normalization-20261003` evidence holdを登録。
  保存先 `.git/orca-normalization/20261003-1433`、consumerは通常切替と任意構想ソース保存。
  mainの初期保全utilityで3repoの現在source/dirty patch、control/UI独自commit bundle、SQLite online backup、
  profile/custom-state/entrypointsコピーを開始。原本変更・台帳書換え・通知再送なし。
  このcopyはlive初期保全であり、M3最終checkpointや同時点整合の証拠ではない。
- 初期保全は13 artifact / 4509044809 bytes、独立SHA再検査で不一致0。SQLite integrity_check=ok/schema43。
  supplemental manifestにPATH/current/監視設定、incremental bundleの元Git依存、ignored原本の所在、
  統括と固定reviewerの会話UUID・inode・archive member・一致SHAを記録。ignored原本は削除/移動していない。
  tar中のraw DB/WAL/SHMはforensic copyに限り、正規backupへ上書き復元しない契約を明記した。
- 14:49以降：短い隔離profile `/tmp/orca-normal-UWjm1l` で通常版のapp/runtime/graph readyを確認。
  fixture repo・通常terminalを作成し、一回送信したpwdの実出力を読取確認。browser about:blankのsnapshot取得も成功。
  最初の長いprofile pathはLinux socket上限で失敗した。自分の試験appだけpidfd終了確認後に短いpathへ変更した。
  旧統括へは再起動/再送しなかった。空profile受入を履歴移行の合格とは扱わない。
- 15:16:03〜15:16:05：一回限りのoperator廃止処理を固定read-only review後に実施。
  script SHA `58d1016fb72ff5a713b1ead1cf01a5cd14d17df21483649b15f6920abf96072a`。
  旧Codex2103721へexact pidfd SIGTERM、launcher2087516は自然終了。bwrap2103719/2103720もkernel exit観測済み。
  daemon1459822・shell1565694は継続。raw exit0/ACK/旧Run成功は主張せず、append-only廃止receiptを別保存した。
  最終task_complete11:46:07.138Zの会話inode/bytes/SHAと停止直前更新なしを束縛し、実行中turnを中断していない。
- 停止後：正規DB backup/schema43/integrity、状態更新6files、廃止receiptを追加保全。
  旧daemonは他PTY用途で残るためglobal quiescenceとは主張しない。統括/固定reviewerの2会話を
  `~/.codex/sessions`の同dated pathへ上書き禁止・独立inode・SHA一致でコピーし、原本を保持した。
  通常providerの起動/新turnはまだ未確認。旧会話からの自動統括replayは行わない。
- 旧heartbeat `orca` は追加保全後、正式automation APIでdeletedを確認。旧修復promptからの復活経路を廃止した。
- 通常化の規則/文書変更：agent rule契約pass、関連tooling4tests pass、両索引/diff pass。
  Help Skill実判断はNo impact: 開発環境の規則/文書のみでゲームInputAction/UiIntent/Help producer/consumer/成立条件/文言/assetへ到達しない。
  `HELL_WORKERS_DIFF_BASE=80df44c471a964907fb0fe8b4d0c634d11f33f13`のgateはno production changes。
  full docs gateは既存vendorリンク4件の不合格が残る。ゲームCargo/Clippy/nativeは非対象・未実行。
  storage15:15:27 check pass、allocated680742133760 bytes、legacy0、187batches。追加copy分は切替後に再計測する。
- 固定read-only review後、公式packageを`~/.local/opt/orca-ide/normal-1.4.205`へ独立配備。
  executable SHA `c921ce34c34b674c43401296267d9e9d3d99c111e1663f757c6194e3d749eb6d`、
  app.asar SHA `b3f5690562d487d63410bee3f7eeeeb5a5cd6c6a45bd8e8e5712cc2432012f88`。
  current/desktop/canonical CLI/PATHを同配備へ揃え、旧launch-supervisedは通常起動aliasへ変更。
  controlのdefault-entry接続を除去。独自source/testsは公式配布物に含めず、元forkとsource archiveへ非実行参考原本として保持。
- 通常repo ID `9cc2bd86-38e5-4d4a-9c8c-7a570362cacd`。標準hookのlocal-only/doctor/wait-for-setupを
  新profileだけに保存し、app終了/再起動後にreadback。旧DBやRun ledgerは変更しない。
  既存19worktreeを参照でき、TAK-14課題リンクを標準metadataへ復元。LinearはIn Progressの読取のみで外部writeなし。
- 15:41:34.752〜15:41:49.334：同保存会話`01a0d377-feef-7520-bef5-50565ef282cb`に
  新turn`01a1026d-863b-7243-bedd-6f0ba2417ab0`。primaryのpwd/HEAD/guide読取の実操作とtask_completeを確認。
  初回は同梱Codexのbackground package不足でturn前に終了したため、正式案内のno-daemonで開始した。
  旧通知再送・Enter/旧loop復旧ではない。限定turn終了後は/exitと当該Codex process終了を確認。
- 最終標準設定の固定review：utility SHA `603e117f8c36dc3691889e080aee83d6eb4c1d1bd0603053b4a401c27d23f224` approved。
  PID/birth/exeをpidfd取得後にも照合して今回appのみ終了し、設定backupをfsyncして変更。
  Codex通常defaultをno-daemon/workspace-write/on-requestへ変更し、旧共有コマンド拒否は維持。
  最終app3775634/runtime`3df7e421-2002-4913-8bd5-d8061363cf23`、同公式package、新profile/schema41。
- 専用fixtureの2未使用terminalと自己app/daemonの終了後、`/tmp/orca-normal-UWjm1l`、保全領域内の
  candidate/isolated-configだけを整理。logical bytes計580761736。採用package・元source・会話・旧DB原本は削除していない。
  storage consumerを正式解除しcheck pass、allocated680599842816 bytes、legacy0、187batches。
  保全領域はfeedbackと構想source引継ぎ用途でowner/consumerを保持。全rawの永久保持は契約にしない。
- 15:53:13.733〜15:53:25.676：最終再起動後も同会話の新turn
  `01a10278-30fa-7950-8108-d069c2798439`でprimary pwd/HEADの実操作とtask_completeを確認。
  通常tab`c55d6e69-8427-4f90-a14c-0f311780847d`、terminal`term_25337f6c-208b-437b-b10c-01d5178bf862`、
  incarnation`d1309bc7-ce81-4c4e-87a4-9102ebb125bc`を通常Codex待機として保持。
  graph/runtime ready、旧統括3programのprocessなし、旧他案件daemon/shell継続を確認。
  初回受入の未使用shellとblank browserは正式CLIで閉じた。旧tab/PTYを新tabと同一とは偽らない。
- Fanza repoの登録も正式repo addで復元。旧Fanza PTYと旧profile全設定/履歴は元位置に保持した。
  旧標準automation「毎日の変化のレビュー」は原本保持のみで新profileへ自動replay/新規job生成していない。
  他案件tabの新profile移植や全設定丸ごと移植は実施しておらず、全旧profile互換完了とは主張しない。
  Normal Aの今回受入はhell-workers作業場・必要会話と標準機能の独立動作。製品工程の再開/TAK-14完了は含まない。

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-10-03 | Codex main | 利用者の方針転換に基づき通常化Aと任意外付けBを分離。実境界、6工程、保存/互換分岐、撤去・退避・受入を策定。時間上限なし |
| 2026-10-03 | Codex main | 利用者指示によりB構想を独立repository Orca Conductorへ移管。元本文はbyte同一で移管先へ保全し、hell-workersには案内だけを残した。ゲーム・通常Orca設定・旧原本は不変更 |
