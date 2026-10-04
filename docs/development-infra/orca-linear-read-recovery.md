# Orca Linear読取の待機と復旧

> 歴史資料（2026-10-04整理）。独自統括は廃止済みで、以下のcommand・Run・復旧経路は実行しない。
> 現在の規則は[通常Orca運用ガイド](../orca-quickstart.md)。旧未完条件と過去の結果は履歴のまま保持する。

## 完全性と再試行の境界

`scripts/orca_issue_context.py`はfull issueを完全に取得できた場合だけsnapshotを返す。
Orcaの`meta.includeErrors`がcomments / children / attachments / relations / activityについて
`linear_timeout`、`linear_rate_limited`、`linear_network_error`だけを返した場合は、
`LinearRuntimeUnavailable`として通常driverの次tickへ延期する。部分取得を成功扱いせず、
同じ工程・成果・Runを保持する。これは読取の再試行であり、外部writeの再送ではない。

認証、権限、未知コード、不正形式、cap到達は従来どおり拒否する。
top-levelの`runtime_unavailable`をincludeErrorsの有効コードとして受理しない。
providerのmessage本文は例外へ転記しない。

## 既に停止している工程

H3-aでは、`ExecutionConnectionUnavailable`も同じDriverの接続待機へ分類する。
lane step / integration stepの包括的ValueError処理より先に再送出し、未着手の接続拒否を
durableなpausedへ変換しない。Driverの起動import失敗や未知例外はfailedに記録し、
生きたprocess PIDだけを根拠に稼働中とは表示しない。HostBusyの待機理由は資源使用中と明示する。
H3-aは同一source全検証と同統括sessionへの反映を確認済み。最新の配備状況は
[恒久修正計画](../plans/orca-system-hardening-plan-2026-09-29.md)を参照する。

統括の`resume-linear-runtime`は、freshなshowのloop digestと可視統括を拘束する。
旧`LinearIntakeError: Linear full issue context is incomplete`も対象にするが、
同じlaneの既知のpre-command工程からの停止だけに限定する。
最新のfull issueが完全で、同じ課題identity、active状態であることを再取得してから
同じ工程へ戻す。認証情報が読めない場合のcached-active fallbackを復旧には使用しない。
未処理inbox、別の停止理由、古いdigest、外部Done/取消、不明な前工程は解除しない。

利用者へIDやコマンド入力を求めず、統括が正規helperを使う。拒否時は保存済み成果を保持し原因を調査する。
復旧受理だけを再開と報告せず、同Runで検証・固定review等の次操作が実行されたことを確認する。
コード適用には同一統括sessionの正常終了・再起動が必要であり、稼働中driverの自動reloadは仮定しない。

### Local復旧adapter（H3-b、同統括Driver反映済み）

通常Driverがnative照合後・tick前に既知の読取停止を検出し、上記helperを選択する。
利用者pauseは選択前に待機し、fresh読取後と保存直前にもcontrol・可視統括・元loop・全subjectを照合する。
元の停止loopはprivate journalへ保存する。復旧は同じRunのローカルphase復元だけで、
新規配車・検証・ACK・外部writeをadapterから行わない。通常tickが次操作を所有する。
未知理由やvalidation timeoutをこの自動経路へ分類しない。
復旧条件としてinboxの回収が必要な場合は、paused専用tickで所有・停止・native条件を再確認し、
mail pollだけを通す。承認再検査や新規配車を行わず、回収を待つこと自体で回収処理を止めない。
固定reviewと全tooling 1,461件＋164件は合格。監督controllerを正規refreshし、
製品検証の正常終了後に同統括sessionを正常再起動して反映した。同Run・generation12で
固定reviewが実際に再開したことも確認済み。typed attention通知と横断障害注入は未完であり、
配備と実受入は収束計画で別々に確認する。

### 判断通知と配車の分離（H3-c、候補実装）

review差戻し・validation再診断・統合後の指摘は、同じ依頼・Run・世代・対象sourceに
結び付いた判断obligationとして通知する。受理やturn開始を、修正の適用・依頼の完了とは扱わない。
routeのprivate journalとloop内の唯一のhistory receiptが一致し、journalのcompleteまで
確認できた場合だけappliedにする。保存後に終了しても、Driverはローカルreceipt回収から再開する。

統括の3つのroute入口はscheduler解放まで待機し、取得後に最新の対象・所有・停止条件を
再検査する。通知の送信処理と競合しただけでは判断を失わない。scheduler保持中からの
再入は禁止し、worker/reviewer/workspaceの下位leaseを待機方式へ広げない。

native結果待機中は通常tickと配車を止めたままにする。ただし正式finalize・先行結果の適用・
同一subject・native通知の受理/開始・単一の統合判断を確認できたときは、その判断通知だけを許可する。
既存ターミナルと同じsemantic IDを使い、結果不明の操作を新IDで再送しない。
新規通知の本文にはprivate evidence記録のpathとbinding digestだけを載せ、検証履歴全体を
argvへ展開しない。既存の結果不明な通知本文は変更せず、同ID・同本文を照会する。
CLI送信は本体上限に加え、Linuxの単一引数・argv/env総量の実上限を送信前に検査する。
現在の配備・全体検証結果は収束計画を正本とし、候補コードの存在だけを運用可能の証拠にしない。

## Helpと検証範囲

Help実レビューはNo impact。producerはOrca Linear CLI応答、consumerは開発用host loopであり、
ゲーム入力、描画、runtime assets、Help catalogには接続しない。
ゲーム側のbuild/test/native受入は既存の統括ワークフローの所有とし、本基盤修正の検証には混ぜない。
回帰テストは通信エラーの待機、完全読取後の継続、未知・権限エラー拒否、
認証不足時の停止維持、旧停止の同一Run復旧、統括起動指示を対象とする。

2026-09-28受入: 固定read-onlyレビューAPPROVED、関連47件、変更別contracts/tooling
1090+164件、Ruff/actionlint/perf self-test、primary storageがpass。
比較base/headは`ae5b2066c4fd39a8dd6a6c439e272d125f749241`、sourceは
`14eec1bcc63ed148a68da2018d5b590b3296b25e56b1287c0823f630e9bb1a6b`。
同じTAK-14の統括sessionを正常再起動し、統括自身のresume処理、fresh Help判断、
同Run `run_3236488f0dff` のvalidation_runningとhost検証process起動を確認した。
この記録は基盤停止の復旧受入であり、製品修正・依頼全体の完了承認ではない。
