# Orca UI・受付・終了ライフサイクル統合計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `orca-ui-lifecycle-plan-2026-09-23` |
| ステータス | In Progress — 固定受付、実案件の実装・固定review・統合、承認後role tab整理、通常desktop起動まで受入。新規案件の無介入cold-start計測と稼働中agentの中断・再起動復旧が残る |
| 作成日 / 最終更新日 | 2026-09-23 / 2026-09-24 |
| 作成者 | Codex |
| 関連提案 | [Orca運用素案](../proposals/orca-parallel-development-proposal-2026-09-20.md) |
| 関連Issue/PR | 専用実経路試験`TAK-9`を作成。旧promptの誤handoffで生じた`TAK-10`は試験残骸として照合対象。TAK-5/6/8は既存状態の照合対象であり、本計画の実装依頼に転用しない。PRなし |
| 関連計画 | [受付・分離開発](orca-parallel-development-plan-2026-09-20.md)、[Git基点レビュー反復](orca-git-review-loop-plan-2026-09-22.md) |

## 1. 目的

「タブが重複しない」だけでなく、利用者が **依頼先・進行状況・次の操作・終了後の所在** をOrcaの画面から判断できるようにする。
受付から実装、差戻し、利用者への提示、再修正、最終終了までを一つのライフサイクルとして設計する。

- 新規依頼も既存案件の再開も、固定の「受付・統括」から日本語で依頼できる。内部ID・端末コマンド・課題の付替えを利用者に要求しない。
- 案件を選ぶと統括・A・B・レビューの状態と移動先が一か所に見え、各担当の実画面へ移動できる。
- 使っていない担当の空shellをagentに見せず、終了案件・保守環境・検証環境を実行中案件と区別する。
- 終了操作は成果とフィードバック環境を失わず、再試行しても二重終了・二重作成・別作業の停止を起こさない。
- 名前を変えた画面やmock testだけで完了とせず、実Orcaで入口から終了・再開まで受け入れる。

## 2. スコープ

### 対象（In Scope）

- Orca内の受付導線、案件・役割の表示、親子作業場の移動、状態の正本と表示同期。
- 固定reviewer 1 session、Codex A、単純な局所作業専用Cursor CLI B、統括の既存制御の再利用。
- 中断・完了報告・フィードバック待ち・最終終了・タブclose・作業場撤去を分離した処理。
- 既存6作業場の分類と移行、再起動・手動タブclose・結果不明からの復旧。
- 実装候補と稼働版の配備区別、UIを含む受入、運用文書の同期。

### 非対象（Out of Scope）

- ゲーム機能変更、ゲーム起動・描画/GPU・Rustの全群検証。
- GitHub/Linearの実行正本化、新たなWebhook基盤、一般用途の自作ターミナル/受付Webアプリ。
- 既存タブの無断終了、作業場削除、push/PR作成。
- 稼働版Orcaの無断置換。本体拡張は承認済みだが、別ビルドで受入後に配備を判断する。

## 3. 現状とギャップ

2026-09-23にOrca 1.4.205のCLI help、当時の作業場一覧、candidate実装を読取り確認した。

| 現状 | 埋めるギャップ |
| --- | --- |
| 当初のsidebarにはprimary、TAK-8、TAK-6、Orca基盤、検証環境2件が並んでいた | 依頼先と用途が不明。branch/pathを知らなくても区別できる表示が必要。TAK-8/6とR01〜R14環境は後に利用者が撤去済み |
| TAK-8は`workspaceStatus=completed`でもruntime側は`active` | 作業完了とprocess/terminal存在を分離する。緑点をagent稼働の根拠にしない |
| `Orca運用基盤`には終了済み統括のshellが残る | 保守用基盤を受付に見せない。終了したagentの履歴と生きたagentを区別する |
| 同じ依頼・checkout・役割のタブ再利用は実装済み | checkoutをまたぐreviewや統括移動まで含む「案件内で一つの役割の入口」は未実装 |
| `retire`は所有確認済みidle shellの出力保全とpane closeに限定 | 実行中の安全停止、終了全体のjournal、close結果不明の再照合、案件整理は別途必要 |
| 終了観測エラー後のreview承認を台帳で復旧できる | 古いエラーのscrollbackと最新承認状態の不一致を、最新要約で説明する必要がある |
| 3回の無害なshell起動で1タブを確認済み | 実LLMによる差戻し・終了・再開とUI視認性の受入の代替にはならない |

### 確認済みの機能と未確認の機能

- `worktree set`でdisplay name、comment、workspace status、親子関係を変更できる。
- `terminal list --include-visual-layouts`で実tab/paneを照合でき、rename/switch/closeがある。
- `terminal close`はpane単位、`--tab`はタブ単位。`--all`はprocessだけでなくlayout/resume recordsも除去するため、休止用に使わない。
- helpは再開用にWorkspace Sleepを案内するが、sleepの自動操作契約・UI復帰動作は未確認。
- `worktree rm`は表示を隠す操作ではない。Git worktreeを撤去し、条件によってlocal branchも削除する。archive hookは`--run-hooks`を指定しないと実行されない。
- 固定入口のpin、非破壊の非表示/分類、親作業場から他作業場のroleへ移る固定UI、×ボタンへの終了確認hookは未確認。存在する前提で実装しない。

### M0追加調査（実装依頼後、2026-09-23）

