# アセット共有・同期ワークフロー

`Hell Workers` の画像・モデル・音声などのバイナリアセットを、`git` 以外で複数 PC 間共有するための運用仕様。

## 1. 方針

- 共有手段は `Syncthing` を推奨する。
- **原本** と **ゲームが直接読む実行用アセット** を分離する。
- 原本同期フォルダは **リポジトリ外** に置く。
- リポジトリ配下の `assets/` には、ゲームが直接読む最終ファイルだけを置く。

## 2. 推奨ディレクトリ構成

例:

```text
~/Sync/hell-workers-assets/
├── source/
│   ├── character/
│   ├── buildings/
│   ├── ui/
│   └── references/
└── exports/
    ├── textures/
    ├── models/
    └── audio/
```

- `source/`
  - Aseprite, PSD, Krita, Blender, 参照画像などの原本を置く。
  - ここを `Syncthing` で複数 PC 間同期する。
- `exports/`
  - ゲーム投入前の最終出力物を置く。
  - `png`, `glb`, `ogg`, `wav` など、Bevy が直接読む形式に揃える。

## 3. リポジトリとの責務分離

- リポジトリ外の `~/Sync/hell-workers-assets/source/`
  - 編集中の原本
  - 共同作業用の参照素材
- リポジトリ外の `~/Sync/hell-workers-assets/exports/`
  - `assets/` に反映するための中間成果物
- リポジトリ内の `assets/`
  - ゲーム実行時に読む最終アセット
  - `fonts/` と `shaders/` は既存ルールを維持する

この分離により、原本共有とゲーム実行用アセットの責務が混ざらない。`Syncthing` の競合や一時ファイルがリポジトリ運用へ直接流れ込むのも防げる。

## 4. 日常運用

1. `source/` で原本を編集する。
2. 最終形式へ書き出して `exports/` に置く。
3. `python scripts/sync_external_assets.py --source ~/Sync/hell-workers-assets/exports` を実行する。
4. `assets/` へ反映された内容をゲーム内で確認する。

マゼンタ背景付き画像から透過 PNG を作る場合は、既存の `scripts/convert_to_png.py` を使ってから `exports/textures/` に置く。

例:

```bash
python scripts/convert_to_png.py \
  "~/Sync/hell-workers-assets/source/ui/icon_idle.png" \
  "~/Sync/hell-workers-assets/exports/textures/ui/icon_idle.png"

python scripts/sync_external_assets.py \
  --source ~/Sync/hell-workers-assets/exports
```

## 5. `scripts/sync_external_assets.py` の責務

このスクリプトは、外部同期済み `exports/` からリポジトリ内 `assets/` へ、許可されたサブディレクトリだけをコピーする。

- 既定の同期対象: `textures`, `models`, `audio`
- 既定では削除を行わない
- `--delete-missing` 指定時のみ、コピー元に存在しない同期対象ファイルを `assets/` から削除する
- `fonts/` と `shaders/` には触れない

## 6. 競合回避ルール

- 同じ原本ファイルを複数 PC で同時編集しない。
- 原本のファイル名と書き出し先を安定させる。
- `Syncthing` の conflict file を見つけたら、原本側で必ず統合してから `exports/` を更新する。
- 大きな原本（例: `.blend`）は日次バックアップを別経路にも持つ。

## 7. 向いているケース / 向いていないケース

- 向いている:
  - 個人開発または少人数でのアセット制作
  - 複数 PC 間で同じ原本を扱いたい
  - クラウド専用 SaaS へ依存したくない
- 向いていない:
  - 同じバイナリを複数人が同時編集するワークフロー
  - 厳密なレビュー承認付き配布物管理

厳密な配布管理が必要になったら、`exports/` の公開先だけを `S3` / `Cloudflare R2` に切り替え、`assets/` 反映フローは維持する。
