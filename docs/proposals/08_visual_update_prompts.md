# ビジュアルアップデート用アセット生成プロンプト案

本ドキュメントでは、新しいアートスタイル「Rough Vector Sketch（手描き感の強いラフなベクターイラスト）」に基づいて、既存アセットを置き換えるための具体的な画像生成プロンプトを定義します。

## 概要

全てのアセットは共通して以下のスタイル指定を持ちます：
- `orthographic projection` (正投影)
- `straight placement` (水平・垂直配置)
- `rough hand-drawn sketch style` (ラフな手描きスケッチ)
- `loose ink outlines` (ルーズなインク輪郭)
- `isolated on solid gray background` (マゼンタ背景で分離)

---

## 1. キャラクター (Characters)

### 魂 (Damned Soul)
> ファイル: `colonist.png`

**プロンプト:**
```
Floating ghost figure, cute but sad expression, simple round body with translucent edges, pale blue spectral glow, orthographic projection, front view, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, flat coloring, isolated on solid gray background
```

### 使い魔 (Familiar)
> ファイル: `familiar_spritesheet.png`

**プロンプト:**
```
Small imp creature, red skin, tiny bat wings, pointed tail, holding a small whip, mischievous expression, orthographic projection, front view, straight vertical posture, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, flat coloring, isolated on solid gray background
```

---

## 2. 環境オブジェクト (Environment)

### 黒い木 (Dark Tree)
> ファイル: `tree.png`

**プロンプト:**
```
Dead gnarled tree, dark black wood texture resembling ribs, twisted branches, no leaves, spooky silhouette, orthographic projection, straight vertical trunk, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, slight purple highlights, isolated on solid gray background
```

### 岩 (Remorse Rock)
> ファイル: `rock.png`

**プロンプト:**
```
Large grey rock, rugged texture with deep cracks,  formation in the cracks suggesting agony, orthographic projection, straight bottom edge, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, flat coloring, isolated on solid gray background
```

---

## 3. アイテム・資源 (Items & Resources)

### バケツ (Bucket)
> ファイル: `bucket_empty.png`, `bucket_water.png`

**空のバケツ:**
```
Empty wooden bucket, dark wood texture, metal bands, rusty handle, old and worn look, orthographic projection, straight vertical placement, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, isolated on solid gray background
```

**水入りのバケツ:**
```
Wooden bucket filled with dark murky water, black liquid surface, dark wood texture, metal bands, rusty handle, orthographic projection, straight vertical placement, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, isolated on solid gray background
```

---

## 4. 建築物 (Buildings)

### 水タンク (Water Tank)
> ファイル: `tank_empty.png`, `tank_partial.png`, `tank_full.png`

**プロンプト:**
```
Industrial water tank, scavenged metal parts, rusty iron texture, cylindrical shape, visible pipes and valves, skeletal structure, orthographic projection, straight vertical alignment, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, flat coloring, isolated on solid gray background
```
*(中身の水の量は画像生成後の加工か、Variantで調整)*

---

## 5. 地面タイル (Ground Tiles)
※シームレスなテクスチャとして使用する場合は、生成後に加工が必要になる場合がありますが、ベースとなる素材のプロンプト案です。

### 土 (Dirt)
> ファイル: `dirt.jpg`

**プロンプト:**
```
Dark scorched earth ground texture, cracked soil, small pebbles, reddish-brown color palette, top-down view, orthographic projection, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, seamless pattern style, isolated on solid gray background
```

### 草 (Grass/Barren Ground)
> ファイル: `grass.jpg` (地獄の荒涼とした草地として解釈)

**プロンプト:**
```
Sparse patches of dry grey dead grass on dark soil, wilted vegetation, gloomy atmosphere, top-down view, orthographic projection, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, seamless pattern style, isolated on solid gray background
```

### 砂 (Sand/Ash)
> ファイル: `Sand.png` (地獄の灰の砂浜)

**プロンプト:**
```
Grey ash sand texture, soft grainy surface, small bone fragments scattered, monochromatic grey tones, top-down view, orthographic projection, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, seamless pattern style, isolated on solid gray background
```

### 川 (River)
> ファイル: `River.png` (忘却の川)

**プロンプト:**
```
Dark murky river water texture, black oil-like liquid, subtle purple reflection, swirling currents, top-down view, orthographic projection, rough hand-drawn sketch style, loose ink outlines, textured brush strokes, seamless pattern style, isolated on solid gray background
```
