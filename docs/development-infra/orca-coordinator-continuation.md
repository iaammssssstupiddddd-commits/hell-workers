# 統合レビュー後の統括継続

> 歴史資料（2026-10-04整理）。独自統括は廃止済みで、以下のcommand・Run・復旧経路は実行しない。
> 現在の規則は[通常Orca運用ガイド](../orca-quickstart.md)。旧未完条件と過去の結果は履歴のまま保持する。

単一実装担当へ差し戻せない指摘は、統括の作業終了を意味しない。
`orca_ui_coordinator.py`の起動指示は、封印された全指摘を実装済み・既存証拠・未実施へ分類し、
元の計画・受入条件と照合したうえで、許可済みの正規工程を継続することを要求する。

実機確認、候補採否、文書同期は統括が所有する。コード変更を必要としない証拠不足を、
同じ担当への無意味な再配車で解消しようとしない。native plan/register/launch、文書補正、
再reviewの既存guardを使う。異なるsubjectの結果を流用せず、新subjectの受入は新batchに拘束する。

複数scopeのコード変更やbase更新を単一担当routeへ押し込まない。
所有境界を維持する計画・正規経路を確認し、未実装経路は具体的な不足として記録する。
独立して進められる許可済み工程まで止めず、未知writeの再送、新Run、scope拡大、
指摘の省略、承認捏造は行わない。公開や新たな権限が必要な操作は既存許可と区別する。

ここまでの継続指示は統括の指示契約であり、自動schedulerを新設するものではない。
途中工程の成功を依頼全体の完了と判断せず、最終受入は既存の全条件・固定review gateを維持する。

Help影響はNo impact。変更は開発用統括のpromptと回帰テストに限定され、
ゲーム入力・表示・runtime assets・Help catalogは不変。
稼働中の統括には同じ会話で新しい指示として適用し、次回起動には永続promptを使用する。
既に実行中の担当や検証を、この指示変更のためだけに再起動しない。

準備codecも対象recipeの正規build argv・feature・profileに従う。Cargo profile名は
featureを有効にしないため、`--profile profiling`だけでprofiling feature付きと判断しない。
feedbackと正式candidateの指定を区別し、誤設定buildの成果物を受入へ流用しない。
誤設定の自分のbuildを中断する場合は起動側で所有・終了を確認し、終了不明で再実行しない。

2026-09-28、TAK-14のM2-SCOPE-001で単一担当へ配れないことを理由に統括が終了した経路を是正した。
同Run・同会話へ残工程の分類と継続を伝え、統括自身のnative Skill読込、指摘分類、build操作を確認した。
準備codecのfeature指定欠落も検出し、統括が自分の起動PTYから誤buildを中断・終了確認した。
誤成果は採用せず、既存targetは維持した。全体テストと製品buildの実host lock競合は失敗として記録し、
直列化後に再実行した。恒久変更は起動promptと回帰testのみで、ゲームコードや状態台帳は変更していない。

