# Orca安全地点へのロールバック計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | orca-safe-rollback-plan-2026-10-03 |
| ステータス | Superseded — R1不成立・未適用の履歴を保持。通常Orca復帰・独自統括撤去へ方針変更 |
| 作成日 / 最終更新日 | 2026-10-03 |
| 作成者 / 実施責任者 | Codex main agent |
| 関連提案 | N/A |
| 関連Issue | TAK-14 / request `54cf1dd7-7485-5f49-a8e5-827773355096` |
| 関連計画 | [中断した期限付き復旧](orca-tak14-bounded-recovery-plan-2026-10-03.md)、[配備・変更履歴](orca-system-hardening-plan-2026-09-29.md) |

## 1. 目的

2026-10-03の利用者方針変更により、現行は[通常Orca復帰・外付け化計画](orca-normalization-and-coordinator-extraction-plan-2026-10-03.md)。
以下の上限・launch-supervised維持・S0/S1条件は当時の判断履歴であり、現行の実行指示ではない。本計画を再開しない。

復旧機能の追加を止め、既存成果・会話・Runを失わずに運用を保全する。
その上で、直前の追加配備を撤回できるか判定し、適合する保存済み本体へ限定的に戻す。
「ロールバック」と「統括の自動復旧」は分離する。旧buildへ戻しても旧runtime UUIDは戻らない。
新runtimeで旧EnabledLaunchが有効になるとは扱わない。

### 安全地点を二段階に定義

| 地点 | 到達状態 | 成功に含まないもの |
| --- | --- | --- |
| S0 保全・変更停止地点（先に確立） | 自動復旧/新規配車を行わず、現在の成果・所有・unknown receiptを保全し、整合した退避資料を保持 | Driver復帰、旧版切替、ゲーム工程再開 |
| S1 限定ロールバック地点（条件付き） | 互換確認済みの保存済み旧本体・対応CLIを使い、同daemon/PTY/会話/Runを保持して閲覧できる。統括の自動継続は停止のまま | 過去runtime復元、旧guardの解除、依頼全体完了 |

S0は追加損失を防ぐ退避地点であり、現在の停止を「正常稼働」と呼び替えるものではない。
S1の適合性が証明できなければ、版切替をせずS0で終了する。

## 2. スコープ

### 対象

- 現配備・候補旧配備・起動設定・helper/sourceの比較と変更所有の特定。
- 保存状態の整合した退避と隔離コピーによる旧版互換性確認。
- 合格した場合のみ通常終了と限定的な本体/CLI選択の変更。
- 既存固定read-only reviewerによる切替対象・互換性証拠・保全手順のreview。

### 非対象

- 新しいretirement/succession/enable frameworkの実装、修正build、新規Run。
- request/loop/Run/registry/receipt/会話を過去時刻へ巻き戻すこと。
- ゲーム編集・build/test、push/merge/公開、課題取消、原本削除。
- 既存dirty/untrackedの一括revert、他案件の停止、daemon置換、guard回避、偽ACK/exit0。

利用者「すすめてください」に基づき2026-10-03 13:54:09 UTCに実行準備を開始。
R1期限14:04:09 UTC、全体停止期限14:54:09 UTC。T0を再設定しない。

## 3. 現状と候補地点

本計画作成時のread-only CLI観測:

- app PID3049512 / runtime `2d5830cc-61d0-4f3f-9a17-00349be14e52`。
- loaded build `0515a76b8512bda73db3866d90cd7b6007a4a21950f7bd299603d5a0da5069a0`。
- `current`およびcanonical CLI shimはiteration-02 packageを選択している。
- 同Run `run_3236488f0dff`、consumer_generation2、同統括terminalをreadbackで確認。
- 復旧計画のM1不合格後、新しいlive retirement/registry変更/起動は未実施。
- 既存監視automationは直近確認でPAUSED。実行開始時には再照合する。

### 第一候補：11:15の本体変更前に使われていたpackage09

- path: `/home/satotakumi/tools/orca-ui-lifecycle/.git/orca-ui-acceptance/guarded-create-20261003-09/package/linux-unpacked`
- build履歴: `1e1e2c9d96092b75f768e226031688d4f7aefa8f0a60aea5f69cad7f3e3c4305`。
- executable存在・size224660728 bytesを今回確認。package全体hashは今回未再検証。
- 既存履歴にprotocol37・保存daemon/PTYの維持と08:03の新turn実見がある。
  これは当時の受入証拠であり、現在の保存データとのdowngrade互換性の証明ではない。
- 11:15/11:30以降の保存形式、pending取消履歴、epoch、host receiptを読めることが必要。

