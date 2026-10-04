# Orca 依頼全体の進行・完了判定と連携の横断是正計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `orca-request-lifecycle-correction-plan-2026-09-26` |
| ステータス | In Progress — host差分統合、全体受入gateの候補実装中。全経路受入・運用切替は未完了 |
| 作成日 / 最終更新日 | 2026-09-26 / 2026-09-26 |
| 作成者 | Codex |
| 関連提案 | [Orca導入提案](../proposals/orca-parallel-development-proposal-2026-09-20.md) |
| 関連Issue/PR | TAK-14を発端とする基盤是正。新規課題・PRは未発行 |
| 上位計画 | [A〜D拡張計画](orca-abcd-workflow-expansion-plan-2026-09-25.md) |

## 1. 目的

直近の実装・調査loopの承認を、利用者が依頼した成果全体の完成と誤認しない構造に直す。
Linearへの関連付け、helper単体試験、特定loopの成功を「運用全体の完成」の根拠にしない。
受付から全体計画、配車、検証、固定review、後継工程、外部同期、終了・資源整理までを接続する。

成功指標は、複数工程の依頼が一度の実装指示で、許可範囲内の次工程へ継続し、
全受入条件の証拠が揃うまで全体完了を拒否すること。利用者による内部ID入力、
通常経路の保守command、単なる「続けて」の再入力、重複dispatch、誤ったDone更新は0件とする。
仕様・公開権限の不足や利用者の明示停止では適切に止まることも成功条件に含める。

## 2. スコープ

### 対象

- project-owned host controller全般、受付・追加依頼、計画/DAG/context、loop/固定review/統合、復旧。
- Linearの依頼・工程・完了条件とhost証拠の対応、GitHub/CIイベント、同期intentの生成から実行・再読まで。
- Orca本体の状態契約、既存受付/状態行/role navigation、終了処理、履歴、配備・更新互換。
- 既存案件の非破壊移行、稼働helperの統一、実運用を模した非ゲームE2E、運用文書。

### 非対象・今回の権限境界

- 2026-09-26の「実装してください」により基盤是正のコード変更へ着手した。TAK-14の製品実装再配車は含めない。
- 将来の是正実装にもTAK-14のM1-c以降のゲーム実装は含めない。製品差分を基盤試験のfixtureにしない。
- UIの全面再設計、結果cardのterminalへの重ね表示、新しい操作画面の乱立は行わない。
- 外部公開・製品branch merge・課題取消・原本削除は本計画で新たに許可されない。
- shared checkout編集、worker再委譲、検証/review省略、新Run/空commitによる停止迂回は認めない。

## 3. 現状とギャップ（実コードに基づく監査）

### 2026-09-26 停止復旧の追加範囲

#### 2026-09-27 native終了後の継続欠落（是正中）

- nativeが終了しても統括のturnは自動再開しない。既存Driverの自動継続はloop承認後だけであり、
  手動promptによる再開は恒久対応ではない。`correction_required`を完了・待機と誤解しない。
- 起動前に独立したpending native operationを永続登録する。batch digest、依頼scope/node、Run、
  loop世代、統合source、統括session/process出生、terminal incarnationを固定する。
- 既存host Driverがpending operationを観測する。実行exit 0と独立verifier passを区別し、
  verifierは同じ登録commandだけを実行する。invalid/interrupted/unknownを成功へ読み替えない。
- 配信はprepared/sending/accepted/started、後続工程成立はappliedとして別管理。
  応答消失時は同一operation IDの照会のみ。pause、所有変更、source変更時は送らない。
  新Run・新terminal・native recipeの再実行で回避しない。
- 統括もworker同様にCodexのapp-name titleを起動引数へ含める。実行中providerには反映されないため、
  稼働工程を保全した同session再開と実配信受入を別の完了条件とする。
- 検証: 各WAL境界の再起動/重複、失敗/中断、所有/source変更、user pause、
  acceptedだが未開始、開始したが未適用、正常終了から同terminalの次工程開始までを確認する。
  固定read-only reviewと変更別tooling/Help/storageの合格前に完了報告しない。
- 候補実装: event/loop待機CAS、workflow進行gate、same-head `resume-review`、全体受入gate、
  同session所有handoff、子receipt欠損の照合、既存状態行への投影を追加。
  native・bridge・文書補正・起動guardの73 tests、差戻し履歴の実Git回帰がpass。
  途中候補の変更別gateは942件＋164件pass（source `a7ba4e949b195e289d7d271cbc252d82510fdbf62250fbe400413bf2d6c346d9`）。
  後続reviewで先行event鎖、二度目の所有移転、監視開始順、書込み前gateを補正したため最終gateを再実行する。
  最終コードreviewはAPPROVED（native `eb24175d`、loop `d021bb36`、UI起動 `0ca96f00`、supervision `4c2a6d90`）。
  関連154 testsをreviewerも確認。native単体は追加回帰を含め40件pass。
  これはコード承認であり、配備と自動継続・再起動耐久・失敗時停止の実受入は未完了。
