---
description: 画像生成と透過PNG変換のワークフロー。Bevyで確実に動作する透過PNGを作成するための手順。
---

# /generate-asset ワークフロー

このプロジェクトで UI アイコンやスプライトを生成する際は、以下の手順を厳守してください。

---

## 1. ビジュアルスタイル定義 (Visual Style: Rough Vector Sketch)

本作のアートスタイルは、**「手描き感の強いラフなベクターイラスト（Rough Hand-drawn Sketch）」**で統一する。
「完成されすぎた綺麗な絵」ではなく、開発中のスケッチのようなアナログ感を意図的に残すこと。

### A. ラインワークと塗り (Line & Color)
* **Rough Ink Outlines:** 輪郭線は太く、インクが滲んだようなルーズさ（Loose lines）と揺らぎ（Wobbly）を持たせる。定規で引いたような直線は禁止。
* **Textured Brush Strokes:** 塗りは均一ではなく、筆跡や塗りムラ（Textured brush）を残し、「少し未完成（Slightly unfinished）」な味わいを出す。
* **Visual Density:** 描き込みすぎず、シルエットを重視したシンプルな情報量に抑える。

### B. 視点と構図 (Perspective & Layout)
**【最重要事項】** アセットが斜めに歪むのを防ぐため、以下のルールを厳守する。
* **Orthographic Projection:** 完全な正投影（平行投影）で描く。「3/4 view」という言葉は使用しない（斜め配置の原因になるため）。
* **Straight Alignment:** 壁や配管などの連結物は、必ず**「水平（Horizontal）」または「垂直（Vertical）」**に真っ直ぐ配置する。斜めの配置は禁止。
* **Face Visibility:** 壁などは「正面（Front face）」と「上面（Top edge）」が見える角度で描画し、立体感を表現する。

### C. 世界観設定 (Thematic Context)
* **基本トーン:** 暗く、錆びついた、不気味だが少しコミカルな世界。
* **壁・構造物:**
    * **黒い木材 (Dark Black Wood):** 魂の肋骨のような質感。歪んでいる。
    * **紫の光 (Purple Glow):** 裂け目から漏れる「怠惰のエネルギー」。
    * **錆びた鉄 (Rusty Metal):** トゲのあるバンドや補強パーツ。
* **形状:** ティム・バートン作品のように、少し歪んだり（Distorted）、傾いたりしているシルエット。

---

## 2. 画像生成（`generate_image`）

### プロンプト構成テンプレート

```
[Subject], orthographic projection, straight [horizontal/vertical] placement, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, flat but slightly unfinished coloring, [Specific Details: dark black wood, purple cracks, etc.], isolated on solid magenta background
```

### 必須事項
- **背景**: 必ず「背景を純粋なマゼンタ（solid pure magenta background, #FF00FF）」にするよう指定する。
- **透過指定禁止**: 透過（transparent background）は指定**しない**。AI が格子模様を描き込むのを防ぐため。

### ネガティブ要素（禁止事項）
* **禁止スタイル:** `photorealistic`, `3d render`, `clean vector`, `gradient`, `pixel art`
* **禁止構図:** `diagonal`, `perspective distortion`, `tilted`, `paper texture`, `shadows`

---

## 3. 透過 PNG への変換
// turbo
- 次のコマンドを実行して、生成された画像を変換する。
    ```bash
    python3 scripts/convert_to_png.py "生成された画像パス" "assets/textures/対象パス.png"
    ```

---

## 4. アセットの検証
// turbo
- PNG 署名を確認する。
    ```bash
    head -c 8 "assets/textures/対象パス.png" | od -An -t x1
    ```
- 出力が `89 50 4e 47 0d 0a 1a 0a` であることを確認する。

---

## 5. コードへの適用
- `src/` 内のコードでロードする際は、必ず `.png` 拡張子を使用する。
