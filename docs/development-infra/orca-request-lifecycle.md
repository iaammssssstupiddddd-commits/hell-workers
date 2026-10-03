# Orcaの工程承認と依頼全体の受入

> 歴史資料（2026-10-04整理）。独自統括は廃止済みで、以下のcommand・Run・復旧経路は実行しない。
> 現在の規則は[通常Orca運用ガイド](../orca-quickstart.md)。旧未完条件と過去の結果は履歴のまま保持する。

更新日: 2026-09-26。実装候補の契約を記す。通常運用への切替・全経路受入は未完了。
進行と未実装範囲は[横断是正計画](../plans/orca-request-lifecycle-correction-plan-2026-09-26.md)を正本とする。

## 完了の境界

`orca_review_loop`の`approved`は、一つの世代の実装・調査と統合後reviewが承認されたという意味である。
依頼全体の完了ではない。`orca_request_lifecycle`は、このloopとは別に原依頼・目的・採用仕様・
条件ID・工程依存・scope revisionを保存する。旧loopから全体承認を推定しない。

- 最後の`acceptance`工程は、全条件とすべての先行工程を対象にする。
- 工程とloopは`request_binding`のscope digest/node IDで対応する。同一工程の二重登録と未完の依存を拒否する。
- 原依頼と全条件は実装ticket、統合後の固定review ticketに伝播する。全体reviewでは直近diffだけでなく、
  最新ソース全体の未実装・退行・未検証を阻害指摘にする。
- 全体受入にはimmutableな祖先loopのsealed review、現行HEAD/sourceに対する固定review、
  integration receipt、通知drain、担当の終了確認を要求する。過去の条件を後続ソースへ無条件に流用しない。
- no-change調査の承認は調査工程の結果に限定する。空diff、正常exit、cleanなGitだけでは依頼を完了できない。

## 内部APIと追加指示

操作は統括が行う。利用者にID、spec path、CLIの入力を要求しない。

`orca_review_loop.py request-plan`はprivate specを受け、登録済み受付の原文をhost側で取得する。
specは`objective`、`specification`（出典付き文字列配列）、`criteria`（IDから条件へのobject）、
`nodes`（`id/title/kind/criteria/depends_on`）を持つ。nodeは依存先の後に列挙し、
kindは`implementation`、`investigation`、`acceptance`から選ぶ。最後だけを`acceptance`にする。
loop specには登録結果の`scope_sha256`と今回の`node_id`を`request_binding`として渡す。

既に統合承認された未結合loopには`migrate-scope`を使う。specは`loop_sha256`（inspection digest）、
`request_binding`、結合理由`reason`。同じ可視統括、現scope、activeな実接続、通知drain、全attemptの
終了／正の復旧証拠、cleanで不変の統合HEAD、封印review、保存済みの同一source検証を照合する。
現経路はcommit済み統合だけを対象とし、依存未完のnode・全体acceptanceへの直接移行・結合済みloopを拒否する。
元loopと承認をwrite-ahead receiptへ保存して、同Run・同HEADで全体context付き固定reviewを行う。
旧reviewへのbinding後付けは承認にならない。再レビュー後にだけ通常の次工程判定へ戻る。
同一要求の応答消失はexactな前後状態で照合し、進行後の再送や変更済みsourceを再実行しない。
未結合状態は照合待ちであり、進行driver自体を例外終了させない。

追加指示は、terminalへ送る前に原文とoperation IDをprivate receiptへ記録する。
`accepted`は送信受理だけであり、適用確認ではない。未適用入力が残っている間は全体受入を拒否する。

統括は`reconcile-instructions`へ`expected_scope_sha256`、`instruction_ids`、`reason`、`definition`を渡す。
同scopeの単なる継続・説明なら`definition:null`とし、判断根拠を保存する。
追加条件がある場合は改訂definitionを保存し、旧definitionとdigestをhistoryへ残す。
同じoperationの再実行は同じ適用結果を返す。旧scopeの工程承認は新scopeの依存を満たさない。
目的変更・既存条件の削除/縮小をこのAPIで暗黙に認めない。それらの権限・中止経路は後続実装で扱う。

## 終了・外部writeの防御

全体受入の検査は、通常のclose preflightだけでなく、終了実行、終了途中の復旧、
worktreeのcompleted反映で再実行する。旧版の終了journalを使って検査を迂回できない。
Linear Done intentの生成と実行にも同じ検査を入れる。旧版が作ったqueueも送信前に再確認する。

依頼PRの生成と送信も全体受入へ拘束し、PR head / tested SHA / review SHAが全体受入のheadと
一致することを要求する。部分工程はローカルcheckpoint・統合後reviewとして保存し、
親依頼のPRを部分完了の通知に使わない。GitHub連携はclosing keywordがなくても関連付けで
状態を更新し得るため、本文からkeywordを消すだけの防御にはしない。
このgateはproject-owned executorの範囲であり、人手のPR作成・組織の連携設定を制御するものではない。

### 外部完了による配車前停止の復旧

`resume-linear-state --expected-sha256 <showのloop_sha256>`は統括用の内部操作である。
外部のDone/取消を無視したり、勝手に再開状態へ変更したりはしない。外部状態の変更が必要なら
利用者の明示指示に従って行い、同じissueを再読したあとで復旧する。

- 対象は可視統括が所有する同一loop、未dispatchのplanned/dispatchingからの既知の停止だけ。2担当の同時停止も一括照合する。
- exact digest、空の未処理inbox/attempts、ticket/source不変、最新Linear activeを要求する。
- 認証不明時のcached-active fallbackを復旧には使わない。scope登録済み案件は外部仕様照合も要求する。
- 変更前後のreceiptを先に記録し、応答消失後の同じ要求は重複遷移しない。進行後に同じ要求を再送すれば拒否する。
- 新しいRun、Task、worktreeを作らず、既存driverに同じ工程の続きを委ねる。未知Dispatchの再送には使えない。
- loop内のattemptsが空でも配車未実施とは推定しない。dispatch journal・role launch/bindingが残れば復旧を拒否する。
- 古いPR queueのheadが全体受入headと異なる場合は`blocked_policy`を保存し、既存supersede経路で解消する。
  送信済みか不明な操作はread-backを維持し、未送信扱いに変更しない。

連携された案件の空outboxは「同期不要」ではなく「同期確認の記録なし」と表示する。
この表示だけで接続正常・同期完了を判断しない。
画面は既存の状態行と実terminal履歴を使い、追加の大きな結果UIは作らない。

## 通常driverとcontrollerへの接続（候補batch 2）

### 担当通信の停止と質問の再照会

