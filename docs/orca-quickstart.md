# Orca 運用ガイド

Hell Workersで「統括1名・実装最大2名・専任レビュー1名」を運用するための入口です。
詳細な権限・資源管理は [分離開発の運用仕様](development-infra/orca-development.md) を参照してください。

> Linearの専用試験issueから統括へ相談し、同じ会話へ追記できる正常系まで受入済みです。
> Codex固定reviewer、Codex A、Cursor Bのread-only Task lifecycleと、別worktree・固定ticket・統括検証・固定reviewを通るA/B限定編集および2レーン並列実行を受入済みです。
> 監督付きOrca launcher経由の編集は利用できます。Linear受付からの自動配車、共有file/APIを含む並列化、異常系の全受入は未完です。
> 通常の「Codexを起動」操作では担当範囲の制限が入りません。実装担当とレビュー担当は必ず専用launcherから起動します。

## Linearへの段階移行と現在地

依頼受付・進捗管理をOrca標準のLinear連携へ移し、既存の統括相談・権限制御・固定reviewerを再利用する計画です。
詳細は[現行計画L0〜L4](plans/orca-parallel-development-plan-2026-09-20.md)を参照してください。

| 段階 | 利用する入口 | 現在地 |
| --- | --- | --- |
| 課題管理（L1） | OrcaのLinear課題一覧・詳細 | 専用試験issue [`TAK-5`](https://linear.app/takumi-sato/issue/TAK-5/orca-integration-acceptance-hell-workers) の作成・コメント更新・再読・worktree関連付けを受入済み。権限/通信異常系は未受入 |
| 統括相談（L2） | Linear課題の固定snapshotから既存統括を明示起動 | `TAK-5` の固定snapshot取込、初回相談、同一session追記を受入済み。旧受付移行・異常復旧は未受入 |
| 自動受け渡し（L3/L3E） | Orca Taskと既存launcher、固定reviewer | 3 roleのread-only一巡、A/B限定編集、統括検証、同一固定reviewer、別worktreeの2レーン並列実行を受入済み。受付からの自動接続は未受入 |

Linearへ接続しただけでは統括agentは起動・常駐しません。初期の課題更新は統括の明示操作とし、
常時同期や課題の担当変更による自動起動は追加しません。仕様書は引き続きprimary `docs/`が正本です。
利用対象はworkspace `takumi sato` / team `TAK` / 試験issue `TAK-5` に限定しています。認証値をこの会話やrepoへ貼らないでください。
移行が受け入れられるまでは下記の手入力fallbackを保持し、依頼・会話の削除や同じ依頼の二重投入は行いません。

この環境整備ではゲーム実装のテストは実行せず、変更した運用ツールと連携・文書の検査に限定します。

## このガイドをOrcaで開く

- 文書の正本はprimary作業場 `/home/satotakumi/projects/hell-workers` にあります。
- Orcaのprimary作業場で本ファイルのエディタタブを選びます。
- タブを閉じた場合は、primaryの `README.md` → **Orca 運用ガイド** から辿れます。
- 別作業場にいる場合も、Orcaのターミナルで次の1行を実行するとprimaryの本書が開きます。

```bash
orca file open docs/orca-quickstart.md --worktree path:/home/satotakumi/projects/hell-workers
```

これはローカルファイルを開く操作です。Artifactsへのupload・公開は不要です。
以下の `orca` コマンドはOrca内のターミナル用です。外部ターミナルでは `orca-ide` に置き換えます。
外部の `/usr/bin/orca` は別製品のスクリーンリーダーなので使いません。

## 1. 開発受付と現在の利用範囲

「統括agent」は自動で常駐しているサービスではありません。過去の会話を探して依頼する方式を廃止するため、
固定の受付から必要なときだけ起動し、回答後はprocessを終了します。次の追記では保存済みの会話IDを再開します。
旧受付の実績とLinearへの移行順は [現行計画L0〜L4](plans/orca-parallel-development-plan-2026-09-20.md) を参照してください。

Orcaの作業場一覧で **`orca-parallel-development`** を選び、terminalタブの
**「Linear受付・統括相談」** を開いてください。現在このタブは設置・起動済みです。
メニュー待機中はLLMを動かしません。Orca全体を再起動した際のタブ自動復元は未受入です。

タブを閉じた場合の試行command（同じ基盤worktreeで実行。既に起動中なら二重起動を拒否）:

```bash
python3 scripts/orca_frontdesk.py menu
```

| 入力 | 操作 |
| --- | --- |
| `1` | Linear課題ID/URLとworkspace UUIDを入力し、固定snapshotとして保存。未接続・不完全応答は保存前に停止 |
| `2` | 保存済み依頼の一覧 |
| `3` | 受付番号を選び、`yes` で統括へ初回相談。既に成功した相談なら回答の再表示のみ |
| `4` | 受付番号を選び、追記を入力して `.` → `yes`。同じ会話を再開 |
| `5` | 会話ID・ターン状態・保存済み回答を確認。LLMは起動しない |
| `6` | 異常終了した相談を照合。終了確認済みprocessと既知の会話IDがある場合だけ、新しい確認ターンを起動 |
| `7` | Linear障害時の手入力fallback。目的・完了条件・変更禁止事項を入力し、単独の `.` で保存 |
| `q` | 受付を終了。依頼・会話は保持 |

`3` / `4` / `6` は確認後にCodexを1ターン起動し、モデルの利用が発生します。
受付の終了・再起動を挟んだ `4` で、同一会話IDと過去の内容の継続を実Codexで確認済みです。
Linearのworkspace UUIDは `orca linear team list --workspace all --json` で確認します。
現在はworkspace `takumi sato`（`68abc67b-ca1b-407b-be63-99dd91321b26`）とteam `TAK`を読取り確認済みです。
利用対象と外部書込み範囲の確定前にissueを更新しないでください。認証値ではなく返却されたworkspace UUIDだけを
入力してください。直接取込する場合は次を使えます。

```bash
python3 scripts/orca_issue_context.py HW-42 --workspace <workspace-UUID>
```

同一workspace・issue内部ID・snapshot hashは同じ受付UUIDへ冪等化され、後編集は別snapshotとして新しい受付になります。
対応表は `.local/state/hell-workers/linear-intake/snapshots.json`、依頼本文は
`.local/state/hell-workers/frontdesk/requests.json` にowner-only・atomic保存され、terminalを閉じても消えません。
相談状態は同じdirectoryの `consultations/<受付UUID>.json`。会話runtimeは別の
`.local/state/hell-workers/agents/coordinator/<受付UUID>/` にあります。手動でIDや成功状態を書き換えないでください。
「相談ターン終了」は実装完了・承認ではありません。Orca Task連携の受入まではworkerへ本番投入しません。
結果不明かつprocessの終了を確認できない場合、`6` でも停止します。別agentの自動起動や元指示の自動再送はしません。

保存する依頼本文の例:

```text
docs/orca-quickstart.md と docs/development-infra/orca-development.md に従って進めてください。

目的: ［実現したいこと］
完了条件: ［期待する動作・テスト条件］
変更しない範囲: ［既存作業、共有仕様など］

最初に読み取り専用で調査し、並列化できる独立taskと受入条件を提案してください。
複雑なtaskはCodex A、単純な局所変更だけCursor Bへ割り当てる案にしてください。
今は実装・build/test・別agent起動・commit・push・PR作成を行わないでください。
```

同じファイルを触る変更、共有型や仕様を同時に変える変更は、無理に並列化しません。
公開やcommitが必要になった場合は、その対象を確認して別途指示します。

## 2. 担当と作業場所

| 担当 | 作業場所 | 主な仕事 |
| --- | --- | --- |
| 統括 | primary文書＋対象の分離作業場 | 分割、ticket作成、検証、レビュー確認、許可された統合 |
| worker-a / Codex CLI | 分離worktree A | 設計判断や複雑なtask。build・commit・再委譲はしない |
| worker-b / Cursor CLI | 分離worktree B | 既存patternに沿う局所修正・機械的変更・限定test追加。複雑なtaskはAへ戻す |
| reviewer | レビュー対象worktree（sourceはread-only） | 差分と検証証拠の確認。自分では修正しない |

- 初回受入後の上限は実装2件・reviewer1件。重いbuild/testは全体で1件です。
- 同じworktreeでworkerとreviewerを同時に動かせません。実装担当を終了してからレビューへ渡します。
- reviewerは同じsessionを継続します。別対象へ移る場合も、旧launcherを終了して同じUUIDで再開します。
- primaryの未commitゲーム変更は、新しい作業場に自動で入りません。必要なら先に統括へ伝えます。

## 3. 統括が作業場を準備する

新規worktreeの既定基点は `iaammssssstupiddddd-commits/orca-parallel-development`。
最新commitは `55b27f6e1d9103d7985941c3cbbf135c7299be91` です。
受付・統括相談、同一task再開・固定reviewer拘束、read-only workerとCursor起動修正、制限通信診断、
Codex用の単一Dispatch通信bridge、Cursor hook bridgeを含みます。固定reviewer・Codex A・Cursor Bの
read-only実Task一巡に加え、A/B限定編集、統括検証、固定reviewer、別worktreeの2レーン並列実行を受入済みです。
いずれもローカルのみです。以下は統括が使う監督付き運用手順であり、Linearからの自動運用手順ではありません。

```bash
orca repo show --repo path:/home/satotakumi/projects/hell-workers --json
orca worktree create --repo path:/home/satotakumi/projects/hell-workers --name leaf-task-a --setup run --no-parent --json
```

`leaf-task-a` は実際の一意なtask名へ変更します。`--agent` は付けません。
返されたworktreeの絶対path・branch・HEADを記録し、setupの診断成功、基盤commitの継承、cleanな状態を確認します。
既存のdirty変更を消してcleanにしてはいけません。作業場/cacheは
[storage手順](development-infra/validation-storage-workflow.md)に従いprimary台帳へ用途付きで登録します。

ticketは対象sourceの外に保存します。例: primaryの `.git/validation-storage/` 配下。
`repo` は対象worktreeのcanonical絶対path、`base` は**その時点のHEADの40桁SHA**、
`allowed_directories` は実在する最小限の担当directoryを指定します。
JSONの形式は [依頼票と起動](development-infra/orca-development.md#依頼票と起動) を参照してください。

## 4. 統括が専用launcherで起動する

以下の `/absolute/...` は実際のpathに置き換えます。対象worktreeにあるlauncherを使います。

```bash
python3 /absolute/worktree/scripts/orca_roles.py launch --ticket /absolute/task-ticket.json --slot worker-a --dry-run

orca terminal create --worktree path:/absolute/worktree --title "worker-a | task-name" --command 'python3 scripts/orca_roles.py launch --ticket /absolute/task-ticket.json --slot worker-a' --json
```

`--dry-run` は起動引数の確認のみで、実起動成功の証拠ではありません。
返されたterminal handleを記録し、出力を確認します。taskの初期指示はticketから渡るため重複送信しません。

```bash
orca terminal read --terminal <handle> --json
orca terminal wait --terminal <handle> --for tui-idle --timeout-ms 30000 --json
```

待機結果は `satisfied: true` を確認します。無応答だけを理由に別agentを増やしません。
初回の起動・レビュー・同じreviewerの再開が確認できた後、独立した別worktreeで `worker-b` を追加できます。

### Task bridgeの受入経路は別扱い

上記はticketの指示を直接読む手動起動です。Task通信bridgeは受入済みの限定経路で、受付からは自動接続しません。
Codex A / 固定reviewer / Cursor Bのread-onlyに加え、固定ticketと別worktreeを使う監督付き編集が対象です。Cursor BのShell/MCP/WebFetch denyは維持します。
bootstrap専用ticketで待機させ、readiness→worker-start成功→host armの順を確認して初めてlive Taskへ結び付けます。
armの成功だけではTask受入成功ではありません。詳細は
[単一Dispatch通信bridge](development-infra/orca-development.md#単一dispatch通信bridger3固定reviewercodex-acursor-b-read-only実task受入済み) を参照してください。
編集は受入済みの固定ticket・別worktree・専用launcher経路だけで行います。通常のagent起動へ読み替えないでください。
`escalation`のJSON整形差はcandidateでJSON内容比較へ修正し、空白・キー順・escape差の同値と
値・型・Task相違の拒否を回帰確認済みです。Orcaが未指定本文をreceipt上の空文字へ補う差も正規化し、
Codex Aでheartbeat・ask timeout後の同一質問resume・escalation・最終check・worker_doneを受入済みです。
Cursor Bはlauncher所有のprivate Unix socketと3つの公式hookだけを使い、generationをlive Dispatchへ拘束します。
結果JSONが不正な場合は、上流mutation前にcontroller固定文を1回だけ返して整形を求め、2回目はfail-closedで停止します。
実受入ではheartbeat・check・worker_done、settlement、capability失効、Delivery ACK、source不変を確認しました。
後続L3EではA/Bそれぞれの限定編集と2レーン同時実行も受け入れました。
Linearの課題管理や統括相談の導入は、このbridge受入と分けて進めます。

## 5. 検証して専任reviewerへ渡す

1. workerの報告と実際の差分・未追跡fileを統括が確認します。
2. 当該worker/launcherの終了とworkspace枠の解放を確認します。terminalがidleでも枠は保持しています。
3. 統括が対象worktreeのguard付きdriverで必要な検証を直列実行します。
4. 同じ対象用のreview ticketを作成します。`allowed_directories: []` とし、fingerprintを `source_sha256` に設定します。
5. reviewerを起動します。2回目以降は保存した同じsession UUIDを明示します。

```bash
python3 /absolute/worktree/scripts/orca_roles.py fingerprint --ticket /absolute/review-ticket.json
python3 /absolute/worktree/scripts/orca_roles.py launch --ticket /absolute/review-ticket.json --slot reviewer
# 2回目以降（上の新規起動とは同時に実行しない）
python3 /absolute/worktree/scripts/orca_roles.py launch --ticket /absolute/review-ticket.json --slot reviewer --resume-session <UUID>
```

第3batchのlauncherは終了時にreviewerのsession UUIDとticket IDを表示し、制御台帳へ固定します。
終了まではslotを保持し、新規/別UUIDへの切替を許しません。
固定reviewer・Codex A・Cursor Bはread-only実TUIで正常終了・同会話再開を確認しました。
Cursor Bの起動時設定保存エラーとread-only Task接続に加え、A/Bの限定実編集、編集時のwrite境界、統括検証、固定review、2レーン同時実行を修正・受入済みです。
再開時のUUIDが不明なら新reviewerを作らず、既存履歴を確認します。
レビュー中にsourceやHEADが変われば再レビューです。指摘修正は統括または実装担当が行い、reviewerは編集しません。
編集workerの初回起動はclean必須です。第3batchでは、前回のprocess終了と会話IDが保存され、
ticket全文・担当範囲・branch/base・source/index・会話履歴が一致する場合だけdirtyのまま再開できます。
元ticketを書き換えず、今回の追記を別fileに用意します。

```bash
python3 /absolute/worktree/scripts/orca_roles.py launch --ticket /absolute/task-ticket.json --slot worker-b --resume-session <UUID> --follow-up-file /absolute/follow-up.txt
```

`worker-a` も同じ形式です。外部編集・commit・stagingで対象が変わった場合は停止し、統括へ戻します。
自動reset、別sessionへのfallback、元指示の自動再送はありません。これは手動launcherの継続条件であり、
Orca Task/Dispatchへの自動接続や重複送信の受入を完了したものではありません。

読み取り専用の確認には、ticketに `read_only: true`、`allowed_directories: []`、
現在の `source_sha256` を設定します。dirtyでも観察できますが、前後でsourceが変わると無効です。
Cursor BはAsk modeと書込み禁止policyで起動します。再開時にticketを書き換えて編集可能にはできません。

終了コード0や `approved: false` を承認と読み替えてはいけません。
統括がreviewerの判断、同一対象の検証証拠、必須指摘の解消を照合してから、
[承認記録の整合性検査](development-infra/orca-development.md#レビューと採用)を実行します。
ゲーム変更のcommit・統合・公開は、ユーザーが許可した範囲だけ行います。

## 6. 停止・再開・困ったとき

| 状態 | 対応 |
| --- | --- |
| slot / workspace / host がbusy | 所有terminal/processを確認し、既存処理を待つ。lock削除やraw起動で迂回しない |
| RAM・disk不足 | 新規投入を止める。使用中cacheを一括削除せず、storage台帳で終了済み用途だけ整理する |
| ticketのbranch/base不一致 | 現在の対象を確認してticketを再発行。既存成果をresetしない |
| sandbox・認証・setup失敗 | 出力を統括へ渡して停止。無制限権限や裸のCodexで代用しない |
| reviewerのcontext/UUID不明 | 新規投入を止め、仕様・対象fingerprint・未解決指摘と既存sessionを確認する |
| workerの再開でsource/index/session不一致 | 変更を保全して統括へ戻す。ticket IDの再利用や履歴の書換えで迂回しない |
| role stateがunknown / 起動途中のまま | 同じslotでの新規起動も停止。processと成果を統括が照合する。通常の失敗初回は[限定照合](development-infra/orca-development.md#読み取り専用workerと失敗初回起動の照合第4batch)、Task bridgeはDispatchのabandon・capability失効・exact terminal終了を確認して専用`reconcile-bridge`だけを使う。pending mutationがある場合は`request-show`のreceiptとjournal IDも照合し、Task結果や承認にはしない |
| CLIが応答していてもOrcaのtui-idleがfalse | Taskを投入しない。対話promptやOrcaの認識状態を確認し、裸のagentや直接Dispatchで迂回しない |

統括向けの [read-only通信診断](development-infra/orca-development.md#制限通信の診断入口r3第1batch) を追加した。
対象terminalを1件に固定して公式CLIの接続と待機を確認する。LLMやTaskは起動しない。
通信確認が成功しても `dispatch_allowed: false` のままで、自動運用を有効にする操作ではない。

一時中断では、task ID・worktree・HEAD・担当範囲・terminal handle・reviewer UUID・残件を統括へ残します。
Orca再起動後のterminal handleは再取得し、古いhandleへ重複送信しません。
終了時は必要な成果と検証を確認し、現在の利用者がいない専用作業場だけ整理します。
保持中のreview cacheには日数だけの削除期限を設けません。

## 関連文書

- [分離開発の運用仕様](development-infra/orca-development.md)：ticket、権限、資源制御、承認の詳細。
- [導入・検証計画](plans/orca-parallel-development-plan-2026-09-20.md)：検証済み範囲と未実施項目。
- [検証データの整理](development-infra/validation-storage-workflow.md)：保持・終了・cleanupのルール。
