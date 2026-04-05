# 地形ビジュアル再検討メモ（2026-04-05）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `terrain-visual-reassessment-2026-04-05` |
| ステータス | `Draft` |
| 対象マイルストーン | `MS-3-6` |
| 参照元 | [`ms-3-6-terrain-surface-plan-2026-03-31.md`](ms-3-6-terrain-surface-plan-2026-03-31.md) |
| 前提 | WFC 地形生成（`generate_world_layout`）が現行経路、地形描画が `Mesh3d + SectionMaterial + RtT` に移行済み |

## 問題

旧 `MS-3-6` 計画は「WFC 前」「地形タイプは 4 種だけ見えていればよい」という前提が強かった。現状はその前提が変わっている。

- 地形生成は `GeneratedWorldLayout` を通じて `terrain_tiles` だけでなく、**`layout.masks`（`WorldMasks`）** に `river_mask`、`final_sand_mask`、`inland_sand_mask`、`rock_field_mask`、`grass_zone_mask` / `dirt_zone_mask`、`grass_zone_distance_field` / `dirt_zone_distance_field` などを持っている
- 一方で描画は `TerrainType -> 4 種の SectionMaterial` しか使っておらず、WFC が作った地形の意味差をほぼ捨てている
- `Sand` は「河岸の砂」と「内陸砂」が同じ見た目に潰れている
- `Dirt` も「岩場由来の乾いた土」と「ゾーン境界で出る土」が同じ見た目に潰れている
- `SectionMaterial` は建物と共有のため、地形専用の情報をこれ以上載せると責務が濁る
- 旧案の「タイルごとの 90 度回転」は、現在の連続ワールド UV と相性が悪い。入れると同種タイル間の連続感を自分で壊しやすい
- 旧 2D 境界オーバーレイ素材はファイルとして残っているが、現行の 3D/RtT 経路では未使用。ここへ戻ると設計が逆行する

## 再検討後の方針

### 1. まず「境界を描く」より「地形の意味差を描く」

優先度を次の順に入れ替える。

1. WFC が既に持っている mask / distance 情報を描画へ渡す
2. その情報でマクロな色差・湿り気・荒れ方を出す
3. それでも不足する場合だけ、異タイプ境界のソフトブレンドを追加する

先に境界ブレンドへ進むより、`shore sand` と `inland sand`、`rock field dirt` と通常 dirt を見分けられる方が、現状の実装価値をそのまま画面に返せる。

### 1.1 UV-only フェーズの位置づけ

アセットを増やさない短期フェーズでも、次の 3 点には効果がある。

- **同一タイプ内の反復感軽減**
- **ゾーン単位の塊感の追加**
- **同じ `TerrainType` 内の意味差の可視化**

逆に、次の 2 点は UV-only では限界がある。

- **材質そのものの描き分け**  
  同じ `sand_terrain.png` を使う限り、河岸砂と内陸砂は「別の砂」にはならない
- **異タイプ境界の自然な混色**  
  Grass と Dirt の境界を「別テクスチャ同士が馴染む」見た目にするには、後段の境界ブレンドか追加アセットが要る

したがって、短期の狙いは「境界を消す」ではなく、**同じ 4 枚でも画面上の情報量を増やす**ことに置く。

### 2. metadata texture を正規ルートにする

`GeneratedWorldLayoutResource.layout`（`bevy_app` の `Res`）から、**`terrain_tiles` と `masks` を読み**、render 用の lookup texture を起動時に生成し、地形シェーダで参照する。`hw_world` には Bevy 画像型を置かず、**純粋データの読み出しは `bevy_app` / `hw_visual` 側**に留める（クレート境界は `docs/crate-boundaries.md` と `hw_world/AGENTS.md` に準拠）。

候補:

- `terrain_id_map`
  - セルごとの `TerrainType`
- `terrain_feature_map`
  - `shore sand` / `inland sand` / `rock field` / `grass zone` / `dirt zone` の識別や重み
- `terrain_distance_map`
  - `grass_zone_distance_field` / `dirt_zone_distance_field` を 0..1 に正規化したもの

帯域・サンプル数が気になったら上記を RGBA パック等で 1〜2 枚にまとめる選択肢があるが、まずは分離実装で可読性を優先する。

