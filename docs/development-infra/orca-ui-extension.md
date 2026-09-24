# Orca本体の受付・役割UI拡張（開発中）

2026-09-24。ユーザー許可のもと拡張ビルドへ一時切替し、実画面からの受入を進めている。
全体の完了条件は[UIライフサイクル計画](../plans/orca-ui-lifecycle-plan-2026-09-23.md)に従う。

## A〜D案件panel schema 2候補（2026-09-25）

host candidate `1312a652`とOrca UI local commit `8b4aa93e`で、案件snapshotをschema 2へ拡張した。
旧schema 1は表示互換を維持し、未知versionはfail-closedにする。新panelは既存role状態に加えて、
現在工程、最終確認時刻、Linear発行判断と理由、待ち理由、次操作、外部同期状態と詳細を表示する。

統括のLinear判断は`none / continue_existing / create_child / create_standalone`の4択であり、
「実装依頼なら常に新規課題」という旧経路を置き換える。project-owned controllerがimmutable decisionを先に保存し、
routeと固定write IDをそのdigestへ拘束する。外部同期は現段階ではdurable intentとUI投影であり、
実Linear/GitHub executorが送信済みという意味ではない。

UI contract unit 10件、web typecheck、変更行lint、schema 2表示E2E、host側Orca tooling 438件が成功した。
Linux packageはglibc 2.31/native検査を通し、隔離actual-windowで固定受付の`Intake`、最終確認時刻、次操作、
外部同期と相談・実装buttonを確認した。配備先は`/home/satotakumi/.local/opt/orca-ide/ui-8b4aa93e`。
desktop iconとCLIは同buildへ切替済みだが、稼働中TAK-14を止めないため旧main processはそのままで、次回通常起動から有効になる。
UI sourceは`stablyai/orca`へのwrite権限がないためlocal commitとして保持する。schema 1への通常profile復帰試験と外部provider受入は未完である。

## 一時配備と復帰

本体sourceは `06cfe378`。配備先は `/home/satotakumi/.local/opt/orca-ide/ui-06cfe378`、
既存のOrca IDEアイコンは同directoryの `launch-supervised` を起動する。
サイドバーの `Reception & Coordination`（受付・統括）が固定入口。
controllerは既存candidateの `scripts/orca_supervision.py`、状態保存先は
`/home/satotakumi/.local/state/hell-workers/supervision-panel`。
実相談の送信、実providerの回答表示、確認付き終了、終了済み案件の表示切替、
アプリ再起動後の受付復帰と終了状態保持を確認した。
実画面の「実装を依頼」から `TAK-11`・専用worktree・統括tabを各1件作成し、
acknowledge後の担当移動と、統括が正常終了した後の画面からの案件終了を確認した。
正常終了をunknownにして操作を塞ぐ不具合は、終了code 0と未完了loopの有無を区別して修正した。
終了の所有照合・idle shell・clean worktree検査は従来どおり実行する。
この受入では統括の対話終了に試験担当が `/exit` を送った。実行中・入力待ちagentの自動停止は未受入。
専用課題はDone、無変更の専用worktreeは撤去済み（実測29,351,936 bytes）。
実装A/Bと固定reviewerを含む全反復ループの受入完了を意味しない。

### 実エージェント反復試験（TAK-12、2026-09-24）

**結果は不合格。反復ループは運用可能と判定しない。** 配備中の同一runtimeへ固定パネルと同じ
`supervision.request` APIで明示実装依頼を送信した。今回は画面クリックの再試験ではなく、
実Codex統括・Codex A・Cursor B・固定Codex reviewerを使うbackend/agent受入である。
基点はcandidate `7f7526c997eb1f4e8abb349bd54b097a60e14e17`。

- 統括が専用課題・統合先とA/Bの分離作業場を準備し、同一Runに2レーンを登録した。
  specの内容ではなくpathを渡す必要がある点で統括が一度登録エラーとなり、自身で訂正した。
  統合先の保持登録漏れは外側担当が指摘し、統括が登録した。無介入での完走とは数えない。
- Aは専用fixtureへ、空白除去と小文字化を仕様とする`normalize_label`を追加した。
  初回のみ小文字化を欠落させる意図的な不良注入であり、製品変更ではない。
  完了通知、provider自動終了、AST検証、checkpoint `afe1c7de1ef7634e9db95d12dcd849c3541b2ca9`、
  固定reviewer起動まで進んだ。reviewerは実際の仕様不一致を指摘した。
