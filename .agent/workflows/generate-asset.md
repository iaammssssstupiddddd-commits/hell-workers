---
description: 画像生成と透過PNG変換のワークフロー。Bevyで確実に動作する透過PNGを作成するための手順。
---

# /generate-asset ワークフロー

このプロジェクトで UI アイコンやスプライトを生成する際は、以下の手順を厳守してください。

## 1. 画像生成（`generate_image`）
- **プロンプト**: 
    - 必ず「背景を純粋なマゼンタ（solid pure magenta background, #FF00FF）」にするよう指定する。
    - 透過（transparent background）は指定**しない**。AI が格子模様を描き込むのを防ぐため。
- **スタイル**: 必要に応じて「ピクセルアート（pixel art style）」や「32x32」などのサイズを指定する。

## 2. 透過 PNG への変換
// turbo
- 次のコマンドを実行して、生成された画像を変換する。
    ```bash
    python3 scripts/convert_to_png.py "生成された画像パス" "assets/textures/対象パス.png"
    ```

## 3. アセットの検証
// turbo
- PNG 署名を確認する。
    ```bash
    head -c 8 "assets/textures/対象パス.png" | od -An -t x1
    ```
- 出力が `89 50 4e 47 0d 0a 1a 0a` であることを確認する。

## 4. コードへの適用
- `src/` 内のコードでロードする際は、必ず `.png` 拡張子を使用する。
