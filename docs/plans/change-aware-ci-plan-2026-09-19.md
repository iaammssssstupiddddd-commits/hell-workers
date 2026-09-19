# 変更内容に応じたCI自動実行と開発ルール更新計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `change-aware-ci-plan-2026-09-19` |
| ステータス | In Progress |
| 作成日 | 2026-09-19 |
| 最終更新日 | 2026-09-20 |
| 作成者 | Codex |
| 関連提案 | N/A（ユーザーとのCI運用検討から作成） |
| 関連Issue/PR | [#20](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/20)、[#25](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/25) |

2026-09-19に実装開始。コードは `target/change-aware-ci` の `codex/change-aware-ci` branch（base `3ec4a76b`）で分離し、docs正本はprimaryで更新した。2026-09-20に並行変更のない実装ファイルだけをprimaryへ反映し、既存Cargo cacheで全verifyを実施した。primaryの建築アート文書・他sessionのcommitを保持した。実装はPR #20でmasterへ反映済み。
ユーザーの優先事項は「変更内容に応じてCIを自動実行すること」であり、ルール更新も実装範囲に含める。

2026-09-20: ユーザーの「受け入れを完了させてください」によりM6のcommit・push・PR・merge・トリガー確認・専用環境整理へ進む。別PR #19の未merge refactorを含めないため、同じcandidate worktreeを`codex/change-aware-ci-acceptance`へ切り替え、master `52913b61ca524f1cd429d3d73a0468faf3d0999b`を基点にCI差分だけを載せる。candidate内のdocsはprimary正本からの公開用snapshotであり、計画編集は引き続きprimaryで行う。required check新設は今回は行わず、既存の保護なし設定を維持する。

### レビューで修正した点（2026-09-19）

| 指摘 | 根拠と修正 |
| --- | --- |
| Rust変更でToolingを省くと検証漏れ | `scripts/perf_tool/selftest.py` はRust fixtureの定数と契約hashを照合する。Rust変更はToolingも必須に変更 |
| required checkの「移行」という前提が誤り | master protection APIは `Branch not protected`、適用rules APIは空配列。新設の提案とCI実装を区別 |
| 日次／手動depsで品質jobがskipされる構造 | 品質workflowと依存監査workflowを分離し、監査だけのrunには品質checkを作らない |
| 差分基点・ローカル変更・CLIが未定義 | event別のSHA／diff式、具体的CLI、選択結果schema、未追跡変更と検証中の変更の扱いを固定 |
| 新workflowを含むPRで「文書のみ」を実証できない | 候補branchをbaseとする子PRと、default branch反映後の手動／日次受入に分割 |
| 新ルール有効化とM6完了が循環 | 未導入環境では旧全verifyへ戻る条件付きルールを定義し、M6前後の証拠と完了状態を分離 |

M1〜M5とM6aは完了。M6bはmaster push・手動fullが成功し、独立監査の入力なしイベント不具合を修正して再受入中。自然scheduleは2026-09-20 03:17 UTCの発火を未観測のため、計画はIn Progressを維持する。

## 1. 目的

- 解決したい課題: 変更内容に応じたCI検証の自動選択と、ローカル・CIの重複実行を防ぐ完了規則の整備。
- 到達したい状態: PR更新・masterへのpushから変更を分類し、必要な検査を実行して、対象変更の検証結果を完了判定に利用できる。
- 成功指標: 通常文書のみの変更ではRustビルドが0件、Rust変更ではworkspace全体の既存Rust検証を維持、判定不能・必要job欠落は成功にならない。
- 効果測定: 変更分類、選択job、所要時間、cache hit、実行URLを記録する。固定の時間短縮率は約束せず、同じ分類・cache条件で比較する。

## 2. スコープ

### 対象（In Scope）

- `scripts/dev.py` の検証群分割と、ローカル／CIで共有する変更分類・実行入口。
- `.github/workflows/ci.yml` の分類job・選択実行・集約判定、手動全検証、日次依存監査の維持。
- 対象revisionと差分基点、Help影響検査、失敗／skip処理、CI結果の記録。
- 各エージェント向けルール、task lifecycle、関連Skill、開発ガイド、計画テンプレートの同期。
- 独立した変更の作業branch作成、同目的のbranch再利用、PRを通じた自動CI実行と完了後の整理。
- GitHub上の代表ケースの受入、保護設定のread-only確認とrequired check新設の提案。

### 非対象（Out of Scope）

- ゲームの動作変更、Rustを変更crateだけに限定する最適化、テストケースの削減。
- self-hosted runner・GPU runnerの導入、実機確認を一般CIで代替すること。
- 自動merge・自動公開・PRへの自動コメント、全作業ブランチのpushへのトリガー拡大、merge queue導入。
- 既存のnative acceptance・資源guard・検証データ整理規則の緩和。

## 3. 現状とギャップ

- `ci.yml` はPRとmaster pushで `dev.py verify` を実行する。日次・手動は `deps` のみ。
- `verify()` は固定tool検査、Ruff/actionlint、online依存監査、Python／Blender tooling test、perf self-test、repository契約、fmt/check、profiling testとfeature check、Clippy、通常test、diff hygieneを直列実行する。
- `dev.py check` はfmt・Clippy抑制検査・AIルール・compileであり、文書／Python／全Rust testまで実行する入口ではない。
- `check_help_impact.py` はCIで有効な `HELL_WORKERS_DIFF_BASE` を必須とする。Cargo manifest/lockも対象で、bot更新後も新しい実レビューが必要。
- 現在の完了規則はローカルでのverify／Clippyを要求する記述が複数に分散している。CI振り分けだけ変更しても重複実行は解消しない。
- 調査時の[PR実行](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35451237309)は約18分、[master push実行](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35243183308)は約87分（runの開始から最終更新まで）。cache条件や各検査の差を分離した性能比較ではない。
- 2026-09-19のread-only確認: repoはpublic、default branchはmaster。`GET /repos/iaammssssstupiddddd-commits/hell-workers/branches/master/protection` は404で本文 `Branch not protected`、`GET /repos/iaammssssstupiddddd-commits/hell-workers/rules/branches/master` は200で `[]`。現在のmasterにrequired checks／適用rulesはない。実装時に再確認する。

## 4. 実装方針（高レベル）

### 4.1 検証群と共通入口

群IDとCLIは本節で固定する。YAMLに検査コマンドを複製せず、driverの同じ関数を呼ぶ。

| 群 | 内容 |
| --- | --- |
| 共通契約 `contracts` | 文書index／リンク、AIルールとSkill同期、Help影響gate、repo hygiene、crate依存境界、Clippy抑制検査、差分hygiene、実行環境のstorage check |
| Tooling `tooling` | 固定版Ruff/actionlint、`scripts/tests`全件、Blender tooling全件、perf self-test |
| Rust `rust` | fmt、workspace check、profiling構成のworkspace test、memory/tracy/renderdocの現行feature check、Clippy全target警告0、通常workspace test |
| Dependencies `deps` | 固定版cargo-denyによるonline依存監査 |

`dev.py verify` は全4群を実行する互換入口として残す。全固定toolのpreflight後に `contracts → tooling → deps → rust` を実行し、初期失敗時に高価なcompileを開始しない。判定項目と引数は維持し、順序の変更は意図した変更としてtest／docsに記録する。
各jobには必要なtoolだけを導入する。共通契約でゲームbuild、native依存のapt導入、Cargo target復元を要求しない。
CIのstorage検査成功はローカル作業環境の整理を証明しない。元環境の検査は完了前に別途必要とする。
初期導入ではRust群を1つのjobにまとめ、profiling／通常構成が同じcacheを使う。分割による重複compileを避ける。

既存処理の移管先は次のとおり。これを実装の対応表とし、M1では実コードとの差がないか確認する。

| 現行処理 | 移管先／維持する契約 |
| --- | --- |
| `run_quality_tools(lint=True, deps=True)` | Toolingはlintのみ、Dependenciesはdepsのみ。tool版のpreflightとonline/offlineの区別を維持 |
| unittest discover 2系統、`perf.py self-test` | Tooling。Pillow 12.3.0を供給し、実GPU／Blender起動を要求しない |
| storage／agent／Help／hygiene／crate境界／docs検査、Clippy抑制検査 | 共通契約。crate境界検査はTOML解析なのでRust install不要 |
| fmt、`check --workspace --locked` | Rust |
| `test --workspace --no-default-features --features profiling --locked` | Rust。feature構成を変えない |
| `check -p bevy_app@0.1.0 --lib --no-default-features --features <feature> --locked` | Rust。memory/tracy、Linux/Windowsではrenderdocを維持 |
| `clippy --workspace --all-targets --locked -- -D warnings`、通常workspace test（`--locked`） | Rust。警告0と通常構成を維持 |
| `diff_hygiene_command()` | 共通契約。Helpの基点とtree差分の基点を混同せず、4.3の分類対象diffを検査 |

### 4.1.1 実装するCLIと責務

下記は**導入後のインターフェース仕様**。`<SHA>` 等は実際の値に置き換える。

| CLI | 契約 |
| --- | --- |
| `python3 scripts/dev.py quality --group contracts` | 指定1群を実行。groupは `contracts/tooling/rust/deps` のenum。CIでは `--plan-json <JSON>` を必須にして同一対象を照合する。単独群成功を全verify成功とは表示しない |
| `python3 scripts/dev.py ci plan --github-event <JSON-file> --github-output <path>` | `GITHUB_EVENT_NAME` とevent fileとcheckoutから分類。標準出力へJSON、GitHub outputへ小さな固定schemaを出す。検証・fetch・installは行わない |
| `python3 scripts/dev.py ci result --plan-json <JSON> --needs-json <JSON>` | 全jobの状態と出力を照合し、不整合で非zero終了。JSONはenv経由で引数へ渡し、式展開をshellコードへ埋め込まない |
| `python3 scripts/dev.py ci check --base <SHA> --mode auto` | ローカル用。commit差分＋staged/unstaged/untrackedを分類し、必要群を順に実行。modeは `auto/full`、既定auto。baseは必須 |
| `python3 scripts/dev.py verify` | 分類不要の全4群。既存のローカルHelp差分基点解決を維持し、解決不能時は失敗 |

新規moduleは `scripts/quality.py`（群の実行）、`scripts/ci_scope.py`（diff・分類・schema）、`scripts/ci_result.py`（結果照合）。`dev.py` はCLIと既存guard付き実行関数を所有する。quality moduleにはrunnerを引数で渡し、`quality.py → dev.py → quality.py` の循環importを作らない。
新規testsは `test_quality.py`、`test_ci_scope.py`、`test_ci_result.py`。既存の `lint/deps/check/verify/cargo` のCLI互換を維持する。

### 4.2 変更分類

共通契約はPR／push／手動品質検証で常に実行する。下表の群は追加分。複合変更は和集合とし、より強い判定を優先する。

| 変更 | 追加群 | 判定上の注意 |
| --- | --- | --- |
| 通常の `docs/**/*.md`、README等の説明文書 | なし | 下記のルール・テンプレートを除く。拡張子だけで全Markdownを軽量扱いしない |
| 通常のPython／Blender開発tool・test | Tooling | 検証の選択・実行・環境供給に関わるpathを除く。初期導入ではPython testを個別ファイルに絞らない |
| Rust source・Rust test・snapshot・回帰seed | Rust＋Tooling | `perf_tool/selftest.py` がRust fixtureの定数を読むためToolingも必須。crateを問わずworkspace全体 |
| Cargo manifest/lock、toolchain、`.cargo/**`、依存監査設定 | 全群 | 依存更新もHelp実レビューを維持 |
| CI、分類器、driver、資源guard、品質tool供給、契約検査の実装／test | 全群 | 検証経路を変え得るため通常Pythonより優先 |
| エージェントルール、Skill、task lifecycle、`docs/plans/plan-template.md` | 全群 | 文書形式でも検証・完了規則を変え得る |
| runtime asset／shader／settings、crate内asset | 全群 | 初期は保守的に扱う。buildで資産内容の妥当性まで証明したことにしない。必要な資産検査・実機受入を別途判断 |
| 上記の許可リストにないpath | 全群 | 新しい拡張子／rootも省略側に倒さない |

`check_help_impact.py` のproduction判定を正本として利用し、分類器に別のHelp対象定義を作らない。
Help gateは常時実行するため、productionを含むのに軽量分類された場合もレビュー義務を失わせない。
新しいruntime data root／拡張子追加時はHelp分類とCI分類の両方にfixtureを追加する。

path判定はrepo相対のPOSIX pathで行い、次の順序と範囲で固定する。末尾拡張子にかかわらず先に強い規則を適用する。

1. **全群**: `.github/**`、`.cargo/**`、root／crateのCargo manifest、`Cargo.lock`、`rust-toolchain.toml`、`deny.toml`、`ruff.toml`、`.gitignore`、`.gitattributes`、`scripts/dev-tools.toml`。
2. **全群**: `check_agent_rules.ROOT_RULE_FILES` と `active_rule_files()`、`.agent/**`、`.cursor/**`、`.codex/**`、`.gemini/**`、`.claude-plugin/**`、任意階層の `AGENTS.md/CLAUDE.md/GEMINI.md/_rules.md`、`docs/plans/plan-template.md`、`docs/DEVELOPMENT.md`、`docs/development-infra/validation-storage-workflow.md`。列挙はtreeから消えたpathにも適用する。
3. **全群**: `scripts/{dev,quality,ci_scope,ci_result,cargo_runtime,build_lane,build_coordination,validation_storage,dev_tools,install_dev_tools,sync_agent_skills,update_docs_index}.py`、`scripts/check_*.py` と対応する `scripts/tests/test_<stem>.py`。
4. **全群**: `assets/**`、`settings/**`、`crates/*/assets/**`。runtime path内のREADMEもここへ入る。
5. **Rust＋Tooling**: `crates/**/*.rs`、`crates/**/*.snap`、`crates/**/proptest-regressions/**`。`crates/*/build.rs` は例外として全群。
6. **共通契約のみ**: 上記以外の `docs/**/*.md` と、basenameが正確に `README.md` の文書。docs画像など未登録形式は全群。
7. **Tooling追加**: 上記以外の `scripts/**/*.py`、`scripts/perf_tool/contracts/**`、`tools/blender_ai_workflow/{scripts,tests,bin,fixtures,templates}/**`。
8. **全群**: その他すべて。新しい軽量対象を追加するときは分類器変更となり全群で検証する。

`contracts` は常に必要、`rust=true` なら必ず `tooling=true`。`full` は4群すべて。
Rustのみの変更ではdepsを省くため、そのrunは「全verify」ではない。依存／監査設定変更時と日次のonline監査は継続する。

### 4.3 差分・revision・失敗時の契約

`B` は比較元commit、`H` は変更head、`T` は実際に検証するcheckout、`G` はmerge-baseとする。可変branch名をjobごとに解決せず、eventの完全SHAを開始時に固定する。

| event | SHAと分類するtree差分 | Helpへ渡す基点 |
| --- | --- | --- |
| PR | B=event base、H=event head、T=`GITHUB_SHA`のmerge commit。G=merge-base(B,H)。`G..H` と `B..T` のpathの和集合 | B（既存Help resolverがTとのmerge-baseを確認） |
| master push・fast-forward | B=before、H=T=after。`B..H` | B |
| master push・non-fast-forward | B=before、H=T=after、G=merge-base(B,H)。`B..H`、`G..B`、`G..H` の和集合。必ずfull | G |
| 手動auto/full | B=input `base_sha`、H=T=`GITHUB_SHA`。BはTの厳密な祖先であること。`B..T` | B |
| ローカル `ci check` | B=必須 `--base`、H=T=現在HEAD、G=merge-base(B,H)。`G..H` と現在worktreeの変更の和集合 | G |

- PRのTは親がB/Hと整合することを確認する。不一致はref移動と扱い失敗する。`G..B` のbaseだけの変更はPR分類に加えず、PRの最終統合結果における変更を `B..T` で捉える。
- 全jobは分類されたTを明示checkoutする。後続jobが最新PR refを再解決して別Tを検証することを禁止する。cloneは `fetch-depth: 0`、必要なevent SHAが未取得ならそのSHAのfetchを試み、なお不足なら失敗。
- tree差分は `git diff --name-status --no-renames -z <from> <to>` で取得する。renameを削除＋追加として扱い、両pathを確実に分類する。コミット件数やファイル件数で打ち切らない。
- statusの未知値、解析不能、unmerged index、zero/missing SHA、merge-base不在、未対応eventはexit 1。これらを「未知pathなのでfull」と同じ扱いにしない。未知pathは正しく取得した差分なのでfull、差分自体の取得不能は検証対象を確定できないため失敗。
- 手動 `base_sha` は必須の完全commit SHAで、空文字・T自身・祖先でない値は拒否する。masterを暗黙にbaseにして空差分にしない。初回commit等は本CLIの範囲外として理由を出す。
- localは `git diff HEAD --name-status --no-renames -z` と `git ls-files --others --exclude-standard -z` を含める。stagedだけ／unstagedだけを見ない。Git追跡対象のmode変更・削除も含める。
- ローカルの全追跡ファイルとignoreされていない未追跡ファイルのpath・mode・内容から開始／終了fingerprintを比較する。検証中に別作業でソースが変われば成功結果を現状へ流用せず、再分類を要求する。ignored build出力は含めず、tracked fileはignore規則に関係なく含める。
- diff hygieneは分類に使った各tree差分とlocalの `HEAD` 差分を検査し、新規未追跡textも検査対象とする。Helpのためのmerge-baseへ正規化してpushのtree差分を失わない。
- commit途中で追加・revertしたproduction変更も、Helpは既存 `collect_commits()` でレビューする。分類器は最終treeの検査選択、Helpはcommit batchのレビューという別の役割を保つ。
- 有効な同一treeの空差分でもcontractsを実行する。`[skip ci]` 等でrunがない場合は成功証拠なしとして扱う。

### 4.3.1 選択結果のschema v1

`plan_json` は `schema_version=1`、`event_name`、`mode`、`base_sha/head_sha/tested_sha/help_base_sha`、`diff_pairs`、`groups`、`reason_codes`、`path_count`、`paths_sha256`、`run_id/run_attempt` を持つ。groupsは `contracts/tooling/rust/deps` の全keyとJSON booleanを必須とし、文字列booleanや未知keyを拒否する。
`paths_sha256` は正規化・整列した全pathのNUL区切りbytesをhash化する。path全件をjob outputへ詰めず、Summaryでは分類別件数と理由を示す。実pathを表示する場合はMarkdown／HTMLをescapeする。
後続jobは同じeventとTからplanを再計算してschema・SHA・groups・path digestが一致することを確認する。成功したjobは `tested_sha` と `plan_sha256`（run_attemptを除いたcanonical JSONのhash）をoutputへ出す。集約では必要job全件の出力一致も要求する。JSON key・reason codes・diff pairsの順序を正規化する。run_attemptは記録用とし、同一runのfailed jobs再実行で前回成功jobを不当に無効化しない。対象SHA・planが変わった場合はrun全体を新規実行する。

### 4.4 Workflowと集約結果

- `.github/workflows/ci.yml` はPR・master push・手動auto/fullの品質検証専用にする。PR typesは `opened/synchronize/reopened/ready_for_review/edited` を対象とし、base変更も再分類する。draftでも通常どおり検証する。
- 新規 `.github/workflows/dependency-audit.yml` へ03:17 UTCの日次と手動依存監査を移す。品質job／品質集約を一切定義せず、依存監査の成功・skipから品質checkが生まれないようにする。
- PR未作成のbranch pushでは自動品質検証は始まらない。手動auto/fullかローカルを使う。base branchへ別PRがmergeされただけでは当該PRの再実行を保証しないため、完了時に最新baseとの一致を確認し、ずれていればbranch更新後のPR runを取得する。

| job ID / 表示名 | needs・開始条件 | 環境／時間上限 |
| --- | --- | --- |
| `changes` / Classify changes | 常時 | checkout・Python標準lib・Gitのみ、5分 |
| `contracts` / Repository contracts | changes成功 | 同上。必要なSHAのhistoryを取得、10分 |
| `tooling` / Tooling tests | changes/contracts成功、tooling選択 | Python venv＋Pillow 12.3.0、固定Ruff/actionlint、20分 |
| `dependencies` / Dependency audit | changes/contracts成功、deps選択 | 固定Rust/cargo-deny、native apt／target cache不要、15分 |
| `rust` / Rust quality | changes/contracts/tooling成功、deps非選択またはdependencies成功 | 現行native apt、Rust、Cargo cache、90分 |
| `quality` / **Quality gates** | 上記5jobを直接needsに列挙、`if: always()` | 同じTのcheckout、Python標準lib・Gitのみ、5分 |

Rustは `needs: [changes, contracts, tooling, dependencies]`。dependenciesが正常にskipされたケースでも実行されるよう、job条件は `!cancelled()` と各必要jobのsuccess／選択booleanを明示し、暗黙のsuccess条件に依存しない。
ToolingとDependenciesは別runnerで並行可、Rustは両方の必要検査が成功してから開始する。現行の資源guardとRust内の直列実行は維持する。
lintは現行の `run_quality_tools(lint=True)` を共有する。Lint用のCargo環境正規化を残してもRust toolchainの導入／ゲームcompileは要求しないことをM1で検証する。

集約規則は次のとおり。

1. changes成功、plan schema妥当、contracts選択、`rust ⇒ tooling` を確認する。
2. 必要jobはresultが厳密にsuccessで、tested SHA／plan hashも一致しなければ失敗。
3. 不要jobはskippedのみ許可する。選択していないjobが実行された場合も振り分け不整合として失敗し、理由を表示する。
4. failure/cancelled/欠損出力／未知状態を失敗にする。run自体がcancelされて集約が起動しなかった場合も成功証拠はない。`continue-on-error` は品質jobに使用しない。
5. `quality` のjob名は既存の `Quality gates` を維持する。日次workflowはこの名前を使用しない。

concurrencyは `workflow + event + PR番号またはref + 手動mode`。PR autoの古いrunだけを置き換え、fullとauto、監査と品質を相互cancelしない。
SHA固定Action・`contents: read`・現行Rust/tool版を維持する。cacheはRust jobだけが現行key／保存対象を利用し、初期導入でcache方式の変更は混ぜない。各runでcold/warmの区分を記録する。共有cacheの削除は受入に含めず、coldが未観測なら未測定と記録し、性能比較の対象外にする。
Python tool群の分割導入は既存installerの `--tool` を使う。各jobのsetupとpreflightに必要なものだけ導入し、共通契約から全toolのdoctorを呼ばない。

required checkの新設は別の設定変更として、`Quality gates`、source=GitHub Actions、最新baseを要求するstrict設定を推奨する。既存のmaster直push運用への影響を示してから反映する。現時点では未設定なので、新設しない場合も自動CIの導入は可能だが「mergeを強制的に防ぐ」とは報告しない。
merge queueは本計画で有効化しない。再調査で既に有効なら `merge_group` 用のSHA分類を追加するまで保護設定変更を保留する。
GitHubのjob checkはskippedでも通過扱いになり得るため、集約自身を条件skipしない。また、手動runは開発上の検証証拠に使えてもPRのrequired check充足に流用しない。[required checksの公式仕様](https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks)

Step Summaryに分類理由・必要群・各結果・base/head/tested SHA・run URLを出す。PRコメント投稿は追加しない。

GitHub仕様の参照: [workflow構文・path filter・needs](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)、[job条件とskip](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-jobs-with-conditions)、[eventとPR checkout／手動実行](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)。実装時に再確認する。
Bevy API変更は予定しない。Rust検証の既存Bevy 0.19構成は維持する。

### 4.5 更新する開発ルール

完了条件の正本を `docs/DEVELOPMENT.md` に集約し、各ツールには短い必須規則と参照先を置く。

1. 作業中は変更に応じたローカル検査を使う。Rust変更時は `dev.py check` と必要なfocused test、文書のみなら文書検査を使う。
2. 対象変更について分類・必要job・集約結果を検証済みのCI成功を、対応する品質検査の完了根拠として認める。文書限定CIを「全verify成功」と表現しない。
3. 同じ対象でCIが実施済みのClippy／test／verify相当を、完了のためだけにローカルで重複実行しない。警告0という基準は変えない。
4. CI未実行・利用不可・未push・dirty差分・結果が古い場合は、`dev.py ci check --base <SHA> --mode auto` でbatch全体の必要群をローカル実行する。未検証の追加差分だけをCIへ足し合わせて成功としない。不確実なら全verifyを使う。自動でcommit/pushして条件を作らない。
5. 対象branch名だけで成功を流用しない。PR head、base、tested merge SHAを記録し、追加commit・rebase・base更新で前提が変われば再検証する。未コミット／未追跡の対象変更も確認する。
6. Help実レビュー、必要な実機受入、元環境のstorage整理はCI成功とは別に完了させる。Help no-impact理由を分類器や環境変数で自動捏造しない。
7. 完了報告には選択範囲、成功したrun URL／SHA、ローカル追加検証と残課題を書く。CI待ち・失敗・キャンセルを完了としない。
8. CI導入計画への合意はcommit/push/mergeやruleset書込みの包括許可ではない。既存の明示的な許可範囲で操作し、未許可の場合はレビュー可能な結果と残る公開手順を提示する。

共有する短い規則は「変更範囲に対応する最新の品質検証を完了条件とし、CI結果の対象SHA・選択群・全必要jobの成功を確認する。CIで証明できない変更はローカルで検証する。Help実レビュー・実機受入・元環境の整理は別途完了する」とする。
各root rule、task lifecycle、2つの品質関連Skillに同じ意味の必須blockと `docs/DEVELOPMENT.md` の参照を置き、`check_agent_rules.py` の専用定数／検査で欠落を拒否する。一般文書の「verify」という語まで禁止するような広い正規表現は作らない。

新CLIのないcheckoutでは現行の全verifyを使う。新CLIがあるcheckoutでは、対象Tにおける新集約checkの証拠を確認できる場合のみCI代替を利用する。運用全体の導入完了はM6で別に記録し、個々の作業でM6の完了宣言だけを成功根拠にしない。
今回の導入batchそのものは規則変更としてfull対象で、ローカル全verifyと候補CI全群の両方を受入証拠とする。この一度の移行検証を、以後の全タスクの重複検証規則にしない。

### 4.6 通常開発のブランチ運用

**独立してレビュー・mergeできる変更は作業branchへ分け、PR更新でCIを自動実行する。** branchを細かく増やす単位はcommit数やファイル数ではなく、変更の目的と検証・mergeのまとまりとする。

| 状況 | 方針 |
| --- | --- |
| 新機能、不具合修正、refactor、CI／ルール変更など独立した作業の開始 | 原則masterを基点に専用branchを作成。通常の実装をmasterへ直接積まない |
| 同じ機能の続き、レビュー修正、CI失敗修正 | 所有と目的が一致する既存branch／PRを継続。毎回新規branchを作らない |
| 独立した文書のみの変更 | 公開する単位で文書用branch／PRへ分離し、軽量CIを利用する |
| 実装に伴う仕様・Help・ルール文書の更新 | 実装と同じbranch／PR。検証を軽くするために一体の変更を別PRへ逃がさない |
| 調査・相談のみで変更しない | branch作成は不要 |
| 別タスクの未コミット差分／並行sessionがある | 所有を確認し、他タスクの差分を混ぜてbranch切替・commit・stashしない。必要なら専用worktreeでコードを分離し、docsの正本はprimary repositoryで維持 |

命名は `codex/<topic>` を既定とする（例: `codex/change-aware-ci`）。ユーザー指定や既存の同目的branchがあればそれを優先する。作業開始時にbranch・base SHA・対象PR・既存差分の所有を計画へ記録する。依存する別PRをbaseにする場合は依存先を明示し、master向けへ変更した時点でCIを再実行する。

標準の流れは次のとおり。

1. 作業開始時にstatus・現在branch・baseと並行作業を確認し、新規作成／継続利用を決める。shared worktreeのbranchを他sessionの確認なしで切り替えない。
2. ローカルで変更とfocused検証を進める。commit/push/PR作成が依頼範囲で許可されていれば、意味のある最初の区切りでdraft PRを作り、以後のpushで自動CIを使う。既にある許可を毎回取り直さない。
3. branch作成やbranchへのpushだけでは本計画の自動CIは始まらない。PRを開くことを自動CIの入口とする。公開が未許可の作業はローカル検証で進め、「CI未実行」と明記する。
4. pushはレビュー・検証可能な区切りで行う。ローカル編集のたびにはpushせず、同じ目的の追加修正は同じPRへまとめる。最終headと現在baseのCIを確認して完了を判定する。
5. mergeはユーザーの許可範囲で行う。merge後はmaster pushのCIも確認し、レビュー・検証consumerがなくなった自分のbranch／専用worktreeを既存の整理規則に従って撤去する。共有・未mergeの作業を巻き込まない。

masterのbranch保護が未設定でも、この開発規則としてPR経由を基本にする。保護設定による強制と日常の運用規則は区別する。masterへの直接変更を明示的に依頼された場合はその指示に従い、push後のCIと検証範囲を記録する。
M4でroot rulesとtask lifecycleの「開始時」に本節を反映し、終了時には対象branch／PR／SHA／CI結果と整理状態の確認を追加する。

## 5. マイルストーン

### M1: 現行検証の棚卸しとdriver分割

- 変更ファイル: `scripts/dev.py`、新規 `scripts/quality.py`、新規 `scripts/tests/test_quality.py`、`scripts/tests/test_dev.py`、`scripts/README.md`。
- 現行の各コマンド・preflight・差分基点・storage副作用を検証群へ対応づける。旧verifyの機能を保ったまま部分実行入口を追加する。
- 完了条件: 全群を組み合わせると従来の全検証を包含する。共通契約のみではRustコンパイルや無関係tool導入が発生しない。
- 検証: driverの境界／群選択テスト、既存tooling test。全verifyは変更をまとめたM5で実施し、途中の失敗・新たな懸念がなければ各段階で反復しない。

### M2: 変更分類とrevisionの確定

- 変更ファイル: 新規 `scripts/ci_scope.py`、`scripts/tests/test_ci_scope.py`、driverの接続部、diff hygieneの対応test。
- 4.2の優先順位・和集合・保守的fallbackを実装し、理由付きの機械可読結果を出す。Help正本と共通の差分前提を確立する。
- 完了条件: 文書・tool・Rust・依存・CI／rules・asset・未知path、追加／削除／rename、複数commit、PR／push／手動を取りこぼさない。4.3.1のschemaとlocal fingerprintを実装し、対象確定失敗をfullでごまかさない。
- 検証: 一時Git fixtureでmerge-base、force push、zero/missing SHA、大量path、特殊文字、途中commitにproduction変更がある場合を検査する。

### M3: CIの条件実行と集約

- 変更ファイル: `.github/workflows/ci.yml`、新規 `.github/workflows/dependency-audit.yml`、`scripts/ci_result.py`、`scripts/tests/test_ci_result.py`。
- 群別のtool供給、Rust cache共有、手動mode、日次監査、Summary、concurrencyを接続する。
- 完了条件: 4.4の6jobと別監査workflowが構成され、不要Rust jobがskipされ、必要jobの失敗／skip／cancel／分類出力欠損で集約は成功しない。dependencies非選択時も必要なRustが起動する。全検証modeも維持する。
- 検証: actionlint、集約の異常系fixture、M6のGitHub実走。workflow自体の変更は全群を選ぶ。

### M4: ルール・Skill・ドキュメント同期（必須）

- 変更対象を以下の表で管理し、実際の全文から完了条件の矛盾を除く。

| 対象 | 更新内容 |
| --- | --- |
| `docs/DEVELOPMENT.md`、root `README.md`、`scripts/README.md` | 分類表、branch／PRからCIを使う開発手順、同一対象の判定、手動fallback、日次との区別 |
| `AGENTS.md`、`CLAUDE.md`、`GEMINI.md` | branchの新規作成／再利用基準、verify／Clippy必須の達成方法、ローカルcheckの適用範囲、CI結果確認と報告 |
| `crates/**/_rules.md` と各 `AGENTS.md/CLAUDE.md` symlink、`.cursor/rules/*.mdc`、`.cursor/docs/*.md`、`.agent/rules/*.md` | 現行規則として読む全文を監査し、矛盾する完了規則だけ同期。symlinkは正本を編集して維持 |
| `.cursorrules`、`.kilocoderules`、`.github/copilot-instructions.md`、`.gemini/antigravity/project_rules.md` | 同じ完了条件へ同期。`check`だけで完了とする旧規則も是正 |
| `.agent/workflows/task-lifecycle.md`、`.cursor/workflows/task-lifecycle.md` | 開始時のbranch／base／差分所有確認、PR経由のCI利用、失敗時の修正・再検証、branch／元環境の整理 |
| `.cursor/skills/hell-workers-update-docs/SKILL.md`、`.cursor/skills/hell-workers-review-help-impact/SKILL.md` | 文書限定／広範囲変更の検証基準、CI代替可能範囲。Help実レビュー義務は維持 |
| native acceptance Skillと関連rules | 全文を確認し、品質gateを参照する箇所だけ必要に応じ同期。実機証拠・launcher契約は維持 |
| `scripts/sync_agent_skills.py` の同期先 | Cursor正本から `--write` でCodex／Gemini／Claude版へ同期。生成先を独立編集しない |
| `scripts/check_agent_rules.py`、対応test | 共通のCI完了規則が必要な各surfaceにあり、同期漏れ・旧完了条件への後退を検出する契約を追加 |
| `docs/plans/plan-template.md`、本計画、関連する現行計画 | 検証欄／DoDに対象SHA・選択群・CI URL・ローカル残務を記録可能にする。archiveを現行規則へ書き換えない |

- 完了条件: 各surfaceで「無条件にローカル全verify」「checkだけで完了」「CI成功だけで実機／整理も完了」が残らない。旧規則の検索結果は歴史記録と現行命令を区別してレビューする。
- 検証: `python3 scripts/sync_agent_skills.py --check`、`python3 scripts/check_agent_rules.py`、関連test、docs検査。
- 有効化条件: 4.5の条件付き規則としてコードと一緒に導入する。M6完了を要求しながらM6のために新規則を先に使う循環を作らない。M6未完了は運用導入未完了として報告する。

### M5: Help影響レビューと全体整合

- `hell-workers-review-help-impact` Skillに従い、完成した開発tool変更が通常ゲームの入力・状態・表示へ到達するか確認して判断する。
- 開発用変更という名前だけでNo impactにしない。runtimeへの影響がなければ具体的根拠を記録し、commitが許可された場合だけ必要なtrailerを残す。
- 完了条件: Help検査の既存対象・レビュー義務を維持し、CIへ移したことによる免除がない。docs・rules・コードが同じ分類を説明する。
- 検証: Help gate、rule／docs検査、全verify。本実装は開発品質検証に限定し、ゲーム実装・runtime dataを変更しない。
- M5は最終整合レビューの工程名であり、途中でcommit／完了報告する場合のHelp Skill必須実施を延期する理由にはしない。

### M6: GitHub受入・導入確認・close

M6aとM6bを分ける。CI定義を含む実装PRへ文書commitを追加してもPR全体はfullのままであり、文書限定受入の証拠にはならない。

**M6a: default branch反映前**

1. 許可された候補branch CにM1〜M5を集約し、master向け実装PRでfullを成功させる。
2. Cの固定SHAから文書のみの子branch Dを作り、base=Cの子PRでcontractsだけが実行されることを確認する。同様に通常tool変更の子PR、Rust変更の子PRを確認する。Rust caseではdependencies skipとRust実行の両立を実証する。
3. 子PRの変更は受入専用で、masterへmergeしない。Rustを選択する子PRに文書の壊れたリンク等を加えてcontractsを故意に失敗させ、選択済みRustが起動せず集約がfailureになる代表ケースも確認する。
4. 分類全パターン・欠損／cancel等は第7節のfixtureで網羅する。高価な依存変更のdummy fullを何度も実走せず、実装PRのfullを群実行の証拠として使う。

**M6b: default branch反映後**

1. ユーザーの許可範囲で実装を反映する。merge方式でHelpレビュー記録を失わないことを確認し、master pushのfullを成功させる。反映の許可がまだなければ「実装・M6a検証済み、導入待ち」としてここを未完了にする。
2. `ci.yml` の新しい手動inputがdefault branchに存在する状態で、既知の祖先 `base_sha` を指定して手動fullを確認する。`dependency-audit.yml` の手動実行で依存監査だけが走ることも確認する。
3. 次の日次schedule発火を観測し、手動監査の成功で代用しない。既存cron時刻を維持するが、実際の起動遅延を失敗と即断しない。未観測なら日次受入のみ未完了と明記する。
4. required check新設を行う場合のみ、`Quality gates`の成功checkを確認した後、strict／source=GitHub Actionsを設定し、未成功PRがmerge不可になることを確認する。新設しない場合は「保護なし・任意CI」と明記する。後日既に保護が導入されていたら既存要件を維持した段階移行へ切り替える。

文書caseはRust job・native apt・Cargo target cache復元が0件であることをjob/stepから確認する。各runに分類・cache hit・cold/warm・実行時間を記録する。
CI導入の完了条件はM6a＋M6bの品質／監査確認、rules同期、不要受入branch／PR／workspaceの整理。保護新設の採否は別項目として記録し、未設定なのに保護済みとしない。
永続仕様を `docs/DEVELOPMENT.md` 等へ反映し、本計画はarchiveまたは削除してindexを再生成する。

### M6受入記録（2026-09-20、進行中）

公開候補は`134fe6c3b0cbcb7fe2c23555ec8d13d1d9010da1`、baseはmaster `52913b61ca524f1cd429d3d73a0468faf3d0999b`。別PR #19と建築アート文書のcommitは含めない。子PRのbaseはすべて候補の固定SHAで、masterへmergeしない。既定branchの保護は新設せず、保護なしのままとする。

| ケース | PR / run | 結果 |
| --- | --- | --- |
| 実装全群 | [#20](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/20) / [35459170013](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459170013) | C/T/D/R・集約success、masterへmerge済み |
| 文書のみ | [#21](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/21) / [35459220974](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459220974) | 成功。Cのみ、T/D/R skip、集約success。実行jobにnative apt・Cargo cache stepなし |
| Toolingのみ | [#22](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/22) / [35459250390](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459250390) | 成功。C/T、D/R skip、集約success |
| Rust testのみ | [#23](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/23) / [35459267058](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459267058) | C/T/R・集約success、D skipでもRを完走 |
| Rust選択＋意図した契約失敗 | [#24](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/24) / [35459294822](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459294822) | 期待どおりfailure。C failure、T/R未起動、集約failure |

文書caseのtested merge SHAは`2db4bbf79c9a41f4171b34151f33038ac0cf62a0`、Toolingは`34c4a4e20fcdd1c49f77ff531a2957ab33545005`、意図した失敗は`0ba88bfa5e0732d86e3e90310c95888555c5b573`。成功／失敗とplanのSHAはjob logで照合した。自然scheduleは03:17 UTCを維持し、手動実行と区別して観測する。

PR #24の連続pushで[旧run 35459533448](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459533448)のみcancelled、[新run 35459541624](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459541624)は意図した契約エラーでfailure。他PR #20/#23のRust実行は継続し、PR間の取消分離を確認した。

PR #20のtested merge SHAは`1697b499ecbe60f98723bf5c3b6860c2123ffdaa`、Rust子PRは`bd83ece0b00c5c22b6e046b90d26d3f1aad52c33`。実装PRは9分21秒（Rust 7分55秒、既存cache exact hit）、文書のみ32秒、Toolingのみ84秒。文書・ToolingではRust cache自体を使わない。cold cacheの新規実測は行っておらず、cold時の時間短縮は主張しない。

PR #21〜#24は証拠保存後にmergeせずclose済み。4本の受入専用branchは既知SHAと全diffを確認してremote/local双方から削除済み。fixtureはmasterへ入れていない。

master merge SHA `a0b515a8f44711ca81a3713a5aece4e5f6ef44a2`で[push full 35459807499](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459807499)と[手動full 35459817786](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459817786)が全群成功。手動baseは`52913b61ca524f1cd429d3d73a0468faf3d0999b`。

独立監査の[初回 35459819407](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35459819407)は`workflow_dispatch`の`inputs:null`をdictとして処理して準備段階で失敗した。PR #25（head `4a339c657cc9c529dfe429922a37b871b825a852`）で修正し、null／省略／空inputsとscheduleの回帰test、手動品質planのbase必須条件を確認した。ローカルtooling全群成功（Python 205、Blender tooling 151、perf self-test・lint）。[修正branchの独立監査 35460335250](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/35460335250)は成功し、jobはDaily dependency auditだけ、Quality gatesは生成されない。PR全群と反映後の再受入を継続する。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 分類漏れ・rename元の見落とし | 必要な検査が省略される | 未知pathは全群、優先規則、Git fixture、共通Help gate |
| workflow／jobのskipを成功と誤認 | 未検証のmerge／完了 | 常設集約と必須job集合の照合、異常系test |
| PR headとmerge commitの混同 | 古い／別の対象へ成功を流用 | base/head/tested SHAを区別し、更新時に再検証 |
| 文書形式のrulesを軽量扱い | 検証規則の変更が検証されない | rules／Skill／templateを全群へ優先分類 |
| 群の分割でcacheが重複する | 以前より遅い・高負荷 | Rustは1job、依存導入の選択、実測後に追加最適化 |
| 部分成功を全verifyと表示 | 証拠の過大評価 | Summary・完了報告に選択群と未実施範囲を明記 |
| CI移行でHelp実レビュー／実機／整理が抜ける | プレイヤー案内や受入の欠落 | 独立した完了項目として全ルールに維持 |
| required checks移行が不整合 | merge停止または検証なしmerge | 新check実走後に移行し、保護の空白を作らない |
| Python testとRust fixtureの依存を見落とす | 契約hash不整合を検出できない | Rust選択時はToolingを必須化。依存方向の具体例を回帰fixtureにする |
| 必須check未設定を保護済みと誤認 | CI失敗でもmergeできる | 現状のAPI結果、新設の採否、実際の強制力を分けて記録 |
| 受入用の子PRにも実装差分が混ざる | 全検証しか試せず軽量化を証明できない | baseを候補Cに固定し、event base/headと分類結果を確認 |

## 7. 検証計画

- 計画書整備・レビューの今回: docs index生成・整合性、差分hygiene、既存rule／Help gateの確認、GitHub設定のread-only調査。CI起動・コード変更・現行ルール変更は行わない。
- 実装時の必須: M1〜M3の分類／driver／集約fixture、既存Python tests、actionlint、Help gate、rule／Skill同期検査。
- 初回導入の全体検証: `python3 scripts/dev.py check`、`python3 scripts/dev.py verify`。verify内のClippy全target警告0・全Rust testを記録し、同一差分での不要な繰返しは避ける。
- 明示的なClippy再現入口: `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`。
- docs変更後: `python3 scripts/dev.py docs --write`、生成されたplans／proposals両indexレビュー、`python3 scripts/dev.py docs --check`、`git diff --check`。
- GitHub実走: M6aの子PRと実装PR、M6bのmaster push・手動full・独立監査、concurrency、保護設定の採否を記録する。
- native acceptance: 本計画はゲーム挙動を変えないため一律に要求しない。runtime変更へ波及した場合は該当Skillとrecipeを適用する。

### 必須の受入fixture

以下のIDをtest名またはcase IDへ対応づける。C/T/R/Dはcontracts/tooling/rust/deps。

| ID | 入力・状態 | 期待結果／証拠 |
| --- | --- | --- |
| S01 | `docs/building.md` のみ、削除のみも含む | C。GitHub子PRでR jobがskip、native apt・Cargo cache復元なし |
| S02 | `scripts/convert_to_png.py` のみ | C+T。Pillow供給と両unittest系統＋perf self-test成功 |
| S03 | Rust `.rs` のみ／Rust testのみ | C+T+R。DがskipでもR実行。Rust fixtureに依存するTooling検査を含む |
| S04 | root／crate Cargo manifest、lock、toolchain、deny設定 | C+T+R+D。このうちCargo manifest/lockのproduction変更はHelpレビュー未記録ならCで拒否 |
| S05 | CI、分類器、driver、環境guard、契約test、Skill、`_rules.md`、DEVELOPMENT | C+T+R+D。通常Python／文書の規則より優先 |
| S06 | runtime `.ron/.wgsl/.png`、未知拡張子・未知root | C+T+R+D。必要な実機受入は別の判断項目 |
| S07 | 文書＋Rust、Rustからdocsへの移動 | C+T+R。削除元のRustが失われない |
| S08 | 文書＋tool、toolから未知pathへの移動 | 前者C+T、後者full |
| D01 | PR head／base／merge Tの関係が正常、baseに無関係なRust変更 | `G..H ∪ B..T` を分類。base側変更だけで文書PRを常にfullにしない |
| D02 | zero/missing base、Tの親不一致、取得失敗、未対応event | 計画生成失敗。Rustを開始せず集約も成功不可 |
| D03 | 複数commit push／force push／大量path／空白・改行path | 全path取得。non-fast-forwardはfull。各diffのhygieneも検査 |
| D04 | 手動base省略／自己指定／非祖先、手動full＋有効base | 前3件は失敗、最後はfull。比較元不明をfullで迂回しない |
| D05 | staged＋unstaged＋未追跡、検証中のsource変更 | 全差分を分類。開始／終了fingerprint差で失敗。ignored生成物だけなら不変 |
| D06 | 途中commitにproduction変更があり最終treeでrevert済み | 分類とは独立して既存Help batchレビューを実行。基点をHEADへ変えて通さない |
| A01 | 必要job success、不必要job skipped、schema／SHA／hash一致 | 集約success |
| A02 | 必要job failure/skipped/cancelled、出力欠損／文字列boolean／未知key／SHA不一致 | 各ケースで集約failure。CI全cancelでjobが起きなければ成功証拠なし |
| A03 | D非選択でskipped、T/R選択 | Rが暗黙のneeds skipに巻き込まれず実行 |
| A04 | C失敗、R選択 | Rを開始せず、集約failure（GitHubで代表実走） |
| A05 | failed jobsだけ再実行、対象planは同一 | run_attemptの増加だけでは既成功jobとの照合を壊さない |
| A06 | 依存監査workflowだけ成功 | 品質checkなし。required check／品質完了の代替不可 |
| P01 | 新runで旧PR runをcancel、手動fullと日次auditが併存 | 旧PRのみ置換、品質と監査で相互cancelなし |
| R01 | root規則／task lifecycle／Skillの必須block欠落、Skill同期ずれ | rule gate失敗。archiveの歴史記録は誤検出しない |

S04〜S06等の分類網羅はGit fixtureで行い、すべてに高価なGitHub fullを重複実走しない。GitHubでは実装PRのfull、子PR3分類、A04、M6bのトリガーを必須とする。
群の実行テストでは単に実装と同じリストを比較するだけでなく、失敗時の後続抑制・群別preflight・Cargo guard経由・未選択compileが起きないことを確認する。
ToolingにはRust／Blender実行ファイルのstubを置いた環境でも通常の全tooling testsが通ることを確認し、実compilerやGPUへ隠れた依存があれば分割完了前に解消する。

### 検証データ管理（各バッチの開始前・報告前に更新）

- 正本: `docs/development-infra/validation-storage-workflow.md`。ローカルの検証workspace／jobを作る場合はprimaryの `dev.py validation` で登録・結果確定・整理する。
- 今回の所有者: Codex / change-aware-ci。Git common directoryのretain IDは`change-aware-ci-candidate`、consumerは`change-aware-ci-implementation`。通常の品質検証はprimaryの既存開発cacheを再利用し、native job・binary copyは作らない。
- 保持path: `target/change-aware-ci`、branch `codex/change-aware-ci`、base `3ec4a76bbb2a2b1f7ad5f32a38a308bb036ae944`。開始27,004,928 bytes、2026-09-20時点27,435,008 bytes。コード候補のレビューと公開準備に使用し、docs正本はprimaryに限定する。
- 次の作業: primaryの確定docsと実装差分を、commit/公開が依頼された際に候補branchへ集約する。release_when: primaryへ採用済みかつ候補のreview/CI consumerが終了した時。修正中は同じworktreeを再利用する。
- 回収量0 bytes。共有の既存cacheは本計画の所有物とせず、既存consumerを変更しない。テスト用一時Git repositoryとcompiler stubは各テスト終了時に撤去済み。
- 実装時に記録する項目: batch ID、判断対象、責任者／consumer、base/head/tested SHA、worktree／job roots、開始bytes、結果、採用証拠、削除path／回収bytes、残存理由と終了条件。
- GitHub evidenceはrun URL・必要なSummary／結果を正本へ記録する。全ログ・全jobのcapsule化は不要。
- 一時検証環境を作った場合は最終consumer終了後に撤去する。保存すべき証拠があれば小さなcapsuleへ集約し、hashと回収量を記録する。既存・並行作業の変更やcacheを削除しない。

## 8. ロールバック方針

- 振り分け不具合時は、常設の集約checkを維持したまま全群実行へ戻せる構造にする。旧required check名へ即時に戻して保護を壊さない。
- `dev.py verify` の全検証入口を残すため、分類を使わないローカル／CI検証へ切り替えられる。
- ルールは運用実態と同じ変更単位で戻す。CI代替が利用不可ならローカル全verifyを正式なfallbackとする。
- 破棄やrevertの前に直近履歴・全対象diff・並行作業の所有を確認する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- M1〜M5・M6a完了。M6bのmaster pushと手動fullは成功。独立監査の不具合修正と自然schedule観測を継続中。詳細は「M6受入記録」を参照。
- primaryのCI・driver・rules・Skill・開発ガイド・templateを更新済み。全verify成功後の分類修正はfocused testとtooling検証で確認した。ゲームのRust/runtime assetは変更していない。
- 作業開始時の既存差分: `docs/README.md` の建築art文書リンク、`docs/art-style-criteria.md`、未追跡の `docs/building-art-direction.md`。本計画とは別の変更として保持する。

### 次のAIが最初にやること

1. primary/candidateのstatusとHEADを再確認する。別sessionの建築アートcommit・差分を変更しない。
2. PR #25の全群成功を確認してmasterへ反映し、修正後のmasterと手動監査結果を記録する。M6の公開・merge・整理はユーザーから許可済み。
3. 2026-09-20 03:17 UTC以後のDependency auditの`event=schedule`を確認する。起動遅延は許容し、手動成功で代用しない。自然起動成功と環境整理の完了後、本計画をarchive／削除して両indexを更新する。

### ブロッカー／注意点

- 技術上の確定ブロッカーなし。CLI・schema・job・diff式・分類優先順は本書で確定済み。実装時の外部確認は公開許可、最新の保護設定、required check新設の採否、実際のrun結果。
- コード編集は主担当が直接行い、subagentへ委譲しない。実装はユーザーの「実装してください」に基づく。公開・merge・保護設定は各操作の許可範囲に従う。
- 差分基点不足でHelp gateを迂回しない。CI成功と実レビューは別の義務。

### 参照必須ファイル

- `.github/workflows/ci.yml`、`scripts/dev.py`、`scripts/check_help_impact.py`。
- `scripts/check_agent_rules.py`、`scripts/sync_agent_skills.py`、`scripts/tests/test_dev.py`。
- `docs/DEVELOPMENT.md`、`docs/help-screen.md`、`docs/development-infra/validation-storage-workflow.md`。
- M4の各rules／Skill、`docs/plans/plan-template.md`。

### 最終確認ログ

- 2026-09-19: `python3 scripts/dev.py docs --write` 成功。plans indexへ1件追加、proposals indexは変更なし。文書リンク・root index検査成功。
- 2026-09-19: `python3 scripts/dev.py docs --check`、`git diff --check`、新規計画書の `git diff --no-index --check` 成功。
- 2026-09-19: `python3 scripts/check_agent_rules.py` 成功。`python3 scripts/check_help_impact.py` も成功（既存branchのproduction batchに対する結果。今回の文書差分の実装レビューを意味しない）。
- 2026-09-19レビュー: master protectionは `Branch not protected`、適用rulesは `[]`、default branchはmaster／publicとread-only確認。Rust fixtureを読むperf self-test、Helpのmerge-base／commit列挙、GitHubのskip／手動check仕様を確認して計画を修正。
- 2026-09-19レビュー後: docs生成／freshness、文書リンク、AI rule、既存Help gateを再確認してpass。tracked diffの空白検査と未追跡の本計画の空白検査に指摘なし。`git diff --no-index --check` は新規ファイルとの差分ありでexit 1になり得るため、診断出力の有無も確認する。
- 2026-09-20: primaryで`python3 scripts/dev.py verify`成功。contracts/tooling/deps/rust全群、通常・profiling workspace test、memory/tracy/renderdoc最小feature、Clippy `-D warnings`、online依存監査を通過。`python3 scripts/dev.py check`も成功。
- 2026-09-20最終: `python3 scripts/dev.py ci check --base 3ec4a76bbb2a2b1f7ad5f32a38a308bb036ae944 --mode auto`も全4群成功。HEAD=`a5bd322e68892a77abb106ee1cdce381a2526748`、開始/終了source fingerprint=`01457ff9ce48d96d95971d00c2d52a4f5aa8e799f7d966e43a1771cf1c856084`（この結果追記前のsnapshot）。途中の試行はMemAvailableが8 GiB未満のためRust開始前に拒否され、メモリ回復後に同じコマンドを成功させた。ガードは変更していない。
- 2026-09-20: Python tooling 238件＋Blender tooling 151件、perf self-test成功。Cargo/rustc/rustup/Blenderを失敗するstubに置換したPATHでもtooling全群成功、実compiler呼出し0件。追加の分類中変更・SHA取得fixtureを含むfocused CI/rule testも成功。
- 2026-09-20: reviewでGitHub output名`plan_sha256`の数字許可、stagedとunstagedの相殺による見落とし、分類中のsource変更を修正。plan→group→aggregateの実output接続、revert済みproduction commitのHelp拒否、欠落event SHA取得/失敗、未選択depsと選択rustの集約をfixtureで確認。
- Help実レビュー: **No impact**。変更経路は`dev.py`→品質群／Git分類／Actions結果出力と開発用ルールで完結する。`build_help_panel_content`→`manifest::feature_specs()`／provider→`hw_ui`の静的表示、InputAction・UiIntent・通常ゲーム状態・runtime assetへ変更を加えておらず、プレイヤー操作・文言・成立条件は不変。`3ec4a76b`以後とdirtyを対象にHelp gateも「no production changes」で成功。理由の自動注入・Help sourceへの空変更は行っていない。
- Rust sourceは未変更のためrust-analyzer診断の新規取得とnative受入は対象外。workspace compile/Clippy/testで既存Rust契約を確認した。
- sandbox内で一部testのIPCと`.git`台帳lockが拒否された実行は成功扱いにせず、同じ検証を承認済みの制限外実行で再確認した。
- GitHub受入: M6a完了、M6b進行中。「M6受入記録」にURL・SHA・群・cache・時間を記録。required checkは新設しない判断とし、保護なしを維持。

### Definition of Done（実装完了時）

- [ ] M1〜M6を完了し、分類別の必要検証と失敗伝播を確認した。
- [x] 全verifyとの対応表があり、既存検証の意図しない脱落がない。
- [x] 文書のみのCIでRust buildが0件、Rust変更ではworkspace全体を検証する。
- [x] 対象変更のcheck・Clippy警告0・全Rust testを含む初回導入の全検証が成功した。
- [x] 集約は必要jobのskip／cancel／欠損を成功扱いせず、独立した依存監査から品質checkを作らない。
- [x] required check新設の採否を記録した。採用時は実際の保護を確認し、未採用時は「保護なし」と報告した。
- [x] ルール・Skill・task lifecycle・開発ガイド・templateの完了条件を同期した。
- [x] 通常開発のbranch作成／再利用、PRによるCI開始、同目的修正の継続、終了後の整理をルールに反映した。
- [ ] Help実レビュー、必要な実機受入、元環境のstorage整理を別途完了した。
- [ ] SHA・実行範囲・CI URLと残務を記録し、専用検証環境を整理した。
- [ ] 永続仕様への反映、本計画のarchive／削除、両indexの再生成を完了した。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-19 | Codex | 変更別CI分類、検証群分割、集約判定、ルール同期、GitHub受入と完了条件を含む初版を作成 |
| 2026-09-19 | Codex | 計画レビュー。Rust→Tooling依存、保護未設定、監査workflow分離、CLI／schema／diff／job仕様、local dirty検証、M6a/M6b導入順と受入fixtureを具体化 |
| 2026-09-19 | Codex | 通常開発のbranch作成／再利用基準、PRによる自動CI開始、並行作業の保全、終了後の整理を追加し、M4とDoDへ反映 |
| 2026-09-20 | Codex | 品質群・Git分類・集約・Actions・rules/Skill/docsを実装し、ローカル全verifyと異常系fixtureを確認。GitHub受入・公開・導入は未実施として継続 |
| 2026-09-20 | Codex | M6aの全分類・失敗伝播・取消分離を受入、PR #20をmasterへ反映。M6bのpush/full成功、独立監査のnull inputs不具合を修正。子PR・branchを整理し、自然schedule待ちを明記 |
