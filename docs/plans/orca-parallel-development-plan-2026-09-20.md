# Linear受付・Orca実行基盤の統合計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `orca-parallel-development-plan-2026-09-20` |
| ステータス | In Progress — Linear `TAK-5` のL1正常系・L2相談継続、3 roleのread-only実Task、A/B限定編集と2レーン並列実行を受入。自動配車・異常系・L4切替は未完 |
| 作成日 / 最終更新日 | 2026-09-20 / 2026-09-21 |
| 作成者 | Codex |
| 関連提案 | [並列実装と専任レビューの運用素案](../proposals/orca-parallel-development-proposal-2026-09-20.md) |
| 関連Issue/PR | N/A（公開なし） |
| 調査対象 | Orca `v1.4.205`、Fedora 44 x86_64 / Wayland |
| repository基点 | `6abeeed92cf14e2e5aafc8240e1c64989b609a84`、primaryに別作業のdirty変更あり |

初回依頼の導入・仕様確認後、ユーザーがOrcaへ移行し、ルール見直しを含む環境実装を依頼した。
主担当が専用作業場でM2/M3を実装し、拒否試験を通した経路だけを条件付きで許可する。
ゲーム変更・公開は含めない。既存のprimary上の並行変更は保全する。

## 現行計画: Linear受付と既存Orca基盤の再利用

2026-09-21のユーザー指示により、依頼受付・進捗管理はLinearの標準連携へ寄せる。
既存の資源制御、role launcher、固定reviewer、会話再開を再利用し、受付UIの自作拡張は止める。
本節L0〜L4と§7・§9を今後の実装順・検証範囲の正本とする。後段R0〜R4/M0〜M5の実績は保持するが、
旧受付の拡張、全群ゲーム検証、並列効果測定を今回の必須作業として再開しない。
L0改訂時は計画と関連文書のみで、Linear認証・課題作成/更新・既存依頼のuploadは行っていなかった。
後続L1/L2では専用試験issue `TAK-5` に範囲を限定して接続・更新・固定snapshot取込を受け入れた。
L3では固定reviewer、Codex A、Cursor Bのread-only実Taskを起動して受け入れた。
L3Eでは別worktreeのA/B限定編集、統括検証、同一固定reviewer、2レーン同時実行まで受け入れた。

### 正本と責務

| 正本 | 保持する情報 | 他の状態へ読み替えないもの |
| --- | --- | --- |
| Linear | 人向けの依頼、優先順位、進捗、相談・レビュー結果の要約 | 担当者/ラベル/Doneは実行権限・process終了・承認の証拠ではない |
| primary `docs/` | 仕様、運用規則、現行計画 | 課題ごとに全文を複製して別仕様を作らず、issueから参照する |
| 既存host制御台帳・固定ticket | 開始時の依頼snapshot、scope、source/index、provider/session、排他、承認対象 | Linear本文の後編集で実行中ticketや承認対象を変更しない |
| Orca runtime | worktree/terminal、監督付き実行のRun/Task/Dispatch・receipt | UIの表示状態やLLMのexit 0だけでTaskを成功にしない |

Linearは常設の受付・可視化先であり、統括LLMの常駐や自動配車を提供したことにはならない。
統括は明示操作で起動し、A=Codex、B=単純task専用Cursor CLI、reviewer=固定Codex session 1つを維持する。
共有checkoutと任意のbackground編集は禁止を維持する。primaryのルールは、別worktree・固定ticket・mount境界・
最大2 worker・固定read-only reviewer・統括所有の検証/commit/直列統合を満たす専用Orca launcherだけを条件付き例外とする。
L3Eの実受入は完了したため、明示ticketと専用launcherを通る監督付き編集だけを利用できる。
Linear受付からの自動配車、通常のOrca agent起動、共有file/APIを含む並列化は引き続き許可しない。

### 実装済み資産の採否

codeの参照元は専用candidateの `55b27f6e1d9103d7985941c3cbbf135c7299be91`。primary未統合。
以下の再利用は既存コードと検証済み境界の採用であり、Linear対応済みという意味ではない。

| 資産（candidateの `scripts/`） | 方針 | 実績と追加作業 |
| --- | --- | --- |
| `host_coordination.py` とbuild/validation driver | 再利用 | host重実行1枠・role/workspace排他・子へのlease継承。Linear状態をlockの代わりにしない |
| `orca_roles.py` / `orca_providers.py` | 再利用 | A/Bのprovider固定、Bの単純task制限、mount/policy分離。read-only起動・再開とA/B限定編集、Cursor Bのtool denyを実受入済み |
| `orca_role_state.py` / fingerprint / `verify-review` | 再利用 | 同一ticket/session、unknown停止、固定reviewer、変更後の承認失効。承認記録の整合性検査であって署名検証ではない |
| `orca_coordinator.py` | 入力adapterを追加して再利用 | 明示起動・相談/追記・同UUID再開は実受入済み。既存のrequest ID/本文入力へ固定snapshotを渡し、実行処理は作り直さない |
| `orca_frontdesk.py` | 内部snapshot保存を再利用し、candidateのUIをLinear優先へ変更 | `submit()`のUUID/本文拘束を使う。queuedは進捗ではなく内部受付状態として保持し、Linearとの常時双方向同期は作らない |
| `orca_preflight.py` | 再利用 | 対象terminal限定のread-only通信診断。結果は常にdispatch許可と別扱い |
| `orca_task_bridge.py` / `orca_cursor_bridge_hook.py` | 監督付き経路だけ段階採用 | JSON内容比較、exact identity、限定復旧、3 roleの実Task、A/B限定編集と並列実行まで完了。受付からの自動接続は未接続 |
| `orca.yaml`、専用branch/worktree、既存tests | 再利用 | setup/待機、通常agent起動の迂回禁止、回帰fixtureを維持。既存会話・成果・review-active cacheを消さない |

host側の薄いissue入力adapter `scripts/orca_issue_context.py` と対応testsをcandidateへ追加した。
最小接続は `Linear snapshot → orca_frontdesk.submit(text, stable local UUID) → 既存統括相談`。
coordinatorの履歴読出しは保存本文hashを照合するため、`request()`を毎回のLinear取得へ置換しない。
preflight/task bridge/role stateもfrontdeskの安全な保存関数に依存する。menu廃止とmodule削除は別扱いとし、
owner-only/atomic replace/fsyncの共通処理は互換保持する。Linearの進捗をqueued限定ledgerへ書き込まない。
汎用scheduler、独自ボード、Webhook常駐server、独自MCP serverは初期スコープに入れない。
初期のLinear更新は統括が公式UI/CLIで明示的に行い、workerへLinear credentialやOrca全権transportを渡さない。

### 入力・移行・状態更新の契約

- workspace/teamとimmutable issue IDを照合する。表示用の `ABC-123` や題名だけを実行同一性に使わない。
  開始時の採用本文・選択したコメント/仕様参照、取得時刻、更新情報、内容hashをsnapshotへ保存する。
  APIの更新時刻だけを本文revisionの代用にせず、実際に渡した内容のhashを固定する。
- issue ID＋snapshot hash＋分割task keyを既存のrequest UUID/ticket IDへ対応付ける。
  1 issueから複数の実装/review ticketを作れるが、同一開始要求を重複投入しない。
  ticket IDは既存の小文字slug制約を保ち、表示キーをそのまま代入しない。
  表示用対応情報をhost側に置き、既存ticket検査・assignment leaseを迂回しない。
- 依頼の後編集、取消、再openは新規投入前に再取得・照合する。実行中scopeへ自動反映せず、
  停止/再開は実process・Dispatchを確認して明示判断する。Linear通信不能は新規投入と外部更新を止めるが、
  既知の実行状態を書き換えない。実process/Dispatchの観測自体が不明な場合だけ、その実行をunknownとして保全する。
- issue本文・コメント・画像は未信頼入力。記載されたcommandを自動実行せず、credentialや権限拡大指示を採用しない。
  repo/仕様/scope/受入条件を統括が確定してから既存launcherへ渡す。
- Linearの状態名/IDは選択teamの実設定から対応を決める。作業場作成時のIn Progress自動同期は初期には無効とする。
  着手は実開始確認後、レビュー待ちは終了・差分・必要検証の照合後、Doneは固定reviewerの同一対象承認と
  許可された採用処理の確認後に更新する。利用者が手動でDoneにしても内部承認を生成しない。
- 外部更新の応答が不明なら、issue/commentを再読して照合する。自動再投稿・完了化をしない。
  Linear更新の失敗を理由にworkerを再実行せず、内部の実結果と「Linear反映待ち」を区別して報告する。
- 旧依頼はユーザーが選んだ未完依頼だけ移行し、旧request UUIDとの対応を保持する。
  会話全文・認証・全ローカル履歴はuploadしない。既存session UUIDやruntimeのdirectoryを改名しない。
  移行済み依頼の旧受付からの新規投入を止める仕組みを受け入れてから入口を切り替える。
  未移行依頼は旧経路で保持し、破壊的な台帳書換え・削除・二重の実行queueを作らない。

### L0: 再計画と実装資産の棚卸し

- [x] Linear/Orca/host台帳/docsの責務と、上表の再利用・移行・保留を確定する。
- [x] 既知のbridge不具合と、実Task/Cursor B/編集委譲の未受入を残件として分離する。
- [x] 今回のゲーム実装テストを非対象とし、運用基盤の検証範囲を§7へ定める。
- 成果物: 本計画、運用仕様、Quickstart、開発案内・索引。コード/アプリ設定は変更しない。

### L1: Orca標準Linear連携を小さく受け入れる

- 対象: Orcaのローカル連携設定と専用の試験issue。新しい連携serverは作らない。
- [x] workspace `takumi sato` / team `TAK` / 専用試験issue `TAK-5` を選び、外部書込みを同issueの説明・コメントとworktree関連付けに限定する。
- [ ] ユーザーがOrca Settings → Integrations → Linearへ最小権限のAPI tokenを設定する。
  tokenを会話・repository・ticket・logへ貼らず、既存接続があれば無断で差し替えない。
- [x] 公式CLIでworkspace `takumi sato` / team `TAK`を認証値の露出なしに読み取る。
- [x] `TAK-5` を公式CLIで作成し、コメントを1件明示更新してfull再読取りし、既存worktreeへの関連付けを確認する。
  GitHub等の既存issue linkがある場合は上書き前に確認し、通常agentの自動起動を使わない。
- [ ] 未接続・権限不足・別team・通信失敗で成功を偽装しない。未接続の実runtimeでは保存前の拒否を確認済み。
  権限不足・別team・通信失敗と、In Progress自動同期が無効であることは接続後に確認する。
- 完了条件: 認証値を露出せず、許可されたissueだけを読み書きできる。これは「課題管理接続」の受入であり自動開発の受入ではない。

### L2: Linear入力から既存統括・ticketへ接続する

- 対象: issue入力adapter、`orca_coordinator.py` / `orca_frontdesk.py`の入力境界と対応tests。
- [x] snapshot・外部ID対応をcandidateへ追加し、既存のcoordinator slot、turn UUID重複排除、session照合・unknown停止を維持する。
- [x] `TAK-5` の固定snapshotから統括を明示起動し、初回相談と同じsessionへの追記を受け入れる。
  L1の画面に表示できただけではこのcheckboxを閉じない。起動入口とcommandは実装後にQuickstartへ記載する。
- [x] 同じissue/snapshotの二重選択、本文変更、別workspace、不完全context、保存失敗の拒否・復旧試験を追加する。
  古いreview対象の受入は実相談・実review一巡で確認する。
- [ ] 選択した旧依頼の対応付けと、移行済み依頼の旧入口からの重複開始拒否を確認する。
- 完了条件: Linearを人向け受付として使い、既存統括の相談・再開が失われない。更新は統括の明示操作で、worker自動投入はまだ無効。

### L3: 既存Task bridgeを修正し、read-onlyの一巡を受け入れる

- 対象: `orca_task_bridge.py`、role接続・対応tests、host統括の受け渡し。
- [x] **P2: escalation receiptのJSON正規化差**を修正する。導入版Orcaはpayloadを再serializeするが、
  現bridgeは文字列完全一致で拒否する。空白付きraw payloadは送信後unknownとなることをsource＋模擬試験で再現済み。
  payloadだけを厳密な型/schemaを維持したJSON内容比較へ変更し、ID/capability/他fieldの照合は緩めない。
  compact/空白/キー順/escape差の同値と、値・型・Task/Dispatch相違の拒否を回帰testに加える。
- [x] 固定reviewerの実readiness→worker-start→arm→check→done→settlement→process終了を確認する。
  arm成功、exit 0、LinearのDoneのいずれも受入成功に読み替えない。
