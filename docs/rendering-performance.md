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

### P00 current inventory（登録済みhistorical baseline）

single Scene RtT / indoor light migration前のcurrent構成は次である。下記inventoryはP00 canonical
RenderDoc formal legと照合済みであり、現sourceのstartup testではなく登録済みartifactが正本である。

| 項目 | current |
|---|---:|
| Camera3d RtT | 2（Scene 1、Soul mask 1） |
| world color target | 2（Scene 1、Soul mask 1） |
| Camera2d | 3（Main、Overlay、WorldForeground） |
| FHD High target | 各1920×1080、2 handleはdistinct |
| DirectionalLight entity | 2（標準1 active、extra 1 default disabled） |
| composite sampled texture | 2（Scene + Soul mask） |

### P01 Scene-only runtime inventory

現sourceはP01移行によりScene-onlyである。`RttRuntime`、Camera3d、world color target、composite sampled
textureは各1となり、Soul mask camera / target / proxy / material / layer / toggleは存在しない。P00互換CSVの
mask列は削除せず、P01 stageでは意味上の0を記録する。`p01_scene_only_rtt_startup_inventory_is_explicit`とvisual_testの
resize testは、Scene targetの再生成後にCamera3dとcomposite materialが同じhandleへrebindされることを検証する。

| 項目 | P01 source |
|---|---:|
| Camera3d RtT | 1（Scene） |
| world color target | 1（Scene） |
| Camera2d | 3（Main、Overlay、WorldForeground） |
| DirectionalLight entity | 2（標準1 active、extra 1 default disabled） |
| composite sampled texture / sampler | 各1（Scene、binding 1 / 2） |
| Soul mask target / camera / proxy | 0 |

### P02 TopDown presentation runtime inventory

P02 は Scene-only RtT を維持したまま、MainCamera を composite 後の唯一の `LAYER_2D` camera にする。structural Building は3D、foreground Buildingは2Dのどちらか一方だけが active presentationとなる。Soulは共有pool billboard 1 / owner、Familiarは2D前景のみである。

| 項目 | P02 source |
|---|---:|
| Camera3d RtT / Scene target | 1 / 1 |
| Camera2d | 2（Overlay、Main） |
| active `LAYER_2D` pass | 1 |
| Soul billboard / Soul | 1 |
| Soul GLB / shadow proxy / Familiar 3D proxy | 0 / 0 / 0 |

P02 固有値は legacy `render_inventory.csv` を読み替えず `p02_presentation.csv` と RenderDoc checkpoint の `p02_presentation` blockへ出す。formal gateは duplicate=0、全Building exactly-one、billboard ratio=1、Familiar3D=0、state/bounce probe=trueを同一 medium/GPU checkpointで評価する。

画像側の補完はproduction indoor-light fixtureの専用actual-window matrixが所有する。18 caseすべてでgame process所有の
X11 clientを2 frame採取し、black frame / scene detail / animationをpixel判定する。semantic sidecarのDoor 3状態、
structural state / bounce、foreground分類、visible時のSoul billboard 1:1とhidden時のbillboard 0を同じcaseへ束縛するため、
desktop全体や独立`visual_test`の画像だけではP02受入にならない。

P00のmeasurement contractはfrozenの`rtt-light-v1`である。canonical contract hashは
`121a365ac3349cd4fa7890ab3069f0392098ced17e0d47f920095a1490c2ba11`、fixture hashは
`a688d564f8f50c2fdcdbe49dca7625b2cb05d01f8555378215fb8ba89b553eed`である。stage別projection義務と
gate expected row、resolved window backend / effective present modeの開始・終了検証、formal attempt
validatorは実装済みである。freeze後の変更は同じv1を編集せず新generationを追加する。

P00 canonical current baselineはsubject `10763a4da6bfbe0b480971fb85c474e6ff7a5f86`、attempt
`9e813f24-0f7b-47f5-8a8d-e3ff34775370`として登録済みである。Intel Arc / Vulkan / X11、1920×1080、DPI 1.0、
High、immediate presentを記録し、audit / behavior / Capture / RenderDoc / Memoryの全legをoffline verifierと
baseline registry verifierで再検証した。raw artifact 884件のdirectory SHA256は
`a9f4927186fe7c8c5f009583fd645cd7963b6ad36398e9d614fbcbbff1f8a6aa`である。