担当の正式な完了通知を受理した後、launcherは同じterminalのidleを確認して自身の子processだけを終了する。
idle観測は10秒を上限とし、Orca本体の2秒pollと3秒quiet windowをまたいで待つ。
1秒で観測を繰り返すと、復帰済みpaneでは最初のpoll前に毎回期限切れになり、永久に進まないため使用しない。
timeout・通信不明は終了許可ではない。exactな完了receipt、runtime、terminal所有、idle、終了後sourceの検査は維持する。
既に旧launcherで待ち続けている担当は修正ファイルを再読込しないため、受理済み完了とidleを照合して正常終了し、
既存launcherのcheckpoint記録を経て同じloopの検証へ渡す。新しいTaskやDispatchで代替しない。

工程間の`finalize-tabs`は全attemptのsettlement/ACKとexact loopを照合し、
最新固定reviewを結果terminal、その他の登録済み担当terminalを依頼履歴として保持する。
履歴保持はprocess終了・slot解放・再利用許可を意味しない。Orca inventoryと登録identityを照合し、
未確定retirement journalがあれば停止する。close/send/idle待機を行わず、
後継工程の開始を旧表示タブの破壊的整理に依存させない。再配車時の所有・idle guardは維持する。

統括の`request-status`、外部scope照合、工程開始前の接続確認は、常駐driverが
request-runtime lockを使用中なら同一lockの取得を最大30秒待つ。0.1秒間隔で正規取得を
再試行し、期限切れはbusyとして返す。driver自体は非待機のまま次tickへ延期する。
lockを削除・強制解除したり、隔離namespace内で所有者が見えないことを終了証明にはしない。

候補worktreeのstorage保持登録不足は製品実装の不備と区別する。最初のcontracts/storage gateが
exact候補pathの未登録だけで終了した場合、retry ticket発行前に統括準備待ちへ移す。
判定は非切詰めdiagnosticだけでなく、canonical検証argv、現ticket・slot・source、証拠digest、
保存host record、現host runnerの一致を要求する。混在エラーや別対象の証拠では復旧しない。
統括が正式な保持登録を行った後、既存`resume-coordinator-validation`が同じ証拠・担当の
settlement/release・role bindingを再照合し、同じsourceの検証へ戻す。実装revisionを消費せず、
storage検査自体を省略したり成功へ書き換えたりしない。実行中driverは自動reloadされないため、
候補コードの検証と新driverへの適用確認は別に行う。

checkpointと統括所有の文書補正は、共通のticket生成器でcontext packageのbaseとdigestを
新HEADへ結び直す。仕様・受入条件・権限・計画世代は変更しない。packageのgenerationは
loopの計画世代であり、ticketの試行generationと同一にしない。base不変の検証差戻しはpackageを維持する。
committed generationの読取でも正規の後継ticketと元担当／次担当の割当を照合する。

旧writerがref/index更新後にcontext不一致で停止した場合は、`resume-checkpoint-context`が
exactな停止digest、可視統括、同担当のsettlement/ACK、成功検証、既存checkpointを照合する。
旧prepared ticketが元ticketをそのまま複写した既知形状に限り、元journalとhashを別receiptへ保存し、
base/digestと次割当だけを更新する。候補commitの親/tree、実HEAD/index/source、元割当、
世代receiptの非競合を要求し、未知変更は拒否する。成果commitや検証を作り直さず、
同Runの固定review待ちへ進める。これは固定review承認や依頼完了の代わりにはならない。

2026-09-27の実受入では、TAK-14の同一Run・計画世代5・統括sessionを維持してこの復旧を実行した。
既存成功検証とcandidate `fee68f391eda3d67a37694de944d6d54806fc053`を再生成せず、
prepared checkpointのcommitted化、laneのreviewing移行、固定reviewerの実差分読取開始、
Orca表示「実装差分を固定レビュー中」を確認した。統括は正常終了後に同じタブ・会話へ
新driverを読み込み、検証中だけのheavy保留も解除済み。旧driverの自動reloadとは扱わない。
基盤差分は固定read-onlyレビューAPPROVED、重点35件と変更別contracts/tooling
982+164件・perf self-test・Ruff・actionlintがpass。全群の同一sourceは
`f0903962585331cc997f5d213d02b914a5c1367b74decdfa963ea00bcc9a66af`、
比較基点は`ae5b2066c4fd39a8dd6a6c439e272d125f749241`。
Help実レビューはNo impact（host制御・記録のみでゲーム内入力／表示／Help consumerを変更しない）。
この受入は基盤停止の復旧までであり、TAK-14全体の完了や製品review承認ではない。

別途明示した担当tabの破壊的整理では、launcher終了後に残るforeground shellにTUI idleを要求しない。
同一terminal所有・cwd・foreground・同session子processなしをOSで正に確認し、close直前に同一shellを再検査する。
shellの証明ができない場合だけ有界10秒のTUI idle観測を使用する。timeoutでcloseを許可しない。
scrollback保全・exact incarnation・positive PTY closeの既存検査は維持する。

回答取得済みの同一質問を再度resumeしても、別の未回答質問がなければ、同じDispatchの
所有・sourceを再確認して確定済み回答を返す。外部への質問送信を繰り返さない。
未知質問、別質問待ち、所有・能力の変更、結果不明の通信は引き続き拒否する。

未取得の質問回答・未ACKのDeliveryが残る状態の正常な`worker_done`は、送信前に
`communication_pending`と対象IDを返す。この入力順序の誤りだけではbridgeを失効させない。
担当は同じDispatchでask resume／Delivery処理へ戻り、その後で完了通知を再提出する。
所有・source変更、未知の通信結果、不正な完了形式の防御は緩めない。

bridgeがunknown/revokedの場合は、provider終了とrole lease解放を待たず異常を検知する。
loopをpausedにし、既存担当タブを保持して通信停止を投影する。「開始確認待ち」へ戻して隠さない。
正常なsettled bridgeの失効は異常とせず、別途completionの所有・終了・成果検査を通す。

内部の`resume-bridge`は、exact loop digest、可視統括、空inbox、最新Linear active、
通常はcleanかつ不変のsource、正のworker/launcher終了証拠、同じprovider sessionを要求する。
編集後に通信だけが停止した場合は、統括が現在の差分を確認して
`--inspected-source-sha256`を明示したときだけ、そのexact sourceを保存して復旧できる。
この経路は記録済みlauncher終了、元ticket/HEAD・編集許可範囲・未変更index、同一sessionを追加照合する。
開始時sourceと現在sourceをreceiptに併記し、復旧後の再検証・固定reviewは省略しない。
通信履歴は全件confirmedで、heartbeat・空check・回答済みaskだけを許可する。
pending操作、未回答質問、未処理delivery、worker_doneを含む履歴はこの復旧では扱わない。
旧試行をreceiptへ保全し、同じRun/Task/sessionへ新たな権限付きretryを準備する。
旧bridgeを再有効化したり、新しいTaskを作って失敗履歴を回避したりしない。