- [x] Codex Aでheartbeat/ask/escalationを含むread-only lifecycleと失敗/再開を受け入れる。
- [x] Cursor BはShell/MCP/WebFetch denyを保った狭いhook通信経路を設計・受入する。generation拘束、
  同時hook直列化、上流mutation前の一回限りの結果整形retry、二回目のfail-closedを回帰試験と実Taskで確認した。
- [ ] Linear受付→統括→A/Bの独立read-only調査→必要なtooling検証→固定reviewer→差戻し/承認→Linear反映を一巡する。
  A/Bは開発ツールの既存仕様確認など非ゲーム課題を使う。コード編集が必要な修正はmainが行う。
- [ ] 実行中の課題編集、Linear通信断、Orca再起動/古いhandle、終了不明、旧Dispatchの通知で重複起動・誤承認しない。
- 完了条件: 固定reviewerと既存安全境界を通したread-only監督運用が成立する。並列編集の運用許可は別判定のまま残す。

### L3E: 監督付き編集を段階受入する

- 対象: 専用branchから作るcleanな別worktree、固定ticket、`orca_roles.py`の編集worker経路、
  統括所有の検証・commit・直列統合、同一sessionの固定read-only reviewer。通常のOrca agent起動や共有checkoutは対象外。
- [x] primaryの全面禁止を、上記専用経路だけを許す限定例外へ改訂する。worker自身のcommit、再委譲、
  重いbuild/test、shared contract・save・renderer・infrastructure変更は禁止を維持する。
- [x] A/Bが共有fileを触らず試せる非ゲームfixtureを別directoryへ追加し、Cursor Bの`acceptance-edit`は
  `scripts/tests/fixtures/orca_edit_acceptance/worker-b` だけをexact許可する。通常のsimple leaf制約は緩めない。
- [x] Codex Aで単一leafの非ゲームfixtureを1件編集し、許可scope外・Git metadata・primary・他worktreeが不変であることを確認する。
- [x] Cursor Bで既存patternに沿う単純な単一leaf変更を1件編集し、Shell/MCP/WebFetch deny、許可scopeだけのwrite、
  provider固定、complexity/task_kind/acceptance検査、終了後source/index照合を確認する。
- [x] 統括が同じcandidateで必要なtooling検証を実行し、worker終了後に固定reviewerを同一sessionで起動する。
  reviewerはbase/head・差分・scope・検証証拠を読み、sourceへ書き込まず承認/差戻しを返す。
- [x] 実reviewerからの指摘修正は発生しなかった。同一ticket/session/worktreeでの再開境界、
  source/index/session不一致の停止、approval後変更による承認失効を自動testで確認し、統括だけがcommitした。
- [x] A/Bの独立leaf 2件を別worktreeで同時に開始し、実装slot最大2・reviewer最大1・重い実行最大1、
  共有file/APIなし、レビューと統合は直列、という資源・所有権条件を受け入れる。
- 完了条件: AとBの各編集一巡および独立2件の並列試行が、範囲外変更・自己承認・重い処理競合なしに完了する。
  失敗時は成果を保全して新規投入を止め、全面禁止へ戻せる。

### L4: 入口の切替と引継ぎ

- [ ] L2/L3の証拠を確認し、Orca UIの入口をLinearと明示起動の統括へ統一する。
- [ ] 旧受付menuは新規運用から外し、未移行依頼/既存会話の参照・復旧consumerを解消してから不要なUI部分を撤去する。
  再利用する保存/lock/復旧処理は残す。既存の未確定attemptを「移行済み」として捨てない。
- [x] 運用ガイドを実操作で確認し、「課題管理」「統括相談」「read-only監督」「並列編集」の受入状態を別々に表示する。
- [ ] 同目的branchの採用、ローカルcommit/基点変更はその時点の許可を確認する。push/PR/primaryのゲーム変更は含めない。
- [ ] storageの残るconsumerを確認し、成果・会話・review-active cacheを保持したまま不要な試験出力だけ整理する。