導入済み1.4.205の`resources/app.asar.unpacked/out/shared/`を一次情報として確認した。
既存のruntime CLIと、画面内から呼べる公開plugin APIは別物である。

| 機能 | 調査結果 | 必要な対応 |
| --- | --- | --- |
| project既定terminal | `orca-yaml.js`はtitle/command/colorを扱う。任意の役割button定義はない | 既定tabだけで案件navigationを実現したと扱わない |
| pluginパネル | `plugins/plugin-manifest.js`は右sidebarのsandboxed HTML panelとcommand contributionを定義 | 画面を追加する拡張点は存在。ただし下記の操作APIが不足 |
| 案件・担当の読取り | `plugins/plugin-host-api.js`の`workspace.readContext`はfocused worktreeのbranch/displayName/terminal IDだけ | 全案件一覧・role状態・所有確認済み移動先の型付きAPIが必要 |
| 担当tabへ移動、起動、終了、休止 | 同ファイルの公開APIには存在しない。panel actionはreadContext/sendText/通知の3つ | 本体側に限定されたnavigation/lifecycle APIを追加するか、必須UI要件を変更する |
| command alias | `plugins/plugin-command-actions.js`は履歴前後・sidebar切替・rename・board・Tasks等の固定集合 | 任意のroleへの移動やclose controller呼出しには使えない |
| panelからworkerへの独自接続 | `plugins/plugin-panel-bridge.js`は上記公開APIだけを許可 | 私設RPCやterminalへのshell注入で境界を迂回しない |
| Sleep一覧表示 | command aliasに`sidebar.sleepingWorkspaces.toggle`が存在 | Sleep実行・再開の動作確認は別途必要。toggleを休止操作と混同しない |
| 実画面検査 | `computer capabilities`はscreenshot=false、`list-apps`にOrcaなし、`get-app-state --app Orca`は`app_not_found` | UI受入は未実施。画面操作環境の回復も必要 |

**M0の判断**: 標準CLIだけのbackend実装では、計画した「案件画面から役割へ移動・終了」を満たせない。
公開plugin APIにも必要な操作がないため、Orca本体UI/API拡張を追加範囲として提示した。
本体拡張を許可された場合は、稼働アプリを置換せず別ビルド・隔離profileで検証する案とする。
標準機能内に限定する場合は、固定role button等の受入条件を明示的に再合意する。
2026-09-23に利用者が本体UI/API拡張と別ビルド検証を承認した。権限待ちは解消した。
公式repositoryの導入版tag `v1.4.205`（`11aba8bdc5e492d3ba01fc7fe333495ace74128f`）を
`/home/satotakumi/tools/orca-ui-lifecycle`へ分離取得し、本体UIと型付きAPIを同じ版で拡張する。
稼働版・設定・credential・登録済み作業場はコピー/変更せず、隔離profileと試験fixtureを使う。

## 4. 実装方針（高レベル）

### 4.1 固定入口と案件画面

目標は **「受付・統括」を開く → 日本語で依頼 → 対象案件が開く**。Tasks/Linearは一覧・検索・履歴として残すが、利用者による課題作成を前提条件にしない。
これは既存計画の「Tasksから課題/worktreeを作ることが唯一の入口」という制約の改訂案であり、現時点の実装済み仕様ではない。

- 受付は特定の試験課題や製品branchに従属させない。専用の非編集コンテキストを1つ登録し、起動・再開操作をOrca内から行う。
- **固定するのは入口であり、LLMの無期限常駐ではない。** 停止中は「受付・停止／開始」、起動時は「起動中」、受信可能になってから「受付中」と表示する。
- 最小の状態・移動UIはagent停止中も利用可能にする。shellがあるだけで受付可能としない。
- 実装を指示された時だけ、既存案件照合→課題/worktreeの作成または再利用→文脈移送→案件統括のacknowledge→表示切替を行う。相談だけでは作成しない。
- 受付統括と案件統括は所有権を明示的に引き渡す。複数の統括LLMを常駐させず、既存のhost資源制御・単一ownerを維持する。
- 引継ぎ前の送信先、未処理メッセージ、移送先、受領を記録し、失敗時は入口に理由と再開操作を残す。起動失敗を空shellで終わらせない。
- 案件画面に「目的／現在の工程／待ち理由／次の操作／統括・A・B・レビューの状態と移動先」を置く。A/Bの作業checkoutは分離のまま、閲覧を一か所から辿れるようにする。
- B未使用やreview前は状態項目だけ表示する。4担当を見せるためだけに4つの空terminalを作らない。
- 利用者が担当を開く操作だけでfocusを移す。配車・heartbeat・バックグラウンドの状態更新で見ている画面を奪わない。案件への初回引継ぎ時は移動を明示する。

UI実現方法はM0で固定する。名前変更と文書リンクだけでは「役割へ移動できるUI」を満たしたことにしない。

### 4.2 表示と状態の正本

| 層 | 表示するもの | 正本・禁止する読み替え |
| --- | --- | --- |
| 案件 | 受付済／作業中／レビュー中／確認待ち／休止／終了処理中／終了／要確認 | 既存loopに必要なlifecycleを追加。Linear Doneだけで終了しない |
| 役割 | 未割当／起動中／実行中／回答待ち／再開待ち／終了／要確認 | Run・Dispatch・session・process・receiptを照合。exit codeと承認結果は別表示 |
| 作業場 | 案件用／受付／保守／検証保持、保持理由 | 所有台帳・storage consumer。UIの色やbranch名から推測しない |

