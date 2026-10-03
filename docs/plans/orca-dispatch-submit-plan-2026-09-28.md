# Orca配車の着手確認とモデル通知競合の是正

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | orca-dispatch-submit-plan-2026-09-28 |
| ステータス | In Progress |
| 作成日・最終更新日 | 2026-09-28 |
| 作成者 | Codex |
| 関連Issue | TAK-14（既存依頼を維持） |

## 1. 目的

配車入力の受理を実装着手と誤認せず、対話通知に入力を奪われた場合も証拠を照合して復旧する。
成功条件は同一依頼・Run・会話を維持した実作業開始の確認。設定変更だけでは完了にしない。

## 2. スコープ

監督付きCodex起動、入力状態の照合、既存復旧経路の適用、関連tooling検証。
ゲーム変更、ゲーム実機テスト、モデルの独断変更、台帳の手編集、盲目的再送、公開は対象外。

## 3. 現状とギャップ

実装Aはinput_acceptedだがbootstrapの完了後にDispatch本文も新しいturnもない。
同時刻にモデル切替設定イベントがあり、通知への入力競合が疑われる。
統括は監視担当の復旧待ち。implementing投影は着手証拠ではない。

## 4. 実装方針

公式設定notice.hide_rate_limit_model_nudge=trueを監督付き新規・再開起動へ渡す。
権限警告や利用上限を無効化せず、選択済みモデルを維持する。
既存の入力、provider履歴、bridge、terminal incarnationを照合し、終了の正の証拠なく再配車しない。
同目的のorca-abcd-expansion branchを再利用。比較基点ae5b2066c4fd39a8dd6a6c439e272d125f749241。
既存差分は保全し、push・PRは行わない。Bevy変更なし。

## 5. マイルストーン

- M1: 通知抑止を統括・実装A・固定レビューの新規／再開起動へ追加し回帰検証。
- M2: 既存停止attemptを正規復旧経路で再開。元Task・会話・source・履歴を保持。
- M3: 入力受理と着手の証拠を分離し、実際のtool作業確認後に復旧を報告。
- M4: 再開履歴の旧bridge誤用を防ぐ現起動ORCA_CLI_COMMAND固定と作業前heartbeat契約。
- M5: 固定レビューの実装＋文書混在指摘を、統括文書補正→同担当実装→複合subject検証・固定レビューへ直列接続。
  sealed finding全件とrepair mapを保持し、workerの文書権限を拡張しない。
  文書subset提出、既存承認流用、未知のcheckout再送を拒否する。
- M6: Cargo用一時環境が単体testへ流入する経路を分離し、失敗確定済み文書検証を
  exact journal・candidate・所有照合と修正済みhost executorで検証のみ再開できるようにする。
- M7: 文書検証のscheduler admissionを単一callerの待機にし、再送loopと開始の競合をなくす。
  検証attemptの開始identity・終了receiptを永続化し、中断後も同attemptを照合できる経路を整備する。
  既存のidentity未記録attemptは成功や未実行と推測せず、終了の正の証拠を取得してから回収する。

## 6. リスクと対策

M8（継続実装の配車前bootstrap回収）を追加する。結果nullをreadyにはしない。
既存の生launcher・未arm bridge・source・会話・terminal incarnation・全Run履歴を照合し、
準備応答を正常に確認してから、送信前journalを持つ単一のworker-startへ進む。
新規Runやterminalは作らず、unknown送信を再実行しない。受理後の投影・armは
保存済み結果を照合して再開可能にし、実装担当の新turnと実作業で受け入れる。
関連fixture、拒否系、保存境界、Help/storage、固定read-only reviewを適用前に実施する。

M9（複合レビュー範囲の継承）: 統括文書commit後のworker checkpointでも、世代receiptの鎖と
現在の文書blobを照合して、レビュー対象へ正規の文書を引き継ぐ。worker write scopeは変更しない。
停止済みの旧reviewはsealed findings・全ACK/終了・同source/検証を検査し、旧ticketと指摘を保全して
修正済みticketの新しい固定reviewへ遷移する。空の文書変更・commit・承認流用で解消しない。

M10（Help判断登録のscheduler admission）: 長いtickの承認照合と短時間の登録retryが
競合し続ける経路を、単一callerのscheduler解放待ちへ変更する。lock取得後に保存済み
loop/source/所有/Help対象を再照合し、待機中の変更や中断は無書込で拒否する。
role/workspace/heavyのfail-fastは維持。関連test・tooling・固定review後、同統括の正式登録と
次工程の実動作を確認する。game検証は監視側から起動しない。

入力不在だけから終了と推測しない。通知抑止だけで送信保証とは扱わない。
未知送信を再送しない。provider終了と既存復旧guardを照合する。

M11（レビュー形式訂正の分離）: upstream未送信の形式不備を2回で通信失効へ昇格させない。
同一Dispatchで訂正できる応答を維持し、実改行とJSONをshellを介さないargvで渡す方法を明示する。
対象・権限不一致、未知送信、一般のrequest budgetは既存の拒否を維持する。
既に失効した実レビューは終了の正の証拠を取得し、既存resume-review-bridgeで同Task・会話へ戻す。
旧端末の承認文を正式承認へ転記しない。関連tooling・固定レビュー・Help/storage後に適用し、
新しいreview settlementと同統括の新turn内の具体的操作を受入条件とする。

## 7. 検証計画・データ管理

M13（編集前のscope不足再計画）: 単一Aが初回編集前に正式failed settlementした場合、
同request/node/Run/Task/session/tab/base/sourceを維持し、統括が不足directoryと理由を指定して
ticket世代を改訂する。元受入条件・validation・仕様は変更しない。全終了・release・ACK・
clean source・利用者control・tab incarnationを照合し、旧ticket/plan/失敗をwrite-ahead記録に保存。
generation admissionとdispatcherは同receiptを再検査し、元Taskへのretry-ofだけを許す。
未確定操作・dirty source・別所有・保護path・受入条件変更は拒否。保存中断・重複提出・
別停止の拒否をtoolingで検証し、固定レビュー後に同統括の次工程操作と担当の実着手を確認する。

