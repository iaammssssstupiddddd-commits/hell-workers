# Orca運用基盤の恒久修正・収束計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `orca-system-hardening-plan-2026-09-29` |
| ステータス | Superseded — 独自統括を撤去し通常Orcaへ戻す方針へ変更。過去実績・未完・不成立の履歴を保持 |
| 作成日 / 最終更新日 | 2026-09-29 / 2026-10-03 |
| 作成者 | Codex |
| 関連提案 | N/A（既存運用の是正） |
| 関連Issue/PR | TAK-14を観測対象とする汎用基盤修正。新規Issue/PRなし |
| 関連計画 | [配車・復旧の実績](orca-dispatch-submit-plan-2026-09-28.md)、[依頼ライフサイクル是正](orca-request-lifecycle-correction-plan-2026-09-26.md) |

## 1. 目的

2026-10-03の利用者方針変更により、現行は[通常Orca復帰・外付け化計画](orca-normalization-and-coordinator-extraction-plan-2026-10-03.md)。
以下は独自統括の実装/調査履歴。未完hardening・復旧を続行する指示ではなく、conceptと保全対象の根拠として読む。

停止するたびに監視担当が原因を解釈し、個別helperを追加して統括へ指示し直す運用から脱する。
利用者が指定した依頼と権限を保ったまま、既知の停止は正規経路で復旧し、未確認の操作は
二重実行せず調査へ移す。工程の成功を依頼全体の完了と誤認しない。

成功指標:

- 受入シナリオ内で、通常継続に利用者の「続けて」、内部ID入力、監視担当の手動Enterを要しない。
- 送信受理・turn開始・具体的操作・工程完了・依頼完了を別々の証拠で判断する。
- 重複実行・誤ったDone・未検証の承認・意図的停止の自動解除は0件。
- 復旧の成否を、修正ファイルの存在でなく同一依頼の具体的な次工程開始まで確認する。
- 上限回数・保存日数など、利用者が指定していない制限を追加して停止させない。

## 2. スコープ

### 対象

Orca本体の送信・状態投影、project-owned controller/Driver/bridge、検証の登録入口、
復旧・版管理・起動、既存ターミナルへの移動、終了・保持、非ゲームE2E、運用文書。

### 非対象

- TAK-14のゲーム仕様・アート・製品コードの代理実装。製品工程は既存統括が所有する。
- UI全面改造、ターミナルを覆う新しい結果card、タブの追加による回避。
- 新Run、台帳の直接編集、guard/pin緩和、承認捏造、公開・push/merge・原本削除。
- 基盤修正を理由とする稼働担当・検証の強制中断。

## 3. 現状とギャップ

### 3.1 観測基準

2026-09-29の実履歴、既存計画、基盤候補
`/home/satotakumi/orca/workspaces/hell-workers/orca-abcd-expansion/scripts/`を照合した。
計画時点のTAK-14は同Run・generation12で、実装A revision5承認後の統合検証中。
稼働状態は変動するため、この記述を次回の稼働証拠に流用しない。

「実装済み」は個別経路のコード・試験・実適用の記録があることを表し、
全経路の運用可能性・配備完了・TAK-14の完成を意味しない。

### 3.2 個別に対応済みで再利用するもの

| 項目 | 根拠・現状 | 本計画で残す契約 |
| --- | --- | --- |
| 文書と実装の混在差戻し | M5/M9、文書commitとloop保存の回収 | 全指摘、文書blob、source、固定reviewを維持 |
| scheduler競合・検証終了記録 | M6/M7/M10 | 同一callerの待機、取得後の所有/source/control再照合 |
| bootstrap・旧bridge・レビュー形式 | M4/M8/M11 | 現起動bridge、正式settlement、未送信形式エラーの訂正 |
| 終了済み工程と作業場の寿命 | M12 | 最終receiptに依拠し、撤去済みworker checkoutを再要求しない |
| scope不足・失敗した全体監査からの再計画 | M13/M17 | 失敗履歴と成果を保持、同Run、権限と受入条件を維持 |
| Help更新後のsource継承・検証失敗の復旧 | M14/M15 | 正式な新subjectと再検証、旧worker承認の捏造禁止 |
| Help結果の表示enum | M16 | producer値をUI境界で変換し、未知値は拒否 |
| 複数所有者の段階補正 | M18、全tooling1356+164、固定reviewと実配車 | 実装成功でnative/assets/docs未達を消さない |
| primary検証入口の案内 | M19、関連81 testsと固定review、実復旧 | **案内修正まで**。別版での登録自体を防ぐ実装は残る |

個別実績は[配車・復旧計画](orca-dispatch-submit-plan-2026-09-28.md)に残す。
完了済みを削除して再実装するのではなく、以下の共通契約へ接続する。

### 3.3 恒久修正一覧

| ID / 優先度 | 確認済みの事実 | 恒久修正と残件 |
| --- | --- | --- |
| S1 / P0 | 送信がinput_acceptedのままでも後で実turnが始まる。`/exit`はtarget_unverifiableとなり入力が残った実例がある。既存計画でも着手証拠連携は未完 | 送信receipt、composer、provider turn、次操作の照合を正式APIに集約。通常promptと終了commandを区別し、未送信・未知・開始済みを判別する |
| S2 / P0 | 製品worktreeの相対dev.pyが古いcontroller hashで登録し、hostのinspectが拒否した。M19はpromptで絶対pathを指定 | 登録をhost側canonical serviceへ統一。登録前にcontroller契約を確認し、誤った版のbatchを台帳へ作らない |
| S3 / P0 | 既知の復旧helperが増え、統括が経路不足を報告して待機する事例が繰り返された。M18/M19も監視側の指示を要した | 既知のエラー分類→guard付き復旧→同一依頼の次操作確認を共通dispatcherへ接続。自由文の手動解釈を通常経路にしない |
| S4 / P0 | 実装・調査・native・文書などの途中成功と全依頼の受入は異なる。段階補正では全指摘保持を実装済み | 依頼単位の未達条件と次工程を唯一の終了判定へ統合。Linear/GitHubのDoneは完成証拠にしない |
| S5 / P1 | 稼働Driverが旧moduleを保持し、同sessionの正常再起動を何度か必要とした。controllerはexact hash pin | app/CLI/Driver/helper/schema/recipeの互換manifestと安全な更新handshakeを整備。稼働中・次回反映・未対応を可視化 |
| S6 / P1 | UIがqueuedの間にも統括が次工程を調査していた。これだけで配送未達とは断定できない | 通知配送・統括作業・後続登録を分離して投影。稼働証拠に合う既存状態行と、該当する最新terminalへの移動を保証 |
| S7 / P1 | GPU選択値`Intel`と観測名の完全一致要求で実機検証がinvalid。collector/verifierがあっても実行recipeが不足する場合がある | 検証能力の事前確認とproducer/verifierの共通契約。GPU不具合そのものとは断定しない。製品固有補正は既存統括へ戻す |
| S8 / P1 | registered/needs-finalize中のstorage checkは拒否する。個別修正の検査と稼働検証の時間境界が衝突した | 正常な実行中/終了処理中と孤立を区別。ownerと具体的な次actionを示し、終了証拠後のseal/finalizeを正規経路で継続 |
| S9 / P0受入 | 多数の単体試験と個別実復旧はあるが、すべてを繋ぐ無介入受入の完了証拠は揃っていない | 非ゲームの一貫E2Eと障害注入を必須化し、「修正済み」の範囲をコード・配備・実受入で分ける |

S1のprovider別原因、S6の投影遅延の内訳、最新版Orcaとの互換性は追加調査対象。
観測だけで全原因が確定したとは扱わない。S7のGPU契約修正は計画時点で既存統括が検証中であり、
ここから二重配車しない。

## 4. 実装方針

### 4.1 共通の操作契約

各操作に安定したoperation IDを持たせ、最低限request/Run/node、source、owner、
terminal incarnation、control世代、入力digest、効果の種類を束縛する。
prepared→accepted→started→appliedを分け、rejected-before-write、unknown、
failed、intentional-pauseを成功へ丸めない。既存receiptを移行元として再利用する。

「exactly once送信」を一般に保証するのではなく、同ID照会・冪等な確定・副作用の証明により
二重実行を防ぐ。未知の副作用は自動再送しない。残存composerは全文・所有・incarnationの
一致を正式に証明できた場合のみ、同operationのsubmit回復として扱う。

### 4.2 権限・承認

既存許可内の復旧を内部都合の再承認待ちにしない。新たな仕様・公開・不可逆な操作だけを質問する。
利用者停止は入場時・副作用直前・保存直前に検査する。検証や固定reviewを迂回しない。
レート制限・資源busyは次の実行可能条件を待ち、再試行回数だけで恒久停止にしない。

### 4.3 作業場と配備

基盤修正は既存の同目的branchを再利用。既存未commit変更の所有とdiffを先に確定する。
計画時点の基盤比較baseは`ae5b2066c4fd39a8dd6a6c439e272d125f749241`。
実装開始時にbase/head/sourceを再取得し、値を無条件に引き継がない。
正本文書はprimary、製品subjectは凍結、旧subjectへ最新helperをコピーしない。
Orca本体変更は別build検証後に保全切替。通常起動は既存launch-supervisedを維持する。
公開・PR・push/mergeは別許可。Bevy API変更は本計画の対象外。

## 5. マイルストーン

### H0: 現状・境界の固定

- 対象: 既存計画、起動経路、各helper/receipt/schema、稼働版の一覧。
- [ ] S1〜S9を「実装済/配備済/実受入済/未確認」に分けた実装台帳を確定。
- [ ] 共通operationの型と互換方針を先にレビュー。稼働状態の一括書換えはしない。
- [ ] 非ゲームE2E fixtureと失敗注入点を用意する。

2026-09-30の反映境界（TAK-14の完成状態とは別）:

| 契約 | コード | 稼働中への反映 | 実受入の範囲 / 残件 |
| --- | --- | --- | --- |
| canonical host登録 | H1-d固定review・全tooling合格 | host APIと同統括session復帰を確認 | 同Runの実batchでhost登録・実行・finalized pass・固定review再投入まで確認。全障害注入はH5に残る |
| planningのreceipt/後続証拠 | H2-a固定review・全tooling合格 | 同統括sessionへ反映済 | legacy退役は実確認。新しいsuccessor通知の一貫E2Eが残る |
| 型付き接続/資源待機 | H3-a固定review・全tooling合格 | 同統括sessionへ反映済 | 実障害注入の一貫受入はH5で扱う |
| 判断待ち通知と安全なhelper選択 | H3-bの限定3型local recoveryとH3-cの3型attentionはDriver接続済 | 監督controller・同統括Driver反映済 | Help等を含むcategory横断接続と無介入一巡は未完 |
| 版互換・安全切替 | 既存refresh/loaded pinあり | H1でrefresh使用済 | 全依存manifest・rollback・cold startをH4で一貫検証する |
| UI履歴/折畳み/状態投影 | 既存実装を維持 | 既存build稼働 | H4の別build画面5シナリオ・独立verifier・画像4枚確認済。最終H5 buildとの比較が残る |
| 全依頼受入 | 既存request scopeとfixed reviewあり | 個別経路は稼働 | H3未達barrierとH5全シナリオが残る。工程承認を依頼Doneへ読み替えない |

### H1: 検証登録の単一入口（S2、S7の事前確認）

- 対象候補: `orca_native_bridge.py`、`orca_native_recipes.py`、`dev.py`、`validation_storage.py`と各tests。
- 登録要求はbatch specと既知recipe/subjectを受け、hostが承認済みprimary controllerを選ぶ。
  任意commandや任意helper pathの実行APIにはしない。subjectのcommand/verifierは版を固定する。
- CLIの相対入口はcanonical serviceへ委譲するか、書込前に修正可能な型付き拒否を返す。
- [x] primaryと製品が異なる版でも、誤ったcontroller登録が0件。
- [x] controller交換、symlink、別repo、owner不一致、source変更、応答消失を拒否/照会できる。
- [x] 登録済み未実行の旧batchは正式終了と置換に限り移行。実行済み/unknownは再送しない。

固定reviewの棚卸しで上記を既存証拠により確定した。CLIの実Git linked worktree試験、
`test_orca_native_registration`のreal socket応答消失・同一登録照会、
`test_orca_native_bridge`のunknown/timeout再送禁止・旧controller履歴の再起動拒否、
下記TAK-14のhost登録からfinalized passまでの実績を対応させる。H5の横断受入は別に残す。

#### H1-a: CLI入口の実装（2026-09-29）

候補checkout `orca-abcd-expansion` の `scripts/dev.py validation` は、新しい
`scripts/validation_entry.py` を通し、Git common directoryからprimaryの
`scripts/validation_storage.py` を選択する。subjectのcwd・引数・終了codeは維持する。
missing、symlink、所有/mode不正、別repo、別primary指定では実行前に拒否する。
`GIT_DIR` / `GIT_COMMON_DIR` / `GIT_WORK_TREE` によるGit文脈の差替えと、
別primaryを指す `HW_VALIDATION_PRIMARY` の継承も拒否する。
失敗時に旧worktree helperへfallbackせず、起動済み操作を自動再試行しない。

- 実装: CLI入口のみ。host brokerの登録API・recipe事前承認・応答消失時の同ID照会は未実装。
- 配備: 候補checkoutで利用可能。凍結した製品worktreeやprimaryのdev.pyへは未適用。
  稼働中の統括/Driver、controller pin、台帳、成果は変更しない。
- 試験: 異なるcontrollerを持つ実Git linked worktreeを含む関連21 tests、Ruff成功。
  候補の `dev.py validation check` からprimary台帳へ到達してpassを確認。
  新規batchの実登録・host起動のE2E成功とは扱わない。
- 固定read-only review: H1-aの3ファイルに限定してAPPROVED。
  SHA256は `validation_entry.py=3c8a118cc2217d0157fdd5cb3cd0d8f599fd17777ba9a7775207ab54baed6e42`、
  `test_validation_entry.py=c3c78d9aba43a909c59c77f600d2333dd6f5efde7e5402bc21708cad56dbe754`、
  最初の承認時の `dev.py=047ee4423f81c3f6b40d402706976aea44c61127b86d455b298bab910ce99e87`。
  reviewer独立実行の12 testsとdiff checkも成功。
- 比較base/headは `ae5b2066c4fd39a8dd6a6c439e272d125f749241`。
  同baseを指定したcontractsは成功。全toolingではrouter呼出し試験が1件失敗した。
  全suiteがscriptsをsys.pathへ追加した場合の二重module読込が原因で、dev.pyをpackage有無に
  よる明示importへ変更。修正後は同条件を再現した関連21 tests、Ruff、別tooling164 tests、
  perf self-testが成功。失敗した全toolingは成功へ読み替えない。
  後続の全tooling再実行は1,369件＋164件が成功し、H1-aのimport補正を確認した。
  この実行中にH1-bを追加しているため、H1-b最終版を含む単一sourceの全suite成功ではない。
  H1-bは下記の最新sourceに対する関連64件と固定reviewを別証拠として扱う。
  修正後 `dev.py=b940d1721ba21d69dbe3a9af085ffdd5da0fe681d5466a1b015b8d4ef289ed62`。
  この最終hashでも固定read-only reviewはAPPROVED（独立12 tests成功）。
  候補source fingerprintは `32c03e2848bee7b6515830def21e0dac5e5a300c86f8b64c4856f8e0e2b699e8`。
  最終storage checkは159 batch・666209239040 bytes・未分類0でpass。
  本修正で新規製品batch、binary copy、作業場を作成せず、既存成果・cacheは削除していない。
- Help: No impact。開発CLIから検証controllerを選ぶ経路だけを変更し、ゲームの入力・
  成立条件・表示・Help catalog・runtime assetsは変更していない。
- `ci check --mode auto` はdev.py変更をquality-controlと判定しRust群も選択したため、
  tooling途中で中断（exit 130）。完了証拠に使わず、対象のcontracts/toolingを別途検証する。
  本計画のゲームbuild/test除外を維持し、全CI成功とは報告しない。

#### H1-b: 保存前のhost検査境界（2026-09-29）

候補の `validation_storage.register` にhost所有のin-process `preflight` を追加した。
CLI/specからcallbackを選択・import・実行するAPIではない。
既存呼出しは引数省略で互換を保ち、指定した場合だけ次を保証する。

- spec/commandをdeep copyし、controllerが確定したsubject・hash・root・verifierを検査する。
- ledger lock内・保存前に、さらに独立copyをcallbackへ渡す。callbackは検査のみとし、
  外部起動・保存・storage lock再入は禁止する。
- 例外、非None戻り値、candidateの変更は登録拒否。batchとhistoryはどちらも未保存。
- 検査後にroot不在と通常admissionを再照合。source/helper/controller/policyの変化を拒否する。
- 既存IDはcallbackを再実行せず拒否する。応答消失を自動再登録する許可ではない。

関連64 testsとRuffが成功。内14件が新preflight試験で、実際の宣言的recipe validatorと
無害なcommit済みfixtureを接続した登録成功・誤verifier拒否も含む。fixture commandは起動しない。
storage既存29件、CLI入口12件、dev9件も同じ実行で通した。
最初のhelper変更試験は、拒否自体は成立したが期待するerror文字列が誤っていたため失敗。
helper hash拒否とsource fingerprint拒否を別試験に分離して再実行した。

production hash `validation_storage.py=cddfde11e4b8110491160b7b7e8232e98da714e862bab79cf56908712d2c73d6`
は固定read-only review承認済み。対象baseを指定したcontractsも成功。
最終test hashは `dd7db0a79334acbb763cb8a9499d9ca4ec581a4fce344b3843f0e0730834fa1f` で
固定review承認済み（独立14 tests成功）。候補source fingerprintは
`e86eea0d660779a08809d39cf4d7ab5b307ea4a98b6ca4c128fc27fd939c1ff7`。
HelpはNo impact（開発用検証登録だけで、ゲームからの到達経路や表示は変えない）。

**未配備・未接続:** primary controllerとlive broker/pinは一切変更していない。
hostの本人確認、request/Run/control/sourceの拘束、known recipe選択、同ID照会を行う
broker登録actionは残る。callback単独を、それらの認証・権限保証と説明しない。
primary controllerの更新は旧版を読み込んだbrokerと既存batchのhash契約へ影響するため、
S5の安全な版切替・履歴互換と合わせて配備する。旧batchのhashや台帳を書き換えて対応しない。

#### H1-c: host登録transaction（2026-09-29）

候補の `scripts/orca_native_registration.py` に、認証済みbrokerから呼ぶ登録・照会処理を追加した。
独立serverやcommand実行器ではない。live `orca_native_bridge.py` は変更していない。

- native→scheduler→storageの順でlockを取得し、caller、request/Run/generation、repo、
  owner、可視統括terminal、operator control、controller版、recipeを保存前に照合する。
- canonical repo/rootと限定specを要求し、内部metadataの注入を拒否する。
- 保存直前にもcaller/control/subject/recipeを再照合し、H1-bの再admission後にだけ登録する。
- scheduler待機前だけでなく取得後・storage lock前にもexecution connectionを再観測する。
  待機中のLinear完了/取消・接続失効ではstorage登録へ進まない。
- 要求のdigestと内容を同一batchへ保存。同ID・同要求の応答消失は照会として扱い、
  再登録・command起動を行わない。保存された要求とbatchの不一致は成功として返さない。
- 履歴照会では現在のhelper/outputの存在を要求しない。正常終了後の削除を妨げず、
  現在のpause中も認証済み照会を許す。不存在は再実行許可ではない。

関連227 testsが成功（新規transaction19、preflight14、storage29、入口12、
既存bridge/continuation/recipe153）。Ruffとdiff checkも成功。
検証は実storage/recipe validatorと一時Git fixtureを使用し、liveの権限・状態取得はmockする。
実socket認証・実Orcaからの登録・command起動のE2E受入とは扱わない。
固定reviewで待機後のexecution connection再観測不足を指摘され修正した。
その回帰testは最初のmockで関数署名が失われ、対象経路より前に拒否されて失敗したため、
autospecで署名を維持して227件すべてを再実行した。最終固定read-only reviewはAPPROVED。
reviewer独立のregistration/preflight33件とdiff checkも成功。承認対象hash:

- `orca_native_registration.py=a91a6d1b437685efb2f9dd72b729f028b4d0461dc405b909bdcd09c75cb1ae74`
- `test_orca_native_registration.py=ea1b2b9559ba7534bca145ba4e3363efee7f43a6d7813ea387a0e2f8736d45d4`

Help影響はNo impact。開発用登録要求→storage metadataの経路だけであり、
ゲームの入力・成立条件・表示・runtime asset・Help consumerへ接続しない。
primary storage checkは159 batch・666351480832 bytes・未分類0でpass。
新規の実案件batch/作業場/binary copyは作成せず、既存成果・cacheを削除していない。

**残件:** 稼働endpointとclientへの接続、loaded controllerの版証明、旧batch/receipt互換、
安全点での配備、同一subjectの全tooling・非ゲーム実受入。
live bridgeは自身のdisk hashをreceiptへ結び付けるため、稼働途中に編集して接続しない。
controller pinの緩和や旧台帳の書換えでは解決しない。H1全体は未完。

#### H1-d: socket接続と安全点でのcontroller反映（2026-09-30、検証中）

bridgeの認証済みsocketからregistration transactionへ接続し、context/register/status clientを追加した。
contextは現在subjectとloaded controller版を返す。CLIは同じintentを照会し、自動再送しない。
socket peer credentialsを使うfixtureで登録・照会・応答消失後の単一登録を確認。
固定reviewで指摘された直接登録marker欠損、別request、同HEAD旧generationの起動を拒否し、
native wait CASのscheduler lock内でも登録subjectを再検査する。
controller diskとloaded版、bridge/registration diskとloaded版の差も拒否する。

TAK-14には現在工程の正常完了後だけ安全待機を依頼。統合検証成功・固定review結果の保存後、
correction_requiredで新規配車なしを確認し、同ターミナルの `/exit` による正常終了と旧PID終了を確認した。
全native batchがfinalizedで稼働brokerがない状態でprimaryへ登録preflightを反映した。
primaryは候補の従来heavy wrapperを持たないため、最終反映hashは
`96491adeaabed6017eb15f7564a3c3ee6399344ea084e2c779b68bc3d4b10490`。
対応する二版をexact pinで区別し、任意版の許容や旧batchのhash変更は行っていない。
固定reviewでの直接CLIによるmarker持込指摘に対し、storage所有の`host_admission`を追加した。
予約fieldの入力拒否とcallback後だけの証明発行を、実CLIを含む回帰試験で確認した。
最終productionとtest差分の固定read-only reviewはAPPROVED。関連227件、追加後の登録32件、
primary storage29件の試験が成功。
レビュー修正前の全toolingはSIGINTで中断し、成功証拠には使わない。

最終source `67810e68936146cb925c4d4987f0a457403b7d39ca68b3ca7becb6de4ce384c1`、
base/head `ae5b2066c4fd39a8dd6a6c439e272d125f749241` でcontractsとtoolingがexit 0。
1,417件＋164件、Ruff/actionlint/perf self-testが成功し、実行前後のsource一致を確認した。
primary storageは159 batch・666473631744 bytes・未分類0でpass。実案件batchや作業場を増やさず、
既存成果・cacheの削除はない。ゲームbuild/testは実施していない。
表示サービスはexact reviewed source `c16bdb8419d41258b896d7742235ca07026d8286762abe27c81dde71243089b2`
への正規refreshで同PIDのまま更新し、running/errorなしを確認した。
同統括session `01a0d377-feef-7520-bef5-50565ef282cb` と同tabを再開し、providerの新turn開始を確認。
実統括がcontext/capabilitiesを読み、期待controllerと3 registration actions、同Runのsubjectを確認した。
live bridge revisionは `24d4ce1dc8dc58bc1282924b3f5accffba9eaafde88ac9a83b1bf0759ea8e10e`。
Driver PID367350はrunning/errorなし。全3 findingsを保持した修正計画を作成し、正規routeとwatchへ進んだ。
これはH1のAPI接続・同会話復帰の証拠であり、新native batchの実登録/完走、H2〜H5、製品完成の証拠ではない。

### H2: 配送・着手・終了commandの正式照合（S1、S6の証拠側）

2026-09-30T04:47Z監視追記: runtime `542cfcbb-38a8-43a8-957c-53379bec2158`、
同統括terminal/incarnation、Run `run_3236488f0dff` / generation12を確認した。
台帳はactiveだがintegrationはcorrection_required、worker-aはapprovedで、
統括の最新turnは正式採否・ordinary-world lifecycle/performance recipe不足の報告で終了している。
terminal cursor137084以降に新しいturn/操作はなく、controller-health runningを進行とは扱わない。
不足producerを既存scopeの補正工程として照合する新案内を送ったが、operation
`dd0ba73f-53e4-4304-92cd-8beeaf04a1da`は`terminal_prompt_target_unverifiable`で失敗した。
同tabへswitch後、同ID照会は`operation_unknown`を返した。previewには案内本文が残り、
cursorは137084のままである。配送成功・未送信のいずれも補作せず、再送/Enter/新Runを行わない。
このoperationと残存composerは次の正式回復の対象として保持する。再開確認は未達。

継続確認: 同terminal/incarnation・runtimeで残存入力を確認した。候補の隔離入力proofは同rootに
対して4回とも同fingerprintを返したが、失敗時の入力状態を証明するものではない。
旧operationの再送やEnterは行っていない。

2026-09-30T05:38Zのscreen読取でも同じ案内が`draft`に残り、新規terminal出力はない。
固定reviewによる現行経路の再確認では、legacy NULL receiptを安全に自動取消する入口はない。
実装A用のpre-terminal取消は統括のcomposerには適用しない。通常のinterruptはdurable prompt
mutationの対象外であり、使う場合も明示operator取消として別記録し、元operationを成功・未送信へ
変更しない。統括の履歴保全・同会話復帰を伴う取消は既存の復旧指示の範囲として扱い、
追加の許可待ちを停止理由にしない。
恒久経路は既存mutation executorとexecution-host input fenceへ接続する。
旧daemonにepoch/composer能力がなければ、新appだけへの更新で条件付き取消が使えるとは扱わない。
停止理由は許可不足ではなく、H2/H4の正式な取消・再開経路の残実装である。

2026-09-30追記: 本体候補の既存execution-host input fence/composer/RPC経路に
`cancelVersion: 1`の能力交渉と条件付き取消を追加した。非空の観測composer、input epoch、
output sequence、bracketed-paste modeを最終flush後に照合し、一致時だけCtrl-Cを1回送る。
旧hostは能力不在で副作用前に拒否し、通常writeへfallbackしない。送信後の例外・応答消失は
unverifiableで、消費済みepochを再利用しない。RPC/flush競合/例外/通常promptの関連4file
65 testsとnode typecheckがpass。これは取消transportの検証であり、取消前の全文保全WAL、
旧operationとの関連付け、provider同一性照合、本体配備、同依頼の再開確認はまだ未完了。
Help影響はNo impact: Orcaの開発端末入力だけを変更し、ゲームの入力・表示・保存経路を変更しない。

同単位の固定read-only reviewはAPPROVED、oxlintとdiff-checkもpass。
既存復旧指示に基づく旧版移行処置として、同PTY/incarnation/session/process birth/親子関係と
非空draftを直前照合し、全文・保持済みterminal履歴・元operation状態をprivate directory
`/home/satotakumi/.local/state/hell-workers/operator-cancel-20260930-iwG3OD/`へ保全した。
`terminal.send --interrupt`を一回だけ送信し、accepted/1 byteと、その後の実screenで
空の入力欄（Ask Codex to do anything）を確認した。元operationはpending/unknownのまま保持し、
再送・Enter・ACK補作・Run新設はしていない。これは明示operator取消の移行処置であり、
条件付き取消の配備・自動回復の受入証拠ではない。cursor137084は不変で、新turn・次操作は未確認。
保全情報はこの取消から同会話復帰までの照合用として保持し、復帰確認と最終受入後に用途を再判定する。

2026-09-30追加: legacy NULL receipt専用の取消WAL/API/CLIを本体候補へ接続し、
固定read-only reviewはAPPROVED。元terminal.sendのcaller・binding・pending/NULL・
in-flight不在を再照合し、観測draft全文をcancel_armedへ保存してから条件付きCtrl-Cを行う。
取消IDはcallerと元operationから固定生成し、既存取消は照会のみ。空composerと同providerの
正の観測がない限り成功にせず、元operationのunknownは変更しない。
取消記録が消えると再取消できる問題を防ぐため、元operationがpendingの間はage/capacity pruneと
旧binaryのDELETEから記録を保持する。schema42の専用移行で既存v41 DBにも保護を追加した。
CLIは能力交渉・inspect・同ID mutation・redacted照会を行い、cancelled以外はnonzero。
利用者に内部ID入力を要求する受付変更ではない。
同一sourceの関連11 files / 128 tests、node/CLI typecheck、RPC catalog検査、対象oxlint、
diff-checkがpass。容量保持とv41→42移行、応答消失、再起動後再試行、元record不変を含む。
取消本体SHA256は`01813fbf10373043b4d000b0dcfe2b6bf8924abe3c354b25d63887683675ed6c`、
RPCは`698faa465341ad02039830c0426ed4a6bade9ddcc9eb248d4e4d72b3d33fe4df`、
retentionは`dfe505830a4824ad0bad6173270114c7b500839b4d87d6f3b75e9c1ac6978782`。
Help No impact: Orca開発用CLI→RPC→端末入力・操作記録のみで、ゲーム入力/UI/保存/Helpは不変。
primary storageは173 batches・未分類0でpass。候補の配備、controller接続、typed exit、H4/H5は
未完。実terminalのcursor137084に増分はなく、取消APIの承認をTAK-14再開とは報告しない。

追加の別build回帰 `orca-hardening-prompt-20260930-03` は、背景Electronで3/3 pass
（retry/skip/flakyなし）。source/build fingerprint
`2986fa3aaa80d5e4513ef45fb12afb0b44d4fb36a2d1b2db0d59fea500128f90`で独立verifierと
primary seal/finalizeがpassした。対象は模擬providerの遅延paste、Enter非再送、permission時0 bytes。
実隔離provider・取消APIの実daemon受入・配備・全loopはこの証拠の対象外。
report 12288 bytesは同candidate `.git/orca-ui-acceptance/hardening-prompt-20260930-03`で
H2実provider受入との比較用に保持し、最終結果への置換時に用途を解除する。
旧buildへの依存はない。配備候補は同candidateの
`.git/orca-ui-acceptance/package-hardening-cancel`へ生成し、ownerは基盤統括、consumerは
H2配備前受入。配備/復帰先として採用しない場合は受入終了時に整理する。

- 対象候補: Orca本体terminal send/receipt/provider detection、`orca_native_continuation.py`、
  `orca_dispatch.py`、`orca_ui_coordinator.py`。本体のexact pathはcheckout特定時に記録する。
- [ ] paste遅延、通知・permission UI、隔離Codex、shell、`/exit`、restartを別々に試験。
- [x] 通常promptのsame-ID replayは再入力せず結果照会。acceptedだけをworkingにしない。
- [x] 通常promptでturn_started未観測でも、正規の同operation後続証拠を得た場合は照合可能。
- [ ] 証拠不足時は原因と安全な次actionを返し、手動Enterを通常手順にしない。

#### H2-a: 工程継続通知の証拠強化（2026-09-30）

2026-10-01追加（候補・未配備）: planning WALとquarantine envelopeのversion検査が
Pythonの数値比較により`true`/`1.0`をinteger `1`として受け入れる欠陥を補正した。
実advanceから生成したWALで修正前の2負例失敗を再現し、exact int化後は原本byte不変の
quarantine・再送なし・raw保持を確認した。関連74 tests、対象Ruff、diff check、固定read-only
reviewがpass。production SHA256は`259bcfe7b6b3476ad5051cde68edb4121a9609d5d429046371047a8d4db9fd0e`。
Help Skill判定はNo impact: 開発用継続通知記録の型検査だけで、ゲームの入力・UI・条件・assetsは不変。
primary storageは177 batches・未分類0・669380403200 bytesでpass。新規job、実送信、台帳補正なし。
この欠陥をTAK-14停止の直接原因とは確認していない。nested全体の互換検査、配備、再開は未完。

固定read-only設計reviewで、nativeの厳密receipt検証とplanning側の弱い検証の差を確認した。
`orca_prompt_delivery.py`へ入力を送らない純粋validatorを抽出し、planning側へ接続する。
最初の送信前にterminal incarnation・Run/generation・predecessor・head/source・operator controlと
本文digestを保存し、再照会で一致しない場合は新入力を行わない。first raw receiptとlatest readbackを
別々に保持し、acceptedとstartedを区別する。送信後の観測喪失はreceiptを残して同ID照会へ戻す。
利用者pause時は入力を送らず、pause自体をDriver失敗にしない。
固定reviewの指摘から、WALを読む時点のcanonical検査、送信直前fence、runtime UUIDとterminal
incarnationの区別、正式predecessor archiveに結び付いたapplication proofを追加した。
旧/破損recordのquarantineは読み取り投影だけとし、別経路のconnection更新でも元planningを保つ。
旧appliedは、同じtarget bindingの正式登録と全lineageを確認できた場合だけ、旧全文と登録済みloopを
private historyへ保全して退役させる。旧accepted/unknownを成功へ変換せず、通知を再入力しない。
移行lockはscheduler→requestへ統一し、実flockの2-thread競合とcrash後のloop変更拒否を検証した。
関連44 testsとRuff、最終固定read-only reviewはAPPROVED。後述のH3-aと同sourceで配備した。
全toolingはmock Cargo/build試験が実accountのheavy lockを使い、TAK-14の検証と競合したため
失敗を確認してroot所有の試験だけを中断した（exit 130）。製品検証は中断していない。
試験fixtureのhost/activity/lane authorityとworkspaceを分離し、実flockの競合・解放は維持する。
production guardは変更しない。親の5環境値の除去とcleanup後復元を含む関連36件が成功。
固定reviewのlane FD除去指摘にも対応し、同一sourceの全tooling 1,447件＋164件が成功した。
稼働中nativeのpinを変えないため、native側の共通validator呼出しへの置換は安全な反映点まで行わない。
`/exit`の別効果契約、既存composerの正規回復、H3の未達barrierはこの単位の完成に読み替えない。

#### H2-bの設計境界（専用終了操作は未実装、起動記録候補あり）

終了は通常promptのturn開始では証明できない。能力交渉済みの終了操作として扱い、hostが
literal `/exit`だけを送る。旧provider/launcherのPID出生と終了、同じPTY・root shellへの復帰を
照合する。PTY自体の終了は要求しない。launcherの`exit_code=0`だけではhandoffによる終了と
自然終了を区別できないため、raw child return codeと終了原因を別々に保存する。
観測は操作前に開始し、再起動後のPID不存在や画面文字列だけで終了receiptを補作しない。

2026-09-30固定read-only設計reviewで、最小接続点を確認した（未実装）。
本体の専用typed mutationがliteralを構築し、通常promptのturn_startedとは別の
bound→effect_possible→exit_observed→completedを既存durable契約へ束縛する。
`linux-isolated-codex-prompt-proof.ts`の二重読取process chain/birthは再利用できるが、
launcherをargv推測で特定しない。統括registryがlauncher・spawnしたprovider-rootのPID/birth、
launch/operation bindingを正規記録し、appのhost proofと照合する。
既存handoffはterminate/killしたchildでもexit_codeを0へ変換するため、raw_child_return_code、
exit_cause、transferredを別記録にする。schema1 exited/0を自然終了へ昇格させない。
Python launcher自身は自分の終了後を観測できないので、外部finalizerが以下を全て要求する。

- 事前に束縛したproviderとlauncherそれぞれのPID/birthの自然終了を正に観測。
- registryの同operationでraw child 0、exit_cause natural、transferred false。
- 同じPTY handle/incarnationが生存し、同じroot shell PID/birth/tty/session/repoが
  sole foreground idleへ復帰。既存idle_shellとpreflight_pauseの照合を再利用する。

PID reuse、chain/PTY/root shell差替え、handoff/signal/nonzero、片側だけの終了、
registryだけ/shellだけ成立、観測開始前の不在、app再起動後の不在を全て拒否する。
effect_possible以後は同ID観測だけで再Enterしない。旧capability・remote/WSL/SSH・複数agentは
送信前に拒否する。通常prompt WAL合格をこの専用終了契約の合格とは扱わない。

追加の読取reviewで、現行`launch()`が`ui-coordinator` lease解放後に終了registryを書込む
競合窓を確認した。終了保存まで同leaseを保持し、後続launchのstartingを旧finalizerが
上書きできない順序へ候補修正済み。異なるrequest/terminal/worktree/repo/created_at/Linear識別と
既終了recordは上書きせず拒否する。初回ui-coordinator入場前だけをlaunch_waitの再試行対象とし、
入場後のHostBusyからproviderを再起動しない。最終coordinator lockの取得前busyだけは同じ
publicationでdeferし、取得後の読取・保存失敗を盲目的に再試行しない。
旧コードで再現試験を失敗させた後、関連96件・Ruff・固定read-only reviewを通した。
当該ui helper SHA256は`e3ef59f77ee855daf5093efbf89d94fb9685e6d6d51463167c150f8c769363f8`。
これは未配備の限定修正であり、schema2/typed exit全体の完了ではない。
schema2記録ではlauncherとPopenが返す外側sandbox-rootの
PID/birthを区別し、後者をinner Codex providerの終了証明へ読み替えない。
schema1は原本のまま読取互換を残し、schema2のnatural/signal/handoffとraw return codeを
exact検証する。新しい自然終了承認にはschema1のexit_code 0を流用しない。