- 短い用途・状態・目的を名前の前方へ置く。例「確認待ち・保存領域復旧」「保守・Orca基盤」「検証保持・壁と扉」。branchやUUIDは詳細へ置く。
- 同名案件は短いissue識別子等で区別するが、利用者へ入力は要求しない。未確認状態は必ず文字でも示し、色だけに依存しない。
- 内部判定の正本は既存台帳とし、UI専用の独立実行状態機械を作らない。表示はversion付きsnapshotから投影し、更新失敗なら「表示未同期・最終確認時刻」を示す。
- 古い世代の完了eventで新しい実行中tabを「終了」にしない。runtime、request、Run、role、generation、terminal incarnationを結び付ける。
- UIに反映できない間も安全な終了照合は続けるが、「表示まで完了」と報告しない。継続実行に必要な状態が不明なら新規配車を止める。
- 要確認の案件は既定表示から隠さない。未知状態で黙って別terminalを作ることも、利用者にUUIDや復旧commandを要求することもしない。

### 4.3 タブと固定reviewer

- logicalな役割の入口は案件ごとに統括/A/B/review各1つ。実terminalは担当が起動する時だけ作成・再利用する。
- 同じcheckoutの再試行・差戻しでは既存`orca_role_tabs.py`の所有確認と世代付き再利用を維持する。
- reviewerは同じ固定sessionを順次使い、同時に起動しない。A→B→統合結果と対象checkoutが変わる場合は、旧providerの終了と新しい読取scopeを確認して移送する。
- UI上のreview入口は同じものを維持する。旧checkout側の履歴は最新実行と区別し、結果を保全後、所有する不要paneを整理する。同一terminalで安全にmount/cwdを変更できるとは仮定しない。
- タブ名ではなくhandle/incarnation・実process・所有記録で操作対象を確定する。未知のsplitや手動terminalは保全する。

### 4.4 「閉じる」を分ける

| 操作・契機 | 実行する処理 | 実行しない処理 |
| --- | --- | --- |
| 表示を離れる／タブを隠す | UI切替だけ。非破壊操作が提供される場合のみ利用 | agent停止、課題Done、成果削除 |
| Orcaの×でterminalを閉じる | 切断を検出し「要確認」。process・未決操作を照合する | 正常終了と推定、同じ依頼の自動再送 |
| 中断・後で再開 | 新規配車を止め、安全停止/checkpoint、sessionと同一candidate/cacheを保持 | 全terminalのresume記録削除、作業場撤去 |
| 実装・review完了報告 | 結果提示後は「確認待ち」。不要な実行processを止め、修正用環境を保持 | 無応答を承認扱い、cacheや唯一の会話の削除 |
| 最終承認／明示終了 | 下記のclose処理。終了要約から再開/新規依頼へ進める | 未許可のpush/merge、他案件のconsumer解除 |
| 作業場・branch撤去 | 所有、成果保全、consumer=0、process=0を確認し、許可範囲の専用環境だけ撤去 | UI整理を理由としたforce削除、primaryや他案件の削除 |

**close処理の順序**:

1. 終了対象・理由・権限・未決処理・残す成果を確定し、永続close intentを保存。新規配車を禁止する。
2. 処理中の通知を照合し、bridge失効・in-flight要求の排出を既存の順序規約に従って行う。Task outcomeと実process終了を別々に記録する。
3. 所有するprovider/controllerの安全停止とwaitを確認。期限超過は「終了要確認」で停止し、未知のprocessをkillしない。
4. 差分、未追跡/ignoredの原本、独自commit、review結果、再開contextを照合。採用成果と最終要約を正本へ集約する。全scrollbackの永久保存を条件にしない。
5. 所有する不要paneを閉じ、receipt・完全なterminal一覧・実process・layoutを再読する。部分失敗では未確定対象だけを照合し、bulk closeで迂回しない。
6. 仕事のconsumerだけを解除。残るconsumerがある環境は用途・owner・bytes・次工程・release_whenを残す。最終利用が終わった専用環境だけ、成果保全確認後に撤去する。
7. 案件を通常の進行一覧から終了表示へ移し、要約と戻り先を確認してclose完了とする。案件統括は最後に終了し、独立したhost側finalizerがその終了・UI反映を確認する。自分の終了を自分で成功記録しない。

各段階にoperation ID、対象identity、期待状態、receiptを保存する。応答断後は同じintentでread-backし、確定済み段階を繰り返さない。
未知の終了は新規起動より先に解決する。Orca停止中は完了としない。UI側の×やアプリ終了をinterceptできなくても、次回起動の照合で回復できる設計にする。
利用者向け操作は「中断」「再開」「終了」に絞り、途中成果を失う可能性がある場合だけ、対象を人が読める名前で確認する。

### 4.5 実装済み資産と配備