M12（終了済み工程の作業場寿命）: 最終統合承認・全settlement/ACK・UI終了記録が成立した工程は、
統合先のexact receipt/sourceと固定最終reviewを検査し、撤去済みworker checkoutを再検査しない。
未統合・未終了のlane承認検査は維持する。旧tickで無効化された工程は、停止前の状態をUI終了時の
subject digestから完全照合できる場合だけ復元し、停止状態と復旧前後hashを保存する。
不足証拠・別停止・source変更・利用者停止は拒否。新Run、再検証、承認捏造、作業場再作成は行わない。
関連拒否系test・tooling・固定review・Help/storage後、同統括の通常再開と後継工程の実操作で受け入れる。

起動argv単体試験、変更別CI、Help no-impact実レビュー、primary validation check。
ゲームテスト不要。専用job・binary copyは作らない。既存candidateとreview-active cacheを保持。
consumerは本復旧、ownerは統括／監視main、release_whenは復旧受入とフィードバック終了。

## 8. ロールバック

今回追加した通知設定だけを差分照合して撤去できる。履歴・台帳・成果物は戻さない。

## 9. AI引継ぎメモ

M1は実装・固定read-onlyレビュー承認済み。新規／再開の48 tests成功。
変更別gateはcontracts/tooling、1128+164 tests成功、exit 0。
source fingerprint: 757e6b5d5ce0bbcbd4e079c895ab36ed001b7ef6342adbf461d2ce43bb407b44。
Helpは開発ツールの起動引数のみでplayer入力・asset・Help本文へ到達しないためNo impact。
primary storage check pass（152 batches、486367109120 bytes、削除なし）。
M2は正常終了exit 0、空bridge operations、同じsourceを照合して旧Dispatchをabandonし、
resume-bridge --inspected-source-sha256の既存guardで同Task・会話を再開待ちへ戻した。
統括も同会話で通知抑止付きlauncherへ正常再開。
08:32 UTCに同Taskのretry Dispatch ctx_0a07fb77b08aが同タブ・同会話で開始。
bootstrap後の別turnで依頼本文への応答と調査tool実行を確認した。
復旧後のモデルは通知事故後のgpt-5.6-lunaを継承している。事故前はgpt-6-astra。
起動設定追加自体はモデルを指定・変更しない。モデル選択の保全も残件とする。
M3の恒久的な着手証拠と表示連携は未完了。局所復旧を計画全体の完了としない。
通知dismiss後もOrcaのprompt-text待機判定が残る別問題は未修正。
08:38 UTCに実装Aが旧bridgeへのENOENTで完了通信できず終了した。
現bridgeのoperations空を照合し、同Dispatchへのfollow-upと最新bootstrap参照の補正で
同じ担当がcheck・ACK・worker_doneを実行。現bridge settled/completed、role recorded/exit 0確認。
M4はコード実装・固定read-onlyレビュー承認・関連79 tests合格。
変更別contracts/tooling gateも1129+164 tests、Ruff/actionlint/perf成功、exit 0。
source fingerprint: fe8a91fc6d0836cd0f3722dd83318047282a0fa5a0b2e9dac20586300343f119。
Help No impact（隔離された開発用通信先の指定のみ、製品runtimeへの経路なし）。
primary storage pass: 152 batches、486367547392 bytes、削除なし。
統括の新turnが正式ACK/worker_done受理を確認し、Helpレビュー提出・同Run検証を実行。
新しい環境変数は次回起動から適用。今回の稼働担当を設定反映だけのために再起動していない。
変更しただけで実運用の復旧完了としない。
09:18 UTC: generation 16の固定レビュー後、混在routingの経路が未実装のため統括停止を確認。
M5を追加し実装・回帰検証中。部分配車や指摘省略、利用者への不要な再承認要求では解消しない。
固定レビューで文書commit後のloop保存失敗からの復旧欠落と、混在時の戦略履歴欠落を検出。
validated write-ahead journalとexact before/after照合、混在戦略の同時保存を追加。
文書のみ／混在それぞれのsave直前・直後中断からの再開を回帰対象にした。
統括への待機指示は基盤検証後に同会話で明示解除し、新turnと具体的操作まで確認する。
M5固定read-only review APPROVED。関連24 testsを実装側・review側それぞれで合格。
回数による文書補正停止も撤廃し、同source・同指摘・同戦略の反復拒否だけを維持した。
変更後source `32adea387be2320e48c71ca24ca38c27b4d5f7da5a337799aa58a3c71238099c`
はrole fingerprint。変更別contracts/tooling gateは1137+164 tests、Ruff/actionlint/perf成功、exit 0。
CI source fingerprintは`be467fa4c678cc21b65aa9fb4b1fee50ddd9c827c34022403f2f5e9a1c0e88bb`。
以前のsource変更中のgateは成功証拠に使わない。primary storage pass（152 batches、未分類0）。
Help実レビューNo impact: host側の配車・所有照合・文書補正journalのみで、ゲーム入力・状態・
assets・Help catalogへ到達しない。ゲームコード・資源の差分なし。保持領域の削除なし。
09:40 UTC以降: 同じ統括terminalへ待機解除を送付（receipt c6955914-4901-4be0-b528-0b533ed5d29e）。
送信時はinput_acceptedのみだったため成功とは扱わず、後続terminalの新turn・watch実行を確認。
一時的workspace lock競合の後、統括自身がfresh loopを再照合して正式routeを実行し、
`documentation_then_implementation`受理を台帳とterminalの両方で確認した。
同Runのままprimary文書補正へ着手。実装Aの再配車・複合subjectの最終承認はまだ未確認であり、
TAK-14全体完了とは扱わない。新Run・Task・タブ新設や手動台帳改変は行っていない。
09:48 UTC: 文書candidate 92ef53697efd93df50e6b701bf1ac5f45cdd92bc後に検証失敗。
Cargo TMPDIR継承によるsocket path過長・tmpfs拒否fixture前提崩壊、MCP fixture障害を記録。
文書WALはvalidation_failedであり、結果不明の中断とは区別できる。M6を実装・検証中。
M6固定read-only review APPROVED、主担当61関連tests・review担当52関連tests合格。
dev.py変更でauto分類がRustも選んだため、監視側のゲームbuild/test禁止を守って自動gateを
tooling途中で明示中断（exit 130、成功扱いにしない）。contractsは合格済み。
同じ長いTMPDIR/TMP/TEMPを親に与え、host runnerのtooling groupを明示実行し、
1142+164 tests、Ruff/actionlint/perf合格、exit 0。前後のrole fingerprint一致:
`6016732efbf04373babb43ae7bf198a35fc6c91dd91e6a2761f6fa506291b75b`。
Help No impact（開発用検証subprocess環境と文書検証回復のみ、製品runtime変更なし）。
primary storage pass（152 batches、486348025856 bytes、未分類0、削除なし）。
10:04 UTC: 統括がscheduler HostBusyへのshell再試行を中断した際、最後の試行で
WALがvalidation_runningへ進んでいた。外側のCommandExecutionはexit 1だが、
validator自身の起動identity・終了receiptは未保存。以前のexit 1証拠を今回へ流用しない。
現在も同candidate・Runを保持して停止中であり、M6の局所検証成功を復旧成功と扱わない。
M7のscheduler待機変更に関する文書補正20 testsと、admission中断時に
candidate・loop・WAL・検証を変更しない追加1 testは成功。待機変更は固定read-only review承認。
検証中断の回収は未完了。primary storage pass（152 batches、486348034048 bytes、未分類0）。
文書索引・リンク検査pass。追加変更は開発用admissionのみでplayer経路は不変（Help No impact）。
今後の検証には実行主体が保存する終了証拠を必要とし、監視側の報告だけで復旧済みとしない。
M7続行: `orca_validation_fence.py`を実装中。既存統括のlauncher・bwrap・namespace initを
pidfdで観測し、同会話の失敗command、元spec、前journal、process所有lock、private transcript prefix、
現candidate/CI群/controller source closure、当該依頼のnative/external記録へ拘束する。
三processの正の終了と正常なregistry終了を確認した場合だけ中断用receiptを発行する。
このreceiptは検証成功ではなく、同candidateの新検証attemptを1回予約する根拠に限定する。
初回レビューのbinding不足を補正し再レビュー中。稼働中TAK-14にはまだ適用していない。
M7固定read-only review APPROVED（fence source `9af1193750e91f1e7d627e59a1b6f944abc42d6a7a4ef1a16a1fdf14495c9dd8`）。
外部同期の終端状態、native batch確定・exit identity、実Codex祖先のopen rollout所有照合まで追加。
rolloutはCodex既定0644を保全し、group/world writable・symlink・非regular・hardlinkを拒否する。
修正途中の全tooling結果は最終subjectの証拠へ流用しない。最新source
`c7cb194e9288809393d2c4cc337a9b50e3b1a6791d6abef527ed9c6b04d2b6cc`で最終gateを行う。
参照: docs/development-infra/orca-native-host-execution.md と validation-storage-workflow.md。
完了条件: 関連検証・固定レビュー・storage合格、同一依頼で実作業開始を確認。

