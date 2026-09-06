# アートスタイル受入基準

作成日: 2026-03-21
ステータス: 確定済み項目あり / PoC待ち項目あり

関連: `docs/world_lore.md` §6.2〜6.3 / `docs/plans/3d-rtt/asset-milestones-2026-03-17.md` MS-Asset-0 /
[`production-wall-art-plan-2026-08-31.md`](plans/3d-rtt/archived/production-wall-art-plan-2026-08-31.md)

---

## 1. 全体方針

本作のアートスタイルは **「手描き感の強いラフなベクターイラスト（Rough Vector Sketch）」** で統一する。
3Dモデルを使用する場合も、assetごとにnative受入で確定したlightingとtexture-baked lineworkを使い、**「体積のある存在に見えない」** 2Dイラスト的外見を担保する。

> 判断基準: 「これは3Dモデルである」と感じられるかどうかではなく、「平面的なイラストとして成立しているか」で合否を判断する。

---

## 2. Camera3d パラメータ（確定済み）

| 項目 | 値 | 備考 |
| --- | --- | --- |
| VIEW_HEIGHT | `150.0` | Camera3d の Y 座標 |
| Z_OFFSET | `90.0` | Camera3d の Z オフセット |
| **仰角** | **≈ 59°**（水平から） | `arctan(150 / 90)` |
| 投影方式 | 正射影（Orthographic） | 透視投影は禁止 |

GLB モデルの参照画像・入力画像はこの角度（水平から59°）で撮影・生成する。

---

## 3. キャラクター（Soul・Familiar）基準

### 3.1 シルエット・形状（確定済み）

| 要素 | 基準 |
| --- | --- |
| **全体シルエット** | Tim Burton 的に少し歪んだ有機的シルエット。均整の取れた美しい形は避ける |
| **輪郭線** | 太く、インクが滲んだようなルーズさとゆらぎ（Wobbly）を持たせる |
| **塗り** | 均一ではなく筆跡・塗りムラ（Textured brush）を残す |
| **情報量** | 描き込みすぎない。シルエットが読めれば十分 |

### 3.2 Soul の外見仕様（確定済み）

| 要素 | 仕様 |
| --- | --- |
| **体色（通常）** | 白〜薄青（`#88ccff` 相当）。半透明 |
| **素材感** | 半透明の霊体。輪郭がぼんやりしている |
| **移動表現** | ふわふわ浮遊。物理的な歩行ではない |
| **感情による色変化** | 通常: 白〜薄青 / やる気: 黄緑 / 疲労: グレー / ストレス: 赤 / 恐怖: 紫 |

### 3.3 production billboard仕様（確定済み）

```
shared Rectangle mesh
+ 既存Soul sprite 8枚の有限shared StandardMaterial pool
+ AlphaMode::Mask + unlit
+ SoulAnimVisualStateによるframe選択
+ 左右方向はinstance transformのscale.xで表現
```

### 3.4 表示構成（確定済み）

| 要素 | 内容 | 備考 |
| --- | --- | --- |
| `ActorBillboard3d` | Soul本体 | ownerあたりexactly one、Scene RtT内でdepth共有 |
| `SoulBillboardFrame` | 表情・状態 | Normal / Exhausted / Happy / Sleep / Wine / Trump / Stress / StressBreakdown |
| `ActorBillboardOwnerCache` | owner lookup | despawn/reset用。legacy proxy mapは持たない |

### 3.5 旧GLB仕様の扱い

旧`CharacterMaterial`、face atlas、Soul GLB、shadow proxyはP08でruntimeと`visual_test`から削除した。過去の採用理由やPoC値はarchive plan/proposalだけに保持し、新規assetやruntime実装の前提にしない。

---

## 4. アウトライン基準（PoC待ち）

> **現状**: production billboardの輪郭は入力spriteのalpha境界を使う。新しい輪郭処理は別提案とnative比較を必要とする。