schema2候補を追加した。`orca_coordinator_activation.py`と通常launcher経路で、fresh activation ID、
launcher PID/birth、Popenが返す外側sandbox root PID/birthを記録する。childのpoll/wait前に
birthを採取し、高速終了した未reap zombieも区別する。記録失敗時は自分でspawnしたchildを
停止/reapし、starting記録から自然終了を補作しない。終了時は同activation全fieldを照合し、
raw child return code、natural/signal/handoff、transferredを保存したうえで互換exit_codeを出す。
schema1は原本のまま読取る。監督の正常終了表示・終了後close/pauseとmaintenanceの入場は
schema2 natural/0/not transferredを要求し、legacy/0やhandoff/0では許可しない。
既存validation/help fenceはschema2 exact transitionを追加した。明示allow_legacyは過去の
interruption-accounting経路の互換だけで、typed exitの正の観測へ昇格させない。
追加reviewの4指摘（fence readerとの不一致、未確定activation上書き、birth形式、reader例外型）を
修正した。schema2のstarting/readyは新activationで上書きせず、非終了時のexited_atを拒否する。
prepareは現在launcherのPID/birth、bindはその直接childと二重birth読取を照合する。
両fenceのobserveとcheckedが同じexact transitionを使い、記録のlauncher/provider-rootと
観測したprocessのPID/birthも一致を要求する。validation/helpの実reader往復、handoff・signal・
birth差替え拒否を追加し、関連201件・Ruffがpass。一度限定承認されたが、追加確認で
common transition helperがbefore=readyを必須にしていないことを確認し、一時保留とした。
共通helperでbefore ready/未終了とschema2 ready activationを必須に修正した。
Help/validation両readerの改変前提拒否を追加し、関連100件pass・固定read-only再review承認済み。
activation SHA256は`e9077d43c09ebc1301f99ef353a281d586fcd33767561bc65e6d099a72c7cccf`、
ui helperは`36885d9dd0b06cb88f33b5f611fc996444ce0526f101b180c96b82d3bc0de3ee`。
前段sourceの全contracts/tooling回帰は1653件＋Blender fixture164件、perf self-testがpass。
最新の追加3test・transition差分は後段の100件による別証拠とし、前段passへ混ぜない。
Help No impact: 開発用起動・復旧の記録経路だけであり、
ゲーム入力・UI・runtime asset・プレイヤーHelpの成立条件は変更していない。
typed mutation、内側providerの事前束縛、外部observer、
同PTY/root shell帰還の一貫確認、本体接続、配備は未完であり、記録追加だけで終了操作を許可しない。

終了観測の既存2経路（validation/help fence）で重複していたpidfd待機を
`orca_process_exit_observer.py`へ集約し、両consumerへ接続した。各processのopen前後と
全fd取得後に同じlive identityを照合し、途中で先に登録したprocessが変わった場合も入場を拒否する。
pollは最大60秒、全対象のpidfd readinessが揃うまで未完了とし、部分open失敗・割込み・
caller例外でもfdを解放する。context終了後の観測再利用は拒否する。
`fence.process`も/proc複数fieldの取得前後でbirth identityを再照合する。
既存のjournal/transcript/source/external audit/activation照合は両経路に残す。
最初の28 tests（実際に起動した試験専用childの自然終了を含む）と固定read-only reviewはpass。
追加のbirth読取競合testも固定reviewを通過した。contracts/toolingの全体回帰は
Python 1672件・Blender tooling fixture164件・perf self-test・Ruff/actionlintがpass。
実行前後のsource fingerprintは
`3728747a11f62f3aeed48ecd188f88122d848f6145c77508d922b8ab8d534152`で一致した。
ゲームbuild/testは実行しておらず、fixture出力中のCargoコマンドはゲーム検証の証拠ではない。
observer SHA256は`be108e43faeb697ee22e707b75ad95f7a443abf23cfd14d6e50fc6127d119ea7`。
終了時primary storage checkは173 batch・未分類0・668314607616 bytesでpass、文書検査もpass。
この単位は終了観測の共通化であって、signal送信・typed exitの承認・配備ではない。
Helpは開発用復旧だけのNo impact。TAK-14の既存送信はunknownのまま保全し、再送していない。

composer回復は既存mutation receiptを拡張し、paste後の`draft_parked`とEnter直前の
`submit_armed`を永続化する。同operation・exact本文・入力世代・terminal incarnationの
一致をhostが証明できる未submitだけに限定する。`submit_armed`後や結果不明の操作には
Enterを再送しない。既存画面に文字列が残っているだけの旧操作を遡及して回復可能とはしない。
旧host、remote観測不能、未対応providerでは送信前に拒否し、通常入力へfallbackしない。

2026-09-30、H3-c反映準備の保守確認文でこの未対応を実再現した。operation
`350106a2-08e2-4293-a75e-b62f328a2e26`は`terminal_prompt_target_unverifiable`を返し、
統括の入力欄に保守文が残った。同ID照会は`operation_unknown`で、新規入力はしない。
現在の同rootに対する隔離Codex proofは連続60回成功したが、失敗時からの入力epochや
canonical draftを証明しないため、過去の未送信証拠として流用しない。原因を推測してguardを緩めない。
当初は保守hold `2c914f9c-ed1d-5604-97b4-6bf0a8534862`を維持した。
その後、同sessionの実turn開始・正常終了を観測し、H3-cを同Driverへ反映して正規解除した。
rootから手動Enter・入力消去・強制終了・新タブへの逃避は行っていない。
旧pending receiptを成功へ書き換えたものではなく、composerの自動回復は引き続き未実装。

同日、実装Aの再利用送信でも `runtime_error` を観測した。operation
`59f1d41a-0695-4394-ab14-ae19fd72c5a6` はOrcaの保存済みmutation receiptが
`pending/receipt=NULL`、role-tab registryが`unknown`である。既存の同incarnation shellと
出力cursor不変は、過去の入力が未実行である証明ではない。固定read-only調査でも既存の
bootstrap/input/prelaunch回復はこの証拠条件を満たさず、再送を許可できないと確認した。
既存履歴・タブ・Runを保全し、新IDでの配車や台帳書換えは行わない。

本体候補に、effectPossible後の失敗理由をpending receiptへ固定codeで保持する補正を追加した。
任意の例外本文やcommandは保存せず、同ID照会は従来どおり`operation_unknown`を返す。
完了済みreceiptを上書きせず、旧NULLから証拠を補作しない。これは診断欠損の補正であり、
起動再開・composer回復・H2-b完了ではない。関連37 tests、node typecheck、対象lint、
max-lines検査はpass。production 2ファイルとexecutor testは固定review承認、未配備。

ユーザーから履歴・成果を保全した取消と同tab再起動の明示許可を受けた。
候補の`orca_launch_cancellation.py`は旧receipt不存在のpre-Dispatch shell launchだけを対象とする。
同loop/source/role/session、正の前回終了、全既存Task/Dispatch、同incarnationのidle shellを照合し、
元状態と保持済みterminal履歴を保存してからCtrl-Cだけを送る。肯定receiptと再照合が成立した場合に限り、
同Run・同session・同tabの通常配車を再開可能にする。旧mutationのunknownを成功へ変更しない。
中断後の投影も同じ5leaseを取り、取消receiptのcomplete保存後にだけloopをactiveとして公開する。
新しい送信WALが存在する操作は、このlegacy取消では扱わない。

今後の再利用起動には`orca_role_launch_receipt.py`で送信前から同operation ID・exact本文digest・
terminal identityを記録し、初回から`--retry-request`を指定する。肯定receiptはincarnationまで照合し、
成功・失敗の返却をprivate WALへ先に保存する。例外の公開文面はredactedのまま維持する。
新attemptのtab_launch_id→launch WAL→取消receiptにより、旧unknown履歴への参照を保つ。
関連60 testsとRuffはpass、固定read-only reviewもAPPROVED。初回fixtureのdirectory mode不備による
1件失敗は0700へ補正して再実行した。実対象のread-only inspectもpassしたが、
その後、source `00366ad9f173dec3564c9a751992255418a05f6d2df95ec9a91bb1995e0774b8`の
contracts合格を確認し、監督controllerをhelper digest
`8f3332caf39671fc527f890cc5424efbf2c51eb34ccecf460aa8fac9e07d487d`へ正規refreshした。
保守instructionで新規工程を止め、旧統括PID2188273の正常終了exit 0と同PTY shell復帰を確認。
同session・同terminalでPID2638488へ再開し、ready確認後にscopeを変えず保守instructionを正規解除した。
承認された取消helperはcompleteとなり、既存terminalの保持済み2000行と旧状態をprivate receiptへ保全した。
旧mutationはunknownのまま、同Run/session/tabの通常配車に戻す`tab_retry_ready`を公開した。
同Runで通常配車が進み、Task `task_25d8a7356614` / Dispatch `ctx_d7391a46e6e1`がarmed、
laneはimplementingとなった。同A terminal/incarnationで新dispatchへの応答、heartbeat送信、
対象のcandidate export・Rust検証経路の調査開始を観測した。新launch WALはacceptedで、
取消receiptへの参照も保持している。その後同laneはvalidatingへ進み、統括Driverはrunning/errorなし。
先行sourceの全tooling 1508件＋164件はpassだが、最終sourceの証拠とは分離する。
取消receipt・新launch WAL・registry・current Dispatch/loopの実データは固定read-only reviewで
同一性・private権限・履歴参照を照合しAPPROVED。運用コードのdigestは反映後も不変。
全体回帰試験ではnative Driver単体fixtureに本番host lock/attentionへの接触漏れを発見した。
既存`isolate_coordination`とscope外attentionのmockで試験を隔離し、production guardは変更しない。
補正前sourceの全toolingは1515件中、このDriver試験1件のみ失敗（native advance未到達）でexit 1。
関連70 tests、独立69 testsと固定reviewはpass。補正後source
`43d29cfc9ab3b6e42d5389d2b052512ee620b020f5eb3b5f7eeb8edae5728826`でcontracts、
全tooling 1515件＋164件、Ruff/actionlint/perf self-testがpass（exit 0）。終了後のsource一致も確認した。
primary docs/storageもpass（164 batches、666526937088 bytes、未分類0）。
実行中helperのdigestは変わらず、追加再起動は行っていない。Helpは開発用制御・記録のみのNo impact。
取消・同tab再起動の復旧単位は完了。H2-b全体、本体候補配備、H4互換manifest、H5一貫受入は未完了のまま維持する。

#### H2-b追加: execution-host入力世代の基礎（2026-09-30、未配備）

本体候補`/home/satotakumi/tools/orca-ui-lifecycle`に、daemon Session寿命に結び付く
入力epochと同epochのCR一回送信を追加した。通常入力だけでなくstartup ingress、
shell-ready flush、DA応答、PowerShell repaintを物理write手前で数える。
比較とepoch予約とwriteは同一同期処理とし、write例外でもepochを元へ戻さない。
shell-ready待機・終了・停止処理中は送らない。別incarnationのtokenは使えない。

daemon read-only RPCで能力とepochを取得し、runtime controllerまで接続した。
旧daemonや未対応local/SSH providerへ通常writeで代用せず、能力不足を拒否する。
preflight後には同provider・epoch・所有者runtimeFenceを再確認し、非同期確認中の
所有者変更を送信許可へ持ち越さない。host応答のexact shapeと同incarnationの
sequence一増分を照合し、応答喪失・不正応答を成功や未送信へ丸めない。

実socketを使う別client入力・応答喪失・例外・CR以外の拒否、parser拒否matrix、
所有者/接続先変更を含む関連202 testsとnode型検査が成功。lintのfile長超過は
protocol/route/testの分離と委譲の整理で修正し、対象21 filesのlintはpass。
最終整理後の202件再検証と追加2件（同期再入・DA応答）・node型検査がpass。
固定read-only reviewはこの接続範囲をAPPROVED（独立108件pass）。主要承認hashは
`pty-input-epoch.ts=5dddfa7f384deea2238783d16521ddd1b905247b914ca7c8b494e3f4be2393df`、
`session-input-fence.ts=620bbf3e97cd769cf8927aba092c3ae42ad340f246bb5634d3233ff6a827c1a9`、
`daemon-pty-session-input.ts=bab8555805d1e64a9f8fec117ef83e8c25b996ebb513350cbe0f238123da8953`、
`input-epoch-operations.ts=5f3d5186076111537d0b39d00c8d0fb31637a404b10ca92322313f6b0aa66e81`。
primary docs/diff検査とstorage checkもpass（164 batches、668058279936 bytes、未分類0）。
通常prompt writer、draft_parked/submit_armedの
永続記録、composer全文証拠、typed exitは未接続であり、H2-b完了や自動回復許可ではない。

接続前のread-only reviewで、boolean pasteまたは`writeWithSettlement`の後に
別RPCでepochを取得しても、その間の別入力を取り込むためpaste固有の世代を証明できないと確認した。
次の実装は専用host-side park操作でexpectedEpoch照合・1回のbracketed paste・厳密な
afterEpoch=before+1を同じ実行主体へ置く。既存`detectTerminalComposerDraft`と
`RuntimeTerminalRead.draft`は本文抽出に再利用するが、それ単独を未送信の証拠にしない。
provider snapshotを同一epochに拘束してexact本文を照合した後だけ`draft_parked`を保存し、
Enter直前の`submit_armed`以降は再送しない。snapshotと入力の直列化確認は残件。
live app・daemon・TAK-14のSession/Run/terminalはこの変更で置換していない。

#### H2-b追加: park/composerと通常RPCのWAL接続（2026-09-30、候補・未配備）

host側のatomic parkとepoch/output-sequence拘束composerを実装した。読み取りと書込み直前に
実parser queueをflushし、入力・出力・mode・終了・所有の変化を拒否する。parkのproofは公開型でも
必須とし、main境界で不正proofをprovider照会前に拒否する。primitive範囲は69試験・node型検査と
固定read-only reviewに合格した。その後のhost byte proof追加は別の未承認差分として扱う。

通常terminal.sendの既存mutation receiptへpark_armed→draft_parked→submit_armedを保存する
候補を接続した。同request/payload/caller/terminal incarnationでdraft_parkedの場合だけCR再開を
許し、park/submitの結果不明を再貼付けしない。本文はWALへ複製せずSHA256だけを保存する。
固定reviewでstreaming dispatcherのeffect/replay callback欠落を発見し、unaryと同じadapterへ統一。
両RPC経路の同ID再開・貼付け一回・CR一回・完了後再照会を実handler試験へ追加した。

screen scrapeは省略表示・viewport・空白正規化があり、exact本文の正本にはできない。
そのためdaemon Sessionが実際のatomic parkの本文hashをafterEpochへ保持し、任意入力前に失効する
parkedProofVersion能力を追加中。既存composer検出は存在/mode/outputの確認に限定し、本文hashと
同epochを併せて照合する。draft_parkedはこの正の証拠の後だけ保存する。resumeには新pasteがないため
render待機を省き、既存のpermission/binding再照合とconditional CRへ進む。
post-park proof不成立を通常writeへのfallback許可にはしない。

host byte proofを含む10 files・178試験がpass。その後の固定reviewで、保持hashだけでは
output-drivenに別の非空composerへ変わった状態を拒否できない点を修正した。
post-park表示text/modeのdigestをdraft_parkedへ保存し、resumeとsubmit境界で同値を要求する。
本文の正本はhost hashのまま、表示digestは継続性にだけ使う。変更後の3 files・39試験、
node型検査・対象lint・固定read-only再reviewはpass。checkpointの承認hashは
`04ac54a220ec432c37907a20aab71a424b4961e72522f01edde9e6c9bf403cf6`、guarded入力は
`b55bc165279b59b6356c99cf4724bac11958420a9fd0c9610d3f7818e3e8e108`。
実daemon/provider受入、typed exit、配備、互換manifest、H5は未完了。

追加補正: 候補実装でも`draft_parked`保存が第二target fenceより後になっており、
paste後に当該fenceが拒否すると`park_armed`のまま回復不能となる経路を修正した。
hostのepoch・parked本文hash・composerを照合した直後、第二fenceの前に未submit証拠を保存する。
submit時にも保存した表示digestとhost証拠を再照合し、変更された入力を送らない。
unary/streaming両RPCで第二fence失敗→同ID再開→pasteは一回・Enterは一回を試験した。
関連42 tests、node typecheck、対象lint、固定read-only reviewがpass。
guarded-input SHA256は`ab317810f1dc03a16b69a8b190f114fd9ac4f897a2a6f3ff44a20cc108da05aa`、
writerは`79e11e8efc04e72552e45f7fa4aa82676b18cc221ed49d66353e34abdc29ee4d`。
旧NULL receiptへの証拠補作は行わず、今回停止中の旧operationが回復済みとは扱わない。未配備。

追加監査で、同PTY/incarnation/epochを維持したまま内側providerだけが置換された場合の
識別情報がcheckpointにないことを確認した。上記の限定承認はこのケースについて保留とした。
candidate checkpointをv2にし、routeが実process proofから生成するtarget digestを全phaseへ固定する。
同ID復旧時にも現在のtargetと一致を要求し、v1/NULL/identityを提供できないrouteは復旧不可を維持する。
unary/streamingでprovider差替えとidentity欠落を注入し、receipt不変・Enterなしを検証対象へ追加した。
この追加単位は固定再reviewでAPPROVED。関連96 tests、node型検査・対象lintがpass。
稼働本体への反映はまだ行っていない。
checkpoint SHA256は`0e9d5945d36c5871a817f525a1c8ba48f53774c1e89e23a72ede1e826b16626b`、
guarded-inputは`9b7be6d18de2ff6c21bfeaf4d998b1c4df78d4c9febb51cf6e8ba81026a3bc9a`。
再接続で再発行されるhandleは永続target digestから除外し、root/birth-bearing fingerprint/agent/
PTY/incarnationを固定する。同試行のbeforeWriteではhandleも引き続き照合する。
同host processへの新handleは同digest、別birthは別digestとなる負例も追加し、固定再review承認済み。
route SHA256は`7067b8362f4a9f6370d802dc51f088ff71b73f2c1d600979fc90cc5076e3b2fc`。
Help No impact：Orca開発環境の入力配送・復旧だけで、ゲームの入力・UI・成立条件・assetsへは到達しない。

### H3: 復旧dispatcherと未達条件の接続（S3、S4）

- 対象候補: `orca_review_loop.py`、`orca_request_lifecycle.py`、`orca_request_runtime.py`、
  `orca_combined_repairs.py`、既存`orca_*_recovery.py`/replan helpers。
- 既存helperを直ちに削除せず、型付きerror・証拠・許可済actionのadapterとして段階統合する。
- [ ] 既知エラーは経路選択・admission・記録・再照合を経て自動継続。
- [x] 未知エラー、source/所有不一致、利用者停止では停止し、具体的理由を保持。
- [ ] コードなし調査、文書のみ、複数所有者、監査failed、Help変更を同じ全体受入へ接続。
- [x] コード経路・回帰試験: 全未達条件が閉じるまで依頼完了・Linear Done同期を拒否。後続なしの未完状態を検出する。
- [ ] 上記barrierを含めた実Orcaの無介入一巡（H5）。

2026-09-30固定read-only棚卸しで、`request_lifecycle.completion/progress`、
`external_sync.check_completion_gate`、`request_runtime.synchronize/completed`、
`supervision.complete_worktree/finish_close`までの終了barrier接続を確認した。
ordered DAG・最後のacceptance・全criteria/predecessor・sealed review・native applied・
mail drained・attempt accounting・attention appliedと外部readbackを要求し、
通常工程承認、古いqueued Done、新規instruction、外部reopenの拒否試験が存在する。
全tooling1572件の実行と合わせてコード側の項目を確定し、H5証拠は別に残す。

自動復旧の未完範囲は全称のまま維持する。local adapterはLinear transient、inbox correlation、
review直後workspace busyの3型、attentionはlane review/validation routing・combined reviewの3型。
Help判断待ち通知、documentation/Help/tooling修復、scope/acceptance replan、validation timeout等の
横断dispatcherは未完。
明示helperの存在を自動接続済みとは数えず、未知の副作用を自動再送することで埋めない。

正規経路の二重consumer防止は実装済み。`watch`はlocal ledgerの読み取りのみ、
`decide`はscheduler lock下のreply/判断記録のみでDelivery取得・ACKをしない。
Driverの`mail.poll`と専用recoveryは同lockで直列化し、Run/consumer generation/handleを照合する。
非消費watch、未知checkの再送拒否、consumer変更拒否の回帰試験も確認した。
固定read-only再レビューにより、これを未実装とした直前の所見は撤回した。
任意shellから同handleでraw checkを直接実行する契約外の迂回は別の脅威モデルであり、
正規watch/decideの競合とは扱わない。

#### Help通知の接続（2026-09-30、候補実装・固定read-only承認、未配備）

残修正は稼働中helperを変更しないため、同base `ae5b2066c4fd39a8dd6a6c439e272d125f749241`の
Orca管理worktree `orca-hardening-next` / branch `codex/orca-hardening-next`へ分離した。
primaryのhold `orca-hardening-next-source`は残修正の検証・固定review・反映用であり、
最終受入と独自成果の統合後、他consumerがなければ解放する。初回実測30576640 bytes。

既存attentionへHelp通知と判断のapplication markerを接続した。context付きRunでは
非nullの全Help pauseを列挙し、global/owner pause・単一owner・subject/baselineを厳密照合する。
空pauseをlegacy経路へ落とさず、保存直前にcontrol/可視統括/loop/sourceを再確認する。
lane Help-only source successionは既存helperを再利用し、integration source差替えは拒否する。
save後の中断は同spec/同markerでreceiptを補完し、再保存・再判断しない。
legacy contextなしの履歴shapeは維持する。通知消失は適用証拠ではない。
focused35件pass、固定review独立16件pass・承認。最終sourceでcontracts、tooling1591件、
関連tool fixture164件、Ruff/actionlint、perf self-testがすべてpass。
検証前後source fingerprintは `c97c1a096402c303426b558a7029de3ae00abdd2e30e90ae3228970e2d6d3d93`で一致。
`ci check --mode auto`は既存dev.py変更をquality-controlとして全群へ分類したが、追加testのlintで終了。
当該lint修正後は、本依頼で対象外のゲームbuild/testを起動せず、基盤contracts/tooling群を明示実行した。
全CI群成功とは扱わない。下記H4 loader追加はこのfingerprint以後の別単位である。
承認hash: contract `a68bac09f380c6f6bf2d9d1cb086c62cb58f9d8cc9cc288c451029ce5c854f66`、
loop `7c7299656906785b1b0029e2e41a9c03fd4ed12012a3779c374b1bd762359dc2`。
Help No impact: 開発統括の確認・通知・保存経路だけを変更し、ゲームHelp本文・入力・表示・成立条件は変更しない。
この単位の承認は配備、typed exit、H4互換切替、H5全工程の完了とは区別する。

- 既存attentionの第4 kind `help_review`として接続し、通知台帳を増設しない。
  明示`help_pause.from=validating`と正規停止理由が一致する場合だけ通知する。
  bindingは停止時subject/source、Run/generation/request binding、lane baseline receipt、
  Help履歴digestを含める。同sourceで再判断が必要な場合も旧applied IDと衝突させない。
- 通知前はlive sourceを照合する。編集後は旧通知を再送せず、正規Help提出による
  application markerだけでappliedへ進める。Help判断そのものは統括が行い自動承認しない。
- `submit-help-review`は既存control/owner/source判定を維持し、Help receiptと必要な
  lane-only succession receiptを用意する。lane/integrationのhistoryを1件追加し、
  prepared route→loop単一save→succession acceptance→route completeの順に確定する。
  同spec replayは新しい判断・saveを作らず、保存後中断のreceipt補完だけを行う。
- 既存3 kindのschema/operation ID/outcomeは維持する。Help専用outcomeで最終判断の
  decision/reason/paths/source/Runとsuccession前後を束縛する。integrationにlane successionを流用しない。
- native attentionはcombined kindを抽出して一意性を判定し、native-only tickでHelpを誤送信しない。
  別reason/利用者停止では送信しない。
- 負例: baseline/Run/source/owner改変、同source再pause、history移植・欠損・重複、
  source編集後の旧通知、保存前/acceptance前/complete前の中断、応答消失replay、
  native pendingとの混在、旧3 kindのbyte互換を試験する。

現在TAK-14がこのhelper候補を使用中である。新しいschemaを使用中controllerへ部分反映せず、
検証・固定review後の安全な世代切替で反映する。この設計確定は実装完了ではない。

#### H3-a: 型付き接続待機（2026-09-30）

Driverは`ExecutionConnectionUnavailable`をgeneric failureから分離し、同じconsumerを維持して
`waiting_for_connection`として再観測する。外部状態変更・新Run・pause解除は行わない。
HostBusyも`waiting_for_resource`として記録し、runningの表示を残さない。
既存状態行へ具体的な接続/資源待ち理由を渡す。未知例外は従来どおりfailedで停止する。
固定reviewで判明した内部step/integrationの包括catchと起動時import失敗も補正した。
両実経路の接続拒否を同じDriverから再観測してloop/Runが変わらないこと、import失敗はfailedとし
tickを実行しないことを検証した。関連178件、Ruff、diff checkが成功し、固定review APPROVED。
指摘前の全tooling試行は中断（exit 130）し成功扱いしない。最終source
`cc0077a7fe46bf209552fc940deb829d5d50e5fe1cfb1e97dd2ca986da5c8f52`を固定して再実行し、
tooling 1,447件＋164件、Ruff/actionlint/perf self-testがすべてpass（exit 0）。
同sourceのcontractsはpass、storageは159 batch・666474418176 bytes・未分類0。
監督controllerは正規refreshでgeneration `8feb53b8-fa02-415b-a87e-bfb4a6418b2b`へ更新し、
同runtime・同PIDでhealth running/errorClass nullを確認した。loaded scripts digestは
`dd96652af6ed6badbaf91878f8b228499b32b635fefa5f32d61b6be55565a14b`。
既存統括が新batch未登録・未起動の安全点を報告した後、通常の`/exit`を送り、
launcher registry exited/exit_code 0、元process終了、同PTYがshellへ戻ったことを確認した。
同session `01a0d377-feef-7520-bef5-50565ef282cb`・同terminalで再開し、新Driver PID 803158、
registry ready、統括自身のacknowledge/show、Driver runningを確認した。
旧planning `15b3b22d-608e-557b-ada5-3dae33d9c7f8`は正式loopと照合してprivate historyへ保全され、
active planningから自動退役した。旧input_acceptedをturn_startedへ捏造していない。
保守待機解除後、統括のcontext/capabilities/status再照合と、保持consumerの正規更新への進行を
terminalで確認した。これはH2-a/H3-a配備であり、H2の終了effect APIや
H3の自動adapter/未達通知、H4/H5全体の完成を意味しない。

#### H3-b: 既知のlocal復旧adapter（全検証合格・同統括Driver反映済み）

`orca_local_recovery.py`は既存3helperを再利用する。auto選択は旧Linear transient読取拒否、
exact inbox receipt相関拒否、review_pending直後のworkspace HostBusyに限定する。
unknown・validation timeout・結果不明の実行は自動対象にしない。利用者pause/追加指示では
adapter選択前に待機し、復旧前と保存直前にcontrol・可視統括・loop全体・各subject sourceを照合する。
review待機復元は中間保存を除き、active＋review_pendingを一つのatomic saveで公開する。
通常Driverのnative照合後・tick前に接続し、復旧はphaseのローカル復元だけとする。
配車・検証・ACKは通常経路が所有し、adapterから外部送信しない。
候補の関連123件と追加focused試験を実行。最終の10件ではfresh読取中のpause・source変更・
owner交代拒否、同Driverからの通常step再開、旧停止のpreimage保全を確認した。
Linear復旧も保存前に`linear-runtime-recoveries/<inspection digest>.json`へ元loopを保全し、
WAL保存後に再照合する。停止履歴を消して復旧済みと見せない。
最終固定reviewはAPPROVED。loop hashは
`0c4bb865f07c780777b592544dbe8616eca25f78dcf4fe3ae3b15cf4d4b25808`、追加test hashは
`1ddadb326317606603c1a1ce3a27e2587f53e4d85db9ebba9284e4f55b42dce1`。
source `7f723f4ddef875daa6d340270628c9e8f8a0e35682bf261b792f627dd5537d56`の全toolingはexit 1。
出力の中間切詰めでfailure本文を取得できなかったため、成功とは扱わず以降のgateは全出力を保持する。
同sourceのcontractsはpass、storageは160 batch・665929068544 bytes・未分類0。
配備・typed attention通知は別の残件であり、H3-bのコード承認だけでは完了にしない。

追加確認で、Linear復旧helperがinbox非空を拒否した場合、Driverが`waiting_for_recovery`で
通常tickをskipし続け、完了mailを回収できない経路を発見した。`tick(paused_only=True)`を追加し、
scheduler内のpaused・native・operator control・可視統括を確認して、通常mail pollだけを実行する。
approval再検査・phase復元・新規配車・自動回答はこの分岐から行わない。
最初の補正はapproval検査を通ってしまうと固定reviewで指摘され、専用早期分岐へ修正した。
実mail fixtureの完了ACK回収、同Driverの次周回復旧、approved sibling/finalの検査拒否に
影響されないこと、利用者pauseと新規配車拒否を含む13 focused testsが成功。
追加補正前の再gateはroot所有の検証だけを中断（exit 130）し、製品buildは中断していない。
現在のloop hashは`abcc8d06808b6526413cd43efd4f166e36237c983d89054ec54922caf6b0f81f`、
test hashは`1350f4e0afa43207691bb743112805b73c4145acde579d3fa17d4f84d5ef1cb8`。
この補正後の固定read-only reviewはAPPROVED。全toolingのsource
`7bf201edd178a1ccfae155663f27b1e85dbeba9f888f3a4f73af62396b6bfc17`は1,460件中9 errorsで失敗。
全9件はstorage fixtureが稼働製品のheavy slotと競合したもので、実ロックを維持した既存
`isolate_coordination`を適用した。script-mode importもfixture内で同一moduleへ束縛する。
本番storage/controller/guardを変更せず、busy時の子起動前拒否も追加し、31 focused testsとRuffがpass。
test hash `684c0fd26c7271bb769fbe834c6e33f05fd3f7f4c9426821cb415a800ae170a1`は固定review承認。
全toolingは補正後source `642884768e29322de4b7c0787462b65546db595ca4ab6abb4a285373b7433290`で
1,461件＋164件、Ruff/actionlint/perf self-testがpass（exit 0）。終了後のsource/hash一致も確認した。
比較base `ae5b2066c4fd39a8dd6a6c439e272d125f749241`を明示したcontractsもpass。
storageは161 batch・667015884800 bytes・未分類0。Helpは開発基盤のみのNo impact。
監督controllerを正規refreshでgeneration `4ef76140-1f3d-45a3-9c91-d966ea56b577`へ更新し、
同runtime `542cfcbb-38a8-43a8-957c-53379bec2158`、同PID 2870379、health running/errorClass null、
loaded scripts digest `cb2c85a79ece2f1014f6117fb49f5a492830d29124c1591688d1c7361c3f2b1f`を確認した。
製品ArtPreview検証のfinalized passと統括の安全なhandoffを確認後、旧Driver PID803158の
正常終了・消失を確認した。同terminal/incarnation、同session
`01a0d377-feef-7520-bef5-50565ef282cb`を新Driver PID1331703で再開し、ready/acknowledgedを確認。
保守待機を解除後、統括自身がfresh stateを照合して同Run `run_3236488f0dff`・generation12の
固定reviewを再開した。送信受理だけでなく、実際の`integration=reviewing`とreview質問への応答を確認した。
製品の3未達条件は保持し、ArtPreview passを依頼全体の完了とはしていない。

H2-a/H3-a反映後、既存統括がhost登録APIを使用して
`tak-14-full-tank-strokes-04f3fbac-20260930-v1`を登録・実行し、primary台帳の
`phase=finalized/result=pass`、canonical controller `96491ade…`のhost_admission receiptを確認した。
同Run/generation12、head `04f3fbac90596ce922ed04ed1b0b46ad3fc2379e`、source
`49af3726e0e949921efedb5637a12ab12556e40f0d54bde23e36c1f83c173f41`に拘束されている。
統括の正規`resume-review`はevent `08d4f6e9-7856-59ac-88fc-6d1a7b30de17`を返し、
固定reviewへの再投入を確認した。rootが製品検証を二重起動したものではなく、既存工程の観測である。
この一件のpassで他のnative/performance/art条件や文書を完了にしない。

H3全体の設計reviewでは、native照合後・通常tick前の単一barrierとtyped attentionを使う。
初期adapter候補は`resume_linear_runtime`、`resume_inbox`、`resume_review_wait`に限定する。
各helperの副作用前条件・保存後条件を検査し、重処理やDispatchの結果不明な再実行は対象外。
判断を要するroutingは別operation台帳から同統括へ通知し、H2 planningを上書きしない。
このbarrier/notification/adapter実装はH3-aの待機修正とは別の残件である。

#### H3-cの接続設計（同Driver反映・判断適用済み、実装A起動失敗はH2残件）

固定read-only設計reviewで、既存watchの3種（lane review routing、lane validation routing、
combined review）だけを純粋obligation生成へ抽出する方針を確認した。
Driverは`tick → attention通知 → 工程継続通知`とし、配送未確認で他laneやmail回収を止めない。
runtime.planningとは別の複数件対応private台帳へ保存する。IDは全loop digestではなく、
Run/世代/slot/sourceと封印reviewまたはvalidation evidenceから導出し、他lane進行による再通知を防ぐ。
最初に観測したloop digestは監査情報として保存する。旧terminal/controlへの通知は新IDで逃がさない。

既存の`review_routing`は後で除去され、validation strategyもexact入力証拠を十分に保持しないため、
通知済みやwatchからの消失だけをappliedにしない。正規の3route helperで、host導出のobligation、
before digest、route spec、証拠、source、Run/世代を同じloop保存へmarkerとして含める。
prepared receipt → loop marker → complete receiptの順に保存し、中断後はmarker照合による
receipt確定だけを行う。route自体を再実行しない。通知前に正規routeが成立した場合は送信不要とする。
この設計の承認は実装・配備・一貫受入の承認ではない。

先行実装は`orca_attention_contract.py`と`orca_attention_routes.py`およびfocused tests。
固定reviewで、未回収mail時のcombined通知、laneの適用証明不足、修正slotの混同を指摘され補正した。
combined通知にはcallerの明示的なdrained証明、stageには実mail照合とrequest binding検査を要求する。
lane/combinedとも新規history行へoperation IDとoutcome digestを記録し、markerとのexact1件・
owner・digestの双方向照合を行う。先行4ファイルは15 focused testsと固定reviewを通過した。

続いて3routeを`stage → loop単一save → complete`へ接続し、load/save/archiveはmarkerと
private journalを検査する。新しい`orca_attention.py`はruntime.planningとは別の記録を持ち、
同ID通知・再照会・適用証拠を扱う。Driverはローカルreceipt回収→native照合→通常tick→
判断通知→工程継続通知の順とする。未送信の一時観測不能はdefer、送信後はunconfirmedに保ち、
正の所有/source変更や不正receiptを再送しない。watchのRun ready経路は同じobligationを参照する。
依頼完了・次世代開始では未解決判断とapplied証拠を検査する。通知開始やwatchからの消失を
完了へ読み替えない。過去世代は正規archiveのRun/世代/target/head/sourceと処理済みjournalを照合する。
validation通知の証拠照合はread-onlyであり、新しいretry世代の認可はroute helperだけが行う。

focused検証はattention 31件、request runtime/review loop 117件、native/combined/attention 88件がpass。
固定reviewで検出したancestor targetの`base`有無の差はrepo/branch投影へ修正し、正規archive生成器で
回帰を追加した。追加固定reviewではnative successorの保存integration全体をjournal outcomeへ
拘束し、currentの唯一のhistory行と一致することを要求した。native pending中は通常tickを
引き続き禁止し、exact finalized結果・既適用predecessor・同一subject・accepted/started通知・
単一combined判断が揃った場合だけscheduler lock内で判断通知を許可する。
この補正は固定review APPROVED、attention 32件と関連79件がpass。
旧sourceの全toolingは1,492件中2件エラー。completion fixtureのlanes欠落と、保存時に拒否すべき
history改変を許す旧テスト前提を修正し、production guardは緩和していない。
さらに通知側のscheduler保持と統括routeの取得上限が競合し、通知開始後にrouteだけ拒否される
再停止経路を確認した。3routeの最外schedulerだけをqueued admissionへ変更する。
取得後に最新loop・owner・control・sourceを再検査し、下位leaseと通常Driverの取得方式は変えない。
実flockでreview/validation/combined（legacy・staged両方）の一度だけの適用、read-only replay、
待機中pauseの拒否を確認した。外部CLI entry専用とし、scheduler保持中の再入は禁止する。
旧source `4d07b331…` の全体試験はこの修正のためroot所有process groupだけを中断（exit130）。
source `06f554c0283d0e7099a40a0e444c650181d1d69ba3a2035bfd79ecbfd4be7807`
でcontracts・tooling（1,496件＋164件）・depsがpass。分類で選ばれたゲームRust compileは
対象外のためroot所有process groupだけを中断（exit130）し、全CI成功とは扱わない。
controllerをgeneration `b181b3d5-222b-43ad-a1d5-7d9cdb377ea7`、scripts digest
`01f0d7fe29c634674f1bd0a69937c859db8b88510ffe7a0ae560d90dc0c95c93`へ正常refreshした。
保守hold・安全点確認・旧Driverの正常終了後、同session・同terminalでDriver PID1926324を起動し、
readyを確認してholdを正規解除した。新Runや製品工程の代理実装は行っていない。