- 再利用: `orca_role_tabs.py`、`orca_ui_coordinator.py`、`orca_dispatch.py`、`orca_review_loop.py`、Git checkpoint/統合、固定reviewer、host lock、既存Linear adapter、primary validation台帳。
- 追加: 状態の読取・UI投影、案件単位のrole navigation、再開可能なclose journal/finalizer、固定受付の起動/所有権移送。新しい配車方式や並列編集の抜け道を作らない。
- 実装は同目的の基盤branch `iaammssssstupiddddd-commits/orca-parallel-development`を再利用する案。開始時にHEAD・dirty・現行ルールを再確認し、主担当が編集する。
- 現在のcandidate参照は`3181baba`、Orca本体cloneは`9fb9f672`、primary文書の基点は`8a9d7555`。文書正本はprimaryのみ。
- base ref変更は新規worktreeだけに効く。既存process・TAK-6・凍結candidateを更新済みと扱わない。launcher版とstate schemaを記録し、idle確認・移行preview・backup・再照合を経て切り替える。
- 公開/PRによるCIはその時点の許可範囲を確認する。本計画は公開権限を追加しない。
- Bevy API変更なし。レビュー待ちのゲーム環境へOrca codeをコピーしない。

## 5. マイルストーン

### M0: UI実現方式と状態契約を確定

- 固定入口、起動/再開、案件roleへの移動、非破壊の休止/一覧整理、×/アプリ終了の実仕様を、導入版の一次情報・実画面で確認する。
- 「既存UI/APIで可能」「repo adapterが必要」「Orca本体の対応が必要」の機能表と画面ラフを作る。状態遷移・ownership・終了条件を先に固定する。
- [x] 入口→案件→各役割→戻る方式を、本体の固定サイドバーパネル＋型付きRPCへ決定した。controllerは既存正本をprivate snapshotへ投影し、UIの操作はdurable intentへ記録する。詳細は[本体拡張仕様](../development-infra/orca-ui-extension.md)。運用受入はM1以降で別途確認する。
- [x] 必須UIが既存版で成立しなければ、本体変更等の追加範囲を提示し、実装へ進む前に方向を確認した。2026-09-23に本体拡張・別ビルド検証を承認済み。
- 対象: 本計画、`docs/orca-quickstart.md`、`docs/development-infra/orca-development.md`。M0が未完ならM1以降を着手しない。

### M1: 固定受付と一貫した状態・移動先

- 既存入口/引継ぎを拡張し、役割navigationと状態snapshotを接続する。Orca UI adapterの置き場所はM0で決める。
- 対象: `orca.yaml`、`scripts/orca_ui_coordinator.py`、状態/表示adapter、対応tests。
- [x] 新規依頼・相談・既存再開を区別し、受付から内部ID入力なしに対象案件へ到達できる。
- [x] 未起動・空shell・実行中・待機・unknownを実状態どおり表示し、案件内から各担当へ2操作以内で移動できる。
- [x] A/Bは分離checkout、reviewerは固定1 session、資源上限と編集境界を維持する。

2026-09-23の部分実装: Orca本体のopt-in sidecar起動、固定受付snapshot、UI要求のdurable receipt、
既存受付台帳へのUUID重複防止、読取専用相談の起動intentとunknown時の自動再送禁止を追加した。
Python単体テスト34件は通過。node/web型チェックと変更行品質ゲートも通過した。
隔離した非表示ElectronのUI/API 4シナリオと、backendを接続した
画面送信→隔離受付台帳1件→同一UUIDのreceipt／相談起動intentの1シナリオは通過した。
実provider相談・可視統括tab・課題/worktree配車は未受入である。
追加で「相談」と別の「実装を依頼」actionを設け、後者だけが受付receiptから
内部Linear課題1件・分離worktree1件を段階journal付きで作る候補経路を接続した。
既存のLinear連携を利用者へ露出させず、linked issueの既定タブから可視統括が登録されれば
4役割パネルの統括ボタンへ正確なterminal identityを投影する。結果不明の外部作成は自動再送しない。
Pythonの関連単体テスト34件とOrca node/web型チェックは通過したが、実Orcaへの外部作成・起動、
実provider、終了処理はまだ受け入れていない。
候補は基盤branch `64748035`、別Orca本体clone `6cc02cdf`にローカルcommit済み。
push・PR・稼働版アプリの置換は行っていない。
保存領域ガードが欠損を検出した3作業場について、利用者が意図的な撤去と確認した。
disk・Git worktree・Orca一覧・実行processを照合し、残るGit commit/branchを保全したまま、
各holdの当該consumerだけを`validation release`で閉じた。空pathの再作成はしていない。
`validation reconcile`と`validation check`は通過した。Linear課題やbranchは終了・削除していない。
`attempt-8`はE2E用ビルド設定の不足で無効として台帳に記録・成果削除済み。
再ビルド後の`attempt-9`をUI/API受入とする。`attempt-12`は後続の試験で置き換え、
保持consumerを解除して証跡を撤去した。`attempt-13`〜`15`は順にビルド設定不足・
非同期台帳読取・空directoryの期待値誤りで無効とし、個別にseal/finalizeして試験成果を撤去した。
最終sourceの`attempt-16`では隔離Electronの相談・実装依頼2件と独立証跡検証器が通過した。
これらは試験modeの受付台帳とfixtureを使うため、実Linear/worktree/provider配車の受入ではない。
route journalに`terminal_starting`を追加し、子受付import後に可視統括terminalを背景起動する経路を
`7932a355`へcommitした。外部作成の応答不明時に二重起動せず、既存tabの所有を照合して再利用する。
関連Pythonテスト71件とRuffは通過。専用`TAK-9`でLinear課題1件・分離worktree1件・可視統括tab1件、
統括のacknowledge、routeの`ready`、再実行後もtab1件を実Orcaで確認した。
ただしこれは既存稼働版のCLI経路による受入であり、未配備の固定パネルから操作した証拠ではない。
実装A/B・固定review・終了の一巡も未受入である。試験状態とworktreeは受入時にprimary検証台帳へ保持登録し、後述のclose受入後に専用worktreeだけ解除・撤去した。
この実経路で旧promptが試験課題を実装課題へ誤って再handoffし、`TAK-10`と余分なSetup tabを作った。
`d5992437`で受付から自動作成した専用課題に由来記録を付け、同じ課題からの再handoffを
promptと実行gateの両方で拒否した。handoff作業場作成は`--setup skip`に変更し、
`e68c22e3`で作業場作成時の`--activate`も外して、統括tab自身だけを前面起動する。
`TAK-10`の2 shellは実processと成果を照合し、Setup tabを保全receipt付きで閉じたところ
Orca一覧から2 tabとも消えた。意図した片方以外も消えた理由は未解明のため、close受入に算入しない。
`TAK-10`は誤作成の試験課題として理由をコメントしCanceledへ移した。clean・固有成果なし・
terminal一覧とprocessを照合後、当該hold consumerを解除して専用worktreeを非forceで撤去した。
保存検証は通過し、計測上の割当量は29,089,792 bytes減少した。

