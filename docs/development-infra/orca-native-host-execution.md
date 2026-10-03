# Orcaのhost実行と表示サービス更新

> 歴史資料（2026-10-04整理）。独自統括は廃止済みで、以下のcommand・Run・復旧経路は実行しない。
> 現在の規則は[通常Orca運用ガイド](../orca-quickstart.md)。旧未完条件と過去の結果は履歴のまま保持する。

2026-09-30。隔離統括からの候補生成・独立検証・終了処理。
製品依頼の完了条件は[依頼ライフサイクル](orca-request-lifecycle.md)を維持する。

## 実行契約

`orca_native_bridge.py`は統括のprocess subtreeだけを受け付ける。任意commandではなく、
hostが認証・recipe検査・subject照合を行って登録したbatchとdigestを指定する。
凍結worktreeの古いvalidation controllerは使わず、直接CLI登録したmarkerなしbatchも起動前に拒否する。

### 登録・版照合（H1接続、実受入の結果は収束計画へ記録）

統括内部では `orca_native_registration.py context --request-id ...` でhostの
subject（request binding/Run/generation/repo/head/source）、owner、controller hash、recipe revisionを取得する。
これと既知recipeのspec/commandを一つのprivate intent fileへ保存し、`register --intent-file ...` を送る。
利用者にintentや内部IDの入力を求めない。登録と実行は別操作で、登録API自身は外部commandを起動しない。
通信失敗時は同じintent fileの `status` で照会する。clientは要求を自動再送しない。
登録済みの同要求は同じbatchを返し、異なるintent・owner・subjectの同ID利用を拒否する。

本人確認とoperator controlに加え、scheduler待機後にもfresh execution connectionを確認する。
storage lock内ではネットワーク通信せず、保存直前のcallbackでcaller/control/source/recipeを再確認する。
起動側も保存markerを再検査し、native waitのscheduler CAS内で登録時subjectと現在subjectを一致させる。
同じHEADでもRunやgenerationが違えば起動しない。

controllerは実際に検証してcompileしたbytesのhashを保持し、disk hashと別々に照合する。
対応版はprimaryの `96491adeaabed6017eb15f7564a3c3ee6399344ea084e2c779b68bc3d4b10490` と
候補の `53eff2b795625d58be1cf5e88362b43541da1d5e03569c740ab5bbc966d7e446` のexact二版。
両者は同じ登録preflightを持ち、候補だけが従来の直接CLI child用heavy lease wrapperを持つ。
primaryへ未導入依存をコピーしない。hostからの実行は従来どおりhost所有leaseを使う。
bridge/registration/controllerのdisk drift時には、新しいhashで旧receiptを書き直さず拒否する。
`host_admission`は入力specで常に拒否し、`native_registration`はhostのpreflightなしでは拒否する。
callback成功と再admissionの後だけstorageがcontroller/marker digestの証明情報を保存する。
起動側はその証明も照合するため、通常の登録CLIでmarkerだけを書いても起動許可にならない。

旧 `86662716dc35e9dbed67ece4eb8d5c3ecb653c76e6e23d4dd5ac183e6b872ff0` controllerのbatchは、
finalizedの場合だけread-only履歴として参照できる。旧batchを再起動・再検証・再登録しない。
履歴参照で撤去済みhelper/outputを再要求しないが、owner/repoと保存証拠の照合は維持する。
更新は全host実行の終了と統括の正常終了を確認した安全点で行い、同じ会話・タブを再開する。

`orca_native_recipe_catalog.py`のliteral registryだけがargvを定義する。
`building-m2-generate-v1`はTank/MudMixer、planes/strokes、名前、登録rootに限定したbuildとexact verifier。
prepare・release・任意shellは許可しない。登録helperのhash・commit内容に加え、workflow内の
scripts/bin/fixtures/templates/vendor全体をbatch.subjectのGit treeへ照合する。
未追跡・ignoredファイル、欠損、symlink、mode差、byte差を起動前と結果確定前に拒否する。
host子processはPython bytecodeを書き込まず、成果は登録した専用output rootへ置く。