固定read-only review APPROVED、関連31件、contracts/tooling 1090+164件、Ruff/actionlint、
perf self-testがpass。base/head `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、source
`711a58ee9d23a2fc9c481026b38d82ee807e30f6251a2a9a176bf01cb397aacc`。
primary storage pass（126 batch、480451936256 bytes、未分類0）。本基盤修正の永続job新設・削除なし。
検証後に統括へhost資源待ちの解除を通知した。製品全体の受入完了を意味しない。
同じ統括の新turnでcapabilities・現subjectの再照合を行い、
`cargo build --profile profiling --no-default-features --features profiling --bin bevy_app`
の実process（PID148053）起動まで確認した。受理receiptだけを再開証拠にしていない。

## 単一laneの実装・文書混在指摘

`route-review`は、sealed finding全件に対する明示repair mapが、既存worker scope内の
コードとreviewが引用する文書に分かれる場合、`documentation_then_implementation`として保存する。
混在を理由に追加の利用者承認や未実装の計画承認APIを要求しない。

順序は次のとおり。

1. 統括がprimary文書正本を修正する。workerへdocs権限は渡さない。
2. `orca_lane_documentation.py`へ全routing文書のexact hashを渡す。subsetは拒否する。
   同じsealed review・checkpoint・source・退出済み担当・ACK・Run所有を再照合して
   文書だけの後続commitを作り検証する。中断journalからcheckoutを盲目的に再実行しない。
3. 混在routingでは文書成功だけでreview/承認へ進めず、同worker会話と未変更の編集scopeへ戻す。
   全指摘と全repair mapを引き継ぎ、コード修正の後に複合subjectの検証・checkpoint・固定reviewを要求する。

コードのみの指摘は従来の同担当への差し戻し、文書のみは統括補正後の固定reviewを維持する。
文書の成功をコード指摘の免除へ読み替えない。receipt/source変更・未ACK・scope外修正の拒否を維持する。
ゲームの挙動やplayer Helpを変更する経路ではなく、開発用所有・順序の是正としてNo impact。

混在時も修正戦略をrouting保存と同時に記録し、同source・同指摘・同計画の反復を拒否する。
文書補正の累積回数では停止しない。過去の補正履歴を保持したまま、新しい正規の指摘・subjectを処理する。
文書検証の成功は、世代receiptやloop更新より先に`validated` journalとして保存する。
loop保存前後の中断では、同じspecを再提出するとbefore/after loop、candidate/tree/source、
担当session・所有、旧checkpoint、成功検証証拠を再照合し、未完了のローカル保存だけを完了できる。
checkout・commit・検証は繰り返さない。検証結果が不明な中断、異なるsource・所有・loopへの
変化は自動再実行しない。文書のみ・混在の両方に同じ復旧契約を適用する。

### 失敗が確定した文書検証

`validation_failed`は検証subprocessのnonzero exitと証拠が保存された状態であり、
結果不明の`validation_running`とは異なる。統括は失敗内容とhost修正を確認し、
同じspecと`--retry-validation <検査済みjournal digest>`を指定して検証のみを再開できる。
同candidate/source・変更前loop・担当session/所有・保存済み失敗証拠と、変更されたhost executorを
要求する。失敗履歴を先行保存し、新検証が中断した場合は未知再実行を拒否する。
文書コピー・commit・checkoutは再実行しない。host executor証拠にはrunnerと現行dev.pyのhashを含む。

host runnerのPython unittest subprocessはCargo用`TMPDIR`/`TMP`/`TEMP`を引き継がない。
Unix socket fixtureのpath上限と/tmp拒否fixtureの前提を維持し、永続保存が必要なfixtureは
自分で明示pathを選ぶ。Cargo本体の永続一時領域・資源guardは変えない。

文書補正のscheduler admissionと成功後の保存再開は、単一callerで既存lockの解放を待つ。
HostBusyに対してshellからhelperを繰り返し起動しない。role・workspace・heavyの所有検査は
緩和しない。待機変更だけでは既存の結果不明なvalidation_runningを回収できない。
外側のshell終了はvalidator子processの終了証拠ではなく、以前の失敗結果も新attemptの
結果ではない。

### 結果不明の文書検証の回収（受入中）

`orca_validation_fence.py`は既存統括の正常終了を外側から観測するだけで、停止signalや
loop/journalの書換えは行わない。launcherのui-coordinator lock所有、bwrapとnamespace initの
親子関係・PID出生を検査し、3つのpidfdすべての終了を待つ。PIDの不存在だけでは発行しない。
同会話の失敗CommandExecution、元spec、前journalのdigest、private transcriptの不変prefix、
candidate/source、local CIの選択群、controller Python modulesを拘束する。
対象は監査した同期local CIのcontracts/tooling/rustのみで、任意の外部処理を回収するAPIではない。
当該依頼の外部送信は全件終端済み、nativeの前工程は適用済み、launchは実行終了・batch確定済みを
要求し、関連ファイルの前後一致も確認する。進行中の外部処理を隔離namespace終了で済ませない。

終了receiptは検証の成功ではなく「この試行は中断済み」の証拠である。
同じ統括sessionを通常再開した後、文書helperに元spec、検査済みjournal digest、
`--interruption-fence`と検査済み`--fence-sha256`を渡す。
launcherがresume argvから注入した`ORCA_COORDINATOR_SESSION`もreceiptと一致させる。
環境変数だけを所有証明には使わず、helperの実祖先にいるlive Codexがopenしているrollout FDを
既存のprovider identity検査で読み、session ID・正本path・作業場も元会話と照合する。
既存のsource・所有・checkpoint照合を通した後、中断履歴と新attempt UUID・executor・command・sourceを
原子的に先行保存し、検証だけを行う。コピー・commit・checkoutや担当Dispatchを重複させない。
同じ終了receiptは、次に中断した別attemptには使えない。運用への適用と再開実見はまだ未完了。

## 継続担当の起動準備中断

`orca_worker_bootstrap_recovery.py`は、継続担当のbridge生成後・Task投入前に停止した
`dispatching`を、同じterminal/sessionで回収する。対象はCodexの正常な準備完了が確認できる場合に限る。
利用上限等による`last_agent_message: null`は正常応答ではなく、起動完了へ昇格させない。

loop digest、ticket世代receipt、source、follow-up、固定session、launcher lease、terminal incarnation、
未armかつ操作のないbridgeと、同RunのTask/Dispatch全件を照合する。投入前WAL後も操作中断・
Linear状態・接続を再照合する。正常な`worker-start`を一度だけ呼び、同Runの次世代Taskを作る。
結果不明では再送せず停止し、結果保存済みの場合だけ同じ外部Taskのarm・ローカル投影を再開する。
arm後の投影中断も元のlive launcher/role/terminal所有を要求し、終了不明を成功扱いしない。

準備turnの明示失敗後に準備応答を取り直す場合、元turnにtool/RPC実行がないこと、同一所有とidleを
確認し、`BOOTSTRAP ONLY:`と現bridge markerを含む準備だけのpromptを一度送る。受理だけで完了とせず、
正常な`Waiting for supervised dispatch`応答を確認する。作業本文・旧Dispatch権限は再送しない。
この回収helperは自動Driver再起動ではない。Driver自体が終了している場合は、元統括の正常終了と
registry終了を確認し、既存のrequest/sessionに拘束した通常再開を別途行う。

## 統括文書を含む後続レビュー

workerの書込scopeと、累積base/headのレビュー対象は別である。統括が文書補正を行った後に
workerがコードを再修正しても、文書の正規所有をレビュー依頼から落としてはいけない。
`review_documents`は世代receiptの鎖を遡り、統括の文書commit・成功検証・現在の文書blobを
照合する。証明できた文書だけをread-onlyレビューscopeに含め、workerの編集権限は変更しない。
未認可の文書や文書以外のscope外差分、証拠欠損、blob変更は拒否する。
古い非context形式も、世代fileだけでは証明にしない。各committed世代の保存checkpointと
成功validation、前後assignment、parent commit/treeを別途照合する。

旧形式のレビューscopeで停止した場合、`orca_review_scope_recovery.py`が検査済みloop digest、
元のsealed rejection、全settlement/ACK、担当と固定reviewerの終了・会話、clean checkpoint、
同subjectの成功検証を照合する。元ticketと全指摘を履歴へ保存し、修正scopeを持つ新ticketで
固定reviewへ戻す。source変更・空commit・再検証の起動・外部配車はこのhelperでは行わない。
停止理由・correction種別・遷移履歴がレビュー振分け待ち、または文書のみの正規routing待ちで
あることも必須とする。別の停止理由や実装修正待ちを上書きして解除しない。
単一のatomic loop保存後に通常Driverが配車する。保存前の失敗は元状態を保持し、保存後の
応答喪失では新しいloop状態を読み戻す。同じdigestを再送してレビューを重複させない。
新レビューは全指摘を再評価し、旧指摘の免除や承認への昇格は行わない。

## Help判断登録の待機

`submit-help-review`はscheduler leaseを単一callerで待つ。定期tickが承認を照合している間に
短時間のHostBusyとshell再起動を繰り返さない。待機前は入力形式だけを検査し、loop読込・
receipt作成・状態更新はlease取得後とする。取得後のexact loop、統括、source、対象path、
停止理由の照合は省略しない。同じ判断を複数callerが待った場合も、先行commit後のdigest不一致で
後続を拒否する。role・workspace・heavyのfail-fastやdispatch再送禁止は変えない。

利用者のpause/closeは待機より優先する。取得後、receipt直前、loop保存直前に同じUI control
snapshotを確認する。停止・所有変更ではvalidationを再開せず、待機中の中断ではloopやreceiptに
触れない。receipt保存後・loop保存前の拒否は承認済み工程として扱わず、現在のloopを確認する。
この変更は開発用admissionのみであり、ゲーム入力・表示・Help catalogには影響しない。

## 統合後の段階補正（M18）

### Native batch登録のcontroller選択

新規native登録は[host登録API](orca-native-host-execution.md)のcontext/register/statusを使う。
統括の現subject・owner・loaded controller・recipe契約を保存前に照合し、通常のvalidation CLIへ
host markerを持ち込む経路は拒否する。登録は実行ではなく、実行には別のinspect/launch照合が必要。
保持・整理はprimaryの絶対入口
`python3 /home/satotakumi/projects/hell-workers/scripts/dev.py validation`を使う。
製品worktreeの相対helperは凍結subject側の版であり、同じ台帳を見てもhost側controllerと
同じ版とは限らない。`--primary`で台帳を指定するだけでは実行helperは切り替わらない。
specのrepo/command/verifierは製品subjectのまま保持する。

版不一致が送信前拒否で、launch intent・process・成果を照合して未実行と証明できた場合だけ、
primaryの正規seal（abandoned、理由、verifierなし）とfinalizeで旧登録を閉じ、同subjectを
別batch IDでhostへ正規登録する。Runは維持し、旧登録hashの上書きやbridgeのpin緩和をしない。
結果不明の実行はこの経路で再登録・再実行しない。M19のprimary CLI案内はH1のhost登録契約で置き換えた。

### 工程継続通知のreceipt契約（H2-a）

planning通知は入力受理・turn開始・正式successor登録を分離する。
`orca_prompt_delivery.py`は純粋validatorであり、入力送信やraw Enterの回復機能を持たない。
request/Run/世代、前後node、source、terminal incarnation、operator control、本文digestを
送信前WALへ保存し、保存後の送信直前にも再照合する。Orca runtime UUIDだけの変更は
terminalの入れ替わりと区別する。同IDの再呼出しはOrcaのreceipt照会であり、本文の再入力ではない。
first receiptとlatest readbackを保持し、不明な送信結果を捨てない。

appliedには正式registration、predecessor archiveのpath/digest/target、前nodeのbindingを要求する。
旧・破損planningは検証前にappliedへ昇格させず、非書換のblocked投影とする。
connection等を更新する別writerでも元planningの内容を維持する。
exact旧appliedで同じ後続bindingが正式登録済みの場合だけ、scheduler→requestのlock順で
旧全文と登録loop全文/digestをprivate retirement journalへ保存し、planningをclearする。
crash replayは同じfresh loop・同じ旧planningを再照合し、別状態には適用しない。
これは通知の不要化であり、旧turn開始や依頼完了の証拠を新造しない。
コードの固定reviewと44件の関連試験、H3-aとの全tooling 1,447件＋164件が成功。
同統括sessionへの配備で、旧planningを正式loopと照合して履歴保全・退役したことを確認した。
全tooling・実切替の詳細は
[収束計画](../plans/orca-system-hardening-plan-2026-09-29.md)で管理する。

### 段階補正の契約

H3-aでは、Driverの型付き`ExecutionConnectionUnavailable`を接続待機として再観測し、
HostBusyは資源待機として表示する。どちらも新しい実行の成功・作業開始を意味しない。
同じconsumerを維持し、利用者pauseや未知例外を自動解除しない。
配備・実受入は収束計画の記録を正とし、候補コードだけで適用済みとは判断しない。

統合後の`correction_required`で全指摘が単一担当の実装だけでは閉じない場合、
既存`route`のprivate specへ`loop_sha256`と`repairs`を追加する。
`repairs`は全sealed finding IDをkeyに持ち、各値は
`owner`・`paths`・`action`・`acceptance`を持つstep配列とする。
同じfindingを実装と実機・文書に分解してよい。IDの省略や未知ownerは拒否する。

- `worker-a`: 現ticketの書込scope内の明示path。docs編集やasset採否を委譲しない。
- `coordinator-native`: pathsは空。既存の独立アート判断、実機、release等の正規経路で処理する未完作業。
- `coordinator-docs`: `docs/*.md`の明示path。primary正本の統括所有を維持する。

少なくとも実装stepと統括stepを1件ずつ要求する。実装Aだけを同会話・同scope/baseへ戻し、
元review、全指摘、repair plan、統括の未完工程を統合履歴に保持する。最終固定レビューには
全指摘・統括工程を再提示し、実装承認をnative/assets/docs受入へ読み替えない。
単一atomic loop保存で次工程を登録し、外部配車は通常Driverが行う。
exact loop、承認済みworker checkpointと統合source、全終了/release/ACK、現所有、
native非競合、利用者controlを確認する。保存直前の停止・所有変更は無書込で拒否する。

これは収集driverやhost recipeそのものを生成する機能ではない。offline verifierだけがある場合、
製品側の収集producerを実装・検証・固定reviewした後にhost側の具体的なrecipe契約を追加する。
空のrecipe、任意command起動、観測値の合成で受入を通さない。
Help No impact: 開発用routingとレビューpromptのみで、ゲーム入力・表示・runtime dataは変更しない。

2026-09-29のTAK-14で固定review・最終tooling（Python1356/Blender164）・contracts・
primary storageを通過後、同統括sessionへ読み込み直して実適用した。同Run/generation12を維持し、
finalized native eventの`correction_routed`後続証拠、同実装A terminalの新Dispatchと
観測producer・収集driver・verifierの調査開始、監督表示working/implementationを確認した。
これは段階補正と再開の受入であり、製品native/assets/docsの全体受入完了ではない。

## レビュー完了本文の形式訂正

`review_format_retry`はupstream送信前の形式拒否であり、完了結果不明ではない。
形式不備の回数だけではbridgeを失効させず、同一Dispatchでの訂正を許す。
Pythonの`chr(10).join([summary, 'ORCA_REVIEW_JSON:' + json.dumps(record)])`で実改行を作り、
shellを介さずsubprocessのargvの1要素として渡す。literal backslash-nやJSON quoteの二重escapeを避ける。
本文を自動修復・承認へ変換する処理は入れない。正しいJSONでも対象hash・所有・権限が違えば拒否する。
一般のrequest budget・source検査・未知操作の再送禁止は不変。
旧実装で既に失効したbridgeはそのまま再利用せず、正常終了と正式Dispatch終了を確認し、
既存guard付き復旧で同じ固定reviewer会話へ戻す。旧本文からの承認捏造は行わない。

## 終了済み工程と作業場の寿命

最終統合承認・全settlement/ACK・タブ終了記録が成立した工程の承認は、統合先の
保存receipt・現在のsourceと固定最終レビューで照合する。終了したworker checkoutの存続は要求しない。
統合前、最終承認前、終了記録前のlane検査は従来どおり維持する。最終統合先の変更や
証拠・所有・settlementの不一致は停止する。これは作業場や履歴を自動削除する機能ではない。

旧tickが終了後のlane検査で停止させた場合は `orca_closed_loop_recovery.py` を使う。
検査済みloop digest、UI終了時のsubject seal、最終承認・統合receipt/source、Run所有、
全attemptのsettlement/ACKを照合する。最後のapproved→pausedだけを除いた状態が
終了時のsealと完全一致するときだけ復元する。元の停止台帳と前後digestを先行保存し、
atomic saveで同工程を戻す。新Run・再配車・再検証・承認の新規作成はしない。
利用者停止は入場時と保存直前に拒否する。途中失敗後は現在の状態を読み、結果不明の操作を再送しない。
稼働Driverが旧コードを保持している場合は、同統括の正常終了を確認して同会話を再開する。

## 編集前に判明した担当scope不足

単一Aが必要な共有境界を編集できないと報告し、変更前に正式failed終了した場合、
統括は既存依頼の仕様・受入条件を維持して不足directoryを判断する。利用者に内部scopeの
再承認を求めない。`orca_scope_replan.py --request-id ... --coordinator ... --spec ...` に
fresh `loop_sha256`、整列した`allowed_directories`（元範囲を包含）、`reason`、`follow_up`を渡す。

helperは全settlement/release/ACK、同一Run/Task、終了済みprovider、clean source/base、
統合先、担当tab incarnation、現request bindingと利用者controlを照合する。
旧loop/plan/ticket/assignmentは保存し、同じTask・会話・tabへ新しいticket世代のretryを接続する。
validation・受入条件・request scopeは変更しない。担当の書込みmountは新起動でのみ切り替わり、
生きた担当へ後付けで権限を広げない。新Runや代替Taskで失敗を隠さない。

旧失敗は失敗のまま残る。製品変更、checkpoint済みsubject、別停止、未ACK、所有不一致、
保護path、利用者停止はこの経路で処理しない。receipt保存後の中断では同じspecと未変更状態を
照合し、ローカル保存だけを再開する。適用直後と完全一致するspecの再提出だけは同状態を返し、
配車等で既に進行した状態への再提出は拒否して現在状態の確認を求める。
実際の起動直前にもreceiptと元Task/終了/所有を再照合し、正規dispatcherのretry-ofを使用する。

Orcaの元Task本文は履歴として維持する。改訂receiptに拘束した追記が旧本文の編集pathと
contextの対応項目だけを置き換え、Task/Run、受入条件、禁止操作、baseは維持する。
receiptの形式不正は制御可能な拒否として扱い、Driverを構造例外で終了させない。

CLIからの適用は既存の認証済みhost brokerへexact specを渡す。隔離統括側の`/proc`を
hostのshell終了証拠に使わない。brokerはrequestとterminalを自分の所有情報から選び、
任意command/path/別terminalを受け取らない。scheduler取得後と保存境界で呼出元の生存・
所有を再確認し、host側で同じ終了・source・Task・tab検査を実行する。

## 統括によるHelp更新と実装終了sourceの継承

実装担当の終了sourceを、統括がHelpを更新したsourceで上書きしない。
`orca_help_source.py`はHelp停止時のprivate baselineと終了担当のticket/session/sourceを拘束し、
旧Helpをメモリ内で復元したfingerprintが終了sourceと完全一致するときだけ継承を作る。
runtime Help sourceとapproval snapshotの両方の変更、fresh Help review、同じrequest/Runを必須とする。
HEAD/index/非Help変更、symlink/hardlink、証拠の不一致は拒否する。

Help reviewと同時にloopへ受理されたことを保存して初めて継承を有効にする。
receiptだけ残った未受理の判断は使わない。loop保存後の中断では通常のvalidating tickが
exact current loop/sourceを再照合し、欠けたローカルacceptance保存だけを完了する。
検証・checkpoint・失敗後の世代・固定reviewの全経路で同じ証拠を再検査する。
Helpがworker scope外でも、統括所有と証明したexact pathだけをcheckpoint/read-only reviewへ含める。
workerの書込mountは広げず、candidateのHelp blobもref更新前と固定review前に照合する。

既存の停止は`resume-help-source`で回収する。停止理由だけで検証未実行とは断定しない。
`orca_help_fence.py`が同統括session/terminal/launcherとPID namespaceの正常終了をpidfdで観測し、
source、同期local CIのargv/controller bytes、外部/native操作の終端状態を前後照合する。
この経路の依存検査は同期のlocked cargo-deny checkまでを含み、公開操作・任意commandは扱わない。
結果不明の旧試行を履歴に残し、同sourceの保存済み検証結果があれば再実行を拒否する。
新attempt UUIDと正規再構築した前後loopを先行保存し、同会話の通常再開後にCASでvalidatingへ戻す。
実検証は通常Driverだけが開始する。新Run、担当再配車、検証成功・review承認の代作は行わない。

Help影響はNo impact。この基盤変更は開発用証拠の継承と停止回収だけであり、
ゲームの入力・表示・runtime data・Help本文は変更しない。TAK-14の製品Help更新とは別の判断である。
2026-09-29、固定レビューAPPROVED、関連215 tests、全tooling1326+164 tests、Ruff/actionlint/perf、
明示baseのcontractsとprimary storage checkが成功。旧統括の正常終了fenceを取得し、
同じsession/terminalを通常再開してprovider内のfence照合も成功した。
TAK-14のguarded recoveryは同Runをactiveへ戻し、通常Driverの`validation_running`へ移行した。
既存実装作業場でhost validation実process（PID706223、Driver606351）と子検証の起動を確認した。
これは停止回収と次工程開始の受入であり、製品の検証合格・固定レビュー完了・依頼全体完了は意味しない。

## 統合検証で確定したtooling失敗の修正

`orca_tooling_correction.py`の`category: validation-tooling`は、完了済みnonzero検証を
保存ledger・digest・head・source・commandと照合する。成功、結果不明、別sourceの証拠は拒否する。
統括は元target baseからscripts/限定の修正commitと同じvalidationを指定する。
旧失敗はwrite-ahead journalへ残し、ゲーム差分・worker承認を改変しない。
修正した新しい統合subjectには新Help判断、実検証、固定reviewが必要であり、旧成功を流用しない。
commit済み・loop保存前の中断ではexact journal/receiptからローカル投影だけを回収する。
新経路はvisible owner、request binding、execution connection、fresh Linear、native終端、
UI controlを確認し、Git/WALの変更前とloop公開前に同じcontrol/owner/loop/sourceを再照合する。
途中の利用者停止ではpausedを維持し、commit済みreceiptを破棄・再実行せず、正規復帰後に回収する。
保存検証証拠はproducerのexact schema、型、hash、diagnosticとexecutor形式も照合する。
同じTAK/Run/session/tabを維持し、新Runや無変更の盲目的再試行で停止を隠さない。

MCPの疑似backend fixtureはテスト専用subprocess harnessでcoordinationを分離する。
実flock・daemon共有・idle再起動は引き続き検証し、実accountのheavy slot競合や空きRAMで
単純なJSON fixtureの成否を変えない。本番adapterのhost/RAM guardは変更しない。
Help No impact: 開発テストと統括所有の修正経路のみで、ゲーム入力・表示・Help本文は不変。
M15は固定review APPROVED、最終tooling1332+164件とcontracts/docs/storageに合格。
同統括がテスト限定補正commitを正規適用し、新統合head
`ba1df06caa70a68e6e07c49f356ede43da0538cb`でfresh Help判断を登録した。
同Runのvalidation_running、実host validatorと子process、UI working/validationを確認済み。
これは停止修正と同依頼再開の受入であり、製品検証・固定最終review・TAK-14全体完了ではない。

## 全体受入の失敗から不足実装へ戻す経路

`orca_acceptance_replan.py`は、正式failed・終了/release/ACK済みの単一Aによる全体受入を対象にする。
統括が元の依頼scope・受入条件を維持して補完計画を作り、fresh `loop_sha256`、`reason`、
通常の`build_loop`形式の`replacement`をprivate specとして渡す。別のclean作業場とfresh ticketを使い、
旧監査作業場・監査source・会話は変更しない。統括正本文書の所有も変更しない。

これは承認世代の追加ではなく、同じ未承認工程の再計画である。同じloop generationとapproved predecessor、
Runを維持し、旧失敗attemptはcurrentのaccounted historyへ残す。旧loop/spec/proofはdigest拘束した
private archiveへ保存し、読込・再提出・依頼完了・bootstrap accountingで照合する。
失敗を承認へ昇格させず、新しい候補には通常のHelp判断・変更別検証・固定reviewを要求する。
継承する全attemptには明示release/ACKを要求する。bootstrap中断fenceだけで終了を証明した
attemptは、旧lane historyに依存するためこの再計画へは受理しない。変更前に拒否し、ACKの合成や
新laneへの旧状態遷移のコピーで帳尻を合わせない。

隔離統括からは認証済みhost brokerへexact specだけを渡す。helperと依存moduleのpin、
caller生存・所有、利用者control、保存前後のloop digestを検査し、未知操作や別Runを拒否する。
archive後の中断は元の失敗loopを維持し、同じspecの再提出でローカル登録だけを完了する。
Help No impact: 開発用再計画と監査証拠の保存のみで、ゲーム入力・表示・runtime data・Help本文は不変。
2026-09-29、固定review APPROVED、関連136 tests、最終tooling1350+164 tests、Ruff/actionlint/perf、
contracts・primary docs/storageに合格。旧統括の正常終了後、同terminal/sessionでbrokerを更新し、
統括が保存specを正式適用した。同Run・generation12のactive/implementingと実装Aの実コード編集を確認。
旧監査とfailed attemptは保持した。これは停止回収の受入であり、補完実装・製品全体の受入完了ではない。

## Help更新済み結果の監督表示（受入記録）

loopのHelp判断`updated`は実施済みの更新を表す。監督UIの既存impact分類は
`update_required`/`none`なので、`confirmed_result`の表示境界で前者へ変換する。
保存されたHelp判断・review receiptは変更せず、未知値の拒否も維持する。
工程の承認と依頼全体の完了は別であり、この変換で後続工程を完了へ昇格させない。
Help No impact: 開発UIへのprojectionだけで、プレイヤーHelp本文・操作・ゲーム状態は不変。
2026-09-29、関連73 testsと最終全tooling1333+164件、contracts/docs/storage、固定reviewが成功。
既存のsource/runtime/generation拘束付きrefreshで表示controllerだけへ適用し、health正常と
同runtimeのsnapshot更新を確認した。統括・実装担当・同RunのM6進行は中断していない。