実受入では判断通知が92,015 UTF-8 bytesあり、project側の固定64KB argv検査で送信前に拒否された。
`unconfirmed`記録の存在は未送信の正式証明ではないため、本文の短縮書換え移行は採用しない。
新規通知だけprivate evidenceへの短い参照とdigestを送り、既存記録は同ID・同本文で照会する。
Linuxのargv検査は実OSの単一文字列上限（PAGE_SIZE×32、NUL込み）とARG_MAXへ整合し、
実executable・全argv・環境・pointer・wrapper余裕を送信直前に照合する。非Linuxは従来上限を維持。
この補正の4ファイルは固定review APPROVED、focused51件pass。source
`4431ad2e404dcea4ae8401b7ef1ad37ec5331c4b3aeca82aa1616a47dd46c0db`の
contracts/tooling（1,500件＋164件、Ruff/actionlint/perf self-test）はexit0。
実行後のsourceと固定review対象4hashの一致も確認した。primary storageは164 batch・
666523832320 bytes・未分類0でpass。新規製品batch・作業場・binary copy・削除はない。
同sessionの安全点と正常終了後、監督controllerをgeneration
`009da993-a25f-4ddb-bba9-c62af8a986ca`、scripts digest
`d5e59418efe55ca9e6644b75d1bec59a33d193c7a39769cd363e4e9f191cfec5`へ正常refreshした。
統括は同terminal・同sessionでDriver PID2188273へ再開し、readyと保守hold解除を確認した。
継続promptを追加せず、Driverの通知から統括が保存済みstaged routeを適用した。
attention `8f6384f3-cc02-56f8-907d-0c28e464783d`はexact markerと照合され`applied`。
3 findingsとnative/art/release/docs工程を保持したまま、M2の補正だけを実装Aへ戻した。
続くrole-tab-sendが上記H2の結果不明エラーになり、loopはpaused。通知・判断適用の成功を
実装Aの起動や全体受入の成功へ読み替えない。
HelpはNo impact。入力は開発用loop/evidence、consumerはOrcaの統括判断・終了制御のみであり、
ゲームの操作、表示、成立条件、runtime assets、Help catalogに変更はない。

#### H3-d: 検証差戻し後の所有範囲改訂（2026-09-30、反映・受入中）

実装Aの取消・再開後、質問の回答とACKを可視統括が直接処理し、Driver側の同じ質問だけが
decisionに残る二重consumer経路を確認した。既存回答の本文・Run・Dispatchを照合し、
正規decideの冪等応答で同じanswer IDを回収した後、通常Driverがmailを回収した。
これは局所復旧であり、汎用Orca通知とproject-owned watch/decideの競合防止は未実装。
Help判断待ちもattention対象外であり、今回は同統括への復旧連絡から実判断・検証へ進んだ。
この手動連絡を無介入受入成功と扱わない。

続く検証では189件中7件が既存fixtureの独立numeric approval不足で失敗した。
同担当への通常差戻しは起動したが、必要なfixtureがticketの編集範囲外だった。
既存scope-replanはcleanな初回失敗専用で、checkpointとvalidation_retryを保持する
今回の正式failed settlementには対応せず、書込み前に拒否した。2026-09-29T22:41Z時点で
同Runのlaneはpaused。再検証が走ったことを検証合格や恒久復旧完了に読み替えない。

- 初回clean再計画のguardを削除せず、保存済みvalidation_retry・exact source・checkpoint・
  旧review・正式failed settlement・終了・release/ACKが揃う補正用admissionを追加する。
- requestの目的・受入条件・禁止操作・Run・Task・会話・担当tabを維持し、追加pathは統括が
  現受入に必要な範囲として選ぶ。旧checkpoint/失敗evidence/承認を消さず再検証を必須とする。
- 保存前と配車前にcontrol・所有・source・同Task specを再照合する。旧receiptを書換えず、
  改訂receiptと新generationを記録し、応答消失・保存中断・再照会で二重配車しない。
- 拒否系はsource/index変更、未settlement、未ACK、別Task/Run、旧reviewの偽装、
  利用者停止、scope縮小、受入条件変更、途中receipt改変を含む。固定reviewとtooling後に反映する。
- 二重consumer防止、Help/failed-worker判断の通知、検証失敗とcheckpoint到達の表示区別も
  同じS3/S6残件に含める。現案件の台帳直接補正や製品fixtureの代理編集で回避しない。

固定read-only reviewで、先行候補の17 focused passだけでは入場証明が不足すると判明した。
先行候補は未配備のまま撤回し、従来の拒否条件を維持した。必要な順序は以下のとおり。

1. 過去の完了通知のACK投影回収。`ctx_d7391a46e6e1`だけが未accountedである。
   read-only DBで`delivery_eae245c89d38`が同Run/generation1、単一message
   `msg_9fd097b8c8a7`、acknowledged（2026-09-29 21:55:17）と確認した。
   対応するprivate bridgeの正式worker_done、現Orcaのsettlement/release、exact batchを
   前後照合し、正規の同Delivery冪等readbackを保存する専用WAL/CASが必要。
   readbackは同owner/Runの`check --ack <delivery> --peek --retry-request <stable UUID>`とし、
   通常checkによる別のoutstanding Delivery生成を避ける。応答消失で新IDを作らない。
   completion_acknowledged以外を変えず、drained条件は緩和しない。
2. その後のscope改訂。gen12→11→10→canonical committed checkpointの全receipt鎖を検査する。
   各failed validationの保存証拠・assignment・sourceを束縛し、historical checkpointは
   review_checkpoint_proof、reviewはcanonical review_ticketとsealed承認を再照合する。
   integrationの5件のhistory/previousは正常な過去成果であり、消去や無条件許容ではなく
   before/after不変として保持する。旧承認を現在dirty sourceの承認に使わない。
3. Brokerのexact-byte pinを維持する。新しい通常import依存を増やすだけではpin対象から漏れるため、
   admissionを既存pinned helper内へ置くか、依存bytesも一回read/compileして明示的に固定する。

この時点で追加のlive mutation、再配車、製品編集は行っていない。手動の質問照合回復と
後続検証開始は確認済みだが、この新しい停止の復旧は未完了である。

ACK投影の候補実装を`orca_ack_projection.py`へ分離した。
単一Deliveryのread-only SQLite snapshot、private bridge、canonical worker settlement/retention、
同owner/Run/generationを照合し、`--peek`付きの同操作UUID readbackをWALへ保存する。
prepared/received/completeの中断再開は同じ操作を継続し、loop差分を既存attemptのACK bitだけへ
限定する。source・operator control・可視ownerは再試行間も保存証拠と照合する。
隔離21試験で中断3箇所、再試行、control/source変化、foreign/multi-message/unacked delivery、
誤readback、canonical DB以外・symlink/hardlink/権限異常、WAL破損とDB非変更を確認した。
固定reviewの追加指摘に対し、標準profileの内部導出・opened FD/inode照合、canonical peek結果の
exact形とmutation操作ID・replayed型、WALのphase別schemaを追加し、固定reviewで承認された。
完了済みnative履歴は消去せず、通常native mutation guardとbefore-loop-saveを無変更copyへ適用し、
全event/predecessor digestを前後で照合する。pendingな継続や新successorの作成は拒否する。
contracts群はpass。operation `e33a1fb5-2ef8-4c36-9a0b-c01502603489`で同RunのACK bitだけを
復元し、mail.drained=trueを確認した。phaseはpausedのまま、9件のnative event履歴を保持した。
通常Driverへの自動接続はまだなく、この復元を依頼全体の再開とは扱わない。
Help判断はこの追加helper/testに限定してNo impact。開発用受領台帳からOrcaのcompletion accounting
への経路だけで、製品Rust・assets・player-visible consumerへは到達しない。

後続のpost-checkpoint scope改訂を既存pinned scope module内へ実装した。失敗validationの全世代鎖、
canonical checkpointとsealed fixed review、immutable Taskに実際に渡した診断follow-up、
previous integration receiptの現target/head/source、保持済みexternal workerのownerを検査する。
historical_review_ticketは履歴専用であり、通常review_ticketのclean-source拒否は維持する。
再構築した旧承認は現在のdirty sourceの承認には用いない。before/afterではintegration全historyと
previous、checkpoint/review/evidence、Run/Task/sessionを維持し、後続の再検証・新reviewを要求する。
固定review承認、関連120試験、最終scope19試験（previous integration/external resource拒否を含む）、
Ruff/diff検査pass。実対象のread-only proveも同session/sourceでpassした。反映・通常配車の実受入は継続中。
先行tooling全群1526+164/perfはpassしたが、その後のscope改訂を含む最終全群証拠ではない。

同日、保守instructionを正規登録して工程開始を止め、統括の通常終了とshell復帰を確認した。
supervisionをreview済みhelper-set `111859b7abd2738f9225d922e588a5173210958ae67f109464a61978957c5370`
へ正規refreshし、generation `6304c65b-eeb2-4cd3-84c8-f89626485ace`のrunningを確認。
同じ統括terminal/sessionでlaunchし直し、新Driver PID 3609634とfresh acknowledgeを確認した。
保守instructionだけをdefinition/revision不変の正規reconcileで解除した。会話・Run・Task・成果・
native履歴の削除や新規作成はない。通常scope改訂を可視統括に依頼し、同会話での処理開始を確認。
その後、可視統括の正規brokerでscope改訂operation
`5f5da03f2e6d58f143d73fd8871572c1fd7b0201551add7a81a78c9d1769d3f6`を適用。
generation 13、同じTask `task_68260bf320ff`、同じA terminal/sessionで
Dispatch `ctx_c43bc5d553a5`が開始し、retryOfは直前のfailed Dispatchを指す。
2026-09-29T23:28:01Zの実heartbeatと、担当terminal上のfixture編集・Help影響確認を観測した。
これは実起動・作業再開の証拠であり、担当終了後の検証・固定review・依頼全体の完了証拠ではない。
primary docs/diff検査とstorage checkはpass（164 batches、666524758016 bytes、未分類0）。

### H4: 配備・UI・資源ライフサイクル（S5、S6、S8）

2026-09-30入力回帰の隔離受入: batch `orca-hardening-prompt-20260930-02` は
e2e modeの別buildで3/3成功（skip/flaky/retryなし）。独立verifierでもsource・fixture・
app/preload/CLI/renderer全成果物fingerprint
`58ab0512d8fc2e799a0d6d046612d369cc90792bee04c122772e612f1c930f00`一致を確認した。
確認範囲はfake providerの実PTYで、遅延composerのpaste framing、飲み込まれたEnterの
再送防止、permission dialogへ0 bytes。typed exit、実provider隔離、restart、H5全工程の
受入には読み替えない。稼働本体はまだ切り替えていない。
batch02はpass seal/finalize済み。固定受入review用reportの実測12288 bytesは用途付き保持。
設定不備だったbatch01は比較診断を終了し、consumer解除・open fileなしを確認して
`/home/satotakumi/tools/orca-ui-lifecycle/.git/orca-ui-acceptance/hardening-prompt-20260930-01`
を削除した（118784→0 bytes）。唯一のコード/assetや稼働履歴は含まれず、診断結果は本文へ集約済み。
次に同本体candidateの既存restart回帰6件を背景の別profileで実行する。
新しい限定configと独立verifierはskip/retry/flakyを成功として扱わず、source/testsと全buildを束縛する。
対象はclean quit/relaunchのscrollback・cursor・tab数・title・idle保存頻度・close済tab非復活であり、
park途中の異常終了やtyped exitの証拠とは区別する。
restart batch01は背景windowのframe待機により3pass/2timeout/1skipでinvalid。
既存offscreen opt-inでbatch02/03は6/6通過したが、正式verifier呼出しのaction欠落と
共有fingerprint import時の副作用を順に検出したため、どちらもsealed passとはせずinvalidで記録した。
batch04はentrypoint判定のimport.metaがPlaywrightのCommonJS変換と非互換で設定読込時に失敗。
fingerprintを副作用のない専用moduleへ分離し、configのexact6件列挙を確認した。
設定失敗で本体テストへ到達していないbatch04の生成reportは診断終了後に削除した（8192→0 bytes）。
batch05は6/6成功、独立verifierと正式seal/finalizeも成功した。
whole-source/build fingerprintは `30e3578d35a4df9f4e3c9651392e10869e2e411b17667c5874165a58bbe4f461`。
pure fingerprint module抽出と2config/2verifierの固定reviewも承認済み。
現行report16384 bytesは最終H2/H4受入reviewの具体的consumer付きで保持する。
診断済みrestart01/02/03はconsumer解除・open fileなし・生成report/traceだけであることを
確認してexact出力directoryを削除した。順に106496/16384/16384→0 bytes。
稼働terminal履歴・ソース・採用成果は削除していない。失敗理由は本文と台帳に残す。

直前のH3運用是正（2026-09-30）: combined reviewから再評価へ戻したAがcompleted・変更なしで
終了した際、assignment_baseと現在baseの差を理由にresolve-no-changeが拒否された。
当該guardを緩めず、既存return-correctionを「failedのまま変更なし」に加えて
「completedかつ正式no_change_pending」に対応させる。後者は同ticket/sourceの成功validation、
正式settlement・ACK・release、既存lane approvalのexact HEAD/source、保存済みcombined rejectionを
照合する。新承認・空commit・再配車を作らず、未達指摘を保持したcorrection_requiredへ戻す。
関連拒否系・固定read-only review後、同統括の新turnと次工程操作まで確認する。
Help No impact：開発用所有権返却のみでゲームの入力・表示・成立条件・assetsは不変。

同日適用: helper `1a59dcd7c38ab3c97a689d0be7382b26314bb985f4964beddf12ed9765e8d998`、
test `44e69e724cf29d5d9ba93d40eb921525684b64802f3a0b2f4fbda64b6b5e3c5a`、
統括案内 `198ec120ccb27bd2bffed38db5d4eb88bd85c10bdd68916d2e7a894d5e2d3cf7`は固定review承認。
関連9件・統括35件と独立reviewの9件がpass。primary storageは164 batch・未分類0でpass。
同統括への操作案内receipt `726f3ff3-1711-406e-93ab-3e3cef991a32`は当初input_acceptedのみだったが、
再送せず後続の新turn、fresh show、return-correction実行を確認した。元Runのままactiveへ戻り、
元の3件のcombined指摘を保ったcorrection_requiredと帰還digest
`7386e3111ffc4dad984ae8ba3b7fdcd74b4cb7e7247d821b30672cfd0b41f06c`を確認。
続く同turnでnative registration contextの正式照会とM2候補生成の前提調査へ進んだ。
rootは製品build/testを起動せず、修正担当への再配車や新規Runも作っていない。

帰還修正の全toolingは1565件＋164件、perf self-testとcontractsがpass。
統括が次の`tak-14-full-tank-strokes-8e462c75-20260930-v1`を登録・実行し、
独立検証pass、保持用途登録、finalizeまで実施した。これは候補生成の技術結果であり採用承認ではない。
終了通知後、正常なneeds-finalizeの間にattention_obligationが厳密終了guardを呼び、
ValueErrorがDriver最外側まで伝播してconsumerが終了する追加不具合を確認した。
`NativeFinalizationPending`はsealed fieldsとevent-bound result proofが完全一致する
needs-finalizeだけを分類する。attention_obligationだけがこの型を待機として扱い、
他のmutation・review・完了経路のfinalized必須条件は維持する。破損・unknownは待機へ丸めない。
関連122件、独立固定review14件、全tooling/Blender164件/perf/contractsがpass。
固定review承認hashは`orca_native_continuation.py=1324e4690e15acb0d7bf0fd79451b3a00ed8c4ade50ba656213117f26f7515c7`。
Help No impact：開発用終了待機の分類のみで、ゲームからの到達経路・表示・assetsは不変。

保守instruction `112be920-dc40-46a4-a2cc-250606d00b40`で次工程を保留し、同統括の
正常終了exit 0と同PTYのshell復帰を確認した。supervisionはhelper-set
`66c94a30808026cbf4072c95ee96af3df4e08ee0ad6de43e4f9cdbfb3b5390b1`へ正規refreshし、
generation `58bda6e0-139c-471d-9cb0-6e44add633b3`のrunningを確認。
同terminal・同sessionで再開し、新Driver PID92520とprovider新turnを確認した。
保守解除後、同Runのnative固定reviewが完了し、統括が全3指摘の照合と補正経路の選択へ進んだ。
ただしcompleted/no-changeの同Taskを再配車し、failed専用retry guardで工程がpausedとなった。
Driverのrunningを工程稼働とは扱わない。Orca本体もcompleted Taskのretryを許可していないため、
retry guardを緩めず、route保存前に終了済み同ticketを拒否する補正を実装中。
旧routerで保存された拒否済みrouteは、既存return-correctionにより元の終了・成功validation・
dispatch receipt・launch registry・source/review・release/ACKが不変の場合だけ統括へ戻す。
以前のreturn記録と最新combined指摘を保持し、新Task/Run/approvalを作らない。
追加補正は固定review、全tooling1572件＋164件、perf/contracts/storage検査に合格した。
review対象4ファイルを再照合し、helper-set
`9cc8bfe5dd70ead284bca7eca4e8bf013a014d8c714dbb0eed8988995a8ee6d6`へ正規refresh。
generation `0e3082c2-e929-4385-b8c9-c529c5cfe7a6`、同統括session、Driver PID462112、
2026-09-30T02:02:48Zのfresh acknowledgeを確認した。保守instructionだけを正規解除し、
revision 1 / scope hashを維持した。統括自身のfresh show→return-correction→watchにより
active / correction_requiredへ復旧し、3指摘を維持したことを確認した。
追加の案内送信operation `d8b6be88-0a9c-4ede-b8f2-ad4c3dc89b74`は旧本体の
target_unverifiableで拒否されたため再送していない。再起動時の既存案内から統括が
実処理を進めている証拠であり、この送信の成功証拠とは区別する。
rootは新Run/Task/Dispatchを作成せず、製品テスト・commit/pushを行っていない。

H2-b別build検証: 既存本体candidateとcacheを再利用し、旧H4画像比較には旧outを要求しない。
ownerはOrca infrastructure coordinator、consumerはhardening H2入力配送受入、出力は
`/home/satotakumi/tools/orca-ui-lifecycle/.git/orca-ui-acceptance/hardening-prompt-20260930-01`。
primary validationへ登録後、隔離Electronの既存3配送試験を実行し、exact source/build verifierで
照合する。これは全H5や実providerの受入を代替しない。出力の用途は失敗時の原因調査または
次の長文/再起動比較であり、その終了時に整理する。稼働app/daemonは変更しない。

初回batch `orca-hardening-prompt-20260930-01`は通常mode buildで`window.__store`が
非公開だったため、3件ともfixture準備で失敗した。配送assertionは未到達であり、入力機能の
失敗/成功とは扱わない。invalidでseal/finalizeし、118784 bytesを次の訂正buildとの
準備経路比較用途で保持した。旧out bundleへの依存はない。
訂正run `orca-hardening-prompt-20260930-02`は同じcandidateを`--mode e2e`でbuildする。
模擬Codexはmode2004・bold glyph・placeholder/footerとcursor復元を正式表示に合わせ、
stdinのpaste frame/Enter回数/permission時0 bytesの独立判定を維持する。
verifierはsrc/config/tests全inventoryとmain/preload/cli/renderer bundleを照合する。
2026-09-30T02:10時点、TAK-14がheavy slotを取得して実profiling buildを開始したため、
rootの訂正buildは15秒ごとに正規admissionを再照会して待機中。枠を迂回しない。
本体candidate保持は4186492928 bytes、consumer `hardening H2 guarded prompt delivery acceptance`。

- 対象候補: launcher/互換manifest、`orca_supervision.py`、`orca_role_tabs.py`、
  `orca_native_continuation.py`、storage controller、既存Orca状態行/折畳み操作。
- [ ] 実行中の版とdisk上の版を区別し、互換確認済みの安全点で同sessionへ反映。
- [ ] 更新・rollbackは台帳を巻き戻さず、旧binaryが新schemaを扱えない場合は拒否。
- [ ] 統括/A/B/固定reviewの同tabを再利用。完了した実turnの履歴へ直接移動できる。
- [ ] 状態表示はターミナル操作を妨げず折り畳める。既存実装は回帰確認し不要に作り直さない。
- [ ] 実行中、終了処理中、孤立を区別。成果とreview-active cacheを勝手に削除しない。

2026-09-30: UIを作り直さず、既存の折畳みとterminal再利用を検査する回帰試験を補正した。
除去済みの長い説明文が存在するという旧assertを廃止し、非表示後のpoll-only更新では再表示せず、
意味のある状態変更で最新状態が現れることを確認する。独立verifierは非bridgeの5シナリオを
タイトル・件数・一度だけのpass・source/build fingerprintで厳密照合する。
`supervision-panel.spec.ts=82b76303472df94b1d483324c1e8869ca000d8ba9b94e27ef82605236f486fa9`、
`verify-supervision-evidence.mjs=a5baf18ed65b349f767eab3ed4505333c5ee6e349efca140baf03c1ebc926f15`
は固定review承認。status bannerの2単体試験、isolated prompt/leaf routeの40試験はpass。
続いてhost heavy lease下でprebuildとCLI compileを行い、primary登録の
`orca-hardening-panel-20260930-01`でbackground/offscreenのElectron受入を実施した。
5シナリオすべてpass（1.5分）、独立verifierもpass。状態更新・stale controller・終了待機の
画像4枚を目視確認した。primaryでseal/finalize/checkを完了し、162 batch・666360143872 bytes・
未分類0。cold start/実配車/最終互換切替の証拠とは分離する。

同batchの画像・reportは425984 bytesをhold `orca-hardening-panel-20260930-01-evidence`で保持。
consumerは`orca-system-hardening-h5-ui-comparison`、次actionは最終H5 buildとの画面比較、
release_whenは最終baselineへの置換または比較終了。旧binaryの保存や旧verifier再実行は不要。
候補source/cacheは既存holdを維持し、新consumerを`orca-hardening-ui-source`として登録
（観測4186877952 bytes）。固定reviewのfeedback終了まで保持し、他consumerも確認して解放する。
稼働package・daemon・製品検証領域の削除や置換はしていない。

#### H4配備前のpackaged起動照合（2026-09-30、未配備）

通常production buildを既存candidate内の
`.git/orca-ui-acceptance/package-hardening-cancel/linux-unpacked`へ梱包した。
glibc floor・daemon entry・plugin resourceの梱包検査はpass。
build labelは`c24fdc529a60f7e3f392bb991bd77d36095ff231de79cbe4e5164813cc1d49ab`。
label単独を承認やソース全体のhashとは扱わない。

primary batch `orca-hardening-packaged-20260930-03`は隔離profile・hidden windowで
1/1 pass。実行中のexecPath/appPathを指定package内のcanonical pathに直接拘束し、
loadedReleaseのbuildIdを独立に与えた期待値と比較する。reportにもbuildIdを保存し、
独立verifierが同値、試験件数、単一pass、現在のsource/test/build fingerprintを確認した。
package fingerprint v2はroot/entry symlinkを拒否し、entry type・mode・内容を含む。
空file/空directory・内容・modeの差異とsymlink拒否の2試験もpass。

- package v2: `0b08cc52080ee16161c97000b63189183e7d23884f9ee45dcbdd1872fb344976`
- acceptance: `269a0e1c0816269006b58fe6fe023bb40ecbbcdf5c20c72d9cff502eec400881`
- batch01は旧照合の履歴。batch02は起動成功でも独立build照合が不足したためinvalidで終了。
- batch03はseal/finalize済み。最新reportのconsumerは同packageのlive切替照合、
  release_whenは実配備の版・互換性確認を本計画へ記録した時点。
- 固定read-only reviewは上記5試験関連fileを承認し、独立に2/2試験とbatch03 verifierのpassを確認。
  fingerprint実装は`e3e9e9cc90af308db09e5143b4d94693685013af2abac55db57a584667fa0584`、
  verifierは`79acb6efb49ee1caea50a0b6c020dcf8330f3a93b67098d612a19ef0b346d740`。
  この承認は本体の実配備やH2/H4/H5全体の承認ではない。

2026-09-30T07:00Z時点、稼働Orcaの切替は未実施。通常終了の操作経路は、
旧CDP endpointが接続拒否、accessibilityがframeのみで取得不能、X11一覧にも対象なし。
強制終了やdaemon停止はせず、利用者へOrca本体の通常終了を依頼した。
元pending送信の再送・ACK・新Run生成はしていない。配備後の同PTY再接続と
実turnの開始、typed exit、controller/Broker admission接続、H5一貫受入は未完。

Help No impact：この単位は開発アプリの隔離起動・配備証拠の照合だけで、ゲーム実行経路・
プレイヤー入力・表示・成立条件・runtime assetsを変更しない。既存の並行ゲーム変更は別対象。

#### H4の残実装の接続点

2026-09-30追加設計review: `main()`でdisk hashを採るだけでは、top-level import済みの
helper bytesを証明できない。stdlibと承認readerだけのbootstrapから、独立承認tuple
（manifest canonical path/hash、expected loadedRelease全field）を検査し、取得した固定bytesで
helperをloadする。loadedReleaseは同一status応答のruntimeId/capabilityと照合し、
buildIdだけでなくappPath/execPath/isPackagedもexact比較する。旧appの欠落を補完しない。
refresh intentと後継healthにtupleを拘束し、Electron identityが変わる更新はPython execだけでは拒否する。
保存互換はcanonical readerが変換前disk schemaと既知pending phaseを提示し、operation単位で検査する。
新たな台帳scannerや、schema1をmemory上で2へ変換した値の流用はしない。
このbootstrap/admission接続は未実装であり、既存readonly manifest検査の承認とは区別する。

追加read-only設計reviewで実装順を具体化した。bootstrapは検査後にraw scriptへ処理を渡さず、
`FrozenHelpers.install()`をcontroller/Brokerの全寿命中維持する。操作はclosed enumに限定し、
任意module/path/functionを外部入力から選択しない。固定moduleへ渡すReleaseSessionは独立approval、
manifest、runtime、loadedRelease全field、component集合に拘束する。
既存lock取得後と最初の副作用前に、同status応答とoperation別canonical raw-reader rosterを再照合する。
新しい全体scannerや巨大lockは追加しない。未整備readerがあるoperationは入場を許可しない。
接続順はsupervisionのSyncWorker前→tick/consult/route→統括prepare/Broker/Popen→native child。
各子processも同approvalのbootstrap経由にし、raw helperへのfallbackを残さない。
refreshのhealth/requestはfrom/target tupleを保持するschema2とgeneration単位CASを採用し、
Python execだけで別loadedReleaseへ更新しない。effect後のdriftでは新操作だけを止め、
同activationのraw outcomeを保存する経路を失わせない。これらは設計上の接続条件であり、
下記read-only entryの限定承認から実装済みとは読み替えない。

H4 loader候補: `tools/orca_release_bootstrap/loading.py`はapproved helper inventoryのbytesを固定取得し、
manifest digestごとの専用packageから直接compileする。相対importとpath lookupは同一moduleに
収束し、既存bare import、未知sibling、reload、PathFinder/pycache fallbackを拒否する。
操作前は固定stamp/inventoryを比較し、変化を再hashして自己採用しない。
bootstrap自身の二重module化を避けるためhelper directory外へ分離した。
`scripts.*`による通常importも拒否し、frontdeskの3動的importをpackage-relativeに統一した。
admitted bridgeのscope/acceptanceはcanonical moduleを再利用し、recipeは固定取得bytesを使う。
primary controllerはstdlib-onlyではなくhost_coordination→cargo_runtime依存を持つことを実コードで確認した。
明示されたcontroller bytesとsupported SHAを別に固定し、専用importerから同じ承認済みhost helperへ
だけ接続する。未知local importは拒否し、稼働primary controllerは変更しない。
`orca_release_runtime.py`は独立承認tupleと一回のstatusに含まれるloadedRelease全field・runtimeId・
capabilityを照合する。関連148件と全scripts/tools Ruffはpass、当該限定単位の固定reviewは承認済み。
loader SHA256は`078e26ed7b3d981e4cb4620855cba9f2840f809f22aa43c14834902317b027c8`、
runtime SHA256は`5cc0d7caa3e06cbf77a8a4219db3eac1d33340de0bfebc619baa3c770922abdc`。
既存`scripts.*`およびhelper配下の別名moduleの事前読込も拒否するnegative testを含む。
runtime authority・storage互換・起動/refreshへの接続と
配備はまだ行っていない。loader単体に実行許可を与える意味はない。

続く読取専用入口`tools/orca_release_bootstrap/entry.py`は、cold processで独立approval digestから
固定bytesを読込み、全componentを照合してから承認CLIの`status --json`だけを実行する。
同じ応答のapp identity/runtimeを確認し、照合中のartifact/document変更をstampで拒否する。
controller起動・台帳保存・更新lock・rollbackは実装しないため、この結果をwrite admissionに
流用しない。稼働launcherへの接続と配備は未実施。
既存`orca_request_runtime.read`はcanonical `read_raw`を共有する構造へ分離した。
`read_raw`は変換前schemaと未知planning原本を返し、従来`read`のschema1→2投影・quarantineは維持する。
bool/floatを保存schema番号として受け入れない。第二の台帳scannerや自動移行は追加していない。
この2追加単位は初回関連94件、review修正後の該当53件合格・固定review承認済み。
status失敗時もfinallyでartifactを再検査する。不存在だけをsentinelで区別し、実在する空・
不正・extra-key wrapperをbyte不変で拒否する。entry SHA256は
`33feb06bb85d1b06398ccb1111d6dbd913c79d5bd279636112bcc6f8515a91f1`、raw readerを含むruntimeは
`e65e2746924e704ed6530a6dabaebba4802e5a17739e2f46cb74d83ee0ae7381`。
既知pending phaseと全readerの互換gateは未完。追加後の全contracts/tooling回帰はpass。
git列挙したscripts/tools配下Python sourceの前後fingerprintは
`2fdaf6fab8b5dfda0dfaaa29609b426ddf1f6ebcf6603a50f3e2f823c7f61374`で一致した。
関連Blender tooling fixture164件とperf self-testもpass。ゲームbuild/testは未実施。
H2の終了保存・retry境界の追加差分はこの全体検証より後で、関連96件による別検証として扱う。

続いてexternal-syncのcanonical `read_raw`を追加し、通常`read`も同readerを共有した。
従来のfalsy判定では保存済み空object/null/listを不存在扱いしていたため、identity sentinelで
不存在だけを区別する。schemaはexact intを要求し、既知sending/unknownは書換えず保持、
未知phaseや壊れたoutboxはenqueue前に拒否して元bytesを保全する。関連82件・Ruff・
限定fixed reviewがpass。production SHA256は
`f7326a4d60fcf92752f669c35a11bedb0f4c0c5e8532a40db71e315ae8af23ce`。
既存path helperは不存在directoryを作成するため、cold startupの完全read-only inventoryへ
接続する前にnon-creating入口が必要である。新しいscannerやstartup write admissionは追加していない。
この時点のprimary storage checkは173 batch・未分類0・668313886720 bytesでpass。

non-creating readerを追加した。既存の`checked_directory`に`create=False`を設け、
external-syncとrequest-runtimeのcanonical raw readerだけで使用する。
request-lifecycleのpath構成まで同flagを伝播し、不存在時はrole-state/requests/runtimeも作らない。
既存directoryのtype/owner/mode、symlink・dangling symlink拒否は維持し、write側の既定は変更しない。
関連118 testsと固定read-only reviewがpass。Helpは開発用台帳読取だけのNo impact。
frontdesk SHA256は`5a7405a6fbd606162d8dc8a1eb8f04c2182d23a54c94af23d3785c1c81e1306e`、
request-runtimeは`30a4c89ef50fc09e768fa4302067afe9255abe0647ba471d0f54ff9e1d12bcb5`。
起動へのadmission接続と配備は別に残る。

最新infra sourceのcontracts/toolingを同一fingerprint
`303e5a493547361e0fb78105ab3e7bb29728622417181d61d81467857639512e`で実行し、
Python 1663件、Blender tooling fixture164件、perf self-test、Ruff/actionlintがpass。
自動分類は既存quality-control差分と未登録tools pathによりRust/depsも選択したため、
その実行はgame gate到達前に中断し、ユーザー指定の非ゲームscopeを明示して再実行した。
全CI群の合格とは報告しない。今回のHelp判断は開発用制御と台帳のみのNo impactを維持する。

固定read-only棚卸しで、既存refreshは監督panelだけを更新し、独立した統括Driver/Brokerは
更新しないことを確認した。cold startは現diskを自己採用し、healthにもloaded helper版がない。
`launch-supervised`はapp/CLIだけcurrentへ追随し、helperは候補絶対pathなので、
旧appへ戻すだけの切替を構成全体のrollbackとは扱わない。

共通互換入場検査はapp/CLI artifact・helper-set・primary controller・recipe revisionと
読書き可能な保存schemaを承認manifestに固定し、版番号だけの照合にしない。
supervisionのSyncWorker開始前、refresh要求/exec直前/後継起動時、統括の
`prepare_runtime/prepare`による保存前へ接続する。既存exact pinを緩めず、台帳を移行名目で
巻き戻さない。現在diskとloaded版は別記録にする。旧refreshの厳密field契約があるため、
初回配備と新protocolへの切替を同じ操作へ混在させない。
未知schema、同version別artifact、source交換、pending/unknownを扱えない旧releaseを
副作用前に拒否する試験と、H5の実cold start/同tab復帰を残す。

#### H4-a: 読み取り専用release照合（2026-09-30、起動経路未接続）

2026-10-01追加: external-syncの保存互換検査をcold bootstrapの読取専用入口へ接続した。
`release_observation`はcanonical `read_raw`を共有し、不存在をschema1へ補完しない。
全nonterminal（blockedを含む）と未処理proposalをpendingとして扱い、未知phase・破損は
縮約前に拒否する。`check_storage(mode='read')`はread互換とpending能力を検査し、
write可能性を偽らない。status前後の保存内容digest・artifact・runtimeを照合し、
結果に本文・認証情報を出さない。検査は書込lockを取得しないため、書込許可には流用しない。

関連94 tests、対象Ruff、diff check、固定read-only reviewはpass。実canonical readerを
固定bytesから読み込むcold subprocess試験で、不存在directory非作成、unknown保持、
未知phase拒否、pending未対応拒否、検査中変更、片側selector拒否を確認した。
承認SHA256: external-sync `8c8eef274811889797a2a66b7747ad37539897b2d10452d5cbf045b02bae2aa6`、
contract `38ff65300d7236442bd21112ff8a3c526d01e8e007d5e96f79473387c9ed9d2a`、
entry `c8f46857cfbdd1df66b901ad9bf1c96c27e3413f6fda548b6e5140e323b2ead8`。
Help Skillの判定はNo impact: 開発用保存台帳から配備前照合へ至る経路のみで、ゲームの
入力・表示・成立条件・assets・Help consumerは変更しない。ゲームbuild/testは実行していない。
primary storage checkは177 batches・未分類0・669380308992 bytesでpass。
新規の検証job・binary copy・worktreeは作成せず、既存成果・履歴の削除はない。

2026-10-02追加: request-runtimeのcanonical `read_raw`へ、connection/external/completionの
nested record検査を接続した。outer checksumが正しくても未知phase・extra/missing field・
timestamp bool/非有限値・content digest不一致・不完全な完了proofは原本を保持して拒否する。
pure validatorは保存・通信・scope承認を行わない。未知planning versionを原本保持でquarantineする
既存経路とschema1→2のread-only投影は維持する。release preflight専用の第二readerは追加しない。
既存completion試験の省略mockを実producerと同じ4-field proof/5-field receiptへ補正した。

runtime/external関連86件、bootstrap/loading/release/prompt関連75件、Ruff・diff checkはpass。
固定read-only reviewはこの追加単位を承認した。保存構造の検査であり、完了proofの真正性は
既存の後段same-subject/固定review照合に委ねる。配備とTAK-14復旧の承認ではない。
runtime SHA256: `ab224b276c64165c66ed68b00a101e4df9c281ec110e434ce7397bd04879d7d4`、
validator: `bf2e677d73f8f5894b8427873aa1b6308df6d82c516f9bde3039c888068ef70c`。
Help No impact: 開発用台帳のcanonical読取から制御の拒否判定に至る経路のみで、
gameの入力・表示・save・assets・Help consumerへ接続しない。ゲームbuild/testは未実施。
primary storage checkは177 batches・未分類0・669394440192 bytesでpass。原本削除はない。

追加後のinfra candidate全体を、host重実行leaseと既存quality runnerで明示的な非ゲーム
`contracts/tooling` scopeとして検証した。Python 1703件、Blender tooling fixture164件、
perf self-test、Ruff/actionlint、candidateの文書・Help・hygiene・規則/storage契約がpass。
前後のsource fingerprintは
`a773784e4f597b203ecdd79789e5f00928d048a4cc9658e7e048b8bceb1b88e1`で一致。
Rust/deps群・ゲームbuild/test・native製品受入は実行しておらず、全CI群合格とは扱わない。
この回帰はcandidate scopeの証拠で、全差分の固定review・write admission・配備の承認ではない。
primary最終storageは177 batches・未分類0・669394509824 bytesでpass。
primaryのdocs indexは新規依存directoryを含まない専用index checkで確認する。
前回のprimary `dev.py docs --check`は他sessionの未追跡`node_modules`内READMEの
link検査で失敗しており、primary全docs gateを合格と読み替えない。依存物は削除していない。