| 項目 | 候補 | 現状 |
| --- | --- | --- |
| **線幅** | 1px / 2px / 3px | ⏳ PoC目視後に確定 |
| **ゆらぎ量** | 弱（均一に近い） / 強（手描き感強） | ⏳ PoC目視後に確定 |
| **アウトライン色** | 純黒 `#000000` / 暗茶 `#1a0a00` | ⏳ PoC目視後に確定 |
| **ズームアウト無効化閾値** | Camera2d scale 値（TBD） | ⏳ PoC目視後に確定 |

**仮基準**（PoC前の制作指針として使用）:
- 線幅: **2px** を起点に評価する
- ゆらぎ: **中程度**（ゆらぎなし → ゆらぎあり の2パターンを PoC で比較）
- 色: **暗茶** `#1a0a00` を起点に評価する

---

## 5. 建築物・地形基準

### 5.1 建築物（確定済み）

| 要素 | 基準 |
| --- | --- |
| **視点ルール** | 正面（Front face）と上面（Top edge）が見える角度（Camera 59°準拠） |
| **壁デザイン** | 黒い石積み。錆びた鉄バンド・トゲのある補強パーツ |
| **裂け目** | 紫の光（`#8b008b`）が漏れる「怠惰のエネルギー」表現 |
| **金属** | 錆びた鉄（Rusty Metal）。均質な金属感は禁止 |

#### Wall production geometry contract（確定済み）

`TILE_SIZE = 32 wu`、壁高 `H = 32 wu` に対し、本番Wallの**公称構造厚は
`9.6 wu = 0.30 tile`** とする。これは見た目のmesh厚であり、論理占有は従来どおり
32×32 wuの1 cell全体である。

| 項目 | 基準 |
| --- | --- |
| 基準高 | `32 wu`。local Y=`[-16, 16]`、world Y=`[0, 32]` |
| 公称石積み厚 | 各armの局所横断方向で`9.6 wu`（中心線から`±4.8 wu`）。連続するvisible bodyも9.6 wu未満へ細らせない |
| 石の凹凸・鉄バンド・トゲ込み外形 | 各arm中心線から局所横断`±6.4 wu`、全幅最大`12.8 wu = 0.40 tile`。junctionで交差する別armの長さは厚さへ数えない |
| Wall同士の接続port | cell境界で公称厚`9.6 wu`の同一profileへ戻す |
| cell内余白 | 公称面から各側`11.2 wu`。装飾最大時も各側`9.6 wu`を残す |
| lighting | `TopDownStructuralMaterial`のlit経路。directional shadingと紫emissiveを保持し、比較用unlit materialは本番へ残さない |
| 輪郭 | texture-baked lineworkを採用。Wall専用screen-space outline rendererは追加しない |
| normal map | 不採用。候補GLB 6種にtangentがないためtechnical gateで終了し、production coreはalbedo＋emissiveの2 textureとする |

現行59°CameraとRtT縦補正後は、画面縦軸への寄与が
`-world_z + 0.6 × world_y`となる。高さ32 wuの壁は正面が19.2 px相当、公称上面が
9.6 px相当となり、straight E-W壁の全投影高は28.8 px（`0.90 tile`）。装飾最大でも
32 px（`1.00 tile`）を超えず、旧32 wu厚Cuboidの51.2 px（`1.60 tile`）から
箱状の占有感を除ける。この基準の本番Wallはasset set `wall-production-v1` generation 4として
canonicalへ昇格済みであり、通常起動のWallは6共有GLB（triangle `24 / 24 / 24 / 36 / 48 / 72`）と
shared albedo / emissiveで描画される。

また、High・標準zoomで片側2 pxのtexture-baked lineを基準にすると、現行最大zoom-out factor 5では
公称厚が1.92 px、両側線が合計0.8 px、内部の塗りが1.12 px残る。塗りを1 px以上残す
下限9.0 wuを、0.05 tile単位で上へ丸めた値が9.6 wuである。固定screen-space outline、
zoom上限、Camera角度、wall LODのいずれかを変更する場合は、この式から厚さを再評価する。
1 pxの数値gateはHighにだけ適用し、Medium / Lowの最大zoom-outは最終compositeで切れない
anti-aliased silhouetteを定性的に確認する。Lowの内部色を物理1 pxと主張しない。