担当の進捗種別`status`は未対応であり、勝手にheartbeatへ変換して送信しない。
正しい認証・live所有・Task/Dispatch・heartbeat相当の入力shapeを照合できた誤用だけ、
上流送信前に`message_type_retry`を返す（最大2回）。同じDispatchでheartbeat／ask／
worker_doneへ訂正させ、入力の誤りだけで通信を全面失効させない。
この応答は送信未実施の証明であり、未確定mutationや権限不一致の再送許可ではない。

2026-09-27のTAK-14実復旧では、旧Dispatchを正常provider終了後にfenceし、正式な
`resume-bridge`で変更3ファイルを保持した。現在sourceを明示し、既存checkpointの世代receiptと
元sessionを再照合したうえで、同Run/Task・同じ実装Aタブに後継`ctx_f9510ede7c97`を配車した。
実装Aの再開応答と差分読取を確認した。旧失効bridgeを再有効化せず、未検証成果を承認扱いにしない。
同Dispatchの`worker_done`をconfirmed/completedで受理し、同sessionのprovider exit 0と
変更前後で同じdirty sourceを確認した。統括は後続の新差分Help影響レビューへ進んだ。
基盤の固定read-only reviewはAPPROVED、重点70件と最終contracts/tooling 996+164件、
perf self-test・Ruff/actionlintがpass。比較基点は`ae5b2066c4fd39a8dd6a6c439e272d125f749241`、
最終sourceは`a3654e6a36aeac5c0768be7d376f1c52559145f19047b1528827a601bd615609`。
検証中に差分を追加した先行2回のgateはsource変更で不採用とし、この最終結果だけを採用した。
Help影響はhostの通信・再開のみのNo impact。製品の検証・固定reviewとTAK-14全体完了は別工程である。

統合後の固定reviewには`resume-review-bridge`を使う。上記の正の終了・所有照合に加えて、
全実装laneの承認、現scope結合、統合receiptと同一sourceの検証を確認する。
review ticketは通常生成器、または照合済みscope移行receiptのspecから再構築し、
id・base・検証証拠・全文promptを含めて保存ticketと完全一致させる。
この経路だけは、送信結果がconfirmedのaskで回答未取得でも、旧Dispatchのfailed/fence、
provider終了、空inboxが証明済みなら回復可能とする。pending操作・settlementは引き続き拒否する。
同じRun/Task・固定reviewer session・tabを維持して新しいDispatchへretryし、結果の再提出を要求する。
失敗試行を成功・ACK済みに書き換えず、照合可能な復旧receiptとして履歴に残す。

復旧先reviewerの起動後、`worker-start`が`agent_unconfigured`で配車前拒否された場合は
`resume-review-start`がexact停止digest、元Taskの最新failed Dispatch、clean source、固定session、
現scope、未arm/無操作bridge、端末incarnationとlauncher固有leaseを再照合する。
外部送信前に記録し、同じrequest IDで一度だけ再照会する。未知結果は再送しない。
成功応答を保存した後のローカル中断は、外部Dispatchを再読してarm/台帳投影だけを再開する。
この経路はagent認識自体の緩和ではない。認識拒否が継続する場合は識別情報の生成経路を調査する。
2026-09-27は隔離wrapperの内側にいるCodexがnamed sessionの名前だけをOSC titleへ出し、
本体からforeground providerを識別できないことが原因だった。supervised Codexの通常起動・resumeに
`tui.terminal_title=["app-name","run-state","activity"]`をプロセス限定で指定する。
Codex自身のidentity/status通知を使い、任意のPython processやtui-idleだけをagentと認めない。
既存の稼働reviewerは公式`/title`で同じ項目を選び、同session・端末incarnationのまま
`terminal.agentStatus`が`isRunningAgent=true, status=idle`へ変わったことを確認した。
本体の安全判定・画面layout・ユーザー共通設定は変更していない。
外側JSON-RPC相関IDは`rpc`、耐久mutation IDは`error.data.orchestrationRequestId`由来の
`request`として別に記録する。耐久IDがない場合は相関IDを再試行権限に代用しない。
既存sending WALの配車前拒否を主担当が直接観測済みの場合だけ、非公開復旧APIの明示確認と
全binding再照合を経て旧WAL・観測RPC IDを保存し、同じ耐久requestを再試行できる。
timeout/応答不明、単なるrequest-show absentはこの明示確認の根拠にならない。

実復旧では同Task/Run/session/端末で`ctx_e4563220d15c`の配車・結果受領・provider正常終了を確認した。
reviewは承認ではなくM1-c受入証拠不足のchanges_requestedであり、driverは空inboxで
`active / integration.correction_required`へ進んだ。依頼全体の完了とは扱わない。

2026-09-26のTAK-14復旧では、停止最終turn、provider正常終了、source不変、5件の通信の確定を確認した。
旧試行をfenceし、generation 4、同じRun/Task/provider sessionと実装Aタブを保持してretryした。
旧Dispatch `ctx_1cd6522b4f25`は履歴として保持し、後継`ctx_3ccda4c1dd03`で担当の再開応答を確認。
本体再起動後は古い接続情報を持つ統括processも同sessionから再接続した。
snapshotの`working`/worker-a `running`と実terminalの再開応答は一致する。
これはM1-cの再開受入であり、M1-cの成果承認やTAK-14全体完了ではない。

修正対象はhostのtask bridge、review loop、supervisionと回帰試験。
固定read-only reviewが承認したdiff SHA-256は
`626600c28f58abd0d9545c06b98e55fb3de224e2d1e6a4a8e2e411cddc2a22d3`。
base `ae5b2066c4fd39a8dd6a6c439e272d125f749241`に対するcontracts/tooling gateは最終source
`1b6daf97a6551175f5edad77d8e91aff9ef46b4de20b6be5065b91033f2a25a7`で合格した。
途中でsourceが変わった検証は採用せず、固定した最終差分で再実行している。

