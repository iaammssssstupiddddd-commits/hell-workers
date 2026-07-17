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
```

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
docs更新Skill本文は `.cursor` 版が正本で、adapterへの反映は
`python3 scripts/sync_agent_skills.py --write` を使う。

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
