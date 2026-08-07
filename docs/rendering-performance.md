# レンダリングパフォーマンス

描画パイプラインごとの draw call 構造・バジェット・最適化方針をまとめる。

---

## 1. パイプライン構成

このゲームは 3 つの独立した描画パイプラインを持つ。draw call のカウントは各パイプラインで別枠になる。

| パイプライン | 内容 | draw call に影響するもの |
|---|---|---|
| **3D RtT** | 地形・建築物・Soul | `Camera3dRtt` の frustum 内にある 3D entity |
| **2D world** | 夢の泡パーティクル・前景スプライト | `Material2d` / `Sprite` を持つ 2D entity |
| **UI** | DreamBubbleUiMaterial・UI ノード | UI パイプライン（`UiMaterial` 等） |

### P00 current inventory（source/startup固定）

single Scene RtT / indoor light migration前のcurrent構成は次である。`startup_systems`の
`current_rtt_startup_inventory_is_explicit`が実際にstartup / composite spawn systemを実行し、カメラの
order / active / RenderLayers、RtT image descriptor、DirectionalLight marker / 有効状態、compositeの
2 texture参照、`LAYER_2D` 2 passを検証する。これはsource/startup側の契約であり、GPU replayのpass構造は
下記のformal evidenceで別に照合する。

| 項目 | current |
|---|---:|
| Camera3d RtT | 2（Scene 1、Soul mask 1） |
| world color target | 2（Scene 1、Soul mask 1） |
| Camera2d | 3（Main、Overlay、WorldForeground） |
| FHD High target | 各1920×1080、2 handleはdistinct |
| DirectionalLight entity | 2（標準1 active、extra 1 default disabled） |
| composite sampled texture | 2（Scene + Soul mask） |

P00のmeasurement contractはfrozenの`rtt-light-v1`である。canonical contract hashは
`121a365ac3349cd4fa7890ab3069f0392098ced17e0d47f920095a1490c2ba11`、fixture hashは
`a688d564f8f50c2fdcdbe49dca7625b2cb05d01f8555378215fb8ba89b553eed`である。freeze後の変更は同じv1を
編集せず新generationを追加する。

### P00 registered current formal baseline（2026-08-07）

正式artifactはGitへ追加せず、`target/perf-runs/rtt-light/rtt-light-v1/`に保持する。比較referenceは
`current-43e353eeb90f6c93/attempts/0e0b8d96-d757-4505-9cd8-c5f093387f8c`だけであり、invalid / interrupted
attemptは削除もreference登録もしない。

| 項目 | 登録値 |
|---|---|
| subject / source fingerprint | `43e353eeb90f6c930366b8e9e275088e55275c16` / `a47fa2318c76a5c1331a285dcfe750e7092477874128c1511575c2a051ac5b86` |
| environment | Intel(R) Arc(tm) Graphics (MTL)、Intel open-source Mesa driver / Mesa 26.1.5、Vulkan、X11、1920×1080、scale 1.0、High、effective immediate present |
| environment lock | `c87c01f9286bf035414ef365c4d5cde15340d8b1178ec6619ed4b527c9d08d99` |
| five formal legs | audit 9、behavior 6、Capture 18、RenderDoc 1、Memory 18 valid run。invalid 0 |
| gate / raw provenance | required gate 94 row pass。gate CSV `1cf6cc99d5da07d1fcd7f3217cfddda86dba6bee1f1ddbf0333a3ed889b45327`、raw directory 881 files / `c9f1fedb59a274ed92909ecc127bb8260ae6c28f41b5c1bd9becae53c1c54a7a`、root `SHA256SUMS` `67e305c9518e8ee0a3bdeb6446bd15e632177e2e519ed502b57dc29c15a67a3b` |
| offline revalidation | `verify-rtt-light`、`verify-rtt-light-baseline`ともにpass。baseline rootは886 filesで`status=valid` |