- 最終変更別`contracts, tooling`は950件＋164件、lint/perf self-test/Help/storage/docsがpass。
  base/HEAD `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、source
  `b2ec4c4eba89e7acb57e46b56b84e03c3309af3e66363a12e286e6e79c5e90a8`。
  Helpはhost側制御・診断だけでplayer-facing経路に変更なし（No impact）。
- 実反映: idle統括を`/exit`で正常終了し、旧owner/providerの消滅と同terminalのforeground shellを確認。
  Orcaの通常Close→固定launcher再起動で監視controllerを更新。runtimeは`8038038a-6ac1-4c5e-92ad-b1f17e2dce4d`。
  terminal `term_b9fb015e-4bb9-4f08-827f-06fe7ecaec67`とincarnationを維持したまま、
  session `01a0d377-feef-7520-bef5-50565ef282cb`を再開した。これは反映確認でありnative自動継続の実受入とは区別する。
- 同統括が修正済みrouteを実行し、非routing履歴を維持したまま同Run `run_3236488f0dff`の
  実装Aが`implementing`へ進んだ。統合工程は`correction_required`から`planned`へ移行。
  driver生存、同sessionのrollout FD、app-name付きtitle、既存terminalへの実行履歴追記を確認。
  自動native通知の新規eventはまだ発生しておらず、その実受入を成功扱いにはしない。
- 実統括の現在の別停止: sourceごとに凍結したharnessを使わない比較helperと、非routing履歴を
  routing扱いする差戻し経路。後者のKeyErrorと修正回数混同を基盤側で補正した。
  前者は既存実装Aの許可scope `scripts`内にあり、同じRunの正規差戻しで修正する。主担当は製品sourceを編集しない。

#### 2026-09-27 隔離統括からのnative起動経路

追加実受入で、native終了通知の観測に未解消の欠陥を検出した。
新batch `tak-14-m1c-foundation-current-cf08061d-primary-20260927`は独立passとなり、
event `c14602be-98db-5e6f-b712-d061a7e40fa8`の本文は同じ統括rolloutへ一度だけ到達した。
しかしOrcaのforeground検出は`--new-session`隔離配下のCodex（controlling ttyなし）を除外するため、
durable send receiptは`unsupported / input_accepted`であり、hostはblockedへ誤分類した。
title変更だけではこのprocess判定を修正できない。隔離解除・raw Enter追加・通知再送では回避しない。
通知開始の観測と、同じeventに束縛した正規後続操作の成立を分離し、原receiptを保全した照合経路を整備する。
本項の是正・固定review・変更別検証・同eventの回収が終わるまで、自動継続の実受入は未完了。

追加修正の契約（実受入待ち）:

- `unsupported / input_accepted`を受理未観測として保持し、同ID read-backを継続する。開始済みとは表示しない。
- 初回receiptを不変保存、最新read-backは別保存。外部write後のowner/control変化でもreceiptを失わない。
- 旧誤分類eventはexact successor成立後だけreconcileし、旧block/hash・正規replay receiptを保全する。
- 比較結果の文書補正でGit公開後・loop保存前の中断も発見。統括から未検証差分を引き継ぎ、
  基盤checkoutの追加編集を停止して単独所有に戻した。新Run、通知再送、native再実行は行わない。
- 統括補正はexact prepared journal・全loop投影・committed Git receipt・新sourceを照合する。
  source変更はこの証明付き遷移に限定し、fresh validationと固定reviewを要求する。
  Git公開済み／loop保存済みの各中断は既存証拠から投影を回収し、Gitを書き直さない。
- Help影響reviewはNo impact。開発用host制御からOrcaへの経路のみで、ゲーム入力・描画・Help catalogは不変。
- 最終コードreviewはAPPROVED。native `8140334e`、tooling correction `4e06fb87`、
  対応tests `e10bf239` / `0138cfca`。結合62件pass。復旧intentをhandoff前に保存し、
  応答喪失・最終保存直前の中断後も元hashと同request IDで回収する回帰を含む。
  直前候補の全体gateは966件＋164件passだが、intent追加後の最終sourceを改めて検証する。
- 最終source `0cdbd7e916f2745a0c8c5f661466967f0b1a3d68c54cd9c35bf0280d3a0dbece`、
  base/HEAD `ae5b2066c4fd39a8dd6a6c439e272d125f749241`の変更別contracts/toolingは966件＋164件pass。
  Ruff/actionlint/perf self-test/Help/storage/docsもpass。primary storageは445732134912 bytes、未分類0。
- 実回収: idle統括の正常終了を確認し、同terminal・同incarnation・同sessionを再開。
  新owner 3217703/provider 3217723のopen rollout FDで同sessionを確認した。
  prepared補正`60febf59…`はGit再公開なしでcompleteへ、event `c14602be…`は元hash
  `6cee54c7b0cc454a3c437d3a6f999fd8a13615af55ac814eaf0328eaf78fa5a8`から
  正規reconcileでappliedへ回収。元通知IDのreplayed受理receiptを残し、turn_startedはfalseのまま。
  新source `adec479fc5db035bc07dbbde36a319bc4cf96730`はfresh Help reviewを要求し、旧承認を流用しない。
  同統括へ製品のHelp確認・変更別再検証・固定reviewを返す通知を送ったが、後続調査で
  composer残存・モデル未受理と判明した。実際の作業再開とは扱わない（次節参照）。
  これは停止記録の実回収であり、TAK-14全体の完了や次回native通知の開始観測成功ではない。

#### 2026-09-27 隔離Codexへの送信確定（追加是正中）

idle統括への引継ぎ入力はdurableに受理されたが、rolloutへ届かずcomposerに残った。
`unsupported`を未観測として扱うだけでは、raw送信による貼付・Enter競合を解消しない。
Orca本体のsettled prompt入口に、Linux実行host専用の隔離Codex証明を追加する。

- 既存provider inventoryのexact PTY incarnation/rootProcessIdを再利用し、local/daemonを同経路で扱う。
  SSH/WSL、古いproviderの証拠欠落をlocal `/proc`で推測しない。
- fresh process snapshot、root tty、foreground ancestor、唯一のnative Codex子孫、
  fd 0/1/2の同PTYS device、PID出生と実行commandを束縛する。title/comm/rollout単独は証明にしない。
- 汎用foreground/stop/idleness判定は変更せず、貼付前・submit直前に同じ証明を再照合する。
- 既存のatomic bracketed paste・ingest待ち・permission/lifecycle gateを利用し、raw Enterで回避しない。
- provider/PTY差替、停止process、複数agent、別TTY、headless command、remote/証拠欠落の拒否をテストする。
  本体typecheck/lint/関連回帰、別build検査後に切替し、同terminalで実際の会話受理・開始を確認する。
- すでに受理された入力は再送しない。残存composerの処理は元receiptに束縛した正規復旧契約を別途確認する。

追加是正の検証経過:

- 本体 `f46b80ab`で隔離Codexのexact proof・各write fence・local inventory root PIDを実装。
  固定read-only review APPROVED、関連79 tests、周辺1,309 tests（1 skipped）、node typecheck、
  changed-code qualityがpass。ゲーム実装・アセット・ゲーム内Help経路に変更はなくNo impact。
- 別profileの梱包済runtimeでPython→bwrap新session→native probeを実行。
  bracketed pasteとEnterが1回、`codex / supported / turn_started`、同じrequest replayで再送なしを確認。
  これはtransport実機試験であり、TAK-14の実provider再開とは分ける。
- 同試験でdaemonのshell確認がidle shellにもfalseを返したため、既存bare-shell互換の受入は不合格。
  同inventoryからroot出生・前景自己所有・shell executable・同端末FD・子processなしを二重照合する
  補助証明を追加。新しいsourceの43 focused testsがpass。再梱包・固定review・実shell受入を継続中。
- native依存は変更していない。hostでの再compileはglibc 2.42要求により梱包gateで拒否。
  同一C/C++ source/binding.gypの既設互換binaryを再利用し、正式なprepared-runtime経路で再梱包した。
  glibc 2.31/native 17件、daemon entry、plugin資源検査がpass。gateは無効化していない。
- 旧unsupported入力のreceiptはreplay-onlyで、本文の再送や自動Enterを認めない。
  現在の表示draft全文を確認済みだが、旧receiptからcomposerの完全一致を自動証明するAPIはない。
  残存入力1件の明示送信について利用者へ確認中。通常の次回送信は新しいguarded経路を使う。
- 最終候補 `6c38bc463d0b6fd95ed6a5516901713775456fb8`を固定reviewでAPPROVED。
  shellはroot PID=process group=foregroundと他group member不在も要求し、広いboolean fallbackを撤去。
  最終focused46 tests、周辺1,351 tests（1 skipped）、node typecheck/changed-code qualityがpass。
  最終梱包の別runtimeでもshell command成功・隔離probeのFRAME=PASS/SUBMIT=1・turn_startedを確認。
- `ui-6c38bc46`へ保全切替し、runtime `d60fc502-ba3c-4151-af16-b57b67a08a5f`とPID3388130の
  exeを照合。元の統括/レビュー等3 terminalのhandle・incarnationは同一。新Run/製品変更はなし。
  rolloutの最終完了は04:45:33Zのままで、新しい製品作業は開始していない。
  既存draftの明示送信は確認待ち。開始未観測を再開/完了と表示しない。
- 試験用profile・native probe・束縛証明の一時出力は全process終了/開放を確認して撤去。
  `/tmp/orca-isolated-prompt-acceptance.PPk8LN`と`/tmp/orca-prompt-proof.c1CwqK`の
  実測8,257,536 allocated bytesを削除した。実案件の履歴・保存台帳・旧build/設定snapshotは保持。

- 実レビュー差戻し後のnative batchは登録済みだが未実行。統括sandboxには表示socket/GPUがなく、
  PID namespaceとprivate tmpもhostのprocess照合・native lockに適合しない。
- 統括sandboxと担当隔離は変更しない。host launcher所有の限定bridgeへ登録batch IDとexact digestだけを渡す。
  peerのUID・統括process出生/子孫・現在の受付所有を照合し、他担当・別依頼・任意commandは拒否する。
- primary台帳の対象source/command/保持/admissionと同じGit common directoryを照合し、
  既存kitty→primary validation execute→重実行guardの経路を使う。未知結果のlauncherは再送しない。
- receiptは「起動要求受理」と「batch実行」を分離。応答消失・二重送信・source変更・host環境欠損を試験する。
  基盤のtooling/固定read-only review/Help/storage確認後、同じ統括へ返し、製品検証は統括自身が実行する。
- 既存batch `tak-14-m1c-foundation-baseline-20260927`と比較checkoutは保持。
  主担当がゲームtestを代理実行せず、基盤修正の完了をTAK-14完了と混同しない。
- 限定recipe bridge、primary controllerのimport前pin、同一heavy leaseのFD継承、
  子所有のverifier終了receipt、既知失敗/中断確定、資源busyだけのexact試行再開を実装。
  native関連27 tests（実OS lock継承/競合を含む）と統括関連を合わせた57 testsがpass。
- 固定read-only review APPROVED。bridge SHA `7d25742481cfa6882e8a14a37fa29b9a0b36b814991a7b8d1dc08e6d8d77d56f`、
  UI coordinator SHA `8aaf37c4cd918fee9464cedc3af43245a38741aa81da63505ca84d40f47cca82`。
- 同じ統括processへbridgeを追設し、実統括からinspect成功・登録digest一致を確認。
  主担当など統括の子孫でないprocessからのinspectは正しく拒否した。
  この時点ではlaunch/execute/sealは未実施。最終toolingを待って統括へ実行を戻す。
  storage checkは当該registered batchの未終了を正しく拒否しており、完了扱いに変更しない。
- 最終host toolingは909件＋Blender補助164件、Ruff/actionlint/perf self-testがpass。
  base/HEAD `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、前後同一source
  `8200b7a518d226812446940f8b03df1aafa00dbb679a278a954db4a56aaf53c1`。
  Helpはhost専用producer/consumerの変更でNo impact。全変更別gateは当該batchの終了待ちでありpassとはしない。
  同じ統括へ既存batchの正規実行を返した。送信受理は確認したが、それだけを実行開始の証拠にしない。