iteration-01は停止原因となったruntime変更後の版なので第一候補にしない。
さらに古い版・Git HEADを根拠なく「最後の正常版」と選ばない。
特にprotocol36へ戻して現protocol37のPTYを作り直す案は不採用。

### 戻すもの・戻さないもの

| 対象 | 方針 |
| --- | --- |
| 本体package / current選択 / CLI shim | 互換確認・固定review後、同じ候補へ一体で切替。元選択とファイルbytesを先に保存 |
| launch-supervised | 監督表示設定を維持。候補packageとhelperの適合性を確認。環境なしCLI open/直binary起動は禁止 |
| control helper/source | 当時のexact一式・現保存状態との適合を証明するまで戻さない。個別SHAの混在を作らない |
| 今回追加の未接続復旧部品 | 起動へ接続せず保全。未適用部品を削除してlive復旧したとは報告しない |
| 会話 / Run gen2 / request / loop / registry / receipt / epoch | 現在値と履歴を保持。旧DB・旧JSONによる上書き復元は禁止 |
| daemon / PTY / 統括・固定reviewer・必要担当tab | 同一性を維持。通常終了で保持されることを先に確認。不明なら切替しない |
| ゲーム・他sessionの未commit成果 | 変更しない。backup名目でも原本を移動/削除しない |

## 4. 方針・総量と工数上限

実施時は**合計60人分かつ経過60分**を上限とする。main最大50人分、固定review最大10人分。
T0は利用者の実行指示後、最初の実行準備開始時刻を一度だけ記録する。
調査・試験・待機・終了処理・報告を含む。期限内に記録・報告を終えるため末尾5分を確保する。
前の復旧計画の期限をリセットして復旧実装を再開するものではない。

上限は成功予測ではなく損失上限。新しいコードが必要になったら、この計画では実装せず中断する。
復旧失敗を取り返すための試行的本体切替・繰返し再起動をしない。
Git branch切替/commitなし。同目的control branchとprimaryの文書を保持する。
Bevy/Rust変更なし、PR/CIのための公開なし。

## 5. マイルストーン

| 工程 | 上限 / 累積 | 内容・完了条件 | 不合格時 |
| --- | --- | --- | --- |
| R1 対象確定 | 10分 / 10分 | 現package/CLI/helper hash、全関係process birth、Run/registry/source、停止すべき自動経路と稼働担当を特定 | 変更せず終了 |
| R2 S0保全と隔離互換確認 | 20分 / 30分 | 同一checkpointの退避検証、候補全package照合、現保存形式→旧版の読取/書込挙動・daemon接続を隔離試験 | S0で終了。live downgradeなし |
| R3 固定read-only review | 10分 / 40分 | exact切替tuple、全保存範囲、非自動起動、同daemon/PTY保持、失敗時戻し手順を承認 | S0で終了 |
| R4 通常終了・限定切替 | 10分 / 50分 | 通常終了を実証、daemon/PTY生存再確認、package/CLI選択、監督設定を維持して通常起動 | unknown操作を再実行せず保全 |
| R5 S1照合・終了報告 | 10分 / 60分 | 新loaded build/runtimeと現在terminal再特定、同会話/Run/PTY/原本、閲覧可能・自動処理停止を確認。末尾5分は報告 | 復旧成功とはせず、到達地点・残存状態を報告 |

R1で停止対象が実行中の製品工程/他案件を含むと分かった場合は、中断せずその工程を保持して計画を止める。
R2で保全用lockを得られない場合はguardを迂回しない。保存に介入せず未達を報告する。

## 6. 保全・互換性・適用の具体的条件

### R1/R2：適用前

1. bounded status/terminal/run/worker情報とhostのactual process/FD/birthを照合する。
   worker一覧空、PTY生存、旧ACKだけを安全証拠にしない。
2. 監視PAUSEDと製品新規effectなしを確認する。既存正式な制御で自動起動を抑止できるか特定する。
   `launch-supervised`は現在controller/helperを設定するため、旧版を起動するだけでは非自動実行を保証しない。
   **既存の支持された非自動起動手段を証明できなければR4へ進まない。新しい抑止機能は作らない。**
3. primary validationへ用途を登録し、必要な状態だけをowner-privateに退避する。
   SQLiteは正規backup APIまたは確認済みquiescent checkpointでDB/WAL整合性を保持する。
   live DBファイル単独のcp、異なる時刻のDB/JSON寄せ集め、認証情報の出力は禁止。
   loop/registry/immutable input/fence/transfer/WAL/unknown receipt、会話参照、tab/PTY対応、起動選択を含める。
   会話全体を毎回巨大複製せず、正規の保全方式・範囲を確定する。唯一原本はその場に維持する。