### M2: 役割の再利用・中断・終了の一本化

- 対象: `scripts/orca_role_tabs.py`、`scripts/orca_review_loop.py`、`scripts/orca_dispatch.py`、close journal/finalizer、対応tests。
- [x] 同一checkoutの3回の差戻しでタブが増えず、reviewのcheckout移動でも現在のrole入口が増えない。
- [ ] close各段階の中断・応答断・再実行・世代交代をテストし、別作業へ影響しない。
- [ ] 長いscrollback、手動split、terminal先行消失、process残存、Orca再起動を扱い、未知状態は理由付きで停止する。
- [x] 確認待ち/休止から同じsession・candidate・targetで修正を再開できる。終了はprocess・資源・UIまでread-backする。

`d41887aa`では、実装経路が`ready`で統括terminalの所有・idleを確認できる場合だけ中断し、
同じ作業場を保持したまま再開する限定経路を追加した。終了は作業場clean・担当/レビューtabなし・
統括1tabを事前条件とし、scrollbackを保存して当該paneだけ閉じる。busy、dirty、複数tab、
作成結果不明なら拒否または要確認にする。UIは`9fb9f672`で操作IDの待機表示をworkflow revision
更新時に解除する。Python関連76件、Orca本体web型チェック、隔離Electronの終了待機1件は通過。
これはworker/reviewerを経た案件、稼働中agentの安全停止、Orca再起動後の復旧、
実固定パネルからの閉鎖を受け入れたことを意味しない。
`0725f8d6`は長いTUI出力をcursorで保全する処理と、終了結果不明時に同じpaneを
再度閉じずreceipt・全terminalを照合する回復経路を追加した。実`TAK-9`ではCodexの
`/exit`後、最初のpreview上限と空inventoryの扱いで2回fail-closedとなった。
修正後は保護された再照合でclose receiptとtab 0件を確認し、案件snapshotが`closed`へ進んだ。
試験課題は結果をコメントしてDoneへ移し、clean・固有成果なし・tab 0件の専用worktreeを
hold consumer解除後に非force撤去した（計測上29,351,936 bytes減）。試験状態台帳は
引き続き実経路受入の証跡consumerとして保持する。これはCLI/controllerの受入であり、
本体パネルからの最終終了とworker/reviewer実行後の終了受入ではない。
`66cd4514`は固定パネルの一覧をパネル所有receiptのある案件だけに限定し、
手動取込や旧相談の混入を防ぐ。監督loopが存在する案件では実担当terminalのhandle・
worktree・host・incarnationを読取照合してA/B/reviewerを投影する。存在しない担当の
空tabは作らず、照合不能は要確認とする。実A/B/reviewerのUI移動はまだ受入前。
`3bf2a999`はroute・統括・loop・lifecycle台帳の変化をworkflow revisionへ反映し、
古い版の操作はrejected receiptとして消費する。これにより画面の再表示後に同じ終了を
別operation IDで重ねてもcontroller全体が停止しない。実Orcaでの再起動・競合受入は未実施。
`56accc4e`は監督loopの最終承認、attempt release、最終review、通知排出と全対象worktreeの
clean状態をclose前に照合し、A/B/reviewerの登録tabを順に保全・閉鎖してから統括tabを閉じる。
対象identityを永続journalに固定し、部分終了後の再照合ではclose-returned receiptを読んで
同一paneを再度閉じない。監督loopが未解決なら中断・終了actionを画面から隠し、統括だけを
休止させない。Python関連36件とtooling 620件は通過。実agentのA/B/reviewを経たclose、
partially closed案件のOrca再起動復旧、固定パネルからの終了はまだ受入前である。
`d5572d8f`は閉鎖receiptへLinux shellのPID・開始時刻を記録し、再照合時に元shellが残っていない
ことも確認する。担当作業場が後からdirtyになった場合は残りの終了を止める。最終版のtooling
Python 623件・Blender 164件、Ruff/actionlint、perf self-testとcontractsは通過した。
`4ad62830`は閉じた案件の担当を「未割当」でなく「終了」と表示する。`8ca7b917`は終了時に
課題と作業場identityを再照合してOrca board statusを`completed`へ移し、再読で確定する。
作業場の削除は別操作とし、途中の更新結果不明では閉鎖完了を偽装しない。最終候補のtoolingは通過。
`3181baba`は同じ役割の旧タブが正規のclose receiptで置換された履歴を照合し、現在の登録タブだけを
終了対象とする。旧タブの閉鎖が不明なら保留し、再起動・checkout移動による誤終了を避ける。