これにより、地形の見た目を「4 テクスチャ固定」から「4 テクスチャ + WFC 意味情報」へ広げられる。

### 2.1 metadata texture の最小パック案

初手は texture 枚数を増やしすぎない。

| Texture | チャンネル | 用途 |
| --- | --- | --- |
| `terrain_id_map` | `R` | `TerrainType` の index |
| `terrain_feature_map` | `R` / `G` / `B` / `A` | `shore sand` / `inland sand` / `rock field` / zone bias |
| `terrain_distance_map` | `R` / `G` | `grass_zone_distance` / `dirt_zone_distance` |

初期実装では 3 枚でよい。パック最適化は、見た目が成立してからで十分。

### 2.2 metadata の読み方

シェーダでは `world_position.xz` を `MAP_WIDTH` / `MAP_HEIGHT` 基準で 0..1 UV に変換して lookup する。地形メッシュは全セル同サイズ・同配置なので、地形タイル entity ごとの追加属性を持たせる必要はない。

実装上の注意:

- メタ情報は **nearest サンプリング**でセル境界を保つ
- アルベド用 terrain texture は従来どおり **repeat サンプリング**
- `terrain_id_map` は `TerrainChangedEvent` で **部分更新可能**な形にしておく
- `feature_map` / `distance_map` は起動時 bake のままでもよい

### 3. 近距離の改善は world-space ベースで行う

継続推奨:

- 連続ワールド UV
- 川スクロール
- 草の低周波歪みと弱い明度変調

非推奨:

- **タイル（セル）／メッシュ単位の 90° 回転**（フラグメントの stochastic UV とは別物。直下の段落参照）
  - 今の world-space UV の利点を崩しやすい
  - 「同種タイルの面として続いて見える」ことより「タイルごとの差」を優先してしまう

**旧 MS-3-6 の「A: Stochastic UV 回転」との関係**: 親計画ではフラグメント側の **UV 空間での stochastic（同一タイプ内の繰り返し緩和）** を想定していた。本メモで非推奨にしているのは **タイル（セル）単位・メッシュ／エンティティ単位の 90° 回転**であり、前者と後者は混同しないこと。

必要なら later phase では「タイル単位回転」ではなく、world-space の domain warp / macro noise を強化する。

### 3.1 推奨する UV 処理案

実装優先度順に並べる。

| 優先 | 処理 | 目的 | 備考 |
| --- | --- | --- | --- |
| 1 | **低周波 domain warp** | 同一 texture の反復感を崩す | 全地形で有効。既存 grass distortion の一般化 |
| 2 | **macro brightness / tint variation** | 面としてのムラを出す | Grass / Dirt / Sand に効く。高周波は禁止 |
| 3 | **feature tint** | `shore` / `inland` / `rock field` の意味差を出す | metadata texture 前提 |
| 4 | **wetness / roughness variation** | 川沿い・河岸を湿って見せる | 見た目は subtle に留める |
| 5 | **flow distortion** | 川だけ流れ方向の揺れを足す | 今の scroll の上に追加する |

### 3.2 具体的な shader 処理

#### A. domain warp

`world_xz` から低周波ノイズを作り、`compute_terrain_uv` の入力座標をずらす。

```wgsl
let macro = sample_macro_noise(world_xz * 0.015);
let warped_world_xz = world_xz + (macro.xy - 0.5) * 10.0;
let uv = warped_world_xz * section_material.uv_scale;
```

狙い:

- タイル境界を壊さずに、同じ草地の中で模様の流れを変える
- 「同じ 1024px テクスチャが平面上に周期的に並ぶ」印象を弱める

推奨:

- まずは grass / dirt / sand に適用
- river は別処理にして、domain warp は弱める

#### B. macro brightness / tint variation

アルベド sampled color に対し、低周波の係数を掛ける。

```wgsl
let shade = 0.92 + sample_macro_noise(world_xz * 0.01).z * 0.16;
base_color.rgb *= shade;
```

推奨レンジ:

- Grass: `±6%`
- Dirt: `±5%`
- Sand: `±4%`
- River: 基本なし。やるなら明度より色相寄り

#### C. feature tint

