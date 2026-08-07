# Development Tools

ローカルとCIの品質ゲートは `scripts/dev.py` を正本とする。Python標準ライブラリ
だけで動作し、workspace rootを自動解決するため、どのディレクトリから呼んでも
同じCargo workspaceを対象にする。

## 基本コマンド

```bash
# 必須/任意ツール、Rust、mold、assetsのread-only診断
python3 scripts/dev.py doctor

# 日常の高速ゲート
python3 scripts/dev.py check

# package限定（必要ならtestsも実行）
python3 scripts/dev.py check --package hw_jobs --tests

# CIと同一の完全ゲート
python3 scripts/dev.py verify

# 暗黙cleanupを行わないbuild
python3 scripts/dev.py build
python3 scripts/dev.py build --release

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

通常のcheck/buildはビルドキャッシュを削除しない。容量整理が必要な時だけ、対象と
影響を確認して `post-build-cleanup.sh` / `.ps1` や各OS向けmaintenance scriptを
明示実行する。クロスターゲットの成果物を自動削除しないこと。

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
