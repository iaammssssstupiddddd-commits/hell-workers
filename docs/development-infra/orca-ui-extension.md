# Orca本体の受付・役割UI拡張（開発中）

2026-09-24。ユーザー許可のもと拡張ビルドへ一時切替し、実画面からの受入を進めている。
全体の完了条件は[UIライフサイクル計画](../plans/orca-ui-lifecycle-plan-2026-09-23.md)に従う。

## 一時配備と復帰

本体sourceは `bb614da7`。配備先は `/home/satotakumi/.local/opt/orca-ide/ui-bb614da7`、
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
元依頼のreceiptを確認してから内部Linear課題と分離worktreeを一度だけ作成する。
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
