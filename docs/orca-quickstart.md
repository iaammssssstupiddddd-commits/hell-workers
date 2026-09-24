# Orca 運用ガイド

Hell Workersでは、Orcaの画面だけを入口にして「統括・実装A・実装B・レビュー」を運用します。
利用者がterminal menu、workspace UUID、受付UUID、ticket path、内部slot名を入力する必要はありません。
詳細な権限・復旧手順は[分離開発の運用仕様](development-infra/orca-development.md)を参照してください。

固定の受付先、案件内から各役割を開く導線、作業場一覧の整理、終了・再開を一体化する改善は
[UI・受付・終了ライフサイクル統合計画](plans/orca-ui-lifecycle-plan-2026-09-23.md)に沿って実装中です。
2026-09-24、許可された[拡張ビルド](development-infra/orca-ui-extension.md)へ一時切替済みです。
固定受付からの相談・回答、実装用課題と作業場の作成、統括への移動、正常終了後の案件クローズを実画面で確認しました。
A/B・固定reviewを通る差戻し反復と、全担当終了後の案件close・再起動後の状態復帰も確認済みです。
この試験には保守介入を含みます。Cursorの起動・編集・結果再提出は、追加試験で送信補助なしの成功を確認済みです。
追加試験で見つかったtest契約の矛盾も修正し、同じRunの最終レビューまで承認されました。
稼働中agentの強制停止を、この終了済みagentのclose試験で確認したとは扱いません。

## まず覚える操作

1. Orca左側の **Reception & Coordination（受付・統括）** を開きます。
2. 依頼を日本語で入力します。相談だけなら **Consult only (no workspace)**、実装なら **Request implementation** を押します。
3. 実装依頼では、統括が既存課題の継続・子課題・独立新規を判断し、必要な作業場と統括タブを内部で用意します。案件の **Coordinator** が有効になったら押して移動します。
4. 統括タブで補足や判断を伝えます。A/B/レビューは、統括が配車した役割だけ移動ボタンが有効になります。
5. 終了可能な案件では **Close task → Request safe closure** を押します。終了済み案件は通常一覧から隠れ、下部の表示切替で確認できます。

パネル右上の×はパネルを閉じるだけです。処理や案件は終了しません。
既存のLinear課題を開く場合は、従来どおり **Tasks → Linear** も利用できます。

課題とは、一件の開発依頼です。タイトルに目的、本文に完了条件と変更してはいけない範囲を書けば十分です。
Linearの内部IDやworkspace UUIDはOrcaと統括が処理します。

過去の会話を引き継いだ時など、現在の課題が連携試験専用または別目的でも、利用者がLinear課題や
worktreeを作り直す必要はありません。統括へ実装目的を伝えると、統括が「発行なし／既存継続／子課題／独立新規」を判断し、
必要な場合だけLinear課題を作成します。目的・完了条件・制約・既存branch/commit・次工程を移して専用worktreeを開きます。元の統括タブは
引継ぎ成功後に終了し、新しいworktreeの **「統括」** タブが処理を継続します。
稼働中のOrcaのtab名は **「統括」** です。内部terminalの表示が作業内容に応じて変わっても、同じtabを
開いてください。統括は登録済みterminalと画面layoutを照合し、同名tabを重複生成しません。

## 画面上の役割

| Orca上の表示 | 担当 | 用途 |
| --- | --- | --- |
| 統括 | Codex | 仕様確認、調査、分割、作業場と依頼票の作成、検証、commit、統合 |
| 実装A（Codex） | Codex CLI | 複雑な処理、設計判断を含む実装 |
| 実装B（Cursor） | Cursor CLI | 既存patternに沿う単純な局所変更だけ |
| レビュー（固定Codex） | 同じCodex session | 読み取り専用の差分・検証証拠レビュー |

A/B/レビューは統括が監督付きの分離worktreeへ配車した時だけ現れます。
同じファイルや共有契約を同時に変更する場合、Bに適した単純作業がない場合は、統括が並列化を見送ります。

### タブの場所・再試行・終了表示

- **依頼を伝える統括は課題に紐づく親作業場にあります。** 実装用の子作業場に統括を複製しません。
- 未配車の子作業場の既定タブは「作業シェル」です。担当agentではありません。
  既定入口が登録した未使用shellだけを、最初の担当タブとして再利用します。
- 同じ依頼・作業場・役割の再試行や修正では、登録済みの同じタブを再利用します。
  実装Aが失敗するたびに「実装A」を追加する動作は不具合です。