#### 2026-09-27 native証拠ログの環境継承補正

- 同一統括で登録batchの実起動を確認。初回build後Capture smallの3反復は完走したが、
  起動元の`RUST_LOG=warn`継承でINFOのAdapterInfoが欠落し、独立判定はinvalidとなった。
  window証拠だけでpassに書き換えず、既知exit 1→invalid seal→診断用途保持→finalizeしstorageはpass。
- host bridgeの検証子だけを固定log filterへ正規化し、親環境・凍結subject・verifierを変更しない。
  unset/warn/off/traceの実子process回帰を追加し、native＋統括58 testsがpass。
  固定review・全変更別検証後、同じ依頼/Run/統括で新しい検証batchを登録する。
  旧batchの再送・既存失敗の上書きは行わず、同じCargo cacheを再利用する。
- Help判断: 検証hostからdiagnosticログへの経路だけを補正し、player-facing操作・結果・表示は不変（No impact）。
  本補正の実受入・TAK-14全体完了は未確認。
- 固定read-only review APPROVED。bridge SHA `337232c06ce33ec2cb9a171d43571da3f7eadfa960a8d3b1f7cac517b7f0aeee`、
  test SHA `b4dffbc3e03b15da8dbc7057b64856b43c8e34deae9ba42fdd09fd2528cbe8c5`。
- 変更別contracts/toolingは910件＋Blender補助164件、lint/perf self-test/storage/docsがpass。
  base/HEAD `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、source
  `54822b5a1e8b1a7ca6fd9ff5e1a79d9441fd7d81ad96ef7f25d926e31110d7d9`。
  native子の終了と旧batchのfinalizedを確認後、追設brokerのみ更新し、統括のprocess/sessionを維持した。
- 同じ統括から`tak-14-m1c-foundation-baseline-rustlog-20260927`を正規登録・起動。
  registration digest `cd17afab4c61e17197d286fae4169c54fc7736222b957d2fe8b9c18c639db2cd`、
  output `building-art-static-20260927T005736Z-545595a9`のrunningと同一sourceを照合。
  run-001の実ログでIntel Arc / VulkanのAdapterInfo復帰を確認した。
  後にbaselineと`tak-14-m1c-foundation-current-20260927`双方の独立pass・finalizeを確認。
  比較/固定review用の証拠を保持している。これはnative自動継続の実受入や依頼全体の完了ではない。

#### 2026-09-27 未取得回答が残るreview完了拒否

- 質問timeout後にask resumeではなくcheckを続けたreviewerがworker_doneを出し、
  未取得回答のpreflight拒否だけでbridge全体が失効した。監視待機では復旧しないため主担当が是正する。
- 上流送信前のpending question/deliveryは、完了を受理せず必要な通信取得へ戻すrecoverable応答にする。
  所有不一致・不正payload・送信済み不明は失効を維持する。
- 失効済みreviewerは最終turn・正のprocess終了・同source・Orcaのfailed fencingを照合し、
  元Task/Run/sessionで固定reviewをやり直す正式復旧を追加する。旧review結果を代理送信・承認しない。
- 未完の質問を含むconfirmed通信だけを認め、pending mutation・settlementを拒否する。
  exact receipt保存と再照合、固定review、tooling/Help/storage、実review再開まで確認する。
- 正式復旧receipt `845be75cfa7d4a9caa78919eac93b84fa4166a0ca0046b7c4c0a73e51265df87`を適用。
  旧Task/Run/session・統合HEADを維持したが、再起動したreviewerへのworker-startが
  `agent_unconfigured`で配車前拒否された。tui-idleの正の判定と本体のagent-presence判定の不一致を追加是正する。
  この段階は再開完了ではない。未arm bootstrapを再生成せず、same request replayと
  received/arm後のlocal projection回復を備える正式経路を検証する。
- 追加調査でnamed CodexのOSC titleからアプリ識別子が消える経路を特定。本体guardは緩和せず、
  supervised通常起動/resumeでCodex自身にapp-name/run-state/activityを通知させる。
  同じ稼働reviewerへ公式`/title`で適用し、同session/端末のagentStatus=true/idleを実測した。
  拒否された再送のWALは保全し、確認済みpreflight拒否と未知送信を区別する復旧を固定reviewする。
- 固定review承認後に同じbootstrapへ正式復旧を適用し、`ctx_e4563220d15c`の実配車・結果受領・
  provider正常終了を確認した。未検証M1-c受入証拠のchanges_requestedを統括へ戻し、
  `active / correction_required`まで進んだ。TAK-14完了ではなく、不足証拠の是正が次工程。
- 追加監査で発見したRPC相関IDと耐久mutation IDの混同を修正した。欠損時の代用は禁止し、
  異なる2つのIDとnull/欠損ケースの回帰試験・固定reviewを実施した。

#### 2026-09-27 工程承認後のscope未結合停止

- M1-c統合HEAD `e4b79a8a0a69901f0679d9f4f74df444ade5e614`は検証・固定review承認済みだが、
  進行中loopに後から登録した全体契約のbindingがなく、driverが終了した。
- 承認済み・通知drain済みの未結合loopだけを対象に、可視統括、exact digest、現在のscope、
  同一source/evidence、全attempt終了を照合する正式移行を追加する。
- 旧loopと封印reviewは保存し、同じRun/統合HEADでscope付き固定reviewを新規に行う。
  bindingの後付けだけで承認を継承せず、未確認の依存・全体受入への直接移行を拒否する。
- 未結合は既知の照合待ちとして表示し、driverを例外終了させない。復旧後は同じ統括sessionへ戻す。
- 拒否経路、応答消失、同一要求の再照合、変更別tooling、固定read-only reviewと実復帰を確認する。
- 上記を実装し固定read-only review APPROVED。関連64 tests、最終contracts/toolingは860＋164 tests、
  lint/perf self-test/docs/storageまでpass。base/HEAD `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、
  source `a107e212f7cfb9559e59d82e08110fbdb309ba92a7d4458e5b2f0341a017ecb3`。
  review指摘修正中の先行gateはsource変更で拒否され、不採用。Helpは開発基盤のみでNo impact。
- 統括を同session・同terminalで正常再起動し、旧driver終了と新driverのheartbeatを確認。
  全attempt accounted・通知drain・現Linear接続・封印review・統合source不変を照合してguard付き移行を適用。
  generation 4、Run `run_3236488f0dff`、統合HEADは不変。旧loopはscope-migration receiptへ保全。
  Task `task_08c0cfb1699c`／Dispatch `ctx_be0e7904075e`で同じ固定reviewer sessionが実際に仕様確認を開始。
  snapshotはreview/review、統括も監督を再開した。M1-cのscope付き承認・次工程完走・TAK-14全体完了は未確認。
- tab整理の別停止は、launcher終了済みshellにTUI idleを要求したため。正のforeground shell確認と
  close直前の再照合を追加し、shell未確認ならTUI idleへ戻す。今回実tabの削除は行っていない。

#### 2026-09-27 統合前の文書差戻し補正（実復帰確認済み）

- 固定reviewの文書指摘を編集権限のないworkerへ再配車しない。統括所有の補正へ振り分ける。
- 同一Run・source・封印review・全完了ACK・担当終了を照合し、指摘されたMarkdownだけを補正する。
  workerのallowed directoriesを広げず、原依頼・実装成果・失敗した試行を保持する。
- 補正前にjournalを保存し、clean HEADをCAS更新。補正後の検証と固定reviewを必須にし、旧承認は継承しない。
  中断・source競合・未確認終了を自動再実行しない。