`terrain_feature_map` の値に応じて、同一テクスチャに軽い tint を加える。

例:

- `shore sand`: 少し湿った暗めの灰青寄り
- `inland sand`: 少し乾いた黄灰寄り
- `rock field dirt`: 赤みを落として灰褐色寄り
- `grass zone`: 緑寄りだが彩度は低く
- `dirt zone`: 茶寄りだが明度は上げすぎない

重要:

- tint は `10%` 未満の混色に留める
- texture の筆致を塗り潰さない

#### D. wetness / roughness variation

`shore sand` や `river` 近傍では roughness と暗さを少し変える。

例:

- `shore sand`: base color を少し暗く、roughness を少し下げる
- `rock field dirt`: roughness を上げて乾いた粉っぽさを出す

`StandardMaterial` ベースの都合上、per-pixel でどこまで反映するかは material/shader 実装次第だが、少なくとも base color 側だけでも効く。

#### E. river flow distortion

今の `uv_scroll_speed` に加え、U 方向だけでなく弱い法線外乱風のオフセットを入れる。

```wgsl
let flow = sample_macro_noise(vec2(world_xz.x * 0.02 - globals.time * 0.2, world_xz.y * 0.015));
uv += vec2(-section_material.uv_scroll_speed * globals.time, 0.0);
uv.y += (flow.x - 0.5) * 0.03;
```

狙い:

- 単なるテクスチャ横流し感を減らす
- 黒い濁流の「うねり」を少しだけ足す

### 3.3 UV-only フェーズで入れないもの

- セル単位 90° 回転
- 高周波 grain / dither
- 4 地形同時サンプルによる本格ブレンド
- 法線マップ前提の派手な specular 表現

これらは `Rough Vector Sketch` と相性が悪いか、現行 material の責務を超える。

### 4. 境界ブレンドが必要なら `SectionMaterial` 拡張ではなく地形専用 material を切る

現行 `SectionMaterial` は建物の cut / progress 表現を主目的に持っている。ここへさらに次を足すと重い。

- 4 種地形アルベドの同時参照
- metadata texture 群
- 近傍サンプルによる境界ブレンド

そのため、異タイプ境界のソフトブレンドまで進む段階では `TerrainSurfaceMaterial` の新設を第一候補とする。

理由:

- 建物の material と地形の material を分離できる
- 4 地形テクスチャを常時 bind できる
- 将来 `shore foam` や `wetness` を足しても建物側へ漏れない

### 4.1 地形専用 material を切る判断基準

次のいずれかを満たしたら、`SectionMaterial` 延命ではなく `TerrainSurfaceMaterial` を優先する。

- 地形 4 種を同時 bind したい
- metadata texture を 2 枚以上引く
- border blend のために近傍サンプルを増やす
- `SectionCut` とは無関係な地形専用 uniform が増え続ける

### 5. 旧 terrain overlay asset は「戻す前提」で持たない

`grass_edge.png` などの旧オーバーレイ素材は、再利用する明確な計画がない限り「互換資産」と見なす。地形改善の主ルートにはしない。

復帰させない理由:

- 現在の描画は 3D 地形面 + RtT 合成が正規経路
- 境界オーバーレイを戻すと、境界判定・向き判定・前後関係の責務が再び CPU 側へ戻る
- `TerrainChangedEvent` と 2 系統管理になりやすい

## 期待できる効果

### 見た目

- WFC で作られた「川沿い」「内陸砂」「東側岩場」の意味が画面上で読める
- Dirt / Sand / Grass の塊感が出て、単なる 4 色塗り分けに見えにくくなる
- 異タイプ境界をまだ混ぜなくても、単調さがかなり減る

### UV-only で期待する見た目の着地点

- Grass の広い面が「同じハンコ」に見えない
- 河岸砂と内陸砂が、同じテクスチャでも「別の場所の砂」と読める
- 東側岩場の Dirt が、通常 Dirt より荒れた帯として読める
- 川が「スクロールしているテクスチャ」ではなく、少しうねる黒水に見える

### 実装コスト