#### Wall provisional formwork contract（候補実装済み・未release）

仮設Wall用のruntime wallset v2は、本設6 familyと同じ接続mask・quarter turn・中心anchorを使い、
別の木製型枠6 GLBを選ぶ。中心支柱、上下横桟、端半支柱、筋交いをgeometryで構成し、板間の空隙を
半透明ではなく実形状で示す。木材albedoは512×512・完全Opaque・normal / emissiveなしである。
family別triangle数は`60 / 60 / 108 / 108 / 156 / 204`、上限は240とする。

技術candidate generation 5は`art_preview` authority専用で、通常起動、正式candidate、promotionでは
受理されない。ゲーム所有windowでの目視判断とDoor側M2統合後の正式受入を終えるまで、generation 4の
通常release表示は変更しない。

#### Door production geometry contract（候補実装済み・未release）

Doorは高さ32 wu、左右jambの外端X=`±16`、前後Z=`±4.8`をWall portへ合わせた固定枠と、左右2枚の木製leafを持つ。上枠はY=`[14.4,16.0]`の1.6 wu、奥行7.2 wuとし、jambより細いシルエットを保つ。Closed / Open / Lockedは各1 node・1 mesh・1 primitiveの共有GLBで、triangle数は`156 / 156 / 168`、上限240。Openは枠を動かさず左右leafと付属金具だけを各蝶番から68°開き、左右を前後反対側へ振り分ける。EWでは扉面と中央開口、NSでは枠の両側へ分離した扉面を読める形にする。Lockedだけが中央を跨ぐ骨の閂を持つ。

共有albedoは512×512・完全Opaque、emissive / normalなし。黒ずんだ厚板、風化した骨、少量の錆鉄をRough Vector Sketchの太いbaked lineworkで描く。EW/NS previewは同じClosed原本から59°正射影で256×256 RGBAへ固定レンダーし、pixel anchor `(128,192)`を64×64 logical canvasへ対応させる。technical candidateは`art_preview` authorityだけで有効であり、ユーザーの実画面判断、正式candidate、Wall型枠とのjoint受入、releaseが終わるまで通常Doorはprocedural fallbackを使う。

### 5.2 地形テクスチャ LOD 基準（確定済み）

地形は 3D チャンクメッシュ + RtT 経路で描画される。マテリアルは `tile_rtt_px`（RtT 上での 1 タイル見かけサイズ）を基準に切り替わる。

#### LOD 切替閾値

| 遷移 | 閾値 |
| --- | --- |
| LOD1 → LOD1-lite | `tile_rtt_px < 22 px` |
| LOD1-lite → LOD1 | `tile_rtt_px > 25 px` |
| LOD1-lite → LOD2 | `tile_rtt_px < 14 px` |
| LOD2 → LOD1-lite | `tile_rtt_px > 16 px` |

#### テクスチャスロット一覧

| テクスチャ | 現行サイズ | LOD1 | LOD1-lite | LOD2 | 備考 |
| --- | --- | --- | --- | --- | --- |
| grass_albedo | 1024×1024 | 必須 | 必須 | 必須 | LOD2 は UV 量子化（8 wu ステップ）で使用 |
| dirt_albedo | 1024×1024 | 必須 | 必須 | 必須 | 同上 |
| sand_albedo | 1024×1024 | 必須 | 必須 | 必須 | 同上 |
| river_albedo | 1024×1024 | 必須 | 必須 | 必須 | 同上 |
| terrain_macro_noise | 256×256 | 必須 | 未使用 | 未使用 | RGB、明暗ムラ全体用 |
| grass_macro_overlay | 256×256 | 必須 | 未使用 | 未使用 | グレースケール |
| dirt_macro_overlay | 256×256 | 必須 | 未使用 | 未使用 | グレースケール |
| sand_macro_overlay | 256×256 | 必須 | 未使用 | 未使用 | グレースケール |
| terrain_blend_mask_soft | 256×256 | 必須 | 未使用 | 未使用 | グレースケール |
| river_flow_noise | 256×256 | 必須 | 未使用 | 未使用 | グレースケール、川面アニメ用 |
| river_normal_like | 256×256 | 必須 | 未使用 | 未使用 | RGB、水面反射的演出 |
| shoreline_detail | 256×256 | 必須 | Sand 経路のみ必須 | 未使用 | グレースケール、砂浜際の grain |
| terrain_feature_lut | 256×1 | 必須 | 必須 | 必須 | RGBA、地物カラーグレーディング LUT。ロード後は uniform fast-path を優先 |
| terrain_id_map | 生成 | 必須 | 必須 | 必須 | 起動時に CPU 生成 |
| terrain_feature_map | 生成 | 必須 | 必須 | 必須 | 起動時に CPU 生成 |
| boundary_mask | 生成 | 必須 | 必須 | 必須 | PostStartup で後設定 |
| boundary_proximity_mask | 生成 | 必須 | 必須 | 未使用 | PostStartup で後設定。境界外 early-out 用 |