## 10. 更新履歴

2026-09-28 13:03 UTC: M7の実運用回収後、文書検証exit 0と同Runの実装A着手・
正式settlementを確認。generation 18の変更別検証とHelp判定を経て次工程へ進んだが、
generation 19 bootstrapで利用上限終了の`last_agent_message: null`をstripしdriverが例外終了。
現在の利用可否は許可されており、同統括会話の新turn・調査tool実行を確認した。
`orca_dispatch.wait_for_codex_bootstrap`は文字列型を必須にし、null・欠落・非文字列を
起動準備完了と認めず既存deadlineで拒否する。タスク本文や権限の再送は行わない。
関連22 testsと固定read-onlyレビュー合格。tooling全群1163+164 tests、
Ruff/actionlint/perf成功（exit 0）、修正2ファイルの前後hash一致。
Help No impact: 開発用provider履歴の準備判定のみでゲーム入力・表示・assetへ到達しない。
storage pass（152 batches、486226853888 bytes、未分類0）、削除なし。
世代19の担当再開とdriver回復は未確認。統括調査の完了だけで製品工程再開と扱わない。

2026-09-28: モデル通知競合と未着手状態を分離し、復旧完了条件を定義。

2026-09-28 13:30 UTC: M8の固定read-only review APPROVED、実装側・review側とも15 tests合格。
helper SHA256 `7c95d3fa5bb0bde265a152fd7ae699e57985a9c9230b2e10ddf3cd12bc963be6`。
canonical generation admission、保存証拠の完全照合、WAL後の操作中断検査、arm後の所有検査を追加。
TAK-14の同source/session/bridge・全Run履歴をread-only照合済み。
準備のみの単発送信 `96e990f9-21bc-4bf8-ba48-a1d9207a60c8` はturn_startedまで確認し、
新turn `01a0e834-9fd5-7ef3-83f7-27bc4526decd` が正常な準備応答で終了した。
まだ製品Task投入・Driver再開は未実施。この準備成功を製品作業再開と扱わない。
auto CIは既存control差分でRustも選ぶため、contracts成功後に明示中断(exit 130)。
監視側のゲーム検証除外を維持し、tooling全群を別途実行。1178+164 tests、
Ruff/actionlint/perf成功、exit 0。承認済みhelper/testの前後hash一致。
Help実経路はNo impact。primary storage pass（152 batches、486227402752 bytes、未分類0）。
文書索引・リンク・diff検査pass。削除・モデル変更・利用枠resetなし。

13:35–13:37 UTC: M8正式helper適用がcomplete。同Run・同terminalでgeneration 19を
`ctx_39c359a784f6` / `task_3e831f239066`へ投入し、実装Aがrelease test配置修正を行い
worker_done succeeded受理・bridge settledを確認した。回収journalの元loop digestは
`0faedf766ed6a6be95c863ef7f1a0c2207dc7ced4c5d04d8e3eb17c8ee44c329`。
統括は`/exit`でlauncher exit 0・registry exitedを確認し、同じrequest/session/terminalへ
通常launchで復帰。新Driver PID 1038273、running・fresh heartbeatを確認。
待機解除通知 `76bc2b68-d759-4666-b343-d5287d2dad05` はterminalの応答と契約読込tool実行まで確認。
新Run・タブは作成せず、製品変更の検証・固定レビューは統括の継続工程に戻した。
今回の復旧を依頼全体の完了とは扱わない。

