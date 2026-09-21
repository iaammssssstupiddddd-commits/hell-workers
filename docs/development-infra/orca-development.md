# Orcaによる分離開発の運用

更新日: 2026-09-21。対象: Orca 1.4.205 / Linux / Codex CLI 0.155.1 / Cursor CLI 2026.08.04-aaa8809。

日常操作は [Orca 運用ガイド](../orca-quickstart.md) を入口にする。本書は権限・ticket・資源管理の詳細仕様。

## Linear受付の運用方針

依頼受付と人向けの進捗はOrca標準のLinear連携を使い、以下の既存guardを再利用する。
実装順・完了条件は[現行計画L0〜L4](../plans/orca-parallel-development-plan-2026-09-20.md)を正本とする。
Linearのworkspace/team読取りに加え、専用試験issue `TAK-5` の作成・コメント更新・再読・worktree関連付けを受入済み。
入力adapterはcandidateへ実装済みで、`TAK-5` の固定snapshot取込、初回統括相談、同一session追記も受け入れた。
編集Task bridgeと受付からTaskへの監督付き受け渡しcontrollerはcandidateへ実装・tooling検証済み。
実Linear課題を使う編集→検証→review一巡と権限/通信異常系の実受入は未完。
独立したCodex固定reviewer、Codex A、Cursor Bのread-only実Task lifecycleは受入済み。
別worktreeのA/B限定編集、統括検証、固定reviewer、2レーン同時実行も受入済み。
以下の現行launcherと手入力fallbackの実績を区別する。

- Linearは依頼・優先順位・結果要約、Orcaは作業場・terminal・監督付きTaskを管理する。
  固定ticket、実process/session、排他、承認対象の正本は既存host台帳に残す。仕様正本はprimary `docs/`。
- `host_coordination`、role/provider/state、source fingerprint、固定reviewerとread-only preflightを再利用する。
  Linearの担当・ラベル・Doneはscope権限、process終了、検証成功、レビュー承認に読み替えない。
- 初期は標準UI/`orca linear`の明示操作だけを使う。認証値はOrcaの設定で管理し、workerへ渡さない。
  統括相談agentにも全権の外部更新権限を追加しない。Webhook・独自MCP・常時双方向同期は初期対象外。
- `scripts/orca_issue_context.py` は公式 `orca linear issue --full --json` だけをshellなしで実行し、
  workspace/issue IDと採用本文のhashを固定して既存frontdeskの`submit()`へ渡す。
  ローカルrequest UUID、ticket ID、会話UUIDは別のIDとして対応付ける。既存coordinatorのslot・再開・復旧は維持する。
  Linearの後編集を実行中ticketへ上書きせず、明示追記または新snapshotとして扱う。
- frontdeskのqueued台帳は内部snapshot保管として再利用し、Linear進捗の複製にはしない。
  preflight/bridge/role stateが使うowner-only・atomic保存関数を含むため、menuと一緒にmodule全体を削除しない。
- 手入力menuは開発途中の診断・復旧fallbackであり、移行対象の独立運用ではない。通常入口をLinearへ統一し、
  fallbackから同じ依頼を重複投入しない。外部反映に失敗してもworkerを再実行せず、実結果とLinear反映待ちを分けて扱う。
- Task連携は、Codex bridgeの固定reviewerとCodex A、Cursor hook bridgeのCursor Bについて
  read-only一巡と監督付き限定編集を完了し、受付から同じ専用経路へ接続するcontrollerも実装・tooling検証した。
  controllerはworker完了後の検証、review ticket作成、採否、commit、Linear更新を代行しない。
  A=Codex/B=軽量Cursor/固定reviewerの構成を維持する。共有checkout/background編集は禁止し、
  primaryルールが許す専用launcherの限定例外だけを使う。通常のOrca agent起動へ許可を拡大しない。

candidateの環境整備受入はゲーム実装テストを対象外とし、関連Python/連携/文書・storageの検査に限定した。
Rust/Bevyのbuild・Clippy・workspace test、Blender実行、ゲームnative/GPU/performanceはcandidateでは実行していない。
変更範囲gateが選んだBlender用Python tooling testは実行対象に含む。primary文書commit後のgateはcontrol文書を理由に
deps/rustも自動選択してpassしたが、native/window/GPU受入は行っていない。
通常のゲーム変更の検証規則を緩和するものではない。過去の全群結果は当時の証拠として下記に保持する。