#### テクスチャ受け入れ基準

**albedo 4 種（grass / dirt / sand / river）**
- LOD1: タイリング継ぎ目が近景（`tile_rtt_px ≥ 14 px`）で視認されないこと
- LOD1-lite: 中景（概ね `14 px ≤ tile_rtt_px < 25 px`）で色調ジャンプが出ないこと。特に Sand の shoreline tone が LOD1 と連続して見えること
- LOD2: 8 wu 量子化後に「ブロック感」が著しく不自然でないこと（タイルが小さいため許容範囲は広い）
- 色調: §6 パレット（地面 `#2d1810`・川 `#3d1515`系）と整合していること

**macro_noise（LOD1 のみ）**
- 地形全体に自然な明暗ムラを与える。周期が一様でないこと

**macro_overlay 3 種（LOD1 のみ）**
- albedo に乗算した際、草・土・砂それぞれの質感が強調されること

**terrain_blend_mask_soft（LOD1 のみ）**
- 境界ブレンド計算に使用。エッジが過度に硬くないこと（滑らかなグラデーション）

**river_flow_noise / river_normal_like（LOD1 のみ）**
- 川面に流れ・波状感が視認できること

**shoreline_detail（LOD1 / LOD1-lite の Sand 経路）**
- 砂浜の水際に粒状感が追加されること

**terrain_feature_lut（全 LOD 共通）**
- 256 エントリ × RGBA。各エントリの符号付き tint（0.5 = 無変化）が地物（岩場・汀・内陸砂）の色補正として自然に見えること

---

### 5.3 壁ノーマルマップ（PoC待ち）

> **判断タイミング**: MS-Asset-Build-A（壁 GLB PoC）の目視確認後に確定する。

| 選択肢 | トレードオフ |
| --- | --- |
| あり | 石積みの立体感・凹凸が強調される。ポスタライズと相性確認が必要 |
| なし | Unlit 一貫性。手描き感との整合性が高い |

**仮方針**: **なし** で実装開始し、PoC で比較検証する。

---

## 6. 色彩パレット（確定済み）

```
背景:   #1a0a0a（暗い赤黒）
地面:   #2d1810（茶色がかった暗色）
空:     #3d1515（暗赤色）

魂の光: #88ccff（青白い光）
紫オーラ: #8b008b（怠惰の象徴）
焚き火:  #ff6b35（唯一の暖色）
警告赤:  #ff3333（ストレス表示）
アウトライン（仮）: #1a0a00（暗茶）
```

---

## 7. 未確定項目の確定トリガー

| 未確定項目 | 確定トリガー |
| --- | --- |
| billboardアウトライン線幅・ゆらぎ・色 | 新しいbillboard outline提案とnative比較 |
| ズームアウト時の表示調整 | production camera / DPI / qualityのnative比較 |
| 壁ノーマルマップ あり/なし | MS-Asset-Build-A（壁 GLB PoC）目視後 |
| キャラクター向き管理方式 | 同上（左右ミラーのみ vs フル8方向） |