`orca_request_runtime`は既存loop driverのtick後に実行する。scope拘束された承認済み工程から
依存が満たされた次nodeを選び、同じ統括へのplanning入力を送信前に保存する。
request/scope/nodeから決めた固定prompt IDと本文をOrcaのdurable prompt APIへ渡す。
応答消失後も同じID/本文だけを照合し、旧host・process交換等で安全なretryが否定されたら自動再送しない。
`accepted`、`started`、後継loop登録による`applied`を別状態にする。入力受付やturn開始だけで工程を閉じない。
worker間通信のRun inboxは引き続き既存driverだけが消費し、別consumerを作らない。

read-only `run-current`が本体再起動中に`runtime_unavailable`または`stable_pane_required`を
返した場合、Driverは`waiting_for_runtime`を保存し、2秒間隔で同じRun・所有者を再照合する。
端末の登録待ちだけで監督threadを永久終了させない。復帰後もRun ID・consumer generation・
統括handleの一致を要求し、新Runや別consumerへ置き換えない。mutation側の同じエラー、
stale handle、未知のエラー、所有不一致は再送許可ではなく、従来どおり保全して停止する。
既に終了した旧Driverへの反映は別工程であり、安全な終了点で同じ統括sessionを再開して確認する。

planning通知には全仕様を巨大なCLI引数として埋め込まず、正本pathとscope/nodeを渡す。
統括は正本全体を読み、実装・reviewのticketには引き続き全体contextを含める。
送信時の通信例外は同じIDの未確認状態として保全し、driverを終了させない。
通常のterminal本文はUTF-8で64,000 bytesまで、その他のCLI引数は2,048 bytesまでに制限する。

### 画面側terminalへの継続指示配送

Orca本体はrenderer leaf由来とruntime PTY由来のhandleを同じtracked PTYへ解決してから、
隔離Codexのprocess・stdio・incarnation証拠を検査する。renderer handleを見つけられないことを
「shellなので通常入力してよい」とは扱わない。未知local wrapper・欠損PTYは送信前に拒否する。
pasteとEnterの両方で元handle、generation、incarnation、同じprocess証拠を再確認する。
SSH/WSLにlocal `/proc`照合を適用せず、既存receiptのretryは照会だけで本文/Enterを再送しない。

2026-09-27、本体base `6c38bc463d0b6fd95ed6a5516901713775456fb8`への修正を
`ui-leaf-prompt-8475`へ配備した。production file SHA-256は
`8475c0de1e83dada9eddf3f8b8296a49a9719df68a90c2f9df36a8f0512441f2`。
固定read-only review APPROVED、回帰55件、node型検査、変更行品質gateに合格。
登録batch `orca-leaf-prompt-20260927b`でexact source/build付き非表示Electron 3試験をsealした。
先行batchはverifier契約不足でinvalidとし、最終合格へ流用していない。
別packageの非表示起動と隔離profileを確認後、設定を保全して本体だけを切替。
TAK-14統括のprocess/session/terminal incarnationと既存Runは維持した。
実送信 `f6160334-0560-4162-8ebb-5f3d021607a6`はprovider=codex/observation=supportedとなり、
追加Enterなしで同じ統括の受信応答と既存build待ち継続をscreenで確認した。
busy中のreceiptはinput_acceptedに留まり、これ自体をturn_startedと読み替えていない。
Helpは開発用terminal配送だけのNo impact。ゲームsource/入力/描画/Help consumerは変更しない。
この送信受入はTAK-14の製品検証・依頼全体完了とは別である。

同日の追加補正では、送信側Orca CLIを対話agentと数える誤判定を修正した。
basenameだけでなく共通command-line認識器を使い、通常CLIとclaude-teamsを区別する。
終了済みprocessは除外するが、生存する第二agent・停止中agent・不明なstdioは拒否する。
固定review APPROVED、回帰54件・node型・品質検査と、登録batch
`orca-sender-proof-20260927`の非表示Electron 3試験に合格。
production SHA-256 `5700c67e128b238b8510040df9115da388eb6161f19bd4cf108b58dc89f5e5ee`、
main bundle `de15394e19d9acf74b0ff9d33b0a8f2f4509c495941f89752f19667e4f85618d`。
隔離package起動後、`ui-sender-proof-5700`へ可逆切替済み。実通知の再開確認は別途行う。

通常`serve`は単一の同期workerを所有する。外部通信はUI snapshot更新とは別threadで実行し、
送信前のdurable intentと送信後read-backには既存executorを再利用する。
CLIとcontrollerが競合しても、一つのrequestのread/send/read-backは専用leaseで直列化する。
個別loopのreviewではLinear案件をreview/Doneへ進めず、依頼全体はstartedを維持する。
最後の全体受入でのみDone intentを生成し、同一subjectの受入証拠と外部確認operationを結ぶreceiptを保存する。
close preflight/実行/復旧では、そのreceipt、現在の承認、直近の接続観測を再確認する。
外部での取消や完了後reopenを観測しても自動的に上書きしない。

完了writeの既存confirmed記録だけで現状態をDoneへ合成しない。送信後にもLinearを再読し、
接続断・reopen・本文変更があれば全体完了を保留する。同期先はworkspace UUIDとissue UUIDへ拘束する。
同期workerはroute/lifecycleの読み取り失敗を案件単位で隔離し、他案件の観測を止めない。

### 外部本文と採用scopeの照合（候補batch 3）

`request-status`は最新Linear本文・子課題の観測内容とdigestを`runtime.external`へ示す。
初回と本文/子課題変更時には新規配車・Done・closeを拒否する。統括は原依頼と採用済み条件への
影響を確認し、`reconcile-external`へ`scope_sha256/content_sha256/reason`を渡して採用版を固定する。
直前に外部内容を再読し、expected digestが変わっていたら採用しない。未照合の追加指示も拒否する。
外部データを実行指示や権限追加とは扱わない。scope改訂が必要なら追加指示の照合を先に行う。
通常の仕様確認は統括側の内部操作であり、利用者へのID入力要求ではない。

runtime ledger schema 2はschema 1を読めるが、外部仕様の採用を自動推定しない。
旧loopの封印は書き換えず、再照合まで全体完了を禁止する。旧hostへの無検査downgradeは禁止する。

Linear全workspace一覧は復号失敗を正常な0件と扱わず、partialとworkspaceごとの
`linear_credential_unavailable`を返す。APIキーの失効とは区別し、設定・保存credentialは削除しない。
本体`60a5d78e`でUUIDによる課題のfull読取を受理し、返却issue IDの一致も確認する。
利用者の再登録後、修正版の実runtimeとhost adapterで読取成功を確認した。復号障害は現在の開始阻害ではない。