Captureのp50は診断値、p95 / p99はfrozen gateに対する比較値である。`cpu` / `gpu`は描画構成の切替名であり、
CPU / GPU時間そのものではない。

| size | render | p50 ms | p95 ms | p99 ms |
|---|---|---:|---:|---:|
| small | cpu | 5.790177 | 7.042881 | 7.885782 |
| small | gpu | 41.877964 | 47.355937 | 49.723764 |
| medium | cpu | 27.266096 | 42.250438 | 47.900270 |
| medium | gpu | 43.056060 | 48.749744 | 50.982734 |
| large | cpu | 28.525122 | 61.455438 | 76.117119 |
| large | gpu | 46.272950 | 61.974057 | 76.575507 |

Memory legのframe値は性能比較には使わず、Rust allocatorのpeak live bytesとGNU timeのprocess peak RSSを
allocation / process-footprint referenceとして使う。全caseでallocator accounting errorは0だった。

| size | render | peak live bytes | peak RSS KiB |
|---|---|---:|---:|
| small | cpu | 667496702 | 1399992 |
| small | gpu | 690490522 | 1521448 |
| medium | cpu | 715549403 | 1438772 |
| medium | gpu | 749008908 | 1579848 |
| large | cpu | 755769242 | 1384100 |
| large | gpu | 820362477 | 1640684 |

RenderDoc evidence schema v2はこのsource inventoryをGPU replayで厳密化する。registered replayは18 render
pass、212 draw、582 event、516 attachment、1996 bindingを確認した。composite event 1109は1 drawで、fragment
descriptor set 2のScene texture / sampler `(1, 2)`とSoul mask texture / sampler `(3, 4)`を同時に使う。
raw RDC SHA-256は`278b2d7f866b39f56648966ebd7d5994b689f2023f7253682b84aaca6bf2356f`である。抽出はVulkan
subpass transitionを正しく分割し、全drawに散らばったsampler数では代用しない。

---

## 2. draw call の基本規則

### 発生条件
- **画面内（frustum カリング後）** のエンティティのみ draw call を生成する
- 画面外・VRAM キャッシュ済みのアセットは draw call に含まれない

### 自動インスタンシング（Bevy）
同一の `Handle<Mesh>` かつ同一の `Handle<Material>` を持つ entity は自動バッチされ、インスタンス数に関わらず **1 draw call** になる。

### バッチが壊れる条件

| 条件 | 結果 |
|---|---|
| entity ごとに `materials.add(...)` でハンドルを生成している | 1 entity = 1 DC |
| 状態変化のたびに material を clone/mutate している | variant 数 × DC |
| `RenderLayers` が異なる | レイヤーごとに分離 |
| `AlphaMode::Blend` と `Opaque` が混在 | 透過パスと不透過パスで分離 |

---

## 3. 現行の draw call 構造（3D RtT パイプライン）

### 地形

| 要素 | DC 数 | 備考 |
|---|---|---|
| `TerrainSurfaceMaterial` (LOD1) | 1 | 49 chunk が同一ハンドルを共有 |
| `TerrainSurfaceMaterialLod1Lite` (LOD1-lite) | 1 | 同上 |
| `TerrainSurfaceMaterialLod2` (LOD2) | 1 | 同上（LOD 切替で一方だけが有効） |

LOD 切替閾値（hysteresis）:

- `LOD1 -> LOD1-lite`: `tile_rtt_px < 22px`
- `LOD1-lite -> LOD1`: `tile_rtt_px > 25px`
- `LOD1-lite -> LOD2`: `tile_rtt_px < 14px`
- `LOD2 -> LOD1-lite`: `tile_rtt_px > 16px`

### 建築物（現行プレースホルダー）

| 要素 | DC 数 | ハンドル管理 |
|---|---|---|
| 壁（完成） | 1 | `Building3dHandles.wall_mesh` + `wall_material` |
| 壁（建設中） | 1 | `wall_provisional_material`（別マテリアル） |
| 床・ドア・設備 | 各 1 | 種類ごとに 1 ハンドル |