4. 退避copyのhash/DB整合性/各正本参照を検証。復元試験は隔離領域だけで行い、原本を上書きしない。
5. 候補旧版を元profile・live daemon・認証・外部連携へ接続しない隔離環境で確認する。
   現在状態の取消unknown、Run gen2、所有、tab/session、schema/versionを保持して読めること。
   migrationによる欠落/再送/自動spawnを拒否できること。単なる起動成功では不十分。
6. protocol37の既存daemonとPTYを保持する接続経路、通常終了の保存範囲を一次sourceと隔離試験で確認する。
   helperの当時tupleが未確定、互換性不明、必要な既存試験がない場合はR2失敗とする。

### R4/R5：切替（R1〜R3全合格時のみ）

1. 正式な通常終了を使用する。CLIに正常quitがなければ利用者による通常メニュー終了が必要。
   待機は予算に算入。SIGTERM/kill/terminal closeを通常終了の代用にしない。
2. 旧app終了とdaemon/全保持PTYの生存を独立確認してから、exact package/CLI選択だけを変更する。
   原設定の退避、atomic選択変更、readbackを行う。支持された配備手順がなければ切替しない。
3. `launch-supervised`経由で候補を一回起動。起動結果不明なら同操作の読取照合だけ行い再起動しない。
4. 新runtimeで統括terminalを再特定し、同incarnation/PTY/保存session/Run gen2とimmutable履歴を照合。
   旧runtime IDへJSONを書換えず、旧EnabledLaunchは旧権限のまま保持する。
5. 本体閲覧・監督表示・必要tabsが成立し、新effectが始まっていないことを確認してS1と判定する。
   Driverが止まったままであることを明記する。製品工程の再開は別の計画で扱う。

## 7. 検証と停止条件

- package09の過去受入は候補選定にだけ使用。現DBとのdowngrade受入へ流用しない。
- R2の独立verifierとR3 exact read-only reviewが両方必要。source変更で承認無効。
- Helpは計画文書のみで機能変更なし。実行時に本体/helper変更があるなら実経路のHelp判定を行う。
- primary storage checkを開始/終了時に行う。ゲーム/Cargo/native受入は今回の対象外。
- schema不一致、未知の自動migration/effect、other worker稼働、PTY切断リスク、source drift、unknown送信、上限到達で停止。
- 以下の原operationは再送・再取消・巻戻し禁止:
  通知`7e40892f-ab1d-4efe-8200-c65ca5efb0d7`、取消`ca60adfa-1727-5885-804f-a9fca1fab976`、
  既存Run consumer移行、bootstrap/recover-settled、過去resume-bridge/Enter。

### 検証データ管理

正本はprimaryの[storage workflow](../development-infra/validation-storage-workflow.md)。
ownerはmain、consumerは本rollbackの比較・保全・固定review。開始時bytesと退避copyの実測bytesを登録する。
現在のpackage09とiteration-02は比較・戻し先として必要。既存review-active cacheと原本を保全する。
実行中は登録batchをseal/finalize/checkする。不要jobは所有確認後に処理し、今回の原本削除は禁止。
退避copyのrelease_whenは切替判断・事後保全の確認終了。原本を消してcopyだけ残す運用は禁止。
この計画作成では新backup/job/packageを生成していない。

## 8. 切替失敗時の戻し方

- R4以前：現選択を維持。退避・記録だけを残しS0/未達の別を報告する。
- R4以降：旧版が書いた状態は捨てず、現DBを過去snapshotへ上書きしない。
- iteration-02へ再選択する場合も、旧版での書込差分・双方互換性・通常終了・同daemon保持を確認する。
  **選択を戻すだけで元runtime/EnabledLaunchが戻るとは保証しない。**
- この逆切替の条件が未証明/予算外なら、正常quit可能な範囲で追加effectsを止めて保全し、再試行しない。
- Git reset/checkout/clean、保存台帳の直接書換え、原本削除は一切使わない。

## 9. AI引継ぎ・完了条件

- 現在地：R1を実施し不合格。R2〜R5未実施。S0の整合退避もS1切替も未達。
- 次のAIはこの条件不成立を無視して切替しない。60分残枠を新実装へ転用しない。
- 現最大の未確定事項：package09と現保存形式の互換性、非自動起動経路、helper当時tuple、正常quitの実行手段。
- 参照：関連2計画、orca-cli/orchestration/recovery契約、primary storage workflow。
- 完了：S0またはS1のどちらまで到達したか、保全範囲・unknown・残存Driver停止・使用時間を報告。
  S0だけなら「ロールバック未適用」、S1なら「本体限定ロールバック、統括未復旧」と明示する。