`building-m2-interactive-v1`は統合済みhelperの通常ゲーム観測を登録する。
外側planの登録hashから参照runtime planのhashへ結び、同じrepo・subjectと
非承認scopeを照合する。codecは既知のdebug/profiling build、または登録時にそのbuildと
byte一致したrepo/target内の保持copyだけを許可する。参照plan・codecは結果確定時も
同じhashを要求するため、feedback rebuildでは別の保持copyを用意する。
copyの具体的consumer・終了条件は通常のvalidation保持台帳で管理する。
観測passをlifecycle操作結果・数値凍結・性能・アート採否・release承認へ読み替えない。

registryはbroker起動時のsnapshotに拘束する。検証済み契約の反映は、idleな統括の正常終了と
同じsession・terminalでの再開によって行う。Orcaアプリ全体の再起動や新Runは不要。
再開後にcapabilitiesを再読し、受付成功だけでなく次の具体的処理への着手を確認する。

## 競合と終了

共通atomic writerは未公開の一時ファイルを `.requests-*.tmp` とし、
正式な `*.json` 記録と名前空間を分離する。file fsync、atomic replace、directory fsyncは維持する。
native履歴・所有引継ぎ・Driver列挙は、旧writerの予約済み `.requests-*.json` だけも除外する。
通常の正式eventの破損・digest不一致・欠損は引き続き拒否し、記録を消して復旧しない。
書込み途中を正式eventと誤認する停止を防ぐための基盤共通修正であり、TAK-14専用例外ではない。

private JSON読込は、`O_NOFOLLOW | O_NONBLOCK`で開いた同一fdをfstatし、
regular file・所有者・mode・hardlink数を検査してから読む。atomic replaceで旧inodeが
unlinkされた場合だけ、pathが別inodeへ移ったことを確認して再読する。不正権限・symlink・
hardlink・破損JSONの拒否は緩めない。連続置換は一時I/Oエラーとして扱う。

`resume-review-bootstrap`は、起動済み固定reviewerを再起動せず、未配車の
bootstrapだけを引き継ぐ。canonical ticketと起動prompt hash、source、同一会話・pane・
launch ID、未arm bridge、RunのTask/Dispatch全件照合、現行統括権限を必須とする。
未反映の追加指示とUIのpause/closeも送信前・arm前に確認する。既存Taskのretryは対象外。
送信前WALと受信結果の保存を分け、unknown送信を再実行しない。受信済み結果のarm・
台帳反映だけは同じ証拠・phase別schema・canonical argvを再照合して再開できる。
承認済み先行loopはdigest・Run・世代・唯一のDispatch所有を照合して履歴として扱い、
旧sourceの再受入は要求しない。worker-listはcursorを最後まで読み、件数上限で履歴を切らない。

Driverと統括commandは同じworkspace leaseで直列化する。統括commandはlease解放を待ち、
通常の競合を失敗として利用者へ返さない。heavy・role admissionは従来の非blocking調停を維持する。
待機後に受付状態、所有者、batch、subjectを再検査する。
provider終了時にはDriverのlease解放より先に受付を閉じ、待機要求が終了後に動くことを防ぐ。

開始前のnative wait登録では、native用leaseからscheduler leaseの順で直列化する。
schedulerの短時間timeoutを統括へ繰り返し返さず、登録CASの前で解放を待つ。
取得後はoperator control・現行subject・正式predecessorを再照合し、起動intentを書く前に
provider受付終了とcaller権限も再確認する。heavy/worker枠の即時拒否は変更しない。
この待機は未知の外部起動を再送する許可ではなく、未送信登録の最初の起動に限る。
launch clientはconnect/writeの45秒timeoutを維持した後、同じUnix接続で返答を待つ。
応答待ちだけを時間切れにして権限照合対象のcallerを消滅させない。別要求は送信しない。
接続終了・server終了・callerの中断は待機を終了し、caller終了後の起動は権限検査で拒否する。
状態照会など他actionの45秒timeoutは維持する。