`Building3dHandles`（`startup/visual_handles.rs`）が全ハンドルを Resource として保持し、
entity はこれを clone して参照するため、インスタンス数が増えても DC 数は変わらない。

### キャラクター（Soul）

| 要素 | DC 数 | 備考 |
|---|---|---|
| body mesh | ≒1〜数 DC | `CharacterMaterial` 共有だが GLB 子孫に分散 |
| face mesh | Soul 数 × DC | face は Soul ごとに material を複製（uv_offset のため） |
| shadow proxy | 別 DC | `LAYER_3D_SOUL_SHADOW` |
| mask proxy | 別 DC | `LAYER_3D_SOUL_MASK` |

---

## 4. LOD0 建築物の draw call バジェット（将来）

### 前提

| 変数 | 値 |
|---|---|
| RtT 解像度 (High/FHD、DPI 1.0) | 1920 × 1080 |
| tile_rtt_px（LOD0 仮定、DPI 1.0） | 32 px |
| 1 world unit（orthographic scale 1.0） | 1 logical px = `Window DPI × RtT quality` physical RtT px |
| カメラ仰角 | 59°（VIEW_HEIGHT=150, Z_OFFSET=90） |

RtT Camera3d の `ImageRenderTarget.scale_factor` は `Window DPI × RtT quality` であり、
`world_to_viewport` の logical target px は LOD 観測時に同じ倍率を掛けて physical `tile_rtt_px` へ戻す。
したがって上の面積・triangle 概算は DPI 1.0 / High の基準値である。

### 可視ピクセル数（59° 投影係数）

| 面の向き | 投影係数 |
|---|---|
| 水平面（上面） | cos(59°) ≈ 0.515 |
| 垂直面（前面） | sin(59°) ≈ 0.857 |

設備の推定可視面積：

| 設備 | 仮定高さ | 可視面積 |
|---|---|---|
| 1×1 (Tank 等) | 1.5 tile | ~1,840 px |
| 2×2 (MudMixer 等) | 1.8 tile | ~5,760 px |

### Triangle バジェット導出

micropolygon 下限（1 tri ≥ 4 px²）× カリング率（可視率 35%）から GLB total tri を逆算:

| 建築物 | 可視 tri | GLB total (÷0.35) |
|---|---|---|
| 壁 1×1 | 80〜175 | **150〜350 tri** |
| 設備 1×1 | 460 | **~1,300 tri** |
| 設備 2×2 | 1,440 | **~4,100 tri** |

### 20 種類での draw call 数

Trellis 等で生成した GLB を種類ごとに 1 ハンドルで管理すれば:

```
20 種類 × 1 DC/種類 = 20 DC（全建築物合計）
```

建設中/完成の 2 状態を別 material handle にしても 40 DC。
現代 GPU では問題ないレベル。

### インスタンシングを壊さないための運用ルール

1. **GLB ロード時に種類ごとに 1 ハンドル**を `Res` に格納し、entity は clone して参照する
2. **状態変化は `commands.entity().insert(MeshMaterial3d(handle.clone()))` で差し替える**
   - `materials.get_mut(handle)` で mutate しない（他のインスタンスのバッチも壊れる）
3. **per-entity material clone は禁止**（現行の `soul face` は必要性があるため例外）

---

## 5. 2D パイプライン：夢の泡パーティクル（要対応）

### 現状の構造

world-space の夢泡は `Mesh2d + DreamBubbleMaterial` で描画する。
現在は以下の共有構造に整理されている。

| 要素 | 現在の構造 |
|---|---|
| mesh | `DreamBubbleHandles.circle_mesh` を全粒子で共有 |
| world material | `DreamQuality × alpha bucket` の 24 ハンドル共有 |
| shader time | `DreamBubbleMaterial.time` ではなく `globals.time` を使用 |
| alpha 更新 | `Assets::get_mut` ではなく `MeshMaterial2d` の handle 差し替え |