標準機能の根拠は[Orca Linear仕様](https://www.onorca.dev/docs/review/linear)。
公式CLIでworkspace `takumi sato`（`68abc67b-ca1b-407b-be63-99dd91321b26`）とteam `TAK`の読取りを確認した。
利用対象・書込み範囲は専用試験issue [`TAK-5`](https://linear.app/takumi-sato/issue/TAK-5/orca-integration-acceptance-hell-workers)
自身の説明・コメントとworktree関連付けに限定し、作成・コメント更新・full再読取りを確認した。
課題からの通常agent起動で専用launcherを迂回しない。

## 適用状態

環境実装は専用branch `iaammssssstupiddddd-commits/orca-parallel-development` の
`/home/satotakumi/orca/workspaces/hell-workers/orca-parallel-development` にある。
比較baseは `b68bafd7358580ff8ad77962fa02987a215b5b60`。
ユーザーの許可により基盤24 fileだけをローカルcommit
`d85dba0f17e21f96a385fdec9e2b660393692a5f` として採用した。
受付・統括相談・Cursor Bのcode/rule/test 19 fileもユーザーの指示で
`a03c4d5401f0486427aac81c0a91f335f79468bc` へ追加commitした。
第3batchの同一task再開・固定reviewer拘束も全群gate後に
`074f47bcb260831b92ee9dfe0041b9079bf2c4a9` へcommitした。
第4batchのread-only worker、Cursor設定分離、終了確認済みの失敗初回起動の照合も全群gate後に
`5abe7db6294f4ebbad07faa1cee6aa2985faf85c` へcommitした。
R3第1batchのread-only通信preflightとtest 2 fileは全群gate後に
`d06e912e68d1c933144d7ff98c4d535082ec683d` へローカルcommitした。
R3第2batchのCodex read-only単一Dispatch bridgeは全群gate後に、code/test 5 fileを
`a7c1bbbcb9ff8e1ac3321f52780d655c18237d82` へローカルcommitした。
Linear固定snapshot adapter・受付UI・専用排他とP2修正は、tooling限定検証後にcode/test 8 fileを
`6210b885b429341b5a9101c77332d57af008b0d8` へローカルcommitした。
実Task失敗時のexact identity照合、mutation-free限定復旧、task-private CLI wrapperを
`bdf0ea8bdab1e39ff14605aa10b5debfc4947714` へ、Codex内側sandboxとローカルOrca IPCの競合回避を
`1673c6be9d737fc10228ae635557f95c7b687547` へローカルcommitした。実receiptの空本文正規化と
fenced Dispatchの曖昧mutation照合を`fca4fd43bda7696246be481039af4d6469f9a4c5`へ、test state隔離を
`422a74d6`へcommitした。Cursor hook bridge、失敗照合、同時hook直列化、generation拘束、結果整形の一回限りの
再試行を`1c11b068`〜`29b9cb51`の7 commitへ追加し、hookのGit実行属性を
`09642de4022577fd442c4c9971c64a4d1f649e26`で修正した。実編集受入用のA/B分離fixtureと、
Cursor Bの`acceptance-edit`を専用fixture 1 directoryだけに限定するadmissionを`0a32de46`へ追加し、
fixtureをtooling分類可能な形式へ直した`85cf28431a7fa367367d5bd933bf6709024c0ee7`に続き、
実編集時のCodex二重sandbox、tracked project設定のreview可視性、fresh MCP設定、限定失敗復旧を
`3d6e2032`、`e5ff06dc`、`8ddad3ce`、`55b27f6e1d9103d7985941c3cbbf135c7299be91`で修正した。
編集workerのTask bridgeを`6cba754fc9a58c8adcccde143425554da73460b6`で有効化し、Linear受付・成功済み統括相談・
固定ticketからRun→専用terminal→worker-start→bridge armを順序付けるcontrollerを
`5cb73cac`で追加した。このcommitを新規worktreeの最新基点とする。
実Linear課題 `TAK-5` のL1正常系とL2相談継続は受入済み。固定reviewer、Codex A、Cursor Bのread-only実Taskも一巡済み。
実編集はA/Bの直列一巡と、別worktreeでの2レーン同時実行まで受入済み。
primary文書正本の変更は別作業と混在するため、これらの専用branch code commitには含めていない。
primaryへの統合、push、PR作成は行っていない。
文書正本はこのprimaryの `docs/` に置く。primaryの別作業中のsource・ルールは変更していない。
従って、**旧checkoutを含むhost全体で並列開発の安全性を確立済みとは扱わない**。
採用前に対象checkoutを更新し、旧解析backend/旧driverを利用中のsessionと実行順を調整する。

Orcaの当該repoのlocal設定には、次をruntime APIから反映した。

- setup: `python3 scripts/dev.py doctor`（誤った `pnpm install` を置換）。
- `setupAgentStartupPolicy: wait-for-setup`。
- `setupRunPolicy: run-by-default`、`commandSourcePolicy: local-only` とarchive設定は維持。
- target共有、全体のagent既定引数、既存terminal/sessionには変更なし。

candidateの `orca.yaml` も同じsetupと待機順を定義する。local-only設定下ではlocal commandが正本であり、
YAMLを置くだけでlocal overrideが更新されるわけではない。setupは軽い診断だけで、sandbox/検証gateではない。
repoの `worktreeBaseRef` は上記専用branchへ設定済み（設定後の `repo show` でも確認）。
明示的な別baseやparentを指定しない新規treeは、commit済み基盤を継承する。
既存treeには遡及せず、primaryの未commitゲーム変更も含まない。

## 開始入口

この設定は分離作業場と監督付き配車を開始する入口であり、統括LLMの常駐や無監督の自動運用を意味しない。
Codex A・Cursor B・固定reviewerのread-only実TUI起動・正常終了・同UUID再開は確認済みで、
3 roleのread-only Orca Task連携も一巡した。A/B限定編集、編集時のwrite/tool境界、統括検証、
固定reviewer、2レーン同時実行も受入済み。受付からのguard付き配車はtooling検証済みだが、
実Linear課題を使う一巡と共有変更の並列化は未受入で、定常運用の全受入完了とは区別する。
統括が最初の独立taskを選び、次のように基点指定を省略して分離treeを作る（task名は都度一意にする）。

```bash
orca worktree create --repo id:88983a13-d1dd-47a0-8ffb-3772ea15d3f1 --name leaf-task-a --setup run --no-parent --json
```

返されたpathでHEADが上記commitを含み、cleanでsetup成功していることを確認してから、下記のticketを作成する。
`--agent codex` や通常agent起動ボタンは使わず、ticket付きlauncherをterminalの起動commandにする。
初回はworker-a 1件→同じ候補をreviewerで確認→同じreviewer sessionの再開確認の順とする。
この受入は完了済みのため、scopeが独立したtaskに限りworker-a/bを最大2件まで開始できる。
試行taskが未指定の間はagentを自動起動しない。
文書正本はprimaryの本書であり、新規tree内の古いdocsを最新の運用状態と取り違えない。

## 役割と条件付き例外

| 担当 | 上限 | 許される変更・責務 |
| --- | --- | --- |
| 統括 | 1 | 分割、ticket、primary文書・共有契約、build/test、証拠確認、直列統合 |
| worker-a / Codex | 1 | 複雑なtask。別linked worktreeのticketに指定した既存directoryだけを編集 |
| worker-b / Cursor CLI | 1 | 単純な局所taskのみ。Aとは別worktree、同じmount境界を適用 |
| reviewer | 1 | source read-only。指摘と承認判断のみ。workerの修正を自分で実施しない |

同じcheckoutを別roleと同時利用することもworkspace lockで拒否する。
同じreviewer terminal/sessionを再利用し、再起動時はsession UUIDを明示する。
reviewer履歴があるのに新規sessionを暗黙に作ることは拒否する。contextを失った場合は投入を止め、
統括が承認済み仕様・未解決事項・対象fingerprintを再提示し、再レビューする。

編集agentへの委譲は、別worktree・固定ticket・mount境界・最大2 worker・固定read-only reviewer・
統括所有の検証/commitを満たす専用経路だけを条件付きで許可する。
`scripts/orca_roles.py` と `scripts/orca_dispatch.py` の分離worker経路だけを対象とし、
Orcaの通常agent起動ボタンや裸の `worker-start --agent codex` を例外経路にしない。
多段委譲、worker自身のcommit/push、workerによる重いbuild/test/解析server起動は禁止する。
controllerは汎用schedulerではなく、固定された1件の受付・ticket・slotを監督付きTaskへ渡すhost境界である。

### Cursor Bの依頼条件

slotからproviderを固定し、ticketで別providerを指定しても拒否する。Bのticketには次を追加する。

```json
{
  "provider": "cursor",
  "complexity": "simple",
  "task_kind": "test-addition",
  "complexity_reason": "既存patternに沿う単一leafのtestで共有APIを変更しない",
  "acceptance": "対象のregression testを追加し、統括が実行して成功を確認する"
}
```

`task_kind` は `local-fix` / `mechanical-change` / `test-addition`。
scopeは `crates/<crate>/src/<leaf...>` より狭い既存directoryを指定し、`hw_core` / `hw_jobs` /
`hw_world` / `hw_visual` / `visual_test` とsave/load/render/asset/startup/pluginを含むpathは初期対象外。
意味上の単純さは統括が判断し、曖昧なtaskはBへ投入しない。directory検査だけで難易度を自動判定するものではない。

Cursorは `--workspace ... --sandbox enabled --trust` で起動する。`--force` / `--yolo` は使わない。
専用permission policyは担当pathのWriteとReadだけをallowし、Shell/MCP/WebFetchはdenyする。
第4batchではglobal `cli-config.json` とproject `.cursor/cli.json` を分離する。
globalは専用runtime内でCLIがmodel/cache等を保存でき、初期値にschema必須field
`version: 1` / `editor.vimMode: false` を含める。既存global設定やmodel選択は毎回初期化しない。
変更不可のpolicyはcontroller側で生成したpermissions専用JSONとし、sanitized directory全体を
candidateの `.cursor` へreadonly mountする。元のproject設定を混入させず、cwdはrepo rootへ固定する。
policy欠損・内容不一致・追加file・symlink・project directory欠損は起動前に拒否する。
CLIのglobal atomic renameを許してもproject allow/denyが最終値となることを、導入版の実loaderで
networkなし・linked worktree fixtureを使って検査する。配列はunionではなくproject側で置換される。
`scripts.tests.test_orca_cursor_config` はglobalのdenyを空にするtransform/reload後もpolicy不変を確認する。
Cursor未導入CIでは同testをskipする。bundle構造が更新された場合はfail-closedで再調査する。
この試験は実LLMやtool実行の受入ではない。
global/project `.cursor` / `.claude` とGit common側primaryのprovider設定は隔離し、config/data/cacheをslot別に分離する。
元のXDG configにある `cursor/auth.json` だけをread-only mountし、tokenのcopy/表示はしない。
認証更新が必要でread-only writeが失敗した場合は停止し、境界を緩めて再試行しない。
enterprise/team管理hookは別経路であり、全hook停止を保証しない。組織管理policyは勝手に解除しない。
この境界はsource write制限であり、任意network/CPUまで封じる敵対的sandboxではない。

### Linear snapshotと受付台帳

candidateの受付UIは `1` でLinear snapshot、`7` で手入力fallbackを保存する。
Linearの `TAK-5` で実issue取込・統括相談・同一session追記まで受入済み。L4まではfallbackと既存会話を保持する。

`scripts/orca_frontdesk.py menu` は開くだけではLLMを起動しない受付UI。Linear取込・fallback保存・一覧・統括相談・追記を扱う。
台帳はaccount homeの `.local/state/hell-workers/frontdesk/requests.json`、directory 0700 / file 0600。
Linear対応表は `.local/state/hell-workers/linear-intake/snapshots.json` に別保存し、workspace・issue内部ID・
snapshot SHA-256からUUIDv5の受付IDを決める。対応表を先にatomic保存し、その後のfrontdesk保存が失敗した場合は
同じ取込の再実行で照合する。対応表だけでは受付成功・dispatch・承認を示さない。
専用flockとatomic replaceを使い、明示request UUIDの同一本文再送は二重登録しない。
fileと親directoryをfsyncし、replace後の同期失敗では同一UUID再送時にもdirectoryを再同期する。
壊れた台帳、未知状態、symlink/hardlinkは初期化で隠さず停止する。状態は現段階では `queued` のみ。
Orcaの基盤worktreeに「Linear受付・統括相談」terminalを配置した。
受付情報はOrca Run/Task/Dispatchの代替ではない。Task連携はR3の残件。

### 統括相談と同一会話の再開

`scripts/orca_coordinator.py` がread-onlyの `codex exec --json` を選択時だけ起動する。
専用coordinator slotを終了・waitまで保持し、受付state lockは短い読取りだけで解放する。
元のCodex設定/MCP/Orca transportを継承せず、外側mount境界で全sourceをread-onlyにし、
account内のHell Workers制御state・他role runtimeを隠して自分のruntimeだけを再公開する。
source編集やTask投入を相談agentの出力だけで実行しない。

- 制御state: `frontdesk/consultations/<request UUID>.json`、0700 directory / 0600 file。
- provider runtime: `agents/coordinator/<request UUID>/`。制御stateとは別で、このruntimeだけrw。
- spawn前にturn UUID、prompt digest、repo/branch/common dir/HEAD/fingerprint、terminal/workspaceを保存。
- 今回の `thread.started` UUID → `turn.started` → `turn.completed` → exit 0 → session metadata一致を要求。
  UUID欠落・重複・誤順序・途中EOF・過大event・非ゼロ終了は成功にしない。
- 初回相談は受付IDから決まるturn UUIDで重複排除する。追記は明示turn UUIDを使い、同一ID/本文の再送は
  保存済み回答を返す。違う本文で同一IDを使うと拒否する。
- 追記は正確なsession UUIDをresumeし、元依頼本文を再注入しない。`--last`、新規sessionへのfallbackは使わない。
  branch/common dir/受付本文が変わった場合は別の受付が必要。sourceが相談中や回答後に変わった場合は注意を表示する。
- 異常はunknownのまま保全。`recover` はcontrollerが子processの終了を確認・保存でき、既知sessionを照合できる場合だけ、
  元turnを再送せず新しい照合turnを作る。SIGKILL/電源断等で終了証拠がない場合は停止し、手動調査を必要とする。
- stdinは一時file、stdoutは逐次JSONL、stderrは端末へ直接出す。1 event 4 MiB、回答128,000文字、1 turn 600秒を上限とする。
  上限はagent実行のガードであり、保存cacheの自動削除期限ではない。

CLI利用時は `consult --request-id <UUID>`、`consultation --request-id <UUID>`、
`follow-up --request-id <UUID> --turn-id <新UUID> --request-file <本文file>`。
復旧は `recover` に同じ3引数を渡す。これらは `python3 scripts/orca_frontdesk.py` のsubcommand。
利用時にモデル名・reasoning effortは強制しない。新規依頼同士の会話は分け、同時LLMはcoordinator枠1件。

実UIで初回応答、メニュー終了・再起動、同一UUIDでの追記応答を確認した。
相談ターンの成功はrequest queuedを変更せず、実装・検証・review承認の証拠にしない。

`TAK-5` の受入ではsnapshot SHA-256 `5ece754b065de0771944c9e61d938931c9f0dd8201286b66d77736b2d8d16e0f`から
受付ID `0e283873-4e4e-5a96-b4fd-ba103a724f44` を確定した。初回turn
`32f365ab-9a0b-588f-8f4b-945a1fdb38b9` と追記turn `3a0a489c-ec28-431e-9a9e-94e63a1f534f` は、
同じsession `01a0c090-3de7-7923-8094-e78db4e3913d`、HEAD `1673c6be`、source fingerprint
`c227c45adf25aac42c51598fc2b3d6d1d9a5fa50df45a016fc513145f1c2d131` でexit 0・source不変だった。
受付はqueued、Run/Task/Dispatch IDはnullで、自動投入・承認・Linear status同期は発生していない。
同じsnapshotの再取込は同じ受付IDを返して台帳1件を維持し、同じturn UUID/本文の再送は保存済み回答を返してCodexを再起動しなかった。
その後の説明更新はCLIが`linear_write_unconfirmed`を返したため再送せず、full再読取りで反映済みと確定した。
更新後snapshot `26c213cac8f8ab1ca50275a586cae09fe9287d061c3825457e9d9fb756376c47`は既存相談を上書きせず、
別受付ID `28971b6b-f77c-5a61-9d7e-3f8dc1cbf00c`としてqueued保存した。旧相談sessionは変更していない。

## 依頼票と起動

統括がOrca CLIで別worktreeを用意し、対象branchが本基盤を含むことを確認する。
編集worker初回開始時はclean checkoutが必要。既存のdirty成果を消して通してはいけない。
CLIはOrca内では `orca` shim、外では `orca-ide`。外部の `/usr/bin/orca` はGNOME Orcaで別製品。

ticket JSON例（path/branch/baseは実在する対象へ置き換える）:

```json
{
  "schema": 1,
  "id": "leaf-task-a",
  "repo": "/absolute/path/to/linked-worktree",
  "branch": "task/leaf-a",
  "base": "対象HEADの40桁SHA",
  "allowed_directories": ["crates/hw_core/src/example"],
  "prompt": "変更目的、成立条件、変更禁止事項、必要な検証をここへ記す"
}
```

primary、branch/base不一致、root全体・docs・target・Git/agent設定等の保護領域、
重複directory、既存symlink/hardlinkを含む書込み範囲を拒否する。root-level設定や共有型を含む広い変更は
無理にworkerへ割り当てず統括が編集する。初回は独立したleaf変更を選ぶ。

candidateのlauncherを統括terminalから実行する:

```bash
python3 scripts/orca_roles.py launch --ticket /absolute/ticket.json --slot worker-a --dry-run
python3 scripts/orca_roles.py launch --ticket /absolute/ticket.json --slot worker-a
```

`--dry-run` は引数確認だけで、実際のlock/sandbox起動成功を証明しない。
launcherはmodel/effortを指定しない。Codexの引数はworkerが `--sandbox workspace-write`、reviewerが
`--sandbox read-only`、共通で `--ask-for-approval never --disable multi_agent`。
追加の外側bubblewrap mount境界がsource/Git/他worktreeへのwriteを制限する。
Linux/bubblewrapが利用できなければ安全性を落として再試行せず停止する。

### 同一taskの継続と固定reviewer台帳（第3batch）

`scripts/orca_role_state.py` がaccount stateの `role-state/<slot>.json` に制御状態を保存する。
この台帳はagentのrw runtime外にあり、0700 directory / 0600 file、atomic replaceとfile/directory fsyncを使う。
worker runtimeは `agents/<slot>/tasks/<task-key>/`、reviewerは `agents/reviewer/` の固定runtime。
task-keyはcommon Git directoryとticket IDから生成し、別slotへの暗黙再割当は
`role-state/assignments/<task-key>.json` とtask leaseで拒否する。

- 初回の編集workerはclean、継続は同じticket全文のdigest・provider/slot・repo/common dir・branch/base・
  scopeと正確なsession UUIDを要求する。Bのcomplexity/acceptanceもticket digestに含む。
- 起動前intentを保存し、子processの終了をcontrollerがwaitした後だけcheckpointを保存する。
  checkpointはsource/index fingerprintと会話fileのdigestを持つ。外部編集やstagingを継続成果に混ぜない。
- worker再開は `--resume-session <UUID> --follow-up-file <本文file>` を指定する。元task本文を再送せず、
  明示追記だけを渡す。ticket書換え・任意dirty許可・新session fallbackはない。
- 起動前/終了後にscope・branch/baseを再検査する。staged差分、範囲外変更、renameの元/先を拒否する。
  NUL区切りのGit pathを扱い、改行を含むpathも検査する。
- reviewerは初回UUIDを永続固定し、同じrepositoryの別worktreeへ移る場合もそのUUIDを使う。
  Codexの初回session metadataのcwdはoriginとして保存し、現在のレビュー先との違いを許容する。
- Codexは唯一のrolloutとsession metadata、Cursorは
  `cursor/chats/<md5(canonical cwd)>/<UUID>/store.db` のhex JSON metadataを照合する。
  Cursorはread-only SQLite接続でcommitted WALも読み、接続終了後にDB/WALをhashする。SHMはhash対象外。
  導入版の実serializer・workspace keyをoffline compatibility testで照合する。
- 初回/継続とも0個/複数/別session、履歴の外部変更、欠損台帳、保存失敗、途中中断では停止する。
  `starting` / `unknown` は同slotの新規taskでも迂回できない。自分の子はTERM→必要ならKILL→waitしてからslotを解放する。
  未確認processや破損stateの自動修復は行わない。実TUIのCtrl-C/強制終了の受入はR3/R4へ残す。

`recorded` はlauncherの終了と会話/checkpoint保存を表すだけで、Task完了・turn成功・review承認ではない。
非ゼロexitも値を保存して統括へ返す。模擬providerでの成功を実Codex/Cursorの作業受入に読み替えない。
Orca Task/Dispatchはまだ作成しないため、既存受付の相談とこのlauncherを自動で繋ぐものではない。

### 読み取り専用workerと失敗初回起動の照合（第4batch）

A/Bに `read_only: true`（厳密なboolean）、`allowed_directories: []`、現在の
`source_sha256` を与えると、dirty sourceを変更せずに観察できる。
Codexはread-only sandbox、Cursorは `--mode ask` と `Write(**)` denyを追加し、
いずれも外側mountでsource書込みを許さない。Bの単純さ・理由・受入条件は依然必須で、
会話の起動/再開試験にはread-only時だけ `task_kind: acceptance-probe` を使える。
同一task/session拘束はそのままで、途中でread_onlyを解除したticketへ変更できない。
前後のfingerprintが変わると結果はunknownとなり、並行編集を成功に混ぜない。

`abandon-start` は自動復旧ではない。統括が起動出力、差分、終了証拠を照合した後だけ使う。
対象はread-only workerの初回失敗で、exact attempt ID、元ticket/assignment一致、nonzero exitと
process_exitedの保存、bound sessionも実履歴もないこと、現sourceの明示照合を全て要求する。
reasonと旧attemptを `abandoned` に保存し、旧ticket IDの再実行を拒否する。履歴やruntimeは削除せず、
processの起動も承認も行わない。必要なら原因修正後に別IDの明示ticketを作る。
編集worker・固定reviewer・終了未確認・exit 0・保存会話がある失敗には使えない。
Task bridgeを付けたattemptも対象外。providerが履歴を作らず非ゼロ終了しても、
Orca側の未settled Dispatchがない証明にはならないため、監督付きlifecycleの照合を先に行う。
`--dry-run` との併用は台帳変更前に拒否する。

```bash
python3 scripts/orca_roles.py abandon-start --slot worker-b --ticket /absolute/failed-ticket.json --attempt-id <失敗attemptのUUID> --observed-source <現在のfingerprint> --reason '<出力・終了・sourceを照合した根拠>'
```

Orca `tui-idle` とCLIの応答/終了は別の証拠。現版ではモデル切替案内を閉じた後も
`agent-interactive-prompt` が残る事例を確認しており、Taskの投入条件を満たしたとは扱わない。

### 制限通信の診断入口（R3第1batch）

`scripts/orca_preflight.py` は統括が明示実行するread-only診断で、Task lifecycle bridgeではない。
Orca 1.4.205 / Linux bubblewrap / local Unix transportに限定する。実行file・metadata directory・
対象terminal・canonical worktreeを全て明示し、別CLIや別terminalへfallbackしない。
以下は基盤worktreeのterminalから実行する。primaryにはcodeをまだ統合していない。

```bash
python3 scripts/orca_preflight.py \
  --orca /home/satotakumi/.config/orca/linux-orca-cli-shim/orca \
  --metadata-dir /home/satotakumi/.config/orca \
  --terminal <現在の対象terminal-handle> \
  --repo /home/satotakumi/orca/workspaces/hell-workers/orca-parallel-development
```

- hostだけが本物の接続tokenを読み、隔離内の公式CLIには使い捨てproxy専用tokenを渡す。
  proxyはruntime ID、terminal handle/incarnation、worktree ID/pathへ拘束する。各RPC前とwait後に再照合する。
- 許可RPCは `status.get`、exact terminalの `terminal.show`、`terminal.wait` の `tui-idle` だけ。
  追加field、別対象、未知RPC、runtime/token交代、remote control、版違い、未接続を拒否する。
  Run/Task/Dispatch作成・send・check・ask/reply・preamble注入は許可しない。
- bubblewrap内のsourceはreadonly、元metadata/socketを隠し、network/PID namespaceと環境変数を分離する。
  選択した公式CLIを同じpathで渡し、元設定directory全体を再公開しない。LLMは起動しない。
  account全体のread機密性や敵対的実行を保証するsandboxではない。
- raw応答・preview・title・認証値は返さず、確認済みidentityと固定schema/enumだけを射影する。
  JSON重複key、非有限値、型違い、部分frame、過大出力を拒否し、そのprobeを失効させる。
  frame/CLI各streamは256 KiB、proxy requestは16件、wait指定は最大15秒。
  keepaliveで延びないdeadlineを持ち、過大出力・timeout時はCLI process groupを停止して回収する。
- `.local/state/hell-workers/preflight/p-*` のmetadata/socketはowner-onlyで一時作成し、
  CLIとserverの終了後に削除する。会話runtimeや既存credentialは変更・削除しない。

既定は通信と対象同一性だけを確認し、`readiness: "not-checked"` を返す。
`--wait-ms 1000` 等を追加すると待機観測も行う。`transport: "verified"` / exit 0だけでは投入可能ではない。
正規の `satisfied: false` も観測成功として返す。timeoutなど観測できない場合は一般化した拒否とexit 1で停止する。
**全結果で `dispatch_allowed: false`**。trueの待機結果でも実agent応答・監督開始・review承認の証拠ではない。

実runtimeで受付terminalのstatus/showを隔離内CLIから確認した。受付menuに対する1秒のwaitは拒否となり、
投入は0件のまま。模擬runtimeと同じ公式CLIではfalse応答、identity交代、異常JSON、出力量/deadline、
metadata/socket境界、終了時cleanupを確認した。実LLMの起動待ちを解消した受入ではない。

readinessの一次source調査では、1.4.205のprompt検出が末尾12非空行を参照し、
通常の `›` が現れても先行する確認案内を消さないことを再現した。
ANSIの画面全消去・絶対位置移動の扱いも完全なterminal emulatorではない。
以前の失敗terminalの該当tailを採取していないため、これを今回の原因と断定しない。
偽headerの出力、行埋め、外側隔離なしのsandbox解除、idle未達での直接Dispatchは解決策にしない。
続く第2batchでTask専用policyを追加する。実agentのreadiness再観測・監督付き受入は依然必要。

### 単一Dispatch通信bridge（R3・固定reviewer/Codex A/Cursor B read-only実Task受入済み）

既知P2はcandidateで修正済み。`escalation`の空白付きraw JSON payloadを導入版Orcaが再serializeしても、
payloadだけをduplicate key・非objectを拒否する厳密なJSON内容比較で照合する。
空白・キー順・escape差の同値と、値・型・Task相違の拒否を模擬runtimeで回帰確認した。
ID/capability/type/subject/bodyの照合は緩めていない。Orca 1.4.205が未指定のsend本文をdurable receiptで
空文字へ補う場合だけ未指定へ戻し、callerからの空本文入力は引き続き拒否する。編集roleへはまだ本運用しない。
Linearの標準課題管理（L1）と統括相談（L2）は、このTask bridgeの完了を前提にしない。

`scripts/orca_task_bridge.py` はCodex A / 固定reviewer / Cursor BをOrca Taskへ接続する専用通信経路。
`scripts/orca_dispatch.py` はimmutableなLinear受付、成功済み統括相談、固定ticketを照合して
Run作成→role terminal作成→既存terminal指定の`worker-start`→private bridge armを順序付ける。
同じrequest/ticketのarmed状態は同じ記録を返し、pending/unknownは自動再送しない。
承認・レビュー判定・検証・commit・Linear更新は代行しない。dry-run ticketとの組合せは起動前に拒否する。
CursorのShell/MCP/WebFetch denyは変更しない。
Task bridge付きCodexはCodex自身の内側sandboxを使わず、既存の外側bubblewrapを強制境界とする。
これは内側sandboxがUnix socketによるOrca IPCを拒否したためで、通常起動には適用しない。
外側はrootをread-only bindし、role runtime以外のprivate stateをmaskし、Task専用proxyだけをread-onlyで再公開する。

通常運用はOrca管理下の統括terminalで受付menuの`8`を選ぶ。CLIから同じcontrollerを使う場合:

```bash
python3 scripts/orca_dispatch.py start \
  --request-id <受付UUID> \
  --ticket /absolute/task-ticket.json \
  --slot worker-a
```

`worker-b`も同形式。reviewerは`read_only: true`、空の`allowed_directories`、exactな`source_sha256`を持つ
review ticketで`--slot reviewer`を指定する。保存済み固定reviewer sessionがある場合、controllerが同じUUIDを再利用する。
`ORCA_TERMINAL_HANDLE`がない環境では`--coordinator <統括terminal-handle>`を明示する。

以下はbridge単体の受入・診断で使うlauncher引数であり、通常の開始入口ではない:

```bash
python3 scripts/orca_roles.py launch --ticket /absolute/readonly-bootstrap-ticket.json --slot worker-a \
  --bridge-orca /home/satotakumi/.config/orca/linux-orca-cli-shim/orca \
  --bridge-metadata /home/satotakumi/.config/orca
```

ticketは `read_only: true` / 空scope / exact source fingerprint、本文はbootstrap待機専用とする。
固定reviewerの場合は `--slot reviewer` と既存の正確な `--resume-session` を使う。別reviewerを新設しない。
起動順は **bootstrap → 実agentのreadiness → hostのworker-start成功receipt → host arm → 通信 → settlement → 終了・照合**。
`tui-idle` がfalseのままRun/Taskを作らず、直接Dispatch注入で迂回しない。

launcherが表示する `orca_bridge` UUIDに対し、hostだけが次を実行する。

```bash
python3 scripts/orca_task_bridge.py arm --bridge <UUID> --run <Run-ID> --task <Task-ID> \
  --dispatch <Dispatch-ID> --coordinator <統括terminal-handle>
```

これはhost命令を保存するだけで、live identity検査・worker-start・Task受入の成功receiptではない。
最初のlifecycle RPC（mutation）時にterminal incarnation / source / runtime / latest Dispatch / supervised workerのready状態を照合する。
bootstrapのstatus照合はarm前でも可能。注入とarmの間の初回mutationは最大5秒、**上流未送信のまま**待てる。
期限切れ・不一致・不明結果はunknownとし、
同じ要求や元preambleを自動再送しない。armは1回だけで、未照合の値の上書きは禁止。

- workerへ渡すのはproxy専用tokenと公開metadata/socketのみ。本物のOrca transport認証はhostから出さない。
  live preambleのexecutable / handle / Task / Dispatch / capabilityを変更・再生成せず使う。
  `ORCA_TERMINAL_HANDLE` 等は隔離内に継承せず、CLIに `--from` / `--terminal` を明示する。
- 許可するのは最小status、heartbeat、escalation、worker_done、worker自身のcheck、統括へのask/resume。
  send/askはlive capabilityをそのまま上流へ渡す。checkのrun指定・別宛先・未知field・Task作成・gate操作は拒否する。
- 接続規約としてask/checkの待機は `--timeout-ms 10000` を明示する（許容1〜15000ms）。
  1.4.205のpreambleにある600000msのask例はこの経路では使用できない。preamble自体のauthorityは書き換えず、
  短い有限待機で同じ未回答message IDをresumeする。回答済みの別質問へ戻って現在の質問を消すことはできない。
- checkのDeliveryは全件を処理した後に明示ACKする。未観測ACK、未回答質問や未ACK Deliveryを残したdoneは拒否する。
  公式CLIのask timeoutはJSONのokがtrueでもexit 1。途中のaccepted receiptも回答・承認へ読み替えない。
- mutation request ID / 内容digest / capability hashをhost journalへ保存し、上流送信前にpendingをfsyncする。
  同一の確認済みrequestは現在のgenerationを再検査して保存receiptだけを返す。上流へ再送しない。
  内容変更、未知upstream replay、送信後の不明結果は失効し、人による照合待ちにする。
- worker_doneは実runtimeのcompleted/failed verdict、Task/Dispatch ID、settled workerとcapability失効を照合する。
  exit 0・メッセージenqueue・bridge終了をTask成功やreview承認と同一視しない。
- 1.4.205のcheck APIにはexpected Dispatchを原子的に指定する引数がない。runtime/terminal incarnationの
  協調leaseを持ち、bridgeを失効してin-flight要求を排出しroleが終了するまで、同terminalを再配車しない。
  生のOrca CLI/UIからの別controller操作まで原子的に防げる保証ではない。専有できなければ接続しない。

保存先はaccount homeの `.local/state/hell-workers/task-bridges/<UUID>/`。
`identity.json` / `arm.json` / `journal.json` はhost限定、`p/` 内のmetadata/socketだけをworkerへread-only mountする。
他roleの会話やproxy tokenを隠すため、Hell Workers state親全体をmaskして自分のruntimeと公開部分だけを戻す。
RPCは最大256件、既存の256 KiB frameと有限deadlineを使用する。終了時は受付を失効し要求を排出した後、
socketとproxy credentialを除去する。起動途中の失敗でも同じ後処理を行う。
認証後の待機中は公式形式のkeepaliveを1秒間隔で返し、askのCLI inactivity timeoutを保つ。
一方、worker RPCを1件受信してからの処理全体に対するhostの45秒期限は延長せず、各上流RPCへ同じ期限を渡す。
bridge全生存期間の上限ではない。期限切れは成功ではなくunknown。
arm指示後にworkerが一度も通信しないまま終了した場合もunknownで、通常のrole checkpointや再開を許さない。
host journalのownerは統括、consumerは未決attemptの照合、next_actionは実receiptとの照合、
release_whenはsettlement/明示終了と必要な結果の正本文書への集約。恒久的なjob archiveは作らない。
失敗Dispatchがhost側でabandon済み、capability失効済み、exact terminal/launcher終了済みの場合だけ、
`python3 scripts/orca_roles.py reconcile-bridge ...` で失敗事実をrole stateへ記録できる。journalがmutation-freeなら
従来どおり照合し、pending mutationが残る場合はauthority一致・未settled・pending IDの形式と実runtimeの失敗状態を確認して
曖昧request IDを証跡へ残す。`orca orchestration request-show --request <ID> --json`で上流receiptも別途確認し、元要求は再送しない。
この操作はTask結果・レビュー承認を作らず、live runtime、exact terminal/process incarnation、最新または直接retryの
Dispatch、capability失効をすべて再照合する。条件を満たさないunknownは保全して手動調査する。

模擬runtime＋導入済み公式CLIと実bubblewrapで通信schema・ACK・質問再開・完了verdict・隔離/終了境界を検査した。
さらに実Orca Run `run_d7dd7dbe187e` / Task `task_3193fbe0521b` / Dispatch `ctx_21cdd586d0a4` で、
固定reviewer session `01a0bfc7-0a67-7c33-a55f-b215b411c3de` がHEAD `1673c6be`をread-only確認した。
最初と最後のcheck、source読取り、`worker_done` succeeded、Task/Dispatch settlement、capability失効、
coordinator Delivery ACK、role exit 0と同一session記録まで一巡し、変更fileは0件だった。
さらにHEAD `fca4fd43`のCodex Aでheartbeat、ask timeout後の同一message ID resume、統括回答、escalation、
指定source読取り、最終check、`worker_done` succeeded、capability失効、role exit 0を一巡し、変更fileは0件だった。
Cursor BはCLIへShell権限を追加せず、launcher所有のprivate Unix socketに接続する
`beforeSubmitPrompt` / `afterAgentResponse` / `stop` hookを通して同じhost bridgeへ接続する。未知hook fieldは
controller境界で捨て、live `generation_id` を固定してbootstrap turnや古い応答の混入を拒否する。
`stop`と`afterAgentResponse`はbridge内で直列化し、結果JSONが不正な場合も上流mutation前に固定の
`followup_message`を1回だけ返す。任意follow-up、2回目の不正結果、別generationはfail-closedで停止する。
実Orca Run `run_5d892b17c872` / Task `task_863d3336d44f` / Dispatch `ctx_743acfd14a79`で、
Cursor Bがheartbeat、check、`worker_done` succeeded、Task/Dispatch settlement、capability失効、
coordinator Delivery `delivery_02aace26e50b`のACK、exit 0を一巡した。source fingerprintは前後とも
`11c044725736bf37bc7d3df7d25329daa594f35e57357cba489f956d349dd67d`で、変更fileは0件だった。
これは限定read-only受入であり、受付→割当→検証→固定reviewerの自動接続や編集運用の受入ではない。

## レビューと採用

worker終了後、統括が差分と範囲を点検し、必要な検証を直列実行する。review用ticketでは
`allowed_directories` を空にし、次の結果を `source_sha256` として追加する。

```bash
python3 scripts/orca_roles.py fingerprint --ticket /absolute/review-ticket.json
python3 scripts/orca_roles.py launch --ticket /absolute/review-ticket.json --slot reviewer
# 同じ専任reviewerを再開する場合
python3 scripts/orca_roles.py launch --ticket /absolute/review-ticket.json --slot reviewer --resume-session <UUID>
```

source fingerprintはHEAD、tracked/非ignored未追跡fileの内容・mode・symlink・削除、index diffを含む。
review中に対象が変われば無効。ignored runtime file、submodule/nested repositoryの内部は網羅しないため、
そのような入力を変更するtaskは初期運用の対象外とする。採用時は実際の統合headで再確認する。

統括の記録には `ticket`、`base`、`head`、`source_sha256`、`verdict: "approved"`、
`reviewer_session`、`validation_evidence`、`blocking_findings: []` を記す。

```bash
python3 scripts/orca_roles.py verify-review --ticket /absolute/review-ticket.json --review-record /absolute/review-record.json
```

これは**統括が記した承認情報の整合性検査**であり、reviewer本人の署名やCI成功を検証する仕組みではない。
第3batchでは台帳の固定reviewer UUID、直近ticket digest・source fingerprint、終了コード0、会話file不変も要求する。
証拠のbase/head/fingerprint/必要群、reviewerの実際の判断、指摘解消は引き続き統括が実物と照合する。
agentのexit 0や `worker_done` は承認ではない。launcher終了結果も常に `approved: false` を返す。
本commandはcommit/merge/pushしない。公開には別途ユーザーの指示が必要。

## host資源管理

account home（OSのuser database）配下 `.local/state/hell-workers/coordination` に重実行slotを置く。
`HOME` / `XDG_STATE_HOME` の変更で別lockへ分岐させない。永続disk、owner、mode、symlinkを検査する。

- 更新済みdriver間でcompile/test/clippy、依存監査、native/perf recipe、MCP解析backendを合計1件に制限。
- Cargo jobs=1、`RUST_TEST_THREADS=1`。CLIのjobs/thread overrideや未知Cargo aliasも制御対象。
- compile開始前に `MemAvailable >= 8 GiB` とtarget filesystemの空き `>= 2 GiB` を要求する。
  最低開始条件であり、実行中のメモリ/容量消費を上限固定する仕組みではない。
- host→workspaceの順で取得し、busyなら子を起動せず統括queueへ戻す。busyを無制限retryしない。
- fdは同一inodeだけでなく取得済みopen-descriptionを確認して子へ継承する。
  親のfd closeだけでは生存中の継承子のlockを解放しない。
- frozen helperのsourceは変更せず、更新済みprimary validation coordinatorが外側から包む。
- MCP backendは別namespaceで起動し、idle 15秒で解放する。旧daemonへは接続しないが、
  旧daemonやIDEの独自rust-analyzerを自動終了するわけではない。

laneはsession/target固定、host slotは実行排他で役割が異なる。別worktree間でtargetを共有しない。
初回buildには依存の再コンパイルが必要。review中は同じcandidate/targetを維持する。

### 保証の範囲

これは協調する開発ツールの制御であり、悪意あるプログラム向けの完全な隔離ではない。
source書込みはmount境界で制限するが、account読み取り全般やネットワークは完全に遮断しない。
Codex接続用のnetworkを共有し、既存 `auth.json` はread-only mountする（複製・表示しない）。
agentごとに分離した `CODEX_HOME` を使い、Orcaの元transportと`/run`のsocketを引き継がない。
trackedなproject設定directoryはGit差分確認のため隠さず、project `config.toml`で列挙されたMCPは
`enabled=false`と無効commandをCLI overrideしてfresh configでも起動前検証を通しつつ実行させない。
Codex内側sandboxは外側bubblewrapとの競合を避けるため無効化するが、外側のexact write scope、
Git metadata read-only、private state maskを強制境界として維持する。明示したTask受入経路だけは限定proxyを公開する。
元credentialへのアクセスを戻すものではない。
任意のCPUを使うraw programやネットワーク越しのpublishまでOS強制で禁止するものではない。
したがってuntrusted code実行に使わず、raw Cargoや旧driverによる迂回は禁止・投入前確認の対象とする。

## 検証・引継ぎ

専任read-onlyレビューの指摘を修正し、lock競合/偽装/子存続、CLI迂回、ticket拒否、
worker範囲外write拒否・reviewer全source write拒否、古い承認拒否の自動testを追加した。
bubblewrap内の `codex --version` とDNS解決は実行確認済み。
実LLMを別worktreeで2件同時に動かす限定編集は受入済み。Orca Task/Dispatchの取消、setup異常系の全matrix、
ゲームnative/GPU受入は未実施。共有変更を含む並列化の効果や完全な定常運用を確認済みとはしない。
旧基盤commitの変更別全群gateは成功（Python218件、Blender151件、通常/profiling Rust workspace test、
各feature check、Clippy警告0を含む）。検証対象のSHA/fingerprintと残件は
[実装計画](../plans/orca-parallel-development-plan-2026-09-20.md) が正本。
第1batchのCursor B/受付保存候補も同じ基盤をbaseに全群gate成功（Python239件対象、Clippy警告0を含む）。
第2batchの統括相談の実LLM起動/再開は上記の通り確認済み。workerの実tool deny・Task連携は含めない。
第3batchもcontracts/tooling/deps/rust全群成功（Python279件、Blender151件、Clippy警告0）。
同一taskのdirty継続と固定reviewerのworktree間継続は模擬providerによる境界試験であり、実TUI受入ではない。
第4batchではA/B/固定reviewerの実TUI各2turnをread-onlyで受け入れた。
Cursor初回はreadonly global設定へのatomic renameで失敗し、終了・履歴不在の明示照合後に失敗証拠を保全した。
global metadataと不変project permissionsを分離した新ticketでは、同じ会話の再開まで正常終了した。
Codex AとCursor Bは後続のTask bridge受入でreadinessからsettlement・role終了まで成功した。
CLI応答の成功はOrca supervision・review承認・編集運用の成功を意味しない。
第4batchの最終sourceもcontracts/tooling/deps/rust全群成功（Python291件、Blender151件、
通常/profiling Rust tests、Clippy警告0）。exact SHA/fingerprintは実装計画へ記録した。
R3第1batchもcontracts/tooling/deps/rust全群成功。追加のpreflight境界23 test、
実runtime read-only接続、通常/profiling Rust tests、Clippy警告0を確認した。
Task lifecycleは対象外であり、専用launcherの固定reviewer sessionを新たに起動した承認でもない。
R3第2batchはbridge境界24 testとroleのunknown再開拒否testを含む当時の全群gate成功
（Python339件、Blender151件、通常/profiling Rust tests、Clippy警告0）。読み取り専用レビューの指摘は修正・回帰検証済み。
遅延arm＋askが公式CLIの通常のinactivity期限を越えても応答を受け取れ、keepaliveでもhost総期限を延ばせないことを確認した。
続くL3実Task受入では、最初の2件と修正後の1件目がagent内の最初の`check`で`runtime_unavailable`となり、
source未読・mutation 0のままabandon/reconcileした。task-private wrapperで裸のCLI取り違えを除外した後も再現したため、
Codex内側sandboxによるUnix IPC遮断と切り分け、bridge付きread-only Codexだけ外側bubblewrapへ一本化した。
read-only bridge受入時のcandidate `09642de4022577fd442c4c9971c64a4d1f649e26`ではOrca系147件のfocused検査に加え、
変更範囲判定のcontracts/tooling（Python 361件、Blender tooling 151件、Ruff、repository hygiene、
Help No impact）とdiff検査に成功し、固定reviewer・Codex A・Cursor Bの実Task一巡も成功した。
編集fixture追加後のclean HEAD `85cf28431a7fa367367d5bd933bf6709024c0ee7`でもOrca系148件と
変更範囲判定contracts/tooling（Python 362件、Blender tooling 151件、Ruff、repository hygiene、
Help No impact）がpassした。Rust/Bevyゲーム検証は選択していない。
L3Eの修正後clean HEAD `55b27f6e1d9103d7985941c3cbbf135c7299be91`ではOrca系151件、
変更範囲判定contracts/tooling（Python 365件、Blender tooling 151件、Ruff/actionlint、docs/hygiene/help/perf self-test）、
Help No impact、`git diff --check`がpassした。source fingerprintは
`c4ba881001b7df3acbb0d388ce69ad36dd394dfc5f7956131b5bb8d49d65ee1f`。
Codex AとCursor Bは別worktreeで同時に各1 fixtureだけを編集し、固定reviewerを直列再開して承認、
統括がA `f66a9a55aa4ed9f0a71f6bc57e8aadfc8b2c5cd0`、B `3711ae70c0da88c2c42b153298bdeb5face0b6ae`へcommitした。
exact session・fingerprint・失敗経路は[実装計画のL3E受入](../plans/orca-parallel-development-plan-2026-09-20.md#l3e監督付き実編集並列受入2026-09-21)を正本とする。
Rust/Bevyゲーム検証はユーザー指定により選択していない。
実Taskの失敗を成功へ読み替えず、失敗Run/Task/Dispatchとjournalは照合記録として区別する。

Help影響は今回scopeで **No impact**。変更producerは開発時のPython driver・role起動・ルールで、
gameの入力/状態/UI/runtime dataは不変。`build_help_panel_content` は従来の静的manifest/providerから
生成され、Orca設定や本driverをHelp本文へ取り込まない。primaryの別作業のproduction変更はこの判断に含めない。

storageのownerはOrca環境実装/統括、consumerは本実装の検証とレビュー。専用worktreeはprimary台帳へ
`orca-development-environment` として保持登録済み。修正中cacheを日数だけで撤去しない。
継続consumerはL1/L2の権限/通信異常系、L3の受付からの一巡・編集連携受入。既存candidateを再利用し、
review-active cacheは保全するが、過去のゲーム検証結果を再現するためだけの再実行は行わない。
preflightの一時socket/metadataは各実行後に除去済み。空のpreflight rootは次の明示診断で再利用する。
同じcandidate/cacheの最新storage台帳実測は27,607,179,264 bytes（`du -sb` apparent 27,556,122,983 bytes）。
Task bridgeのsocket/proxy credentialは各launcher終了時に除去し、実Task journalは失敗照合と成功settlementの
最小metadataとして5,959 bytesを保持する。
この受入または明示終了後、残る利用者がなくcommit済みsourceを保全できていれば専用作業場を閉じる。
第4batch時点のruntime実測（allocated bytes、ownerはいずれも統括）:

| account home下のpath | bytes | 現在のconsumer / 次の操作 / release_when |
| --- | ---: | --- |
| `.local/state/hell-workers/agents/reviewer` | 61,433,013 | 固定reviewerの継続運用。L3実Taskで同UUIDへ接続済み。置換・廃止時に唯一の状態を引継いで解除 |
| `.local/state/hell-workers/agents/worker-a` | 57,643,008 | R3の会話同一性・readiness比較。受入時の履歴を照合し、比較終了または打切りで解除 |
| `.local/state/hell-workers/agents/worker-b` | 8,474,624 | R3の会話同一性・失敗初回照合の比較。失敗runtimeもnegative controlとして使用し、比較終了または打切りで解除 |
| `.local/state/hell-workers/role-state` | 9,859 | 固定reviewer・失敗再送拒否の制御正本。L3の失敗照合と成功subjectを記録済み。環境廃止/移行時に引継ぐ |
| `.local/state/hell-workers/coordination` | 8,192 | 排他とCursor不変policyの運用正本。R3通信境界を追加し、環境廃止/移行時に解除 |
| `.local/state/hell-workers/agents/coordinator` | 85,181,595 | `TAK-5`の初回相談・同一session追記と今後のR3接続受入。受入または打切り後に試験runtimeを解除 |
| `.local/state/hell-workers/frontdesk` | 16,051 | `TAK-5`の旧/更新後snapshotと相談対応の制御正本。試験依頼は受入または打切りで解除。運用中の依頼は別扱い |
| `.local/state/hell-workers/linear-intake` | 777 | `TAK-5`のsnapshot→受付ID対応。移行完了または打切り後、frontdeskとの対応を保って解除 |
| `.local/state/hell-workers/dispatches` | 起動件数依存 | controllerのRun/Task/Dispatch/terminal/bridge IDとunknown再送拒否の正本。統括が一巡の照合に使用し、依頼close後に対応する記録を解除 |

旧worker ticketはsource/HEAD変更後の再開を許可しない。保持履歴を使う比較と旧taskの再開は区別する。
既存cache・成果・会話履歴の削除はしていない。

## 一次仕様

- [Orca YAML @v1.4.205](https://github.com/stablyai/orca/blob/v1.4.205/src/shared/orca-yaml.ts)
- [repo.update schema @v1.4.205](https://github.com/stablyai/orca/blob/v1.4.205/src/shared/rpc-contract/repo-update-params.ts)
- [runtime repository settings controller @v1.4.205](https://github.com/stablyai/orca/blob/v1.4.205/src/main/runtime/runtime-repository-settings-controller.ts)
- [Codex非対話モード・JSONL・session再開](https://learn.chatgpt.com/docs/non-interactive-mode)

Orca/Codex更新時は起動引数・setup順・mount/DNS/auth・session再開・拒否testを再確認する。
