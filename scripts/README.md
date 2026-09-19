# Development Tools

ローカルとCIの品質ゲートは `scripts/dev.py` を正本とする。driver自体はPython標準ライブラリ
で動作し、workspace rootを自動解決するため、どのディレクトリから呼んでも
同じCargo workspaceを対象にする。

## 基本コマンド

```bash
# 必須/任意ツール、Rust、mold、assetsのread-only診断
python3 scripts/dev.py doctor

# 日常の高速ゲート
python3 scripts/dev.py check

# package限定（必要ならtestsも実行）
python3 scripts/dev.py check --package hw_jobs --tests

# 変更に応じた完了ゲート（staged/unstaged/untrackedも含む）
python3 scripts/dev.py ci check --base <full-SHA> --mode auto

# 分類を使わない完全ゲート
python3 scripts/dev.py verify

# 固定版Ruff/actionlintと依存監査
python3 scripts/dev.py lint
python3 scripts/dev.py deps
python3 scripts/dev.py deps --offline

# 暗黙cleanupを行わないbuild
python3 scripts/dev.py build
python3 scripts/dev.py build --release

# 修正中の見た目・操作確認: dev + profiling feature、incrementalを強制有効化
python3 scripts/dev.py feedback
python3 scripts/dev.py feedback --build-only
# -- 以後はCargoではなくゲームの引数
python3 scripts/dev.py feedback -- --spawn-souls 20

# 2窓運用: 空いているlaneを開始時に取得し、shell終了まで固定
python3 scripts/dev.py lane status
python3 scripts/dev.py lane shell
# shellを開かずに1コマンドだけlaneを使う場合
python3 scripts/dev.py lane shell -- python3 scripts/dev.py check
```

`lane shell` は `target/lanes/a` または `target/lanes/b` をOSの `flock` で占有し、
そのshellから起動した `scripts/dev.py` のCargoを同じlaneへ固定する。セッション中に
別laneへ移動したり、2 laneとも使用中に標準 `target/`へfallbackしたりしない。3つ目の
sessionは明示的なbusyエラーで終了する。laneごとのCargo jobは1に固定されるため、2窓
合計のcompile fan-outは2を超えない。native acceptanceとperformance runnerは引き続き
canonical `target/`を使う。lane leaseはPOSIXの`flock`を使い、未対応hostでは共有targetへ
fallbackせず明示的に停止する。

lane cacheは自動削除しない。容量を確認するときは `du -sh target/lanes/a target/lanes/b`
と `python3 scripts/dev.py lane status` を明示的に実行し、不要になったlane成果物だけを
確認後に保守作業で整理する。

対話Cargoのcompile / run / test / clippyは `target/.cargo-activity.lock` のshared leaseを
Cargo childの生存中だけ保持する。performance runnerとnative acceptance recipeは同じlockの
exclusive leaseをrecipe全体で保持し、競合時は子processを起動せずbusyとして終了する。
lane lease（session所有）とactivity lease（実行中資源）は別物であり、idleなlane shellは
performance/nativeを妨げない。

互換wrapperとして `scripts/check.sh` / `check.ps1`、`scripts/build.sh` /
`build.ps1` も残している。wrapperは引数を `dev.py` へ渡すだけで、ログファイル作成、
Cargo出力の再解釈、`target/`の削除を行わない。

## 品質ツール

`quality.py`はcontracts/tooling/deps/rustの共通実行器、`ci_scope.py`はGit差分と不変のevent SHAに基づく分類、
`ci_result.py`は必要jobの成功とSHA/digest一致を確認する集約器。群の切り分けには
`dev.py quality --group contracts|tooling|rust|deps`を使う。CIでは必ず`--plan-json`を渡し、eventから再計算して照合する。
`dev.py ci plan --github-event ... --github-output ...`と`dev.py ci result --plan-json ... --needs-json ...`はCI用入口。
選択条件・branch運用・同一対象のCI証拠要件は[開発ガイド](../docs/DEVELOPMENT.md)を参照する。