2026-10-02追加: cold preflightへrequest-runtimeのcanonical readerを接続した。
`--request-state-dir`はprivate state rootのnoncreating selectorで、`--request-id`は子依頼を示す。
明示rootは`create=True`と併用できず、存在しないrole-state/requests/runtimeも作らない。
縮約結果は実disk schemaとdigestを保持し、全planning（旧applied・未知versionを含む）を
保全すべきoutstandingとして扱う。未知planningを解釈・再送せず、既存quarantineと両立する。
read互換・pending能力をstatus前に検査し、同runtime照合後にcanonical recordを再読して変化を拒否する。

初回fixed reviewが、両familyへ同じUUIDを使う誤りを検出した。実producerはrequest-runtimeを
child ID、external-syncをparent IDへ保存するため、独立`--external-sync-request-id`を追加した。
両root同時指定時は親IDを必須にし、子IDで親outboxを探して不存在と誤認する経路を拒否する。
external-onlyの従来`--request-id` aliasは維持するが、新旧aliasの二重指定・root不足は拒否する。
異なる親・子UUIDの実在fixtureで初回・再観測の両方を検証し、再reviewで限定承認を得た。

cold/raw reader58件、runtime/lifecycle/external/release145件、Ruff・diff checkがpass。
entry SHA256: `01a55abd2232c6cb19b5077354cf7c51dbb3fb37b6f174632c431d3ac9f7da38`、
test: `685c28045f3af4cc551b186e0075dca9d702f968d9b17c2f340f34ed72368e90`。
Help No impact: 開発用の保存読取・配備前照合のみで、player-visible consumerを変更しない。
primary storageは177 batches・未分類0・669394628608 bytesでpass。原本は削除していない。
これはread-only観測であり、全reader roster、controller/Brokerのwrite admission、
配備・TAK-14再開を承認しない。前記1703件の全回帰より後の追加単位として扱う。

2026-10-02追加: cold preflightへ統括registryのcanonical readerを接続した（候補のみ）。
`--coordinator-state-dir`はnoncreating private root、`--request-id`は子依頼を指す。
既存validatorを再利用し、作業repoを採用せず、schema1/2をdiskのまま観測する。
旧readyが現runtimeに見つからなくてもpendingを維持し、終了・再起動許可へ読み替えない。
status前のread/pending互換検査、status後の同record再観測、親external-sync selectorとの
独立性を守る。存在しないroot/registryを作成せず、未知schema・不正activationを上書きしない。
schema2 ready/exited、raw outcome不整合、非作成、symlink、status中のrecord変更を含む
初回reviewで`exists/is_symlink`がPermissionError等を不在へ読み替えるPython 3.14の
挙動を指摘された。`lstat`のFileNotFoundErrorだけを不在扱いに修正し、実directory権限拒否が
status前に止まるcold testとEACCES/EIOの回帰testを追加した。
関連152 tests、Ruff・diff・Help gateがpass。Help判断はNo impact：開発用保存registryから
配備前照合までの経路のみで、game input/表示/save/assets/Help consumerへ接続しない。
coordinator SHA256: `ea8a3eca45feba689dc431f601e5d49538db16f2fc96f4be27be674da60fd36b`。
entry: `43f85d71935d337cad18906c47ee11f4f8280820e3972d49040804179d0f75d5`。
test: `fbb20ca061a47e40635d4600f21c3b56d66982900792b12cfa14b9a92acb9cc8`。
primary storageは177 batches・未分類0・669394743296 bytesでpass。原本削除なし。
P2補正後に固定read-only reviewがこの限定差分を承認した。
配備・write admission・legacy reboot復旧は未実施であり、
TAK-14の再開や恒久修正全体の完成とは扱わない。前記全回帰の後の追加単位である。

2026-10-02追加: 同じ`--request-state-dir`から依頼全体のscope契約もcanonical readerで
観測する候補を追加した。runtime記録だけを見て全体scopeのschema互換を見落とさない。
`read`はdefault/明示rootともnoncreatingにし、不在だけをidentity sentinelで区別する。
presentの空wrapperを不在と誤認して上書きする従来経路と、boolをschema1と読む経路を拒否する。
契約単独では正式acceptance/lineageを証明できないため、present契約は常にpending互換を要求する。
status前の互換検査とstatus後の同契約digest再観測を通す。保存scopeの修復・移行・完了認定はしない。
関連178 tests、Ruff・diff・Help gateがpass。Help No impact：開発用scope読取と配備照合だけで、
game input/表示/save/assets/Help consumerを変更しない。ゲームbuild/testは未実施。
lifecycle SHA256: `83f28b7d528c6d81a7e96103f5f56c77d843075e8c0be62d972fbfb77984d6c6`。
entry: `259750aa6a2c59be2eec917716fb9a1b18d5ff7fb3bf32a612a2e23f58076f28`。
test: `3281340d5814afd34275aea96fa8bf3e6eaab6d6a37764519f364640af00ae59`。
primary storageは177 batches・未分類0・669394837504 bytesでpass。原本削除なし。
限定fixed read-only reviewがこの読取専用差分を承認した。
write admission・配備・TAK-14再開の証拠ではない。

未完: controller/Brokerの操作前接続、typed exit、
本体配備、同依頼再開とH5無介入一巡。旧appが起動中でもcandidate実装は進める。
この限定単位の承認を恒久修正全体の完成やTAK-14の稼働とは報告しない。

候補helper `orca_release_contract.py`に、別途承認したmanifest digest、app/CLI/helpers/
controller/recipesのexact artifact、helper全Python inventory、保存version・pending能力、
loaded構成の照合を実装した。manifestの検査と取得は同じfdで行い、FIFO・symlink・
所有/mode不正・重複JSON keyを拒否する。集約検査後に全artifactとdirectoryを再照合し、
検査途中の差替えや追加moduleを見逃さない。既存台帳への保存・移行・自己承認はしない。

21 tests・Ruff・contracts・固定read-only reviewはpass。承認hashはhelper
`b97f2a51f5b6e44e44174df56732dbd0b9fdfe58c74b2f503a125e2e5689f284`、test
`448643f609f3d8a1bbbe122f3c530b7457d01c344c18c73721d52c4c2e28ceca`。
これはartifact predicateの証拠であり、稼働appのloaded証明、全保存台帳の互換性、
startup/default-entry/once/refresh/rollbackへの入場gate接続は未完。

次のadapterは配備側が固定した現在承認tupleだけを採用し、起動時disk hashの自己承認を
禁止する。台帳は既存canonical readerで操作単位に検査し、第二の全台帳scannerや
schema1→2の読み取り変換結果をdisk schemaと偽る実装は作らない。別releaseへの
書込rollbackは、そのfrom→targetの互換試験なしでは拒否する。
Help判断はNo impact：今回のrelease照合と本体入力epochは開発環境の制御経路だけで、
ゲームの入力・成立条件・表示・Help catalog・runtime assetsを変更しない。

#### H4-b: 読み込み済みapp identity（2026-09-30、未配備）

本体候補のmain bundleへbuild時の`ORCA_SUPERVISED_RELEASE_ID`をliteralとして埋め込み、
Electron共通起動点からappPath・execPath・isPackagedをruntime constructorへ渡す。
constructorは入力をコピー・freezeし、statusはこのsnapshotだけを返す。
環境変数・current symlink・後から読んだdisk hashを稼働版の証拠にしない。
Node-only runtimeとbuild identityなしの旧版ではfieldと能力を出さず、他instanceから補完しない。
statusのoptional `loadedRelease`はlocal/paired/SSH legacyの各CLI経路で保持する。
build IDは独立承認ではない。次の入場adapterが承認manifestのartifact closureへ拘束する。

固定read-only reviewでconstructor capture後の派生class field初期化による上書きを検出し、
実runtime生成からstatusまで通す試験で修正前fail→修正後passを確認した。
最終の入力epoch・daemon・runtime・local/SSH CLIを合わせた13 filesの回帰試験は
1523 tests pass、1 skip。node/CLI typecheckと対象lintもpass。
実app切替・startup admissionの受入とは区別する。
主要承認hash: runtime identity `58f99cef5cbb7a34b336bdc2f51199ee04997a3abe1e63da97b2f1296f54ab41`、
runtime回帰 `79f9233285806dcaff9c860649622d55d54226d84f60f7096d51988473e16263`、
SSH投影 `612094ae5c2a0b63b279ea003a604c072ba07e31a7b05c305689d7946fa0199e`。
Help No impact：開発アプリの状態照会にのみ追加し、ゲームの操作・表示・成立条件は不変。

### H5: 一貫受入・運用切替（S9）

- H0からfixtureを作り、H1/H2を独立検証、H3→H4→H5の順に統合する。
- [ ] 受付→A実装→固定review差戻し→再実装→統合→native模擬処理→文書→全体終了を無介入で一巡。
- [ ] 再起動・応答消失・busy・source変更・誤版登録・明示pauseを各保存境界へ注入。
- [ ] 固定reviewの承認対象と最終source/検証対象が一致する。
- [ ] cold startと途中再開を実Orcaの同ターミナルで確認。終了時に必要な履歴が見える。
- [ ] 残件なしを確認後に運用ガイドへ集約し、重複計画を整理する。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| 「自動復旧」が二重実行・承認迂回になる | 副作用分類、同ID、exact証拠、WAL、保存直前のcontrol検査 |
| pinを緩めて版不一致を隠す | reviewed manifestで互換組合せを固定。未対応版は書込前拒否 |
| 巨大な共通化で既存復旧が壊れる | helper adapterから段階移行、旧receiptの読み取り互換とfixture |
| 監視と統括の二重所有 | 製品工程は統括のみ。監視は基盤修正と証拠照合に限定 |
| UI刷新で履歴が見えなくなる | 既存terminalを正本とし、移動先と折畳みだけを検証 |
| storage未終了を無視して成功扱い | owner付き進行中と孤立を区別し、close時の厳格checkは維持 |

## 7. 検証計画・データ管理

- 変更対象ごとのunit/integration、拒否系、保存境界の障害注入、固定read-only reviewを必須とする。
- base/head/dirty sourceを固定し、同一対象のCIまたは
  `python3 scripts/dev.py ci check --base <full-SHA> --mode auto`で必要群を確認する。
- 基盤のcontracts/tooling、本体変更時のtypecheck/lint/tests/梱包検査を実施。
  分類不能なら勝手にゲーム検証へ広げずscopeを整理する。ゲームbuild/testは本計画では行わない。
- Helpは実diffでレビュー。開発用制御のみならNo impact、ゲーム経路を変える場合は別途扱う。
- 実UI受入は非ゲームfixtureの統括・担当・review terminalで行う。実案件の失敗を捏造しない。
- 正本は[保存ワークフロー](../development-infra/validation-storage-workflow.md)。primaryで登録・確認する。
- 本計画書作成では新規batch、binary copy、worktree、原本削除はない。
- 実装時のbatch/subject/owner/consumer/roots/開始終了bytesは各実行前に記録する。
  既存candidateとcacheは現在の利用者がいる間保持し、一律期限・容量上限を追加しない。
- 採用成果と簡潔な結果を正本へ、不要jobは所有と正の終了証拠を確認後に整理する。

## 8. ロールバック方針

独立した変更単位でreview済み版へ戻す。先に稼働操作・所有・schema互換を照合し、
台帳・会話・成果・Git履歴を巻き戻さない。稼働担当を強制終了せず、安全なhandoff点まで待つ。
不明な副作用をrollback名目で再実行しない。

## 9. AI引継ぎメモ

- 現在地: H1/H2-a/H3-aは全gate・固定review・同統括反映済み。
  H1は実host登録からfinalized pass・固定review再投入まで観測。H3-bはmail回収補正まで固定review・全gate合格、監督controller・同統括Driver反映と実review再開を確認。
  H3-cは同Driverで自動通知・staged route適用を確認したが、A起動のunknownで停止。
  H0の共通契約、H2の終了/submit回復、H4/H5の一貫受入は未完。
  既存M1〜M19の成果は保全し、局所成功を全体完成へ読み替えない。
- 次工程: H2の起動送信WAL・終了/submit契約へ進む。H4画面5シナリオは受入済み。
  live brokerのcode revisionを途中変更せず、配備と安全な反映点を先に確定する。
- 本計画は横断残件の正本。既存計画の過去ログは証拠として残し、二重の実装backlogにしない。
- 参照必須: 上記関連計画、`orca-coordinator-continuation.md`、`orca-native-host-execution.md`、
  `orca-request-lifecycle.md`、保存ワークフロー、実装対象helperとtests。
- 本文書だけでTAK-14の条件・実装scope・外部操作権限を変更しない。

### Definition of Done

2026-10-02 起動経路の追加是正（候補のみ、未配備）:

- 新しい統括を開くcommandへ、解決済み`ORCA_UI_COORDINATOR_SCRIPT`を明示的に埋め込む。
  appより長生きしたterminal daemonの古い環境を継承しても、指定したhelperへ拘束する。
- `launch`はDriver/Broker import・provider起動前、`default-entry`はtab/台帳操作前に、
  設定pathと実行中moduleのpath一致を要求する。設定だけ新しく実装は旧helperという混在を拒否する。
- 起動promptは今回のhelper directoryを明示し、保存会話にある旧pathや凍結subjectのhelperへ
  戻ること、履歴上限を理由に保存台帳を短縮することを禁止する。
- 引用符を含むpath・実childでの古い環境上書き・混在時の副作用前拒否を含む4回帰testを追加。
  UI coordinator/activation/finalizationの関連62件、Ruff、diff checkがpass。
  固定read-only reviewはこの追加差分だけを承認した。
  coordinator SHA256: `6fa94254022eecb491757f730fffc0d1703a54f73ed877882fc51d94c8945624`。
  test SHA256: `2110eb57c3b7d8b82ca84081195713b4e6ea4871b1a785022569a650c01b5d22`。
- Help判断はNo impact。開発用の起動commandと制御pathだけを変更し、gameのinput・表示・save・
  runtime data・player Helpのconsumerへ接続しない。ゲームbuild/testは実行していない。
- primary storage checkは177 batches・未分類0・669394104320 bytesでpass。削除は行っていない。
- 現runtime `d9aba80c-fa91-4f47-81ec-aef1666f3429`はloaded release証拠を返さず、
  app processにもsupervision設定を確認できない。監督snapshotは再起動前runtimeのまま。
  TAK-14のcoordinator registryはschema1/readyを保持しているが、そのterminal handleは現runtimeにない。
  これを工程進行・正規終了・配備済みの証拠として使わない。
- 本修正はpath一致のみ。同一path上のartifact交換、cold bootstrapへの全reader接続、
  正式終了照合・安全な配備・同依頼再開・H5一貫受入は引き続き未完。
- 2026-10-02限定反映: 上記のpath固定・混在拒否・prompt拘束だけを、設定済みhelper
  `/home/satotakumi/orca/workspaces/hell-workers/orca-abcd-expansion/scripts/orca_ui_coordinator.py`
  へbackportした。H4全体の配備待ちをこの独立修正の停止理由にはしない。
  サービス設定のないapp・対象Python process不在・新規入口を起動しない条件を確認し、
  `ui-coordinator`と`frontdesk-ui`の所有lockを保持してmain agentが限定hunkを適用した。
  適用前後のruntimeは`d9aba80c-fa91-4f47-81ec-aef1666f3429`で一致し、TAK-14のterminalは空。
  新規hunkだけをメモリ上で除いたsource hashは適用前
  `e8b88a7baf8e57585a87aa1d163a286b278bbd3fb3fc4ef0b1a2e1b0b0843cd8`と一致し、既存dirtyを保持した。
  live helperに存在するUI coordinator/coordinator/issue context/frontdeskの92 tests、
  対象Ruff・diff checkがpass。候補にだけ存在するactivation/finalization test名を初回に
  誤指定して2 loader errorsが出たため、その試験をlive版の合格証拠に含めていない。
  fixed read-only reviewはこの限定反映を承認した。反映後coordinator hashは
  `abede38b4536b831303098eae3b633222df722be94f8f88e5a858a01e48e66be`、
  test hashは`2110eb57c3b7d8b82ca84081195713b4e6ea4871b1a785022569a650c01b5d22`。
  Help No impact: 開発用起動からhelper選択・拒否に至る経路だけを変更し、ゲームのinput・
  表示・save・assets・player Help consumerへ接続しない。ゲームbuild/testは実行していない。
  primary storageは177 batches・未分類0・669394923520 bytesでpass。原本削除はない。
  これは設定先のコード反映であり、稼働appへの読込・保存済み旧commandの更新・監督service起動・
  同sessionの新turn/次工程をまだ確認していない。TAK-14は停止中で復旧完了とは扱わない。
- 2026-10-02追加観測: hostのbootは同日02:15:02（Asia/Ho_Chi_Minh）、boot IDは
  `117677d4-de8c-4686-a75b-a057f8deb767`。旧統括tabの履歴metaは
  `startedAt=2026-10-01T18:46:36.560Z`で、`endedAt/exitCode`を記録していない。
  registryもschema1/readyのまま。host再起動後の保存表示をlive processと読み替えない。
  ただし旧registryにはprocess birthがなく、この時刻比較だけでproviderの正常終了・
  raw return code・unknown送信の不実行を捏造しない。再起動をまたぐ未終了activationの
  正式な照合・履歴保全・同session再開をH2/H4の受入条件に含める。
  その所有検査では、schema2のlauncherと直接childが別boot、またはchild出生がlauncherより
  前という不可能なbindingも拒否する。現bootとの差だけで旧recordを破損とせず、過去の正当な
  recordを読める状態を維持する。この検査は終了proof・旧schema1の取消許可ではない。
  candidateへこの検査と別boot・逆転tick・同tick・過去record保持の回帰を追加し、
  activation/fence/cold observation関連59 tests、Ruff・diff checkに合格した。
  coordinator activation hashは`9eb73eacfbb4a335114598fdc2272435296b9dc4d1e1d0b007b32fc662290c88`、
  test hashは`ae35bd6c16dc416518b9baa9662aef0f4b5f2ac097d70afe77a428c355e2665b`。
  Help No impact: 開発用所有bindingの検査だけでgame consumerへ接続しない。
  primary storageは177 batches・未分類0・669395001344 bytesでpass。削除なし。
  fixed read-only reviewの初回はcapacity errorで未成立となり、同reviewerへ再依頼した。未配備。
  legacy取消の設計確認も同reviewerへ依頼した。旧native continuationにはbirth記録があるが
  旧term_b9fbに属し、現registryのterm_2d1fの終了証拠には流用しない。
  同runtimeの正式`run-show`ではRun `run_3236488f0dff`のconsumerが
  `term_b9fb015e-4bb9-4f08-827f-06fe7ecaec67`、consumer_generation=1、legacy=0のままで、
  registryの`term_2d1f4b5a-4e46-4351-a5e5-5401bbc0bcc5`と一致しないことを確認した。
  単にregistryの拒否を解除して通常Driverを起動しても、Run consumer照合で拒否される。
  復旧を二段階に分け、履歴を保全したinterruption/supersessionの正式receiptを先に作り、
  同sessionを副作用禁止の復旧モードへ戻す。通知・mail consuming/ACK・通常Driverは開かず、
  同Runの正式consumer移行とpending操作の照合が完了した場合だけ通常実行を再開する。
  この案の実装・固定review・実受入は残件であり、`run-use`はまだ実行していない。
  Orca本体の`db/runs/run-binding.ts`では、consumer変更は世代を増やし、未ACK Deliveryを
  fenceし、旧/新handleの未読直接mailをRun mailboxへ移す。同一bindingの再照会と変更を区別し、
  移行前世代のDeliveryへACKを再送しない。現Runは非legacyであるため、adopted Run限定の
  `--takeover-legacy`を使う経路へ誤分類しない。結果不明時は移行WALの同operation IDと
  正式receiptを照合し、新しいRunや二回目のconsumer変更で回避しない。
  再依頼後、fixed read-only reviewerはactivationの限定差分を上記exact hashで承認した。
  配備・TAK-14再開は承認対象ではない。復旧設計も実sourceから次の形で確認した。
  `orca_coordinator_recovery.py`の専用入口を追加し、read-only `inspect`→明示的旧所有権失効
  `revoke`→receipt一回消費の`prepare_recovery`→通常実行admissionの`enable_execution`に分離する。
  receiptは旧registryのraw bytes/hash・boot/runtime・terminal・session prefix/hash・loop/Run/
  control/source digestと未確定operationを保持し、exit code/自然終了/未送信証明を補作しない。
  通常prepare/acknowledge/require_ready/Driverも同receiptの失効を強制検査し、optional sidecarの
  存在だけで終わらせない。既存のlock順序・全host inventory・pause/close・pending instructionを守る。
  復帰sandboxはhistory/tmp以外ROで、実Orca discovery/token/socket・他認証・effectful Brokerを
  非公開にする。status/指定terminal show/waitだけの限定preflight権限を用い、文章だけの禁止にしない。
  通常DriverはRun consumer移行・元message ID/settlement/release・sourceの正規照合後に限り解放する。
  unknown ACK/送信は保存し、返答消失時の新operation発行、旧世代ACKの新世代コピーを拒否する。
  実装試験には二重消費、各保存境界の中断、source/control/runtime変更、旧receipt混同、意図的pause、
  unknown送信不変、旧ACK拒否、Run移行応答消失、sandboxのwrite/IPC拒否を含める。
  この復旧単位をH4全体の完成待ちにはしないが、設計レビューを稼働証拠には扱わない。

- [ ] S1〜S9すべてに実装/配備/実受入の証拠、または明示的な対象外理由がある。
- [ ] 手動の継続prompt・内部ID・Enterなしの一貫E2Eが成功。
- [ ] 同sourceの変更別検証・固定review・Help確認・必要な実UI受入が成功。
- [ ] 最終storage確認と、保持する各資源の具体的利用者・終了条件が記録されている。
- [ ] docs/運用手順と索引を同期し、局所復旧を全体完成と報告していない。

- 2026-10-02復旧単位の第一段階: candidateの`orca_coordinator_recovery.py`へread-only
  `inspect`を実装した。実mountをROにした別namespace内でcanonical registry/loop/lifecycle/
  runtime/control/session/sourceを読み、同じ対象を再観測して一致を要求する。
  RO rootだけでなくhelper/state/sourceのRW child mountも拒否し、canonical reader import前に
  helper/stateを検査する。HOME依存ではなくpasswdのaccount homeへ検査rootとmaskを揃える。
  `~/.config`全体、provider既定rootと設定済みcredential rootを隠し、networkを隔離する。
  実Orca socketやCursor認証を必要としない入口である。親processも戻り値の全shape・型・
  request/session・scope・digestを検査し、同bytesのsession交換もinode/device/mtime/ctimeで
  別subjectとして扱う。inactive/unresolved controlを許可へ正規化せず拒否する。
  timeout/拒否を終了証明や再実行許可へ変換しない。
  15単体test（実bwrapのRO write拒否、RW子mount拒否、変更HOMEとRW実stateの拒否、
  default Cursorの無害fixture非公開、custom認証fixtureとUNIX socket非公開・接続拒否を含む）がpass。
  最終sourceで関連5moduleの120 testsもpass。Ruff・diff checkがpass。
  module hashは`992f8a601b7bb660216694892cdae5426d063fc9a8debb507fc9aea5db042247`、
  test hashは`5597490353fe45ed78ab274379690afd1f054fa82385cb730042dfc2f2cbc129`。
  固定reviewのRW子mount・不完全戻り値・HOME/account rootの指摘を補正し、同reviewerが
  上記exact hashesのread-only inspection単位を限定承認した。
  Help判断はNo impact: developer-owned制御台帳・保存session・source fingerprintの読取だけで、
  ゲームinput/render/save/runtime data/Help consumerへ接続しない。Help gateの既存commit判断を
  今回の実経路判断の代用にしていない。primary storageは177 batches・未分類0・
  669395324928 bytesでpass。専用出力・新worktree・binary・原本削除はない。
  本段階は失効receipt、Run consumer移行、launch、通常実行admissionを実装していない。
  承認後、candidateのread-only sandboxでTAK-14の実inspectionを実行し成功した。
  subject digestは`75d5706341706b088be72b4bf7ef4989078627bddd7e965d16146666d058e84b`、
  controlはactive、sourceはTAK-14本体とfull-acceptance-repairを含み、保存sessionの
  844889319 bytesを前後照合した。registry/loop/snapshotのfile hashは実行前後で不変。
  digestを旧process終了・未送信・失効許可・Run整合・依頼完了の証明にはしない。
  同runtimeには依然TAK-14 terminalがなく、snapshotは旧runtimeのまま。
  configured helperへの配備、正式失効receipt、同Runのconsumer移行、TAK-14再開は未完。

- 2026-10-02失効単位の下地: candidateへ`orca_coordinator_revocation.py`を追加した。
  canonical inspection、旧registryの原bytes/hash、operation、許可の参照元を保持する不変receiptを
  作成・検査するlibraryである。参照元の文字列自体を許可の検証と読み替えない。
  publish先はaccount固定pathに限定し、原registryを変更せずatomic no-replaceで一度だけ発行する。
  同operation/同subjectの再照会だけを許し、別operation、null/malformed、symlink、危険なowner/
  permission、未確定hardlinkを拒否する。receiptは`interruption_fenced`であり、自然終了や
  exit code、通知不実行、Run移行の証拠を追加しない。
  present fenceは通常prepare/acknowledge/ready/save_state、dispatchの旧consultation fallback、
  Driver開始と各iteration、直接loop tick/visible coordinator検査、Broker生成とauthorizeを拒否する。
  read-only registry inspectionは保持し、receiptだけで通常実行を解放する例外は追加していない。
  CLI publisherは公開しておらず、lock・既存人間許可・source/control/runtime・全host inventoryを
  検証する上位transaction、およびconsume/Run移行/enableは未実装。実sidecarを発行していない。
  当該下地を正式revoke command、旧launchの終了、TAK-14復旧完了とは扱わない。
  初期6 testsとinspection/UI/native関連110 tests、初期補正後のrevocation/loop/dispatch関連105 tests、
  Ruff・diff checkがpass。初期6filesはfixed read-only reviewerが非公開下地として限定承認した。
  追加でworktree bindingを使うprepare前UI入口、独立SyncWorkerのlock前後、follow-up保存/送信前、
  固定review tab再試行、既知requestのlaunch runtime準備前へgateを追加し、Driver/tickはlease後にも
  検査する。worktree inventoryはunknown/null/不完全bindingを別worktreeと読み替えず拒否する。
  最終12新testsには、hash整合した欠落/null/型不正/別repoの拒否、lease中の失効、follow-upの
  instruction/WAL/sendゼロ、link応答消失/fsync不明後の同receipt再照会・原inode保持を含む。
  追加guard後のUI/runtime/supervision関連138 tests、loop/dispatch/native関連149 testsがpass。
  最終12新testsもpass。追加範囲もfixed reviewerのworktree binding指摘を補正後、
  同reviewerがexact source/testを非公開下地として限定承認した。全復旧経路の承認ではない。
  revocation hashは`98cf3a4c4f6d37f153cfef9e7b3f9bc2f314c431133212a79949f1a0ecb3913e`、
  test hashは`6af8681ae6c440985c017264cb2b6d9c7836fca8276535a4cf0a66abc1211705`。
  実適用前にはrouteのraw ready再採用、直接validation/review/settlement/maintenance復旧、
  native continuation、external reconcile/advanceの全副作用入口を閉じる。supervisionの停止意思保存は
  禁止せず、resume/close/未確定closeの副作用を分離する。publisherは各入口と同じ排他を保持し、
  排他後の再検査を網羅する。save_stateの一律拒否で正の終了を失うため、現下地をlive ownerへ
  発行せず、静止確認・正式終了の保存と失効を区別する上位transactionを完成させてから適用する。
  Help No impact: 開発用統括のauthority検査とreceipt保全だけで、game入力・表示・save・assets・
  player Help consumerを変更しない。ゲームbuild/testは実行していない。
  primary最終storageは177 batches・未分類0・669395746816 bytesでpass。削除・配備はしていない。

- 2026-10-02失効gateの追加網羅: candidateの独立settlement/maintenance復旧、routeの親と
  既知child（origin保存・terminal開始・raw ready再採用前）、runtimeのsave/reconcile/advance、
  native continuationのowner検査・handoff・advance・直接reconcile、直接Help/validation/review
  復旧入口へpresent fence拒否を追加した。主要lease/scheduler/request lock後にも再検査する。
  native reconcileの二つのlock窓はfixed reviewの指摘で補正し、lease前を通過して取得中に
  失効した場合もread/WAL/writeへ到達しないtestを追加した。read-only registryと人間の
  pause/close意思受付は一律に禁止していない。finish_close/未確定closeの副作用はchild gateで拒否する。
  最終14新testsには独立21入口の副作用前拒否と両native reconcileのlease後拒否を含む。
  関連7module194 tests、loop/local recovery/attention111 tests、最終native/revocation83 tests、
  Ruff・diff checkがpass。fixed reviewerがnativeのlock窓補正を含む最新source/testを限定承認した。
  test hashは`0c6ed459a505f3bd7171f7e06eebfa6da34552f9c79b42c62380b9db6be88af4`、
  native continuation hashは`b3a6831e09fea684f249b67bb13e4e02f9a3c8ccdde6c16115f7c4b6c00a41d8`。
  Help No impact: developer-owned authority拒否と復旧制御だけで、game input/render/save/assets/
  runtime text/player Helpへ到達しない。既存commitのHelp gate結果をこの実経路判断と混同しない。
  primary storageは177 batches・未分類0・669396246528 bytesでpass。削除・配備はしていない。
  実適用前の残件にはtooling_correctionのcategory別raw registry経路、request未指定の
  launch/launch-wait直接入口、全入口とpublisherの共通排他、
  真の終了保存と失効の区別、上位revoke transaction、receipt消費、同Run移行、通常実行解放を含む。
  この追加網羅を全effectful入口の閉鎖やlive失効の完成と報告しない。実receiptは依然未発行。
  現runtimeとTAK-14のterminal空一覧は不変で、Run consumerもterm_b9fb/世代1のまま。
  同sessionの新turn・次工程は確認できておらず、再開は未完である。

- 2026-10-02残る二つの入口を候補補正: request未指定の`launch`もmanaged terminalのworktree
  fenceをruntime準備・provider探索・import前に検査し、launch admission後にも再検査する。
  `launch-wait`はこの入口を呼ぶため同じ拒否条件を守る。tooling_correctionのcorrect/recoverは
  三つのcategoryすべてで入口・scheduler lease後に検査する。実worktree receiptを用いた
  launch/launch-waitの拒否と、三categoryのcorrect/recoverのread/lockゼロを回帰で確認した。
  revocation/UI/tooling関連71 tests、Ruff・diff checkがpass。fixed read-only reviewerが
  下記exact source/testの追加3filesを非公開下地として限定承認した。live発行・配備・Run移行の
  承認や再開証明ではない。
  UI hashは`e2c90591982d34c700223e448f3fa5b2d55f8a3a80ef1d0826571dfb33da11e5`、
  tooling hashは`3d11b3c830356867114ae8310db9a049998b4e92c953bcc9c2e40ee57aa71335`、
  test hashは`180152281e03131e61503378f2cfd41bcf0efe2f8672cc6297232dd7364397e7`。
  Help No impact: developer起動と権限拒否のみでgame入力/表示/save/assets/Helpは不変。
  primary storageは177 batches・未分類0・669396312064 bytesでpass。削除・配備なし。
  本補正も非公開下地であり、live ownerの失効や正の終了/送信結果保存は未対応。
  上位transactionの全関係lease/静止/原本保全、receipt消費、同Run consumer移行、実行解放と
  同session新turn・具体的次工程の確認を次の実装対象とする。TAK-14の停止状態は変化していない。

- 2026-10-02上位復旧transactionの原本捕捉を候補実装: `orca_recovery_registry.py`は
  canonical inspectionのrequest/session/subject digestを検査し、account固定registry pathから
  private regular inodeを非作成・NOFOLLOW/NONBLOCKで読む。FD前後とpathのinode/size/timeを
  照合し、元JSONのcanonical digestがinspectionと一致する場合だけ、整形しない原bytesと
  inode identityを返す。欠落、別subject、symlink/hardlink/FIFO、不正permission、同bytesの
  inode交換を拒否する。原registryの終了記録・改変・失効・起動は行わない。
  fixed reviewの親directory差替えP2を補正し、NOFOLLOW directory FDを固定してbasenameを
  相対openする。終了時もdirectory identityとcanonical pathを再検査する。pin前・pin後の
  親差替え拒否を含む新規8 testsとinspection/revocation関連の合計38 tests、Ruff、diff checkがpass。
  fixed read-only reviewerが最新追加2filesを読取専用captureとして限定承認した。
  module hashは`d8c6ce40b9d16fab2caeab00115caa5406e2f6d1bcace334ab203aff00febb98`、
  test hashは`300654f07b815d380d528eaec30b94a26a597ce6f6d5a8f2162d3274335cf8c0`。
  上位transactionのleaseと再検査を代替せず、
  capture単体を許可・終了証明・Run移行・execution admissionとしない。
  Help No impact: 開発用registry原本読取だけでplayer input/render/save/assets/Help consumerは不変。
  primary storageは177 batches・未分類0・669396430848 bytesでpass。削除・配備なし。
  TAK-14はterminal空一覧、旧Run consumer世代1、旧snapshotのままで再開未完。

- 2026-10-03上位復旧transactionの共通排他を候補実装: `orca_recovery_leases.py`は
  validated loopのrequest、親request、lane ticket、integration review ticket、対象repoから
  既知cooperative lease setを構築し、sort/dedupしてinheritなし・nonblockingで取得する。
  UI/publication、driver、scheduler、native、親子request runtime/external executor、route/outbox、
  worker/reviewer/heavy、workspace、dispatchの既存keyを使う。busy/errorで部分取得を全解放し、
  全取得後にも同loop digestを確認してからcallerへ渡す。busy時の中断・無条件retryはしない。
  このprimitiveは人間許可、親子binding、source/control/runtime、全host inventory、正の終了、
  unknown operationのreconcile、Run移行の確認を代替せず、公開CLI/publisherへの接続も未実装。
  新規5 testsとregistry/inspection/revocation関連43 tests、Ruff・diff checkがpass。
  fixed reviewで不足した親子lifecycle lease、各repoのrole-tabs lease、各lane review ticketの
  dispatch leaseを追加し、既存producerとのkey一致を回帰確認した。同reviewerが最新2filesを
  非公開cooperative lease primitiveとして限定承認した。上位復旧や実運用への適用承認ではない。
  module hashは`d8bf6c6ae0ef258fc44e7edad6f8d6fadc32bfa7c88d73084085e669d0248cfb`、
  test hashは`e3b43eebf5747a417e21cca38e74d8f85f7e71118bc3eac976e09b3e3eae8d1c`。
  Help No impact: developer復旧用の排他だけでgame input/render/save/assets/player Helpは不変。
  primary storageは177 batches・未分類0・669396545536 bytesでpass。削除・配備なし。
  現runtimeはd9aba80cのまま、TAK-14のterminalは全host確認済み空一覧。
  旧registry・Run・snapshotを保全し、同session新turnと具体的次工程の再開はまだ未完。