- metadata texture の導入までは既存 `spawn_map` / startup 経路の延長で進められる
- **`TerrainChangedEvent` とメタテクスチャ**: ゲーム中に変わるのは主に **`WorldMap` 上の `terrain_tiles`**。`WorldMasks` や距離場は **生成時スナップショット**のまま想定し、見た目は「**起動時に焼いた feature / distance** + **実行時に更新する terrain_id（タイプ）**」の合成で足りる。岩撤去などで「タイプだけ変わり、元 `rock_field_mask` の見た目が残る」かどうかは、シェーダで **現在の `TerrainType` を優先**するかどうかで制御する（必要なら `terrain_id_map` の該当セルだけをイベントで更新）
- 本格的な境界ブレンドだけを独立トラックに分離できる

### パフォーマンス

- 起動時の texture 生成は `100x100` マップなら軽い
- 常時コストは shader の追加サンプル分のみ
- per-tile material 化や overlay entity 復活よりずっと安い

## 追加アセット方針

UV-only で十分改善できるが、最終品質の上限はアルベド側で決まる。下記の優先順に接続・追加していく。

### 既存アセット接続状況

追加候補として挙げていた次のファイルは、`assets/textures/` 直下に**生成済み**で存在を確認した。

- `terrain_macro_noise.png`
- `river_flow_noise.png`
- `terrain_feature_lut.png`
- `grass_macro_overlay.png`
- `dirt_macro_overlay.png`
- `sand_macro_overlay.png`
- `terrain_blend_mask_soft.png`
- `shoreline_detail.png`

確認時点の状態:

- **コード未接続**: `asset_catalog.rs` / shader / material からはまだ参照されていない
- **Git 未追跡**: `.gitignore` の `/assets/**` により `git status` には出ない
- **形式**:
  - `terrain_macro_noise.png` / `river_flow_noise.png` / 各 `*_macro_overlay.png` / `terrain_blend_mask_soft.png` / `shoreline_detail.png` は `256x256`
  - `terrain_feature_lut.png` は `256x1`

repeat 前提の簡易評価:

| Asset | 現状評価 | 備考 |
| --- | --- | --- |
| `terrain_macro_noise.png` | そのまま使いやすい | 最初の接続候補 |
| `terrain_feature_lut.png` | 問題なし | LUT 用。repeat 前提ではない |
| `grass_macro_overlay.png` | そのまま使いやすい | 草の macro variation に向く |
| `dirt_macro_overlay.png` | そのまま使いやすい | Dirt の面変化に向く |
| `terrain_blend_mask_soft.png` | そのまま使いやすい | 境界ブレンドの falloff 用 |
| `river_flow_noise.png` | 要確認 | 片軸方向の継ぎ目が出る可能性あり |
| `sand_macro_overlay.png` | 要確認 | repeat 使用前に継ぎ目再確認 |
| `shoreline_detail.png` | 要確認 | repeat より clamp / 局所 detail 向きの可能性 |

短期実装での優先接続順:

1. `terrain_macro_noise.png`
2. `terrain_feature_lut.png`
3. `grass_macro_overlay.png` / `dirt_macro_overlay.png`
4. `river_flow_noise.png` は継ぎ目方針を決めてから
5. `shoreline_detail.png` / `terrain_blend_mask_soft.png` は border blend 導入時

### 優先度 A: まず効くアセット

| Asset | 用途 | 効果 |
| --- | --- | --- |
| `terrain_macro_noise.png` | domain warp / brightness variation の共通ノイズ | WGSL の疑似ノイズより制御しやすい |
| `river_flow_noise.png` | 川専用の流れ外乱 | 横スクロール感を減らす |
| `terrain_feature_lut.png` | feature ごとの tint / roughness 参照 | shader の定数分岐を減らせる |

### 優先度 B: 見た目を一段上げるアセット

| Asset | 用途 | 効果 |
| --- | --- | --- |
| `grass_macro_overlay.png` | 草地の大きな塗りムラ | 広い草地の情報量を増やす |
| `dirt_macro_overlay.png` | 土地帯・岩場の荒れ表現 | Dirt の面の単調さを減らす |
| `sand_macro_overlay.png` | 河岸 / 内陸砂の乾湿差 | Sand の意味差を強める |
| `river_frame2.png` または `river_normal_like.png` | 川の流れの変化 | 流水表現を自然にする |

### 優先度 C: border blend 用