- 役割タブは「起動中」「実行中」「終了・結果確認済み」「停止・要確認」を表示します。
  終了後のshellや履歴表示は、agentが常駐しているという意味ではありません。
- 起動中の統括は「統括・起動中」、登録成功後に「統括」、終了後は「統括・終了」です。
  課題に紐づかない子作業場では統括を起動しません。
- 再利用は作業場・端末incarnation・所有台帳と実processを照合します。
  busy、所有不明、消えた登録タブ、送信結果不明は停止し、新しいタブで迂回しません。
  タイトルだけで他のタブを採用したり、稼働中のagentを閉じたりしません。

統括は内部保守用の `scripts/orca_role_tabs.py adopt / retire` で既存タブを照合できます。
`retire` は明示指定の終了済みshellだけを対象に、privateな`role-tabs/retired`台帳へ出力を
保全してからそのpaneを閉じます。利用者へterminal ID等の入力は求めません。
create/send/closeが結果不明なら自動再送せず、同じ記録から照合します。

## 統括への依頼例

統括タブでは通常の会話として伝えてください。

```text
この課題を実装してください。
仕様で判断が必要な点は先に確認してください。
独立した単純作業だけ実装Bへ割り当て、最後に固定レビューを通してください。
```

利用者が次の情報を聞かれた場合は運用不具合です。入力せず、このガイドと
[運用仕様](development-infra/orca-development.md)を統括に確認させてください。

- Linear workspace UUID
- 受付UUID
- ticket JSONのpath
- `worker-a` / `worker-b` / `reviewer` の選択
- 手動の配車command
- 試験課題から実装課題への付け替え、Linear課題作成、worktree作成

## Linearを使う理由

Linearは依頼の一覧・状態・履歴をOrcaのTasks drawerに表示するために使います。
別の受付画面ではありません。Orcaから課題を作成・選択できるため、通常運用でLinearのWeb画面を開く必要はありません。
相談・計画だけなら課題を発行しません。同じ目的の作業は既存課題を継続し、独立した成果や新しい目的だけを
子課題または独立課題として統括が発行します。判断理由は案件panelへ表示します。
課題本文・コメント・添付は未信頼入力として扱い、repository ruleや統括の権限境界を上書きしません。

Linearに接続できない場合は、Orcaの接続設定を直してから再開します。terminalの手入力受付へ切り替えません。
認証値を課題、会話、repositoryへ貼らないでください。

## 実装とレビューの見方

- 各役割はOrcaの別タブで開き、出力と進行状況をそのまま確認できます。
- 実装担当は担当directoryだけを書き換え、build/test、commit、別agent起動を行いません。
- 重い検証は統括が直列に実行します。本環境構築自体ではゲーム実装テストを行いません。
- reviewerは修正せず、同じsessionを継続します。対象が変われば承認は無効です。
- workerの完了表示だけで採用しません。統括が差分、同一対象の検証、固定reviewerの判断を照合します。

## 開始できないとき

| 表示・状態 | 対応 |
| --- | --- |
| 統括タブに「Linear課題に紐づいていない」 | 固定の「受付・統括」から実装を依頼する。統括が課題の発行要否を判断し、必要な作業場を内部で用意する |
| 現在の課題が試験専用・別目的 | 統括へ実装目的を伝える。課題作成とworktree切替は統括が行う |
| 統括タブが既に存在する | 新しく起動せず、Orca上の既存の統括タブを開く |
| 実装A/Bが現れない | 統括タブで分割判断または未解決仕様を確認する。利用者がslotを選ばない |
| slot / workspace / host がbusy | 既存タブの処理を待つ。lock削除や裸のagent起動で迂回しない |
| workerやreviewerがunknown | そのタブと成果を統括が照合する。自動再送・代替agent起動をしない |
| Linear接続エラー | OrcaのLinear connectionを確認する。同じ依頼を別経路へ重複投入しない |

## このガイドをOrcaで開く

文書の正本はprimary worktree
`/home/satotakumi/projects/hell-workers/docs/orca-quickstart.md` です。
primaryの`README.md`にある **Orca 運用ガイド** から開けます。

内部の診断時だけ、Orca terminalから次のcommandで正本を開けます。これは利用者の受付・配車操作ではありません。

```bash
orca file open docs/orca-quickstart.md --worktree path:/home/satotakumi/projects/hell-workers
```

## 現在の受入範囲