- reviewerの`ORCA_REVIEW_JSON`内の受入条件に未escapeの引用符が含まれ、JSON解析が失敗した。
  成功したreview Taskを承認と混同せず、driverは`invalid review verdict`で停止した。
  Aの修正回数は0。同担当差戻し・同session再実装・再レビューは未達。
- Bは正式Dispatchが成立する前の初回promptでfixtureの定数を編集した。
  Cursor用promptにはCodex用と同等のbootstrap待機指示がなく、bridgeも`bootstrap`・authorityなしのまま。
  `worker-start`は不成立、dispatch台帳は`unknown`、Task/Dispatch IDなしで停止した。
  配車エラーの低位理由は保存されておらず未確定。配車前編集を成功成果として採用しない。
- Aとreviewerの2 Dispatchはcompleted、hostのrelease処理は実行済み。
  reviewer完了のDeliveryは停止時に未ACKのまま残り、排出完了とはしない。
  BはDispatchがないためworker-release対象にはせず、差分を保持して所有terminalを明示closeし、
  `ptyKilled=true`とlauncher PID消滅を確認した。role台帳の`starting`は未照合として保持した。
- 全体は`paused`、integrationは`planned`。統合、最終レビュー、通常UI終了は未実施。
  A/B作業場には既定の空shellも残り、担当タブだけに整理できたという受入証拠にもならない。

再現作業場は`tak-12`、`tak-12-worker-a`、`tak-12-worker-b`を同じ場所に保持する。
保持の目的は、配車前編集防止、構造化判定の安全な再提出、停止時の通知処理、空shell整理の修正と再検証。
試験差分・未確定記録を破棄せず、新規Run、手修正した承認JSON、手動ACKで成功を作らない。
ゲームのbuild/test、製品branchへの統合、pushは行っていない。
Help判断はNo impact。変更した2つのPython fixtureは開発用provider試験だけから参照され、
ゲームの入力・状態・asset・Help catalogへ到達しない。

前回のTAK-8は利用者が意図的に作業場を撤去済みで、最終統合承認・全attemptのrelease/ACK・
launcher終了記録を照合した。消失したsourceへの再照合だけでpausedになった旧loopを、
内容を変更せずprivate台帳の`loops/closed/`へ履歴退避した。これは明示保守であり自動終了の成功ではない。

### TAK-12からの受付修正と再受入（2026-09-24）

candidate `9cb4103e`で以下を実装した。旧試験の成功記録へ書き換えず、停止中の作業場・session・差分は維持する。