P01 canonical Scene-only candidateはsubject `29a4a719e9fe92b10618f36ce548c4bb5a4c7e80`、attempt
`8bc82f04-10ac-4903-89b6-89011dacdada`として登録済みである。同じcontract / fixtureとIntel Arc / Vulkan / X11環境で、
audit / behavior / Capture / RenderDoc / Memoryの全5 leg・18 caseを再検証した。gate ledgerは123 / 123 row pass
（`RLV1-P01-RTT` 9 row、`RLV1-P01-PERF` 20 row）で、raw artifact 884件のdirectory SHA256は
`68e470e51cf30f7659bb87eb1893235758d49f2c9d7a1e8f881f0e5f2a9f7502`である。

| case | frame p50 / p95 / p99 (ms) | max RSS (KiB) | allocator peak live (bytes) |
|---|---:|---:|---:|
| small / cpu | 14.701 / 21.441 / 24.271 | 1,338,744 | 669,949,156 |
| small / gpu | 28.567 / 36.939 / 40.385 | 1,519,344 | 687,635,837 |
| medium / cpu | 17.846 / 24.675 / 27.604 | 1,388,468 | 696,767,330 |
| medium / gpu | 30.272 / 38.812 / 42.531 | 1,475,668 | 732,618,413 |
| large / cpu | 23.772 / 30.750 / 34.008 | 1,425,912 | 739,108,778 |
| large / gpu | 33.679 / 42.488 / 46.635 | 1,527,076 | 801,151,582 |

frame値はCapture leg、RSS / allocator値はMemory legの正本であり、相互に代用しない。P01以降は同じcontract /
fixture / adapter matrixとstable projectionで比較する。

P00 RenderDoc runtime checkpoint schema v3とextraction schema v2は、historical current inventoryをGPU replayで厳密化する。composite drawはfragment
descriptor set 2のScene texture / sampler `(1, 2)`、Soul mask texture / sampler `(3, 4)`を同じ1 drawで
使うことを要求する。抽出はVulkan subpass transitionを正しく分割し、全drawに散らばったsampler数では代用しない。
canonical formal captureではVulkan 18 render pass、212 draw、516 attachment record、1,996 binding record、
composite draw 1、Scene target attachment / binding各1、Soul mask target attachment / binding各1を実測した。
compositeのScene texture / sampler `(1, 2)`、Soul mask texture / sampler `(3, 4)`も同一drawで一致する。raw RDCは
697,940,813 byte、SHA256は`5b33c53d0f81da746f92136edc1f1fe2143a97db89369a2ad70ba20654823f42`である。
RenderDoc binary SHA256は`5d0ac3accba20db0d9071ea036a770e1b236884335927121b64f4e873c9efb2f`、App APIは
requested 1.6.0 / returned 1.7.0、capture / replay processはすべてexit 0かつorphan 0だった。

P01 canonical RenderDoc captureは14 render pass、163 draw、369 attachment record、1,780 binding record、
composite draw 1を実測した。tracked world color resourceはScene targetだけで、compositeのfragment set 2は
Scene texture / sampler `(1, 2)`を各1回使用し、Soul mask target / attachment / binding / sampleは0である。
raw RDCは695,577,267 byte、SHA256は
`de501ede816213662e86eca2c63983667a0b87dd7ad5700e3e86b80b5337cfbf`である。

RenderDoc leg は通常の `profiling` output を使わず、`profiling-renderdoc` feature と同名の専用 Cargo profile
で build した capsule を使う。専用 profile は `profiling` を継承しつつ debug assertions を有効にし、native build の
RAM peakを抑えるため LTO を無効化して codegen unit を 16 に固定する。これは `wgpu-hal 29.0.4` が debug assertions
無効時に RenderDoc bridge を無効化するためであり、Capture / Memory
の通常性能 profileへこの条件を波及させない。

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
| billboard mesh | frame variantごとに最大1 DC | 全Soulが同一 Rectangle meshを共有 |
| billboard material | 最大8 variant | frame切替は共有handle差替えで、entityごとのmaterial生成なし |
| GLB / shadow proxy | 0 | production spawnとsystem登録を停止 |

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