新規`register`/`register-successor` CLIは、scope/node、工程planning、統合先、Linearの現状態を必須にする。
旧案件は接続状態の読取までに留め、全体契約未照合のまま自動配車/Doneを行わない。
稼働中も追加指示の入口は残すが、実行中ticketを書き換えず、受理した指示の照合を全体受入前に要求する。
UIのstageはlane/integrationの保存済みenumから決め、日本語の説明文に含まれる語から推測しない。

## 実装済みと残件

### 隔離統括とnative検証host（候補・実受入前）

統括sandboxの表示socket、GPU、PID namespace、private tmpを開放しない。
`orca_native_bridge.py`をhost launcherが所有し、同じ統括processの子孫だけが
SO_PEERCRED UID・host PID祖先・出生identity・保存済みterminal/worktree所有の照合後に利用できる。
既存launcherへの追設も同じ所有検査を使い、別Run/terminalを作らない。

入力は登録batch IDとexact registration digestだけで、任意argv/path/envを受理しない。
recipeの契約は基盤所有の`orca_native_recipe_catalog.py`へ宣言し、
`orca_native_recipes.py`がargv形式とJSON plan形式を共通検査する。
catalogはliteral dataだけを構文解析し、batchが指定するpluginをimport/execしない。
現在は静止基盤recipeとbuilding-art共通recipe（feedback/art-preview/candidate）を登録する。
building-artは建物種別でbridgeを分岐せず、modeとauthority、同repo/HEAD/output、
封印plan、固定位置のcodecと計画時hashを照合する。codecの再build後はrecipe verifierが結果binaryを検証する。
clay独立verifierだけはTank/MudMixerの未承認previewに限定し、art採否や性能合格へ読み替えない。
同じGit common directory、primary登録source/HEAD/helper hash/policy、保持用途・未作成outputを再検査する。
汎用shellや、登録済みであるだけの任意commandは起動できない。新recipeは契約追加・関連試験・
固定reviewを経て配備する。新しい入力形式は共通検査器の拡張も必要で、無制限の自動対応ではない。
`capabilities --request-id`は稼働host自身の契約と版を、既存所有確認後に返す副作用なしの照会である。
統括は担当配車・batch登録前に対応を確認し、未知recipeを担当の結果待ちとして放置しない。
実行receiptはbridgeに加えて検査器とcatalogの版も束縛する。稼働中の契約差替えは拒否し、
未確定の実行を再送せず、同一統括の安全な再起動後に照合する。
契約追加時は`id/helper/directories/format/flags/bindings/verifiers`を定義し、
plan形式では`values/authority/executables`で許可値・実行可能入力を拘束する。
helperと独立verifierはsubjectのcommit済みbytesと登録hashの両方が一致しなければ使えない。
planも登録hashに含め、別repo/root/HEAD、未知引数、重複引数、symlink、plan改変は実行前に拒否する。
結果判定はregistryの対応可否とは別である。capabilities成功、launch受理、execute exit 0だけでは
レビュー承認・native合格・依頼完了にしない。

2026-09-27の共通化は固定read-onlyレビューAPPROVED、重点90件、変更別contracts/tooling
1007+164件、perf self-test、Ruff/actionlintに合格した。比較基点は
`ae5b2066c4fd39a8dd6a6c439e272d125f749241`、同一sourceは
`6e49b3f68bc2e24bea4f86eebba46239ea4ff75bb8dfa1f328cdd64244041d7f`。
catalog読取とrevisionの更新競合、旧argvのHEAD不一致を追加試験で拒否した。
同じTAK-14統括sessionの正常再起動後、実hostのcapabilities revision
`834cc2b4003f6033cd51024a960db3ae62506e86e93d08aa480d23bb18553e30`と
共通building-art契約の応答、それを受けた統括のnative準備再開を確認した。
これは共通契約の反映受入であり、native batchの一巡や現在の実装担当待機からの遷移までを
合格とはしていない。native開始の所有・settlement条件は従来どおり別途照合する。
Help影響はNo impact（host実行制御のみ）。ゲームbuild/testは本修正側では実行していない。
primary controllerはimport前にcanonical path/所有/権限と対応版SHAを検査し、検査したbytesだけを実行する。
primary controllerの更新時はbridgeの互換試験・対応版の更新が必要であり、自動的に未知版を信頼しない。

hostの直接kitty launcherは薄いhost実行adapterを子として起動する。adapterは現在の重実行枠を
取得したまま、**変更しないprimary validation controller**のexecuteを呼ぶ。
これは重実行枠をまだ持たない凍結primary/recipeとの互換用であり、対象sourceを更新しない。
子processへ同一leaseの環境とFDを渡すadapterで、既に重実行枠を使うrecipeの二重取得も防ぐ。
runner・native lock・PID観測はhost namespaceに揃う。利用者へコマンド入力は求めない。
検証子processの`RUST_LOG`は`info,wgpu=error,bevy_app=warn`へ固定し、実行receiptへ記録する。
desktop/agentの`warn`や`off`を継承してrendererのAdapterInfo証拠を失わないためであり、
親環境・凍結recipe・ゲームsource・verifierの証拠要件は変更しない。

起動前にWALを保存し、kittyの成功はsubmittedに留める。timeout/応答消失/非zeroを再送権限にしない。
唯一の資源待ち再試行は、host子がprimary execute前の`host_busy`を記録し、そのexact出生の終了と
同じattempt/source/所有を確認できる場合。旧receiptを保全し、未実行batchのadmissionを再検査する。
古いbusy記録は後継attemptの再送権限にならない。独立verifierも同じ規則を使う。
`inspect`でhost側のbatch phase・実process・exit code・独立検証結果を読む。
成功実行後の`seal-pass`は登録verifierだけをhost側で実行し、送信受理だけではpassを記録しない。
`finalize`は既存primaryの保持/整理検査を使い、ファイルを削除しない。
実行nonzeroまたは独立verifierの記録済みnonzeroは`seal-failed`でinvalidを確定できる。
verifier子が自身の開始/終了receiptを永続化するため、親processの終了だけでは結果を失わない。
`recover-interrupted`はrunning/sealingの保存済みPID/出生の終了をhost側で正に証明した場合だけ
primary recover→interrupted記録へ進める。registeredのままの未知送信は再送も終了合成もしない。
隔離側PIDで終了を推測しない。source/receipt不一致・観測不能は保全して主担当が調べる。
基盤tooling/固定reviewと登録batchの実再開は確認中であり、TAK-14完了の証拠にはしない。

### native終了から同一統括への継続（候補反映・実受入中）