- Cursor初期promptから実作業本文を除去。controller hookはlive preamble、host arm、
  同一Task/Dispatch/coordinator・terminal incarnationを確認した時だけ`continue=true`を返す。
  配車前の任意promptは`continue=false`、bridgeエラーも明示拒否とexit 2にする。
  hookの15秒timeout内でhost armを最大5秒待つ。実作業とCursorの完了形式は配車specへ渡す。
  拒否契約は[Cursor公式hook仕様](https://prod.cursor.com/docs/hooks)と照合した。
- launcherがreview対象のticket/base/head/source/validationをbridgeへ固定する。
  succeeded `worker_done`の構造化reviewをupstreamへ送る前に検査する。
  JSONまたはfindings形式の不備は送信なしで最大2回まで同一Dispatchの再提出を受け付ける。
  対象hashの不一致、通信・権限不明、予算超過は従来どおり停止する。
  loopの読み取り側も同一parserを使い、後段のsession/source照合は省略しない。
- paused loopでも通知処理を行う。release確認済みの完了だけを通常mailbox経路でACKし、
  新規配車・統合・質問への自動回答は行わない。
- 配車失敗では機械可読なerror codeとrequest IDを診断に残す。
  provider本文、stderr、認証tokenは診断へ転記しない。

関連テスト102件と、比較基点`7f7526c997eb1f4e8abb349bd54b097a60e14e17`からの
変更別`contracts, tooling`が成功した。検証sourceは
`d61e56a6f8a12dbb0547f50e23bca0a19caed27b10a4d23b98c1c2266fe2d08d`。
Help実レビューはNo impact。host開発制御→Orca/CLI/private台帳だけの変更で、
ゲーム入力・UI・保存処理・runtime dataへ到達しない。ゲームテストは実行していない。

初回 `9cb4103e` 時点では既存TAK-12への配備・状態回復と再完走は未確認だった。
candidateから直接旧loopをtickする試行は作業場一致guardで拒否され、通知ACKも行っていない。
既存統括のdriverは旧worktreeのmoduleを保持しているため、candidateの更新だけでは切り替わらない。
次は旧subject/session/終了証拠を保全する回復・配備経路を整え、同じ試験環境で再受入する。
新規Runや手書き承認を使って未完の試験を迂回しない。

停止中の保守は `scripts/orca_maintenance_recovery.py` が所有する。統括の正常終了、全Dispatchの
settlement/release、全作業場のsource fingerprintを照合し、通常のmailbox経路で通知を処理する。
tooling以外の変更・生存中process・未確定Dispatchがあれば更新しない。Aのcheckpointと会話を保持し、
Bの未配車差分はprivate回復台帳へ保存してexact patchのみ取り消す。正のterminal close証拠を
終了根拠とし、終了code 0や成功Taskを生成しない。更新後は同じRun・作業場・provider sessionで
新subjectを再検証・再レビューする。保守中断時は再適用せず照合が必要。
統括の `launch --resume-session` は保存会話のIDとcwdを確認してから起動状態を変更する。
Help判断はNo impact（host管理と開発試験に限定、ゲーム側の入力・状態・文言の変更なし）。

再受入で、固定reviewerの同じterminalを使った2回目の通知を、旧Dispatchとの曖昧性で
停止する問題を確認した。通知の帰属はterminal単独ではなく、bridgeの確定送信receiptと
Task/Dispatch/Run/本文の一致で決める。複数の確定owner、本文改変、権限不一致は拒否する。
`resume-inbox`はexact停止台帳と全通知の確定receiptを再照合する保守で、手動ACKは行わない。
統括の実作業場は起動cwdとOrca terminal identityから照合し、host制御コードの配置場所と分離する。
dispatcherも制御コードと同じ配備のrole launcherを使う。凍結中のreview対象を更新せず制御を修正できる。
Cursorのhook生成format再提出では、beforeSubmitPromptが省略される自動followupも、
同じ会話に対する未消費のhost発行retryが1件ある場合だけ次generationへ結び付ける。

TAK-12は同一Runで差戻し1回、同一A sessionで修正、A/B承認、統合検証、固定reviewerの
最終承認まで到達した。統合HEADは `903c6fa6f8d55a1f8fdd939613ceb3f556c333a4`。
8 attemptのrelease/ACKと通知排出を確認した。途中の回復とBの送信・結果再提出には
試験担当の介入があり、無介入完走の証拠ではない。B起動時は復元画面のidleだけでなく、
新controllerが初期bootstrapを拒否したhook receiptを待ってから配車する改修を追加した。

終了時、長時間使用したtabのring bufferから先頭行が既に退避されていると、全履歴のcursor 0を
要求して閉じられなかった。保全対象は現在保持されている `oldestCursor..latestCursor` 全行と
最終preview。事前欠落を `truncated` / `droppedBeforeCursor` に明記し、元の全履歴を保存したとは
記録しない。読み取り中のrange変化、行の欠落、所有不明・busyなshellは引き続き拒否する。
provider会話と確定Dispatch/review記録は別の既存台帳に保持し、closeで削除しない。

TAK-12の全所有tab（担当・review 5件と統括1件）のcloseを確認し、アプリ再起動後も
closed表示・担当handleなし・固定受付readyを確認した。再起動後の新規受付で、controllerの
cwdがGit外の場合にprimary探索が失敗する問題を検出した。primaryの探索は配備した制御コードの
Git common directoryから行い、terminalの作業場照合には引き続き実cwdを使用する。

TAK-13ではCursorの起動・編集・通常文からJSONへのhook自動再提出が送信補助なしで成功した。
個別reviewと統合検証の後、最終reviewがprovider testのREADY初期値固定との矛盾を検出した。
provider testは書込scopeの制限とRESULT文字列の構造を検査し、編集受入後にも初期値を要求しない。
指定した最終値の一致は、その案件のlane・統合validationで引き続き厳密に検査する。

worker scope外のhost tooling補正は `scripts/orca_tooling_correction.py` を統括が明示実行する。
exact loop digest、封印済みの統合差戻し、全attemptのrelease/ACK、clean対象、元worker承認を照合し、
元baseから派生するtooling commitだけを候補へ加える。ゲームpath・worker fixture変更を拒否する。
旧Run・拒否review・元receiptを保存し、新headのvalidationと固定reviewを通常driverへ戻す。
補正自身は承認を生成しない。中断・同じ補正の再実行は照合待ちとなり、未知のGit操作を再送しない。

補正後のTAK-13は同じRunでprovider testとRESULT完全一致の検証に成功し、同じreviewタブ・
固定sessionで最終承認された。HEADは `4bc2c0c177b8f5fa23c331c5858d4a98a6a4ab89`、
sourceは `603eb4e6bf50e2244ef19bf97c33208a9fb4026d6a9276762dda88486f20d0fc`。
全4 attemptのrelease/ACK・inbox排出・同一subjectの承認照合を外側から再確認した。
Cursor部分は無介入成功だが、最終review後のtooling補正は統括の明示介入であり、全工程無介入とは記録しない。
案件close APIで全所有role tabと統括tabが閉じ、未使用の試験shellも個別の所有・出力保全を経て撤去した。
受付はready、TAK-12/13はclosed、試験用の生存terminalは0件。

修正コードはcandidateの `8853d305`、`c6597856`、`66d7e9ba`、`2c4efde5`、
`410174e8`、`688411c6` にcommit済み。最終変更別検証はbase
`2c4efde50a2d8f36c4b3b4e299083314e0616b37`、source
`c9329cccd1870cd14c17a909bb85095958ed1bf7db0c1623f588e7f4113cc5f3`でcontracts/tooling成功。
Python tooling 651件、Blender tooling 164件、Ruff/actionlint、perf self-testを含む。ゲーム検証は対象外。
Help実レビューはNo impact（hostの制御・テスト契約のみで、プレイヤーの入力・表示・状態・runtime dataは不変）。

完了したTAK-12の3作業場とTAK-13の2作業場、および今回のcontroller再読込前の重複設定backupを撤去した。
削除前のdu合計は230,502,400 bytes（約220 MiB）。Btrfsの実空き容量増加とは区別する。
両試験の統合branchとcommit、provider会話・承認/close receipt、元の切替前設定は保持している。
Orca本体source、現行controllerを読み込むcandidate、稼働中terminal daemonのpackageは現在の実行依存として保持する。

復帰先の現行版は `/home/satotakumi/.local/opt/orca-ide/1.4.205/squashfs-root/orca-ide`。
終了後・切替前の設定を `/home/satotakumi/.config/orca-pre-ui-bb614da7-20260924`、
起動アイコンを `/home/satotakumi/.local/share/applications/stably-orca.desktop.pre-ui-bb614da7` に保全した。
CLIの元symlinkは `/home/satotakumi/.local/bin/orca-ide.pre-ui-bb614da7` に保全した。
設定backupの実測は78,557,184 bytes、保持台帳は `orca-ui-switch-rollback`。
稼働中の既存terminal daemonが最初の試験packageを参照しているため、
`/home/satotakumi/tools/orca-ui-lifecycle/.git/orca-ui-acceptance/package` も
`orca-ui-live-daemon-package` として保持する。参照daemonと端末の利用終了後に撤去する。
終了処理修正のcandidateは `7f7526c9`、変更別contracts/toolingと回帰40件はpass。
復帰時は通常終了後に元アプリと起動アイコンへ戻す。設定全体の復元は切替後の依頼・状態を消すため、
必要性を照合し、現在設定も別途保全してから実施する。
保全のownerはOrca統括、consumerは一時配備の復旧、next_actionは実受入と採否確認、
release_whenは拡張版採用または復帰を確定し最後の復旧用途が終了した時点。

Linux packageはCLI依存の展開不足とNode組込みmodule判定を修正した。
このホスト向けpackageでは、同じnode-pty 1.1.0・Electron ABI 148の現行配布済みnative binaryを使用し、
glibc互換性検査を含む通常の梱包検査を通した。Fedora上でのnative再ビルド単独はglibc 2.42依存のため
配布基準を満たさず、一般配布用の再現ビルドを受入済みとはしない。

## 対象と保全境界

- source: `/home/satotakumi/tools/orca-ui-lifecycle`
- branch: `feat/hell-workers-ui-lifecycle`
- upstream: `stablyai/orca` tag `v1.4.205`、base `11aba8bdc5e492d3ba01fc7fe333495ace74128f`
- 元のOrcaアプリと切替前設定を保全し、拡張版を別directoryから一時起動する。既存課題・作業場は受入の編集対象にしない。実経路受入で作成した専用試験課題`TAK-9`はDone、専用worktreeは成果・所有照合後に撤去済み。試験状態は保護台帳に保持する。
- 検証はOrcaの隔離Electron fixtureと一時テストrepositoryを使用する。ゲームのbuild/testは対象外。
- `build:cli`末尾のglobal CLI installerは実行しない。Node 24 / pnpm 12は隔離取得し、system Nodeを置換しない。

## 本体側の責務

サイドバーの固定「受付・統括」からパネルを開く。案件ごとに工程、待ち理由、統括・A・B・レビューの
状態と移動ボタンを表示する。未割当の担当を表示するために端末を作らない。
終了案件・保守・検証保持は既定で隠し、明示操作で表示する。ただし「要確認」は用途に関係なく残す。
隠す操作ではfilesystemを削除しない。この分類は新パネル内のもので、既存作業場sidebarの移行はまだ行わない。
パネルの×は表示を閉じるだけで、agentや案件を終了しない。

`supervision.read / supervision.navigate / supervision.request`を本体の型付きRPC registryへ追加する。
専用private directoryは`ORCA_SUPERVISION_STATE_DIR`で明示指定する。未設定・古い情報・別runtimeは
操作不可として説明を表示し、既存運用のstate directoryへ勝手にfallbackしない。
初期実装はPOSIXのowner/mode照合が可能な環境に限る。それ以外は未対応理由を表示して拒否する。

本体は実行状態の正本にならない。project-owned controllerが投影したsnapshotを読み、終了等の要求を記録する。
要求の受付成功と、担当起動・中断・終了完了を区別する。UIから直接shell commandや強制終了を送らない。

### project-owned bridgeの現状

`scripts/orca_supervision.py`を開発中のOrca本体からopt-inで起動する橋渡しを追加した。
`ORCA_SUPERVISION_CONTROLLER_SCRIPT`、`ORCA_SUPERVISION_STATE_DIR`、
`ORCA_SUPERVISION_ORCA_CLI`をすべて絶対pathで設定した時だけ動き、runtime IDをOrcaの
`status --json`と照合する。一時配備のlauncherにこの3設定を明示した。

bridgeは固定受付と受付台帳の案件をsnapshotへ投影し、UIからの新規依頼を操作UUIDで
`orca_frontdesk.py`の既存台帳へ重複なく取り込む。receiptとconsultation起動intentを永続化し、
パネルに一覧するのは、このパネル自身の受付receiptで所有が確認できる案件だけである。
別経路の旧相談・手動取込を固定受付へ紛れ込ませない。
読取専用の`orca_coordinator.py`相談を順次起動する。起動・終了が不明な場合は自動再送せず
「要確認」とする。相談回答は案件詳細へ投影するが、実providerを使った受入は未実施。
相談は実装dispatchでも、Orca上の可視「統括」agent tabでもない。ここを同一視しない。
「実装を依頼」は相談と別の明示actionであり、`scripts/orca_supervision_route.py`が
元依頼のreceiptとimmutableな受付判断を確認し、必要な場合だけ内部Linear課題と分離worktreeを一度だけ作成する。
段階journalの`issue_creating`/`worktree_creating`で結果が不明なら自動再送せず要確認とする。
linked issueから既定タブに可視統括が登録され、作業場・課題・terminal identityが一致した場合だけ
パネルの統括移動ボタンを有効にする。担当A/Bとレビューは既存のguard付きdispatcher以外で起動しない。
作業場作成後は子受付をimportし、`terminal_starting`を永続化してから既存の
`ensure_coordinator_terminal`で可視統括タブを一度だけ起動する。背景起動はフォーカスを奪わず、
応答不明なら二重起動しない。作業場・課題に一致する統括のacknowledgeを読んだ時だけ`ready`へ進む。
`orca.yaml`の既定タブだけに起動を期待せず、既存tabがあれば所有照合して再利用する。

橋渡しと既存frontdeskのPython単体テスト34件、Orca本体のnode/web型チェックと変更行品質ゲートは通過した。
隔離した非表示ElectronのUI/APIシナリオ4件に加え、controllerを接続した1件で
「画面の送信→隔離受付台帳の1件→同じUUIDのreceipt／相談起動intent」を確認した。
後者は実ユーザーの受付台帳を参照せず、実providerを起動しない試験モードである。
証跡は検証台帳の`orca-ui-lifecycle-panel-20260923-9`（UI/API）と
`orca-ui-lifecycle-panel-20260923-16`（controller接続の相談・明示実装2経路）に記録した。
`attempt-16`では独立証跡検証器も通過した。旧`attempt-12`は保持を解除して撤去済み。
実経路試験`TAK-9`では、Linear課題1件・分離worktree1件・可視統括tab1件の作成、
統括のacknowledge、routeの`ready`、同じ依頼の再実行後もtab1件であることを確認した。
これは稼働版に固定受付パネルを配備した証拠ではなく、A/B/review配車・終了の受入でもない。
旧promptが試験課題を別課題へ誤handoffしたため、`TAK-10`が余分に作成された。
候補`d5992437`は固定受付からの由来記録を使って再handoffをprompt・実行gateで拒否し、
handoff時のSetup tab生成も抑止する。`e68c22e3`は作業場作成の`--activate`を外し、
統括tabだけを前面起動する。`TAK-10`は誤作成と記録してCanceledへ移し、clean確認後に
専用worktreeを非forceで撤去した。この失敗を正常な引継ぎ受入には数えない。
`TAK-9`は統括が正常終了した後のclose journal・長い出力のcursor保全・pane close receipt・
tab 0件の再読までcontroller実経路で確認した。試験課題はDoneへ移し、cleanな専用worktreeを
hold consumer解除後に非force撤去した。固定パネルの実配備から閉じた証拠ではない。

### 状態契約

`src/shared/supervision-contract.ts`がwire schemaの正本。

| 項目 | 契約 |
| --- | --- |
| snapshot | schema 1、runtime ID、生成時刻、案件一覧。256 KiBまで |
| workflow | UUID、revision、title、kind、工程、理由、現在受理可能なaction、4役割を各1件 |
| kind | 受付／案件／保守／検証保持 |
| 工程 | 受付済み／受付可能／作業中／レビュー中／確認待ち／休止／終了処理中／終了／要確認 |
| 役割 | 未割当／起動中／実行中／待機中／休止／プロセス終了／要確認 |
| terminal identity | handle、incarnation、worktree、execution host。表示名から推定しない |
| request | 期待runtime ID、operation UUID、workflow UUID、revision、action、依頼本文（相談submitと実装implementだけ） |

生成後15秒を超える情報、時計の不正な先行、schema/所有権不明は操作不可。
UIはRPCが応答しなくなった場合もローカル時刻で有効表示を失効させる。
controllerは時刻更新だけではrevisionを変更せず、受付・route・統括・監督loop・lifecycleの
台帳内容が変われば世代付きfingerprintを変更する。古い版の操作はreceiptを`rejected`として消費し、
他の案件や次の要求を止めない。

### 移動と終了要求

- 移動・要求とも画面が確認したruntime IDを必須とし、Orca再起動前の操作を拒否する。
  移動時はmain processが実端末を再取得し、incarnation・作業場・host・接続を照合する。
  rendererにも同じPTYの既存tabがなければ停止する。自動wake、resume、空tab生成へ迂回しない。
- 終了は確認画面を通して要求を記録する。成功表示は「統括の処理待ち」であり「終了」ではない。
- requestはprivateな`requests/<operation UUID>.json`へ、全内容とfsyncの後に排他的に公開する。
  同一ID・同一内容の再読でもdirectoryを再fsyncしてから成功とし、内容変更は拒否する。
  古いrevisionを使う新規要求は拒否する。再送の同一性は現在のパネルを開いている間の確認までであり、
  下記の永続結果照合が接続されるまではパネル再表示をまたぐ二重操作防止を保証しない。
- UI/APIの受理可能actionはcontrollerが提供するものに限定する。`closed / closing / unknown`では受理しない。
- 候補`d41887aa`は、実装経路が`ready`で統括が所有するidle shellに戻った場合だけ
  中断・再開を受け付ける。`56accc4e`では、監督loopが承認済みで全attemptがrelease済み、
  最終reviewが承認済み、通知が排出済みの場合に限り、登録済みA/B/reviewerのterminal identityと
  cleanな各作業場を照合して担当paneを直列に保全・終了し、最後に統括paneを閉じる。
  終了対象はlifecycle journalに固定し、応答不明時は既存close receiptとtab不存在を読んでから再開する。
  `d5572d8f`では元shellのPID・開始時刻もreceiptへ保存して終了を照合し、後からdirtyになった
  担当作業場があれば残りのcloseを保留する。
  `4ad62830`は終了済み担当を画面で「未割当」と誤表示しない。`8ca7b917`は所有する作業場の
  Orca board statusを`completed`へ移して再読する。作業場の削除や他案件のstatus変更は行わない。
  `3181baba`は同じ担当の旧タブが正規receipt付きで置換された場合を識別し、未確定の旧タブが
  一つでもあればcloseを止める。
  稼働中loopでは統括だけの中断・終了をUIに出さず、agentを強制終了しない。
  操作はworkflow revisionと同じoperation IDで照合し、拒否理由／結果不明をsnapshotへ表示する。
  本体`9fb9f672`はrevisionが変わった後にだけ操作待機を解除する。
- 不明な終了の再送、process停止、通知ACK、所有資源解放、履歴保全、pane close、統括自身の最後の終了は
  controller/finalizerの責務。単なるqueue書込みをこれらの完了証拠にしない。

## 未実装・未受入

本体UI/APIだけでは全自動運用の完了としない。一時配備で次の接続・受入を進める。

1. 実Orcaの課題・worktree・可視統括タブ作成は専用`TAK-9`で確認した。固定受付パネルからその経路を操作し、画面のrole移動まで通す実受入は未実施。
2. queue→受付台帳の消費と相談起動intentは隔離Orca画面から確認したが、実providerの相談結果照合、パネル再表示／アプリ再起動をまたぐ受入。
3. 限定的なidle案件の中断・再開・終了候補と、承認・release済みloopの担当タブを直列に閉じる候補は実装した。worker/reviewerを経た実案件での終了、稼働中agentの安全停止、再起動後の部分失敗復旧、close journal/finalizerの全面受入は未実施。確認待ちの作業場とcacheは保持する。
4. 実providerによる新規依頼→役割移動→差戻し→確認待ち→再開→最終終了。
5. 既存作業場の非破壊移行、配備版の選択と復旧手順。

fixtureのsnapshotは画面/API検証用であり、実agent稼働や実案件終了の証拠ではない。
検証結果は計画書へ集約する。開発clone/cacheは同じ場所を修正・受入まで保持し、失敗jobを無期限には保持しない。

### TAK-14後の受入更新（2026-09-24）

- デスクトップ起動時に固定パネルだけが見え、プロジェクト一覧が消える回帰を修正した。
  通常起動は標準Orca画面を維持し、固定受付はサイドバーから開く。非表示検証用の環境変数を通常launcherへ残さない。
- `launch-supervised`はcandidateの`orca_ui_coordinator.py`を絶対pathで渡す。
  課題worktreeに同scriptがなくても統括のdefault terminalが起動でき、既存windowへのsecond-instanceは再表示に使う。
- TAK-14で同一Runの実装A、固定review、統合、最終reviewまで承認した。最終承認後の登録済みrole tab 3件と、
  worker作業場に残った所有確認済み補助shell 2件は、出力保全とpositive closeを確認して整理した。
  親作業場には統括1tabだけを残した。
- 全Python基盤試験669件が成功した。ゲーム実装テストはユーザー指定により対象外。

残る受入は、次の新規案件を固定受付から開始し、spec/ref訂正、外部保守resume、追加ID入力が0回の
cold-startを測ること、および実行中agentをユーザーが中断した場合の安全停止・再起動後復旧である。
今回のTAK-14は途中停止を同一Runで恒久修正して完了したため、無介入開始の成功証拠には読み替えない。