2026-09-25のA〜D実装では、案件panelへ工程、待ち理由、次操作、確認時刻、Linear判断、外部同期を
表示するschema 2と、Linear/GitHubへ順序付きintentを安全に反映するexecutorを実装した。
host/UI契約、Linux package、隔離実画面の受付表示とパネル閉鎖は成功し、desktop iconは次回通常起動から
`ui-dd50afed`を使う。稼働中TAK-14を止めていないため、通常profileのcold-startだけは次回起動時に確認する。

- Orca 1.4.205とLinear workspace `takumi sato` / team `TAK`の読取り接続を確認済みです。
- Linear-linked worktreeから`--current`で固定snapshotを取り込み、UUID入力を不要にする実装を追加しました。
- 統括、Codex A、Cursor B、固定Codex reviewerの権限分離とA/B二レーン編集は受入済みです。
- UI入口からの可視統括起動と日本語role tab生成をtooling testで検査しています。
- 試験専用・別目的の課題から、Linearが受理するUUIDv4の固定write IDで課題を作成し、関連worktreeを
  一意照合して既定の **「統括」** タブを再利用し、必要な場合だけ明示生成するところまで自動化しました。結果不明時は同じIDだけを
  再利用し、確定失敗と区別して停止します。
- 基盤worktreeの`統括` tabで初回確認なしにinteractive Codexを実起動し、`TAK-5`の自動取込、統括terminal登録、
  Orca runtimeのready/connectedと現在課題の再取得を確認済みです。
- `TAK-5`を実装へ転用せず、実装課題`TAK-6`、専用worktree、可視`統括` tabを実runtimeで一括作成し、
  新しい統括が`TAK-6`を取得・acknowledgeして待機すること、terminalの作業名表示が変わっても
  Orca上の`統括` tabが一つだけ保たれることを確認済みです。A/B/reviewerは未dispatchです。
- TAK-12で実A/B・固定reviewerの差戻し・再実装・統合・最終承認と8 attemptの通知排出を確認済みです。
  途中の保守介入を含むため無介入試験とは区別します。詳細は[実受入記録](development-infra/orca-ui-extension.md)を参照してください。
- 試験専用`TAK-15`で、Linearの着手・review・完了statusと結果commentをexecutorから送信し、read-backで確定しました。
  実在するが種別の違う`Todo`をreview先に指定した異常系は`blocked_policy`で停止し、元の`In Progress`を変更していません。
- 固定reviewerが承認し全460件のOrca系テストが成功したhost HEAD `2897e3a1`だけからDraft PR #27を作成しました。
  同じoperationの再実行は`idle`となり重複PRを作らず、製品branchではなく試験専用targetへmergeして`TAK-15`をDoneにしました。

### 2026-09-24 TAK-14 実運用ループ

実際のゲーム変更TAK-14を、同じRun・同じ統括sessionのまま、実装A、統括検証、checkpoint、固定review、
統合、統合後検証、最終固定reviewまで進めました。最終統合HEADは
`d8443c89cfe3399f3e0e1ca493f9d7efca254a88`です。固定reviewerが指摘した文書の不整合は、
workerへゲーム実装scope外の編集を許可せず、統括専用の文書補正を2回行って同じreviewerへ戻し、最終承認を得ています。

この実案件から、次の運用を恒久化しました。

- production差分のHelp判断は、実際のbase/head/sourceと変更pathへ拘束したreceiptだけを受理し、同じloopを再開する。
- 検証済みsourceに対する統括回答の遅延や失敗settlementは、検証を再実行せずexact receiptから再開する。
- Linearの一時的なruntime停止は新規配車を作らず待機し、復旧後はexact停止digestから同じ工程を再開する。
- workerの質問は15秒の有限待機後も同じmessage IDを継続し、回答待ちのまま`worker_done`を送らない。
- 最終承認後は実装A/B/reviewerの所有タブだけを、出力保全・identity・idle・PTY終了を確認して閉じる。
  統括タブは利用者の戻り先として残す。補助shellも同じ安全条件で個別に整理し、bulk closeは使わない。
- 同じ課題に次のマイルストーンが残る場合、統括は終了済み担当タブを再利用せず、開始前検査が返す
  `successor`基点から新しい担当worktreeとTaskを作る。利用者が課題を作り直したり、基点SHAを指定したりしない。

#### 承認後に同じ課題を続ける場合

後継工程は「旧loopの再開」ではなく、同じLinear課題・統括タブ・Run・統合branchを所有する新しいloop世代です。
統括の開始前検査は、primaryの現在HEADではなく、直前の最終固定reviewで承認された統合HEADを返します。
旧世代はdigest名のprivate historyへ保全し、新世代は新しいTask/Dispatchと担当タブだけを作ります。