- 2026-10-03共通排他と原本捕捉をjoint preflightへ接続: candidateの
  `orca_recovery_preflight.py`は、canonical inspection、UI origin、settled loop/mailbox、
  原registry bytes/inode、現app PID/birth/runtimeId、全local hostを含む空terminal inventory、
  既存Run id/consumer generation/coordinatorを同じlease区間で前後照合する。
  runtime変化、partial inventory、live terminal、bool generation、旧consumerとの不一致、
  source/control/registry/loop/origin変化を拒否する。CLI観測はstatus/list/run-showのみで、
  30秒timeout後に再実行しない。公開checkはcheckpointを返すだけでwriter、send、ACK、
  terminal作成、run-useはなく、終了・失効・Run移行・launch admissionを与えない。
  fixed reviewで、originの親IDとcontrolが読むrouteの親IDを独立に扱うP2を検出した。
  canonical route readerへ非作成`create=False`を追加し、originとrouteの親子・ready・
  issueIdentifier・worktreeIdを一致確認する。route全文を前後subjectに含め、欠落・別親子・
  非ready・別課題・別worktree・bool schema、親pauseのCLI前拒否とmissing directory非作成を
  回帰化した。復旧foundationとsupervision/route関連109 tests、Ruff・diff checkがpass。
  fixed read-only reviewerがこのread-only checkpointとreaderのcreate引数差分を限定承認した。
  preflight SHA256は`bbe7ceca16e7572d9d446726081519a70e6e1f0006cc394111b7e73c6d9be4eb`、
  testは`1eec7c308494c8833f700400eed5b6b9482f80e330ee83ed076a9ba20568ac6d`。
  Help No impact: 開発用の保存権限とruntimeのRO照合だけでgame input/render/save/assets/
  player Help consumerは不変。ゲームbuild/testは実行していない。
  primary storageは177 batches・未分類0・669396705280 bytesでpass。削除・配備なし。
  承認後、selected CLIで実checkpointを一度実施してpassした。subject SHA256は
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`。
  runtime `d9aba80c-fa91-4f47-81ec-aef1666f3429`、app PID10839のboot/birth、同Run
  `run_3236488f0dff`・consumer generation1・旧owner term_b9fbを前後照合した。
  TAK-14は全local hostを含むterminal inventoryが0件。原registry raw SHA256は
  `2e33b722c4eae3f26f533904ce239596073eec402bd86aaf01a511f57e014bb7`で保持した。
  この結果は観測だけで、原registry/Run/sessionの変更、失効receipt発行、起動はしていない。
  次の実装単位は下地の追加ではなく、leaseを保持した失効→consume→同Run移行→enable→
  同session起動をつなぐ上位transactionとする。旧自然終了を捏造せず、interruptionを独立記録する。
  Run移行は期待owner/generationの原子的照合とdurable操作IDを要求し、結果不明時は原操作を
  read-backする。履歴attemptのcoordinatorを新ownerへ上書きしない。新ownerとRunの投影が
  確定するまでDriver/Broker/providerを起動しない。fenceの発行だけでliveをさらに止めないよう、
  consume/enableと正常終了保存まで完成・検証してから適用する。
  checkpointだけを実復旧と報告しない。現時点でTAK-14の統括・次工程は再開未完。

- 2026-10-03同Run移行の原子的guardをOrca本体候補へ実装: 既存`bindRun`の
  `BEGIN IMMEDIATE`内で期待coordinator handleとconsumer generationを照合する。
  guarded経路では未ACK Delivery、adopted legacy Run、移行先paneに別Runがある場合も拒否し、
  mail移送・他Run解除・waiter取消より前にrollbackする。既存の通常run-useの契約は変えない。
  必須期待値を持つ新RPC`orchestration.runUseGuarded`を既存handlerへ接続し、durable mutationに
  分類した。CLIのrun-useへ両方必須の`--expected-coordinator / --expected-generation`を追加し、
  この指定時は新RPCだけを使う。旧hostのmethod_not_foundで無条件run-useへfallbackしない。
  同request IDの成功replayは保存receiptを返し、consumer generationを二重増加させない。
  これは開発側の復旧APIであり、利用者へ内部handleやgenerationの入力を要求するものではない。
  candidateは`/home/satotakumi/tools/orca-ui-lifecycle`。関連8files・155testsがpass。
  node/CLI型検査、targeted Oxlint、RPC catalog freshness、diff checkはpass。
  fixed read-only reviewerが最新destination guardと追加3testsを含むCAS API差分を限定承認した。
  他dirty差分、復旧admission、配備の承認ではない。配備・実Run変更・アプリ再起動はしていない。
  DB source SHA256は`7207ba0cc245724951425a28a39eb95bb279956ddc414ddf0ef9a4c8f116dd08`、
  DBtestは`50d212ba6ecd19b2c35f7106fc84a0ba1911a145507f3dd30535d16db5abce5e`。
  Help No impact: developer CLI→RPC→OrcaのRun/mailboxだけを変更し、ゲームの
  input/render/save/assets/player Helpへのproducer/consumer経路はない。Help source不変。
  primary storageは177 batches・未分類0・669396799488 bytesでpass。削除・公開・配備なし。
  TAK-14はruntime d9aba80c、terminal0件、旧registry ready、旧Run consumer1、
  古いsnapshotのまま。旧registryの失効と新ownerへのconsume/enable、同session起動、
  新turnと具体的次工程の確認は引き続き未実装／未実行で、CAS APIのpassを復旧完了としない。

- 2026-10-03復旧照合の非決定性を修正候補で解消: 同じ原registry/loop/Run/sessionでも
  `control.revision`だけが別processで変わり、joint preflightが拒否されることを実見した。
  `control_snapshot`が親子requestのsetをそのままlistへ追加してhash化していたため、
  Pythonのhash seedで順序が変化していた。routesと親子requestの順序をcanonical sortし、
  revisionの一致条件を弱めず修正した。pause、pending instruction、複数親の拒否は維持する。
  subprocessのseed 1/2/3/4/5/42で同一revision、実際の親control変更でrevision不一致、
  親pauseでinactiveとなる回帰testを追加した。関連92tests・Ruffがpass。
  candidateのjoint read-only preflightを別processで2回実行し、同じsubject SHA
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`を確認した。
  source SHAは`07cab0f8b909be3810b906e1b3ca345f9c691a7ca8773ae4b9c349a1b95fc4c3`、
  test SHAは`6a2132b1c9062bced8c23fd58c6ad004402cb1f9849c5e5aab5717b3dafc8cfe`。
  Help No impact: developer-only continuationのcontrol証拠の順序だけを変え、ゲーム入力・
  player表示・save/assets・Help catalogへの経路を追加変更しない。並行製品差分の承認ではない。
  primary storageは177 batches・未分類0・669396914176 bytesでpass。削除なし。
  固定read-only reviewerがこの2sortとtestを限定承認した。configured helper
  `orca-abcd-expansion`のbaseline SHA
  `1324e4690e15acb0d7bf0fd79451b3a00ed8c4ade50ba656213117f26f7515c7`を確認し、
  この2sortだけを限定適用した。適用後SHA
  `14de3f9a231b70ccd5f0f7e561dcf0ee3f373c0bd55edc5b4716a5990d4a1c0d`、
  69 native-continuation tooling testsとdiff checkがpass。逆変換hashでも他差分の混入なしを確認した。
  ここでnativeという名称はOrcaのprovider通信helperであり、ゲーム実機testは実施していない。
  同Run移行transaction、consume/enable、同sessionの新turnと次工程はまだ未完であり、
  read-only preflightのpassを実稼働とは扱わない。

- 2026-10-03同Runのconsumer移行後に歴史Dispatchを誤拒否する経路を候補修正:
  mailboxのmessageとinterruption proof、refused completionの再照合が原Dispatchの
  coordinatorではなく現在のloop terminalを使っていた。`attempt_authority`で保存済み
  attemptのrun/task/dispatch/coordinatorを要求し、同Runであることを照合して3経路へ接続した。
  不明・欠損ownerを現在のconsumerで補わず、原journal/arm/settlement照合とACK条件を維持する。
  歴史messageと正式interruption receiptはconsumer generation変更後も照合でき、
  wrong/missing owner・別Runは拒否し、原attempt/ACK/releasedは変更しないtestを追加した。
  上位adoptionではrun contextを新dictへ置換し、旧attempt内のshared_runをin-placeで
  新generationへ書き換えないことも確認した。関連124tests・Ruff・diff checkがpass。
  旧UI driver-admission testは先行installation/env guardをfixture境界でmockして
  本来のDriver-before-provider拒否を検証するよう補正した。guardのproduction変更はしていない。
  mail SHA `5d7f48477b26733a2c8da4e1f63829a58bdc6f6583676d20fbdb40b3796a3d09`、
  completion SHA `d0a01bf00d80b93395174cceca6d688c57b46e5f9812897c2040d8d0c0e0410c`。
  Help No impact: developer-only Dispatch authority→bridge receipt/Orca mailboxの照合だけを
  変更し、ゲームの入力・表示・成立条件・save/assets・Help consumerへ到達しない。
  並行製品差分の包括的なno-impact承認ではない。primary storageは177 batches・未分類0・
  669397344256 bytesでpass、削除なし。固定read-only reviewが今回のhelper＋3置換と
  回帰testsを限定承認し、configured helperへ同じ差分だけを適用した。
  live mailのbaseline SHA `0ae36c8364f6d4dcc1cea79c6a5f8cce6b83aceb58ec96ab860f26ebf859567f`、
  completionのbaseline SHA `33de2d28402cd30f40eb1058c43202b88bccf414cf5fa8dbb0b2abefab4c81b3`へ
  逆変換してhashが一致し、他差分の搬入がないことを確認した。適用後productionのSHAは
  上記candidateと一致し、live helperの45関連testsもpass。再起動・Run変更はしていない。
  現在の38attemptには原coordinatorが保存済みでACK/releasedも維持されている。
  live runtime/Run consumer1/terminal0件/古いsnapshotは変化なく、実際の同Run移行・
  同session新turnと次工程は未完了。候補testのpassを依頼再開や完了とはしない。

- 2026-10-03 CAS候補の配備準備として、既存UI candidateのmutable `out/` cacheで
  CLIを直接tsc compileし、host heavy lease下でElectron main targetをproduction buildした。
  両方exit 0、CLI bin/module境界検査とmain/daemonのNode構文検査がpass。
  `orchestration.runUseGuarded`はCLI handlerとmain bundleの双方に含まれる。
  global CLIを変更する`install-dev-cli`は実行せず、稼働CLI・current symlink・稼働appも不変。
  main SHA `ca3aec7cd8e2f45704bd7c83ecfd645f008aa79a80d9f70a78d61f696f611057`、
  daemon SHA `97f6028a8d2d4ff45f71ac0dd5a4809f8277b1e75423dda03a0a4f1c1c7bf52f`、
  CLI handler SHA `ec98145645d05db3c0fc9e82bf803a2031c79f7f3cb451fce3efc25eb82f4ead`。
  既存candidate cacheを再利用し、凍結した旧受入packageは上書きしていない。
  main-onlyのためrenderer/preload欠落のbuild警告は想定内だが、全desktopの梱包・
  isolated runtimeでのAPI実行・稼働版への切替の成功証拠ではない。
  consume/enable transactionと同sessionの新turn・次工程は引き続き未完。
  今回はproduction source変更なし。開発用生成bundleのみでゲームHelpの経路は不変。

- 2026-10-03 guarded Run transferのatomic receipt補正を候補実装:
  `bindRun`のCOMMITとmutation executorのreceipt保存が別transactionであり、
  間でappが停止するとconsumerだけが移行して正式receiptが残らない経路を確認した。
  trusted RPC contextのreceipt recorderをguarded bindのCOMMIT前に接続し、
  Run/mail/receiptを同一SQLite transactionで保存する。receipt保存に失敗した場合は
  すべてrollbackし、COMMIT後の通知失敗ではcompleted receiptを保持する。
  同operationを新executorから読む場合は元結果をreplayし、consumerを再移行しない。
  通常run-useの契約と公開receiptの内部column除外は維持する。
  real RPC handlerを通した通知失敗→same operation replay、receiptとRunの合同rollback、
  実際にrerouteされた未読mailのrollbackを回帰testで検証する。
  source SHA: bind `244ad4a4cd5884e1b1a6c2aaf3de9e30ad9ee265f13ab06c070144d99bb5c95e`、
  RPC `495668e4547c8d1d4305ca0d9b48564854e1e18db4d2f1380b255d24c272a0a3`。
  Help No impact: developer-only Run consumerとmutation receiptの永続化境界だけを補正し、
  ゲーム入力・表示・save/assets・Help consumerの経路は変更しない。
  先のmain buildはこの補正を含まないため、配備証拠として使わず再buildする。
  関連63tests・tc:node・targeted Oxlint・diff checkがpass。固定read-only reviewerが
  2sourceと3testのatomic receipt差分を限定承認した。指摘されたmail fixtureは
  new owner宛・同Runの未読mailへ補正し、callback内で実rerouteを確認後にrollbackを検証した。
  testのtemplate literal補正後hash `1f58ed7f155df14d70a30e64fe5c4e0e38d010548e33b0a06cb19b7a91aaeb61`
  も同reviewerが再確認・承認した。既存CAS以外のdirty差分を新たに承認したものではない。
  host heavy lease下でmutable main bundleを再buildしexit 0、Node構文検査もpass。
  新main SHA `d8665e2312fe4e7b6a805375d759caa5185b0f699f7b413b4969867c6aa54bb7`。
  凍結package・稼働appを変更せず、実process停止試験・配備は未実施。
  live runtime/Run consumer1/terminal0件は不変で、
  復旧transactionのconsume/enable・同session新turn・次工程は未完了。

- 2026-10-03 guarded Run候補を別outputで梱包したが、既存Linux glibc-floor gateが
  node-ptyの`cfsetispeed/cfsetospeed@GLIBC_2.42`を拒否しexit 1となった。
  isolated startup試験は未到達。batch `orca-guarded-run-packaged-20261003-01`は
  invalidとして正式seal/finalizeし、失敗package 1065226240 bytesだけを
  修正版とのsymbol/ABI比較用に具体的なconsumerと終了条件付きで保全した。
  元package・稼働app・Run・台帳のownershipは変更しない。
  glibc公式commit `5cf101a85aae0d703cdd8ed7b25fe288e41fdacb`とhost headers/ELFで
  2.42はspeed関数のversionだけでなくB38400の表現も変更したことを確認した。
  symbol pinだけでは旧関数へnumeric38400を渡して誤るため、既存x64/arm64の
  node-pty compatibility patchへ両setter pinと旧`__B38400` encoding選択を加える。
  old headers/その他platformは既存B38400を維持し、floor guardは緩和しない。
  実patch shimをcompileしてbaseline ELF symbolとopenpty→tcgetattrの38400 baudを
  確認する回帰test、partial patch拒否を追加。初回39関連testsがpass。
  通常pnpm patch/lock同期が完了し、最終patch SHA
  `a34c4dccdd13014ef8cf8a50a20d02524e99703f54a5e145e5762fa695b65bf3`。
  関連6files 64tests pass・Windows専用3tests skip、targeted Oxlint・diff check・
  installed patch guardがpass。pnpm execがsupply-chain policyを検証してpatch同期と
  既存postinstallを自動実行した後、新subjectで強制Electron native rebuildを実施した。
  新batch `orca-guarded-run-packaged-20261003-02`はLinux floor 17 native binaries、
  daemon/plain Node、plugin resource gatesとbackground isolated startup 1testがpass。
  独立verifierが正しいisolated home/profile、全window hidden、loaded identity、
  現在のsource/build fingerprintとexact package fingerprintを照合し、正式seal/finalizeした。
  package fingerprint `b2ed80e6c4a09c89ee5ec03d8c3486d187e39ed3c096ee1776e16998058ca4cf`、
  acceptance fingerprint `edc4587f8ded55fecf9ac14fba00c8065ff8e2d3700fc631bad8ff0746f2ec23`、
  app.asar SHA `6c1b4ab6facc2a29697c039cb3d2cf6121a08578ce412d0ec4b2d2a50151f96f`。
  修正版packageの両speed setterはGLIBC_2.2.5で、旧2.42 importを除去した。
  storageのsubject fingerprintは当該別appに対して空分類なので、source一致の証拠には
  用いず、独立verifierの上記source/build/package fingerprintを使用する。
  固定read-only reviewerが最終patch・lockを含む今回6pathsの限定差分を承認した。
  既存patch全体・他dirty source・配備・依頼再開の承認へは拡張しない。
  新packageは後続のAPI受入・復帰可能な配備の
  concrete consumerを持つが、稼働版へは未適用。今回のstartup試験は実Run移行の試験ではない。
  startup中のpreflightは一時拒否、終了後の再照合では元checkpoint SHA
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`に一致した。
  旧失敗packageの比較終了後、今回生成した
  `/home/satotakumi/tools/orca-ui-lifecycle/.git/orca-ui-acceptance/guarded-run-20261003-01`
  だけを撤去した（1065226240 allocated bytes、再梱包可能）。制作原本・source・稼働版・
  凍結旧受入package・新02 packageは保持し、01 diagnostic consumerは正式releaseした。
  Help No impact: 開発ツールのnative dependency ABIだけで、ゲームの入力・表示・
  成立条件・save/assets・Helpのproducer/consumerは変更しない。並行製品変更を
  包括承認するものではない。TAK-14の同session新turn・次工程は依然未確認。

- 2026-10-03 packaged RPCの拒否経路を追加受入。既存02 packageを再梱包せず、
  isolated home/profile・hidden windowsの既存試験へ、実`orchestration.runUseGuarded`の
  未存在pane拒否と、別RuntimeClient・同request IDでの再拒否、Run一覧不変を追加した。
  初期schemaはinspect-only `run_legacy_local`を作るため、空一覧ではなく
  legacy 1 / consumer generation 0 / coordinator nullの正確なfixtureを検査する。
  この拒否はexecutorがpending receiptを破棄するので、保存receiptのreplayではなく
  同requestの再評価である。固定review指摘に従いコメント・受入説明を限定した。
  最終test SHA `8d8a5b6ee0fec75b1a3d0a1193ec348d7b6fef5334ca9cae27e07464a6cbad87`を
  同reviewerが限定承認。batch `orca-guarded-run-packaged-20261003-05`は1test pass、
  独立verifierで現在のsource/build/packageを照合し正式seal/finalize。
  targeted Oxlint・diff checkもpass。Help No impact: test-only変更であり、ゲームの
  入力・表示・成立条件・save/assets・Help producer/consumerの実装は不変。
  03は初期fixture前提で失敗、04はpass後のreview文言補正で再試験へ切替と正式記録し、
  今回生成した両report rootだけを撤去した（計32768 allocated bytes、試験で再生成可能）。
  source・live成果・履歴・package02・最新05の8192 bytesは保持する。
  旧02 acceptanceのsource fingerprintはtest変更後に流用せず、最新05を用いる。
  本受入はpositive Run移行・consume/enable transaction・実配備の証拠ではない。
  試験終了後のjoint preflightは元checkpoint
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`へ一致し、
  runtime・Run consumer1・terminal0件・旧registry/snapshotは不変。
  稼働版の再起動・Run変更・fence発行・通知再送は行っていない。
  同session新turnと具体的次工程は未確認で、TAK-14は停止したまま。

- 2026-10-03 positive packaged Run API受入を追加。productionを再buildせず既存02 packageを
  isolated profileで起動し、正規folder workspace / background terminal2本のfixtureを作成。
  fixture Runのgen1→gen2 CAS移行、completed receipt読取、別RuntimeClientの同operation
  replayed:true、新operationによるstale expected owner拒否、新pane current gen2と
  旧pane unbindを実RPCで確認した。作成terminalをcloseし、app・profile内daemonの終了後に
  isolated profileを撤去する。live TAK-14のRun/terminal/Linearには副作用を与えない。
  新helper `tests/e2e/helpers/packaged-guarded-run.ts` SHA
  `35cf2ec36240225450e34385f3e9f59a8f547dda44ecdac91ff704bf1ffc9ec2`、
  呼出元spec SHA `40eedc4617e11cecdd2bbb8c28e9be4a18377d0a079dd41a201b8b274d70edb0`。
  固定read-only reviewerが2pathsの限定差分を承認。Oxlintの3文字列連結を同義template
  literalへ補正し、最終subject batch `orca-guarded-run-packaged-20261003-07`が
  1test pass（4.3秒）、独立verifier・targeted Oxlint・diff checkがpass。
  acceptance fingerprint `26273f15dd7ee2d619ead3dcd00523276b797dcc01b4d91e3a48d3b84b5a7bdf`。
  package fingerprintは既存02の`b2ed80e6c4a09c89ee5ec03d8c3486d187e39ed3c096ee1776e16998058ca4cf`
  と一致し、packageを複製/上書きしていない。07を正式seal/finalizeし現在のconsumer付きで保持。
  05/06のreport用途を07へ移した後、両consumerを正式releaseし今回生成したreport rootだけを
  撤去（計16384 allocated bytes、再試験で生成可能）。package02・最新07・sourceは保持。
  Help No impact: 変更は隔離した開発ツールtestとfixtureだけで、ゲームの実入力・表示・
  前提条件・save/assets・Help consumerの経路は不変。並行製品変更の包括承認ではない。
  別RuntimeClientはapp/executorの再起動ではなく、本試験はcrash間の原子性や実配備・
  TAK-14の新turnを証明しない。残る主要実装はguarded consume/enable transactionを
  launcherのactual activation、immutable interruption、same Run正式receiptへ接続する部分。
  Run contextは新dictへ置換し、歴代attemptの原coordinator/shared_runを変更しない。
  joint preflightだけを終了・移行許可に昇格せず、未完成transactionのfenceをliveへ発行しない。

### 復旧接続の次の実装単位（2026-10-03補正）

これまでのsource単位の承認を上位transactionの完成に読み替えない。次は下地の追加だけで
終了せず、次の一貫した起動経路を実装・検証する。live失効はこの経路が接続するまで発行しない。

1. terminal作成前のintentをdurableに保存する。作成operationはRun移行operationと分離し、
   同request・worktree・command・選択runtime・実行host・callerに固定する。
   Orca本体の既存durable mutation executor / requestShowを拡張して再利用し、別台帳を増設しない。
   `terminal.create`の既存clientMutationIdはin-flight Mapとdeterministic handleによるdedupeで、
   完了receiptを永続保存しない。現在のreconcileExistingは不在時にcreateへ進むため、
   unknownな作成結果のread-only確定手段として使わない。
   新しいguarded作成契約はcapabilityで選別し、非対応hostへ通常createでfallbackしない。
   durable executorのキーはenvelope.orchestrationRequestIdであり、clientMutationIdだけでは
   有効にならない。guarded入口はrequest ID欠落を拒否し、両ID・認証caller・canonical
   worktree・全起動paramsを同一のpayload fingerprintに固定して不一致replayを拒否する。
2. terminal作成のeffect境界を明示し、effect可能後の例外でもpending receiptを削除しない。
   現executorは一般non-prompt mutationのcatchでpendingを破棄するため、単にdurable対象へ
   terminal.createを加えるだけでは不十分。completed receiptは実terminal/incarnation/worktree/
   execution hostに固定し、同operationのcompleted再照会はproviderやcommandを再実行しない。
   pending/absentは未作成証明にせず、未知の副作用を再実行しない。
   pendingでも正規の実行host側spawn claim/resultをread-only照合して同operationを確定できる
   回復経路を含める。handleのみのinventory一致をcommand/所有/incarnation証明へ昇格しない。
   結果保存の中断で恒久的なunknown待ちを作らず、真正な結果がない間だけeffectを保留する。
   spawn結果は即終了・daemon再起動後も同operationで読める保持契約とする。
3. 新しい管理terminal内の同launcherがrecovery入口を実行する。作成結果と実terminal show、
   launcher process birth/親子関係、runtimeを照合する。env値・title・辞書だけを所有証明にしない。
   通常Driver/providerはまだ起動しない。既存会話・原本registry・Run・reviewerを保全する。
   spawn中にcommandが動き得るため、bootstrapはcompleted作成receipt/正式host結果を
   read-onlyで確定するbarrierを最初に待つ。それより前にfence公開・Run変更へ進まない。
   terminal作成前のjoint checkpointはempty inventory専用のまま維持し、作成後はexact
   replacement 1件だけを許容し他actorを拒否する完全inventoryのstage別契約へ進む。
   cached empty checkpointを作成後の静止証明に流用しない。
4. owned recovery lease setを保持し、fresh control/source/historyと正式Run移行結果を照合して
   paired WALを採用する。paneのRunは正式run-currentで照合する。
   stable_pane_requiredやinput_acceptedを成功扱いにしない。
5. exact operation/activation/terminal incarnation/Run世代限定のenableを追加する。
   元fenceを削除せず、旧launcher・旧terminal・他activationは引き続き拒否する。
   新starting registryを通常prepareで再生成せず、同activationのprepared stateとして消費し、
   正規launchのprovider起動・実子process binding・actual wait/終了保存に接続する。
   startupへのlease引継ぎで自己競合を起こさず、排他の空白も作らない。
6. 同Codex sessionの新turnと具体的次工程を実見するまで復旧完了としない。
   作成前/作成後receipt保存前/Run移行応答消失/paired採用中/enable直前/provider起動直後の
   各障害境界で、同operation保持・unknown再実行拒否・原本保全・旧actor拒否を検証する。

2026-10-02 20:45 UTCのread-only照合は同checkpoint `0f085dbc…`、runtime `d9aba80c…`、
TAK-14 terminal inventory0、同Run owner `term_b9fb…`のまま。snapshotは旧runtime
`542cfcbb…`・publishedAt=1790881838335であり、稼働表示の根拠にはならない。
本項は一次sourceに基づく接続設計で、実装・配備・再開証拠ではない。

## 10. 更新履歴

- 2026-10-03 guarded作成結果のidentity確認を追加した。既存createはstable paneを採用した場合に
  新commandを実行せず既存handleへ変更するため、returned receiptの保存だけで新起動を証明できない。
  `terminal.createGuarded`はPTY・UUID incarnation・canonical executionHostId・positive processId・
  tabId/paneKeyと専用namespace/caller/operationから導出したhandle一致を要求し、`isReattach:true`、
  identity欠損、well-formedでも別handleの結果はpendingのまま`operation_unknown`として拒否する。
  旧terminalを閉じたり、unknown副作用を再試行したりしない。これらの返却fieldsは現在のlivenessや
  launcher所有証明ではなく、後続bootstrapが実行hostと改めて照合する。
  actual producer `Session.incarnationId=randomUUID()`と`createTerminal`のhost/pid返却経路を確認した。
  固定reviewのP2指摘でpane構文/所属tab検査不足を補正した。既存parsePaneKeyと
  isValidHostTerminalTabIdで実形式を要求し、parsed tabIdの一致を検査する。
  追加9testsを含む関連8files・114tests、host lease下tc:node、focused lint/format、catalog/diff checkがpass。
  source SHA `73a75e0ca2ce141bedd75ddb4488b884ee6b0d382dd92338fe5820bdeef70a48`、
  test SHA `3927f5b6981cfc0451b1e1bb3fd6cc5ebdd7626ad01dddc2f3e577e78fad45fa`。
  Help No impact: 別repositoryの開発用RPCとtestのみでゲーム入力・表示・成立条件・save/assets・
  root Help catalog consumerは不変。並行製品差分は承認しない。primary storageは184batches/legacy0、
  allocated bytes 670464819200でpass。今回の新規package/job、原本削除、commit/pushはない。
  joint checkpointは`0f085dbc…`、runtime `d9aba80c…`、Run consumer1、terminal0件のまま。
  candidateの補正であり、host spawn journal/pendingの正式確定・consume/enable・実再開は残件。
  現在のlive registry/Run/snapshotは変更せず、TAK-14は停止中。固定read-only reviewerがP2是正後の
  今回exact3pathsを限定承認（追加blockingなし）。returned receipt整合性だけの承認で、
  生存/所有権/host journal/admission/配備/live復旧の承認ではない。
  次のhost journal接続のreuse調査では、`src/relay/pty-handler.ts::spawn`の既存
  agentSessionCreateOperationsがメモリMapとretention timerであり、全params/caller bindingや
  daemon再起動後のdurable receiptを持たないことを確認した。単にagentSessionCreateOperationIdを
  転用して正式host結果として採用しない。`terminal-host-session-create.ts`もspawn後Session登録と
  startup書込を行うため、RPC結果前にcommandが走る。bootstrap barrierは引き続き必須。

- 2026-10-03 `terminal.createGuarded`のreturned-receipt候補をOrca本体へ追加した。
  既存create handlerを再利用し、明示worktreeとclient/envelopeの一致UUIDを必須にする。
  durable executorでauthenticated caller・method・parsed paramsを結び、create経路へ入る前に
  effect possibleを記録する。成功結果は既存mutation DBへ保存し、completedの同operation再照会は
  spawnせずreplayする。legacyの`reconcileExisting:true`は新規spawnへ縮退するため拒否する。
  capability `terminal.guarded-create-receipt.v1`は返った結果の永続receiptだけを表し、host spawn
  journal・launcher admission・復旧完了を表さない。old hostでordinary createへfallbackしない。
  shared schema/catalogへ登録し、既存RPCの順序は維持して末尾に追加した。
  実SQLiteのclose/reopenでcompleted replay、host応答不明、spawn後completion保存失敗の
  pending維持とno-respawn、payload差替え拒否、他callerからのreceipt非公開を試験した。
  固定reviewのP2指摘でlegacy in-flight createとの混同を検出したため、guarded専用namespaceと
  authenticated callerFingerprintを実dedupeのmap/handle生成へ結合した。通常createのstablehandleは
  維持し、legacy同ID/別commandと別callerのguarded同IDを実dedupeで並行させて別spawn/handleを確認した。
  関連8files・105tests、host lease付きtc:node、catalog生成照合、focused lint/format、diff checkがpass。
  初回typecheckのunary handler引数数とshared catalog未登録を補正し、manifestの2つの件数期待を
  38へ更新した。実サービスのcreateは呼んでいない。
  source SHA `e485e6b59355370e69a6c9a69d366db637d5ae330396e9818bad2f116fb238a3`、
  test SHA `b9e802bb0b7ff29ac59999049e4e9bee4672c6592ac83b82354fc3fc172a764d`。
  Help No impact: 別repositoryのOrca内部RPC/testだけで、ゲームの入力・表示・成立条件・save/assets・
  root Help catalog consumerへ接続しない。primary Help gateのpassは外部TS検証の代用ではなく、
  並行製品差分の包括承認でもない。primary storage checkは184batches/legacy0、
  allocated bytes 670464704512でpass。用途付きcandidate/cacheを保持し新規package/jobや削除はない。
  実checkpoint `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`、
  runtime `d9aba80c-fa91-4f47-81ec-aef1666f3429`、Run consumer1と完全なterminal0件は不変。
  host側spawn claim/result復旧、bootstrap barrier、consume/enable、同session新turnと次工程確認は
  未完了。live fence・Run移行・app切替・旧通知再送・commit/pushはしておらずTAK-14は停止中。
  固定read-only reviewerがP2是正後のexact13pathsを限定承認（追加blockingなし）。承認範囲は
  returned receipt永続化だけで、host spawn journal/pending確定回復/起動admission/配備/live復旧は
  含まない。今回の候補は未配備であり、TAK-14再開とは扱わない。
  docs indexはpass。full docs gateは既存node_modules下の無関係な4つのlink欠損で失敗しており、
  full docs passとは扱わない。

- 2026-10-03（2026-10-02 20:55 UTC）Orca本体のdurable mutation executorを候補修正した。
  一般non-prompt操作で`markEffectPossible`後に例外が発生してもpending receiptを破棄せず、
  effect前の失敗だけをdiscardする。terminal.cancelDraft固有の例外扱いに依存させない。
  同operationのunknownを新executorでも再invokeせず、payload不一致を拒否する。
  RunUseGuarded/TaskCreate/sendについてeffect後unknown保持とeffect前retry許容の6testsを
  追加し、draft cancel/request-show/Runを含む4files・70testsがhost lease下でpass。
  workerStartはhandler-owned atomic acceptanceの別契約のため、pending claimを作らない
  fake handlerでは評価しない。初回そのfixture2件が失敗し、通常executor-owned claimの
  TaskCreateへ対象を修正した。既存workerStart試験は維持した。
  source SHA `17c45a3542fdfd212fad238412b67e90c619bc9f0eabb7a6b4f0ecf89ef46a27`、
  test SHA `2268be061bdb0bc72e167cbd0695a7f5a86585bdef04748ef761edb6e14b9803`。
  tc:node / focused oxlint / diff checkはpass、固定read-only reviewは上記exact 2pathsの
  追加差分を限定承認（blockingなし）。実RPC handlerのeffect境界・app/DB再起動・terminal
  作成/復旧/配備の証明は含まない。既存の明示的な再開例外は変更していない。
  Help No impact: 外部Orcaの内部receipt bookkeeping/testsのみで、hell-workers player入力・
  表示契約・成立条件・runtime assets・Help consumerは不変。primary Help gateは同base
  80df44c4に対してpass（外部Orca TypeScriptのレビュー代替にはしない）。
  primary storageは184 batches / legacy0 / allocated 670464581632 bytesでpass。
  新規job/package・原本削除・live効果なし。旧package02は凍結のまま、この新sourceの
  配布成果と扱わない。guarded terminal create/正式host spawn result/enable/正規launch接続
  は未実装であり、同checkpoint・旧snapshot・TAK-14停止は変わっていない。

- 2026-10-03（2026-10-02 20:45 UTC）次の復旧接続をOrca本体sourceで照合した。
  terminal.createのdedupeだけではdurable結果照会にならず、一般mutationの例外時pending
  削除もguarded作成には流用できない。上記「復旧接続の次の実装単位」を追記し、固定
  read-only reviewの3指摘（envelope ID/params binding、作成後の完全inventory、bootstrap
  barrierとhost結果保持）を反映した。reviewerが該当節を限定承認、blockingなし。
  review時点の文書SHAは `f006ef2fa34f30880a4d5a96f861f0ff56d7d0f377a94bd6e255a207ce595de6`。
  実装・配備・live操作・復旧の承認ではない。index/diff checkはpass。
  今回はsource変更・新規実験出力・live効果を行っておらず、TAK-14停止は不変。

- 2026-10-03（2026-10-02 20:34 UTC）監視で、local adoptionの各置換前・最終確認にfresh read-only
  inspectionを接続した。元subjectからWALで許可したregistry/loopのbefore/after SHAだけを
  置換し、source fingerprint、active control revision、lifecycle/runtime台帳、保存sessionの
  inode/bytes/hash/mtime/ctimeは全一致を要求する。再検査失敗をcached admissionで代替しない。
  同じ実lease holder・formal Run結果・launcher照合は維持し、registry採用後にcontrolが停止
  した場合はloopを変更せず保全する。固定reviewでinspectionのsubprocess待機中にFDが
  失われるP2を検出し、inspection復帰後の初回・各write/sync前・final success前にも
  実leaseを再検査する修正を追加した。待機中FD喪失のpre-registry/post-registry/final
  回帰3件を含む新規6tests、関連195 unittestがhost lease下でpass。
  source SHA `9aac3ca9f44817145f036a67b2e879a9e9c0db899626fc19e3f074f077c82c7b`、
  test SHA `f2ce69c95c634caf9d5539a37bda1dd1b843b093b7e8f5ef4bc4157b5c92b42b`。
  修正版の固定read-only再reviewは上記exact 2pathsを限定承認（P2解消・blockingなし）。
  実適用・enable・terminal ownership・起動/配備・復旧全体は承認対象外。
  Help No impact: 内部Orca recovery helper/testsだけで、
  ゲームの入力・表示・成立条件・runtime assetsとHelp consumerは不変。候補Help gateと
  diff check、primary storage（184 batches / legacy0 / allocated 670464364544 bytes）がpass。
  docs indexは29/77 plans・8/14 proposalsで一致。full docs checkは既存node_modulesの
  READMEリンク欠損4件で失敗しており、この候補をfull docs passとは扱わない。
  新規job/package・削除・live台帳書換えはない。実read-only preflightのcheckpoint
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399` は不変、
  runtime d9aba80c・terminal inventory0・同Run owner term_b9fbを確認。
  TAK-14の停止は解消していない。terminal/pane所有、activation限定enable、正規同session
  launcherのlease引継ぎ、実配備、新turnと具体的次工程の確認を残す。

- 2026-10-03 上位recovery transactionが全leaseを保持したままlocal adoptionへ進める接続を
  候補実装した。`orca_recovery_leases.acquire_owned`が同process内のdescriptor setを保持し、
  `OwnedRecoveryLeases.require_current`はactive lifetime、現在PID、canonical request/parent、
  現loopの全slot集合、直接保有HostLeaseのcanonical path、実flock open-description所有を
  照合する。dictやinodeだけ、borrowed/closed FD、別process、missing slot、scope変更では
  admissionに昇格させない。source/authority/exit/terminal ownershipの証拠とは区別する。
  `orca_run_transfer_adoption.adopt_locked`が各replace直前とfinalでこの同holderを確認し、
  上位の排他を取り直さない。単独adoptは同じowned contextへ委譲し、旧read-only acquireは
  同subjectをyieldする互換APIを維持した。前項の独立adoptionの再取得衝突という接続残件を
  解消する候補であり、enable・正規launcherのlease引継ぎ・実配備はまだ接続していない。
  adoption fixtureを模擬leaseから隔離tempの実flockへ変更し、upper-held setで再取得ゼロ、
  終了済holder拒否、registry採用後のFD欠損でloop維持、実open-description不一致を検証した。
  新規6testsを含む関連189 unittestがhost lease下でpass。source SHA:
  leases `7bc1062483c9138ac69d1ee33c7ca88cdae691dd6072a2a48a4ab92c1c4df518`、
  adoption `a6e5f9bf89f9bd77860ef0aec114f4526b967c444f4ba603f623cf0cee5015f0`。
  test SHA: leases `d1f6d040e8e3bf7a241aa54b6ea5d6a059d6f4e6a61fc751cc9eb4769bf92193`、
  adoption `12fde01f975c7d094ef7f5628cc07affd048b2ea23ef3ffc44a05dace9a58c94`。
  固定read-only reviewerが上記exact 4pathsの追加差分を限定承認（blockingなし）。
  上位admission・enable・配備・live復旧全体の承認は含めない。
  Help No impact: 内部Orca lease/adoptionとtestsのみで
  player入力・表示・成立条件・save/assets・root Help consumerの経路は不変。
  候補base ae5b2066のHelp gate/diff check、primary storage（184 batches / legacy0 /
  allocated 670464258048 bytes / 用途付きcache保持）がpass。新規job/package・削除なし。
  実checkpoint `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`は
  不変、runtime d9aba80c、Run consumer1、terminal0件、snapshotは旧runtime542cfcbb。
  live台帳、Run、fence、appを変更せず、再送/新Run/commit/pushもしていない。
  TAK-14は依然停止しており、同session新turn/具体的次工程は未確認。

- 2026-10-03 正式Run projection WALからのguard付きlocal adoption / 部分失敗roll-forwardを
  `orca-hardening-next/scripts/orca_run_transfer_adoption.py`へ候補実装した。
  canonical originと全recovery leaseを取り、選定runtimeの同operation request-show・現在Run・
  actual launcherをread-only再観測してimmutable WALと完全一致させる。registryとloopは
  exact before/afterだけを認め、元registry inode/raw bytesを照合し、registry→loopの順に
  exact frozen payloadを採用する。通常loop.saveの時計更新を通さず、歴代attempt・review・
  generation・元activation/exit未確定情報は保持する。片方だけreplaceした後の失敗、
  replace後directory fsync失敗も同WALから未適用分だけを進め、既適用inodeを再置換しない。
  unrelated drift、結果不明、異なるWAL、busyでは上書き/Run再操作しない。
  固定reviewのP2（WALのpost-link fsync失敗でも見えるfileをdurableと扱う）を補正し、
  WAL directoryと全祖先のfsync成功・再読取一致を採用前の必須条件とした。補正に伴う
  既存post-replace fsync試験の失敗注入位置だけを実after状態に限定し、guardを緩めていない。
  専用11件を含む関連183 unittestがhost lease下でpass。テストのleaseは隔離fixtureで
  模擬し、実排他の既存lease testsも同scopeで実行した。source SHA
  `1d23b599bf63cdeca1a630fdb204dfa06f23a0166c333259be2828cd58e3ca6e`、test SHA
  `a904d3e8fdf5c7c56201270cda4f4d6b012399fa6bbe85b303a96d9ca5e41260`。
  固定read-only reviewerが補正後の上記exact 2pathsを限定承認（追加blockingなし）。
  上位admission・配備・enable/起動・TAK-14復旧全体の承認ではない。
  Help No impact: 内部開発用registry/loop transactionと
  testだけでゲーム入力・表示・成立条件・runtime data・Help consumerへ接続しない。
  候補base ae5b2066のHelp gateとdiff check、primary storage（184 batches / legacy0 /
  allocated 670464151552 bytes / 用途付きcache保持）がpass。新規job/package・削除なし。
  実joint checkpoint `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`は
  不変。live fence/WAL/registry/Run、Orca本体を変更していない。CLI公開・activation単位enable・
  source/control/terminal/paneの上位admission・正規launcher接続・実配備が残る。
  このadoptionは独立stageとして全leaseを取り直すため、上位の保持中leaseとの接続方式も
  明示する必要がある。成功値は`local_projection_adopted_fenced`であり起動許可・依頼完了ではない。
  部分失敗retryはsame proven live launcher内だけであり、launcher自体の死亡後の
  新processへの所有移行をこの候補で解決したとは扱わない。
  TAK-14は停止継続、新turn/具体的次工程は未確認である。

- 2026-10-03 Run/registryの両local destinationをprojection WALへ接続。
  `record`は正式observe後にchecked registry projectionを計算し、Run projection本体と
  `registry`（before hash/new starting row/after hash/元Run projection hash）を同じ
  write-once journalへ保存する。adoption後にmutable registryから再構築する経路を
  作らない。現在のloop/registry本体や旧exit、ACK、terminal、Runは変更しない。
  shared fence fixtureを完全schema1 registryに整備し、不完全registry拒否fixtureでは
  必須fieldの除去と全frozen subject再生成を行った。record testがregistry/Run digest
  の結びつきと同inode replayを照合する。関連172testsがhost lease付きでpass。
  source SHA `25ca12a28bf9333bb5c30919cfa4081da191c80d999df52841d9e8c8e914c32b`、
  revocation test SHA `f23db4a5d5cae79b8e915e05487887198c78496e94eed4fbbb839d5bb38146f2`、
  observation test SHA `58f05396adf231b41112ab95bfe57addef0834c3c16deb0e2ca36bad6cda6063`。
  固定read-only reviewerが上記exact3pathsのlocal adoption前WAL保存までを限定承認
  （blockingなし）。adoption/enable/実起動/配備は含めない。
  Help No impact: 内部復旧WAL/registry projectionとfixture
  のみでゲーム入力/runtime data/player labels/Help consumerへの接続なし。Help gate pass。
  diff check・docs index pass。primary storage pass（184 batches、未分類0、670464024576
  allocated bytes）、新出力job/packageなし。live checkpoint
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`は不変。
  WALはadoption/enable/launchの許可ではなく、実運用への接続は残件。TAK-14再開は未確認。