### M3: 既存作業場の移行と整理

以下は処分指示ではなく分類案。適用前に当日の所有者・成果・consumerを再照合し、previewを提示する。

| 現在の作業場 | 表示・扱いの案 | 保全条件 |
| --- | --- | --- |
| primary `codex/building-art-migration` | 「開発・建築ビジュアル」。汎用受付と分離 | 現在の会話・並行変更を維持。branch/pathは変更しない |
| `tak-8-storage-recovery-trial` | 利用者が意図的に撤去済み。新UIでは履歴扱いの候補 | commit `15066938`を保全。Linear課題の終了状態は未変更 |
| `Orca運用基盤` | 「保守・Orca基盤」 | candidateと必要cacheを保持。終了済み統括shellを受付に見せない |
| `tak-6-implementation` | 利用者が意図的に撤去済み。新UIでは履歴扱いの候補 | baseline commit `192f7d20`を保全。勝手に実装再開しない |
| refactor validation | 利用者が意図的に撤去済み。新UIでは履歴扱いの候補 | commit `a0487a35`と保護branchを保全。検証環境の現存は主張しない |
| detached `hell-workers` | 「検証保持・壁と扉」 | `formwork-release-mixed-quality-matrix`の継続用途。一般project名と混同させない |

- [ ] 登録済み作業場・paneを1件ずつ分類し、所有不明は「要確認」。他projectは対象外。
- [ ] 保守・検証保持を非破壊で区別する。collapse/filter/Sleepの採否はM0の確認結果に従う。`rm`で非表示を代用しない。
- [ ] 移行前後のidentity・表示・dirty・hold・sessionを比較し、終了した案件のタブが再起動で復活しない。
- [ ] runtime登録とfilesystemの欠損を検出し、空directoryを作ってcheckを通す復旧をしない。

現存するhell-workers作業場3件を再照合し、Orca metadataだけを非破壊で
「開発・建築ビジュアル」「保守・Orca基盤」「検証保持・壁と扉」へ分類した。
後者の保持consumerは解除していない。TAK-9/10の専用試験worktreeは成果・terminal・
holdを照合して撤去済み。既存sidebarの用途別filter/Sleepや再起動後の見え方は未受入。

### M4: 実Orca受入・配備・運用文書

- [ ] §7の全シナリオを実Orca UIとproviderで受け入れる。mock・無害shellだけの結果と分離する。
- [x] 基盤版、新規worktree、既存worktree、稼働中processの適用状況を明記して配備する。
- [x] 現行運用ガイドと受付計画の入口記述を同期し、「未実装」「保持中」「終了」を混同させない。
- [x] 画面からの最小操作と終了/再開の動線を利用者へ提示する。全体の完了判定は本計画とGit反復計画の両方の未受入項目を確認する。

2026-09-24、TAK-14を同一Runで実装A→検証→固定review→統合→統合後検証→最終固定reviewまで完了した。
Help判断、Linear一時断、質問待ち、統括専用の文書補正をexact receiptから再開し、別Runや手書き承認へ
迂回していない。最終承認後は登録済みrole tab 3件と補助shell 2件を出力保全・identity・idle・
positive closeで整理し、親作業場には統括1tabだけを残した。通常desktop launcherは標準UIを表示し、
固定受付をサイドバーから開ける構成へ戻した。残る§7シナリオは、完全な新規受付の無介入cold-startと、
稼働中agentを中断した場合の安全停止・アプリ再起動後の部分復旧である。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| UIの能力を推測してrepositoryだけで実装を進める | M0を実装前gateにする。未対応機能の責務と追加範囲を明示 |
| role固定化のため編集隔離やreviewer scopeを弱める | 固定するのは閲覧入口/session。checkoutの書込境界を維持 |
| close hookを通らない手動操作 | hookだけに安全性を依存せず、再起動時の台帳・process・Git照合を実装 |
| 終了報告後すぐ修正依頼が来る | 確認待ちでは環境を保持。最終承認/明示終了まで専用cacheを撤去しない |
| 終了処理中に新しい依頼が届く | 対象世代への投入を拒否/保留し、受付が別の依頼として照合。終了と配車で同じ排他を使用 |
| 古い成功・エラーが画面に残る | 最新の対象SHA・結果・照合状態・確認時刻を要約表示。旧scrollbackは履歴と明示 |
| 一覧を減らすため復旧済み領域を再び失う | 表示整理と撤去を分離。storage consumer・unique work・processを撤去前の必須gateにする |

## 7. 検証計画

### 必須シナリオ