full verifyに必要な3 CLIの正本は`dev-tools.toml`、版/path照合は`dev_tools.py`。
Linux x86_64の明示導入は`python3 scripts/install_dev_tools.py --bin-dir "$HOME/.local/bin"`を使い、
同directoryをPATHへ加える。公式archiveのSHA-256不一致なら既存binaryを置換しない。
doctor/verifyは暗黙installを行わない。Ruffはcacheなし、actionlintはShellCheck/Pyflakes連携なし、
depsは全workspace/feature/targetのnormal/build/dev依存を監査しlockを変更しない。
offlineはcached DB診断として区別し、online失敗を成功へ読み替えない。
固定版更新、license/advisory方針、DependabotのHelp判断とGitHub受入は
[開発ガイド](../docs/DEVELOPMENT.md#quality-toolsfull-verifyで必須)を参照する。

## ドキュメント契約

```bash
# plan/proposal indexを明示更新し、link/indexも検査
python3 scripts/dev.py docs --write

# 非変更検査（CIで実行）
python3 scripts/dev.py docs --check
```

AIルールだけを切り分ける場合は `python3 scripts/check_agent_rules.py`、secret・
生成物・script modeは `python3 scripts/check_repo_hygiene.py`、Markdown linkは
`python3 scripts/check_docs.py` で個別に確認できる。
共有Agent Skill本文は`.cursor/skills/`版が正本で、Codex、Gemini、Claude adapterへの反映は
`python3 scripts/sync_agent_skills.py --write`を使う。新Skill追加時はscriptのmapping、
`check_agent_rules.py`のactive skill一覧、`scripts/tests/test_sync_agent_skills.py`も同時に更新する。
同期scriptは本文を書き換える前に、全adapterの期待`name`・非空`description`とCodex UI metadataを検証する。

## 容量メンテナンス

`feedback` は既存workspace/laneの `debug` cacheを使う。新profileやjobごとのtargetを作らない。
native合同storyboardも `--feedback` で同じbinaryを使用できる。正式の `profiling` binaryとは別であり、
feedbackの計測値は性能判断に使わない。詳しい使い分けは [開発ガイド](../docs/DEVELOPMENT.md) を参照する。

通常のcheck/buildはビルドキャッシュを削除しない。容量整理が必要な時だけ、対象と
影響を確認して `post-build-cleanup.sh` / `.ps1` や各OS向けmaintenance scriptを
明示実行する。クロスターゲットの成果物を自動削除しないこと。

検証job・binary capsule・検証用worktree / cloneの整理は
[検証データ管理](../docs/development-infra/validation-storage-workflow.md)を正本とする。
各検証バッチの報告前に担当者が結果確定・不要領域撤去まで実施する。全jobのarchiveは不要。
`python3 scripts/dev.py validation` は所有者・consumer・実測容量・終了条件の台帳と共通検査を提供する。
init / retain / plan / execute / seal / finalize / reconcile / checkはprimaryから使う。sealは独立検証と結果記録だけで、
コピーを作らない。checkは未登録・未整理・未分類残存と任意の明示予算を検査し、終了済みcapsuleを監視しない。
既定の容量上限・日数はなく、review_atは助言のみ。`dev.py verify`もcheckを実行する。
coordinatorはデータを自動削除しない。凍結した旧helperはprimary coordinatorの子としてそのまま実行する。
通常Cargo cacheの維持と、検証用データをtrack closeまで残すことを混同しない。
フィードバック待ち・修正中のcandidate worktreeとtargetは継続利用し、不要job整理の対象から外す。
レビューの見直し日はcacheの削除期限ではなく、同じ作業場で差分ビルドを続けるための状態確認日とする。

## その他

```bash
# performance runner自己検査
python3 scripts/perf.py self-test

# 画像変換
python3 scripts/convert_to_png.py "source_path" "assets/textures/dest.png"

# 外部asset exportsの反映
python3 scripts/sync_external_assets.py --source <exports-dir>
```

GitHub認証は [GITHUB_TOKEN_UPDATE.md](GITHUB_TOKEN_UPDATE.md) を参照する。