- 2026-10-03 registry adoption前のexact starting projectionを候補実装。
  `ui.checked_registered_state`へ既存read validatorを抽出し、request canonical検査を維持。
  `registry_projection`はsaved intent/正式Run projection照合後、fence内のoriginal bytesを
  共通validatorで確認し、別copyでschema2/新terminal/新activation/starting/ACKnullだけを
  準備する。identity/created_at/Linear/worktreeを保持し、旧起動のexitを捏造しない。
  返却projectionは未公開であり、現WAL record/adoption/enableへは未接続。
  完全registryの履歴保持・input不変・alias不変と不完全registry拒否を2tests追加。
  finalization fixtureの既存環境依存を補正（post-launch unit fixtureにcontrol/terminal/
  worktree admission mockを追加、product guardは不変）。関連172testsがhost lease付きでpass。
  source `orca_run_transfer_observation.py` SHA
  `9c1bc398f47448d711f4e17fc885f9d005327ba4ec0e3893c00dc7bbe12155fc`、
  `orca_ui_coordinator.py` SHA `313a4c5c9c266fea8dd509ee8d55b8591ce34719a3f90bc065503589e0936ee6`、
  observation test SHA `b41e86ad8552ae39aa2b2b5982e2fe6458501e65ab894790f5b382f917b65e96`、
  finalization test SHA `f5e5e4b0fcbbce0c39c394498f834b5d54b98ff86dab0f0a14ec0e5213967a33`。
  固定read-only reviewerが上記exact4pathsの限定差分を承認（blockingなし）。
  未公開計算/validator抽出だけでWAL接続・adoption・起動・配備は含めない。
  Help No impact: 内部統括registry validator/未公開projection
  とfixtureだけでゲームの入力/表示/成立条件/runtime data/Help consumerへの接続なし。
  Help gate・diff check・docs index pass。primary storage pass（184 batches、未分類0、
  670463918080 allocated bytes）、新出力job/packageなし。live checkpoint
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`は不変。
  live台帳/Run/terminalの変更なし、TAK-14の実再開は未確認。

- 2026-10-03 正式Run結果のobserve後、local adoption前のprojection WAL保存を候補実装。
  `orca_run_transfer_observation.record`は選定CLIの同operation・現行Runの照合を再利用し、
  projection全体を`run-projections/<request>.json`へwrite-onceで保存する。
  元intent/registry/loopを上書きせず、保存物のdigestとcanonical serialized内容を
  observed projectionへ厳密照合する。既存journalのbool/float型緩和も拒否する。
  replayは同operationをread-only再照会し同inodeを保持、pending/absentではprojection
  directory作成前に終了する。WAL保存はloop/registry adoption・enable・起動許可ではない。
  同inode/0600/nlink1/original intent保持、unknown時の未作成、別内容/型拒否を3tests追加、
  関連78testsがhost lease付きでpass。source SHA
  `89ab8acf2ff5b9e72fa6689e6d57870bb36a11c0bcb84f2f86a855f573c53b70`、test SHA
  `b123de90eab47b5b72f8fb8823b6805f8b91752490e6a16cdcad63cc0f6c5e54`。
  固定read-only reviewerが上記exact2pathsのwrite-once WAL保存までを限定承認
  （blockingなし）。adoption/enable/launch/実適用は含めない。
  Help No impact: 内部復旧journalだけでgame input/runtime
  data/player label/Help consumerの経路を変更せず、未公開の実行経路にも未接続。
  Help gate・diff check・docs index pass。primary storage pass（184 batches、未分類0、
  670463815680 allocated bytes）。新出力job/packageなし、原本・dirty source/cacheを保持。
  live checkpoint `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`
  は不変。live fence/Run/registry/loop/terminalへ未適用、TAK-14再開は未確認。

- 2026-10-03 保存済みRun移行operationの正式結果を選定CLIから読む
  `orca_run_transfer_observation.py`を候補実装。保存済みexact intent/fenceを照合後、
  runtime ID・ready/reachable・実app PID/birthを確認し、同operationのrequest-showと
  現行run-showを照合、最後にruntime/processとimmutable intent/fence/current launcherを
  再検査する。正常結果も未公開projectionまでであり、loop/registry保存・enable・launch
  やrun-use/retry/send/fallback CLIは行わない。pending/absentは結果不明として拒否し、
  新operationや副作用の再実行をしない。producerはappのcaller fingerprint別request-show
  正式receiptと一致させた。
  completed/別runtime/別operation/旧method/別generation/未ready/親process変更/
  保存intent消失を検証。固定レビューがrun-showのPython bool/float等値による型緩和を
  検出し、legacy/gen strict intとcanonical JSON digest比較に補正。4型拒否caseを含む
  9tests追加、関連75 unittestがhost lease付きでpass。
  source SHA `0a383cd0adbc0d65e9572a836c96581a46cea1ca23d406a02bdcac5c2c59bed4`、
  test SHA `15c902783775dca7abfe418cf1ceb68d984a709196f0ebfa8595c88489b1d353`。
  固定read-only reviewerが上記exact2pathsのread-only observationを限定承認
  （追加blockingなし）。publish/enable・配備・実復旧は含めない。
  Help No impact: 内部CLI read-only結果の照合のみで
  game input/runtime data/player labels/Help consumerへの接続なし。Help gate・diff check pass。
  primary storage pass（184 batches、未分類0、670463709184 allocated bytes）、docs index pass。
  新しい出力job/packageは作らず、原本・dirty source・既存cache/receiptを保持した。
  live joint checkpointは`0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`
  と不変。terminal inventoryは空、Run ownerは旧term_b9fb、gen1。TAK-14は停止中。
  新sourceは実行経路へ未適用であり、次は上位transactionの保存/部分適用回復/enable/
  正規同session起動を接続する。新turnや具体的次工程を確認するまで復旧としない。

- 2026-10-03 Run移行intentのwrite-once保存と保存済みexact intent照合を候補実装。
  `run-transfers/<request>.json`を既存revocationsと別familyに置き、schema/operation/
  session/subject hash/未開始activation/expected owner・generationを厳密検査する。
  歴史読取は消失したlauncherの生存を要求しないが、record/projectionはactual current
  launcherと実保存fence、freshに構築したintentの完全一致を要求する。
  新Run・loop保存・元registryのexit書換え・RPC実行・実行許可をこのprimitiveから行わない。
  固定レビューで初回mkdirの親entry未fsyncを検出し補正した。初回/再照会とも
  target directoryからrootまでancestorをfsyncしてから成功へ進め、親fsync失敗は
  成功扱いにしない。no-replace hardlink/fsync・exact fence path guardを維持する。
  missing fence/intent、改変digest/params、symlink、launcher消失、別target拒否、
  ancestor fsync順序・初回/再試行/保存後再照会のfsync失敗を含む関連66tests pass。
  source SHA `c095980ea83c6bd124f06e4215b8d016547ed17f4bceb2a7cf143d68b54a12c9`、
  test SHA `8ee82de144fdbeb23c25cc5bbed2fa1bc4e1615b116ad9b5f60027bcfee1a3bf`。
  補正後の固定read-only reviewerが上記exact2pathsの保存・照合primitiveを限定承認
  （追加blockingなし）。上位transaction/実行許可/配備/復旧の承認ではない。
  Help No impact: 内部復旧journalとPython test
  のみでgame input/runtime data/player label/Help consumerへの接続なし。関連Help gateと
  diff check pass。primary storage pass（184 batches、未分類0、670463602688 allocated bytes）、
  docs index pass。出力job/専用packageの追加なし、既存source/cache/receiptを保持した。
  live joint checkpointは`0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`
  と不変。TAK-14は停止したまま。実fence/RPC/Run/registry/terminalへ今回の変更は未適用。
  残件は同operationの正式receipt取得、lease下での部分適用roll-forward、activation-bound
  enable、正規同session launcherへの接続と新turn内の具体的次工程確認である。

- 2026-10-03 Run移行前のexact RPC intent producerをprojectionから分離した。
  `prepare_run_transfer`は既存のfence/subject/drained loop/actual current launcher確認を
  再利用し、同Run ID・replacement from・expected元owner/consumer generation、同fence
  operationを含む未公開intentを構築する。`project_run_consumer`もこのproducerを使い、
  intent digestを結果へ結びつける。引数の新設/旧owner補完/新operationの発行は行わない。
  intentはまだ永続保存や実行許可ではない。上位transactionは副作用前にexact intentを
  保存し、結果不明なら同operationの正式receiptを読む必要がある。未送信/不明結果を
  起動成功扱いにせず、publisher/enable/正規launcherへの接続が残件である。
  独立targetコピー・入力不変・exact RPC params・再生成同一digestを2tests追加し、
  関連58 unittestがhost lease付きでpass。source SHA
  `1e1c2a0b5b30cfef42fe0a2fa55fca2b033d4989a4f7269f8274b66dfbb67fd1`、test SHA
  `c3355957e9954ae2744439944822024d36057d42f6f3b2f22eb1eb42ee72798d`。
  固定read-only reviewerが上記exact2pathsの今回差分を限定承認（blockingなし）。
  永続intentとの照合・真正性・fresh lease・実行許可の承認ではない。
  Help No impact: 未公開の内部RPC intent/projectionと
  Python testだけでゲーム入力・表示・成立条件・save/assets・root Help consumerへの
  接続はなく、並行製品変更の包括承認ではない。live app/registry/Run/fenceは未変更。
  joint checkpointは`0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`と
  不変。TAK-14は依然停止しており、同session新turn/具体的次工程を確認していない。

- 2026-10-03 Run receipt消費後のloop projection部分を候補実装した。
  `orca_coordinator_revocation.project_run_consumer`はimmutable fenceのoriginal bytes・inspection、
  joint checkpoint、同一sourceのdrained loop、actual current launcher、同request operationの
  completed `orchestration.runUseGuarded` receiptを照合する。Runの同一ID/非legacy/owner・
  consumer generationの1段移行と他attributes不変を要求し、現在のterminal/Run contextだけを
  変更した未公開projectionを返す。歴代attemptの原coordinator/shared_run、全lane・review・
  generation・integrationを保持し、shared dict aliasによる過去consumer改変を防ぐ。
  original registryはunknown readyのまま保全し、fake exit/ACK/approvalを追加しない。
  writer、CLI、fence公開、解除/enableは接続していない。辞書の整合だけを真正性・鮮度・
  操作許可とせず、上位transactionでselected runtimeの正式読取とlease下の再照合、実terminal/
  paneの所有確認、永続intent/部分失敗回復が引き続き必要。TAK-14への適用/再開ではない。
  専用4testsを追加し、関連56testsがhost lease付きでpass。初回testのmock名を既存loop.saveへ
  補正して再試験した（実台帳には書込なし）。source SHA
  `407dc91df01d8b0c9dea14b6526061d9adaa0a0df5737939f25594a2e2d15bee`、test SHA
  `223e3e9799113fc32693ad9238efac30d71505da085741380b02d9423d101aa2`。
  固定reviewで正式requestShow receiptがrun+mutationであるP2指摘を受け、producerの
  attachMutationReceipt/recordReceiptとrequestShow実装を確認して補正した。mutationの
  requestId同operationとreplayed厳密boolを要求し、欠落/不一致/追加属性を拒否する。
  replayed:trueの正式記録も新たな副作用なしで投影できることを検証した。
  Help No impact: 未公開の内部projectionとtestのみで、ゲーム入力/表示/成立条件/save/assets/
  root Help catalog consumerの経路は不変。並行製品差分の包括承認ではない。
  固定read-only reviewerが補正後の上記exact2pathsを限定承認（追加blockingなし）。
  未公開projectionのみの承認であり、publish/enable・真正性/権限・実Run移行やTAK-14復旧の
  承認ではない。実用のconsume/enableとlauncher接続は残件。
  実checkpoint `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`、
  runtime `d9aba80c-fa91-4f47-81ec-aef1666f3429`、Run consumer1、terminal0件は不変。
  primary storage（184 batches / legacy0 / 用途付きcache保持）・docs index・diff checkがpass。
  新規job/package、削除、commit/push、live台帳の書換え、fence発行、通知再送は行っていない。
  同session新turn・具体的次工程は依然未確認で、TAK-14は停止している。

- 2026-10-03 joint checkpointのserialized契約を実producerのreturnへ接続した。
  `scripts/orca_recovery_preflight.py::checked_result`はexact envelope/subject、lowercase SHA、
  canonical runtime、actual process birth形式、original registry inode、非legacy Runの
  owner/generation、完全なlocal empty inventoryを検査する。digest再計算だけではschemaの
  boolean、欠損owner、generation 0、部分inventory、終了/実行許可へのscope変更を通さない。
  この検査はread-only checkpointの整合性だけで、callerの権限・終了・失効・起動を証明しない。
  専用3testsと既存mockの有効subject補正を追加し、関連52 unittestがhost lease付きでpass。
  source SHA `81475f27940d399360951b0028fc38c5ec29e42e828d637fff17a71a71908087`、
  test SHA `6d80ef2a5fe6c0c6fc3695bc79053ecdecab156a58e2e2d02d31464293a5688a`。
  Help No impact: read-only開発ツール検証だけで、ゲームの入力・表示・成立条件・save/assets・
  root Help catalog/consumerへ接続せず、並行製品差分の包括承認は行わない。
  候補base ae5b2066のHelp gate・diff check、primary docs indexとstorage checkがpass。
  実checkpointは`0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`と
  不変。今回生成したjob/packageはなく、既存の用途付きcache・成果・履歴を保持した。
  固定read-only reviewerが上記2pathsの今回差分とexact SHAを限定承認（blockingなし）。
  内容整合は真正性・鮮度・実行許可を保証しないため、lease下の再観測を継続必須とする。
  consume/enable transactionと正規launcherの接続は未完了。
  live fence発行・Run移行・app切替・通知再送はしておらず、TAK-14はまだ停止している。

- 2026-10-03 21:26 UTC監視では、joint checkpoint
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`、runtime
  `d9aba80c-fa91-4f47-81ec-aef1666f3429`、Run consumer1・terminal0件は不変。
  復旧接続の一部として、Orca候補CLIの`terminal create --guarded-request <UUID>`を
  既存`terminal.createGuarded`へ接続。明示workspace、UUID、同一retry identityとreceiving
  runtime capabilityを副作用前に検査し、parameterとdurable envelopeのIDを一致させる。
  old host・transport不明時はordinary createへ降格/再送しない。通常createの経路は維持する。
  固定レビューのP2（空IDで通常createへ降格、空workspaceでcwd補完）を修正し、空値も
  runtime照会前に拒否するtestを追加した。関連CLI/RPC4files85tests、tc:cli、focused lint、
  diff checkがpass。max-linesを回避せず、既存create CommandSpecを専用moduleへ抽出した。
  今回subject: terminal.ts `003bd2e2dfbefcdb17d4bc9008ea58a6ddebf0f98dec3acdccfdc06821c85089`、
  test `fd57de99d75fdc07b973aab31b3623a3892210a374fc9538300d39433d9258b2`、
  core.ts `c90297737bd5f7971ab3542183ddff188bad2df687a8c7addca58198766a402a`、
  spec `878b856fabbebcb3a25f94ca6d0bd684019a46c0db2095d04907bd0454ef15e0`、
  wire doc `6e5554dcf23a4927e9ba969a513a49bbbb48e046a0f828b0b2c56009f1e594ae`。
  Help No impact: 外部Orca開発CLIのみで、ゲーム入力・表示・成立条件・save/assets・root
  Help catalog consumerの経路は不変。primary HEAD baseのHelp gateはno production changes。
  primary storage checkは184batches / legacy0 / 670465142784bytesでpass。
  これはreturned receiptのCLI接続であり、host spawn journal、bootstrap barrier、consume/enable
  と正規launcherの接続は未完了。候補のみで未配備、新規package/job・live fence・Run移行・
  通知再送・commit/pushはしていない。同sessionの新turnと次工程は未確認、TAK-14は停止中。

- 2026-10-03 21:39 UTC監視の候補修正: completed guarded createの正式記録、同runtimeの
  terminal show/list、managed環境、同UIDの実launcher ancestryとpidfd lifetimeを照合する
  `orca_recovery_terminal.py`を追加した。pending/absent、remote host、別pane/incarnation、
  部分inventoryは再送・代替terminal作成せず拒否する。観測のみで、host spawn journal・
  永続create intent・利用者権限・起動admissionの証明ではない。
  既存Run WAL adopterへlive observerを接続するadapterを候補追加した。
  固定read-only reviewのP2（fresh source検査の後のRPC待ちでpause/source変更が見落とせる）
  を補正し、remote照合→fresh source/control/history検査→非RPCのpidfd/launcher/lease検査
  →書込または成功、の順序に統一。adapter最終部にも追加RPC待ちを挟まない。
  実adopterでregistry前0 writes、registry後loop維持、最終成功拒否のdrift回帰を追加した。
  関連50 unittestがhost lease付きでpass。対象SHA256:
  observer `2f0d43048346ecd3e6176d6510d52f7ed5e0bf1df056772007e5e0869c09b0b1`、
  adopter `9a8360ba6cd3db42919e0d040c5bbbf6244a525a10aae48a3c709865253ef968`、
  observer test `285c875b1a9e59146246d6b5d05c00de65a09cd842f11a20f0fe625fbd59c37d`、
  adopter test `25da674429cc9c1d658ed500e4a207a78d36f083fe2d8f67a5a1a4051d4e0f8d`。
  補正後の上記exact4pathsは固定read-only reviewの限定承認を受領し、追加P1/P2 blockerなし。
  前節のCLI exact5pathsも固定reviewの限定承認を受領した。
  どちらもlive起動・配備・全transactionの承認とは区別する。
  Help No impact: 未公開の開発用観測・fenced WAL接続とtestだけで、ゲーム入力・表示・
  成立条件・save/assets・root Help consumerへの経路は不変。並行製品変更の包括承認ではない。
  primary Help gate（HEAD base）はno production changes、storage checkは184batches/
  legacy0/670465277952bytesでpass。新規job/package・削除・commit/pushは行っていない。
  実checkpointは`0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`、
  runtime `d9aba80c-fa91-4f47-81ec-aef1666f3429`・Run consumer1・terminal0件と不変。
  live fence、Run、registry、通知は変更せず、TAK-14は停止のまま。consume/enable・
  正規launcher接続、同session新turnと具体的次工程の確認が残っている。

- 2026-10-03 21:54 UTC: runtime・joint checkpoint・Run consumer1・完全なterminal0件は
  前回と不変。snapshotは旧runtime `542cfcbb-38a8-43a8-957c-53379bec2158`、
  publishedAt `1790881838335`であり、現runtimeの稼働証明には使わない。
  復旧leaseから通常launcherへの接続で自己競合する前提を補正するため、候補
  `OwnedRecoveryLeases.retain_launch_lease`を追加。同じui-coordinator FDを一度も
  release/reacquireせず保持し、controller側が再取得するcoordinator/driver/heavy等は閉じる。
  full holderは一方向に失効し、scope終了・例外時にはlaunch FDも閉じる。実行権限、
  enable、source照合をこのprimitiveへ読み替えない。上位transactionと通常launcherは未接続。
  実lockで同一FD・競合継続・他scope開放・scope drift拒否・例外解放を3tests追加し、
  関連38testsがhost lease付きでpass。source SHA
  `beef9129f605b0036f2a393eb9f5619035f9746e7247d2f9ffaa6dd11724063d`、test SHA
  `9ac998d20ec5e5361536a19646a1d764642064d80c971370028cd27bc124b7ef`。
  上記exact2pathsは固定read-only reviewの限定承認を受領、追加P1/P2 blockerなし。
  実行許可・通常launcher統合・live復旧は承認対象外。Help No impact: 未公開の開発用lock lifetimeのみで、
  ゲーム入力・表示・成立条件・save/assets・root Help consumerの経路は不変。
  primary Help gateはno production changes、storageは184batches/legacy0/
  670465384448bytesでpass。live app/fence/Run/registry/通知には未適用で、TAK-14は停止中。
  新規package/job・削除・commit/pushはなし。復旧完了として扱わない。