1. Orca起動直後、利用者が依頼先を一意に判断し、日本語だけで新規依頼/既存再開できる。
2. Aのみの案件とA/B並列案件で未使用roleに空shellを作らず、統括・担当・reviewへ移動できる。
3. 実装→統括検証→固定review→差戻し→同じ担当で修正→再review→統合reviewを実providerで一巡する。reviewer session不変・対象SHA・可視role入口数を照合する。
4. 起動失敗/unknown時に理由と復旧先を表示し、空の統括や増殖した実装Aを作らない。
5. タブ×、アプリ終了、close中の応答断を試験専用環境で再現し、再起動後に未処理箇所だけ復旧する。
6. 提示後の確認待ち→追加修正で同じ作業場/cache/sessionを利用する。休止は完了/削除と区別する。
7. 最終終了後は稼働中一覧から区別され、owned process/不要paneがなく、結果を確認できる。保持consumerのある環境は残る。
8. 未追跡原本、独自commit、手動split、他project、primary並行変更、凍結検証領域を巻き込まない。
9. 利用者のスクリーンショット相当のsidebar幅・tab幅で、用途と状態が省略されず区別できる。短い名前、色以外の状態、選択中案件、移動先、終了後画面を画像で確認する。

### 検証の範囲と証拠

- Python unit/integration testで状態遷移・異常系・所有権・再試行を検証し、同一subjectのcontracts/tooling gateを実行する。
- 完了判定は同一base/head/sourceのCIまたは`python3 scripts/dev.py ci check --base <full-SHA> --mode auto`。選択群がゲームを含む場合は分類根拠を確認し、範囲外のskipを全群成功と記録しない。
- Orcaの画面受入はOrca用操作経路で行う。ゲームのnative受入・Bevy起動を代用または追加しない。
- UI受入ごとにOrca版・基盤SHA・provider/session・対象SHA・前後のpane数・状態・終了receipt・残存理由を短く記録する。秘密情報や会話全文をGitへ保存しない。
- CPU/RAM測定は必要なhost資源制御の範囲に限る。軽量UI更新でLLMを反復起動せず、eventまたは有界backoffで更新する。
- `docs --write`、`docs --check`、`git diff --check`、primary `validation check`。コード変更時はHelp-impact Skillで実経路から再判定する。

### 検証データ管理

- 正本: [検証データ整理](../development-infra/validation-storage-workflow.md)。primary coordinatorで保持・実行・結果確定・整理を管理する。
- 計画作成時は読取りとMarkdown編集のみだった。実装承認後は下記の独立Orca cloneと隔離UI batchを登録して利用する。
- 実装・UI試験開始時にbatch、owner、consumer、exact path、開始bytes、次工程、release_whenを登録する。既存holdに無断でconsumerを追加/解除しない。
- 欠損していた3件は利用者の撤去確認と現物照合に基づき、該当consumerを解除済み。過去のbytesを現在値として転記しない。
- 確認待ちのcandidate/targetは同じ場所で保持。採用結果は正本へ集約し、最終利用終了後に不要job/一時fixtureを整理する。削除pathと前後bytes/空き容量差を報告する。

## 8. ロールバック方針

- UI投影と新規受付を停止して既存loopの新規配車を抑止し、実行中の担当の終了照合は継続する。
- schema移行前のbackupと対応versionを保持し、未決operationがある間は古い実装を再投入しない。
- metadataのみの変更は前値へ戻せるが、実processを停止・再起動した事実や外部書込みは巻き戻したと偽装しない。
- 作業場撤去を通常rollbackの手段にしない。唯一の成果物を先に保全し、既存会話・承認対象・他sessionの変更を維持する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 本体拡張を承認済み。`06cfe378`を`ui-06cfe378`へ一時配備し、固定パネル、4役割表示/移動、終了確認、型付きRPCとprivate mailboxを実装。TAK-14の実loopと承認後role tab整理まで接続済み。
- 当初の6作業場とCLI/plugin APIを読取り確認済み。そのうちTAK-6/8とR01〜R14環境は利用者が撤去済みで、現存一覧とは区別する。公開plugin APIの不足を本体拡張で補う。隔離した実Electronによる画面/API検証は実施済みで、稼働中Orcaの操作や実provider一巡の受入とは分離する。
- primaryの`docs/plans/orca-git-review-loop-plan-2026-09-22.md`と索引には本計画以前の保存領域復旧の未commit差分がある。保全する。

### 次のAIが最初にやること

1. 実装指示の有無を確認し、primary文書・candidate HEAD/dirty・当日のruntime/holdを再照合する。
2. TAK-14の最終loop、role tab終了receipt、`06cfe378`のlauncherを確認する。次の新規案件は固定受付から開始し、入力訂正・外部保守resume・追加ID入力が0回かを計測する。
3. 稼働中agentの中断、アプリ再起動、close中の部分失敗を専用試験で受け入れる。途中のunit passを全ライフサイクル完了と報告しない。

### 参照必須ファイル

- `docs/orca-quickstart.md`、`docs/development-infra/orca-development.md`
- 関連する受付・Git反復計画、`docs/development-infra/validation-storage-workflow.md`
- candidateの`orca.yaml`、`scripts/orca_role_tabs.py`、`scripts/orca_ui_coordinator.py`、`scripts/orca_review_loop.py`と対応tests
- 導入版の`orca-ide skills get orca-cli --json`とCLI help

### 最終確認ログ