生成exit 0だけではpassとしない。独立verifier成功、結果照合、具体的な保持用途の登録、
finalizeとstorage checkを区別する。既知の生成を再送せず、同じbatch/digestで終了処理する。
候補生成のpassはアート採否・正式導入・依頼全体の完了ではない。

### 修正前後の履歴

native eventの登録とpredecessor走査は同じsubject境界規則を使う。
同じ依頼・scope/node・Run・世代・repo内で、head/sourceだけが異なる旧eventは、
登録batchの正式finalized結果とapplied successor証拠が一致する場合だけ履歴として保持する。
旧subjectの成功を現行subjectの受入へ読み替えず、未適用・欠損・別workflow・循環は拒否する。
履歴件数に一律上限を設けず、実際の有限event集合と循環検出で確認する。
新waitへの接続でも、旧finalized batchはsealed metadataのID/digestとresult proofを確認する。
旧batchへ実行admissionを再適用して現在のcheckoutのdependency closureを要求しない。
これにより正式終了した履歴の存在を、新sourceで旧recipeを再実行できることに依存させない。
新しいbatch自身のrecipe・source・所有・rootのadmissionは従来通り必須である。

中断回復は、保存済み全process birthの終了を正に確認した**後**、batch/digestを再読する。
終了観測中にrunning/sealing snapshotが正常終了記録へ変わった場合は、回復WALやinterruptedを
書かず次tickの通常検証へ戻す。登録digestの変更は拒否する。
schema1回復WALにはそのpost-exit snapshot全体・hash・process identitiesを保存し、再開時には
元snapshotまたはprimary recoverの厳密な更新後状態との一致を必須にする。
snapshot証明を持たない旧WALから未確定結果を中断確定へ進めない。
正常終了と終了検出の競合を失敗へ変換しない。既に正式記録したinterruptedを手編集してpassへ
変更せず、具体的診断用途で保持・finalizeして同じ依頼の次工程へ引き継ぐ。

Linearのfresh観測不能はtyped待機とし、通知IDや配送phaseを恒久blockedへ変えない。
旧版がその正確な接続未確認理由でblockedにしたeventだけは、同じDriver lease内で
active control・同session所有引継ぎ・fresh Linear権限・batch/result・text hashを照合して回復する。
accepted/started復元は保存した初回raw receiptと最新raw readbackの両方に拘束し、
初回stage subset・最新prompt exact一致を要求する。欠損、別ID、非受理、異常envelopeを拒否。
prepared/sendingの証拠形状も検査し、元IDによる通常readbackだけを継続する。
課題の完了・取消、scope差異、所有不一致を接続エラーとして無視しない。

### 送信前の拒否

`host_busy`限定の再送条件は既にlaunch receiptを持つattemptについてである。
admission/continuation登録の明示的拒否がlaunch WALと外部起動より前であるとコード・応答から確認し、
fresh inspectでregistered・launch/execution/seal空、root未作成・同所有・同subject/digestを確認した場合、
同じlaunchで全guardを再評価する。未送信の初回起動であり、unknown送信の再実行ではない。
receipt不在だけで未送信とは判定しない。prepared continuationは既存registerの照合に任せ、
receipt/event削除、新batchへの付替え、unknown/sending/launcher_failed再送は行わない。

## 表示サービス

### 監督付きproviderの対話と通信先

統括・Codex担当・固定reviewerの新規／再開起動は
`notice.hide_rate_limit_model_nudge=true`を渡す。配車文字列がモデル提案menuへ
入力される競合を防ぐものであり、利用上限や権限確認を解除せず、モデルを変更しない。
`input_accepted`は着手証拠ではない。現在のDispatch本文・新turn・具体的tool実行を照合する。