- 2026-10-02 22:04 UTC heartbeat（local 2026-10-03）: joint read-only checkpoint
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`、runtime
  `d9aba80c-fa91-4f47-81ec-aef1666f3429`、Run consumer1、完全inventory terminal0件は不変。
  旧send `dd0ba73f-53e4-4304-92cd-8beeaf04a1da` は同runtimeの正式照会でpendingのまま。
  原文再送・Enter・新Run・registry/fence変更は行わない。TAK-14は停止中で、新turnと次工程は未確認。
  候補 `orca_ui_coordinator.consume_prepared` を追加し、実所有ui-coordinator lease、現launcher、
  正式saved starting/schema2 subject、managed terminal/worktree、未ACK・未exit・未provider_root、
  受付識別子と通常revocation guardsを照合して読取る。ordinary prepareを呼ばず、保存状態・時刻・
  activationを再生成しない。coordinator lock内の受付読取後もsubjectとguardを再照合する。
  これはread-only primitiveであり、起動admission、enable、正式WALやRun/paneの証明ではない。
  read-onlyの範囲はregistry/intake ledgerの更新とprovider実行をしないこと。通常path helperと
  lock取得は欠落したディレクトリ/lock fileを準備し得るため、全filesystem無変更とは主張しない。
  normal launchへ未接続、未配備。関連66 unittest pass。最初の試験で検出した既存activation
  fixtureの非UUID requestを有効UUIDへ補正し、imported TestCaseの重複収集も解消した。
  subject SHA256: coordinator `a9c82da01cc301be50a5ee32ff8439d32164d797ee0caf44f3a9c72a0cbb5fd6`、
  prepared test `e0798632a4fa71788b7bea2d4e7d329c50d51b7103e53b1a85f2209fad0cacb9`、
  activation test `3a2c455d63a159b4053c8f4e5ea7a8ae5fe6a7daed043f26714c058516e374ea`。
  上記exact3pathsの限定差分は固定read-only reviewで承認、P1/P2 blockerなし。
  normal launch・recovery admission・enable・live復旧は承認対象外。
  Help No impact: 外部Orca開発helperの保存状態照合とPython
  fixtureだけで、ゲーム入力・表示・成立条件・save/assets・Help catalog consumerは不変。
  並行製品差分の包括承認ではない。primary HEAD base Help gateはno production changes、
  storage checkは184batches / legacy0 / 670465507328bytesでpass。新規package/job・削除・
  commit/pushはなし。全復旧transaction・enable・通常launcher接続と実再開の確認は残件。

- 2026-10-02 22:19 UTC heartbeat: 候補`launch`にprepared state/実owned launch leaseの
  keyword接続を追加した。対の引数と同保存session/requestを要求し、開始時のdeepcopyで
  caller辞書を凍結する。元ui-coordinator FDを再取得せず、runtime setup前とDriver/Serverの
  setup後・Popen直前に保存subject/launcher/revocationを再照合する。通常launchは従来の
  prepare経路を保持する。provider実子binding・待機・終了保存を別実装へ複製しない。
  関連82 tests pass。同FD排他保持・同activation/created_atの終了保存（providerはmock）、
  Driver待ち中のcaller辞書とregistry同時変更でも起動拒否、片側引数/closed leaseをruntime
  setup前に拒否する回帰を追加した。mock成功をlive起動・新turnの証拠に読み替えない。
  固定reviewのP2（空sessionが新規会話へ降格）を補正し、runtime setup前に既存canonical UUID
  validatorでstrict文字列を必須化。空文字/False/0/不正/uppercase UUIDではruntime/Broker/
  Driver/Popenを呼ばない回帰を追加。通常resumeにも同じ検査を適用し、fresh launchは維持する。
  SHA256: coordinator `099dc5761087046e1c946083d2b56dcd48e0e6e557e22e62012336a0d3e7625c`、
  prepared test `455fd488f8ac3c0687eb492b11ebbe9d5eeb766587b81769ba77162f1f023a67`。
  補正後のexact2pathsは固定read-only reviewで限定承認、追加P1/P2 blockerなし。
  recovery enable・live配備・TAK-14復旧の承認ではない。Help No impact: 開発用Orca launcherの保存状態とlock接続
  だけで、ゲーム入力・表示・成立条件・save/assets・Help consumer経路は不変。
  primary HEAD base Help gateはno production changes、storageは184batches/legacy0/
  670465626112bytesでpass。新規package/job・削除・commit/pushなし。
  これは上位recovery admission/enableとの接続ではなく、通常revocation guardを維持する。
  CLIへの公開とlive適用はしていない。host spawn journal・bootstrap transaction・限定enable
  が残件。joint checkpoint `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`
  は不変で、TAK-14は停止中。同session新turnと次工程は未確認。

- 2026-10-02 22:27 UTC heartbeat: joint checkpoint、runtime、Run consumer1、完全なterminal0件は不変。
  旧sendの正式照会はpending、snapshotは旧runtime/publishedAtのまま。live再開・新turn・次工程は未確認。
  host journal接続調査で、local daemonのoperation IDがdeterministic session IDへ落ちる一方、
  `DaemonPtySessionSpawn.spawn`が通常`withDaemonRetry`を通り、結果不明のcreateをdaemon再起動後に
  暗黙再実行し得る経路を確認した。候補でoperation ID指定時だけこのretryを外し、一回のdoSpawnで
  元の不明結果を返す。開始時にoperation-bound判定を固定し、待機中のcaller変更で通常retryへ降格しない。
  unbound通常terminalの既存復旧は維持する。host側の実create完了後にreplyだけを失わせ、
  create1回・respawn0回・元host session1件を別clientの正式一覧で確認する回帰と、daemon欠落時に
  respawnしない回帰を追加。host journalやpending回収・enableの代替ではなく、再実行拒否の限定補正。
  最終75testsとtc:node、oxlint、oxfmt check、diff checkはpass。最初のtcで試験のlistSessions引数不足を
  検出し、空paramsを補って再検証した。prettierは未導入のため既存oxfmtを使用、installはしていない。
  exact SHA256: spawn `dd5fefc061330c1e5567cfa847c5c4f42e2fc8fd2cdfda5708c5a925f1dff9e9`、
  test `4ea04f142164502fa9547cbd925cff921bcfd5a42a44891904e924c8aaeab88e`、
  wire doc `363cd5c6d3753a9547a3b34d19a6e9f7aecc5a7f820b0f8f1d29041dbb0dcb4b`。
  固定read-only reviewerへ上記3pathsの限定差分を依頼。先行dirty guarded CLI/docの再承認や
  live配備・正式復旧の承認にはしない。Help No impact: 外部開発用Orcaのspawn制御と試験だけで、
  ゲーム入力・成立条件・表示・save/assets・root Help consumerに変更なし。並行製品差分は対象外。
  primary Help HEAD base gateはno production changes、docs indexesは29/77・8/14でpass。
  storageは184batches/legacy0/670465732608bytesでpass。新規package/job・削除・commit/pushなし。
  未接続のhost durable journal・bootstrap transaction・限定enable・実再開確認を残件として保持する。

- 2026-10-02 22:36 UTC heartbeat: 前回adapter-onlyの3pathsは固定read-only reviewで限定承認。
  reviewerがguarded RPCからoperation IDを渡していない配線欠落を指摘したため、候補で
  caller fingerprint・canonical worktree selector・durable request IDのdomain-separated SHA256
  を`agentSessionCreateOperationId`へ接続した。request/caller/canonical selector欠落は拒否し、
  ordinary createのoptionsは変更しない。renderer-driven createは同identityを伝達しないため、
  guardedのfocus:true/rendererBacked:true/presentation:focusedをschemaで副作用前に拒否する。
  backgroundへの黙示変換やguard迂回はしない。guarded background経路の限定補正であり、
  durable host journalの実装ではない。
  RPC dispatcher→通常runtime.createTerminal→PTY controller bridge→adapter→実daemonの回帰で、
  host create後のreply loss、create1回・respawn0回、正式pending、SQLite reopen後の同request
  再実行拒否を確認した。bridgeは実IPC builderを通らず、全IPC/host crash recoveryの証明ではない。
  identityのscope分離、ordinary create不変、renderer拒否、canonical selector欠落拒否も追加。
  関連104tests/3files、tc:node、tc:cli、oxlint、oxfmt check、diff checkはpass。
  exact SHA256: identity `5e9dee35a4a43c72b035495a5eaed57b880c2d2a0dd06954e1000ce076bdeeea`、
  lifecycle `9a1ef6ff25efb684be4b76e59e3b160262f46c99d17f262a11f7ada9ca68fe79`、
  RPC test `a496b9ff4cedc9a04ab7a88cf9c499602bc064f48abf7e8a5112c6dd9bb33268`、
  schema `00cb781568843bb72da61f42cec1cc1ca785f227b63556dab0a38a47c2cc9e7f`、
  wire doc `80b0bd9169b77e00a913e46fbb365db07da27c2f5b22a013def90439ffb8b86e`。
  上記exact5pathsは同じ固定read-only reviewerが限定承認、P1/P2 blockerなし。
  全IPC builder・実PTY・durable host journal・live配備・起動enableは承認対象外。
  旧doc承認は更新docへ流用せず、更新docを今回のsubjectとして照合した。
  Help No impact: 外部開発用OrcaのRPC/daemon経路のみで、ゲーム入力・表示・成立条件・save/assets・
  root Help consumerは不変。並行製品変更の包括承認ではない。primary HEAD base Help gateは
  no production changes、docs indexesは29/77・8/14、storageは184batches/legacy0/
  670465843200bytesでpass。新規package/job・削除・commit/pushなし。
  joint checkpointは同値、runtime `d9aba80c-fa91-4f47-81ec-aef1666f3429`・Run consumer1・
  完全terminal inventory0件は不変。live fence/registry/Run/app/旧pending sendには未適用。
  TAK-14は停止中で、同session新turn・具体的次工程は未確認。次の未実装境界はdaemon
  createOrAttach payload/admissionのdurable spawn claim/result（現在operation ID未伝達）、
  bootstrap barrier、正式Run/WAL adoption、限定enable。空inventoryやdeterministic IDを
  未実行・再実行許可へ読み替えない。

- 2026-10-02 22:49 UTC heartbeat: joint checkpoint/runtime/Run consumer1/完全terminal0件は不変。
  旧sendの正式request-showはpending、snapshotは旧runtime/publishedAtのまま。再送、Enter、
  新Run、fence/registry変更、live app起動は行っていない。TAK-14の新turn・次工程は未確認。
  前回reviewの検証境界を一段接続し、guarded RPC reply-loss testのbridge内で実
  `buildRuntimePtySpawnOptions`を呼ぶようにした。通常runtimeのspawn argsから実builderを経た
  operation IDがadapterへ渡ることをassertし、実daemonのcreate後にreply lossを注入する。
  既存pane reservationを例外時にrejectし、terminal install admissionをfinallyで解放する。
  preflight/commitを含む全controller、native PTY、durable host journalは未検証である。
  test-only SHA256 `7cfab92b781249f8203ae6ed0b7ee7fd6139e4ef28c18ab7940303798ab395ee`。
  関連104tests/3files、tc:node、targeted oxlint/oxfmtはpass。固定read-only reviewを同じreviewerへ依頼。
  Help No impact: 外部開発tool testの既存builder接続のみで、ゲーム入力・表示・成立条件・
  save/assets・root Help consumerは不変。並行製品差分は承認対象外。primary Help HEAD base
  gateはno production changes、storageは184batches/legacy0/670465961984bytesでpass。
  新規package/job・削除・commit/pushなし。旧packageは今回sourceの受入証拠へ流用しない。
  host journalの一次source照合では`DaemonTerminalAdmission`にdurable store seamがなく、
  operation IDもpayload未伝達であることを確認。既存`mutation-receipt-store`はBEGIN IMMEDIATEで
  claimを保存できるが、completed receiptのcapacity pruningが存在する。再利用時は未回収host resultを
  pruning対象へ落とさない契約と、daemon側のprofile/実行host/caller bindingが必要である。
  app内の返却receiptだけをhost側durabilityへ読み替えず、別の非正規JSON台帳は作らない。

- 2026-10-02 22:56 UTC heartbeat: 前回test-only builder接続のexact SHA
  `7cfab92b781249f8203ae6ed0b7ee7fd6139e4ef28c18ab7940303798ab395ee`は固定read-only reviewerが
  P1/P2なしで限定承認。全controller/native PTY/host journal/live admissionの承認ではない。
  joint checkpoint/runtime/Run consumer1/完全terminal0件は不変で、旧sendの正式request-showはpending。
  live再送・Enter・新Run・fence/registry変更・app切替はしていない。同session新turnと次工程は未確認。
  candidateのguarded returned receiptを初回completionから変更できる経路を補正した。
  terminal.createGuardedだけpending→completedのSQL CASとし、同identity・同receipt bytesはtimestampを
  更新せずidempotentに返す。異なるreceipt/method/hash/caller/requestを拒否し、ordinary terminal.sendの
  completed observation更新は維持する。最初の3paths reviewで容量整理がcompleted guarded rowを削除し、
  同identityを再作成できるP2を検出したため承認を保留し、schema v43の保持へ接続した。
  v43はcompleted guarded rowのDELETE/UPDATEをDB triggerで拒否し、現行age/capacity selectorからも
  除外する。旧SQLite接続のpruning/overwriteにも適用し、既存row上限に達した場合は新mutationを
  fail-closedで拒否する。自動expiry、新release権限、別JSON台帳は追加しない。pendingは既存executorの
  effect-possible契約を維持する。この保持はreturned RPC結果だけで、host spawn journalではない。
  v42 migration/reopen・二接続・old timestamp後の別begin・capacity pruning・legacy connectionの
  DELETE/UPDATE・retained満杯拒否・ordinary observationの回帰を追加した。初回capacity試験では
  既存afterEachのclosed secondDbが次caseに残り6cases失敗したためfixture変数をresetし、再検証した。
  最終93tests/7files（全prior-version migration/version-skew含む）、host lease付きtc:node、targeted
  oxlint/oxfmt、diff checkはpass。固定reviewerが更新exact7pathsをP1/P2なしでsource限定承認。
  host journal・startup admission・live配備・同session再開の承認は含まない。
  exact SHA256: store `59ff4f3c732af5f00f54146d9a888e894b9de08ddf2f19c57852228097500a0b`、
  question DB test `5168c157fe5bcb44ad6923a88dddbffa329b136e71df9f0f5f209cca220274ef`、
  capacity `dbd736b811b65983c0185cddcf96a05f3e2b56f135dcb83da918b8b8d25372c8`、
  capacity test `80822254066dd4ff8d2f57f97db64e717333f92d4299b89b258127ef8c13d4c1`、
  migration `738912b3c322028ddb4a2eea796af6cba02243d1c62d0906b3ead80f480d6982`、
  constants `5e68135e7a5cfdcaf1320339a7b05cd585850e22471286af7f95480a0efe2b9b`、
  wire doc `e8288cd6efbdbd9cf7dc5eb352492dca19449da76fda6e1d831e0fd5bcd48b3d`。
  Help No impact: external開発用Orcaのreceipt保持/試験のみで、ゲーム入力・成立条件・表示・save/assets・
  root Help consumerは不変。並行製品差分は承認対象外。primary Help HEAD base gateはno production changes、
  docs indexesは29/77・8/14、最終storageは184batches/legacy0/670466097152bytesでpass。
  新規package/job・原本削除・commit/pushなし。候補は未配備で、TAK-14停止は解消していない。
  次工程はdaemonの実行host journalを既存durable storeへ接続し、作成結果前のbootstrap barrierと
  Run/WAL adoption・限定enableを一貫経路で検証する。returned receiptの保持をhost側の実行結果へ
  読み替えない。daemon-startのprofile/DB path bindingも未接続である。

- 2026-10-02 23:09 UTC heartbeat: joint checkpoint/runtime/Run consumer1/完全terminal0件は不変。
  旧sendの正式request-showはpending、snapshotは旧runtimeのまま。live再送・Enter・新Run・
  fence/registry変更・本体切替なし。同じCodex sessionの新turn・具体的次工程は未確認。
  candidateの実DaemonServer→router→admission→hostへguarded create journalを接続した。
  authenticated profileのdomain-separated callerと43文字operation ID、全spawn payload hashを
  既存OrchestrationDbのBEGIN IMMEDIATE claimへ保存し、freshOnlyで既存session/teardownの
  detach/replacementを拒否する。実PID/incarnation/daemon launch nonceだけをwrite-once completionへ
  保存し、command/env/snapshot/attach tokenはreceiptへ出さない。pending/completedいずれも
  native create再実行を拒否し、別read-only inspectで照合する。別JSON台帳は作らない。
  初回固定reviewでclient disconnect先行時にhost cancellation cleanupがthrowしてreceiptへ
  到達しないP2を検出したため承認を保留し、mainで補正した。native cancelSignal.abortedを
  待ってからspawn gateを解くdeterministic testへ変更し、fresh Sessionのidentityをcleanup/startup input
  より前に記録するhookへ接続した。cancel cleanupはfinallyで維持し、cleanup実行・startup input0件を
  確認する。completed identityは現在の生存・command実行証明ではない。記録前のcrashや記録失敗は
  unknown-effect pendingとして再実行を拒否し続ける。
  最終129tests/8files、host lease付きtc:node、targeted oxlint/oxfmt、diff checkはpass。
  正常create、client消失、即exit、native例外、completion失敗、DB reopen、daemon restart、
  既存session保護とordinary create/attach/cancel回帰を含む。native PTYはmockであり実機証拠ではない。
  exact11pathsは同じ固定read-only reviewerが追加P1/P2なしで限定承認。production daemon-start/profile DB injection、adapter/RPC
  reconciliation、startup bootstrap barrier、正式Run/WAL adoption・限定enableはまだ未接続・未配備。
  SHA256: journal `9082a37ea952dc6119d4342530a0f3d6c454e6274221d6be15c69ce872f8b79e`、
  test `5e5ce1499b4a8b257788c50841e5475e9fd7105b103475bdca994c6a2b040f67`、
  protocol `2d9f75f76c593c0b0b091b38a8e53836cb846d257c998e57d0b640e2c4d67091`、
  server options `cdbe5d6ae5e471632a128acc145f6ff43491b9672408fd8c6f08a128ee1d800d`、
  types `26e0b433ef946b37fa3f9f804e9a24c12394edaeb48e43aa53f63096b20390bf`、
  server `e453ff6e7269ffed4de5d88a68b2cf9777e43356242cc5d3841c3b70b3f600e3`、
  admission `1a5a0669bd8a47e789a7a24de95cab29aee53193c2372c8378334193255d3a3c`、
  router `90e1dd6132edaba9c0619bc505f0c1c2ba9c937667b90f2a62954de52b77916f`、
  host contract `81dcc0c0e5a150b187aefb4fe9d8f4876cc7524f34480395d179dffd06e5d5b7`、
  host create `6f588cadfaa270a7690587449a9a64236fe445940ed8192279a56746c73d81b6`、
  wire doc `0b9d897df6358131602a83416cce0d78dd4851af80e3f548f40269316739622e`。
  Help No impact: 外部開発Orcaのdaemon journal/cancellation toolingのみで、ゲーム入力・表示・成立条件・
  save/assets・root Help consumerは不変。並行製品差分は承認対象外。primary Help HEAD base gateは
  no production changes、docs indexes29/77・8/14、storage184batches/legacy0/670466236416bytesでpass。
  package/job作成、原本削除、commit/pushなし。次工程はproduction launcherの既存profile由来DB path
  bindingとadapterのguarded wire routing。通常requestへfallbackしない。launcherは既にhostowned
  ORCA_USER_DATA_PATHを子へ渡すが、それだけを正式recovery admission証拠としない。

- 2026-10-02 23:28 UTC heartbeat: runtime/joint checkpoint/Run consumer1/完全terminal0件、
  snapshot旧runtimeと旧send pendingは不変。同じCodex sessionの新turn・次工程未確認。
  live再送・Enter・新Run・fence/registry直接変更・本体起動/切替は行っていない。
  candidate production daemon-entry→startDaemon→serverに、既存host launcherが渡す
  ORCA_USER_DATA_PATH由来のprofile storeをlazy接続した。RPCからDB pathを受け取らず、profile絶対path、
  canonical profile/daemon socket/token、redirectされていない実daemon directory、DB/WAL/SHMが
  symlinkや非regular fileでないことをguarded requestの前に検査する。既存profileのorchestration.dbと
  retentionを再利用し、ordinary createはDBを開かない。guarded missing/mismatchはnative前に拒否し、
  ordinary createへfallbackしない。host shutdown完了後に接続をcloseし、restartは同じreceiptを読む。
  初回固定reviewでRPC/idle shutdownが返却handleのclose wrapperを通らないP2を検出して承認を保留し、
  mainで両callback前の所有closeへ接続した。既存host disposal/serverClose完了後にDB.closeを行い、
  RPC/idleのcallback内でcloseが一度だけ実行済みになる試験を追加した。
  最終72tests/5files、host lease付きtc:node、targeted oxlint/oxfmt、diff checkは同sourceでpass。
  ordinaryのlazy不変更、profile mismatch拒否、DB/WAL/SHM symlink拒否、正常guarded保存、host restart後
  同receipt・再spawn拒否、既存disconnect/cancellation回帰を含む。native subprocessはmock。
  host lease付きdaemon-entryのesbuild node18 target/write:false bundleもpass（2104202bytes、
  disk出力・app起動なし）。Node18で実load/guarded SQLite利用可能性を実機検証した証拠ではない。
  exact7pathsは同じ固定read-only reviewerが追加P1/P2なしで限定承認。adapter/RPC reconciliation・bootstrap barrier・正式Run/WAL
  adoption・限定enable・live配備は未接続。profile bindingをrecovery admissionへ読み替えない。
  SHA256: profile store `96d8deed47e87a87fd35c420b33243d75bac2e419bcc5ef249f7e45b730158f0`、
  daemon main `f3113091fbe203abf1d2951816f8a42754516d2feca1b85f2fd136385940f272`、
  main test `89ef4b5175d5119d340af22ec4266c29d131cc3a168f422630f3ed988a9d6f04`、
  entry `c52754a64ecdc262e4cf3c00331540fe658f9050b46a7cb96a657e5e7b0e17d0`、
  server `9dc569aa5ba5d10ea909afbafef92d8155938d9cd7d06470667d2699c264c164`、
  options `c1ce9e26911582c684be0cfb5d42b5ed1e91f5625a599e27ed94e047e6d5c2ad`、
  wire doc `a8bc56ed9198d48cb75aeb6471b677b562c20086f205554019dd021ddff3a52e`。
  Help No impact: 外部開発Orca daemonのhost-owned profile receipt接続のみで、ゲーム入力・表示・成立条件・
  save/assets・root Help consumerは不変。並行製品差分は承認対象外。primary Help HEAD base gateは
  no production changes、docs indexes29/77・8/14、storage184batches/legacy0/670466367488bytesでpass。
  新package/job・原本削除・commit/pushなし。次工程はadapterのoperation-bound guarded wire routingと
  unknown replyに対するread-only receipt照合を接続し、host evidenceとapp returned receiptを分離する。

- 2026-10-02 23:36 UTC heartbeat: runtime/joint checkpoint/Run consumer1/完全terminal0件、
  旧snapshotと旧send pendingは不変。同じ統括の新turn・具体的次工程は未確認。
  live再送・Enter・新Run・台帳/fence直接変更・本体起動/切替は行っていない。
  candidate adapterからoperation-bound createをguardedCreateOrAttachへ接続し、同operation/exact payloadを
  host-owned既存DB journalへ渡した。attachOnly/ensure・chunked historyを拒否し、inline errorからの
  changed-payload fallbackとordinary create fallbackは行わない。実RPC→runtime options→adapter→hostの
  reply-loss試験でnative create1/respawn0、host completed receiptとapp pendingを独立確認した。
  fixed reviewで正常reply後のlate history probe raceがkill→同operation再createを試すP2を検出し、
  mainでkill前にguardedを除外した。late recovery検出自体は維持し、unseeded scrollbackをsuspend保持。
  新回帰はguarded create1/kill0・coldRestore・writerなし・原scrollback保存を確認する。
  operationなしのordinary history経路は不変。既存local agentSession.createも同operation fieldを
  使うため変更対象であり、旧host拒否・既存agent-session受入と追加拒否分岐の直接試験は残件。
  最終81tests/4files、host lease付きtc:node、targeted oxlint/oxfmt、app diff check pass。
  native subprocessはmockであり実機証拠ではない。exact7pathsは同じ固定read-only reviewerが
  最終SHAでP1/P2なしの限定承認。read-only host receipt→app RPC reconciliation、bootstrap barrier、
  正式Run/WAL adoption、限定enable、live配備は未接続で、承認・完了へ読み替えない。
  SHA256: spawn request `d4215d94b7479a703a77683d99306a1e179a683b97086e9905534e7e4b82fcde`、
  spawn result `6bc488a317649ce059405b6aae617b0d9c39a08f195e87a63207fd98cc728338`、
  harness `6a8df5d42c02d2f0129117c86ee152176c877d94bd0a87c35073edef9327c48b`、
  adapter test `1a5785abcbc695ab2dc7c26b195ce03ee185a710a4d92b5f7c9a8ae8b5f44a05`、
  RPC test `70f0ad418b5f8c47620b34548a692459fc2efa6e3ff51d5bde1baf39fd83b87f`、
  journal test `498eddca195b5b0dfae1e593f83517b46454c75f137acca17d6f5c23c6d0c607`、
  wire doc `03b38aed0cfc93e03675eb23e7121b47cc1e5a1beb24b27e25c5ff9941fd7201`。
  Help No impact: 外部Orca開発terminal adapter/host receipt経路のみで、ゲーム入力・表示・成立条件・
  save/assets/root Help consumerへ到達しない。並行製品差分は対象外。primary Help HEAD base gateは
  no production changes、docs indexes29/77・8/14、storage184batches/legacy0/670466490368bytesでpass。
  新package/job・原本削除・commit/pushなし。次工程は正式read-only host receipt照合をapp pendingへ
  接続すること。host completedは現在の生存・送信成功・同session新turnの証明ではない。

- 2026-10-02 23:48 UTC heartbeat: joint checkpoint SHA/現runtime/Run consumer1/完全terminal0件は不変。
  snapshotは旧runtime/publishedAt、旧terminal.sendはpendingのまま。live再送、Enter、本体起動、
  新Run、台帳/fence直接変更なし。同じ統括の新turn・具体的次工程は依然未確認。
  candidateにisolated read-only host receipt observerを実装した。shared schemaを既存journalから
  抽出し、接続前に元operation/session/exact spawn payload hashを検査する。独立DaemonClientから
  inspectGuardedCreateだけを送り、adapter reconnectのresume/background/respawn hooksを呼ばない。
  応答型・payload hash・operation・sessionを照合し、異常/正常ともfinallyでobserverを切断する。
  absent/pendingは再create許可とせず、unsupported/transport failureをabsentへ縮退しない。
  元bindingを現在のenvやinventoryから再構成しない。実RPC reply-loss fixtureで元host payloadを
  採取してobserverへ渡し、completed host receiptとapp pending維持・native create1を確認した。
  最終44tests/3files、host lease付きtc:node、targeted oxlint/oxfmt、app diff check pass。
  absent/pending/no-create、completed inspect-only、hash/operation/session mismatch、invalid binding
  の接続前拒否、unsupported/unreachableを含む。native PTYはmock。exact6pathsは同固定read-only
  reviewerがP1/P2なしで限定承認。caller bindingのdurable保存、app RPC settlement、bootstrap、
  正式Run/WAL adoption、限定enable、live配備は未接続・未承認。
  SHA256: receipt schema `2acc85ed857133c2af21acd99e57167a8bf70b6780695892883ef3451fd5e171`、
  inspection `411e5a4b80f0e9628e545228dac76b46590a9434415832490a5e31e848fa856e`、
  inspection test `bb000e352be31a1c70e0df9cf7197ec5d8267018b0b85d7eb07b8805b82c0ec2`、
  journal `118f053826b146566aeab1ffc9586a530a92f15c538aa1d0c300b3b6d8a8e2f9`、
  RPC test `ff4169eac2adb1dbd14c688f82cd0c8af85a2b39359738051524132c1a1a1a4c`、
  wire doc `9f2c36932230668992ea498ff750548fdf3000a5538de771425e8698404df776`。
  review中のscope外追跡でdaemon-client-rpc-request.tsのcancel対象がcreateOrAttachだけと判明した。
  guardedCreateOrAttachの送信後abort/timeoutはhost preparationをcancelしないため、live接続前に
  同request-ID cancellationを接続し、timeout付与がexact payload hashを変えない設計と併せて検証する。
  既存agentSession.createも影響対象。observerの限定承認をこのtransport残件の承認へ流用しない。
  Help No impact: 外部開発Orca receipt読取/schema共有のみでゲーム入力・表示・成立条件・save/assets・
  root Help consumerは不変。並行製品差分は対象外。primary Help HEAD base gateはno production changes。
  storage184batches/legacy0/670466629632bytesでpass。新package/job、原本削除、commit/pushなし。

- 2026-10-02 23:55 UTC heartbeat: 現runtime/joint checkpoint/Run consumer1/完全terminal0件、
  snapshot旧runtime/publishedAt、旧send pendingは不変。同じ統括の新turn・次工程未確認。
  live起動/切替・Enter・本文再送・新Run・台帳/fence直接変更は行っていない。
  mainで前reviewのtransport残件をcandidate修正した。guardedCreateOrAttachのnested spawn.sessionIdを
  抽出し、abort/timeoutを既存exact client/session/request-ID cancelCreateOrAttach settlementへ接続。
  cancelAfterMsはguarded requestのspawn外側へ付与し、protocol→admission→preparation.registerへ
  分離して渡す。元spawnとjournal payload hashを変更しない。host timerの既存positive safe integer/
  300000ms clampを維持する。old guarded hostはoptional budgetをignoreし得るため、取消の未知結果を
  成功/exitへ読み替えず、ordinary create fallbackや再spawnは許さない。
  最終58tests/4files、host lease付きtc:node、targeted oxlint/oxfmt、app diff check pass。
  real hostのnative gateでabort/timeout→host cancelSignal aborted→child cleanup/startup input0、
  completed host receiptと元payload hash保持・再create拒否/native1を確認した。nativeはmock。
  exact6pathsは同固定read-only reviewerが上記最終SHAを照合し追加P1/P2なしで限定承認。live配備・durable caller binding保存・app RPC
  settlement・bootstrap barrier・正式Run/WAL adoption・限定enableは未接続で、再開成功ではない。
  SHA256: transport `0d2bbdc0ce4149d157d3a361cd40b2bfe09f5cb79430ae3bed9dd5bf322ef018`、
  transport test `46dc05fef5ec0db371bb50545e35a237198d751721be2022563add8fb252c5b6`、
  protocol `6b57c92625b508234715829d8840b3e84fdc7768877a6de2a2c960653f1f8555`、
  admission `918078446e4ea84d16ef60e4fdf859a9a29a94ed64dae8677937d037880c1e52`、
  journal test `a4ee53158cf062d3ae4370abe1c39c8bd5b299ff8dce245e1a2a23b7bfe15837`、
  wire doc `53d99d96b1107c36f91c3023c31d37ea95445ec7b7a6db04e221f9833577a1fc`。
  Help No impact: 外部Orca transport cancellation/preparation budgetのみでゲーム入力・表示・成立条件・
  save/assets/root Help consumerは不変。並行製品差分は対象外。primary Help HEAD base gateは
  no production changes、storage184batches/legacy0/670466760704bytesでpass。
  新package/job、原本削除、commit/pushなし。次工程は元host bindingを副作用前にdurable app pendingへ
  保持し、正式読取でだけsettle可能な経路を設ける。現在のenvやinventoryから元payloadを推定しない。

- 2026-10-03 00:01 UTC heartbeat: 同runtime `d9aba80c-fa91-4f47-81ec-aef1666f3429`、
  joint checkpoint `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`、
  Run `run_3236488f0dff` consumer1/旧統括owner、完全local terminal inventory0、
  snapshot旧runtime/publishedAt、旧send `dd0ba73f-53e4-4304-92cd-8beeaf04a1da` pendingは不変。
  同統括の新turn・具体的次工程は未確認。本番起動・再送・新Run・台帳/fence直接変更は行っていない。
  mainでcandidateのguarded RPC→runtime→PTY builder→adapterを接続し、host create requestの前に
  既存app pending receiptへ元operation/session、変換済みspawn payload hash、socket/token path digestを保存。
  schemaはbounded/strictでcommand・env・認証tokenを保存せず、trusted callbackをwire paramに公開しない。
  pending checkpointはpreviousReceipt CAS付き、同一checkpoint反復はtimestamp不変、異なるbindingは拒否。
  保存失敗はnative dispatch0となり、unknown pendingから再createしない。DB再open後も元bindingを維持。
  providerのcallback脱落、返却session不一致/非local、SSH/non-daemon、materialized/concurrent pane adoption、
  実行時owner及びdirect attachを拒否する。固定reviewで見つかったguardより前の既存owner副作用を閉じ、
  identity否定fixtureはvalid checkpointを先行させて個別guardの回帰を検査する形へ修正した。
  最終same-subject85tests/6files、host lease付きtc:node、targeted oxlint/oxfmt、app diff check pass。
  正規RPC/runtime/builder/adapter/host fixtureでnative前のpending保存、reply-loss後のhost completedと
  app pending保持、checkpoint失敗/non-daemonでdispatch0、adoptStablePane呼出0を確認。nativeはmock。
  exact23pathsは同固定read-only reviewerがSHA全件を実測照合し追加P1/P2なしで限定承認した。
  `sha256sum`の提示path順一覧を再hashしたsubjectは
  `96b3a22e2e1cfec203d7cfaa5e70d530eec7c8bcc96a9b5f921944090b96a311`。
  critical SHA: shared checkpoint `b48ffebd36fbf352d68e05b8c799581838b6f5b3ce087e46e87aa368fb57f709`、
  pending writer `9eb1aa7ace090e408a78e33c97fee05c47a25a331e5bdc9fbc878fb04412d764`、
  adapter producer `2422c585f897977e05958af5b27d03127d70a72b771bcaec55d68ea6107f949a`、
  DB CAS `7fa305787b8da76165e5a5cc0f4f91f030b58779106c6262d172105c9ba99579`、
  RPC test `5c66c3ad501d8d8cea022ca6bde4c22dc80767b2f39391c10db8d6250ae4222a`、
  route test `df9783b7f8c11329632acc606b3ee322e76e1539e74489f4e1604501c085e385`。
  Help No impact: 外部開発Orcaのreceipt/CAS・fresh routingだけでゲーム入力・表示・save/assets・静的Help
  consumerに到達しない。並行製品差分は承認対象外。primary HEAD base Help gateはno production changes。
  storage184batches/legacy0でpass。新package/job、原本削除、commit/pushなし。
  この限定承認は正式host-result→app receipt reconciliation、bootstrap、Run/WAL adoption、限定enable、
  live配備/TAK-14再開の承認ではない。次工程は保存済みoriginal bindingを正式read-only照会へ渡し、
  元endpoint/owner・session・payload一致を検査してだけapp pendingをsettleする経路を接続する。
  absent/pending・現在env/inventoryの推定・入力受付を稼働証明に使わない。

- 2026-10-03 00:21 UTC heartbeat: joint checkpoint
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`、runtime、
  Run consumer1/旧統括owner、完全local terminal inventory0、旧send pending、旧snapshotは不変。
  同じ統括の新turn・具体的次工程は未確認。本番操作・再送・新Run・台帳/fence変更なし。
  mainでcandidate observerのendpoint照合欠落を閉じた。旧observer schemaが保存済みdigestを
  stripしていたため、既存strict shared host checkpointを直接要求しsocket/token path digestを
  接続前に比較する。不完全な旧bindingを現環境から補完しない。変更endpointは接続/request0。
  元DB checkpoint→real host journalのreply-loss fixtureを含む47tests/3files、targeted oxlint/
  oxfmt、app diff check pass。nativeはmock。固定read-only reviewerがexact4pathsをSHA照合し
  追加P1/P2なしで限定承認。path digestはtoken内容やdaemon本人・生存の証明ではない。
  SHA: observer `f68a203d0cf8532b95a359bd91d621a64d9166d9dc7e912b74369f4af9a677e3`、
  test `87171f0c3f454413c731083402658dec365b45878c3c4cc0d4cec76f15169656`、
  receipt `595a3ac1773183b050949581bffbfaa359c0457844fe33f1aa5e2b69e81cbee0`、
  wire doc `0af3a149c1baad9bb60fb81d3d3d87e6d88d062e008ff15309423e3dc60ca534`。
  Help No impact: 外部Orcaの保存済みbinding/接続前検査のみでplayer InputAction/UiIntent・
  ゲーム表示/save/assets/静的Help consumerに到達しない。並行製品変更は対象外。
  primary HEAD base Help gateはno production changes、storage184batches/legacy0/
  670467035136bytesでpass。新package/job、原本削除、commit/pushなし。
  残件はformal host-result→app pending reconciliation。host receiptにないworktree/tab/pane/handleを
  current inventoryから捏造してterminal.createGuarded成功へ変換しない。元caller/request/method/
  parsed sourceとcheckpointのCASを保ち、保存済み元surface metadataか別種の正式host結果を
  明示する契約が必要。今回の承認はsettlement/bootstrap/Run/WAL adoption/限定enable/配備を
  含まず、候補修正は未適用、依頼の再開成功ではない。

- 2026-10-03 利用者「遅い。再開させてください」への継続調査: current runtimeから旧統括handleを
  terminal showして`terminal_handle_stale`を再確認。TAK-14 workspaceは存在・非archiveだが
  live terminal0。現runtimeはguarded-create-receipt capability未搭載。無関係workspaceに
  attached terminalがあるためOrca全体再起動で復旧を試さない。旧送信は依然pending。
  再承認不足ではなく、正式復旧の配備・接続が未完了である。再開したとは報告しない。
  mainでruntime決定済みworktree/handle/tab/paneをhost dispatch前に既存pendingへ追加保存する
  candidate修正を実施。元surfaceを後のinventoryから推定する必要をなくす。
  shared fieldはhistorical観測互換のためoptional。未保存surfaceをterminal完了へ推定昇格しない。
  providerによるsurface注入は拒否し、元runtime値をsilent overwriteしない。予定surfaceは
  登録・稼働・Run採用の証明ではない。agent-team計算はconst destructureに整理して300行guardを
  維持、重複catch-releaseを除き外側finallyで例外時lease解放を維持した。
  最初の結合testは期待tabを固定fixtureにしたため失敗し、実際のruntime spawn引数へ修正。
  最終50tests/4files+tc:node、ordinary terminal/idempotency/host-binding回帰19tests/3files、
  targeted oxlint/oxfmt、diff check pass。nativeはmock。固定reviewerはexact6pathsをSHA照合し
  P1/P2なしで限定承認、settlement/bootstrap/Run adoption/live配備は含まない。
  SHA: shared `316fc965157da0b8cd85f346635d8b28a88b8978e198bf5c36ae1f9f2dd8e7b9`、
  surface writer `646b1dda6e1ffe9364e0f6e42adb17e14058b893959761675a3f2df4bee6c4c0`、
  writer test `8aaddb8c725f88ccc6ee4dc96bb1494bf7567bb18470040d73522de57e806ce6`、
  runtime `4aba87f0815c69fb6017d1a92a20799d385d660e012352b07c18cdef747816ab`、
  RPC test `9c1260fd856ee9ed674472eb6918873d5393865fbf97c35feb346ff90b1e9e70`、
  wire doc `f0805c55da6ffe00c51adfc2736ae51e5472ea28182c8ec8e05583ded2571833`。
  Help No impact: 外部Orcaの予定surface checkpointのみでゲーム入力・表示・save/assets・
  root Help catalog consumerは不変。並行製品変更は対象外。旧成果・session・Run・担当tabを保持。
  配備と同じ統括の新turn・次工程の確認は未達成。

- 2026-10-03 00:40 UTC heartbeat: joint checkpoint
  `0f085dbc4027a4af0266c7f8bc635b32631d7897671c69d3439709d50170c399`は不変。
  runtime `d9aba80c-fa91-4f47-81ec-aef1666f3429`、同じRun consumer1/旧owner、
  TAK-14完全local inventory0、旧send `dd0ba73f-53e4-4304-92cd-8beeaf04a1da` pending、
  snapshot旧runtime/更新時刻を再照合した。空inventoryをexit/完了扱いせず本番再送なし。
  mainでformal settlementの内部primitiveを追加した。元pending caller/method/source hash、
  derived operation/handle、保存surfaceを要求し、既存isolated observerの正式completed host結果と
  同期的な既存登録のPID/incarnation/session/surfaceが一致した場合だけapp claimを完了する。
  completeMutationReceiptへ元pending receiptのCASを追加し、非同期host照会中に別DB connectionが
  checkpointを変更すると完了を拒否する。未登録・legacy surface欠落・absent/pending・通信不明は
  pending保持。予定surfaceだけで登録を捏造せず、attach/create/adopt/startup input/Run変更なし。
  初回fixtureの非UUID leafとtest callback推論型を修正。最終38tests/4filesとtc:node、targeted
  oxlint/oxfmt check、app/primary diff check pass。固定read-only reviewerはexact4pathsのSHA一致を
  確認しP1/P2なしで内部primitiveのみ限定承認した。正式routing・reader実接続・bootstrap・配備・
  live復旧は承認範囲外。primitiveは未接続・未配備で、同じ統括の新turn/次工程は未確認。
  SHA: settlement `0cf89f042319cf4c2a0f78e03cf0f4a5ca43d483cd122b2cb74d652dd001fba5`、
  test `bb9633773c0fbcd171115f056125c5319b1bda4774ad019286dbb0597346779a`、
  DB writer `fe5bf4eddf991d34edcea79f7ddc03327a03cdbd011ed83c2e71ef2bda8a7e91`、
  wire doc `824d89c8589250efc3cb0766e2d2b390bf9021efa91dec1be4197501fb313b0e`。
  Help No impact: 外部Orcaの未接続settlement/CASはゲームInputAction/UiIntent・Help catalog・
  save/assets/player表示へ到達しない。並行製品差分は対象外。primary HEAD base Help gateは
  no production changes、storage184batches/legacy0/670467186688bytesとdocs index check pass。
  新package/job、原本削除、commit/pushなし。次工程は既存runtimeの本当の登録を読む同期readerと
  正式pending routingの接続、未登録host結果のguard付き復旧・bootstrap、同Run採用、限定配備。

- 2026-10-03 利用者「作業を再開してください」に基づき復旧作業を再開した。automation orcaは
  利用者停止時のPAUSEDを維持し、古い通知・Enter・旧sendを再送していない。
  actual runtime登録readerをguarded fresh-spawnのPID/incarnationへ接続し、元pendingの正式
  host結果を照合するroutingをdurable mutation executorへ接続した。未登録・別incarnation・
  旧surface欠落はpending保持でcreate/attach/adopt/inputを再実行しない。
  固定reviewは実controllerのspawn.tsとspawn-commit.tsがPIDを落とすP2を検出した。
  mainで双方のoptional PID伝播を修正し、real controller/commit/create/registration回帰を追加。
  最終100tests/8filesとtc:node、対象lint/format pass。fixture TS2352補正を含むexact14pathsは
  同じ固定read-only reviewerが追加P1/P2なしで限定承認した。production sourceは未配備。
  隔離packaged受入helperのreplacementをguarded createへ変更。reviewで非UUID requestの
  拒否経路を検出して修正、最終helper SHA
  `62fc90ff16edfa693d3087de6bac09c279b9e31af95d180b95056be5ac5dec12`を限定承認した。
  新batch `orca-guarded-create-packaged-20261003-08`はprimary coordinatorへ登録してhost lease下で
  relay/CLI/main buildと新packageを作成。global CLI installは行っていない。旧package02は不変。
  package08の実daemon/terminal guarded createでactual PID/incarnationとformal completed receipt、
  同fixture Runのguarded移管/replay/旧owner拒否、loaded identity、隔離HOME/非表示windowがpass。
  Playwright 1passed、登録済み独立verifierもpassでseal/finalizeした。live新Runは作っていない。
  package buildId `44d94c216732668e0c4f9e39454ec32ed266e1889bd5523fc6d6485307cac790`。
  exact保持path `/home/satotakumi/tools/orca-ui-lifecycle/.git/orca-ui-acceptance/guarded-create-20261003-08`、
  owner orca-hardening-coordinator、consumer orca-guarded-run-live-recovery、実測1065250816bytes。
  次用途はこのpackageの限定配備・rollback選択と同session復旧受入、終了後用途解除する。
  Help No impact: 外部Orca内部fresh-spawn・receipt・testsのみでゲームInputAction/UiIntent・
  表示・save/assets・Help consumerへ到達しない。並行製品差分は対象外。
  未達はbootstrap barrier→正式Run/WAL adoption→exact activation enable→正規prepared launchの
  上位接続と本番配備。同session新turn・具体的次工程は未確認、TAK-14再開成功とは扱わない。

- 2026-10-03 利用者「作業を再開してください」への継続復旧: bootstrapをnormal prepared launchへ
  接続し、formal create barrier→exact replacement observation→owned lease下のRun intent/settlement→
  paired WAL adoption→immutable enable→同session/prepared/retained launch FDの経路を候補実装。
  native executorは実peer PID/birth/argvと実Popen subtreeで別途認証し、scope照会を独立read-only
  socketへ分離。scheduler lockを子が保持した実socket試験でlock inversionを否定した。
  mixed installation、45秒総deadline、遅着completed、source drift、既存intent再実行ゼロを検査。
  8関連moduleはouter input生成追加後196 tests pass、bootstrap専用10 tests pass。
  固定reviewは接続fixtureとouter input生成追加を限定承認済み。live配備・復旧承認ではない。
  input生成はsource/control/registry/origin/runtime/Run、両operation absentをowned leases下で
  照合しprivate immutable inputだけを作る。既存inputは同一でも再create権限へ昇格させない。
  source tuple / authorization_referenceは単独の実行権限・exit証明ではない。
  live daemon36 PID11490を置換して他案件PTYを失うことを防ぐためcandidate protocolを37へ分離。
  36→37 legacy socket adoption試験は旧PID/incarnation/入力経路保持、spawn0でpass（実機upgrade未実施）。
  protocol/adoption/host PID propagationの3files5testsとnode typecheck/oxlint pass。
  primary登録batch `orca-guarded-create-packaged-20261003-09`をhost lease下でbuild/package/隔離受入。
  actual guarded terminal PID/incarnation、formal receipt、同fixture Run transfer/replay/refusalと
  loaded build identityがPlaywright1passed。独立verifier pass、seal/finalize済み。live新Runなし。
  buildId `1e1e2c9d96092b75f768e226031688d4f7aefa8f0a60aea5f69cad7f3e3c4305`。
  retain path `/home/satotakumi/tools/orca-ui-lifecycle/.git/orca-ui-acceptance/guarded-create-20261003-09`、
  owner orca-hardening-coordinator、consumer orca-guarded-run-live-recovery、1065250816bytes。
  release_whenはdeployment/rollback選択と同session復旧受入終了。08の凍結結果は37へ流用しない。
  primary storage186batches/legacy0/672598343680bytesでpass。原本削除・commit/pushなし。
  Help No impact: 今回の外部Orca内部recovery/daemon routing/receipt/test経路はゲームの
  InputAction/UiIntent、表示文言、save/assets、Help consumerへ到達しない。並行製品差分は対象外。
  live runtimeは依然d9aba80c-fa91-4f47-81ec-aef1666f3429/app10839、guarded-create capabilityなし。
  normal quit CLIがなくnative computer providerはmenus/hotkey非対応、accessibilityはframeのみ。
  packaged通常processにはSIGTERM→normal quit handlerがないため強制終了を代用しない。
  利用者へOrcaメニューの通常終了のみ依頼。終了後はlaunch-supervisedで限定配備/新runtime照合、
  新checkpoint/input→sole guarded create→bootstrap復旧→同統括新turn/具体的次工程を確認する。
  現在TAK-14は未再開。監視automationは利用者停止によるPAUSEDを維持し旧通知再送なし。
  docs index生成/check自体は一致。docs全体gateは既存node_modules/debug・nan READMEの
  欠落link4件で拒否。今回の復旧差分と無関係なvendor原本は編集/削除していない。

- 2026-10-03 利用者「どうぞ」後、旧Orca app10839の通常終了を確認して限定配備。
  同一package09をfresh batch `orca-guarded-create-recheck-20261003-10`で再受入し、
  Playwright1pass・独立verifier pass・seal/finalize。旧09結果は変更していない。
  launch-supervisedとcanonical shim/currentをレビュー済みpackage09へ選択。旧package保持。
  actual app1459518/runtime `817a80b2-cbed-43b8-9274-37e48cc13f57`、loaded build
  `1e1e2c9d96092b75f768e226031688d4f7aefa8f0a60aea5f69cad7f3e3c4305`とprotocol37を照合。
  daemon36 PID11490と他案件Claude PTYは保持、ゲーム編集/build/test、新Run、pushなし。
  初回preflightはapp-owned supervisionの生涯frontdesk-ui lockと衝突、effects前に拒否。
  mainでsingleton/UI mutation lockを分離し旧継承FDはsingleton取得後のみ移行。
  fixed reviewのshutdown orphan P2を修正し、busy中singleton/child保持・取得後cleanupを追加。
  関連94tests、Ruff、diff-check pass、exact3paths fixed read-only approval。
  source-bound refreshで同controller PID1460203のexec/new generation/sourceを確認。
  immutable recovery inputを生成し、sole guarded create
  `b2bdf95e-6d36-49a5-951c-a0716cc1761e` formal completedを確認した。
  復旧terminal `term_756e4b0f1b47a24bb2baa44b01480999`、incarnation
  `fe677615-7fb8-462c-8b67-342c5b69b028`、PTY shell1565694を保持。
  bootstrap admissionが短時間のUI lock busyでeffects前に終了。mainで最大45秒の
  admission-only待機を追加。local retained pidfd/ancestry確認、effects前remote/source/control照合は
  維持しbody例外をretryしない。fixed reviewのRPC期限P2は待機中RPCを除去して解消。
  関連39tests/Ruff/diff-check pass、exact bootstrap SHA
  `d83fed9639320cff12a826bf75f4a9a9252005b46b6230c303bc513e58da5f28`とtest
  `5334226ceed64e5301fd126077995ed4996a3a21ce7a3e437174d2dafe721a0d`限定承認。
  controller正常refresh後source `d4e144bc6da21b3f1de372f6c5b73bd2baad160043cb76a93c02f53468609cff`、
  generation `ab73239d-e2d9-45cd-b529-ff8e5685b8c1`、同PID/runtimeを確認。
  既存bootstrapのshell復帰・children0・fence/transfer intent不存在・Run旧owner/gen1を照合後、
  同terminal/同immutable inputの正規launchを実行（new create/Run/旧通知再送なし）。
  terminal switchに伴い旧履歴tab `term_78d02e59-0c3c-4fe3-8018-6f18204f4c25`が自動復元され、
  complete inventoryのsingle replacement guardが拒否した。旧tabは実PID1636604/bash/children0、
  run-current=null、新bootstrapもshellへ終了。原登録は旧schema1/terminal2d1f...のまま。
  fence/Run移行なし、同saved Codex sessionの新turn/具体的工程は未確認で再開成功ではない。
  この時点では旧履歴tabのcloseを利用者へ確認したが、保持対象を過大解釈した不要な確認だった。
  利用者「そんな指示はしてません。適宜判断」後、実PID/bash/children0・run-current=nullと
  saved session原本844889319bytes/inode30843186を再照合し、その未使用shellだけ正常closeした。
  Help No impact: 変更は外部Orca監督・復旧admissionのみで、ゲームInputAction/UiIntent、
  player表示、save/assets、Help consumerへ到達しない。既存並行製品差分は対象外。
  primary storage187batches/legacy0/672598581248bytes pass。原本削除なし。
  automation orcaは利用者停止由来のPAUSEDを維持。

