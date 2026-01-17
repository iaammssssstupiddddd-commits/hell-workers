# Development Guide (for AI & Humans)

本プロジェクトを開発・保守する上での重要なガイドラインです。

## 開発サイクル

1.  **Planning**: `implementation_plan.md` を作成し、ユーザーの承認を得る。
2.  **Execution**: コードを実装し、`cargo check` で型安全性を確認する。
3.  **Verification**: 動作を確認し、`walkthrough.md` で成果を報告する。

## 開発ルール

### 1. Rust-analyzer 診断の厳守
- コンパイルエラー（赤い波線）を一つも残したまま完了報告をしてはいけない。
- `cargo check` が通ることを必ず確認する。

### 2. 死蔵コードの禁止 ([deadcode.md])
- 将来使う予定があっても、現在使われていないコードや `#[allow(dead_code)]` は残さない。

### 3. 画像生成と透過 PNG ([image-generation.md])
- アイコン等は `generate_image` で背景をマゼンタ (`#FF00FF`) にして生成する。
- `scripts/convert_to_png.py` を使用して透過 PNG に変換する。
- 変換後はバイナリ署名を確認する： `89-50-4E-47-0D-0A-1A-0A`

## 便利なコマンド

### コンパイル確認
```powershell
cargo check
```

### 画像変換
```powershell
python scripts/convert_to_png.py "source_path" "assets/textures/dest.png"
```

### PNG署名確認
```powershell
powershell -Command "[BitConverter]::ToString((Get-Content 'file_path' -Encoding Byte -TotalCount 8))"
```

## コミット・進捗管理
- タスクの状態は `task.md` で逐一管理すること。
- 大規模なリファクタリングの際は、`proposals/` に経緯を残すことを検討する。