- successful review後と再試行時に古いfailed/failure_reasonを消し、今回の失敗理由を表示する。
- 実Git回帰試験、変更別tooling gate、Help No impact、storage確認後に既存TAK-14で再開を確認する。
- 文書補正／復旧18 tests、review-loop 63 tests、checkpoint 20 testsと固定read-only reviewは成功。
  復旧済み旧attemptを完了ACKなしでも正にaccountedとする既存receipt経路を補正した。
  saved/current journal、authority、revocation、settlementなしを照合し、ACKは捏造しない。
- 監視automationの旧「修正禁止・報告のみ」を、利用者の追加指示に従い「許可済み範囲の恒久修正・検証・
  同じ依頼の再開確認」へ更新。意図的停止、権限不足、不明な副作用、外部公開は自動で越えない。
- 最終host変更別contracts/toolingは849＋164 tests、lint/perf self-test/docs/storageまで成功。
  base/HEAD `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、source
  `32a9a3d0ad48e1431fd0998ec4db93f5edee3bb9e79e57d03155cfb203692163`。
  途中source変更のあった先行検証は最終gate証拠として採用していない。
- 既存統括を正常終了し同session・同terminalで再開、修正版driverの実行を確認した。
  guard付き補正でcandidate `6aaac0893691cfd868d2f47f62fbc94e80c2060a`を作成。
  差分は`docs/building-asset-sets.md`だけ、元14ファイルの実装は不変、候補checkoutはclean。
  旧headからの文書変更別検証に成功し、同じRun `run_3236488f0dff`で固定reviewerが実際に再レビューを開始した。
  Task `task_7d9ed87cd830`／Dispatch `ctx_780e23147433`、lane=reviewing、failed/failure_reason解除を確認。
  これは停止からの復帰であり、M1-cの最終承認・TAK-14全体の完成はまだ主張しない。

#### 2026-09-27 完了受理後の終了確認停止

- 実装Aのworker_doneはconfirmed/completedだが、launcherがidle観測を1秒で繰り返し、Orca本体の2秒pollへ到達できなかった。
- 観測を有界10秒へ修正。所有・正式receipt・idle・終了後source検査は維持し、timeoutを終了と見なさない。
- 2秒poll/3秒quiet windowより短い待機を拒否する回帰試験、変更別gate、固定read-only reviewを実施する。
- 既存担当は正常終了させ、同じ14ファイルとTask/Dispatch/terminalを保持して検証へ接続する。
  再配車や新Run、台帳の直接書換えは行わない。TAK-14全体の完成とは区別する。
- 実runtimeで1秒観測のtimeout、10秒観測のidle成立を確認。旧launcherの担当を正常終了したところ
  `recorded/process_exited=true/exit_code=0`から既存driverが自動でvalidatingへ進んだ。
  次のfresh Help review gateは統括が実diffを確認しており、本基盤修正で製品の承認を代行しない。
- 終了確認関連20 testsと固定read-only reviewはpass。対象2ファイルdiff SHA-256は
  `17af7c86d5ebc35430fb0719f07ad1494f1e6d700168e670bcf8d427ca478fb1`。
  変更別contracts/tooling gateは841 scripts tests＋164 Blender補助tests、lint/docs/storageまでpass。
  base/HEAD `ae5b2066c4fd39a8dd6a6c439e272d125f749241`、source
  `a73591fe0e1e16ee22984b71c46270af925a2dd7604bfc10037d7e3aa1a73f9c`。
  今回の変更は開発用launcherだけでHelp影響はNo impact。製品実装のgate結果とは区別する。

#### 回答済み質問の再照会による停止（追加是正）

- 回答済みaskの同一質問resumeは、同じDispatchの所有・sourceを再確認し、確定済み回答を返す。新たな質問や外部mutationを発行しない。
- 別の未回答質問が存在する場合、未知質問、能力・所有変更、結果不明の操作は従来どおり拒否する。
- revoked/unknownなbridgeを、provider終了待ち・host slot取得より先に検知し、loopをpausedにする。UIも「開始待ち」へ逆戻りさせず通信停止を示す。
- 回帰試験、変更別gate、固定レビュー後に既存TAK-14の同じRun/Task/sessionを照合して復旧する。旧bridgeの権限を直接書き換えて再有効化しない。
- 完了条件はコード修正だけでなく、実際の担当再開と台帳・画面の一致を確認すること。

利用者は、GitHub連携でDoneになったTAK-14をIn Progressへ戻し、同じ依頼を再開することを明示許可した。
上記の当初非対象に対する追加許可として、既存M1-c generation 4の配車前停止を復旧する。
新規課題・Run・重複worktreeは作らず、旧世代の封印と成果を維持する。

1. exact loop digest、可視統括、配車未実施、同一source、最新Linear activeを確認する`resume-linear-state`を追加。
   書込み前に前後状態のreceiptを保存し、応答消失後の同一要求は配車を増やさず再照合する。
2. 依頼PRの生成/送信も全体受入と同一headへ拘束する。部分loop承認で親課題に紐づくPRを出さない。
   人手で別経路から作成するPRやLinearの組織設定まで制御済みとは扱わない。
3. 既存の停止loopと新設M1-c worktreeを保持台帳と照合し、変更別検証・固定read-only review後に復旧する。
4. 稼働中の旧driverと配備hostの差は残存リスクとして分離し、単なるapp再起動で更新済みと判断しない。
   全体移行/版固定が完了するまで、今回の復旧成功を全体基盤完成とは扱わない。

調査対象は配備host `orca-abcd-expansion`、TAK-14で実行中のhost `orca-parallel-development`、
Orca本体clone `orca-ui-lifecycle`。下表のpathは各checkout内の相対pathである。
「確認済み」はコード/呼出経路の事実であり、全故障を実環境で再現したという意味ではない。

| ID / 優先度 | 確認した実装と問題 | 是正対象 |
| --- | --- | --- |
| F01 / P0 | `orca_supervision.py:supervised_view`はloop approvedから直接feedbackへ進む。`confirmed_result`は直近integrationだけを要約する | 全体受入判定と工程結果を分離 |
| F02 / P0 | `orca_work_planner.py`はDAGを検査するが`build_loop`は一waveだけを登録し、planning自体が省略可能。依頼全体の必須成果との対応がない | durableな全体計画・受入条件・node対応 |
| F03 / P0 | `orca_ui_coordinator.py:initial_prompt`の「未完ならsuccessor」は文章の指示。hostに全体残件からの強制継続判定がない | host-owned次工程判定と再開journal |
| F04 / P0 | `task_actions`/`preflight_close`は最新loop承認・process/通知/cleanを調べるが、全体残件を調べない。`finish_close`はworkspaceをcompletedへ動かす | 非表示/中断/取消/依頼完了/撤去を分離 |
| F05 / P0 | `queue_linear_status/comment`の通常host側呼出を確認できず、`execute_next`もCLI入口とtestから呼ばれる。通常`tick_under_lock`に同期実行がない | 状態commit→outbox→executorの本番接続 |
| F06 / P0 | `external_sync.projection`はoperationsが空ならnot_required。連携必要性・接続状態は別に検査していない | linked案件での未接続・未観測を隠さない |
| F07 / P0 | `active_issue`はLinear credential取得失敗時にTrue。実読取も`linear_credential_unavailable`で失敗した | degraded実行方針と全体完了禁止。復号失敗の根本原因は未確定 |
| F08 / P1 | `queue_linear_status`はstarted→review→completedの一方向順位。複数工程のreview後に次実装へ戻る表現と衝突する | issueの全体状態と工程状態、世代/sequenceで同期 |
| F09 / P1 | `follow_up_receipt`はsend acceptedを保存するが、開始証拠/依頼全体の仕様改訂へ結びつけない。非approved loopでは追加依頼actionを出さない | 受理/開始/適用を分け、稼働中の変更指示を保全 |
| F10 / P1 | `context_package`生成時のspecificationが空配列。reviewはSHAとticketへ強く拘束されるが全体条件の網羅を保証しない | scope版と受入条件IDを全担当・reviewへ伝播 |
| F11 / P1 | `case_contract.stage_for`は日本語detail部分文字列から工程を推測。feedbackの次操作は一律「必要なら追加依頼」 | 型付き状態と具体的な次工程/待ち理由 |
| F12 / P1 | `orca_reconciler.py`の診断/repair helperは存在するが、検索した通常controllerからの呼出を確認できない | 診断・修復の実接続と限定能力境界の実証 |
| F13 / P1 | launcherはabcd helper、既存統括はparallel helper。本体/host/台帳の版が独立。promptに「統括だけを残す」という現実と不一致の記述もある | release manifest・互換・prompt/文書の実装追従 |
| F14 / P1 | testは各helperと単一loopの成功を強く確認する一方、全体残件がある成功loop後の正常自動継続を受入していない | 利用者起点の複数工程E2Eを必須化 |

Linear側が実際にDoneへ変わったかは現在の認証エラーにより未確認。誤完了の説明・表示と、外部Done更新を混同しない。
今回の読取調査だけでは、credential復号失敗の原因、GitHub標準連携の現在設定、すべての旧案件の所有を確定できない。
これらを実装前のM0検査対象とし、推測で設定や台帳を修復しない。

### 3.1 活かす実装

- ticketed mount、別worktree、A=Codex/B=単純leafのCursor、固定read-only reviewer、host資源guard。
- base/head/source/evidence拘束、Help review、直列統合、no-change/reconciled-target receipt。
- successor lineage、dispatchの重複抑止、正の終了証拠・ACK照合、同session復帰、終了terminal保持。
- intakeの4択判断、外部writeのidempotency/read-back、未知結果の停止、UIのstale検出・非表示操作。

上記は撤去して作り直さず、全体制御への入力/実行先として再利用する。単体成立と実運用接続済みを区別する。

## 4. 実装方針

### 4.1 正本と階層

```text
利用者の依頼 → 全体契約（scope revision・必須成果・受入条件）↔ Linearの案件/工程
                   ↓
             工程DAG（複数世代にまたがる）
                   ↓
       個別loop → A/B → 検証 → 固定review → 統合後review
                   ↓
       全体進捗を再評価 → 次工程／判断待ち／全体受入review
                   ↓
       完了receipt → 必須外部同期の確認 → 依頼完了