13:54 UTC: generation 19の検証exit 0・checkpoint `335b450f54ac6e9451e97bc89f593953839d09e4`
後、固定reviewが`review-scope-leak`で差戻し。M5の正規文書commitが後続worker checkpointの
review promptで失われ、worker write scopeと累積review scopeを混同したことが原因。
M9を追加。世代鎖・文書blob・保存checkpoint/validation/assignment/parent/treeを検査し、
read-onlyレビューscopeだけへ文書を引き継ぐ。旧sealed reviewを保存するatomic reissueを実装。
既存checkpoint/文書補正48 tests、追加復旧13 tests合格。古い非context形式でも、
generation file単独を文書権限の証明にしない。初回レビューでこの不足を検出し補強した。
変更前の全toolingは明示中断(exit 130)し、最終subjectの成功証拠に流用しない。
最終tooling/固定レビューは実施中、実loopへのM9適用は未実施。

14:19 UTC: M9最終固定read-only review APPROVED。関連63 tests、tooling全群
1193+164 tests、Ruff/actionlint/perf self-test、contracts合格（exit 0）。
checkpoint/recovery/testの承認時SHA256はそれぞれ
`21b7eb2fbc0b4a6db0a59f443eddc6061fcfc3e0a8f9fdf6deba83535274641f`、
`52511abdbdae7e030bc977b0515fb1b1dc4b9b34d4e75fc1294216a03f070032`、
`cd65ba7a1ab033416455e553cb77cb2882ef039882a446edc0d19dfd73d52043`。
Help No impact: 開発用レビューの文書証拠と制御状態だけを変更し、ゲームの入力・表示・assetへ到達しない。
primary storage pass（152 batches、485939949568 bytes、未分類0）。削除なし。
同統括は通常終了exit 0を確認し、同request/session/terminalへ通常launchで更新を読込中。
復旧helper適用と次工程の実動作確認はこの時点では未完了。

14:24 UTC: M9正式helperがexit 0でreview_pendingへ遷移。
旧digest `71753cef3539edbfcdc6b7c0bb8c65106123b48e985a811a5d702c1121bf84f1`の
sealed review/routingを保全。同統括会話へ待機解除を一度送信
（receipt `01e8d5f9-84d7-4982-884b-e88fa07277bf`）し、新turnの応答・watch実行を確認。
Driver PID 1450723、fresh heartbeat、同Run `run_3236488f0dff`で
固定reviewerのTask `task_f8ebd2a99bab` / Dispatch `ctx_7f309e331746`を正式起動。
同reviewer terminal `term_93ca8878-1092-4717-9ea4-e28eb259ce03`の読取tool実行と
lane `reviewing`まで確認。bridge `c1c7cdef-61a2-449a-82ba-f59597fe59e4`。
レビューは進行中であり、承認・統合・依頼全体完了とは扱わない。新Run・タブ作成なし。

14:36 UTC: M9固定reviewはapproved、統合headへ進行したが、統括のHelp判断登録が
scheduler HostBusyを繰り返した。lock所有PID1450723は同Driverであり別利用者ではない。
M10はsubmit-help-reviewのみ単一callerのqueued admissionとし、取得後にexact subjectと
operator controlを照合する。controlはreceipt直前・save直前にも照合し、停止後の再開を拒否。
関連73 tests、実flockの2 waiter/待機中中断/loop・source・所有変化/control停止の追加試験pass。
固定read-only review APPROVED。source/test SHA256:
`157571b85e2b9173b45e14d1a10e21862de23cfa6081d62629e9e260f9b6ec3e`、
`bd9c663dadb5378da01d85bb6dcc9f0a71788c05b61052194fd710a4057d5361`。
contracts、explicit base Help no-production-change、primary storage（152 batches、485940510720 bytes、
未分類0）pass。全toolingは実行中。統括への反復中止通知receipt
`6dc134cb-2670-489d-9893-7a846f8a044b`は同会話の応答まで確認。
正式登録と新turn内の次工程操作を確認するまでは復旧完了としない。削除・ゲーム検証なし。

M10全toolingは1197+164 tests、Ruff/actionlint/perf self-testすべてexit 0。
承認対象2ファイルの前後hash一致。Help No impactは開発用登録admissionのみという実経路から判断。
監視側はゲームbuild/testを実行していない。修正版登録commandの適用通知を同統括へ送り、
登録の成功と後続工程を確認中。稼働DriverやOrca本体の再起動は不要。

M10適用通知receipt `a558e72a-94c6-4e4b-aed0-8003da8ad87d`後、同統括の新turnで
Help Skill再読込・fresh対象照合・正式submit実行を確認。判断source/33 paths一致を照合し、
fresh loop digestへ拘束したspecを一度提出してexit 0。
Help receipt `ea37dd3d7d6b7c66a91d2eecd8cab420695138e20e6a07c12a4bcada8d0292e6`、
loop active / integration validatingを確認。既存Run・成果・担当履歴を保持し、
待機解除を言葉だけで終えず実際の登録まで進めた。統合検証・最終レビューの完了は未確認。

19:57 UTC: M4最終レビューでliteral backslash-nによる形式拒否2回の後、旧bridgeが失効。
上流の完了送信はなく、reviewerの承認文だけでは正式承認としない。M11を実装した。
固定read-only review APPROVED、bridge/既存復旧の関連71 tests成功。
source SHA256 `83256ad9e9f326317fa4dda5c5107f7b7c80441245e64c776780bba2ec23d28c`、
test SHA256 `78942d470501a1971825a3c00260872be0696483019aa6bf3cc7d68c7b1e6b6f`。
contracts、Help no-production-change、primary storage（152 batches、541687963648 bytes、未分類0）pass。
Help実経路は開発用結果送信だけでplayerの入力・表示・asset・Help本文へ到達しない（No impact）。
初回の局所testはexcept変数欠落で失敗し、修正後の71 testsを成功証拠とした。全toolingは実施中。
旧reviewerは同会話の正常終了exit 0を確認し、公式worker-abandonで旧Dispatchをfailedへ確定。
tab・会話・成果は保持した。guard付き再開と新settlementはこの時点では未確認。