| Asset | 用途 | 効果 |
| --- | --- | --- |
| `terrain_blend_mask_soft.png` | 境界の falloff 制御 | Grass/Dirt/Sand 境界の馴染み改善 |
| `shoreline_detail.png` | 河岸専用の細部 | shore sand の説得力を上げる |

追加しなくてよいもの:

- 旧 `grass_edge.png` 系オーバーレイの再制作
- タイル 1 種ごとの大量バリエーション png

### アセット制作ガイド

- `world_lore.md` の Rough Vector Sketch を維持する
- 低周波・中コントラストを優先し、高周波ノイズは避ける
- 4 地形で明度レンジを揃える
- 「綺麗な自然」ではなく、荒廃した地獄の地面として寄せる

## 判断メモ

- 旧案の「B: 隣接ブレンド」は今も有効だが、優先順位は一段下げる
- 今の実装で最も捨てられている情報は「隣接関係」ではなく「WFC が作った地形の意味差」
- したがって、再検討後の主軸は「overlay 復帰」でも「タイル回転」でもなく、「worldgen metadata を render path に通すこと」

## 実装ステップ

### Phase 1: 現状固定と観測

1. `HELL_WORKERS_WORLDGEN_SEED` を固定して S0 の比較スクリーンショットを撮る
2. 現行の不満を「同種反復」「意味差不足」「境界の硬さ」に分けて確認する
3. `SectionCut` で地形面が破綻していないかを先に確認する

### Phase 2: metadata 導入

1. `GeneratedWorldLayoutResource.layout`（`terrain_tiles` + `masks`）から render 用 metadata texture を組み立てる resource を追加
2. 地形 shader から world 座標でその texture を引けるようにする
3. まずは cross-type blend を入れず、`domain warp` + `macro brightness/tint` + `feature tint` だけを入れる
4. `shore sand` と `inland sand` を見た目で分ける
5. `rock_field dirt` と通常 dirt を見た目で分ける
6. river に弱い flow distortion を追加する

### Phase 2.5 (optional): アセット接続強化

1. `terrain_macro_noise.png` を追加し、WGSL 疑似ノイズと置き換えるか比較する
2. 必要なら `river_flow_noise.png` を追加し、川だけ別の流れ外乱にする
3. 効果が大きいものだけ残し、texture 数を惰性で増やさない

### Phase 3: 境界ブレンド判定

1. Phase 2 後の画面でまだ境界が硬いかを再評価する
2. 必要なら `TerrainSurfaceMaterial` を新設し、4 地形テクスチャ + `terrain_id_map` を同時参照する
3. ブレンドは cardinal 近傍中心で始め、corner の複雑化は後回しにする

### Phase 4: 後片付け

1. 旧 overlay asset を使わない方針が固まったら load を削る
2. 恒久仕様を `docs/world_layout.md` と `docs/architecture.md` に反映する

## 変更候補ファイル

- `hw_world`: **原則変更しない**（地形データ型・`WorldMasks` の意味は既にある。画像生成は `bevy_app` 等でレイアウトを読む）
- `crates/bevy_app/src/world/map/spawn.rs`
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`
- `crates/bevy_app/src/plugins/startup/asset_catalog.rs`
- `crates/bevy_app/src/plugins/startup/rtt_setup.rs`（texture 初期化 helper を使い回す場合）
- `crates/bevy_app/src/systems/visual/terrain_material.rs`
- `crates/hw_visual/src/material/section_material.rs`
- `assets/shaders/section_material.wgsl`
- `crates/hw_visual/src/material/terrain_surface_material.rs`（新設する場合）
- `docs/world_layout.md`
- `docs/architecture.md`

## 検証方法

- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace`
- 同一 seed で before / after のスクリーンショット比較
- UV-only phase は `HELL_WORKERS_WORLDGEN_SEED` を固定し、少なくとも grass 広域 / shore sand / inland sand / rock field / river の 5 画角を比較する
- 川沿い、内陸砂、岩場、岩撤去後 Dirt の 4 ケースを目視確認
- `TerrainChangedEvent` 発火後に、`terrain_tiles` 由来の見た目（少なくとも `terrain_id_map` 相当）が 1 フレームで追従することを確認