```

- 利用者指示と採用済み仕様が目的・権限の正本。Linearは人が確認する依頼/工程/進捗の管理先。
- hostはLinear参照のID・版・取得時刻と採用した仕様を保存し、実行・検証・承認の証拠を所有する。
  Linearを単なる題名リンクにしない一方、本文/コメントを無審査の命令や承認証拠として扱わない。
- MarkdownのcheckboxやLLMの「完了しました」を直接実行状態にしない。両者の差を統括が分類して版付き契約へ採用する。
- 同じscope/権限内の工程分割・継続は統括が実施する。利用者に毎回承認・課題作成・ID入力を要求しない。
  必須成果の削除/縮小、目的変更、公開・破壊権限拡大は変更根拠を保存し、必要な利用者判断を受ける。
- Linear課題の発行は引き続き統括の4択判断。工程ごとの新規課題発行は必須にしない。
  子課題に分けた場合は必須依存を記録し、子課題Doneだけで親完了にしない。

### 4.2 永続データと完了gate

`request_contract`（新規host modelの仮称）へrequest/issue identity、利用者指示参照、scope revision/digest、
目的・非対象、必須成果/受入条件のstable ID、工程DAG、要求する証拠種別、公開方針、最終人手受入の要否を保存する。
nodeはkind（調査/実装/検証等）、対象条件ID、依存、実行状態、loop世代、成果/証拠参照、次操作を持つ。
原本本文の大量コピーや認証情報は含めない。旧checkpoint/loopの封印を後から書き換えない。

全体完了は以下すべてを要求する。

1. 最新scopeの必須条件が、対象と検証範囲の一致する証拠で覆われる。必須残件・未処理の追加指示が0。
2. 必須工程/子課題の成果が採用され、各loopの承認だけでなく、最終対象に対する全体受入reviewが承認済み。
3. 最新HEAD/source、scope/plan revisionと承認が一致し、必要なHelp/native/配布条件が満たされる。
4. 未確定process/dispatch/外部writeがなく、必要な利用者受入が明示的に成立している。
5. 証拠へのdigestを含む`request_completion`を発行し、必須のLinear等への同期をread-backする。

検証完了と外部反映を循環依存にしない。全体受入→完了receipt準備→Done送信→再読→同期済み完了とする。
外部未反映なら「成果受入済み・Linear同期待ち」とし、工程承認と全体完了を混ぜない。
取消/打切り/明示保留は成功ではない。必須条件の勝手なwaiveや別課題への移送で完了gateを通さない。
過去の証拠は履歴として残し、後続差分や仕様改訂が影響する条件を再検証する。無関係な証拠は関係を示して再利用できる。

### 4.3 継続・復旧と受信

- host reducerがloop承認を工程結果へ反映し、実行可能な次nodeを選ぶ。技術的な継続をLLMの記憶に任せない。
- LLMが次の分割/検証内容を決める必要があれば、永続のplanning待ちを作り、同じ統括へ一度だけ配送する。
- operation keyはrequest、scope/plan revision、node、generation、event IDに拘束し、期待前状態を照合する。
  状態commitとoutbox記録を原子的にするか、write-ahead journalと再照合で同等の回復性を持たせる。
- 受理/turn開始/計画への適用を分離し、送信済み不明は同IDで照合する。追加指示を受けても実行中ticketを上書きしない。
- 再起動、context圧縮、session再接続時は全体目的・残件・現工程・次操作を契約から再生成する。
- 完了報告の入力は全体判定receiptと残件一覧から生成する。loopだけの承認時は必ず「今回の工程結果」と
  「依頼全体の未完条件・次工程」を渡し、ターミナルの自然文を逆に完了証拠として取り込まない。
- no-change調査は対応する調査nodeだけを閉じる。成果が欠ける実装nodeのno-changeは全体条件の充足と認めない。
- busyは資源待ちとし、新規Runで回避しない。既知の復旧のみ自動化し、未知結果は対象・保全・次操作を伴う停止にする。

### 4.4 Linear/GitHub同期と接続障害

- 本番状態遷移からoutboxを生成し、単一ownerのexecutorへ接続する。空queueは接続正常や同期不要の証拠にならない。
- issueの全体状態は未完工程がある間startedを維持し、工程ごとのreview/完了は工程記録へ反映する。
  全体review後の差戻し/reopenは新しい遷移として扱い、単純なstatus順位ではなく世代/sequenceで古いeventを拒否する。
- Linear本文/子課題/状態の変更を取得して差分を提案化する。取消を観測したら新規dispatchを止め、明示再開なしに上書きしない。
- credential失敗を「activeである」と断定しない。cached identity/scopeと最後の観測を明示するdegraded状態へ分離する。
  安全に確定できる実行中loopの収束・証拠保存は維持する。新しい後継dispatchは既存の明示offline許可とscope拘束がある場合だけ許す。
  外部の現状態を確認できないまま全体完了/新規発行/公開は行わない。人の再認証が不可避ならその操作だけを一度案内する。
- credentialの復号失敗は本体の暗号化backend、profile、desktop launcher/update前後のidentityを診断し、平文保存等で迂回しない。
- PRは工程成果か依頼全体かを明示する。部分PRのmerge・CI successを親依頼のDone根拠にしない。
  GitHub標準連携との二重更新を避け、完了権限のownerを一つに定める。現行設定はM0で読取確認する。

### 4.5 表示・終了・配備

- 依頼の進行状態、個別loop状態、process状態、外部同期状態、作業場の開閉状態を別フィールドにする。
  文言からstageを推測せず、既存schemaをversioned更新する。
- 既存の消せる状態行に「依頼継続中・調査承認済み・次:M1-c」等を示す。全画面結果UIを復活させない。
  次の自動処理があるのに一律で利用者の追加依頼待ちにしない。
- 完了に至った実terminal履歴を保持し、工程名/世代を区別する。通常retryは同role tabを再利用し、focusを毎tick奪わない。
- 「閉じる/非表示」は成功判定ではない。中断/取消/完了と別操作にし、未完案件の表示整理でLinear Doneや成果撤去を行わない。
- consumer/process/成果保全を照合したstorage cleanupは別段階。進捗履歴を読むためだけに巨大cacheを保持しない。
- host二系統の差分を棚卸しし、採用版と互換adapterを一つのrelease manifestへ固定する。
  本体build、host SHA、schema、launcher/CLI、移行能力を起動時に照合し、旧promptや片側のみの修正を防止する。
  上流Orca更新では互換試験前に制御機能を黙って置換しない。

## 5. マイルストーン（実装順・担当境界）

統括が契約/文書/検証/commit/統合を所有し、実装はA、単純な独立leafだけB、承認は既存固定reviewerとする。
本計画策定ではworkerを起動しない。実装時も各batchは同一対象の変更別検証と固定reviewを通す。

| 段階 | 内容・主な変更先 | 完了条件 |
| --- | --- | --- |
| M0 / P0 | 全入口の到達性監査、host差分、配備manifest、台帳schema、Linear接続/GitHub設定を読取。`orca_supervision.py`、本体`src/main/linear/`、launcher | producer→consumer→副作用→testの対応表を確定。未接続を「実装済み」と分類しない。稼働案件/保全先/採用helperを特定 |
| M1 / P0 | `orca_request_contract.py`・`orca_request_state.py`（新規候補）、既存`orca_case_contract.py`と完了/close/sync入口 | 全体契約・状態分離・完了receiptを実装。契約不明な旧案件では全体完了/Done/成果撤去を拒否し、安全な現loop収束は可能 |
| M2 / P0 | `orca_work_planner.py`、`orca_review_loop.py`、`orca_ui_coordinator.py`、`orca_dispatch.py` | 全体DAG→一wave ticket→承認→次nodeを接続。planning省略でgateを迂回不可。未完残件がある成功loop後、同じ依頼が継続 |
| M3 / P0 | context package、固定review契約、追加依頼/再計画処理 | 条件IDとscope版を拘束。全体受入reviewを実装。scope変更で必要な承認を失効し、調査成功だけの全体承認を拒否 |
| M4 / P0 | `orca_external_sync.py`、`orca_issue_context.py`、controller、必要なら本体credential/error経路 | lifecycle outbox/executor/再読を本番接続。認証異常を表示。部分完了でDone不可。offline→復帰で重複なく収束 |
| M5 / P1 | `orca_reconciler.py`、close/finalize/storage adapter | 安全修復を実接続。閉じる/取消/完了/撤去を区別。各journal段階で中断復旧し、他owner/未完成果/承認履歴を保全 |
| M6 / P1 | 本体`src/shared/supervision-contract.ts`、`src/main/supervision/`、`src/renderer/src/components/supervision/`、i18n、既存prompt/運用文書 | 新状態を既存UIへ反映。消せる状態行、実terminal履歴、同role再利用、追加依頼入口、表示/実状態一致を受入 |
| M7 / 最終gate | 両hostの共通修正収束、隔離本体build、旧案件migration、全経路E2E、配備 | 複数工程の一依頼を再促進なしで完走。故障matrix成功。TAK-14は未完の全体目的を復元し、既存成果/履歴/HEADを維持 |

依存はM0→M1→M2→M3、M4/M5/M6はM1の契約確定後に進め、M7は全段階完了後。
初回の安全化はM1の完了禁止と同じ契約の最小状態表示を同batchに含める。UIだけ先に完了表現を変更しない。
M7以前の部分配備は候補/opt-inに限定し、「運用可能」とは報告しない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| 全体台帳がLinear/Markdownと別の手入力正本になる | 入力出典・採用revision・同期receiptを拘束し、不一致を明示。正本の責務を分ける |
| 全体計画の条件自体が欠落する | 利用者原文と成果/条件のcoverageを計画reviewで確認。明示的な縮小なしに条件を消せない |
| host制御を増やして永久停止する | 同scopeの自動継続、型付きwait、再照合と限定復旧を正常経路に含める。単なるprompt追記で済ませない |
| 外部障害で既存成果を失う/最新取消を見逃す | 収束処理と新規配車を分離。cached状態/接続時刻を表示し、復帰時に取消/仕様差分を先に照合 |
| 部分PRのmergeで標準連携が親をDoneにする | 完了ownerと部分成果の紐づけを設計・試験。権限なしに連携設定を変更しない |
| migrationが旧approvedを全体completedへ誤変換 | 全件read-only preview。未対応はneeds_reconciliation、工程承認だけ引継ぐ |
| 2系統helperの片側だけ修正される | manifestと共通contract試験、既存callerを互換adapterへ収束。稼働中の凍結workerへコピーしない |
| UI修正が履歴を隠す/タブを増殖させる | terminal主表示を維持し、実履歴/非表示/再表示/同role再利用をE2Eで検査 |

## 7. 検証計画

### 7.1 必須シナリオ

| ID | シナリオ | 合格条件 |
| --- | --- | --- |
| T01 | 3工程、途中にno-change調査、最後に成果受入 | 工程1/調査の承認では依頼未完。次工程が同じ依頼で自動開始し、最後だけ全体完了 |
| T02 | 全体受入に必要な条件を一つ未実装、CI/局所reviewは成功 | 全体review/完了/Done/成果撤去を拒否し、具体的残件を提示 |
| T03 | 全体review差戻し、後続差分による既存条件の退行 | 必要条件を同じ固定reviewerで再検証。旧承認の使い回しなし |
| T04 | 途中で追加指示、scope改訂、取消、同文再送、context再開 | 指示を一度受理して適用版を記録。中断/縮小は明示。元の目的/残件を忘れない |
| T05 | A/B独立並列、共有契約競合、B不適格、資源busy | 最大2枠/固定reviewerを守り、直列化/待機。全体条件のcoverageを維持 |
| T06 | Linear切断/credential failure、queue未生成、復帰時の手動取消・本文変更 | linked案件をnot_required/正常と偽らず、offline規則と再照合で安全に収束 |
| T07 | 外部write送信直前/応答消失/再読前に中断、重複・順不同event | 同operationで照合。重複課題/コメント/PR/Doneなし。古い世代で上書き不可 |
| T08 | 部分PR merge/別SHA CI/skip/取消/親子課題/最終PR | 親全体条件を満たすまでDoneにせず、許可とtested/review subjectを検査 |
| T09 | loop承認→次工程登録の各境界、controller/app停止、未知process | 次工程0件/二重作成を検出し、同じjournalから復帰。unknownを成功にしない |
| T10 | 非表示・中断・取消・全体完了・撤去、部分closeで停止 | 開閉は成果状態を変えない。完了承認履歴と所有/consumerを保全 |
| T11 | schema旧版、片helper旧版、launcher/CLI不一致、Orca更新 | 起動前に不一致検出。readonly履歴を維持し、誤ったmutationを拒否 |
| T12 | Orca UIから自然文を一度投入、実providerで複数工程 | 内部ID/command/「続けて」要求0、状態更新一致、状態行を隠せる、実完了terminalを確認できる |

単体testだけで完了としない。reducer/adapter test→実controllerとfixture providerの統合試験→
隔離Orca profileのUI試験→許可済み試験専用Linear/GitHubで実adapter試験→非ゲーム実provider複数工程を順に実施する。
通常入口からoutbox/復旧/次工程まで到達した証拠を取り、testからhelperを直接呼ぶ成功だけを採用しない。
要件ID→test→結果/receipt→採否の表を一つ作り、未試験を明示する。時間基準はM0で実測し、期限到達を完了扱いしない。

### 7.2 検証・保全の契約

- host: 同一base/head/dirtyの`python3 scripts/dev.py ci check --base <full-SHA> --mode auto`、Ruff/Python test。
- 本体: TypeScript型検査、契約/UI test、lint、Linux package、隔離実画面の入口/履歴/操作受入。
- 本計画の基盤試験にゲーム実装test/build/nativeゲーム起動は含めない。ゲームの通常品質規則は変更しない。
- Help実レビュー: 今回は計画/索引のみでNo impact。実装batchでは実差分を再判定する。
- storageは[保存領域ワークフロー](../development-infra/validation-storage-workflow.md)を毎開始時に読み、primary coordinatorで登録・測定・checkする。
  今回は新規worktree/job/binaryなし、削除なし。将来batchはowner/全consumer/subject/bytes/next_action/release_whenを先に記録する。
- 反復中は同じcandidate/cacheを維持する。終了後は証拠要約を恒久仕様へ、不要なjob/profileを照合して整理し、削除path/実測差を報告する。

## 8. ロールバック・移行方針

- 導入前に全案件のscope/loop/role/session/HEAD/source/外部同期/保持資源をpreviewする。activeな新規mutationは切替境界でdrainする。
- 調査承認を全体承認へ昇格しない。TAK-14は建設物本設ビジュアル移行の残件を統括が正本仕様と照合する。
- 本体とhostの互換版を一組で保全する。新schemaへのwrite後は無条件downgradeせず、mutations停止＋readonly表示で復旧する。
- snapshot/helperのrollbackでGit/Linear/PRを巻き戻さない。未確定外部操作は元operation IDでread-backする。
- 既存タブ・会話・承認済み成果を消さず、再配車/新規Runをmigrationの代わりにしない。

## 9. AI引継ぎメモ

### 現在地と比較基点

- M0〜M7を完了とは判定していない。M0のhost差分統合とM1/M3/M5の受入防御を候補実装中。TAK-14への操作なし。
- primary文書: `1e86c976c2b7ff989b652209c94e4352743acc88`、同目的branchを再利用。
- 配備host: `/home/satotakumi/orca/workspaces/hell-workers/orca-abcd-expansion`、`077774e98c53e54775e5de06637e8909ea6fe415`。
- 既存統括host: `/home/satotakumi/orca/workspaces/hell-workers/orca-parallel-development`、`7d189cc0d49b09791502d18a9d13b6de1c557f71`。
- Orca本体: `/home/satotakumi/tools/orca-ui-lifecycle`、`e6fadf0be0e529ac6829ae5ab56cd636e0b36db1`。
- launcherの現在参照は`ui-e6fadf0b`とabcd helper。実装開始時にHEAD/dirty/processを再照合する。
- 採用hostはM0で共通差分を照合して決定し、両系統への場当たり的なコピペをやめる。公開は別の許可境界。

### 次に行うこと

1. 利用者の再登録後、実Linearの読取回復を確認済み。再入力を追加要求しない。
   実接続で発見したUUID読取拒否を本体側で修正・配備し、同じworkspace/issueの読取を再確認する。
   復号障害の元の原因は未確定であり、APIキー失効とは判定しない。
2. batch 2の候補を固定reviewし、配備manifestと旧案件のscope移行を実施する。既存loopの封印は変更しない。
3. 本文/子課題とscopeの再照合、権限・終了操作の残件を整え、M7の全経路受入前に運用完了と報告しない。

### 2026-09-26 実装batch 1（候補）

- 配備host `077774e9`へ既存統括host `7d189cc0`のguard/recovery修正を統合し、`da02450c`へローカルcommit。
  両方の終了terminal保持を残し、衝突を解消した。受入防御の候補実装は`8796b2f4bac45186d2414fbf32576ae5c0df6fe9`。
- `orca_request_lifecycle.py`を追加。原依頼・scope revision・必須条件・依存nodeをprivate ledgerへ保存する。
  source拘束の固定review、immutableな祖先loop、未適用指示、通知/担当の収束で全体受入を検査する。
- loop specのscope/node拘束、実装と統合後reviewへの全体文脈伝播、追加指示の受理/適用分離、履歴付き改訂を追加。
- close preflight/実行/復旧/worktree completed、およびLinear Doneの生成/送信直前へ共通gateを接続。
- 既存の状態行で工程承認と全体受入を区別。linked案件の空outboxは同期正常と扱わない。新しいUIは追加しない。
- 現時点の通常runtimeはOrca本体停止、旧統括launcher process残存。強制終了・新規Run・既存台帳の手修正をしていない。
- 診断用に既存`ui-e6fadf0b`をCLIで起動し、`linear_credential_unavailable`を再現した。
  Secret Serviceのlogin collectionはlocked=falseだが、保存credentialは復号できない。原因を単なるkeyring lockとは断定しない。
  CLI直接起動には通常desktop wrapperの統括環境がないため、今回起動したmain PIDだけをSIGTERMで終了し、元の停止状態へ戻した。
  既存統括・PTY daemon、token、Linear状態、worktreeは変更していない。標準起動の環境とcontroller版の固定もM0/M7で扱う。
- 詳細と残件は[候補実装の契約](../development-infra/orca-request-lifecycle.md)。このbatchだけでは正常自動継続・全体運用は未完成。
- 候補commit直前の同内容で変更別`contracts, tooling`成功。比較base=`077774e98c53e54775e5de06637e8909ea6fe415`、
  検証時HEAD=`da02450ccb69621fded0d914e2605a5e166710cb`、source=`ce0f5838a7e771d50897f68e13757f1393d9ff8843911d9137ea2a698b5f2ac0`。
  `scripts/tests`798件、Blender補助tooling164件、Ruff/actionlint/perf self-testとdocs/storageがpass。
  実provider・固定reviewの実運用受入とT01〜T12全体は未実施。新規request系13件と実Git loop追加試験は上記798件に含まれる。
  ゲームのRust build/test/native起動、push、PR/Linear write、既存案件migrationは行っていない。

### 2026-09-26 実装batch 2（候補・実受入待ち）

- 候補host commit: `dc6d6aebbbd47940c13b297cd0c2c68151766597`。primary製品ソースへの統合・pushはしていない。
- `orca_request_runtime`を追加し、通常loop driverのtick後から次nodeのplanning配送を接続した。
  request/scope/node固定IDのwrite-ahead、Orca durable promptの同ID再照合、accepted/started/appliedの分離を実装。
  原因不明の別Runや再配車で進行を装わない。旧契約未登録のTAK-14には自動配車しない。
- 通常controllerの`serve`へ単一同期workerを接続。UI heartbeatとネットワーク実行を分離し、
  CLIとの競合も同requestのexecutor leaseで直列化する。未登録の旧案件は接続観測まででwriteしない。
- 部分工程の承認ではLinear全体をstartedに維持する。全体受入後のDone再読をcompletion receiptに結び、
  close preflight/実行/復旧で承認・scope・外部receipt・直近観測・未確定operationの有無を検査する。
- 新規登録CLIは全体計画/工程planning/統合先/接続確認を必須にした。既存loopの収束APIは維持する。
  採用仕様をworker context packageへ入れ、稼働中の追加指示入口を開き、stageの日本語部分一致推測を撤去した。
- 通常desktop wrapperで`ui-e6fadf0b`を起動しても`linear_credential_unavailable`を再現。
  設定でのLinear再接続を利用者へ依頼済み。credentialの削除・平文読出し・置換はしていない。
  appは再接続操作ができるよう起動したまま。旧統括process・担当terminal・TAK-14成果は保全。
- 新規試験は永続ID・応答消失・開始未確認・retry禁止・offline/取消・未処理指示・部分承認・
  全体receipt/未知write/reopen・通常driver/controller接続を対象とする。providerはfixtureであり実受入ではない。
- commit直前の同内容で変更別`contracts, tooling`がpass。base/検証HEADは`8796b2f4bac45186d2414fbf32576ae5c0df6fe9`、
  sourceは`e2be6522cef49d99261792242a30d1fcf02ecd106587473df0f5e5077c014e46`。
  `scripts/tests`811件、Blender補助164件、Ruff/actionlint/perf self-test、docs/storageがpass。
  ゲームRust build/test/native起動は行っていない。Helpは開発用host→Orca/外部サービスに限定されNo impact。
  先行runは検証中のsource変更により最終gateが拒否したため不採用とし、上記同一subjectで全群を再実行した。
- 本体UIの新build、通常案件への切替、実provider固定review、既存案件migration、T01〜T12の実運用受入は未実施。
  本文/子課題の変更採用、終了操作/権限モデル、配備manifestにも残件がある。認証だけ直れば全体完了とはしない。

### 2026-09-26 実装batch 3（候補・未配備）

- 本体の全workspace team/project一覧が復号失敗を黙って除外し、正常な0件として返す問題を修正。
  healthyなworkspaceは維持し、失敗はpartial/個別errorとしてCLIまで伝播する。
- 後継工程の巨大context引数を正本参照へ変更。長い通常本文は別のUTF-8上限を使う。
  prompt応答消失時にdriverを終了せず、同じdurable IDだけを再照合する。
- 外部本文・子課題の観測版と採用scopeを対応付け、変更時の新規配車/Done/closeを拒否。
  `request-status`/`reconcile-external`を統括promptと通常経路へ接続した。
- 同期先をworkspace/issue UUIDへ固定。既存confirmed操作から現在のDoneを合成せず、再読を要求。
  破損routeが同期worker全体を終了させないよう案件単位で隔離した。
- 認証調査: OS keyringはunlocked。稼働版と同一SHAのElectron 43.7.0を使った隔離probeは
  `gnome_libsecret`を選び、合成文字列のroundtripは成功、保存済みLinear ciphertextの復号は失敗した。
  これはAPIキー失効の証拠ではなく、元の暗号鍵/保存情報との不一致原因は未特定。
  credentialの再発行/再入力/削除/置換は未実施。再入力依頼は根拠不足として撤回した。
  probeコードと一時profile（apparent 93 bytes）は撤去した。認証情報・既存terminalは保持。
- host変更別`contracts, tooling`がpass。base/検証HEADは`dc6d6aebbbd47940c13b297cd0c2c68151766597`、
  sourceは`891c479d4e80ee524c3d49bca1dfb8ac36406d944617b80e65ec2ad70ec23ab6`。
  scripts/tests 819件、Blender補助164件、Ruff/actionlint/perf self-test、docs/storageがpass。
  本体関連37 tests、node typecheck、変更fileのoxlintもpass。
  実provider/実Linearの全経路受入、配備manifest、旧案件正式移行、終了/権限モデルの残件は維持。
  本batchや認証回復だけで全体完了とは報告しない。

### 2026-09-26 再登録後の実接続とUUID読取補正

- 利用者が再登録済みと明示。稼働runtimeからTAK-14の`--full`読取が成功し、partial=false。
  設定・credentialをagentが再登録/削除したものではない。
- UUID指定では`linear_issue_required`を再現。CLIはUUIDを案内するが、本体の
  `readLinearIssueContext`はidentifier/URLだけを受理していた。既存UUID判定と同じquery経路を再利用し、
  immutable IDで読み、返却IDが不一致なら拒否するよう修正した。失敗テスト2件を先に確認した。
  本体commit `60a5d78e77c1c1734b7a799154d2faf7c6687e03`、関連40 tests・node typecheck・oxlintがpass。
- `ui-60a5d78e`を別build・梱包検査後に切替。glibc/native 17件、daemon entry、plugin資源検査がpass。
  runtime `3f0a63cf-05bb-4905-a348-453ab7fbdfdb`と実行binaryを照合し、UUID指定の実Linear full読取と
  host adapterが成功した。全workspace一覧もTAK 1件・partial=false。既存タブ数と統括会話を保持。
  切替前設定と旧buildは復帰用に保持し、scope不明なTAK-14は全体完了/再配車せず保留を維持した。
  host `36d9b6678fbda0cbda00bc4db7fe840221ca9801`は候補配備。全体の固定reviewと実provider受入は未完了。
- 再登録後の同依頼復帰・古い認証エラー除去・planning重複送信なしを回帰testへ追加。
  変更別`contracts, tooling`が再度pass（scripts/tests 820件、Blender補助164件）。
  base `dc6d6aebbbd47940c13b297cd0c2c68151766597`、検証HEAD `36d9b6678fbda0cbda00bc4db7fe840221ca9801`、
  source `73132a8076f35cd4f162ddfaa1896cb5c3168bd0baa39d204cac5a4e96bfe824`。
  runtime台帳もofflineではなくobservedへ更新済み。ゲームbuild/test・追加の製品実装は行っていない。
- Linear活動履歴で、TAK-14は2026-09-26T11:57:17ZにGitHub integrationによってDoneへ移動した。
  PR #28は11:57:15Zに運用基盤branchへmerge済み。これは外部連携の動作確認であり、全体受入の証拠ではない。
  この調査ではLinear状態変更・再配車・製品branchの変更をしていない。

### 2026-09-26 外部Doneによる配車前停止の復旧

- `resume-linear-state`を追加し、2担当の一括復旧、exact digest/source/統括所有、dispatch journalとrole証拠、
  最新Linear active、応答消失時の同一要求再照合を検査した。未知Dispatchや取消は復旧対象にしない。
- 依頼PRの生成・送信を全体受入headへ拘束。古いqueued PRのhead不一致はdurable blocked_policyへ移し、
  supersede可能にした。sending/unknownは再送せずread-backを維持する。
- 固定read-only reviewは追加指摘を解消してAPPROVED。対象base/HEAD `ce0c678ab8b7ffedcf23d78d27fc54f5d88707bb`、
  4ファイルdiff digest `b9bbc6a25b935e9b6bcd5dd11596f6a082d82deff9f6efc8ff3c03532b4bfbea`。
  focusedはreview-loop 62件＋external-sync 34件。最終変更別contracts/toolingはscripts 833件＋Blender補助164件、
  lint/perf self-test/docs/storageがpass。source `e1cfa7cb139e4c4c1afbaed2ca3962e2eae7d4e84d1701cf3ef9590516a7e448`。
  検証中にレビュー修正が入った中間runはsource変更gateで拒否されており、不採用。
- 利用者許可に従いTAK-14をIn Progressへ戻して再読済み。同じ統括terminalと会話を正常終了→再開し、
  旧parallel helperのprocessを残さずabcd helperへ切替。Orcaは同じ`ui-60a5d78e`で再起動し、
  runtime `b12847fc-3adc-4856-bd23-c35b14a0a061`、監督と統括の実行元を照合した。
- 実停止loop `5e042611208360350eb0811b876687cb6b5e126831a2ed6a14cdfe9a7d890942`をguard付き復旧。
  generation 4、Run `run_3236488f0dff`、既存M1-c作業場を維持してactive/plannedからdispatchingへ進んだ。
  snapshotもpausedからworkingへ変化した。同RunでTask `task_62c20b5cafa5`、Dispatch `ctx_1cd6522b4f25`が
  一度だけ作成され、実装Aはimplementingへ進んだ。依頼全体の完成やM0〜M7完了の証拠にはしない。
- M1-c既存作業場の保持未登録を修正（28,831,744 allocated bytes、owner/consumer/終了条件を登録）。
  登録用の一時JSONだけを撤去し、作業場・成果・履歴は削除していない。ゲームbuild/testは本修正では未実施。

### 計画作成時の検証ログ（後続実装の結果は上記）

- 今回の検証対象: primaryの計画・上位計画追補・索引のみ。2026-09-26、変更別contracts、docs/index/link、diff、storageはpass。
  比較base/HEADは上記primary SHA。commandは`python3 scripts/dev.py ci check --base 1e86c976c2b7ff989b652209c94e4352743acc88 --mode auto`。
  未commit文書差分を含む検証であり、実装修正の検証成功を意味しない。CI公開は行っていない。
- 実装test/本体build/Linear実更新/ゲーム検証: 未実施（計画のみのため）。
- Help判断: 文書だけでruntime producer/consumerを変更しないためNo impact。
- 計画時点の未解決: Linear認証復号の原因・最新外部状態、標準GitHub連携の現在設定、全旧案件migration。
  その後の認証回復・GitHub活動履歴の実読取は上記の追補を参照する。

### Definition of Done（是正実装全体）

- [ ] M0〜M7とT01〜T12が証拠付きで成立し、部分loop成功を依頼全体成功にしていない。
- [ ] 通常入口から複数工程、全体受入、外部反映、終了まで内部ID入力・再促進なしで完走した。
- [ ] 仕様改訂/外部変更/再起動/中断から同じ依頼と成果を保全して復帰できる。
- [ ] 本体/host/台帳/Linear/GitHub/terminal表示の対応が実runtimeで一致する。
- [ ] 同一対象のCIまたはローカル変更別検証、固定review、Help/必要なUI受入/storageを確認した。
- [ ] 採用差分と恒久仕様を正本化し、使わない専用資源を整理。残存資源は別の具体的consumerへ引継いだ。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-26 | Codex | 全体契約の欠落、未接続helper、認証/同期/終了/配備の境界を横断監査し是正計画を作成 |