統合reviewの差戻し後、担当が追加変更なしで正式failed終了し、統括所有の検証が先に必要な場合は
`return-correction --slot <元担当> --expected-sha256 <fresh loop digest>`で補正所有を返す。
`orca_correction_return.py`は可視統括、現scope/Run/Linear、空inbox、全attemptの終了/release/ACK、
現在担当の正式settlementとprovider終了、両作業場の不変HEAD/source、保存済み固定reviewを照合する。
旧worker承認の対象が不変のときだけそのcheckpointへ戻し、統合側は`correction_required`を復元する。
failed Dispatchや固定reviewの`changes_requested`は変更しない。依頼完了・指摘解消・検証成功にはしない。
差戻し履歴にはnative review contextも保存し、検証証拠付きticketを正規生成してsealed digestと再照合する。
既存native eventがある場合は全先行eventの適用済み証拠を要求し、その待機履歴を消去しない。
変更前のlane/integrationと判断digestを同じatomic loop保存へ記録するため、中断時は全体未適用か全体適用になる。
同じ要求の再送は副作用なし。live担当、dirty source、未確認release、未知失敗、未終了nativeは拒否する。
この経路は統括用で、利用者に内部IDや端末コマンド入力を求めない。

2026-09-27、TAK-14の正式failed差戻し`ctx_8c693db508c5`をこの入口で返却した。
同Run・scope/node・candidate/統合HEADを保持し、active/correction_requiredへ遷移した。
封印済み`changes_requested`とfailed outcomeは不変。新Task・再配車・Git更新は行っていない。
固定read-onlyレビューAPPROVED、重点58件、最終contracts/tooling 1014+164件、perf self-test、
Ruff/actionlintがpass。baseは`ae5b2066c4fd39a8dd6a6c439e272d125f749241`、sourceは
`52fc70c6d5119d6aef85d9aa5e28c05a2f99796024f4c0bbf7d4d7a0f5959069`。
Helpはhost所有遷移のみのNo impact。これは所有復旧の受入であり、製品native成功・M2完了ではない。
同じ統括sessionで新規応答とvalidation status/native準備読取を確認した。
初回継続入力はunsupported providerの受付後もdraftに残ったため、screenのdraftと正のidleを照合し、
本文を再送せず同じ入力を1回submitした。配送の汎用改善は本所有復旧の合格範囲に含めない。

`orca_native_continuation.py`は新しいagentやinbox consumerではなく、既存Driverのtickで動く。
launch前に専用eventとloopの`native_wait`を保存し、同一batch digest・scope/node・Run・世代・
source/HEAD・差戻し時点のintegration状態・期待する後続操作を結び付ける。
`correction_required`自体をnative待ちや依頼完了へ読み替えない。
loop保存入口は未終了nativeを残した工程変更を拒否する。終了済みの正規差戻し配車、
`resume-review`による同一HEADの固定再review、またはexact journalに束縛された統括補正を後続として記録する。
`resume-review`は同一batch/digest、独立判定pass、finalize、active control、source、通知drain、
旧担当の終了を確認する。旧reviewを承認へ変更せず、native証拠付きの新しいreview ticketを同Runへ登録する。
全体completionと後継generationの入口でも、native eventの適用証拠を再確認する。
待機を次batchへ移すときはblockedな旧eventを拒否し、先行event鎖も保持する。
完了判定は最新eventだけでなく鎖全体の適用証拠を要求する。監視はprovider起動前からDriverへ接続し、
nativeの結果投影を通常loop/依頼継続より先に行う。未適用中は通常dispatchと無拘束な文書書込みを許可しない。
文書補正はsealed reviewの引用pathに限定し、元の待機・Run・全loop状態、補正spec、committed Git receipt、
現在のclean sourceがexact journalと一致する場合だけ新しい`validating`へ移す。旧承認は引き継がない。
Git公開後の中断はprepared journalから投影のみ回収し、Git操作を再実行しない。

hostがexecute終了、独立verifier、正のprocess終了に基づくinterruptedを観測する。
各tickはnative専用leaseの内側で自分が起動したverifierのPopenを非blocking pollし、
終了済みの子だけを回収する。外部inspect要求がなくてもzombieを残さず、後続通知の
process証明を妨げない。生存子へのwait/killやreceiptの成功合成は行わない。
execute exit 0だけでは成功通知しない。資源busy以外では元recipeを自動再実行しない。
通知は同一operation IDのdurable promptを使い、sending/accepted/startedを分離する。
`unsupported / input_accepted`は通知失敗ではなく受理済み・開始未観測として保持する。
初回応答は不変に保全し、後続read-backは別に記録する。応答保存は送信後の所有・control再照合より先に行う。
隔離Codexの開始を観測できない場合も、受理を`turn_started`へ読み替えず、正規の後続操作で進行を証明する。
finalizeだけでは`applied`にしない。同じ待機に続くnative登録、固定reviewの開始、正規の差戻し配車が
成立、または上記の統括補正が成立して初めて適用証拠を保存する。これも依頼全体の承認とは別である。
旧版の未観測誤分類だけは専用reconcileで回収できる。exact event/text hash、同owner/session/control、
finalized result、同IDの`replayed=true`受理receipt、実在successorを要求し、元の停止理由と照合証拠を残す。
復旧intentは所有handoffより前に元event全体とhashを保存する。handoff後・read-back後の中断でも、
証明済み所有遷移以外の差分を拒否したうえで、同じ元hash・同じ操作IDから復旧を再開できる。

統括の同一性は、terminal incarnationと実Codex processの出生・開いているrollout FDのsessionで確認する。
アプリruntime UUIDは接続情報であり、daemonが維持するterminalの同一性とは分離する。
本体再起動後もhandle・incarnation・worktree・live provider・sourceの一致を必要とし、
runtime UUIDだけの変更では待機を破棄しない。offline応答は接続待ちとして既存WALを維持する。
旧UUID誤判定の停止は、専用reconcile-runtimeでevent digest・所有・制御・結果を照合してのみ
同じ通知IDのread-backへ戻す。復帰コマンド自体は本文もEnterも送らず、旧receiptを保全する。
最新ファイルの推測やタブ名では代用しない。明示した同session resumeと旧processの正の終了を照合した
handoff以外は所有変更を拒否する。通知結果不明を新ID・raw Enterで再送しない。
handoffは遷移ごとのwrite-ahead receiptを残し、中間停止から同じ投影を回収する。
二度以上のhandoff後のbusy再試行も、実receiptの所有者から現所有者までの鎖を照合する。
primaryが終了を保存した直後に子receiptの記録が失われた場合は、正のprocess終了とprimary結果から
別の照合証拠を保存する。子自身がexit receiptを記録したことには書き換えない。
UIの明示pause/closeと未適用指示を尊重し、blocked状態は既存の状態行へ出す。新しい結果UIは作らない。