20:07 UTC: M11全tooling1198+164 tests、Ruff/actionlint/perf成功、exit 0。承認hash前後一致。
既存resume-review-bridgeがexact loop digest `c14bc18f98d5501c04dcf5262d3f3bfdace78eaf48f4bd91a07f4fce29aa3289`
を照合しactive/review_dispatchingへ復旧。同Task `task_3751033fbfc6`、同session・同tabで
新Dispatch `ctx_df0f36718fec` / bridge `6707df84-385d-4578-aff1-118e8607a49a`が開始。
固定レビュアー自身が再確認・実改行の本文を送信し、正常終了exit 0、最終loop approvedを確認した。
統括への通知receipt `a3032a8e-cd53-4eaf-98df-808a847daaa3`はtarget_unverifiableを返したため
本文を再送せず同IDの照合だけを行った。その後、同統括の新turnに元本文の受信とwatch実行、
approved結果の読取を確認。入力受付だけで復旧とせず、実処理を確認した。TAK-14全体は未完了。

M12検証: 関連82 tests成功、固定read-only review APPROVED。
承認対象はloop `f7c5a7d947ee45ee9c9ad210f16d277cf98b02a3afef26afb4a2b03e30348511`、
recovery `4e841c5e208790d3a27447069dcd025f653685c5b7ab3b4ff59739e4d349ab6f`、
recovery test `c98a0c26bc0ebb15bf52ce180d2ce6abbd5ac2d45c5809b378f4b72047a96ee5`、
loop test `30573a21668d749376ebde68fffb41c53f2bde791b68364d93d6aa78c3d8f784`。
contractsとprimary storage check成功。Help No impact: 開発用の終了済み工程の承認検査と
保存台帳復旧だけであり、playerの入力・表示・runtime data・Help本文へ到達しない。
統括は同terminalで正常終了exit 0を確認。旧Driverとその子processの終了も確認済み。
全tooling完了後に同sessionを再開し、guarded recoveryと後継工程開始を確認する。

2026-09-29 01:03 UTC: M12全tooling1207+164 tests、Ruff/actionlint/perf成功、exit 0。
承認対象4ファイルのhash一致。同統括sessionを正常再起動しreadyを確認した。
guarded recoveryは停止時digest `1f3c72ea0cbd2d1f32a6e202a8b94ff2c3958ea53c8218a255e89867474e7263`
を照合してexit 0、approvedへ復旧（after `069d2a5acea45be078de285560fa001665539368f3e9a4e29426b149de406196`）。
before全文はclosed-loop-recoveriesに保全。再開通知receipt
`93ffc7e2-0cd0-421e-8581-dd09522b3477`の本文受信と同統括の応答を確認。
TAK-14全体の完了ではなく、後継m4-bridge工程の開始を確認中。

01:04 UTC: 同統括の新turnでrequest-status/showが成功し、同scope、generation9の
loop/lane/integration approved、M4 Bridge accepted、追加指示なしを確認した。
続けてM4 Bridgeの現成果と既存導入パターンを調べ、書込み範囲・受入条件・検証argvの
策定に着手した。Driver PID 3666016 running。再開は実コマンドまで確認済みだが、
この時点で後継worker配車・TAK-14全体完了はまだ確認していない。

M13: 後継M4 Bridge担当が編集前に共有UI境界のscope不足を報告して正式failed終了。
同Task・会話・tabを維持するscope改訂とretry-of経路を実装し、固定レビューで承認。
実適用では隔離統括の`/proc`からhost shell所有を証明できず、変更前に拒否された。
これを契機にCLI適用を既存の認証済みhost brokerへ移し、任意commandや別terminalを渡さず
host上で全guardを実行する補正を追加。scheduler待機後と保存境界でcallerを再認証する。
host補正関連117 testsとRuff成功。前版全tooling1219+164件成功はhost補正前の証拠であり、
最終版全tooling・固定レビュー・実適用を別途確認する。Help No impact: 開発用配車所有と
保存receiptの変更であり、ゲーム入力・表示・runtime assetsは変更しない。

M13実適用: host broker補正の固定レビューAPPROVED、最終関連119 tests成功。
scope helper `9477f9f2aae4431db12f25e81ce566a7d072bdfbcef52d6b27de5715d7de52cf`、
broker `7ec8c16ed671d3b7919e50c51046c4583c7f5100c10881e51e166b65b7cfd9f9`。
同統括の正常再起動後、helperが成功しloop digest
`4cf7bef3de8f1be58f41c3022d223b8ae17a9fe62baca0c732852b7cdb596f38`でactiveへ移行。
同Run `run_3236488f0dff`、同Task `task_d55aaf0517aa`のretry Dispatch
`ctx_b60922d88fa3`、ticket generation 1でimplementingを確認。
実装A terminal `term_b2ebc093-11ae-4c7b-86c4-fe62660da823`、session
`01a0eab7-f1b0-7be3-88e7-e5be6ba1b766`を維持し、実装担当自身のplacement関連ファイル読取と
現bridgeによるinbox照会の実コマンドを確認した。受付receiptだけを再開証拠にしていない。
TAK-14全体は未完了。最終版全toolingは引き続き実行中であり成功扱いにしない。

2026-09-29 02:07 UTC確認: 最終版全toolingがexit 0で完了（Ruff/actionlint、全Python tests、
Blender tooling 164 tests、perf self-test成功）。最終承認4ファイルのhashは不変。
runtime `542cfcbb-38a8-43a8-957c-53379bec2158`とsnapshotは一致、実装Aは同Dispatchで
実際のコード調査を継続し、loop implementing・監督表示working/implementation・waitReasonなし。
統括も同会話でwatchを継続。停止や新しい復旧操作はなく、全依頼完了ではない。