一次情報（2026-09-21確認）:
[Orca Linear標準連携](https://www.onorca.dev/docs/review/linear)、
[Linear Agents](https://linear.app/docs/agents-in-linear)、
[Linear Agent API](https://linear.app/developers/agents)。
導入済み1.4.205の `orca linear --help` でもissue取得・更新・comment・関連付けのcommandを確認した。
これはCLI機能の存在確認であり、対象accountの接続・権限・実更新の成功を示さない。
Linear Agent APIによる常駐連携はL1の前提にせず、必要性が生じた場合に別計画で評価する。

## 旧再計画: 常設受付と異種agent運用（実装履歴・残件の参照）

ユーザーの指摘により「この会話を統括と呼ぶ」方式を運用基盤の完成とは扱わない。
当時は既存M0〜M3の資源・権限guardを再利用し、M4/M5の着手順を以下のR0〜R4へ具体化した。
現在の着手順は冒頭L0〜L4。以下の未完項目はそこへ引継ぎ、旧受付を別途拡張しない。
常設するのは受付と状態であり、LLMを無期限に常駐・自動課金させない。

### 役割の確定

| role | provider | 割当基準 |
| --- | --- | --- |
| 統括 | Codex | 受付の依頼を読み、分割・Task/Dispatch・検証・受入・復旧を管理 |
| worker-a（実装A） | Codex CLI | 設計判断、複数箇所の整合、複雑な調査を含むtask |
| worker-b（実装B） | Cursor CLI | 既存patternに沿う局所修正、機械的な小変更、限定test追加など |
| reviewer | Codex CLIの固定session 1つ | 両workerを同じ基準でread-only review。自己修正は禁止 |

Bはticketに `complexity: simple`、判断理由、具体的な受入条件を必須とする。
共有型・save・renderer・build/資源制御・agent基盤自身・原因未確定の不具合は初期のB対象から外す。
「軽量」というラベルだけで制限を緩めず、directory allowlistと外側mount隔離をA/B共通で適用する。
モデル名・reasoning effortは今回指定されていないため固定しない。

### R0: 現状と仕様の再確認

- [x] 統括の入口・永続状態・復旧が未実装で、worker-bもCodex固定であることを確認する。
- [x] Cursor CLI `2026.08.04-aaa8809` が導入済み、`--workspace` / `--resume` / `--sandbox` を持つことを確認する。
- [x] Orca `worker-start --terminal` がcustom launcherの既存terminalを扱えることを確認する。
- [x] 認証を表示・複製せず、隔離後の各provider起動を確認する。
  - Cursorの隔離内 `status --format json` はauthenticated、Codexのversion/DNSは確認済み。
    R2第2batchで統括、第4batchでA/B・固定reviewerのread-only応答と同会話再開を確認済み。
    実worker編集と実tool denyは別途未受入。

### R1: providerと依頼票の境界

- 変更対象: `scripts/orca_roles.py`、provider adapter、対応tests。
- [x] A=Codex / B=Cursor / reviewer=Codexをslotで固定し、Bの複雑taskを起動前に拒否する。
- [x] Cursorのglobal/project config・MCP・認証経路をCodexと分離し、元設定を変更しない。
- [x] lease取得後の対象再検査と、reviewer再開UUIDの形式・履歴fileの存在検査を追加する。
- [x] fake providerと実mount境界で拒否試験を通す。認証・起動失敗は成功としない。
- [x] dirty worker再開時の同一task確認と、固定reviewer UUIDの永続拘束を追加する（第3batchの模擬provider試験。実対話受入は別）。
- [ ] 実Cursor対話でtool denyと担当内編集を確認する。設定loader受入だけで代替しない。

### R2: 固定の開発受付と統括の起動・再開

- 変更対象: `scripts/orca_frontdesk.py`、状態管理module、対応tests、Orca local UI設定。
- [x] Orcaの基盤worktreeに固定名「開発受付・統括相談」を設け、新規依頼・一覧を選べる。
- [x] 受付だけではLLMを起動せず、本文・UUID・queued状態・provider方針をowner-only台帳へatomic保存する。
- [x] 同じ入口でユーザーの選択により統括を起動・再開する（現在はread-only相談/分割のみ）。
- [x] worktree、base、terminal、session UUIDと受付IDを照合可能に保存する。
- [x] 統括slotを排他にし、二重選択・応答消失で重複agentを作らない。
- [x] sourceとGitのdirty状態を保存・復旧と混同しない。未知状態はunknownに留める。
- [x] 正のprocess終了証拠と既知sessionを持つ異常だけ、明示操作で新しい照合turnを起動する。
- [ ] Orca全体の再起動とタブ復元、SIGKILL/電源断等の未確認processの手動照合を受け入れる。

### R3: OrcaのTask/Dispatchと実装→レビューの受け渡し

- OrcaのRun/Task/Dispatchを正本とし、独自汎用schedulerや二重の完了状態機械を増やさない。
- [ ] custom launcherと `worker-start --terminal` の組合せで実際のpreamble・done・ask/replyを確認する。
- [ ] sandboxへOrcaの全権credentialを渡さない。必要ならtask限定の通信bridgeを設ける。
- [ ] 依頼保存→A/B割当→検証待ち→固定reviewer→差戻し/承認の対応IDを受付へ表示する。
- [ ] 終了コード0とreview承認を分離し、旧Dispatchや別fingerprintで採用できないようにする。
- [ ] missing terminal/unknown receiptで自動再送・自動再起動しない。照合してから再開する。

### R3 第1batch: 制限通信のpreflightとreadiness診断

- [x] 先にread-onlyの診断入口を実装する。公式CLIを隔離内で動かし、hostだけが本物の接続認証を保持する。
  proxyは指定runtime・terminal・incarnation・worktreeへ拘束し、`status.get` / `terminal.show` /
  `terminal.wait`（tui-idle、短い有限待機）だけを許可する。Run/Task/Dispatch作成・送信は一切許可しない。
- [x] proxy専用tokenとmetadata/socketだけをread-onlyで渡す。元metadata/socket、別terminal、未知RPC、
  追加field、過大frame、runtime交代、socket切断を拒否し、認証値を出力へ含めない。
- [x] 認証を含むraw応答を転送しない。許可済みfieldへ射影し、readiness trueをagentの実応答・Task受入と混同しない。
  sandbox/LLM設定を緩めず、Codexの対話prompt検出条件を一次sourceで調べる。
- [x] 模擬socketと導入版の公式CLIで境界試験後、実runtimeへの読み取り専用probeを受け入れる。
  Task lifecycle中継・live preamble接続・Cursor shell例外は次batch。preflight成功だけで本番Taskを開始しない。

実装・確認結果:

- candidateに `scripts/orca_preflight.py` と23件の境界testを追加した。
  JSON型/重複key/response ID/runtime照合、wait後のincarnation再確認、7理由/3statusのenum射影、
  CLI各出力256 KiBの逐次上限、絶対deadline、子がpipeを保持する場合のprocess group停止も検査した。
  read-only code reviewの必須指摘を修正して再レビューし、blockerなし。focused testsとRuff成功。
- 実runtime `e52da240-994e-4496-81e3-dbaf29967035` の受付terminal
  `term_a9f8d516-9333-4cf1-98a5-96127b6277ba`、incarnation
  `ce21d2b2-b16a-4cd3-adcf-2eac619fc6cf` に対し、隔離内の公式CLIでstatus/show成功。
  1秒のwaitは拒否で終了。menuへの観測であり、Codex Aのreadiness受入ではない。
  LLM・新terminal・Run/Task/Dispatchは作成せず、全結果のdispatch authorityはfalse。
- 導入版 `app.asar` 内 `out/main/index.js` のprompt detectorをofflineで確認した。
  末尾12非空行にある文脈付き確認案内は通常promptへ戻っても残る。新しい実Codex headerや
  対象行が範囲外となる場合に判定が消えることを模擬文字列で再現した。
  以前の失敗terminalの該当tailは未保存なので原因の確定ではない。偽header/行埋めでの迂回は禁止。
- 全群gate `python3 scripts/dev.py ci check --base 5abe7db6294f4ebbad07faa1cee6aa2985faf85c --mode full`
  成功。contracts/tooling/deps/rust、通常/profiling Rust tests、Clippy警告0。
  tested HEAD/baseは `5abe7db6294f4ebbad07faa1cee6aa2985faf85c`、dirty quality source fingerprintは
  `34656acd77ec86dff2c50049bb4f588a076610d3250011ab4ba7305c0927709c`。
  同じfingerprintを直前に照合してcode/test 2 fileを
  `d06e912e68d1c933144d7ff98c4d535082ec683d` へローカルcommitした。push/PR・primary統合なし。
  Help Skill判断No impact。開発者用CLIだけがproducerで、静的Help catalogへの注入・ゲーム入力/UI/dataは不変。
  primary文書は別作業と分離して未commitで保持する。CI URLなし、ゲームnative受入は非対象。
  primary docs/index/linkとstorage check成功。既存の専用workspace/cacheを27,606,855,680 bytesとして再計測し、
  live Task/Dispatch拘束とreadiness再観測の用途を保持台帳に更新した。一時proxyは終了後除去し、
  preflight rootはallocated 0 bytes、既存cache・成果・会話履歴は削除していない。
- commit後も同じbaseの `--mode auto` が成功（contracts/tooling、Python314件、Blender151件）。
  tested HEADは `d06e912e68d1c933144d7ff98c4d535082ec683d`、clean quality source fingerprintは
  `46bd0677e3351f844881ac06d5b174e65b90f3c1d021406b05122add489ad259`。
  既定base branchが同commitへ解決することを確認し、Orca作業場commentとprimaryガイドを更新・表示した。

### R3 第2batch: Codex用の単一Dispatch通信と段階的接続

- preflightの許可RPCを増やさず、Task専用policyを別moduleにする。公式CLIのstatus / heartbeat /
  escalation / worker_done / check / askだけを対象にし、Task作成・配車・gate・他worker操作はhost統括に残す。
- roleの既存leaseとsource/session照合後、権限ゼロのproxyを起動する。hostがreadinessと
  `worker-start`の成功を確認してから、正確なRun/Task/Dispatch/terminalへ一度だけ接続する。
  初回lifecycle RPC（mutation）は短時間だけ上流未送信で保留できるが、不明・期限切れなら失効し、自動再送しない。
  bootstrapのstatus照合はarm前でも可能とする。
- live preambleのcapabilityはworkerから受けた値を変更せずruntimeへ渡す。credentialを作り直さず、
  本物のtransport tokenはhostだけが保持する。worker_doneの受理とreview承認は分離する。
- bridge/control/他roleのruntimeを全agentのmountから隠し、自分のruntimeとproxy公開部分だけ再mountする。
  Cursor BのShell denyは維持し、本batchではTask bridge接続を拒否する。Bの狭い通信入口は別受入とする。
- 1.4.205のcheckはDispatch capability/expectedDispatchを検査しないため、同terminalを専有する
  controller leaseが必要。proxy失効・in-flight要求排出・role終了より前のterminal再配車を禁止する。
  外部の直接Orca操作まで原子的に防げるAPIとは称さない。
- 模擬runtime＋公式CLIでexact ID、旧Dispatch、capability、replay、異常応答、後処理を検査する。
  live worker readiness未達のまま本番Run/Taskを作らず、未受入なら明確に次段階へ残す。

実装範囲:

- `orca_task_bridge.py`を追加し、read-only preflightの公開許可を維持したまま共有wire層だけを再利用する。
  `orca_roles.py`の明示bridge引数で起動・終了を包み、account state親全体を隔離する。
- 模擬runtime＋導入済み公式CLIでheartbeat / check / ask timeout→resume / worker_doneを確認する。
  実runtimeのTask/Dispatchは作成せず、A/B/reviewer sessionも新規起動・再開していない。
- 読み取り専用レビューから、dispatchのdispatched状態、完了後workerのsucceeded/failed状態、
  wait到着応答のflag省略、未回答質問の再開限定、cache replay時のgeneration再照合、途中起動失敗のcleanupを修正。
- arm済み未RPC終了をunknownに保ち、roleの再開とabandon-startによる迂回も拒否する。
  askの指定値+5秒という公式CLIのinactivity期限には認証後keepaliveで対応し、worker RPC 1件の処理全体の45秒期限は延長しない。
  遅延armと実時間待機を伴う公式CLI fixture、keepaliveで総期限を延ばせない負試験を追加した。
- 公式preambleのask例（600000ms）と有限proxy待機を区別し、接続規約として10000msを指定する。
  armはhost命令のqueueのみ。実readiness、worker-start、注入→armの時間境界は後続実受入で確認する。
  R3全体・M4・R4のcheckboxは閉じない。

検証・採用結果:

- bridge境界24 testとrole再開拒否test、既存境界testを全群gateで確認した。
  Unicode token、別ID/宛先、未観測ACK、回答済みの別質問resume、capability変化、未知replay、
  arm待機切れ、未settled終了、起動途中失敗、source変化、keepalive総期限、state隔離を含む。
- `python3 scripts/dev.py ci check --base d06e912e68d1c933144d7ff98c4d535082ec683d --mode full` 成功。
  contracts/tooling/deps/rust、Python339件、Blender151件、通常/profiling Rust tests、Clippy警告0。
  tested HEAD/baseは `d06e912e68d1c933144d7ff98c4d535082ec683d`、dirty quality source fingerprintは
  `a4bc6a7d674919fbd37697393321f6a7173b23fbf6fb04bd80455afbe283ec84`。
  直前の一致確認後にcode/test 5 fileを `a7c1bbbcb9ff8e1ac3321f52780d655c18237d82` へローカルcommitした。
  途中候補のgate成功を最終証拠へ流用せず、レビュー修正後の対象で全群を再実行した。
- Help Skill判断No impact。producerは開発用role/通信driverで、静的Help catalog・ゲーム入力・UI/runtime dataは不変。
  primaryの別作業と文書正本はcommit/候補gateに混ぜず、未commitのまま保全する。
  CI URL・push・PR・primary統合なし。ゲームnative/GPU受入は非対象。
- 実runtimeは1.4.205 / `e52da240-994e-4496-81e3-dbaf29967035` のままready、既定基点は同じ専用branch。
  新LLM・terminal・Run/Task/Dispatchは0件。実readinessとworker-start/armの接続、Cursor Bの通信、
  受付から固定reviewerまでの自動運用は次段階へ残す。
- commit後も同じbaseの `--mode auto` が成功（contracts/tooling、Python339件、Blender151件）。
  tested HEADは `a7c1bbbcb9ff8e1ac3321f52780d655c18237d82`、clean quality source fingerprintは
  `ec7f38fe9f0c8da58c7385612f5bc34f4dc6dbf42e1b5551054bf2824e0cae1d`。
  既定branchの同HEADへの解決とOrca作業場commentをread-backし、primaryのガイドをeditorで開いた。
  primary docs/index/linkとstorage check成功。同じworkspace/cacheは27,607,040,000 bytesで保持し、
  次のreadiness・実Task接続のconsumerを台帳へ更新した。Task bridge rootは0 bytes、実attemptは未作成。
  fixtureの一時socket/metadataは試験終了後に除去済み。既存cache・成果・会話履歴の削除なし。

### 2026-09-21: 実CLI受入とR3接続条件

- 最新提示ルールでは編集agentへの委譲を禁止しているため、実CLI受入はread-onlyに限定する。
  mainがコードを編集し、A/Bは明示的な `read_only: true` ticket、空のwrite scope、
  exact source fingerprintで起動する。CursorはAsk modeとWrite deny、外側readonly mountを併用する。
  readonly起動はdirty sourceの調査を許すが、起動前後のsource不変と同一ticket/session継続を要求する。
- 固定reviewerの実TUI初回応答・正常終了・同UUID再開応答を確認。
  session `01a0bfc7-0a67-7c33-a55f-b215b411c3de`。合言葉を再注入せずに復元できた。
  これはlauncher/session受入であり、コードレビュー承認・Orca Task完了ではない。
- Linked worktreeのtrust画面がprimary rootを示した。設定読込みの証拠とは区別し、
  candidateに加えGit common側のprimary provider configもmountで隠す実装・回帰試験を追加した。
- 同梱1.4.205 sourceによるR3条件: Dispatch capabilityはtransport authの代替ではない。
  host限定proxyが必要で、公式CLIのlive preamble・capability・Task/Dispatch/terminal/runtimeを
  改変せず拘束する。`dispatch-show --preamble` の再生成previewはlive capabilityを含まない。
  `check`にはcapability引数がないため、完了後の旧bridge失効も必須。
- 現launcherのterminal送信receiptは `provider: unsupported`。TUIが動いたことと
  `worker-start --terminal` のsupervision受入を分け、認識経路を確認するまでRun/Taskを作らない。
  制約を避けるために隔離やCursorのShell denyを解除しない。
- Codex Aもread-only ticketで初回応答・同UUID再開応答を確認した。
  session `01a0bfcf-3aed-72c1-be2b-e744403485ef`、role source fingerprint
  `26eef8dfc2d84b978da360cefe1058e2568deed03facb754a5cea6d574947040`。
  現在のOrcaはモデル切替案内を閉じた後も `agent-interactive-prompt` を保持し、
  `tui-idle` が2回ともfalseだった。画面の入力待ちだけでsupervision可とは判定しない。
  receiptのunsupportedだけでは起動不可と断定しない（1.4.205のCursor observerは元々非対応）。
- Cursor Bの実起動は `cli-config.json` のtmp→renameで `EBUSY`、exit 1。
  初期応答とsession作成を受け入れられず、attempt
  `5f8e178b-84da-4a65-b863-ea85d2a93011` を `unknown / process_exited: true` として保全した。
  再送・別sessionへのfallback・台帳の成功への書換えは行わない。
  source調査ではglobal設定のstartup保存とreadonly mountの衝突が判明。
  global metadataと不変project permissionを分離し、実loaderのglobal書換え後にもdenyが残ることを確認した。
  保存済みexit 1・process終了・履歴不在・元ticket一致を確認し、`abandon-start` で当該失敗だけを
  明示終了した。失敗証拠/runtimeは保全し、同じticketの再実行は禁止したまま。
- 修正後の新ticket `orca-cursor-policy-tui-acceptance` でCursor BのAsk mode応答・exit 0・
  同UUID再開応答を確認した。session `cbda143c-7174-4d80-ba54-25e60ecaea9a`、role source fingerprint
  `45c1f62af74407fd56ae2c60fdf8699de2ed5bd7e5d3c906a4bc955ad673077c`。
  合言葉を再注入せず復元し、両turnでsource不変・終了記録を確認。Orca `tui-idle` もtrueだった。
  実編集・実tool deny・Task/Dispatchは実施しておらず、R1/R3/R4の全受入とはしない。
- 最終read-only code reviewでblockerなし。`abandon-start --dry-run` の拒否testは
  有効な照合引数を全て与える形へ強化し、変更後の対象で全群gateを再実行した。
  `python3 scripts/dev.py ci check --base 074f47bcb260831b92ee9dfe0041b9079bf2c4a9 --mode full` 成功。
  contracts/tooling/deps/rust、Python291件、Blender151件、通常/profiling Rust tests、Clippy警告0。
  tested HEAD/baseは `074f47bcb260831b92ee9dfe0041b9079bf2c4a9`、dirty quality source fingerprintは
  `7141ff95ada5528b58690744f1fd79b0c905ae94889d4896a736f640a45aa82d`。
  Help Skill判断No impact。player-facing入力/UI/runtime dataと静的Help生成経路に変更なし。
  直前のfingerprint一致確認後、7 fileだけを `5abe7db6294f4ebbad07faa1cee6aa2985faf85c` へ
  ローカルcommitした。CI URL・push・PR・primary統合なし。文書正本はprimaryの未commit変更として保全。
  A/B/reviewerの試験terminal 3件は保存済みexit 0とshell復帰を確認後にcloseし、会話/runtimeは保持した。
- commit後も同じbaseで `--mode auto` を再実行しcontracts/tooling成功。
  tested HEAD `5abe7db6294f4ebbad07faa1cee6aa2985faf85c`、clean quality source fingerprint
  `0a9531aff64d894dd97a02ea32fd4c90cfb219b723b34b0cd5b244cfa533bdfe`。
  既定base branchの新HEADへの解決、受付メニュー再起動、primaryガイドのOrca editor表示を確認した。
  受付の固定名は `terminal list --include-visual-layouts` のtab titleでread-backした。
  `terminal show` のpane titleはshellのOSC titleであり、UIの固定tab名とは別だった。
  primary docs/index/linkとstorage checkも成功。保存資源の用途/bytesは運用仕様に更新した。

### R1 第3batch: commitと継続状態の拘束

- ユーザーの「コミットして継続」により第1/第2batchのcode・rule・test 19 fileを
  `a03c4d5401f0486427aac81c0a91f335f79468bc` へローカルcommitした。push/PRなし。
  commit直前のsource fingerprintは第2batchの全群成功対象と一致した。
  primaryの別作業と文書正本は専用branchのcode commitへ混入させていない。
- 今回の実装対象はR1の再開境界。workerはticket内容・provider・repo/common dir・branch/base・
  担当範囲を永続拘束し、再開は同UUID・前回終了時のsource/index fingerprint一致・明示追記を必須にする。
- 制御台帳はagentのrw runtime外に置く。worker runtimeはtask別、reviewerは固定runtime/session。
  起動前intentと終了証拠を保存し、0個/複数/別sessionや不明processでは新規/再開を拒否する。
  TUIの終了はOrca Task成功やreview承認ではない。Orca Task状態はまだ作成しない。
- Codexはrolloutのsession metadata、Cursorは導入版のlocal chat storeとworkspace keyを照合する。
  プロバイダの内部保存形式はcompatibility testと拒否経路を伴わせ、実対話の受入とは区別する。
- 承認記録も固定reviewer UUIDと直近の同一subject観測へ照合する。署名やCI証拠の検証とは称さない。
- 実装済み: `orca_role_state.py`、role launcher/provider adapter、拒否/継続tests。
  同一taskのdirty継続、別slot所有拒否、別worktreeへの同reviewer継続、scope外rename、session分岐、
  保存失敗、欠損attempt barrier、割込み→TERM timeout→KILL→wait→unknown保持を検査した。
  Cursor導入版のserializer/workspace keyをofflineで確認し、committed WALの読取りとsnapshot安定性も検査した。
  実Codex/Cursor TUIの編集・正常終了・Ctrl-Cは未受入。実processとTask bridgeを次に確認する。
- 第3batchの全群gate成功。command:
  `python3 scripts/dev.py ci check --base a03c4d5401f0486427aac81c0a91f335f79468bc --mode full`。
  contracts/tooling/deps/rust、Python279件、Blender151件、通常/profiling Rust tests、Clippy警告0。
  tested HEAD/baseは`a03c4d5401f0486427aac81c0a91f335f79468bc`、dirty source fingerprintは
  `6124865ce7453f1f4e1131c02c663592e3bf495354d46ecdb7876a8fe7b412f5`。
  Help Skill判断No impact（開発用の起動/継続だけでstatic Help注入・game入力/UI/runtime dataは不変）。
  成功対象と一致を再確認し、code/test 7 fileを`074f47bcb260831b92ee9dfe0041b9079bf2c4a9`へcommitした。
  local検証のためCI URLなし。push/PR/primaryへの統合なし。
- commit後も同じbaseで`--mode auto`を再実行しcontracts/tooling成功。
  tested HEAD: `074f47bcb260831b92ee9dfe0041b9079bf2c4a9`、clean source fingerprint:
  `6043adf691c1d6dc1b466c03d13dda045d105609b528a5b042bc09ff40edf93b`。
  Orcaの既定base branchがこのHEADへ解決することをread-back確認し、受付を同terminalで再起動した。
  primary docs/index/link、diff hygiene、storage checkも成功。primary文書は未commitの正本として保持する。

### R2 第2batchの設計と実装

- 受付メニューに「統括へ相談」「同じ相談へ追記」「相談状態/回答」を追加する。
  `codex exec --json` を選択時のみ同期起動し、応答後はLLM processを終了する。
  初期接続はread-only相談/タスク分割に限り、実装workerの自動投入はR3受入まで無効。
- 受付IDごとに専用Codex runtimeと会話UUIDを対応させ、host-wide coordinator slotで同時実行を1件にする。
  worker/reviewer slotとは区別する。制御stateはagentのrw runtimeの外へ保存する。
- spawn前にattempt ID・入力・対象repo/base/fingerprintをatomic保存し、`thread.started` を受けた時点で
  会話UUIDを保存する。初回の同一受付ID再送は応答を再表示し、追加ターンは明示IDで重複排除する。
- `turn.completed` とexit 0は相談ターンの成功のみ。受付依頼やOrca Taskをcompletedにしない。
  再開は正確なUUIDとruntime内session metadataを照合し、`--last`や新規sessionへのfallbackを使わない。
- 中断・破損JSON・異なるUUID・結果保存失敗はfail-closed。自分が起動した子processを停止・waitしてから
  slotを解放する。残ったstarting/running状態は結果不明として自動再開しない。
- fake providerで正常/失敗/重複/中断を検査後、実Codexの限定readonly相談と同UUID再開を受け入れる。
  この受入でゲーム変更・本番Task・commit/push/PRは行わない。

実装結果:

- `scripts/orca_coordinator.py` と対応testを追加し、受付へ相談/追記/状態表示/復旧照合を接続した。
  focused tests 58件成功。UUID欠落/不一致/重複、event順序、非ゼロexit、保存失敗、途中EOF、
  過大event、timeout、二重選択、branch変更、制御stateへのwrite拒否を含む。
- Orca terminal「開発受付・統括相談」で実Codex 2 turnを実行した。受付を一度終了・再起動した後、
  初回の合言葉を追記promptへ再注入せずに問い、同じsessionから正しく返答された。
  request `7fe669f9-745b-43b1-97e4-92ca8dd7cf03`、session `01a0bfa3-a8a9-7861-9d23-7177dd03dd5a`。
  両turnともexit 0 / completed / process_exited、相談中のsource変化なし。workerやTaskは未投入。
- 最終全群gate `python3 scripts/dev.py ci check --base d85dba0f17e21f96a385fdec9e2b660393692a5f --mode full` 成功。
  contracts / tooling / deps / rustを実行し、Clippy警告0を含む。local検証のためCI run URLはない。
  tested HEAD/base: `d85dba0f17e21f96a385fdec9e2b660393692a5f`、dirty source fingerprint:
  `3a0db59ddf5ec45b4d6e8564067c371677880a6317e919b6117cc8e51b4360b9`。
  primaryの文書・別作業のゲーム変更はこのcandidate gateの対象外として区別する。
- Help Skill判断はNo impact。変更は開発者用受付とread-only相談processに限り、static Help catalogへの
  注入経路・ゲーム入力・UI・runtime dataは不変。primaryの別作業はこの判定の対象外。
- 次はR1のdirty同task継続/固定reviewer拘束と、R3の限定Orca通信bridge・Task/Dispatch受入。
  今回の相談機能を並列実装運用の完了とは扱わない。

### R4: 実運用受入と既定基点への採用

- [ ] UIの受付から依頼し、A/B各1件の独立taskと固定reviewerを一巡させる。
- [ ] 統括を終了しても依頼が残り、同じ入口から再開できることを確認する。
- [ ] Cursor Bが担当外を編集できず、複雑taskがAへ戻ることを確認する。
- [ ] 同一対象の運用基盤検証、Help実レビュー、storage checkを通す（現行範囲は§7）。
- [ ] 必要なローカルcommit・基点更新の許可範囲を確認して採用する。push・PR・ゲーム変更は含めない。
- [ ] Quickstartを実際の受付操作へ置き換え、未検証事項を残して「運用可能」と報告しない。

第1/第2batchの比較基点は `d85dba0f17e21f96a385fdec9e2b660393692a5f`、
第3batchは `a03c4d5401f0486427aac81c0a91f335f79468bc`。
同じ専用branch/worktree/cacheを使い、文書正本はprimaryへ置く。コード編集は主担当が行う。
今後はL1から順に進め、L3の通信境界が成立する前に本番workerへ依頼を投入しない。

一次資料: [Cursor設定](https://cursor.com/docs/cli/reference/configuration)、
[Cursor権限](https://cursor.com/docs/cli/reference/permissions)、
[Cursor非対話実行](https://cursor.com/docs/cli/headless)、
[Codex CLI](https://developers.openai.com/codex/cli/reference/)、Orca導入版の同梱orchestration guide。
Orcaのcustom argv/reuseはguideのlow-level-topology条件に従い、裸のYOLO起動へfallbackしない。

### R1 / R2 第1実装batch（実装時点の記録、現在はcommit済み）

- 新規: `scripts/orca_providers.py`、`scripts/orca_frontdesk.py`、対応する3 test module。
  既存role launcher・host slot・7種類のagentルール・ルール検査を更新した。
- Cursor policyの必須field `version: 1` と `editor.vimMode: false` を明示する。
  不完全なglobal configはreadonly修復に失敗すると既定permissionへfallbackするため、配布版の
  実loaderをnetworkなし・readonly fixtureで検査する回帰testを追加した。
  不完全形式はdeny空/fallbackあり、現形式はShell/MCP/WebFetch deny維持/fallbackなしを実測。
  CLI main・認証flow・LLMを起動しない試験であり、実tool denyはR1に残す。
- 受付は排他、atomic replace、file/directory fsync、UUID再送の重複排除を実装。
  replace後のfsync失敗から同一ID再送で再同期し、壊れた台帳・未知状態を勝手に初期化しない。
- focused tests 37件成功。readonlyレビューで設定fallback、未導入CLI依存、fsync再試行を修正済み。
  最終全群gate（`python3 scripts/dev.py ci check --base d85dba0f17e21f96a385fdec9e2b660393692a5f --mode full`）も成功。
  contracts / tooling / deps / rustを全て実行し、Python239件対象、Blender検査、通常/profiling Rust tests、
  feature check、Clippy警告0を含む。local検証のためCI run URLはない。
  tested HEAD/base: `d85dba0f17e21f96a385fdec9e2b660393692a5f`、dirty source fingerprint:
  `1ea685e97eee9ee9909064817cdc1d051f1d6cee8db61bd6bfb4908b991874b4`。
- Help Skill判断はNo impact: 開発用provider/受付/ルールのみを変更し、game入力・UI・runtime dataは不変。
  `help_content::build_help_panel_content` → static manifest/provider → `interface::ui::plugins` の
  catalog注入経路に変更はない。primaryの別作業のproduction差分はこの判断・全群gateの対象外。
  primary docs index/link checkとstorage checkも成功。同じcandidate/cacheをR1〜R3継続用途で保持する。
- Orca terminal上でメニュー表示、`q`による終了、同じterminalでの再起動表示を確認。
  本番依頼・worker・reviewerは投入していない。統括起動/再開とTask bridgeは未実装であり、
  **受付台帳だけを「統括の仕組み化が完了した」と扱わない**。
- 既定baseは引き続き `d85dba0f...`。今回の未commit変更は新規treeへまだ継承されない。
  次はR1の同一task/session拘束とR2の統括起動/復旧、続いてR3のtask限定通信を実装する。

### 2026-09-20 再開時の実装batch

- 専用branch: `iaammssssstupiddddd-commits/orca-parallel-development`。
- code作業場: `/home/satotakumi/orca/workspaces/hell-workers/orca-parallel-development`。
- base: `b68bafd7358580ff8ad77962fa02987a215b5b60`（Orcaの既定`origin/master`）。文書正本はprimary。
- 初期化に`pnpm install`というlocal設定があるため、新規作業場は明示的にsetup skipで作成した。
- 変更順: host重実行枠 → role launcher/拒否試験 → 条件付きルール同期 → Orca設定と運用検証。
- workerのGit metadataはread-onlyとし、commit/統合は統括が直列で行う方針へ変更する。
  shared Git refsをworkerから安全に更新する設計を初回導入条件から外す。
- native/性能・並列効果測定は別の実タスクが必要。環境の成功だけでM5完了とはしない。

### 運用開始準備の採用

- ユーザーが専用branchへのローカルcommitとOrca基点設定を明示許可した。
- 検証済みの基盤24 fileのみを `d85dba0f17e21f96a385fdec9e2b660393692a5f` へcommitした。
  commit前のsource fingerprintは下記の全群成功対象と一致し、commit後のworktreeはclean。
- `repo set-base-ref`で専用branchを設定し、`repo show`でread-back確認した。
- primaryのゲーム変更はcommitせず、branch切替・push・PR作成もしない。
- 最初の独立taskで制御済みworker/reviewer起動を試行する入口を運用文書へ追加した。
  実agent受入やM5の効果測定を、このcommit操作の成功で代替しない。

## 1. 目的

- Orcaを既存ツールと共存させ、Hell Workersを登録して操作入口を用意する。
- 導入版の実際のCLI・設定schema・起動順に基づき、素案の未確定部分を具体化する。
- 統括1・実装2・専任reviewer1、初期の重い実行枠1という運用を、段階的に成立させる。
- 依頼・進捗はLinear標準連携へ集約し、実装済みの安全境界と会話再開を活かして独自受付の保守を減らす。
- 成功指標は、起動互換、範囲外writeと未審査統合の防止、資源競合の制御、統合受入までの時間改善。

## 2. スコープ

### 対象（In Scope）

- M0/M1: user-local導入、既存checkout登録、CLI/terminal smoke、固定版の一次仕様確認、計画・索引更新。
- 今回の拡張範囲: M2/M3のhost資源制御・role起動境界・条件付きルール、M4の安全な初期設定。
- 現在の拡張範囲: 冒頭L0〜L4のLinear受付・既存統括再利用・bridge修正・read-only監督運用の受入。

### 非対象（Out of Scope）

- ゲームの実装task、Orca本体のfork、公開・push・PR・無許可merge。環境の限定read-only agent試行はL3対象。
- 既存GNOME Orcaの削除/置換、全体PATHや既存agent認証の移行。
- 初期段階でのremote host / cloud / mobile、3名以上の実装、多段委譲。
- 独自Webhook/MCP server、受付台帳の常時双方向同期、ゲーム実装テスト、並列編集の効果測定。

## 3. 現状とギャップ

### 3.1 初回導入結果（実測、移行前の記録）

| 項目 | 結果 |
| --- | --- |
| 配布元 | [公式v1.4.205 release](https://github.com/stablyai/orca/releases/tag/v1.4.205)、2026-09-17公開 |
| 配布物 | `orca-linux.AppImage`、197,744,574 bytes |
| SHA-256 | `7bede254c95ad7237098890bbeca53e667a044997f0314bb67dbc4b932094bcf`。GitHub asset digestとdownload後の計算値が一致 |
| 保存先 | `/home/satotakumi/.local/opt/orca-ide/1.4.205/Orca.AppImage` |
| 実行形態 | AppImageを同directoryの`squashfs-root/`へ展開し、同梱実行fileを使用。FUSEを追加installしない |
| CLI | `/home/satotakumi/.local/bin/orca-ide` → 展開先`resources/bin/orca-ide`。独自CLI実装を作らず、公式launcherを利用 |
| desktop entry | `/home/satotakumi/.local/share/applications/stably-orca.desktop`、表示名`Orca IDE`。desktop-file-validate成功 |
| 共存 | `/usr/bin/orca`は既存の`orca-50.2-1.fc44.noarch`のまま。globalな`orca` aliasは追加しない |
| runtime | `status --json`: appVersion `1.4.205`、runtime/graph `ready`、desktopWindowStatus `available` |
| 登録repo | `/home/satotakumi/projects/hell-workers`、repo ID `88983a13-d1dd-47a0-8ffb-3772ea15d3f1` |
| worktree | primaryだけを登録。別checkout/branchは作成していない |
| terminal smoke | 専用terminalで`pwd`を実行し、primary pathを確認。read成功後にexact handleでclose、`ptyKilled: true`、残りterminal 0件 |
| agent | 起動0件。account import、Orca skill global install、並列実装は行っていない |

起動入口:

```bash
orca-ide --version
orca-ide open --json
orca-ide status --json
orca-ide repo show --repo path:/home/satotakumi/projects/hell-workers --json
```

desktopメニューの`Orca IDE`からも起動できるentryを設置した。
今回の初回起動はuserのtransient unit `orca-ide-desktop.service`を使用し、login時の自動起動は登録していない。
設定は`/home/satotakumi/.config/orca`、補助状態は`/home/satotakumi/.orca`に作成された。
Window利用可能というruntime応答とterminal APIを確認したが、全GUI操作・描画品質・native性能受入の証拠ではない。
Wayland起動logにはVulkanとの非互換警告があり、画面描画に問題が出る場合は別途切り分ける。起動成功だけで警告解消とはしない。

version付きdirectoryは再現の入口であり、自動更新を無効化した保証ではない。
runtimeはupdate supportを返すため、後続の各試行開始時にCLIとruntime両方のversionを照合する。
更新を適用する場合は別version directoryとlauncher切替を管理し、M1の互換確認をやり直す。

### 3.2 仕様確認の根拠と設計への反映

一次情報はmovingなwebsiteだけでなく、導入tagのsourceと同梱`agent-context --json`を用いる。
CLI schemaは今回234 commandsを返した。以下で「確認」は実機応答とsource読解を区別する。

| 確認内容 | 根拠 / 区分 | 計画への反映 |
| --- | --- | --- |
| LinuxのCLI名は`orca-ide`、Orca内terminalには`orca` shimがある | [install文書@v1.4.205](https://github.com/stablyai/orca/blob/v1.4.205/docs/site/content/docs/install.mdx)、外部CLIのversionは実行確認 | 外部の起動・検証commandは`orca-ide`へ統一。GNOME Orcaと衝突しない |
| repo登録時のsetupは`run-by-default`、agent開始は`start-immediately` | `repo add`実応答。sourceも[同既定値](https://github.com/stablyai/orca/blob/v1.4.205/src/shared/setup-agent-startup-policy.ts) | setup成功前のagent実行を防ぐ設定が必要 |
| `setupAgentStartupPolicy: wait-for-setup`はroot-level YAML key | [orca-yaml.ts](https://github.com/stablyai/orca/blob/v1.4.205/src/shared/orca-yaml.ts)、[型](https://github.com/stablyai/orca/blob/v1.4.205/src/shared/orca-yaml-hook-types.ts) | YAMLの階層を推測しない。M4で失敗・timeout時にagentが始まらないことを実証 |
| current/existing worktreeのworker-startはsetupを再実行しない。新規treeには`--setup run/skip/inherit`がある | 同梱CLI schema | setupを唯一の資源/権限guardにしない。毎回のrole launcherとdev.pyにも置く |
| 起動引数の既定はYOLO群 | [launch defaults](https://github.com/stablyai/orca/blob/v1.4.205/src/shared/tui-agent-launch-defaults.ts)、[permission args](https://github.com/stablyai/orca/blob/v1.4.205/src/shared/tui-agent-permissions.ts)。agent起動は未実施 | 現行版の既定にread-only保証はない。M3でrole別のeffective argv/envを検査し、専任reviewerのwriteを防ぐ |
| Runはnamespace/inboxで、自動配置・scheduleをしない。Task/Dispatch/deps/gate/messageがある | [orchestration文書@tag](https://github.com/stablyai/orca/blob/v1.4.205/docs/site/content/docs/cli/orchestration.mdx)、同梱schema | nativeのTask/DAG/通知を使用。独自の汎用schedulerを重ねない。host実行枠とcode承認はproject側 |
| `worker_done succeeded`はdispatchの完了。taskIdとdispatchIdを使う | 同文書 | 実装完了とreview承認を分け、後続integrationをgateで止める |
| `worker-start --terminal`は既存terminalを再利用でき、既存treeでは自動setupしない | 同梱schema | 単一reviewerの再利用候補。role境界・dispatch交代をM4で確認 |
| `worker-release`のexit 0にはretained/release_pendingも含む | 同梱schema | 終了コードだけで解放済みとしない。result・owner・子processを確認 |
| `orchestration run` / `run-stop`は廃止され効果なし。`reset`はruntime全体 | 同文書 | `run-create`を使い、回復に全体resetを使わない |
| sharedDirectoriesはignored directoryの共有で、read-only隔離ではない | tagのYAML sourceと[Worktrees@tag](https://github.com/stablyai/orca/blob/v1.4.205/docs/site/content/docs/model/worktrees.mdx) | target/save/settingsを共有しない。worktree登録と権限設定を別工程にする |

初回spec確認では、setup失敗時の実挙動、role別権限、worker lifecycle、worktree作成/削除は未実行だった。
移行後に専用worktree作成とroleのmount境界を実行確認した。実agent lifecycleとsetup異常系はM4へ残す。
Orca orchestrationは文書上Experimentalで、自動dispatchを今回の基盤実装へ含めない。

## 4. 実装方針（高レベル）

### 4.1 責務

- Linear: 依頼・人向け進捗・結果要約。開始snapshotと実行/承認証拠の正本にはしない。
- Orca: repo/worktree・terminal・Run/Task/Dispatch・message・decision gateの可視化と操作。
- project driver: host負荷制御、権限付きrole起動、担当path検査、検証対象の固定、primary storageへの接続。
- 統括: task分割、共有仕様の先行統合、primary文書更新、review queue管理、直列統合。
- reviewer: 同時に1名、source read-only、自己修正なし。承認はbase/head/tested SHAと指摘の解消へ紐づける。

Orcaの「Manual」は確認動作の設定であり、担当path制限やread-onlyと同義とは扱わない。
実装担当とreviewerを同じ無制限引数で開始しない。agentのsandbox/permission仕様は実装時にそのCLI版の一次情報で確定する。
許可済み範囲は自律実行できる設計とし、毎操作のユーザー承認を資源制御の代わりにしない。

### 4.2 資源と起動順

host namespaceはOS user databaseのaccount home配下`.local/state/hell-workers/coordination/`に固定した。
環境変数HOME/XDGで分岐させず、永続disk・owner権限・symlinkを確認し、不明なら開始しない。
最初はcompile/test/clippy/feedback等の重い処理を合計1件、Cargo内部jobsも1とし、nativeはrecipe全体を排他にする。
testのthread/processとrust-analyzerの暗黙buildも対象にし、Orca本体の常駐負荷も予算へ含める。

取得順はhost許可→実行slot→既存workspace activity lease→開始直前のRAM/disk確認→子process。
busy時は取得済みの実行資源を解放して統括queueへ戻し、sessionのlaneは同じ候補へ固定する。
frozen native helperはprimary coordinatorが外側から包み、旧subjectのsourceを変更しない。

**setup script内で`lane shell`を実行しても、そのleaseが後から起動する別PTYのagentへ渡るとは仮定しない。**
setupは短い準備・検査だけにし、session leaseを必要とする場合はagent自身をrole launcherの子として起動する。
既存tree、新規tree、再開、setup skipのすべてで実処理時のguardを必ず通す。

### 4.3 記録と承認

Linear issueと固定snapshotを依頼票へ、Orca Taskを分割taskへ、Dispatchをattemptへ対応させる。
LinearのDoneは承認証拠にせず、primary計画に残す内容はowner/path/base/head、
reviewer、必須指摘、必要検証群、検証対象、保持consumerに絞る。
`worker_done`後はreview taskへ渡し、未解決review gateがあるintegration taskを開始しない。
gateをresolveする統括はreviewerの判断と対象SHAを照合する。gateの存在だけで無審査commitを防げるとはしない。
統合後は同じreviewerが実際の統合headを再確認し、当該scopeの検証・Help/storageを完了して採用する。
ゲーム変更に必要なnative等の検証は別タスクの通常規則に従う。今回の検証範囲は§7を適用する。

### 4.4 作業場・branch・公開

installはrepository外、文書はprimaryに置く。別作業中の`codex/building-art-migration`を切り替えない。
Orcaが生成した専用branch/pathは冒頭の実装batchを正本とする。基盤はローカルcommit済み・primary未統合。
文書正本はprimaryを維持する。実worker用の分離treeはM4の初回task受入で登録・確認する。
本依頼にはpush/PR公開を含めず、CIを使わない場合は変更別のlocal gateを使う。
Bevy API変更は予定しない。後続のRust変更時にはBevy 0.19の既存実装/一次API確認と通常gateを適用する。

## 5. マイルストーン

以下M0〜M4は既存実装・検証の参照。現在の受入は冒頭L0〜L4で管理する。
M5のゲームleaf並列実装・効果測定は別途の編集委譲許可後の将来作業で、今回の完了条件から外す。

## M0: 導入と最小接続（今回完了）

- 変更: 公式AppImageの取得・digest照合・展開、CLI symlink、desktop entry、primary登録。
- 変更対象: §3.1のuser-local install先のみ。repositoryのruntime/driver変更なし。
- [x] CLI/runtime versionが1.4.205で一致する。
- [x] GNOME Orcaを保全し、desktop entryを検査する。
- [x] repo/worktreeのreadとterminal create/read/closeが成功する。
- [x] 確認terminalを終了し、agentを起動していない。

## M1: 仕様と計画の確定（今回の調査範囲を完了）

- 変更: tag付きsource・同梱CLIと実応答から§3.2を整理し、素案と索引を更新する。
- [x] setup並行既定、YAML key、既存treeのsetup省略、YOLO既定を特定する。
- [x] Run/Task/Dispatch/gateとレビュー承認の分担、terminal解放の判定を整理する。
- [x] install確認と未実施のworker/native受入を区別する。
- [ ] 文書batchの限定検査を完了する（現行§7。primaryの別作業のdirty変更を包括承認しない）。

## M2: host資源制御（候補実装・基本検証済み）

- 想定file: `scripts/build_coordination.py`、`cargo_runtime.py`、`dev.py`、`validation_storage.py`、対応tests。
- coordinator/各entry pointにhost leaseを追加し、既存workspace leaseと多段helperの継承を維持する。
- [x] 更新済みdriverで別tree/cloneでも同一slotを競合するtest、継承子がfdを保持するtestを追加した。
- [ ] native recipe中は通常処理・feedback・解析の暗黙buildが始まらない。
- [ ] 親異常終了・子残存・取消・lease偽装・低RAM/disk・旧helperで拒否/回復を試験する。
- [x] candidateの`check_agent_rules`、Help実レビュー、変更別全群gate、primary storage検査が成功する。

host lock、jobs/test threads=1、RAM/disk前検査、CLI override/alias制御、perf/validation fd継承、
MCP backendのhost参加とidle15秒を実装。偽装fd/子存続/RAM拒否はtest済み。
上記未check項目の全native/旧helper/異常終了matrixを完了したという意味ではない。
旧checkout/IDE解析は自動更新・停止しないため、採用時に所有sessionの調整が必要。

## M3: role・所有権・承認境界（候補実装済み）

- 想定file: 新規role launcherとtests（配置は`scripts/`）、関連agent規則・Skill・開発文書。
- 依頼票からcwd/branch/base/allowed pathsを検証し、workerの再委譲とprimary越境writeを防ぐ。
- reviewerはsource read-only、workerもGit read-only。commit/統合は統括へ限定する。
- [x] fixed argvとclean Codex homeを生成し、外側bubblewrapによるscope write境界を実行確認する。
- [x] symlink/hardlink ticket拒否、他scope/primary/docs/Gitへのwrite拒否をtestする。
- [x] review後のsource/base/head/index変更で承認が失効し、統括の追加修正もreview対象になる。
- [ ] 単一reviewerの役割継続・context喪失からの引継ぎ・不在時停止を確認する。
- [x] 条件付き委譲の新規則を7つのagent規則と`check_agent_rules`へ整合させる。

worker2・reviewer1・同一checkoutの排他を実装。reviewer履歴があれば明示resumeを要求する。
`verify-review`は統括作成記録の一致確認で、reviewer本人/CI証拠の真正性を自動証明しない。
ignored file/submodule内部はfingerprint対象外。任意network/CPUを封じる敵対的sandboxではない。
運用と制約は [orca-development.md](../development-infra/orca-development.md) を参照。

## M4: Orca連携の受入（read-only会話継続済み、Task連携は後続）

- 想定file: `orca.yaml`、Orca連携adapterとtests、role起動案内、文書。
- candidateの`orca.yaml`に`setupAgentStartupPolicy: wait-for-setup`と`python3 scripts/dev.py doctor`を定義した。
- live repoにも公式`repo.update` runtime APIで同じsetup/待機順を適用した。
  local-only、run-by-default、archive設定を維持し、設定fileの直接上書き・app再起動はしていない。
- [x] 許可された基盤だけをローカルcommitし、repoの既定baseを専用branchへ設定・read-back確認した。
- sharedDirectories/defaultTabsでtarget共有やagent自動増殖を起こさない。
- worker-startで裸のagentを直接起動してrole境界を迂回しない。制御済みterminalを作り、
  readinessとTask限定通信を確認後に `worker-start --terminal <handle>` で監督対象へ結び付ける。
  `dispatch --inject` はprocessを監督対象にしないため、readiness失敗の迂回には使わない。
  live preamble・context注入・terminal再利用の互換を実証してから採用する。
- [ ] setup成功/非0終了/timeout/skip、新規/既存treeで実処理guardが迂回されない。
- [ ] reviewer terminalを1つに固定し、連続した別taskのDispatchを混同しない。
- [ ] Runに属する全Deliveryを処理してackし、古いdispatchの完了で新taskが閉じない。
- [ ] release_pending/retained/unknownを解放済みと誤認しない。worktree/targetの保持は別管理する。
- [ ] 対象repoを明示し、UI作成・直接CLI・再開の各入口が同じ制御へ入る。
- [ ] 実機での起動・取消・再開とsource無改変を確認し、Orca更新時の再検査項目を固定する。

## M5: 実装2件の試行と定常化（未着手）

- 同程度の独立leaf修正を複数組選ぶ。共有型・save・renderer・資源制御自身の変更は初回対象から外す。
- reviewer待ち候補が2件なら新規実装を入れない。統括が1件ずつ統合し、実際の統合headで検証する。
- [ ] 単独運用のbaselineを測り、並列試行前に時間/費用/手戻りの判定値を固定する。
- [ ] 着手→受入時間、build/review待ち、再修正・統合回帰、peak RAM、disk増分、agent使用量を記録する。
- [ ] 未審査統合・範囲外write・資源逸脱0件、必須gate成功、合意した効果を確認する。
- [ ] 重い実行枠の2件化は別途peak/余裕を確認したときだけ試す。
- [ ] 採用結果をDEVELOPMENT/storage/解析運用仕様へ反映し、計画をclose/archiveする。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| setupだけに依存 | 既存tree/skip/別PTYから無制御実行 | 毎回のrole launcherと実処理driverの両方で確認 |
| YOLO既定の持込み | reviewerやworkerの範囲逸脱 | effective引数・権限を起動前に検査。単にManualへ切替えただけで合格にしない |
| 完了通知の早期承認 | 個別成功を統合成功へ誤転用 | task/dispatchとcommit/reviewを分けて結び付ける |
| reviewer/統合の詰まり | agentを増やしても総時間が悪化 | 1人1候補、待ち2件で投入停止、指標から並列数を調整 |
| 更新による仕様差 | CLI/schema/起動順が変わる | CLI/runtime版を毎回照合、tag一次資料を更新 |
| 他作業との競合 | 文書・cache・branchの破損 | primary正本の担当を調整し、共有branchを無断切替えしない |

## 7. 検証計画

ユーザーの「このタスクに関してはゲーム実装のテストは不要」を本計画のL0〜L4へ適用する。
Rust/Bevyのbuild・workspace test・Clippy、Blender test、ゲームwindow/GPU/native/performanceは実行しない。
過去の全群passは当時の履歴として残し、Linear連携や改訂後コードの証拠へ流用しない。
通常のゲーム開発の検証規則や共通CI分類を恒久的に変更するものではない。

| 段階 | 必要な確認 |
| --- | --- |
| L0 文書のみ | primaryのdocs/index/link、変更文書の整合レビュー、`git diff --check`、Help影響判断、validation storage check |
| L1 標準連携 | 許可した試験issueの読取り・明示更新・再読、worktree関連付け、未接続/権限不足時の拒否。ゲーム起動なし |
| L2 adapter | 変更したPythonのlint・focused tests、snapshot/重複/後編集/旧受付移行の拒否試験、相談/同会話再開 |
| L3 実行連携 | bridgeのJSON正規化回帰、既存role/provider/host境界の関連tests、実read-only Task一巡と失敗/再開の照合 |
| L4 切替 | 入口からの操作確認、旧依頼と会話の保全、二重開始拒否、文書・storage確認 |

各batchでbase/head・変更file・対象fingerprint・実施command・成功/未実施を区別する。
`ci check --mode auto` / `verify` が共通driver・運用文書やprimaryの別作業からゲーム群を選ぶ場合、
今回の明示スコープに反するためそのまま実行せず、対象を限定した上表の検査と未実行群を記録する。
限定検査を全CI成功と称さず、別作業のproduction差分を包括No impactで通さない。
実装内容がゲーム検証を必要とする範囲へ広がる場合は、先にscopeを再相談する。
ルール/Skillを変えたbatchでは既存の同期検査も必要。現在の編集委譲禁止を黙って解除しない。

### 検証データ管理

- 正本: [validation-storage-workflow.md](../development-infra/validation-storage-workflow.md)。後続のjobはprimary coordinatorへ登録する。
- 初回導入時のowner: Orca導入 / Codex。game/native/perf batch・新規worktree・Cargo target・binary copyは0件だった。
- 移行後は冒頭の専用worktreeとそのtargetを保持する。primary retain IDは`orca-development-environment`。
  owner: Orca environment / Codex main、consumer: `orca-environment-implementation`、
  next_action: L1の接続確認→L2の入力adapter、L3のbridge不具合修正・read-only受入（既存candidateを再利用）、
  release_when: 統括・制御済みlifecycle受入または明示終了、残る修正consumerなし、sourceを保全済み。
  初回登録時26,308,608 bytes、全体gate後27,606,331,392 bytes（約25.71 GiB、通常/profilingのbuild cacheを含む）。
  review-activeの同一targetを保持する。今回、既存のmaterialな成果物の削除はない。
- 採用成果: user-localのOrca本体/CLI/desktop entryと本計画。確認terminalは終了済み。一次sourceはremote readのみ。
- 導入directoryはvalidation jobでなく継続利用するアプリ。`du -sb`で753,748,541 bytes（installer＋展開済み本体、導入直後）。
  owner: satotakumi / consumer: Orca通常利用とM2〜M5 / next_action: 本計画の基盤実装 / release_when: 版の置換またはアンインストール。
- アプリ状態は計測時`.config/orca`が9,893,682 bytes、`.orca`が17,473 bytes。起動後に変動する。
  consumer: 登録repo・sessionの継続利用 / next_action: 通常利用 / release_when: 利用終了し保持する設定が不要と判断した時。
- installerは同一版の再展開用。新version採用後に旧版のconsumerを確認して整理する。今回は削除pathなし。
- 後続candidateは同じworktree/targetをreview期間中保持し、用途の終わったjobだけseal/finalizeする。

## 8. ロールバック方針

- Linear連携の問題では外部への新規更新・新規投入を止め、進行中processと未確定receiptを照合する。
  該当issueとの対応と会話は保全し、移行済み依頼を旧受付から自動再実行しない。
  未移行の旧依頼参照・相談は残す。認証解除やissue削除は対象とユーザーの許可を確認して行う。
- 導入だけを戻す場合、今回のOrca processと利用者を確認して終了し、CLI symlink・desktop entry・version directoryを個別に撤去する。
  GNOME `/usr/bin/orca`、既存agent設定、repository sourceは対象にしない。ユーザーの新規session/設定は保全する。
- M2以降で問題があれば新規worker投入を停止し、成果・稼働processを確認して単独編集へ戻す。
  cacheを一括削除せず、review holdを維持する。Orcaの全体resetで回復しない。
- 版変更は旧runtime停止・新version照合・M1/M4再検査を行う。前版へ戻す際の状態schema互換は別途確認する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 現行計画はL0完了。L2のsnapshot adapterとL3のP2/復旧/IPC修正はcandidate実装済みで、
  固定reviewer・Codex A・Cursor Bのread-only実Task一巡、Linear `TAK-5` のL1正常系、
  L2の実issue相談・同一session追記、L3EのA/B限定編集と2レーン並列実行を受入済み。
  権限/通信異常系、旧受付移行、受付からの自動一巡、L4切替は未受入。
- `a7c1bbb`で確認したP2（escalation JSON正規化で送信済みreceiptをunknown化）は、candidateで
  payloadの厳密なJSON内容比較へ修正した。ID/capability/他fieldの照合は維持している。
- M0/M1調査済み、M2/M3候補実装と拒否test済み、M4の3 role Task lifecycleとM5の限定編集並列試行を受入済み。
- 編集workerはA/Bを直列・並列の両方で起動し、exact fixtureだけの変更、統括検証、固定reviewer承認、統括commitを確認した。
  初期基盤では専任read-onlyレビューの指摘5件を修正し、再レビューを受けた。
  後日にP2を発見した時点では未修正だった事実と、この文書レビューはそれぞれ別の結果として扱う。
- Orcaをprimaryへ登録し起動中。user-local CLIは`orca-ide`、desktop名は`Orca IDE`。
- candidateのdriverはprimary未統合。監督付きOrcaだけを許す限定ルールはprimaryへ採用し、
  global agent既定権限は変更していない。実編集は専用launcher経由だけを許可する。
- 最新基盤は`55b27f6e1d9103d7985941c3cbbf135c7299be91`。受付の実Codex相談/同会話再開を確認済み。
  worker同task継続・固定reviewer拘束も実装済み。A/B・固定reviewerはread-only実TUIで各2turn受入済み。
  制限通信preflight、Codex用単一Dispatch bridge、Cursor hook bridge、Linear snapshot取込の模擬検証も追加済み。
  3 roleの実Task bridge接続、A/B実編集、Cursor Bのtool deny、固定reviewerによる編集review、2レーン並列実行を受入済み。
  primary文書の未commit変更は正本に保持し、専用branchのcode commitと区別する。

### 次のAIが最初にやること

1. 冒頭のcandidateとprimaryのdirty差分を区別し、下記の成功gateとsource fingerprintを確認する。既存成果を消さない。
2. CLI/runtime版、現行docs、storage状態を読む。採用前は旧driver/解析backendとの並行実行を調整する。
3. Linear `TAK-5` のL1正常系とL2相談継続は受入済み。権限不足・別team・通信失敗、後編集、新旧入口の二重開始を確認する。
   初期は標準連携だけを使い、旧受付の常時同期を実装しない。既存会話や台帳を改名せず、移行対象以外をuploadしない。
4. 3 roleのread-only lifecycleとL3E限定編集は受入済み。host限定認証・exact Task/Dispatch/terminal・旧bridge失効を維持し、
   Linear反映を含む受付からの一巡を確認してL4へ進む。CursorのShell/MCP/WebFetch denyを解除せず、
   共有file/APIを含む並列化や通常agent起動へ許可を拡大しない。

### 参照必須ファイル

- 本計画、関連提案、`docs/DEVELOPMENT.md`、`AGENTS.md`
- `docs/development-infra/validation-storage-workflow.md`、`docs/development-infra/rust-analyzer-mcp.md`
- `scripts/dev.py`、`scripts/build_coordination.py`、`scripts/build_lane.py`、`scripts/cargo_runtime.py`、`scripts/validation_storage.py`
- §3.2のtag付き一次資料と同梱CLI schema。

### L0文書改訂の確認（2026-09-21）

- 今回の変更はprimaryの未ステージ7文書だけ。README、DEVELOPMENT、docs索引、運用仕様、Quickstart、
  本計画、plans索引を同期した。比較元は作業開始時のindexで、既存のstagedゲーム/文書差分を保全する。
  primary HEADは `6abeeed92cf14e2e5aafc8240e1c64989b609a84`。
  candidateは `a7c1bbbcb9ff8e1ac3321f52780d655c18237d82` のclean状態を確認し、codeは変更していない。
- `python3 scripts/dev.py docs --write` / `docs --check`、`git diff --check`、
  primaryの `python3 scripts/dev.py validation check` はpass。plans/proposalsの両生成索引を確認した。
  読み取り専用の文書レビューで重大な指摘なし。旧R4参照を現行L3へ修正した。
- Help Skill判断はこの7文書に限定してNo impact。`build_help_panel_content` はstatic manifest/providerから
  UI pluginへ注入され、今回の計画・運用文書をゲームruntimeで読まない。入力/表示/成立条件は変更していない。
- `python3 scripts/check_help_impact.py` は既存のstaged production変更に対する判断不足でexit 1。
  同commandのdiff baseは `b68bafd7358580ff8ad77962fa02987a215b5b60`。別作業を覆うoverrideは与えていない。
  これは本改訂の文書検査成功と別の、primary全体の未解決gateである。
- ゲームbuild/test・Clippy・Blender・native、包括 `ci check` / `verify` は今回実行していない。
  限定文書検査だけを報告し、全CI成功や新しいTask/Linear接続受入とは称さない。
  新規worktree/target/binary/job、外部設定・issue更新、commit/push/PR、既存成果やcacheの削除はない。

### L2 adapter・L3 P2候補実装の確認（2026-09-21）

- candidate `a7c1bbbcb9ff8e1ac3321f52780d655c18237d82` を基点に、Linear読取りadapter、
  snapshot対応表、frontdeskのLinear優先menu、`linear-intake-state`排他、P2のJSON内容比較を実装した。
  code/test 8 fileを `6210b885b429341b5a9101c77332d57af008b0d8` へローカルcommitした。
- adapterは公式CLIをshellなし・明示workspaceで1回読むだけで、Linearへ書き込まない。
  未信頼本文を固定ポリシー付きJSONへ包み、workspace/issue内部ID/snapshot SHA-256から受付UUIDを決める。
  不完全context、別workspace、巨大/異常JSON、台帳不整合は保存前に拒否する。
- 実runtimeの `orca linear team list --json` は接続team 0件。実CLIの未接続応答を受けて
  `Linear is not connected in Orca settings` と停止し、`linear-intake/snapshots.json` を作らないことを確認した。
  外部issueの作成・更新、token入力、agent/Task起動は行っていない。
- その後の2026-09-21確認では `orca linear team list --workspace all --json` がworkspace `takumi sato`
  （`68abc67b-ca1b-407b-be63-99dd91321b26`）/ team `TAK`を1件返した。認証値は表示していない。
  続くL1/L2正常系の実受入は下記に記録する。
- `python3 -m unittest discover -s scripts/tests -p 'test_orca*.py'` は136件成功。
  `python3 scripts/dev.py lint`、candidateのHelp impact（No production changes）、
  primaryのdocs索引/link・storage・diff検査も成功した。テストとstorage `du` の並列実行では
  消滅中の一時directoryによりstorage検査が1回停止したため、テスト終了後に単独再実行してpassを確認した。
- ゲームbuild/test・Clippy・Blender・native、包括 `ci check` / `verify` はユーザー指定どおり実行していない。
  Orca UIの既存受付terminalは正常終了後に同じterminalで再起動し、`1: Linear課題を受付` と
  `7: 手入力fallback` の新メニュー表示を確認した。tab名は「Linear受付・統括相談」へ更新し、
  既存依頼・会話台帳は変更していない。

### L1/L2実Linear受入（2026-09-21）

- 既存オンボーディングissue 4件を変更せず、team `TAK` に専用試験issue [`TAK-5`](https://linear.app/takumi-sato/issue/TAK-5/orca-integration-acceptance-hell-workers)
  を作成した。許可範囲を同issueの説明・コメント更新・再読取りと既存`orca-parallel-development` worktree関連付けに限定した。
- 受入コメント `17dc69c4-ac41-4b49-97fb-f18031d0348a` を1回追加し、`linear issue TAK-5 --full`で説明、
  comment 1件、workspace/team/内部issue IDを再読取りした。worktree metadataも`linkedLinearIssue: TAK-5`を返した。
  worktree固有contextの`linear issue --current --full`でも同じissue/worktree IDを解決した。
  自動agent起動・dispatch・status同期は有効化していない。
- 固定snapshot SHA-256 `5ece754b065de0771944c9e61d938931c9f0dd8201286b66d77736b2d8d16e0f`を
  受付ID `0e283873-4e4e-5a96-b4fd-ba103a724f44` としてqueued保存した。Run/Task/Dispatch IDはすべてnull。
- 初回turn `32f365ab-9a0b-588f-8f4b-945a1fdb38b9`と追記turn
  `3a0a489c-ec28-431e-9a9e-94e63a1f534f`は同じcoordinator session
  `01a0c090-3de7-7923-8094-e78db4e3913d`を使い、HEAD `1673c6be` / source fingerprint
  `c227c45adf25aac42c51598fc2b3d6d1d9a5fa50df45a016fc513145f1c2d131`でexit 0・source不変だった。
- 同じsnapshotの再取込は同じ受付IDを返し、frontdesk上の該当受付は1件のまま。同じ追記turn UUID/本文の再送は
  保存済み回答を返し、Codexを再起動しなかった。
- `TAK-5`の説明更新時に`linear_write_unconfirmed`となったため再送せず、full再読取りで反映済みを確定した。
  後編集後のsnapshot `26c213cac8f8ab1ca50275a586cae09fe9287d061c3825457e9d9fb756376c47`は、旧受付/相談を上書きせず
  新受付ID `28971b6b-f77c-5a61-9d7e-3f8dc1cbf00c`としてqueued保存した。
- これはL1/L2の正常系だけを受け入れる。API tokenの最小権限確認、権限不足・別team・通信失敗、issue後編集、
  旧入口との二重開始・復旧、受付からTaskへの自動接続は未受入。

### L3固定reviewer実Task受入（2026-09-21）

- 最初のworker-aと固定reviewer、およびtask-private wrapper追加後の固定reviewerは、実agent内の最初の
  `orca orchestration check`で`runtime_unavailable`となった。全attemptでsource読取り・編集・上流mutation・
  `worker_done`は0件。exact Dispatchをabandonし、launcher終了・source不変・mutation-free journal・
  capability失効・実runtime identityを照合する`reconcile-bridge`で失敗事実だけを記録した。
- `bdf0ea8bdab1e39ff14605aa10b5debfc4947714`でtask-private CLI wrapper、terminal UUIDと
  composite endpoint incarnationの照合、限定復旧を実装した。裸のCLI取り違えを排除しても失敗したため、
  Codex内側sandboxがUnix IPCを遮断する経路と切り分けた。
- Task bridge付きread-only Codexだけ、Codex内側sandboxを無効にして既存の外側bubblewrapを強制境界にする修正を
  `1673c6be9d737fc10228ae635557f95c7b687547`へcommitした。Cursor、編集role、通常起動は対象外。
  外側のroot read-only、private state mask、role runtimeとTask専用proxyだけの再公開は維持する。
- candidateでOrca系139件、`python3 scripts/dev.py lint`、Help No impact、`git diff --check`が成功した。
  ゲームbuild/test・Clippy・Blender・native、包括`ci check`/`verify`はユーザー指定どおり実行していない。
- 実Orca Run `run_d7dd7dbe187e`、Task `task_3193fbe0521b`、Dispatch `ctx_21cdd586d0a4`で、
  固定reviewer session `01a0bfc7-0a67-7c33-a55f-b215b411c3de`がHEAD `1673c6be`をread-only確認した。
  check成功、指定source読取り、5条件の行番号証拠、`worker_done` succeeded、Task/Dispatch completed、
  capability失効、coordinator Delivery ACK、exit 0、同一session記録を確認。変更fileは0件。
- この成功は当時の固定reviewer限定read-only lifecycleだけを受け入れた。Linear L1/L2正常系は別途受入済みだが、
  この時点では受付からの自動接続、Cursor B、編集・検証・review一巡、並列効果測定を未受入として残した。

### L3 Codex A実Task受入（2026-09-21）

- 初回Codex Aではheartbeat自体は上流へ反映されたが、Orca 1.4.205が未指定`body`をreceiptで空文字へ補い、
  bridgeが送信後unknownで停止した。request IDを再送せず、Dispatch abandon・capability失効・exact terminal終了・
  launcher PID消滅を確認して、曖昧operationをTask結果未受入のままrole stateへ記録した。
- `fca4fd43bda7696246be481039af4d6469f9a4c5`でOrca既定の空本文だけを未指定へ正規化し、fenced Dispatchに残る
  pending mutation IDと外部terminal終了を証跡化する限定復旧を追加した。callerの空本文、ID/内容/capability不一致は許可しない。
  `422a74d6`でtestのrole-state保存先もfixtureへ隔離した。
- 再試行Run `run_d729db30c9bc` / Task `task_6e92670948b1` / Dispatch `ctx_c9d5d4d79ab9` で、
  heartbeat、10秒ask timeout、同じmessage IDのresume、確認語応答、escalation、指定2 fileのread-only確認、最終check、
  `worker_done` succeeded、Task/Dispatch completed、capability失効、exit 0、同一source fingerprint、変更0件を確認した。
- Orca系140件はpassし、test前後の実worker-a role-state SHA-256一致も確認した。ゲームbuild/test・nativeは非対象。

### L3 Cursor B実Task受入（2026-09-21）

- Cursor CLIへShell/MCP/WebFetch権限を追加せず、launcher所有のprivate Unix socketへ
  `beforeSubmitPrompt` / `afterAgentResponse` / `stop` hookから接続するbridgeを`1c11b068`で追加した。
  未知hook metadataの除去、Cursor固有の失敗照合、履歴作成前失敗の`session_absent`記録を`71a646ae`で追加した。
- 実attemptで確認した`stop`と`afterAgentResponse`の競合を`b81480d3`で直列化し、失敗stageを
  `2cf9d820` / `4b1aa3a2`で安全に診断可能にした。bootstrap応答がlive Dispatchへ混入する経路は
  `656f0ee3`で`generation_id`へ拘束して拒否した。
- CursorがOrca preambleを説明文として扱って結果を非JSONで返した場合に限り、上流mutation前にcontroller固定の
  整形依頼を1回だけ返す。任意follow-upや2回目の不正結果は拒否する実装を`29b9cb51`へcommitした。
  各失敗attemptはDispatch abandon、capability失効、exact terminal/launcher終了、source不変を照合してから
  bridgeをreconcileした。元要求や上流mutationを推測で再送していない。
- 成功Run `run_5d892b17c872` / Task `task_863d3336d44f` / Dispatch `ctx_743acfd14a79`で、
  heartbeat、check、`worker_done` succeeded、settlement、capability失効、coordinator Delivery
  `delivery_02aace26e50b`のACK、exit 0を一巡した。source fingerprintは前後とも
  `11c044725736bf37bc7d3df7d25329daa594f35e57357cba489f956d349dd67d`で、変更fileは0件だった。
- hookのGit実行属性不足を変更範囲gateで検出し、`09642de4022577fd442c4c9971c64a4d1f649e26`で修正した。
  同clean HEADでOrca系147件のfocused検査と、変更範囲判定のcontracts/tooling（Python 361件、
  Blender tooling 151件、Ruff、repository hygiene、Help No impact）、`git diff --check`がpassした。
  Rust/Bevyのゲームbuild/test・Clippy・nativeはユーザー指定どおり非対象。
- これは当時のCursor B限定read-only Task lifecycle受入であり、この時点では実編集の許可に読み替えなかった。
  後続の編集時Write境界、A/B並列編集、固定reviewerを含む受入結果は次節へ記録する。

### L3E監督付き実編集・並列受入（2026-09-21）

- candidateへCodex二重sandbox回避、review時のtracked project設定可視化、fresh `CODEX_HOME`でも妥当な
  MCP無効化、source不変の初回失敗を閉じる限定復旧を順に追加した。採用commitは
  `3d6e2032cac5e73fb07c6f07d094772216a490ef`、`e5ff06dc8a4a15c34bbd682d5f1a54a38a6981e1`、
  `8ddad3ce6e6d7926e1c5362e02f1b82eb1e6df7e`、`55b27f6e1d9103d7985941c3cbbf135c7299be91`。
  Codex内側sandboxを無効化しても、外側bubblewrapのexact write scope、private state mask、Git metadata read-onlyは維持する。
- 直列受入ではCodex Aが`worker-a/result.py`だけを`CODEX_A_EDIT_ACCEPTED`へ、Cursor Bが
  `worker-b/result.py`だけを`CURSOR_B_EDIT_ACCEPTED`へ変更した。Cursor BはShell/MCP/WebFetchを使わず、
  固定reviewer session `01a0bfc7-0a67-7c33-a55f-b215b411c3de`が各差分と検証証拠を承認した。
  統括commitはA `1a0ec765`、B `b4501cdf`。
- 最終並列受入では別worktreeで同時に起動した。Codex A session
  `01a0c478-2f25-7bc2-bda8-c368aca986c5`、source fingerprint
  `c5d2a5aa95432a221084e3a4be6ede025dfe485be750a34f566c85859b87b11c`はA fixtureだけを
  `CODEX_A_PARALLEL_ACCEPTED`へ変更した。Cursor B session
  `4b8caf79-260d-498d-a63f-54def4b43ed8`、source fingerprint
  `796472fcb70b86080ed4d0ba842d35cbefd9a211386b69e8c3f25592948b1923`はB fixtureだけを
  `CURSOR_B_PARALLEL_ACCEPTED_2`へ変更した。固定reviewerを直列再開して両方を承認し、統括がA
  `f66a9a55aa4ed9f0a71f6bc57e8aadfc8b2c5cd0`、B `3711ae70c0da88c2c42b153298bdeb5face0b6ae`へcommitした。
- 初期のCodex二重sandbox、reviewerからtracked dot-directoryを隠したことによる偽差分、fresh configでの
  MCP transport不足は、いずれも成功へ読み替えなかった。source不変とprocess終了を照合し、失敗attemptを閉じ、
  修正後は同じ受入条件を再実行した。reviewerからの実指摘修正は不要だったため同一worker再開は発生していないが、
  source/index/session不一致拒否とapproval後変更による失効は自動testで維持した。
- 最終candidateではOrca focused 151件、`python3 scripts/dev.py lint`、repository hygiene、Help No impact、
  `git diff --check`がpassした。変更別gateはcontracts/toolingを選択し、Python 365件、Blender tooling 151件、
  Ruff/actionlint/docs/hygiene/help/perf self-testがpass。source fingerprintは
  `c4ba881001b7df3acbb0d388ce69ad36dd394dfc5f7956131b5bb8d49d65ee1f`。
  candidateではユーザー指定によりRust/Bevyゲームtest、native/GPU/performance受入を選択していない。
- primary文書commit `575a0db4d3e2d66401fa46d0271f36a2c38072c9` の変更範囲gateはcontrol文書を理由に
  contracts/tooling/deps/rustを自動選択し、全群passした。source fingerprintは
  `754c572178acbca3bf03b317c25f6ff9aa64edbe99f085fed66695358f9435ed`。これは文書commitの自動gateであり、
  ゲーム実装変更やnative/window/GPU受入を追加したものではない。

### 初回導入時の確認ログ（移行後gateとは別）

- 2026-09-20: digest一致、CLI/runtime 1.4.205一致、repo read、terminal create/read/close、desktop-file-validateはpass。
- Rust check / Clippy / workspace test: 今回はRust変更なし、未実行。
- Help実レビュー: 今回の導入はユーザー領域の開発アプリ、repository変更は文書のみ。
  Helpはroot providerから静的生成され、docsやOrca設定をruntimeで読まないため、このscopeはNo impact。
  別作業のdirty production変更へ包括overrideは出さない。
- ゲームnative受入: 非対象。OrcaのWindow利用可能応答とterminal smokeまで確認、GUI全操作/描画性能は未検証。
- docs/index: `python3 scripts/dev.py docs --check` pass。primary storage: `python3 scripts/dev.py validation check` pass。
- 変更別gate: `python3 scripts/dev.py ci check --base 6abeeed92cf14e2e5aafc8240e1c64989b609a84 --mode auto`
  はcontracts/tooling/rustを選択。storageとAI規則はpassし、別作業のdirty production変更に対するHelp判断未更新でcontracts内に停止。
  比較baseとHEADは上記SHA、dirty scopeには既存Rust/toolingと今回の文書を含む。完走していないため成功fingerprint/CI URLはない。
  tooling/rustは未実行で、workspace全体の成功を主張しない。別作業の包括No impact overrideは出していない。

### 移行後の環境実装検証

- commit後にも同じ変更別commandを再実行し、**contracts/tooling/deps/rust全群pass、exit 0**を確認した。
  base: `b68bafd7358580ff8ad77962fa02987a215b5b60`、
  head/tested SHA: `d85dba0f17e21f96a385fdec9e2b660393692a5f`、clean worktree。
  source fingerprint: `169077f99b4bfae18b2b16df28a27ec6afec24099a7152087161ee629e5d14f2`。
  local検証（CI URLなし）、Help No impact、primary docs/index/storage検査もpass。
  以下の未commit時記録とは区別し、以後はこちらを採用commitの検証証拠とする。
- base/head: `b68bafd7358580ff8ad77962fa02987a215b5b60` + candidate dirty変更。
- `python3 scripts/dev.py ci check --base b68bafd7358580ff8ad77962fa02987a215b5b60 --mode auto`:
  **contracts/tooling/deps/rust全群pass、exit 0**（2026-09-20、local、CI URLなし）。
  workspace check、profiling/通常版workspace test、memory/tracy/renderdoc feature check、Clippy警告0はpass。
  source fingerprint: `d5e62534064eb84e2795b87d09bcc8782a5e5ddf67aafaea482d3c4aeebbfaf6`。
  検証前後のHEAD/index/source一致をdriverが確認した。Rust sourceは今回変更していない。
  依存監査には既存の重複crateとyanked `spin 0.10.0`のwarningがあるがpolicy上pass。依存更新は今回含めない。
- bubblewrap内Codex version/DNS、scope write拒否、source fingerprint/承認失効のtestはpass。
- 最終整理で未使用定数を除去し、旧gateは中断して再実行。RAM回復により実行されたMCP integration testで
  fixtureが親Git rootへ解決される問題を検出し、fixture自身をgit initする修正とroot一致assertを追加した。
  修正後Python218件・Blender151件pass（MCP integrationのskipなし）、追加read-onlyレビューもblockingなし。
- Help実レビュー: 開発driver/ルールだけでgame入力・状態・UI/runtime dataと静的Help生成経路は不変、No impact。
- live repoのsetup/待機設定は更新済み。GUI accessibilityからOrca windowが得られなかったため、
  固定tagのschema/handler確認後に公式runtime APIを使用した。再起動・直接config上書きなし。
- Orca内の専用shellから`python3 scripts/dev.py doctor`を実行しreadyを確認、確認用terminalだけcloseし
  `ptyKilled: true`を確認した。既存のユーザーsessionは変更していない。
- この移行後gate時点では実LLM並列試行/ゲームnative受入/公開は未実施だった。後続L3Eの並列受入は上記に別記した。
  primary別作業のdirty差分は今回の検証対象外。
- primary文書はdocs/indexとdiff hygieneを確認する。primary全体のdirty production変更へ、candidateの成功証拠を転用しない。

### Definition of Done

- [x] 今回依頼の導入と固定版仕様確認ができ、未検証事項を計画へ移した。
- [x] Linear受付への再計画、既存資産の採否、ゲーム検証を除いた受入範囲を文書化した（L0）。
- [ ] L1〜L4の接続・統括再利用・read-only監督・入口切替を完了した。
- [x] L3EのA/B限定編集、統括検証、固定reviewer、別worktreeの2レーン並列実行を受け入れた。
- [x] 改訂後の同一対象で§7の限定検証とHelp/storage確認が成功し、受入済み限定編集と未受入の自動運用を区別した。
- [x] 旧環境candidate採用時には全必要群・Help No impact・primary storageを確認した（改訂後のL1〜L4受入とは別）。
- [ ] 最終closeで本計画の検証consumerを解消し、継続利用アプリは通常運用へ引き継いだ。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-20 | Codex | v1.4.205導入、runtime/terminal smoke、tag sourceと同梱CLIに基づく初版。後続基盤・連携・試行を具体化 |
| 2026-09-20 | Codex | ユーザーのルール見直し承認を受けM2/M3候補実装、専任read-onlyレビュー修正、M4初期設定・運用文書を追加 |
| 2026-09-20 | Codex | 明示許可に基づく基盤のみのローカルcommitとOrca新規tree基点設定、初回試行の開始入口を整備 |
| 2026-09-21 | Codex | A/B・固定reviewerのread-only実CLI起動/再開を受入。Cursor設定保存不具合と限定失敗照合を実装し、Task限定通信/readinessを次段階へ具体化 |
| 2026-09-21 | Codex | R3第1batchのread-only制限通信preflight・負試験・実runtime通信確認を追加。readiness detectorの条件を調査し、Task lifecycle中継とは分離 |
| 2026-09-21 | Codex | R3第2batchのCodex限定単一Dispatch bridgeを実装・検証。Cursor B未接続、実Task未受入を維持し、ACK/質問再開/receipt/終了時の照合境界を具体化 |
| 2026-09-21 | Codex | Linear標準受付へL0〜L4を再計画。既存guard/sessionを再利用し、旧受付移行・snapshot・更新権限・P2修正・read-only受入を具体化。今回のゲーム実装テストを非対象に限定 |
| 2026-09-21 | Codex | L2の固定snapshot adapter・Linear優先受付UI・専用排他とL3のP2 JSON内容比較をcandidateへ実装。Orca系136件と限定gateを通し、実Linear接続・実Taskを未受入として維持 |
| 2026-09-21 | Codex | L3実Taskのruntime_unavailableをfail-closedで照合。task-private wrapper、exact process/限定復旧、外側bubblewrapへ一本化したCodex IPC修正を2commitし、固定reviewerのread-only実Task一巡を受入 |
| 2026-09-21 | Codex | Linear専用試験issue `TAK-5` を作成し、限定コメント更新・再読・worktree関連付け（L1正常系）と固定snapshotからの初回統括相談・同一session追記（L2正常系）を受入 |
| 2026-09-21 | Codex | Codex Aの空本文receipt差を修正し、曖昧heartbeatをfenced復旧。heartbeat・質問resume・回答・escalation・source確認・worker_done・role終了のread-only lifecycleを実Taskで受入 |
| 2026-09-21 | Codex | Cursor Bをprivate hook bridgeへ接続し、同時hook直列化・generation拘束・一回限りの結果整形retryを実装。Shell/MCP/WebFetch denyとsource不変を維持してread-only lifecycleを実Taskで受入 |
| 2026-09-21 | Codex | 変更範囲gateでCursor hookのGit実行属性不足を検出・修正。clean HEADでcontracts/tooling全群を再実行し、Python 361件・Blender tooling 151件を含めてpass |
| 2026-09-21 | Codex | primaryの全面的な編集委譲禁止を、別worktree・固定ticket・mount境界・固定reviewer・統括所有の検証/commitを満たす監督付きOrcaだけの限定例外へ改訂。L3Eの実編集受入完了まではread-only運用を維持 |
| 2026-09-21 | Codex | L3E用のA/B非ゲームfixtureを分離し、Cursor Bの編集受入taskを専用fixture exact scopeへ限定。Orca系148件とcontracts/tooling全群をclean HEADでpass |
| 2026-09-21 | Codex | L3EのA/B直列編集、固定reviewer、統括検証/commitと、別worktreeのA/B同時編集を受入。candidate `55b27f6e`、A `f66a9a55`、B `3711ae70`をcleanに確定 |