統括の起動もapp-nameを含むtitle設定を使う。hostからの結果通知後、統括が証拠の保持、finalize、
比較・固定review等の後続工程を担当する。起動後の手動poll/独立sealとの二重所有は行わない。
移行・再検証のhistory要素は差戻しroutingとは区別し、routingのない履歴でKeyErrorを起こさず、
差戻し回数にも加算しない。回数は監査記録とし、下記の補正方針に従って累積上限には使わない。

2026-09-27の実受入では、同じTAK-14/Run/session/terminal incarnationで旧停止eventを回収し、
元通知ID `e1a05002-c13e-52f6-a8d5-d5c679ebcec8` のCodex応答開始を確認した。
統括自身がTank証拠を保持登録・finalizeし、次のMudMixer batchを正規登録したため、
元eventは`applied/native_registered`となった。続くMudMixerも独立sealでpass、
自動通知から統括の応答・証拠保持・finalizeまで進み、追加の手動通知は行っていない。
次工程へ進んだ証拠であり、TAK-14/M2全体の完了や、未実施の全運用経路の保証ではない。
履歴は同じ統括ターミナルに残し、review・実装担当のタブやRunの再発行はしていない。
MudMixer側も`applied/native_review_registered`へ進み、既存固定reviewの結果を統括へ返した。
基盤の最終固定read-only reviewはAPPROVED。base
`ae5b2066c4fd39a8dd6a6c439e272d125f749241`、source
`64952cf80b51c103d9ee825deaa7e74e6a03ea2afae86cd64c0110b122f6bbcf`に対し、
変更別contracts/tooling（1,022+164件）、perf self-test、Ruff/actionlint、保存領域はpass。
ゲームbuild/testをこの基盤gateから直接実行していない。Helpは開発用制御経路だけのNo impact。
本体試験用reportと展開treeを撤去（見かけサイズ174,608,002 bytes）し、試験batchはfinalized。
稼働版と設定backupは配備・復帰用consumerとして保持し、製品の比較証拠は統括の保持契約を維持する。
その後の製品reviewは`M2-NATIVE-SCOPE-001`を返し、同じlaneの補正回数が既に3回のため
正規routeが停止した（統合差戻し履歴は1件で、今回の停止はlane側の上限）。
これは配送失敗やnative結果欠損ではなく、内部で設けた回数制限による停止だった。
この制限の変更を利用者の判断事項にはしない。履歴・回数はリセットせず、下記の恒久修正で扱う。

### 補正回数と統括の再計画（2026-09-27）

lane revisions、統合差戻し、履歴・attempt・ACKの累積件数を継続許可の上限にしない。
同時実行slot、live inboxの配送件数、入力payload長、所有・scope・未確定writeの防御は維持する。
Orca本体の連続Dispatch失敗のcircuit breakerも、新Runや別Taskで迂回しない。

- sourceまたは原因が変化した補正は通常経路で継続する。
- 同一source・原因の自動反復は統括へ返す。reviewは`route-review`で修正pathと新しい方針を指定する。
  統合reviewは`route`でhead・指摘・方針を拘束し、同じ組合せの再実行を拒否する。
- validationは`validation-routing`と`watch`のattentionへ移し、統括が`route-validation`へ
  `{loop_sha256, slot, reason}`を渡す。元の失敗証拠・ticket・source・所有・現scope・active接続、
  全attemptのsettlement/ACK、native通知drain、利用者のpause状態を照合する。
  原因診断を記録した同Run・同sessionの再配車へ戻し、同じ対象・原因・診断の繰返しは拒否する。
  過去のreviewを新しいvalidation失敗と取り違えない。
- 旧combined budget停止は`orca_correction_policy.py`がexact停止digest・封印review・統合receiptと
  現在の利用者controlを照合し、`correction_required`へ戻す。回数・失敗・証拠を消さず、
  review承認や新規dispatchを捏造しない。別理由のpauseはこの経路で解除できない。

内部の反復回数を増やすかどうかは利用者へ問い合わせない。統括が既存依頼の範囲内で診断・再計画する。
利用者への判断依頼は目的・scope・公開・破壊的操作など実際の権限境界に限定する。
Help影響はNo impact。変更はhostの補正台帳・診断・担当配車だけで、ゲーム内入力・表示・Help catalogには接続しない。

本補正の固定read-only reviewはAPPROVED。base
`ae5b2066c4fd39a8dd6a6c439e272d125f749241`、source
`4421439e46defa49f2c203a8d73cd7d8df25275cac895f4a6688e9fa0e15483f`の
contracts/tooling（1,031+164件）、perf self-test、Ruff/actionlintとprimary storageはpass。
TAK-14は同じ統括session・terminal incarnationを保って修正版へ正常再起動し、
旧停止digestを正規helperで照合してactive/correction_requiredへ戻した。
lane revisions=3と元Run・統合HEAD・封印reviewは保全した。
同terminalで補正方針の更新指示のturn_startedと、統括自身による最新状態の読取を確認した。
これは基盤停止の復旧であり、M2の製品修正・全体受入の完了ではない。

返却済みfailed correctionの再計画では、ticket generationが同じままfollow-upが変わることがある。
旧armed dispatchを成功扱いで再利用せず、返却receipt、同ticket/session/source、封印review、
旧Taskのfailed・capability失効・release/ACK、現controlとidle shellを照合する。
`orca_correction_retry.py`の照合を通した場合だけ、dispatcherが旧journalをprivate receiptへ保存し、
同Taskに`--retry-of`を指定して再配車する。新Run・別Taskで失敗履歴を回避しない。
launcherには新しい統括方針を明示follow-upとして渡し、従来のtab再利用・起動・未知mutationのguardを維持する。
既に旧版のfollow-up不一致で配車前停止した場合だけ、fresh digest付きの同helperでdispatchingへ戻せる。
この移行helperはhost process inventoryを読める制御側で実行する。隔離された統括shellからの
process不可視を終了証明にはせず、通常再配車ではhost driverが同じ照合を行う。

追加是正も固定read-only review APPROVED。最終source
`33092c09fe27fe77115070c2cf790e74f6462340cedefe3d507d534da97db804`で
contracts/tooling 1,036+164件・perf self-test・Ruff/actionlintがpass。
実受入ではRun `run_3236488f0dff`、Task `task_56a6cdf4de22`、元実装A session/tab/incarnationを維持し、
旧failed `ctx_8c693db508c5`に対する正規retry `ctx_f963206ff6bf`をarmした。
loopはactive/implementing。担当による正式checkのconfirmed receiptと、同ターミナルでの
Tank/MudMixer fixture・geometry実装の読取開始を確認した。旧failed履歴と新方針は保存済みで、
製品変更・M2受入・依頼全体完了を先取りした記録ではない。