M14（Help更新sourceの正規継承）: 実装終了後の統括Help更新がfresh reviewへ登録されても、
checkpoint検証が旧worker終了sourceとの一致だけを要求して停止した。workerの終了記録は
不変のまま、Help限定変更を証明するsource継承receiptを導入する。旧Helpをメモリ内で置換した
fingerprintがworker終了sourceと一致することを要求し、非Help・HEAD・index変更を拒否する。
通常経路はHelp停止時baselineを保存し、既存停止はGit baseのHelpで同じ完全一致を証明できる
場合のみ回収する。新sourceの実検証・固定reviewを省略せず、validation/retry/checkpoint/WALの
全経路で同じ継承証拠を使用する。旧承認・worker終了の偽装、新Run・再配車は行わない。
回帰は正常更新、workerの既存Help編集、非Help変更、receipt改変、source再変更、保存中断、
同会話復旧と次工程実行を確認する。Help No impact: この基盤修正自体は開発所有の継承のみ。

M14実装・固定レビュー: Help受理loop snapshot、保存後のローカルprojection、exact schema/path、
source継承、scope外Helpのcheckpoint/固定review、candidate blobのref更新前照合を実装。
旧停止の回収はreasonだけを未実行証明にせず、正常終了PID namespace fenceと同期local CI・
external/native終端監査を要求し、unknown旧試行・新attempt・canonical前後loopをWALへ保存する。
固定read-only reviewer APPROVED、関連215 testsとRuff、primary docs/storage check成功。
全体検証の先行runはfixture directory modeの2 errorsを記録したため不合格。
fixture修正後の全toolingを再実行中。製品コードの直接修正・再配車・新Run・pushは行っていない。

実受入途中: 同統括session `01a0d377-feef-7520-bef5-50565ef282cb`、同terminalの旧launcher/bwrap/
namespace initが正常終了したことをpidfdで確認。Help exit fenceは`exited`で外部/native状態と
source不変を確認した。同会話の通常launchで修正版を起動中。停止loop digestは
`cd0afa7a8b17632c4aa9d33d45b62defa9c916fd2029b6c0cef7a16998c7612a`のまま保持する。
guarded resumeと次の実検証開始はまだ未完了。TAK-14全体完了とは扱わない。

M14最終検証: 最終全toolingはexit 0。Python1326件、Blender tooling164件、Ruff/actionlint、
perf self-testが成功。明示base `ae5b2066c4fd39a8dd6a6c439e272d125f749241`のcontractsも成功。
先行不合格の2 fixture errorsは修正済み最終runで解消。基盤変更のためゲーム試験は監視側から
実施せず、製品sourceの検証は復旧後の正規Driverが所有する。
同統括はreadyに復帰し、provider内からのfence.checkedがPASS。
復旧実行通知`3616ad89-9ec5-4e6d-9909-660dd8ff6773`の実受信・新turnを確認し、同Run復旧へ進行中。

M14実適用完了: 同統括のresume-help-sourceが成功し、同Run `run_3236488f0dff`をactiveへ復旧。
検証attempt `324aad25-cdf8-4fb1-a5d0-bd1c99bff096`、lane validation_running、reasonなし。
Driver PID606351の子としてhost validator PID706223と子検証PID706449が既存
`tak-14-m4-bridge`で実行中であることを確認した。元Task/Dispatch、session/tab、worker終了記録は維持。
before/after・旧結果不明扱い・終了fence・Help継承証拠は専用guarded receiptに保存。
primary storage pass（152 batch、583907446784 bytes、未分類0）。削除・新規検証job・pushなし。
このマイルストーンは基盤停止修正と同依頼の再開までを完了とし、TAK-14全体の完了を主張しない。

M15（統合検証失敗の修正経路、着手）: M14後のworker検証・固定レビューは成功したが、
統合検証のMCP並行client fixtureが`KeyError: result`で失敗。同Runとcleanな統合headを保全する。
疑似backendも実accountのheavy slotを取得しており、slot占有時にerror responseが返ることを再現。
テストだけの独立coordination領域で実flockとdaemon共有を検証し、本番guardの緩和をしない。
正常応答assertにerror診断を追加する。保存済みnonzero統合検証からtooling修正へ進める正規経路は、
exact source/head/command/ledger・所有者・終端状態を照合し、失敗証拠を保存したまま修正後の
検証と固定レビューを必須とする。単なる再試行、承認流用、ゲームの監視側直接編集はしない。
検証: fixture競合再現、正常共有/idle再起動、拒否/改変/保存中断回帰、全tooling、固定review、
同会話による実適用と次工程の実行証拠。現時点では実復旧未完了。

M15中間検証: slot占有で旧fixtureが`host slot busy (heavy)`応答になることを再現し、
修正fixtureは同じ占有状態で共有/idle再起動に成功。MCP4件、補強後の補正17件、統括prompt32件成功。
固定review初回はcontrol再照合不足とevidence形式検査不足でCHANGES_REQUESTED。
binding/接続/native終端/observed Linear/owner/controlをGit/WAL前と公開前に拘束し、
exact schema/type/hashを要求する修正を追加。停止後の未公開commitは保存し、再開時にGit再実行しない。
先行全toolingは成功したが補強前候補の結果。最終版全tooling・再review・実適用を進行中。
Help Skillによる判断はNo impact。開発制御とtest fixtureだけでplayer契約は不変。

M15再review APPROVED。helper SHA256
`7241e823453b62ce2d5b6ddea68e56e69da5bc5e8846c6b4834b2198092912c4`。
補正17件は独立reviewerの再実行でも成功。既存統括へ元baseからMCP test1ファイルのみの
local correction commit準備を依頼し、実helper適用は最終全tooling終了まで待機する。
同terminal/sessionを維持。送信のtarget照合拒否は同request IDで扱い、tui-idle確認後の
受理を観測。受理だけで製品再開とは扱わない。