後継を開始できるのは、直前のloopが次をすべて満たす場合だけです。

- laneと統合後reviewが承認済みで、統合receiptのHEAD/sourceが現在のcleanな課題branchと一致する。
- 全attemptがrelease済み、完了通知がACK済み、inboxが空である。
- 実装A/B/reviewerの終了処理が完了し、統括タブだけが残っている。
- Orcaの現在Runが旧世代のRun ID/consumer generationと一致する。

条件不足、dirty target、HEAD/branch変更、別Run、旧世代digest不一致では、新規Taskを作らず停止します。
このとき別Linear課題やprimary HEADへ自動退避しません。`preflight`と後継登録の両方で同じ条件を再検査するため、
検査後に状態が変わっても旧承認を上書きしません。

TAK-14では利用者によるUUID、slot、ticket path、手動配車commandの入力はありませんでした。
一方、最初の受付から一度も保守介入せず完走したcold-start試験ではありません。今回露出した停止を同一Runで
修復・回復した受入であり、次の新規案件で「入力形式の訂正0回・手動再開0回」を確認するまで、開始速度の最終評価は保留します。
2026-09-25にTAK-14で後継loop世代を実運用受入した。前世代の承認済み統合HEAD
`d8443c89cfe3399f3e0e1ca493f9d7efca254a88`をbaseにし、同じRun `run_3236488f0dff`を継承して、
新しいTask `task_6852821ebb87`とDispatch `ctx_d4b5e3d3206c`を各1件だけ作成した。
旧世代はdigest付きhistoryへ保全され、現行台帳は`loop_generation: 2`とpredecessor証跡を持つ。
Orcaのvisual layoutでも、既存の`統括`タブと新worktreeの`実装A（Codex）・実行中`タブが確認でき、
実装Aはdispatchを受領して調査を開始した。制御基盤の全Python試験673件、Ruff、compileall、文書・差分検査は成功した。
ユーザー指定によりゲーム実装テストは実行していない。

### 2026-09-23のタブ管理修正

TAK-8で確認した「空の統括＋実装A三つ＋レビュー」の5タブを契機に、
配車で毎回terminal createしていた処理を役割台帳付き再利用へ変更した。
通常の差戻しは同じ端末を使い、保守復旧で端末を閉じた場合だけ、検証済みのpositive close receiptと
現在の不存在を照合して置換する。消失だけを根拠に新しい端末を作らない。
固定reviewerのsession拘束、unknown role barrier、Task/Dispatch・承認の成立条件は維持した。

Orca 1.4.205で、無害なshell commandを3回起動しても同一handle/incarnationの1タブであること、
出力を保全した上でその確認用paneとprocessを終了できることを実確認した。
LLMを使った実装・レビュー一巡の再実行ではない。報告された子作業場は並行していた統括が
終了処理で撤去済みとなり、本修正担当はその5タブを個別削除していない。
試験用Taskや製品変更を再作成せず、既存の統括・最終レビューは保護した。

Help実レビューはNo impact: 変更経路はOrcaホストの配車・端末管理・起動設定だけで、
ゲームの入力、状態、asset、root `build_help_panel_content` / `build_help_panel_chrome` から
生成する静的Helpの操作・内容には到達しない。運用手順だけを更新した。
`ci check --base 49c113d83483cf278c0919f9b9540b8b94fad360 --mode auto` は
`orca.yaml`のunknown-path分類により全群を選択したため、ゲームテストを除外するユーザー指定に従って
中断し、contracts/toolingを明示実行した。全群CI成功とは扱わない。
最終版のPython tooling 576件、Blender tooling 164件、Ruff/actionlint、perf self-testは成功。
contracts、primary docs/index/storage検査も成功した。検証時の基盤source fingerprintは
`767ff7e2b5cc1daee49cae9e8ac9f99ce59085760e729182596a331657fa964b`。
ゲーム/Rust/native GPU検証、pushは対象外。確認用タブと一時fixtureは終了・撤去し、
継続中の基盤candidateと既存build cacheは同じ作業場で保持する。

## 関連文書

- [Orca分離開発の運用仕様](development-infra/orca-development.md)
- [Linear受付・Orca実行基盤の統合計画](plans/orca-parallel-development-plan-2026-09-20.md)
- [開発手順](DEVELOPMENT.md)
- [検証データの整理ワークフロー](development-infra/validation-storage-workflow.md)