### 統合前の文書差戻し（所有境界）

固定reviewの`docs/*.md`引用は修正対象の指定ではない。引用がある場合は`review-routing`へ進み、
統括が全sealed findingのIDに実際の修正pathと理由を対応させ、`route-review`へprivate spec
（`loop_sha256`、`slot`、`repairs: {指摘ID: [相対path]}`、`reason`）を渡す。
`watch`はこの判断を`lane_review_routing` attentionとして示す。ユーザーに振分けやID入力を返さない。
元checkpoint・sealed review・同一source・active issue・可視統括・全attemptのsettlement/ACKを照合する。
実装pathは元ticketのallowed directories内だけ。同担当・同sessionへ戻し、文書を編集する権限は与えない。
実際の修正対象がコードと文書にまたがる場合、このrouteで一部だけを差し戻すことは拒否する。
混在修正は全指摘の受入を維持する別途review済み計画が必要であり、残る文書指摘を解決扱いにしない。
全修正対象が文書のときだけ、統括所有の補正へ振り分ける。引用だけで停止した旧documentation状態も、
同じexact検査付きrouteで復旧する。未知・範囲外・未対応ID・重複ID・path traversal・source競合は拒否する。
実装差戻しは新しい修正方針を拘束し、activationと差戻しを単一saveで記録する。旧digestの再送は拒否する。
`orca_lane_documentation.py`はprimary正本の指定Markdownをhashで拘束し、sealed reviewに引用され、
統括が明示した修正pathだけを受理する。
同一checkpoint・review ticket・source・可視統括・active issue・担当終了・全attemptのsettlement/ACK、
または既存の正にfence済みruntime recoveryを照合する。別理由のglobal pauseは解除しない。
workerの編集許可は広げず、原依頼・Run・失敗したattemptを保全する。

変更前journalとclean checkout/CAS確認を行い、補正後の変更別検証と同一固定reviewerの再レビューを必須とする。
旧reviewは履歴であり、新headの承認ではない。途中停止はjournalを保持して照合し、無条件再実行しない。
入力文書・source競合、未確認終了、検証失敗は自動承認しない。
review成功・次試行への遷移時は古い失敗理由を解除し、failed Taskには今回の停止理由を記録する。

2026-09-27、TAK-14のM2-CLAY-001（仕様文書を引用した実装指摘）で旧誤分類を再現し、
同Run・統括session・実装A sessionを維持して`route-review`の実受入を行った。
`ctx_c66de569754a`で実装Aが指摘を受領し、11:40 UTC以降に`structure.rs`と`pool.rs`の
実編集を確認。UIのworking／実装A runningと実terminalの差分を照合した。
起動受理だけでの合格ではなく、同担当が修正を始めたところまでの基盤受入である。
製品修正後の検証・再review・TAK-14全体受入は別であり未完了。
基盤差分の固定read-only reviewはAPPROVED。最終source
`63d2da3feacab31f5daf0e017299fd3d0bf0de26ee2f38624ae072005107f6cd`に対し、
base `ae5b2066c4fd39a8dd6a6c439e272d125f749241`のcontracts/tooling（989+164件）、
perf self-test、Ruff/actionlint、storageはpass。Helpはhost制御のみのNo impact。
混在修正の部分配車は未実装として拒否する限定承認であり、全経路の無介入運用を保証しない。

### 担当タブの起動記録と旧未確定起動の取消（2026-09-30）

再利用する担当タブへの起動送信は、`orca_role_launch_receipt.py`が送信前にprivate WALへ記録する。
同じoperation ID、本文digest、terminal incarnation、依頼と担当を拘束し、初回からdurable IDを指定する。
成功・失敗の返却を先に保存し、肯定receiptと同一incarnationの照合後にのみacceptedへ進める。
acceptedは着手や完了の証拠ではない。未知の結果を新IDで再送せず、公開エラーには秘密本文を含めない。

旧版でこのWALがないpre-Dispatch起動だけは、利用者の明示した取消許可を受けたhost保守から
`orca_launch_cancellation.py`を使える。同loop/source/session・未確定attempt・前担当の正の終了・
全Task/Dispatch・同一incarnationのidle shellを確認し、元状態と保持済み端末出力を保存してから
Ctrl-Cを一度だけ送る。肯定receiptと再照合後、取消証拠をcompleteにしてから通常配車を公開する。
中断後は同じleaseと証拠で投影だけを回収する。未知のinterruptを再送したり、新タブへ逃避しない。
新たな起動WALがある操作、進行したDispatch、未確認process、変更されたsourceには適用できない。

旧unknownは成功へ改変せず、取消receipt→新しい起動WAL→新attemptの参照鎖として保持する。
同じ依頼・Run・provider session・タブで続けるが、未作成だったTask/Dispatchは通常配車で初めて作成する。
これは利用者が毎回IDやコマンドを入力する運用ではなく、制御側の限定した保守経路である。
一般的なcomposerの自動回復、終了commandの専用証明、全版互換・全経路受入の完了を意味しない。
残件と同一source検証は[システム恒久是正計画](../plans/orca-system-hardening-plan-2026-09-29.md)で追跡する。

候補側には上記の受入台帳、revision、ticket拘束、終了/Doneの拒否gateとテストを実装した。
実Gitを使う統合loop試験では、scopeを渡した固定reviewの封印・全体受入・承認後のソース変更拒否を確認する。
providerとOrca接続はfixtureなので、実providerによる全経路受入の代用にはしない。

以下はまだ運用完成を主張できない理由である。

- commit済み統合以外の既存案件のscope移行と、全入口の互換照合。
- 上記の自動継続・通常同期は候補コードとfixture試験まで。実providerでの無介入継続は未受入。
- 本文/子課題の再照合を含む実サービスでの全経路受入。元の復号障害の原因は未確定だが、読取は回復済み。
- 人手受入・公開条件、非表示/中断/取消と成功終了の分離、版互換・配備manifest。
- 同じ通常入口からの実provider複数工程、実Linear/GitHub、UIと再起動を含むT01〜T12の受入。

Help影響はNo impact。変更のproducer/consumerは開発用Orcaのhost台帳・terminal・外部サービスであり、
ゲーム内入力、描画、runtime assets、Help catalogへ接続しない。ゲームbuild/testは本是正の検証に含めない。