Task bridgeは隔離環境内の`ORCA_CLI_COMMAND`を、その起動のUUID固有read-only clientへ
固定する。providerへ渡す最新bootstrapも同じpathを明示し、再開履歴の旧bridge・authorityは
失効した文脈として扱う。live Dispatch受領後、現bridgeのheartbeat受理を確認してから
調査・編集を開始する契約とする。環境変数は通信先の発見手段であり、capabilityの代わりではない。
旧bridgeのaliasや広いmountを足さず、現在のbridgeの所有検査・settlement・ACKを維持する。
prompt指示だけで誤用が不可能になったとは扱わず、実通信を継続照合する。

2026-09-28の復旧では、旧bridgeへのshell起動がENOENTとなったこと、現bridgeの
operationsが空であることを確認した。現Dispatchへの正式follow-upと最新bootstrapの参照で
同じ担当を復旧し、製品編集を繰り返さずDelivery処理・ACK・正式完了通知へ進めた。
通知後の`prompt-text`待機残留、モデル選択の保全、着手表示の恒久連携は
[配車是正計画](../plans/orca-dispatch-submit-plan-2026-09-28.md)の未完了項目である。

`orca_supervision_refresh.py`はsource世代・runtime・PID・健全性を記録する。
検証済みsourceと現在generationを明示したrefreshだけを受け付け、編集中の自動reloadはしない。
所有する相談・routing・同期処理が終わるまで待ち、同じPIDで再読込する。利用者の終了操作を優先する。
古い要求の再適用を拒否し、snapshotの継続更新とhealthを別々に確認する。
旧processへの初回導入だけは、安全な区切りでlaunch-supervisedからアプリを再起動する。

## 受入記録

2026-09-28 再開bridge指定補正: 固定read-only review承認、関連79 tests合格。
base/head `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、変更別source
`fe8a91fc6d0836cd0f3722dd83318047282a0fa5a0b2e9dac20586300343f119`で
contracts/tooling 1129+164 tests、Ruff/actionlint/perf成功、exit 0。
旧pathへの未送信失敗を照合した同Dispatchが、現bridgeでcheck/ACK/worker_doneを完了。
role recorded/exit 0、統括の新turnによる正式受領・Help提出・次の検証開始を確認。
環境変数の固定は次回起動から適用する。今回の稼働担当を設定反映だけのために再起動していない。
Help No impact、primary storage 152 batches / 486367547392 bytesでpass、削除なし。
モデル選択保全・着手表示等の残件と製品全体の受入は完了扱いにしない。

2026-09-28 private読込・bootstrap復旧: 固定read-only review承認、復旧16 tests成功。
base/head `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、変更別gate source
`57c904a22979b50957c1117525b464e3ebb9b1cad8391deb0fbab067982d2028`で
contracts/tooling 1,128件＋164件、Ruff/actionlint/perf self-testがexit 0。
修正中の2回のgateはsource変更を検出してexit 1となり、最終成功へ流用していない。
Help実レビューはNo impact（host private stateと開発用role管理のみでゲーム操作・表示は不変）。
旧統括PID2539503の正常終了exit 0後、同じ会話・tabでPID3424167へ再開した。
実状態の81 Task・89 Dispatchを現行と承認済み先行loopへ照合し、既存bridge
`da94b07f-f6dd-4738-bad4-574b61e293c3`を新規起動せず引き継いだ。
同Run `run_3236488f0dff`のDispatch `ctx_a155d175816e`が受理され、
loop active/reviewing、実reviewerの通信、worker succeeded/settled、bridge settledまで確認。
統括も復旧を明示的に受領し、同じ固定reviewの返却処理へ進んだ。
返却は正規の`changes_requested`（M2-RELEASE-SCOPE-INCOMPLETE）。監視側の復旧待ち解除を
同じ統括が受領し、実装不足・残受入証拠・正式releaseへの分類と保存済みreviewの実読を開始した。
これは停止原因の補正・実復旧であり、TAK-14全体の完了・全指摘承認ではない。
検証時storageは152 batch、486367080448 bytes、未分類0・上限なしでpass。
元成果・履歴・会話・build cacheの削除なし。push/PR/製品編集なし。