- 2026-10-03 runtime接続観測時刻の周期更新によりimmutable inputのruntime digestだけがずれ、
  同terminalのbootstrapがeffects前に拒否した。source/loop/lifecycle/control/registry/sessionは一致。
  mainで未開始時だけcanonical外部scope・pendingなし・旧Run binding・正式operation absent・
  実replacementを再照合し、immutable inputを変更せずappend-only checkpoint証跡を保存する経路を追加。
  fence/intent既存ならrefresh拒否。固定reviewでobserver旧checkpoint残存P1を発見し適用前に修正。
  同pidfds/launcherを維持しinspection fingerprint以外の変更を拒否、fresh remote後にobserver再拘束。
  real adopter厳密比較まで通す回帰を含む44tests PASS、Ruff/diff-check PASS、固定read-only限定承認。
  source-bound refreshでcontroller同PID1460203・同runtime817a80b2...、新generation
  084d3e36-4aff-4934-a5ce-54aca3a31cfb/source
  e8d2d9b3d9cd85d8a5c5041c6e7ab51ae0f5c9cbe151fe9d2c1bb66b322c213bを確認。
  same immutable input/terminal/incarnationで正規launch。受付だけを復旧成功とは扱わず実工程確認中。
  Help No impact: 今回のcheckpoint/observer変更は外部Orcaの復旧だけを消費者とし、
  ゲームInputAction/UiIntent、player文言・前提・結果、save/assets、Help catalog経路を変更しない。
  既存Help gateの別変更理由は今回の判断根拠に流用しない。並行製品差分は範囲外。
  primary storage187batches/legacy0/672598749184bytes PASS。原本削除・製品編集・game build/testなし。

- 2026-10-03 同Run `run_3236488f0dff` の正式移行operation
  `333782fb-fbc3-4f28-91b0-e23aded692aa` completedを確認。consumer generation1→2、
  sole recovery terminalへ移行済み。ただしbootstrapはhistorical attention markerの旧Run bindingで
  provider起動前に拒否。実launcher1770589の消滅・同pane streamの終了コード1を確認。
  registryはstartingのまま、loop原本は旧consumer、enableなし。移行RPCは再実行していない。
  mainでimmutable input/fence/intent/WALに基づくhistorical read-only検査を追加し、旧判断の
  replay/新決定への流用を禁止。別immutable successionで同pane/session/Runのpre-provider
  failed launcherを移行する明示recover-settled候補を追加。原本WAL/終了/ACKは変更・捏造しない。
  固定reviewの3点（RPC後fresh subject/lease順、厳密Run型・intent/WAL binding、fence中の
  Linear期限切れ）を修正。正式read-only再観測を束ね、canonical runtimeの時刻は書換えない。
  成功・各保存間source/control/FD drift・部分保存失敗を含む関連146tests PASS、focused20 PASS、
  Ruff/diff-check PASS。実データhistory/fresh外部scope/source照合PASS。live適用は再review待ち。
  primary storage187batches/legacy0/672598859776bytes PASS。Help No impact: 外部Orca復旧のみで
  player InputAction/UiIntent/表示/save/assets/Help consumer経路に変更なし。製品編集/build/testなし。
  同Codex sessionの新turn・具体的次工程は未確認であり、TAK-14再開成功とは報告しない。

- 2026-10-03 8パス固定review承認後controller同PID/runtimeをsource
  `0ca6abd7442a2f027dbf3c129d93a427e12310b1831f8bc25ad92fc3f251b0b0`へrefresh。
  同pane/incarnation/Run gen2/旧launcher消滅を再確認し明示recover-settledを1回実行。
  受付receipt `be63beec-491b-4523-ba69-19ff377d24a4`。succession
  `8d553ae39cb10c23c5da4f207383adbee7dcc61aa357350dff38d9a6c0e828d3`を保存後、loopのみprojected。
  次検査で旧acceptance_replan所有者が拒否。registry旧after、enableなし、provider未起動。
  実launcher1980482/birth13156710消滅、同shellの直後status probeでexit1を確認した。
  元bootstrap/Run RPC/入力は再送せずmainが追加恒久修正。通常load全契約をchecked_dataへ共用、
  歴史replanは正式transfer証跡に限定した読取copyで所有照合しarchive/attempt/ACKはそのまま。
  実データfull projected契約PASS。既存succession全SHA/activation/birth/失敗表示を束縛した
  今回限定append-only successorを追加し、projected済みloopは再保存しない。未知pair/headは拒否。
  元succession/transfer/input/fenceは保持。関連135tests PASS、追加60tests PASS、Ruff/diff-check PASS。
  固定read-only再review中、未適用・同session新turn未確認。native/replanの新効果guardは緩和しない。
  Help No impact: 外部Orcaの履歴読取・復旧のみ、ゲーム/player/save/assets/Help経路変更なし。
  primary storage187batches/legacy0/672599019520bytes PASS。製品編集・game build/testなし。

- 2026-10-03 追加7パス固定read-only承認後、controller同PID1460203/runtime817a80b2...を
  source `ca744a082acf8ed73d869ff76ce09a46e6995d0804eb4a3e99de59b7cff13a61`へrefresh。
  generation `feea9de9-c863-44f8-98b1-6724cd059149`、snapshot schema4/fresh publishedAtを照合。
  同terminal/incarnation/Run gen2・旧launcher消滅を照合した専用partial successor経路で再開。
  受付receipt `11cbbb07-1962-4e27-b050-cdd028e056e5`のみを成功根拠にはしていない。
  registry ready、activation `7517dc0d-d14a-4746-936a-8ae37dd77c52`、launcher2087516、
  provider_root2103719、ACK `2026-10-03T08:03:41.278105Z`を実見。
  同保存session `01a0d377-feef-7520-bef5-50565ef282cb`の元inode30843186を保持し、
  `08:03:05.204Z` task_started/new turn `01a100c9-c353-7e23-8d8a-1d07e04ae480`を確認。
  新turn内show/watch成功とnative_bridge capabilitiesによる正式残工程照合を実見。
  Driver同PID2087516/running、fresh heartbeatを確認。既存担当/Run/成果/原本を保持。
  復旧と実際の継続開始は確認済み、全体受入correction_required/未解消3指摘は維持。
  automation orcaは古いhandoffを新証拠へ更新してACTIVEに復帰（5分間隔・意味ある変化のみ通知）。
  これは汎用unknown effect再実行や依頼全体完了の承認ではない。

- 2026-10-03 08:14Z 再開後turnの`08:07:28.829Z task_complete`と実際のTUI idleを確認。
  Driver heartbeatだけでは製品工程の継続と扱わず、同pane/incarnation/Run gen2を再照合。
  利用者の最新の直接指示「適宜判断してください」を新規の限定promptとして一度だけ配送。
  receipt `87aa5ea8-004a-47d1-a3ea-8ca8b6309fda`はinput_acceptedに留まったため再送せず、
  同保存sessionの`08:14:07.572Z task_started`、turn
  `01a100d3-decd-7d92-837e-5096eba965f1`を直接確認。
  新turn内でSkill/storage規則読取と正式showを実行、正式native recipeの登録元・guard調査へ着手。
  旧通知/bridge/Run移譲/partial successorは再実行していない。全体受入未完了を維持する。

- 2026-10-03 08:48Z監視で同保存sessionの08:43:07.870Z task_completeを確認。
  runtime817a80b2...、同terminal/incarnation/Run gen2を維持。Driver heartbeatは稼働判定にしない。
  停止原因は別々の2件: exact read-only capabilitiesがnative-continuation leaseに待たされること、
  完了済み承認Taskで新たな実装不足が確定しても通常correctionとfailed-only acceptance-replanの
  いずれにも適合する正式replan経路がないこと。既存3指摘は全て未解消のまま保持。
  mainがlive helperを変えず、/tmp/orca-capabilities-review-zLOokKに2パス修正候補を作成。
  exact capabilitiesのみlease取得前にlive authorize/code_revision/前後admission stopを維持した読取。
  effectful操作と不正payloadは既存serialized拒否経路のまま。候補bridge SHA
  d0ca60070d326a1813eeef3d64899397b9418462e0fce01d090d81f66e2f8ca7、候補test SHA
  2ea3b5377468d4167bfa725bbcc3807245a999b9398cda66a53040169f6f5ae6。
  候補実体dynamic importで54tests PASS、Ruff/diff whitespace PASS。初回はtemp移動したtestの
  __file__由来child cwdだけが不適合で1件失敗、元testlocationを保持した再実行で全件通過。
  固定read-only orca_entry_auditはこの2パス・上記SHAを承認。別socket requestの待機全体を
  解消する承認ではない。live適用・旧launcher正常終了・同会話新turnは未実施/未確認。
  同じ旧bootstrap/recover-settledを再利用せず、現enabled所有の正規source upgrade/restartを
  終了/ACK/source/Run settlementに束縛する次工程をmainが調査中。新効果guardは緩和しない。
  Help No impact: 今回2パスは外部Orcaの能力読取のみ、player InputAction/UiIntent/表示/前提/
  結果/save/assets/Help manifest/consumer経路に変更なし。primary Help gate PASSは並行製品差分の
  既存判定であり、今回候補の承認証拠には転用しない。primary storage187batches/legacy0/
  672599162880bytes PASS。temp候補261812bytesのownerはmain、consumerは固定reviewと安全な
  適用、next actionは正式source upgrade/restart、release_whenは適用検証又は候補取下げ完了。
  原本/旧通知/受付receipt/Run操作は再送せず、製品編集・game build/testは実施していない。
  依頼全体未完了、実際の再開未確認。追加利用者承認待ちではない。

- 2026-10-03 09:02Z 上記の「新replan必須」という見立てを再検証し、既存installed
  orca_tooling_correction.pyのcategory tooling経路を発見。sealed rejected integrationから、
  元target base由来・scripts/限定・実際に変わるhost-tooling commitを統括所有で同Runの
  新subject検証/固定reviewへ渡す経路がある。今回runnerがhost toolingだけで完結する限り、
  failed-only acceptance-replanや完了Task再試行は不要であり、新たなguardを安易に追加しない。
  同runtime/pane/incarnation/Run gen2/current launcher/providerを再確認。全attempt release/ACK、
  approved A、inbox空、tooling_corrections未使用を照合した新情報に基づく限定promptを1回配送。
  receipt7bfa74c1-444f-4ce8-9dd4-b69a5f3675a0はinput_acceptedのみ、再送していない。
  同保存sessionの09:02:22.833Z task_started、新turn01a10100-0c6a-7bf0-8dda-2c39edeee209を確認。
  新turn内09:02:56.971Z tooling補正経路の検索、09:03:24.192Z storage規則/Git/worktree確認、
  09:03:41.761Z元baseと現headのscripts差分照合、09:04:09.704Z primary storage確認を実見。
  同統括の具体的な次工程の適合調査は再開した。製品実装/補正適用/全体受入完了ではない。
  能力照会2パス候補はlive未適用で保持。実行中統括をその適用のため安易に中断しない。
  候補hold orca-capabilities-fix-20261003は正式validation retain経路で登録、274432bytes。
  primary storage再check PASS(187batches/legacy0/672599441408bytes)。docs --writeで両index更新
  成功、docs checkは既知のnode_modules/debug 3箇所/nan 1箇所のリンク欠損で失敗、原本は未変更。
  orca-cli/orchestration/hell-workers-review-help-impactを使用。不要な利用者再承認待ちは作らない。

- 2026-10-03 09:12Z 統括の09:05停止原因は、mainの候補holdが隔離環境で隠れる/tmpに
  あったこと。承認済み2ファイルをworkspaces/hell-workers/infrastructure-candidates/
  capabilities-20261003へ保全し、正式validation retainで新hold
  orca-capabilities-fix-20261003-persistentを登録、旧consumerを正式releaseした。
  旧/tmp原コピーも新hold内temp-originへ移動して保全し、削除はしていない。
  bridge SHA d0ca60070d326a1813eeef3d64899397b9418462e0fce01d090d81f66e2f8ca7、
  test SHA 2ea3b5377468d4167bfa725bbcc3807245a999b9398cda66a53040169f6f5ae6を維持。
  新hold372736bytes、owner main、consumer TAK-14 guarded infrastructure application、
  next action安全な同session適用/検証、release_when適用検証完了又は保全済み取下げ。
  固定read-only orca_entry_auditが保持配置変更を承認。live bridgeは未変更/候補未適用。
  primaryと統括同等mountのstorage PASS。実統括も09:08:58.655ZにPASS
  (187batches/legacy0/672599678976bytes)を確認。新限定通知receipt
  b7a90142-afb6-4a12-86d7-d839b71a9811は再送せず、同保存session09:08:24.367Z
  task_started、turn01a10105-90a5-7113-9a37-7c189918d173を実見。
  09:09:09.706Z、統括自身が元base2ac215e4f6a18ce349319edf9073ece0ca7bc61eから
  tak-14-production-tooling-correction workspaceを作成成功し、正式runnerの調査へ進んだ。
  同Runの具体的次工程再開を確認、正式受入/全体完了ではない。監視mainによる製品編集・
  game build/test・新Run・原本削除なし。orca-cli/orchestration/Help impact skill使用。
  Help No impact: 今回は開発用候補保持先/consumer移行だけで、player InputAction/UiIntent、
  UI文言/成立条件/結果/save/assets/Help manifest/provider/consumer経路は変わらない。

### 2026-10-03 compact terminalの送信前拒否補正

- 09:55:43Z、同統括は統合検証成功後の固定review配送前で停止。`identifier`
  がcompact terminal UUIDを拒否し、dispatch record/attemptなし・mail drainedを確認。
- mainがterm_限定で原表記を維持してcompact小文字UUIDを許可する候補を作る。
  generic UUIDの契約は維持。専用復旧はexact pause、検証証拠、source、所有、
  Run、未dispatch・未attempt・mail排出を再照合し、履歴先行保存後に通常配車へ戻す。
- tooling回帰・Help No impact・primary storage・固定read-only review後のみ適用。
  常駐Driverの旧decoderを使わず、同統括の新turnからfresh helperがscheduler lease内で
  正規integration_stepを一度実行する。通常dispatch WAL/admissionを維持し未知結果は
  再試行しない。新turn内の具体的次操作を確認するまで復旧未完。
- 候補関連108テスト成功、追加7拒否/正常配車テスト成功、5ファイルRuff成功。
  固定read-only orca_entry_auditのP2（operator/native guard不足）をmainが補正し、
  exact-source再レビューapproved。active control/native readiness、同所有・loopを
  入口/履歴保存前/状態保存前/配車前に照合し、途中停止時はdispatchしない。
- 5ファイルを既存dirty差分保持のapply_patchでliveへ適用し、承認SHA一致を確認。
  helper SHA807421058039fd88b664f69a5de941a4b5aefe8f2b11f5bd039378a6c23d8f5e。
  Help No impact: compact terminalの開発用RPC検査とexact送信前review復旧だけで、
  player InputAction/UiIntent、UI文言/成立条件/結果、通常起動、save/assets、
  Help manifest/provider/coverage/consumer経路に変更なし。Help gate PASSは補助証拠。
  primary storage PASS（187batches/legacy0/672657526784bytes）、候補hold10305536bytes、
  owner main/consumer TAK-14 compact review recovery、実再開確認まで保持。
- 同保存sessionの10:17:05.914Z task_started/turn01a10144-742f-7db0-8229-ec40590ccfca、
  通知receipt47b0bee1-2879-4a1d-94c4-9b845fb1d27fを確認（input受付だけを根拠にしない）。
  統括が実専用helperを起動し、busyはlease取得前・状態不変、解放後は過去同id拒否で停止。
  現reviewはgeneration9、新subject SHA f1230f0607…、過去generation7/SHA7118edf10a…は
  ctx_2c335b8c6417の正式settlement/released/ACK済み。idはnative marker由来で
  世代を跨いで維持されるため、id単独一致を未送信の否定にしていた追加誤拒否を修正する。
  現世代WAL/attempt不在を維持し、旧世代のWAL/役割exit/閉鎖bridge/正式ACK/runtime
  をread-onlyで完全照合した場合だけ区別する。旧admissionのreplayや新効果guardの
  歴史読取名目での緩和はしない。補正も関連テスト/固定read-only review後のみ適用。

- generation区別の補正もfixed read-only review approved後にliveへ適用済み。
  helper SHA3154f53255542fe819ce3e9044a444517955ede76b3f1561bba8f3194a917831、
  test SHAf7d01ff5b964911eb663c00bd039e73faca1fbc1bf5b44d453a49f59eff93fb1。
  旧WAL/正式settlement/exit/ACK/runtime既知release pairの実read-only proof PASS、
  関連tooling/Ruff/Help No impact/primary storage PASS。
- 新修正版通知7e40892f-ab1d-4efe-8200-c65ca5efb0d7はpending v2 draft_parked。
  原epoch82に対して現在84、parkedTextSha消失、同target/processと表示digestは一致。
  しかし表示は1231charsのplaceholderで本文同一性は証明できない。same-ID retryは
  正しく拒否、legacy cancelはreceipt非NULLを拒否。原request/ledger/本文は変更せず、
  無条件Enter・本文再送・偽settlementを行っていない。最新実turnは10:17開始/
  10:20:56.561Z終了のままで、固定review再配車・実再開は未確認。
- UI repoで明示observed-current-draft取消の候補をmainが実装。原complete v2
  draft_parked・same caller/binding・非inflight・original process/target・現在composer
  を拘束し、別capability/明示boolean opt-inだけを受理。取消WAL→条件付きCtrl-C→
  empty readbackを再利用し、原結果unknown/原receipt不変/原本文再送禁止を維持。
  73関連テスト、3typecheck、変更範囲oxlint、RPC catalog PASS。fixed read-only
  orca_entry_auditが9pathのexact SHAをapproved、追加P1/P2なし。候補は未配備。
  app再起動でruntime-local generationが変わるとsourcebindingが拒否するため、
  この比較を外したり旧rowを書き換えて配備成功に読み替えない。
- 現在の本文はplaceholderから復元できず、観測下書きの取消は別の利用者入力を
  不可逆に失わせ得る。保存不能な現在下書きの破棄は既存の原本保持条件を満たすと
  断定できないため利用者判断が必要。配車修正は適用済みだが復旧完了を報告しない。
  UI候補もゲームのInputAction/UiIntent/UI文言/条件/結果/save/assets/Help内容には
  影響なし。primary storage PASS187batches/legacy0/672658554880bytes。原本削除なし。

- 利用者「そっちが入力したんだからそっちで判断して」により、上記追加判断待ちは
  不要と訂正。mainが自分の残存通知の観測取消を判断する。元通知の本文は再送しない。
  11:15Zに旧Orca本体1459518の終了、新本体3010870/runtime236ae95a…を確認。
  Driver2087516/provider2103719/Codex2103721、同session/terminal incarnation/Run generation2
  は維持。最初の取消は内部generationの再起動差で副作用前に拒否、成功と報告しない。
- mainがobserved cancellation v2を実装。historical source generationはimmutable binding
  hashの照合だけに使い、現generationを別保存/CAS照合。host input epochが元parked epoch
  より厳密に進んだ同incarnationだけを許可する。元Enterは元epochで拒否されるため、
  旧runtime不在や所有移譲を推測しない。現pane/target/write owner/composerのguardと
  条件付きCtrl-C/WAL/empty readbackを維持し、元receiptは本取消から書き換えない。
  105関連テスト＋最終2file50テスト/型node・CLI/変更範囲oxlint/RPC catalog PASS。
  固定read-only reviewerが最終9pathをapproved。artifact/live再開の確認は別途必要。
  Help実経路のNo impact: 開発用取消RPC/CLIとその検証のみで、ゲームInputAction/UiIntent、
  UI文言/成立条件/結果、通常起動/save/assets/Help経路は変更なし。primary Help gate PASS。
  primary storage PASS（187batches/legacy0）、candidateと比較入力は登録用途の間だけ保持。
- 11:30Z iteration-02（source/artifact固定read-only承認済み）へ通常checkpoint後に
  配備。旧app3010870の終了、新app3049512/runtime2d5830cc…を確認。既存Driver/
  provider/session/PTY/incarnation/Run gen2は維持。停止通知stdoutが残存composerへ
  混ざりcursorがfooterへ移動し、取消inspectは副作用前に拒否した。通常の表示再描画
  Ctrl-Lを一回だけ送り、現在composerを照合後、正式cancel-draft
  ca60adfa-1727-5885-804f-a9fca1fab976を一回実行。直後readbackはunverifiableだが
  後続のhost読取で同incarnation epoch86・text空・bracketedPaste trueを確認。
  取消record/originalOutcome unknown/元send receiptは保持し、取消・元本文は再送しない。
- 11:42:10.257Z 同保存sessionの新turn01a10192-5704-7252-8065-a8bf3d151233を
  確認。11:43:03.755Zに新turn内のlivehelper show/watch・current ticket/attempt/WAL
  の読取操作を実見。新Run・製品build/test・review再送なし。これは統括の照合再開であり、
  製品工程の再開や全体完了ではない。fresh loopはed0fe657…、inbox runtime_unavailable
  pause、mail未drain、current review attempt/WALなし。
- 残存停止原因はmainの本体再起動後もlive EnabledLaunchが旧runtime817a80b2…へ
  pinされていること。Driver threadはfailed。snapshotもiteration-01 runtime236ae95a…
  のままで現在runtimeの稼働証拠に採用しない。保存enableだけで権限を作らず、同Run/
  session/実process/正式settlement/ACKを保全するruntime引継ぎ経路を調査中。
  新turn/input受付だけをDriver・製品工程の復旧完了と報告しない。

- 11:50Z監視の継続で、現app/runtime/build、同pane/PTY/incarnation、既存Run gen2を
  最小限再照合。統括は11:42のread-only turnを終了してidle、Driver復旧・製品工程は未再開。
  元通知・取消・bootstrap・Run移行を再実行していない。
- mainがpost-providerの事前観測基盤を追加。実registryのschema2/ready/ACK、launcherの
  実flock、provider direct-child、PID namespace-init、実Codexのopen-rollout FDから同保存session、
  全4processのbirth/ancestryを拘束してpidfdを保持する。観測終了はraw child outcome/exit0/
  成功/新launch権限ではない。JSONからlive capabilityを再構成しない。
  固定read-only reviewで最終read後の一部process終了を見逃すP2を検出し修正。
  共通ExitObservation.require_liveは1件でもpositive exit（以前のpollで消費済みも含む）なら拒否。
  これはread-only基盤の承認だけであり、retirement/succession/新enableの承認ではない。
- exact fixed read-only approval:
  `orca_post_provider_observation.py` SHA38b2cba481792ad7fc8d827ba195e8691673e98dd4f26830b3aba80f16865121、
  `orca_process_exit_observer.py` SHAd703e6c01cda7b413a22ce5ba60a7f97957b2f9f983bf00b28f91d8af6a7b246、
  対応test SHAe1b8051f91115ddd9e71c1ee0d316e4f6cdff9e8729acdebf2dcdbcab51eec93/
  9b99cbb43ec28d54d7869a002dd691f6f9bd969ae2f532b82b8fe19ab98272c8。
  関連tooling72件＋recovery-enable8件PASS、Ruff/diff-check PASS。
  live抽象socketと競合した2試験はnetwork隔離で検証、persistent storageが必要なactivation fixtureは
  host側で検証。tmpfsをpersistent storageに読み替えず、失敗した混在環境の実行は成功扱いしない。
- 承認後の一回のread-only smokeで2087516/2103719/2103720/2103721を実事前拘束し、
  同session一致と「termination not positively observed」の拒否を確認。観測終了時にpidfdを閉じた。
  この短命観測を将来の終了証拠に流用しない。signal/retirement/registry変更はゼロ。
  Help実経路はNo impact: Python開発用process観測と試験のみで、game entrypoints/crates/Cargo、
  InputAction/UiIntent、player-facing条件/結果/UI文言/save/assets/Helpに変更・新到達経路なし。
  primary Help gate PASSは既存primary差分の確認であり、このscopeの要否は上記実経路から判定。
  primary storage PASS187batches/legacy0/675648077824bytes、原本削除なし。
- 復旧未完了。次の接続工程は (1) 同live observerを上位transaction内で事前保持、
  (2) fresh control/source/settlement/pane/Run id+gen2と正式retirement intentを拘束、
  (3) 同observerのpositive全process終了をraw成功exitと区別してappend-only記録、
  (4) 新runtimeの同pane/PTY/inc/Runを新launcherの実FD/birth/ancestryへ正式succession/enable、
  (5) 同保存sessionの通常起動と新turn内の具体的次工程を実見。上位経路を実装・検証・固定review
  する前に旧ownerをkillしない。既存enable/WAL/親succession/原receiptを保持する。

- 12:05Z監視の継続: current app3049512/runtime2d5830cc…/同packaged build、同pane/PTY/inc、
  同session/正式Run gen2を読取照合。統括screenは11:42の限定read-only turn終了後のidleを維持。
  Driver・製品工程は未再開。snapshotの旧runtimeも稼働証拠へ読み替えない。
- mainがcurrent-runtime binding primitiveを追加し、固定read-only review approved。
  exact SHA: `orca_runtime_succession_binding.py`
  f5654ad6d7bfda12632b402f003182f58a7c0554b7f397774a9f0821857bf2c9、
  test37dbc16147aea0a27b5dc5ac6cd5da194f5a9aca089468188cf278d982d24b9a。
  承認後の一回のread-only smokeで元create/transferの正式settlement、全Run row/gen2、
  current app birth/build、同paneを再照合した。これは保存checkpoint変更、enable、移行の再送、
  artifact全体承認、実行権限の発行ではない。
- post-provider partial lease primitiveもmainが追加。固定reviewで初案に3点のP2を検出:
  threadfailedでも旧Driver lifetime flockはprovider終了まで保持、旧launcherの終了保存は
  coordinator flockを必要とするため取得順序に循環待ち、promotion最終読取のphase-only drift。
  同じ旧ownerを安易にkillせず、上記は許可内のmain修正で解消した。
  修正後はui-coordinator/Driver/coordinatorをdeferredとして区別し、旧Driverの実FD/inode/device/
  owner FLOCKを観測・再照合する。partial holderは完全quiescence/停止admissionではない。
  同prearmed全4processのpositive終了後にだけ3lockを実取得、凍結loopの最終digestまで照合し、
  同FDのfull holderへone-wayで移行。保存receiptや別processからcapabilityを再構成しない。
- exact fixed read-only approval: `orca_post_provider_leases.py`
  66d209050942c8a72ed5919a2ab2fdc5d239fdf9c4192e5b20b1436b73a85f75、
  testbb8bc0e38af1d71464b72f22f01e021805f17b4e22cf80d7d44a969ec2355611。
  86関連tooling試験/Ruff/diff-check PASS。最初のcombined test指定には存在しない
  test_orca_recovery_restartを含めたため1 import errorでFAIL、その実行を成功扱いせず、
  存在する対象に訂正して上記86件PASSを確認した。
  Help実経路はNo impact: 新規developer-only Python観測/lease/testにゲーム側producer/consumerはなく、
  player InputAction/UiIntent・成立条件・結果・文言・save/assets/Helpの到達経路は不変。
  primary Help gate PASSの既存trailerはこのscopeの承認に流用しない。
  primary storage PASS187batches/legacy0/675648335872bytes。原本削除、signal、retirement、
  本文/取消/旧bootstrap/Run transfer再送、registry変更、新launchは今回ゼロ。
  承認後の一回のread-only smokeで同live observer/旧Driver実flockとpartial lease取得を確認し、
  全positive終了のないpromotionは拒否した。context終了時に全partial FD/pidfdを閉じており、
  この短命観測を将来の終了や停止許可に流用しない。
- 上位接続は未完: 旧Driver/Brokerの実処理停止確認とfresh source/control/external settlement、
  正式retirement intent/WALとlive pidfdの全終了証拠、append-only runtime succession、
  新launcherの実FD/birth/ancestryを使うenable、同保存session通常起動が必要。
  primitiveの承認やpartial取得だけで旧owner停止を許可しない。実際の新turnと具体的な次工程操作を
  見るまで製品工程の再開とは報告しない。追加の利用者再承認待ちは作らない。

- 12:28Z監視: app3049512/runtime2d5830cc…/build0515a76b…/同Run gen2は変化なし。
  Driverのcanonical読取はpid2087516/phase failed、旧runtimeに対する失効guardを保持。
  snapshotは引き続きruntime236ae95a…で現稼働証拠ではない。製品工程は未再開。
- 上位接続案の固定read-only reviewで重要なprocess topology条件を確定した。
  外部main observerが持つpidfd/full leaseを、元PTY内の実新launcherへ保存JSONだけで
  引き渡すことは不可。新launcherは元guarded-create shellの実子孫、SO_PEERCREDで認証した
  pid/birth、同pane/PTY/inc、実FD所有を持つ必要がある。ORCA_*注入/origin override/既存shellを
  launcher扱いする回避は禁止。旧owner停止前に、この一回限りのlive FD/authority handoffを
  connected upper transactionへ実装・検証・固定reviewする。
  current-enable resolverも原enableのoverwriteではなく、正式append-only chainの親hash/retirement/
  activation/runtime/Run世代を検証し、fork/欠損/不正headでは旧enableへfallbackしない。
  原registry/ACKを先行保全しexact CASで新startingへ進む。unknown mail/原send/取消unverifiableは
  別残件として保持し、runtime successionを理由にACK/drained/pause解除を捏造しない。
- mainがlive handoff用のFD transport primitiveを追加し、固定read-only review approved。
  bounded Unix seqpacket/SO_PEERCRED、SCM_RIGHTS/CLOEXEC、exact count/protocol/truncation照合。
  syscall前のone-shot消費とsocket claim、送受方向shutdownでunknownの再送を防ぐ。
  受信原FDはquarantine私有とし、callerによるclose/adoptは禁止。明示dupで別のcaller-owned FDを
  作ることはできるが、それ自体はOwnedRecoveryLeases/起動権限ではない。
  固定reviewが検出した並列consume/再構築と途中close失敗時の後続FD leakをmain修正し、
  11 focused＋計97 related tooling試験、Ruff/diff-check PASS。
  exact SHA: `orca_recovery_fd_transport.py`
  0f5bf6e53ca290b61d156a49443766f8a26c10f4938fc8655607019d347583a4、
  testa4806e92fec62f8066925c08ba405aed892030963b03e0a7d31800df12fac6e6。
  承認はtransportのみ。新connection/事前dupを含むoperationの再実行禁止、実peer ownership/birth/
  ancestry、原authorityの生存引継ぎ、FD採用、retirement/new enable/launchは未接続。
  実handoff/旧ownerへのsignal/registry変更/新launchはゼロ。
  Help実経路はNo impact: developer-only IPC/FD受渡しhelperと試験のみで、ゲームのinput/consumer/
  UI/save/assets/条件/結果/文言は不変。player Helpへの到達経路を追加しない。
  primary storage PASS187batches/legacy0/675648512000bytes、原本削除なし。
- loop digestの比較対象を再確認した。保存full digestは
  ac330714ee981da41f3de6c400946856933e7867113d9c1e5a6dadebca318c6f、
  updated_at_msを除外するinspection digestは既報の
  ed0fe657e5bc6eb40c080da83bde6117ac981b730c781daca894d74971ab5d2d。
  両者の差は新しい状態変化ではない。canonical loadは読取のみ、保存mtimeは11:15:42Z。
  pause理由はinbox runtime_unavailable、integration理由はRefused: invalid identityのまま。
  未ACKのcheckを再実行せず、原台帳を保持した。

- 12:47Z監視: current app/runtime/build、統括pane/inc、Run gen2、paused loop、
  failed Driver2087516と旧runtime snapshotは不変。旧通知・取消・Run transferは再実行していない。
  live handoffの上位接続に必要なkernel条件を実別processの試験で確定した。
  inherited socketpairではcreator PIDが見えるため、実connect/acceptでnew launcherの
  SO_PEERCREDを取得する。sender FD close完了のbarrier後にreceiverのSCM_RIGHTS duplicateが
  同inodeのexclusive flockを維持し、receiverの最終close後にのみ第三者取得可能となる。
  同OFDでLOCK_EXしてもfdinfoのkernel owner PIDはsenderのままである。
  現行launcher_lockのPID一致条件を単純に緩和せず、正式一回handoffの実peer birth/ancestry/
  原FDとkernel ownerの生存継承をappend-only successionに結ぶ必要がある。
  test-only差分の固定read-only review approved、exact SHA
  `test_orca_recovery_fd_transport.py`:
  2d9238c9cf1cb42638820c328115cc9054742febab3b0dba8750550f7ae5e984。
  production transportは既承認0f5bf6e5…から不変。新fixtureの初回はfdinfo列位置の誤りで
  1 error、main訂正後12 focused PASS。関連試験の初回は既存enable fixtureがlive abstract
  socketと衝突して2 errors、成功扱いせずlive socketを触らずnamespace分離で8 PASS。
  その他105 related PASS、計113 PASS、Ruff PASS。
  Help実経路No impact: developer-only IPC/FD toolingのtestだけでplayer-visible producer/
  consumer、input/UI/save/assets/成立条件/結果/文言を変更しない。
  test-owned subprocess/socket/temporary FDのみcleanupし、統括へのsignal/registry変更/
  live handoff/new launchはゼロ。製品工程は未再開で追加利用者判断は不要。

- 12:53Z接続修正: transportのEOFはsender全FD close済みを示さないことを固定reviewで確定。
  mainが同transportへ別RELEASED barrierを実装した。FDsend結果不明でも一度relinquishを実行し、
  sendとrelinquish双方成功時だけ別packetを送る。receiverは明示require_relinquishedで
  EOF/不正packet/cleanup失敗を拒否し、release packetに紛れたFDもcleanupする。
  `OwnedRecoveryLeases.handoff_once`へ接続し、全actual canonical OFD/同loop確認後sender holderを
  一方向失効、全aliasを途中close失敗でも処理する。unknownや失敗後の同holder再利用は禁止。
  固定reviewの二重thread/別socket競合P2をmain修正し、handoffとretain_launch_leaseの
  有効確認→失効を共通transition mutexで直列化した。
  固定read-only review approved exact SHA:
  transport cf37bd0c80a28952589d8af9f261862e4f3cf0fe4dcd8c255c72430d31f6bb52、
  transport test 5e36689c37725f3e0625092e86d6c264597e8b0a15f81d72017e543221e70d0a、
  leases 4c43d06398833271654e28cab81dd6ab363a658ca729f5f49a77054e0947daea、
  leases test 74d147591a4c32f2805684fa01c5a325b21b5db4fbbdbe858c0955a1228136c4。
  31 focused、113 related＋net namespace分離enable8件、計121 related PASS、Ruff/diff-check PASS。
  初回barrier試験はclose済みFD番号のrecv後再利用を誤判定して1 FAIL、mainがrecv前のclose確認へ
  訂正し再検証した。成功扱いは訂正後の実行のみ。
  fresh Run照合は同runtime/terminal/id/gen2。read-only外部監査は通常mutation guardで拒否したため
  guardを変更せず原本を保持した。canonical origin/external ledger/native pendingを独立読取し、
  external operationsはconfirmedのみ、native_pending=falseを確認。これはeffect admissionではない。
  上位はformal retirement intent→exact prearmed provider stop→全4process positive exit→full lease→
  元paneの実新launcher authentication→receiver actual OFD/authority→append-only succession/current-enable→
  exact registry CAS→normal ui.launch(session,request,prepared_state,launch_lease)が未接続。
  callback成功やtransport/holder承認だけでこの上位許可を作らない。
  Help実経路No impact: developer-only FD/lease内部経路のみでゲームproducer/consumer、UI/input/save/
  assets/条件/結果/文言は不変。primary Help gate PASSは既存trailerを選んだため今回scopeの承認に流用しない。
  primary storage PASS187batches/legacy0/675648880640bytes、原本削除なし。
  live統括signal/retirement/receiver mint/registry変更/new launchは未実行、製品工程は未再開。

- 13:07Z監視継続: 同runtime/terminal/incarnationでpaneはidle、製品工程の新turnはない。
  mainがFD受信からfull holderへの`receive_owned`を接続した。必須RELEASED確認後、
  全canonical OFD・loop全digestを照合し、quarantine原FDを閉じてから同OFDの所有を返す。
  quarantine aliasを残したままlaunch downgradeしてcontroller lockを保持し続ける経路を防止。
  drift/誤順FDは全closeしyieldしない。これはpeer/birth/retirement/enableの権限ではない。
  固定read-only review approved exact SHA:
  leases 158c4bf4a2e8f6443f3d1cd70c84d94739ccd84c7275a819a61a9d7ddb74d010、
  leases test ed2903bc7a9c6cd4396e4e33499de0edd916ebed59bb29fdcedfb9ceee2d6d5c。
  34 focused、116 related＋net namespace分離enable8件、計124 PASS、Ruff/diff-check PASS。
  Help skillとhelp-screenの実経路再照合はNo impact: 開発側FD所有のみでゲーム入力/表示/
  save/assets/成立条件/結果は不変。primary storage PASS187batches/legacy0/675649040384bytes。
  原本削除・live signal・retirement・registry変更・new launchは行っていない。
  上位formal retirement/current-enable/registry CAS/ui.launch接続は依然未完であり、
  受信primitiveの承認を実際の復旧や全体完了として報告しない。

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-29 | Codex | 実績と現行実装から対応済み・残件・未確定を分離し、S1〜S9とH0〜H5を作成 |
| 2026-09-29 | Codex | H1-a canonical CLI入口と拒否系を実装。候補適用・未配備・残APIを分離して記録 |
| 2026-09-29 | Codex | H1-bの保存前preflightと変化・拒否系を候補実装。host接続/版切替の残件を明記 |
| 2026-09-29 | Codex | H1-c登録transactionと同ID照会を候補実装。227件の関連試験と未配備範囲を記録 |
| 2026-09-30 | Codex | H1-dを同sourceで検証・同統括へ反映。H2-a通知証拠を候補実装し未配備範囲を明記 |
