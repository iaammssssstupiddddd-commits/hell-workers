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

連携された案件の空outboxは「同期不要」ではなく「同期確認の記録なし」と表示する。
この表示だけで接続正常・同期完了を判断しない。
画面は既存の状態行と実terminal履歴を使い、追加の大きな結果UIは作らない。

## 通常driverとcontrollerへの接続（候補batch 2）

`orca_request_runtime`は既存loop driverのtick後に実行する。scope拘束された承認済み工程から
依存が満たされた次nodeを選び、同じ統括へのplanning入力を送信前に保存する。
request/scope/nodeから決めた固定prompt IDと本文をOrcaのdurable prompt APIへ渡す。
応答消失後も同じID/本文だけを照合し、旧host・process交換等で安全なretryが否定されたら自動再送しない。
`accepted`、`started`、後継loop登録による`applied`を別状態にする。入力受付やturn開始だけで工程を閉じない。
worker間通信のRun inboxは引き続き既存driverだけが消費し、別consumerを作らない。

通常`serve`は単一の同期workerを所有する。外部通信はUI snapshot更新とは別threadで実行し、
送信前のdurable intentと送信後read-backには既存executorを再利用する。
CLIとcontrollerが競合しても、一つのrequestのread/send/read-backは専用leaseで直列化する。
個別loopのreviewではLinear案件をreview/Doneへ進めず、依頼全体はstartedを維持する。
最後の全体受入でのみDone intentを生成し、同一subjectの受入証拠と外部確認operationを結ぶreceiptを保存する。
close preflight/実行/復旧では、そのreceipt、現在の承認、直近の接続観測を再確認する。
外部での取消や完了後reopenを観測しても自動的に上書きしない。

新規`register`/`register-successor` CLIは、scope/node、工程planning、統合先、Linearの現状態を必須にする。
旧案件は接続状態の読取までに留め、全体契約未照合のまま自動配車/Doneを行わない。
稼働中も追加指示の入口は残すが、実行中ticketを書き換えず、受理した指示の照合を全体受入前に要求する。
UIのstageはlane/integrationの保存済みenumから決め、日本語の説明文に含まれる語から推測しない。

## 実装済みと残件

候補側には上記の受入台帳、revision、ticket拘束、終了/Doneの拒否gateとテストを実装した。
実Gitを使う統合loop試験では、scopeを渡した固定reviewの封印・全体受入・承認後のソース変更拒否を確認する。
providerとOrca接続はfixtureなので、実providerによる全経路受入の代用にはしない。

以下はまだ運用完成を主張できない理由である。

- 旧案件の正式なscope移行と、全入口の互換照合。
- 上記の自動継続・通常同期は候補コードとfixture試験まで。実providerでの無介入継続は未受入。
- Linear認証回復、本文/子課題の変更とscopeの採用版を対応させる再照合。
- 人手受入・公開条件、非表示/中断/取消と成功終了の分離、版互換・配備manifest。
- 同じ通常入口からの実provider複数工程、実Linear/GitHub、UIと再起動を含むT01〜T12の受入。

Help影響はNo impact。変更のproducer/consumerは開発用Orcaのhost台帳・terminal・外部サービスであり、
ゲーム内入力、描画、runtime assets、Help catalogへ接続しない。ゲームbuild/testは本是正の検証に含めない。