- 索引更新/diff check/storage結果は本計画作成の検証であり、互換性/切替承認ではない。
- 計画文書検証：plans/proposals両索引check、diff checkはpass。primary storageは187batches、legacy0、675649331200bytesでpass。
  `dev.py docs --write`の全体契約検査は既存node_modules/debug・nan READMEの4リンク欠損でfail。
  今回計画のリンク欠損ではない。vendorを変更/削除せず、全体gateがgreenとは報告しない。

### 実行記録：R1不合格（13:57:12 UTCに判定）

T0 13:54:09 UTC。判定まで3分03秒、main保守的計上4人分、reviewer0人分。
期限切れを待たず、§6.2の支持された非自動起動条件が不成立と確認した時点で適用を中断した。
計画作成時にこの実行可能性を先に確認すべきだった。実行時に新たな抑止機能を作って埋め合わせない。

確認した根拠:

1. canonical CLIでcurrent app/runtime/build、同統括terminal/incarnation/PTY、同Run/gen2をfresh readback。
   launcher2087516/provider2103719/init2103720/Codex2103721/shell1565694はhostで生存。
   boot `117677d4-de8c-4686-a75b-a057f8deb767`、各startticksは
   13235454/13247924/13247924/13247925/12832117。
   app3049512 startticks14492180、daemon1459822 startticks12736547。
   daemonは既にpackage09 executableを使いprotocol37を保持。daemonを旧版へ置換する理由はない。
2. automation `orca`の設定はPAUSED。しかしこれはCodex監視だけで、Orca内部controllerの停止ではない。
3. parent route `f6a2c3f6-b00c-4b31-bb11-c490d63e4b44`はreadyで同child requestを所有。
   parent lifecycle fileは不存在。`orca_request_runtime.py`のSyncWorker.pollは不存在をactiveとして
   `synchronize`へ進む（同file514行以降）。loopのpausedとは別の停止境界である。
4. 原本loopのenvelopeを確認し、`.data.phase=paused`、理由runtime_unavailable、Run/gen2を確認。
   `orca_supervision.py:953`のpreflight_pauseはloopがapproved以外なら拒否し、idle shellも必要。
   今回は通常pauseを実行していない。guardを緩和・台帳直接書換えで通す案は不採用。
5. app側`src/main/runtime/rpc/methods/supervision.ts`のread/request/navigateは
   ensureSupervisionControllerを呼ぶ。監督表示のreadも純粋な読取とは限らない。
   controller起動は`supervision-controller-process.ts`のserve固定で、読取専用flagはこの経路にない。
   controlのserveはSyncWorkerを開始し、tick/start_consult/start_routeも実行する。
   script環境変数を外す起動は必要な監督表示サービスを欠くため採用しない。
6. Run全160worker rowsを2ページ読取。全dispatchはsettled、159のlivenessはunverifiable、1はexited。
   この一覧から全process終了は推定しない。release/close/stopは実行しない。

現設定/source照合SHA:

| 対象 | SHA256 |
| --- | --- |
| launch-supervised | `27065b641bb3c3beb693ceb21331d7f2b3571ad17df8390c9b2db70eedb07ac9` |
| canonical CLI shim | `356b6151e01b612285f39abc6afdc37d97bacd6a200a02daca1eab75f52ee6e7` |
| control orca_ui_coordinator.py | `733a19e62b21467e1542121f2aa1671d13bb9281924b849a8496ac7061cc83c5` |
| control orca_supervision.py | `6d139d10e26dc9c11623a6805eb4cc48081e88694614f47f7c0ff214a11d0639` |

結果：新しいbackup/checkpoint、隔離互換試験、固定review、正常quit、本体/CLI切替は未実施。
原本・設定・processへ新しい操作なし。R1で確認した読取証拠とこの文書のみ追加。
現在の保存状態を保持しているが、整合退避copy未作成のためS0達成とはしない。
Driver failedは未解消。ロールバック完了・統括復旧とは報告しない。

続行に必要なものは追加の通常実行許可ではなく、正式に未解決workflowの新効果を抑止できる経路と、
その状態を保持する起動方式の証明。既存経路で証明できない限り、この計画でのlive切替は不可。
新実装を含む別案や監督表示要件の変更はこの実行の範囲へ勝手に持ち込まない。
終了検査13:58:33 UTC：開始から4分24秒、main保守的計上5人分、reviewer0人分。
両索引check/diff check pass、storage187batches/legacy0/675649368064bytes pass。
全体docs gateは既存node_modulesの4リンク欠損でfailのまま。新規job/backup/package/削除なし。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-10-03 | Codex main | 安全地点S0/S1、package09候補、非巻戻し条件、60人分/60分上限と適用gateを制定 |
| 2026-10-03 | Codex main | 実行指示後R1をfresh確認。非自動起動条件不成立で3分03秒時点に適用中断、原本保持 |