`DREAM_PARTICLE_MAX_PER_SOUL = 5` なので、
50 体睡眠時のアクティブ粒子数上限は依然として 250 だが、
material asset 数と per-frame material mutation はこの上限に比例しない。

### 改善方針の概要

`(DreamQuality × alpha_bucket)` のマテリアルプール（24 ハンドル）を `Resource` に保持し、
粒子は bucket が変わったときだけ handle を差し替える。

alpha bucket の運用:

- `bucket 7` は現行と同じ `alpha = 0.85`
- `bucket 0` は `alpha = 0.0`
- `bucket 0` に入った粒子は不可視のまま slot を占有しないよう早期 despawn する

期待できる効果:

- mesh asset 数: 粒子数依存 → 1
- world material asset 数: 粒子数依存 → 24 固定
- world-space per-frame `Assets::get_mut`: 粒子数依存 → 0

注意:

- transparent 2D mesh は sorted phase なので、draw call は shared handle だけでは決まらない
- 同じ mesh / material を共有していても、Z 順で隣接したものしか batch されない
- したがって **24 ハンドル = draw call 上限** ではない

UI 側の `DreamBubbleUiMaterial` は world-space 版と同様に `time` フィールドを削除し、
`@group(0) @binding(1) var<uniform> globals: Globals;` で `globals.time` を shader 内で直接参照する方式に変更済み。
これにより per-frame の `Assets::get_mut` 呼び出しがパーティクル数に比例して発生していた問題を解消した。

UI material は `velocity_dir` のような粒子ごとの時間変化uniformを持たず、alpha × mass × color の
`8 × 4 × 2 = 64` 個の共有handleだけを使う。粒子側はbucketが変わったときだけ
`MaterialNode` のhandleを差し替えるため、粒子数に比例するmaterial asset生成・mutationは行わない。

`TaskAreaMaterial` も同様に `time` フィールドを削除し `globals.time` を使用するよう変更済み
（`mesh2d_view_bindings::globals` 経由、`@group(2)` マテリアルバインドへの毎フレーム書き込みを排除）。

→ world-space 泡の draw call 最適化詳細は `docs/plans/dream-bubble-perf-2026-04-09.md`

---

## 6. テクスチャキャッシュ（draw call と独立した懸念）

draw call 数とは別に、GPU オンチップテクスチャキャッシュ（数 MB）のスラッシングが
フラグメントシェーダーのスループットを落とす場合がある。

| 対策 | 内容 |
|---|---|
| テクスチャアトラス化 | 同素材グループを 1 枚の大テクスチャにまとめる |
| テクスチャ共有 | 同種の建築物は同一テクスチャハンドルを使う |
| 解像度の適正化 | LOD2 では使われないテクスチャは 256px 以下でよい |

Trellis 生成 GLB は各モデルが独立テクスチャを持つため、
多種同時表示時はキャッシュミスが増える。20 種程度なら許容範囲内。

---

## 7. フラグメントシェーダーコスト（参考）

draw call ではなくピクセルコストの観点。LOD0 (32px/tile, FHD) での概算。

| カテゴリ | 占有 px | テクスチャサンプル/px | フレーム総サンプル |
|---|---|---|---|
| 地形 LOD1 | ~1,760,000 | ~15 | **~26M** ← 支配的 |
| 地形 LOD2 | ~1,760,000 | ~4 | ~7M |
| 建築物 PBR | ~250,000 | ~5 | ~1.25M |
| 建築物 Unlit | ~250,000 | 1〜2 | ~250k〜500k |

建築物の fragment コストは地形 LOD1 の 1/10 以下であり、
Trellis 生成の高ポリゴンモデルを使っても fragment 面での影響は軽微。
ポリゴン数の増加は頂点シェーダーコストに影響するが、
50k tri × 30 インスタンス = 1.5M tri/frame は現代 GPU で問題ない。