M15最終全toolingはexit 0（Python1332 tests、Blender164 tests、Ruff/actionlint/perf成功）。
最終contracts/primary docs/storageも成功。統括が作成したlocal correction commitは
`c31dd3d739fec111d9151fc56457c97d7409e6ef`、親は`d40c3b78ba76f3c32d51a062dd57228a473e348a`。
差分はMCP fixture1ファイルのみ、SHA256 `46c2d33e93e385f55f06a349578189976b88e392f27b978e38664b649f6dfb1f`。
生成時にtarget/infraのHEAD/index/status/refs不変を統括が確認し、監視側もcommit/parent/blobを照合。
同統括へ正式適用とfresh Help判断・通常検証の継続を通知した。実適用結果はまだ確認中。

M15実適用: 最初のshowがconsumerの一時inbox check状態を採り、guardは変更前に拒否。
統括がjournal/receipt不存在と安定状態を確認しfresh specへ訂正後、正規helperが成功した。
新統合head `ba1df06caa70a68e6e07c49f356ede43da0538cb`、source
`99bf88e41175ad48e217d1348398138a3e0e508716616bc9584528b308704792`。
旧headとの差分はMCP test1ファイルのみ、working tree clean。同Run/Task/session/tabを維持。
correction journal `e17b15f55838fbf5904efb1493659b109642a56a6494bc4840f2905dbe527d99`へ旧失敗を保存。
通常Driverが新subjectのHelp判断を要求し、統括が処理中。統合検証開始は次に確認する。

M15再開実受入完了: 同統括が新subjectのHelpをupdatedで登録し、同Run
`run_3236488f0dff`はactive、integration validation_running、reasonなしへ移行。
Driver606351の子PID1214810が現行orca_host_validation.pyを既存tak-14で実行し、
子検証PID1215035も稼働していることを確認。runtimeは同じ、監督表示working/validationでwaitReasonなし。
同じ統括ターミナルに適用head・検証開始・継続watchの記録が残る。
M15の停止修正と同依頼再開は完了。統合検証結果・製品固定review・TAK-14全体完了は未確定。
primary storage pass（152 batches、607333228544 bytes、未分類0）、素材・成果物削除なし、pushなし。

M16（Help結果の監督表示変換、着手）: M15後の同head統合検証exit0、固定review approved、
全attempt release/ACKを確認。Bridge工程は承認済みだが依頼全体は未完了で、統括は次工程を準備中。
表示controllerだけがValueErrorでsnapshot更新を停止。producerのHelp decision `updated`を
表示境界が受理せず、UI契約の`update_required`とのenum不一致が原因とreadonly再現した。
保存台帳は変えず、projection境界で`updated`を既存UI impact分類へ変換する。
none/旧表示値との互換、未知値拒否、producer ledger不変とUI schema受理を回帰確認する。
関連/全tooling・契約・Help No impact・storage・固定review後、既存guarded controller refreshで
表示だけを切替え、同runtimeのhealth/snapshot更新と同統括の継続を確認する。製品processは止めない。

M16中間結果: projection関連73 tests成功、固定review APPROVED。
supervision SHA256 `e8c741ffeb43621b823752f125e90a25370c95da431636f3cced7f2e39749431`。
実Bridge承認archiveをreadonly入力として、stored updated不変・UI update_required・UI schema合格を確認。
統括は同Run generation11のm6-nine-closeへ進み、旧表示controllerは未承認loopへ移ったことで自然復帰。
再発防止の修正はまだreload前。全tooling実行中、contractsは新M6作業場のretain未登録を検出し不合格。
統括へ正規登録を通知し、実装Aは中断せず継続する。

M16完了: 最終全tooling exit0（Python1333、Blender164、Ruff/actionlint/perf）、contracts、
primary docs/storage成功。M6作業場は同統括が正規retain登録し、未分類0へ解消。
controller refreshはruntime/source/generationを照合して適用し、PIDを維持したexecで
generation `cb9aaab1-7b96-4eb1-8e9c-1bdf49650591`、source
`5763beef646628df63853610f5308c8daea3f41f57a54422f9bde001cf6890bf`、health running/errorなしを確認。
同runtimeのsnapshotはworking/implementation、waitReasonなしで更新。
TAK-14は同Runのm6-nine-close implementingを維持。統括は同terminalで担当監督を継続。
ゲーム編集/build/testの監視側実行・担当中断・新Run・push・削除なし。
storage152 batches/608051884032 bytes。M4 Bridge工程は合格済みだが、TAK-14全体は未完了。

M17（失敗した全体受入からの補完工程、着手）: generation12の監査担当は未達条件を報告して
正式failed終了した。既存scope-replanはclean編集前のdirectory追加専用で、監査成果を残した
工程再計画には使えない。失敗を承認へ変更せず、exact監査source・終了/release/ACK・同Run・
元依頼binding・直前承認済み統合基点を照合し、統括のprivate specによる補完工程を別のclean
作業場へ登録するguard付き経路を追加する。旧監査作業場・成果・会話・失敗台帳は保持する。
依頼scopeや受入条件を変更せず、補完工程にも変更別検証・Help実判断・固定reviewを課す。
未知settlement、未ACK、変更済みsource/target、意図的pause、別Run/別bindingは拒否する。
tooling・契約・Help・primary storage・固定read-only review後に同統括へ適用を戻し、
新turnと具体的な補完作業の開始を確認する。監視側は製品編集/build/testを行わない。

M17中間: failed acceptanceは承認世代にせず、同generation・approved predecessor・Runのまま
再計画する。旧attemptをcurrent accountingへ継承し、旧loop/spec/proofのdigest拘束archiveを
load・completion・bootstrap・replayで検査する。host brokerは実行前に依存moduleをpin注入し、
archive直前・loop保存直前に呼出元を再認証する。累積再計画回数の上限は設けず、循環だけを拒否。
実TAK-14のread-only終了/source/Run/統合基点照合が成功。統括同会話の新turnを確認し、
補完用clean作業場をstorage retain登録、private replacement specを永続保全した。
関連テストとcontracts/Help/docs/storageは成功。全toolingは修正前fixtureの失敗を記録し、
修正後の最終再実行中。固定review・broker切替・実装Aの実起動はまだ完了していない。

