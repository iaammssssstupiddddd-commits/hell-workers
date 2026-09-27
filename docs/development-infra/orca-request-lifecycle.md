# Orcaの工程承認と依頼全体の受入

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

担当tabの整理では、launcher終了後に残るforeground shellにTUI idleを要求しない。
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
cleanかつ不変のsource、正のworker/launcher終了証拠、同じprovider sessionを要求する。
通信履歴は全件confirmedで、heartbeat・空check・回答済みaskだけを許可する。
pending操作、未回答質問、未処理delivery、worker_doneを含む履歴はこの復旧では扱わない。
旧試行をreceiptへ保全し、同じRun/Task/sessionへ新たな権限付きretryを準備する。
旧bridgeを再有効化したり、新しいTaskを作って失敗履歴を回避したりしない。

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

planning通知には全仕様を巨大なCLI引数として埋め込まず、正本pathとscope/nodeを渡す。
統括は正本全体を読み、実装・reviewのticketには引き続き全体contextを含める。
送信時の通信例外は同じIDの未確認状態として保全し、driverを終了させない。
通常のterminal本文はUTF-8で64,000 bytesまで、その他のCLI引数は2,048 bytesまでに制限する。

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
現時点の対応recipeはcommit済み`building_art_static_acceptance.py run`の固定引数・固定verifierだけ。
同じGit common directory、primary登録source/HEAD/helper hash/policy、保持用途・未作成outputを再検査する。
別recipeへの一般化は未実施。汎用shellや、登録済みであるだけの任意commandは起動できない。
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
差戻し回数にも加算しない。実際の差戻し3回の上限は維持する。

### 統合前の文書差戻し（所有境界）

固定reviewで`docs/*.md`が指摘された場合は、権限外のworker再試行ではなく統括所有の補正へ振り分ける。
`orca_lane_documentation.py`はprimary正本の指定Markdownをhashで拘束し、sealed reviewの引用pathだけを受理する。
同一checkpoint・review ticket・source・可視統括・active issue・担当終了・全attemptのsettlement/ACK、
または既存の正にfence済みruntime recoveryを照合する。別理由のglobal pauseは解除しない。
workerの編集許可は広げず、原依頼・Run・失敗したattemptを保全する。

変更前journalとclean checkout/CAS確認を行い、補正後の変更別検証と同一固定reviewerの再レビューを必須とする。
旧reviewは履歴であり、新headの承認ではない。途中停止はjournalを保持して照合し、無条件再実行しない。
入力文書・source競合、未確認終了、検証失敗は自動承認しない。
review成功・次試行への遷移時は古い失敗理由を解除し、failed Taskには今回の停止理由を記録する。

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