- 2026-09-23: 作業場一覧・CLI help・candidateソースをread-only確認。実画面の追加操作や終了処理は未実施。
- 別sourceのNode/mainとrenderer型検査、19 unit tests（mailbox 7、hidden-window guard 2、既存window 10）を確認。全E2E型検査は未変更のcloud等の領域で219件のエラーがあり不合格。今回のsupervision/offscreenファイルには型エラーなし。全Orcaテスト成功とは扱わない。
- 隔離Electronでは未接続表示、終了確認/同一操作の再送、古い情報での操作拒否と分類、実PTYへの移動でtab数不変の4シナリオを検証。fixtureのroleは実LLMではない。日本語表示を画像で確認する。
- 初期のhidden-window試験は描画frame停止で失敗。通常アプリを表示/focusせず、テスト専用offscreen条件を追加して解決。次の移動試験はfixture端末の接続待ち不足を修正。attempt-5は4件passでも型検査cacheを含むfingerprintが変わったためinvalidとし、結果を上書きせずattempt-6を独立sealした。
- 再送時directory fsync・期待runtime ID・要確認の分類例外を追加し、attempt-7で最終sourceの4件passを独立seal/finalizeした。primaryのRust用fingerprintだけではTypeScriptを証明できないため、source/test/生成bundleを結ぶ独立verifierを登録した。変更ファイルの品質gateは追加指摘0件。
- 不要な試験データは`/home/satotakumi/tools/orca-ui-lifecycle/.git/orca-ui-acceptance/`配下の`attempt-2`〜`attempt-6`だけを削除。実測1,400,832 bytes→0。試験専用report/trace/画像/fixtureで、独自source・実会話・既存作業場は削除していない。
- 保持: 同cloneは実測2,422,947,840 bytes（依存・build・最新画像を含む）、最新`attempt-7`は339,968 bytes。ownerはOrca UI lifecycle implementation、consumerは`orca-ui-lifecycle-implementation-and-review`。次工程はcontroller接続と画面feedback、終了条件は受入/中止と固有source保全。旧試験consumerは解除済み。primary storage checkはpass。
- Rust check/Clippy/test、ゲームnative、実provider受入は未実施。変更別gateはprimary文書と独立Orca sourceを分けて記録する。
- 利用者がR01〜R14候補、TAK-6、TAK-8の3作業場を意図的に撤去したと確認。残存commit/branchを保全して該当hold consumerを解除し、primary `validation check`がpass。`attempt-16`の隔離Electron 2件と独立verifierがpass。`attempt-13`〜`15`および旧`attempt-12`の専用証跡だけを撤去し、現行証跡は`attempt-9`と`attempt-16`を保持する。
- `7932a355`で背景の可視統括tab起動と二重起動禁止を実装。専用`TAK-9`は1課題・1分離worktree・1統括tab、acknowledge、route ready、再実行時の1tab維持まで実Orcaで確認。後にclose receiptとtab 0件まで照合し、専用worktreeはhold解除後に撤去。試験stateのみ証跡として保持する。
- Help-impact Skillの判定: No impact。変更は別アプリの開発用UI/API、隔離検証と運用文書であり、ゲームの入力/状態/UI/Help providerへの実装経路変更なし。ゲーム内Help manifest/provider/approval snapshotは変更しない。

### Definition of Done

- [ ] M0〜M4と実UIシナリオがすべて完了し、入口・進行・終了・再開が画面から判断できる。
- [ ] 変更別検証のbase/head/source・必要群・結果を記録し、fixtureと実provider/UIの受入を分離した。
- [ ] 同じ担当の差戻し/再起動で不要tabが増えず、固定reviewerと分離編集を維持した。
- [ ] 確認待ちの環境を保全し、最終終了のprocess/通知/成果/consumer/UIを照合した。
- [ ] 本計画の終了consumerは0。共有残存はowner・consumer・bytes・次工程・release_whenを引継いだ。
- [ ] 正本仕様・運用ガイドを同期し、未受入・未配備を残して全体完了と報告していない。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-23 | Codex | UI視認性・固定受付・役割navigation・終了/再開・既存作業場移行を一体化して初版作成。計画のみ |
| 2026-09-23 | Codex | 実装依頼を受けM0のCLI/plugin一次情報を照合。公開APIの不足と実画面取得不可を記録し、本体拡張の範囲確認まで実装を停止 |
| 2026-09-23 | Codex | 本体拡張を承認。導入版tagの独立cloneと専用branchで固定パネル/APIを実装、別ビルド検証を開始。稼働版と実案件は変更せず、controller接続・最終終了の未完を明記 |
| 2026-09-23 | Codex | 利用者が撤去済みの3作業場を保持台帳で照合・解放。隔離Electron `attempt-16`で相談/明示実装の2経路を受入。実配車・終了・稼働版配備は未完 |
| 2026-09-23 | Codex | 統括tabの一度だけの背景起動を追加。実Orcaの専用`TAK-9`でLinear→分離worktree→可視統括の1tab経路を確認。固定パネル・終了・配備は継続 |
| 2026-09-24 | Codex | TAK-14を同一Runで最終固定reviewまで完了。Help/検証/Linear/質問/文書補正の回復を恒久化し、role tab 3件と補助shell 2件を安全に整理。通常desktop起動を復旧。無介入cold-startと稼働中中断復旧は継続 |