2026-09-28 scheduler admission補正: 固定read-only review承認、実lock/socket待機を含む
bridge40 tests成功。base/head `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、基盤source
`942042b77485d72fa342030406245b53d03cfec1b5bfcd6395a466faa7e9342f`で
変更別contracts/tooling、Ruff/actionlint/perf self-testがexit 0となった。
storageは148 batch、485052903424 bytes、未分類0・上限なしでpass。原本・cache削除なし。
Help実レビューはNo impact（開発用起動調停のみで製品の操作・表示・assetは不変）。
同session/tabで登録済みMudMixer world-near-farを初回起動し、独立seal pass、
04:30:58Zにfinalized。終了通知の入力確定が証明できない別問題は、同一入力欄のexact本文を
照合して本文再送なしのEnterで継続した。この配送問題自体の恒久解決は本修正に含めない。
最終gate後、監視側が掛けたnew native/build保留をreceipt
`56b9f573-6780-411c-9534-016872d6e23a`で解除した。受付receiptだけでなく、同じ統括の
新turnで解除受領・MudMixer catalog consumer実機確認の選定・native Skill読込を確認。
TAK-14全体は未完了であり、既存Runと固定reviewを維持して次工程を続ける。

2026-09-28 正常終了競合・一時接続待機補正: 固定read-only review承認、関連125 tests成功。
base/head `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、基盤source
`3b84b47c417d4d746f6a47a384413ea3db3ac46ed2967b4d097ecc7ff929571f`で
変更別contracts/tooling 1,102件＋164件、Ruff/actionlint/perf self-testが成功した。
同session/tabのDriver PID2090974が旧接続blockedを元通知ID・raw receiptのまま回復。
未送信だった登録済みMudMixer refining v2を統括が初回起動し、exit 0・独立seal pass・
保持登録・finalized・固定review返却まで確認した。旧v1のinterrupted記録は変更していない。
検証時のprimary storageは139 batch、483599949824 bytes、未分類0・上限なしでpass。
既存review/native成果を具体的consumer用途で維持し、原本・cacheの削除は行っていない。
Help実レビューはNo impact（開発用終了判定・配送制御のみ、製品の操作・表示・asset不変）。
重い最終gateとnativeの競合を避けるための監視側一時保留は、gate終了後に同じ統括へ解除通知。
解除文が同じincarnationの入力欄に残った際は、本文を再送せず表示内容を照合して確定し、
実際の新turn開始を確認した。配送受付だけを工程再開の証拠とはしない。
統括は解除を明示的に受領し、Tank lifecycleの未取得証拠選定、native Skillと保存手順の
再読を開始した。TAK-14全体の受入と完了判定は引き続き同じ統括・固定reviewが担当する。

2026-09-28 atomic公開補正: 固定read-only review承認、関連78 test、変更別contracts/tooling
1,093件＋164件、Ruff/actionlint/perf成功。base/head
`ae5b2066c4fd39a8dd6a6c439e272d125f749241`、dirtyを含むCI classifier source
`b3d28c189fd9f37e2aacce61437d7bed6e7cd0f2a6c0d7d2d3b95d70e389e7ce`。
旧統括のexit 0確認後、同session/tabへ再開。既存
`tak-14-m2-ceaba666-mud-mixer-placement-native-20260928-v1` は再launchせず
独立検証pass/finalized、通知applied/native_review_registeredまで到達した。
固定review返却後も統括の次の受入準備を確認。依頼全体の完了ではない。
primary storageは136 batch、483134173184 bytes、未分類0・上限なしでpass。
既存review/native成果は現consumer用途で保持し、原本・cache削除なし。
Help実レビューはNo impact（開発用state公開・走査のみ、ゲームの操作・表示・asset不変）。