M17実適用: 最終helper `743d4dd2478749955e4d8b38de8c38f5c362ce4026ae5bdf0f00f091bceb11f4`
を固定review APPROVED、関連136 tests合格。全tooling一巡は成功し、最後の拒否条件追加を含む
全群再確認は継続中。旧統括の正常exit0/Driver stoppedを確認し、同terminal/sessionを通常再開。
保存specのlane Help理由だけの相違はguardが変更前に拒否。統括が元validationを維持して補正し、
broker経由の正式適用に成功。operation `2a171a5d1758b0969e256afb2e971b7d507b26729d6f65cdd9e4fd9bb715489e`。
同Run `run_3236488f0dff`・generation12でactive/implementingへ遷移し、通常Driver2294186が
Task `task_8f1331fffaae` / Dispatch `ctx_f9de0ddf79c6` を投入した。
実装A terminal `term_433819d8-6ed9-4cf8-91a5-b731c7689ffe` で仕様・schema・validation実装の
探索開始を確認。旧auditとfailed attemptは保全し、新Run・旧Task再送・承認代作はない。
補完実装の完了やTAK-14全体の受入完了を意味しない。

M17再開対応完了: 最終同コードの全tooling exit0（Python1350 tests、Blender164 tests、
Ruff/actionlint/perf成功）、関連136 tests、明示baseのcontracts、Help No impact、primary docs/storage pass。
基盤branchのゲームsource/assets差分なし。ユーザー指定どおり本基盤修正でゲームbuild/testは実行しない。
実装Aは補完作業場でschema・validation・描画consumer・対応testsの編集を開始し、
統括は同Runをwatch継続。旧auditを保持し、削除・push・新Run作成なし。
storage152 batches/635959296000 bytes、未分類0。新候補作業場は現補完作業のretain登録済み。

M18（統合後の複数所有者への段階補正、着手）: generation12の実装承認・統合検証後、
全体reviewはproduction inventory、native/performance evidence、正本文書の不足を封印した。
統括は既存recipeでTank/MudMixer planes候補を生成・確認したが、収集driver未実装と
全指摘を単一担当に戻すrouteの制約で停止した。offline verifierを実行recipeと偽装しない。
統合routeへ全finding IDを網羅する明示repair planを追加する。実装Aには現在のscope内の
collector補正だけを割り当て、統括native/assets・docs工程を未完のまま履歴に保持する。
同じfindingを複数所有者へ分解できるが、subset・未知owner・scope外path・空の実装段階は拒否する。
依頼scope、Run、base、会話、固定reviewer、元指摘・受入条件は維持する。
実装修正の承認を全体承認へ昇格せず、統合後reviewは繰り延べた全指摘も再評価する。
exact loop/receipt/source・全settlement/ACK・利用者control・native操作非競合を照合する。
回帰test、tooling/contracts、Help No impact、storage、固定read-only review後に適用する。
host recipe登録は収集driverの完成・review後の別段階であり、本route修正だけでは実機受入可能としない。

M18検証完了: 固定read-only review APPROVED、関連72 tests、最終同コードの全tooling
（Python1356、Blender164、Ruff/actionlint/perf）、contracts、Help No impact、primary docs/storage pass。
最終combined repairs SHA256 `0639704743304271798a5006c8b10a8bd7f8fe1d27e477a40ff3aa990311ac6e`。
同統括terminal/sessionを正常exit0後に再開し、修正版Driver3380088の新turn・受付確認を観測。
private specは元の全3指摘を実装3 step・統括7 stepへ分解し、scope/base/Runを変えない。
正式route成功、native event `0425bf34-054f-558c-85a9-1aefb5674035`に
`correction_routed`後続証拠を確認。同Run/generation12、実装A revision4、
Task `task_2a9e5fc6cfab` / Dispatch `ctx_43529117244f`でimplementingへ移行した。
同実装A terminal `term_433819d8-6ed9-4cf8-91a5-b731c7689ffe`でobservations.rs、
native_ui_input.py、production lifecycle/acceptanceの具体的調査を確認。
統括は担当のAPI read-only参照質問を受け、既存write scopeを維持する回答を準備して継続監督中。
監督表示working/implementation、waitReasonなし。新Run・push・削除なし。
M18の段階補正と再開は受入済み。製品native/assets/docs受入と依頼全体完了は未完。

M19（native登録controllerの明示、着手）: M18 collector統合後のstrokes登録で統括が
製品worktreeの相対dev.pyを呼び、hostがpinしたprimary controllerと異なるhashで登録した。
bridgeは送信前に正しく拒否した。guardの緩和や登録hashの上書きはせず、統括promptに
primary絶対pathのvalidation入口、subjectとの分離、未実行登録の正式abandoned/finalizeと
再登録条件を明記する。primaryと製品cwdが異なるprompt回帰testを追加する。
関連tooling/contracts・Help No impact・storage・固定read-only reviewで確認する。
実復旧は同統括が未送信・成果なしを再確認し、正規経路で登録を置換して同Runを続ける。

M19修正・再開確認: promptと回帰testだけを変更し、固定read-only review APPROVED。
UI33/native bridge48 tests、Ruff、明示baseのcontracts/Help No impact、primary docs検査が成功。
UI helper SHA256 `f72ea3261affb0b9ac1ce3c62748c709e15264f3b1c2e3e4fa00a2c3fd3dfe91`。
同統括の新turnが旧Tank strokes v1を正規abandoned/finalizedとして閉鎖し、primaryでv2を
登録・inspect・launchした。controller hashはhost pinと一致し、同head
`0534074df27af764f4b899c586768dfdb2645904`・Runを保持。v2はexit0、独立verify pass。
終通知後も同統括が保持用途登録とfinalizeを実行中。登録中・needs-finalize時のstorage拒否は
正常な未終了工程の検出であり、無視・成功扱いしない。稼働中のstorage検査は157 batches、
665332412416 bytes、未分類0でpass。成果物削除・新Run・pushなし。
現会話へ同じ具体的入口を通知済み。新promptは次の通常起動から使い、稼働工程は中断しない。
M19最終確認: v2 finalized/pass（2026-09-29T15:03:21Z）、同統括が生成画像を開いて
比較工程へ進行。primary storageは157 batches/665332416512 bytes/未分類0でpass。
今回の停止からの復旧は完了、TAK-14全体は未完。