TAK-14の同じRun・統括会話・tabを保持して導入。Tank planes候補のhost生成と独立検証はexit 0。
`tak-14-m2-tank-planes-host-20260928-v2`はpass/finalized。競合で止まった終了処理も、
直列待機への修正後に同じbatchで成功した。上部snapshotは新runtimeで継続更新を確認。
監視側による製品編集・ゲーム検証代行、元生成の再送、台帳直接編集は行っていない。
Helpは開発基盤内部のみの変更としてNo impact。ゲームの操作・表示・runtime assetsは不変。
これは基盤の受入であり、TAK-14/M2全体の完了ではない。
履歴境界補正前の基盤source `a75ed744fe8757eaf58766a9bb78249da7159a12a72dbe08c0739be4ed3c3cd0`、
base/head `ae5b2066c4fd39a8dd6a6c439e272d125f749241`でcontracts/toolingの1,062件＋164件、
Ruff/actionlint、perf self-testを通過。固定read-only review承認済み。

履歴境界補正では関連91 testと固定reviewを通し、統括を正常終了して同じ会話・tabで再開した。
未起動だった`tak-14-m2-tank-strokes-host-20260928-v1`を同じdigestで実行し、
続くMudMixer planes/strokesも同じ統括の終了通知から継続した。3 batchともpass/finalized。
統括が4候補の比較に着手したことまで確認した。旧検証のpass流用ではなく、現行subjectでの
生成・独立検証・保持登録・正式終了の結果であり、数値凍結やアート承認の代用ではない。
続く正規`resume-review`が受理され、同Runの固定reviewへ`review_dispatching`を確認した。
補正後source `de6d2d0353ea3a4a8b1cf359772eb6c138732d437020fe345729720aa774c064`、
同base/headでcontracts/toolingの1,065件＋164件、Ruff/actionlint、perf self-testが成功。
primary storageは123 batch、479437672448 bytes、未分類0、上限なしでpass。
既存の比較・review consumerが必要な成果を保持し、今回の復旧で原本やcacheは削除していない。

interactive登録は固定read-only review承認後、同base/head、基盤source
`14760b840b5032da04f292a57a9580709db0062dc77c831b3a7f195503c3cefc`で
contracts/toolingの1,084件＋164件、Ruff/actionlint/perf self-testを通過。
統括のexit 0・旧process終了を確認して同じ会話・tabで再開した。
稼働host revision `9b3d57d7babf697a314c2e0b38f96409c113b688cffd9c5415cf07b18b670ca5`の
capabilities実読で新recipeを確認し、統括が現行head用の二種runtime plan準備
（既存候補・codec hash・保持用途の照合）に着手した。入力受付だけを再開証拠としていない。
この記録は登録不備の修正・再開確認であり、interactive実機のpassやTAK-14全体完了ではない。
Help No impact、primary storageは125 batch、478455209984 bytes、未分類0・上限なしでpass。

続く初回起動では、旧finalized predecessorへ実行admissionを再適用する不備が露呈した。
履歴証拠だけの照合へ補正し、送信前拒否と未知送信の違いも統括手順・回帰testへ明記した。
関連124 tests/Ruff/docsと固定reviewを通し、同session/tabを正常再開。
同じ登録batchが2026-09-27T19:09:11Zにrunningとなり、build段階の実PIDとheartbeatを確認した。
当初の全体gateは未終了batchで保留したが、同batchのinvalid/finalized後に再実行し成功した。
base/head `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、基盤source
`32f714883297cd84ce7f5225c2e97af6b51c9ba11cbbc87cae1a088f2d021207`で
contracts/tooling 1,086件＋164件、Ruff/actionlint/perf self-testが成功。
storageは126 batch、479275806720 bytes、未分類0・上限なしでpass。
native失敗は製品のinteractive workload起動条件との不整合であり、passとはしていない。
終了通知は同じ統括へ届き、eventはapplied/correction_routedとなり、同Runの実装Aが修正を返却した。
この基盤補正は完了。製品修正の検証・固定review・実機再確認とTAK-14全体の受入は継続する。
