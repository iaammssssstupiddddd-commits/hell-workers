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

- 地形生成は `GeneratedWorldLayout` を通じて `terrain_tiles` だけでなく `river_mask`、`final_sand_mask`、`inland_sand_mask`、`rock_field_mask`、`grass_zone_mask` / `dirt_zone_mask`、距離場まで持っている
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

### 2. metadata texture を正規ルートにする

`GeneratedWorldLayout` から render 用の lookup texture を起動時に生成し、地形シェーダで参照する。

候補:

- `terrain_id_map`
  - セルごとの `TerrainType`
- `terrain_feature_map`
  - `shore sand` / `inland sand` / `rock field` / `grass zone` / `dirt zone` の識別や重み
- `terrain_distance_map`
  - `grass_zone_distance_field` / `dirt_zone_distance_field` を 0..1 に正規化したもの

これにより、地形の見た目を「4 テクスチャ固定」から「4 テクスチャ + WFC 意味情報」へ広げられる。

### 3. 近距離の改善は world-space ベースで行う

継続推奨:

- 連続ワールド UV
- 川スクロール
- 草の低周波歪みと弱い明度変調

非推奨:

- タイル単位の 90 度 stochastic 回転
  - 今の world-space UV の利点を崩しやすい
  - 「同種タイルの面として続いて見える」ことより「タイルごとの差」を優先してしまう

必要なら later phase では「タイル単位回転」ではなく、world-space の domain warp / macro noise を強化する。

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

### 実装コスト

- metadata texture の導入までは既存 `spawn_map` / startup 経路の延長で進められる
- `TerrainChangedEvent` はそのまま dirty update 経路に使える
- 本格的な境界ブレンドだけを独立トラックに分離できる

### パフォーマンス

- 起動時の texture 生成は `100x100` マップなら軽い
- 常時コストは shader の追加サンプル分のみ
- per-tile material 化や overlay entity 復活よりずっと安い

## 実装ステップ

### Phase 1: 現状固定と観測

1. `HELL_WORKERS_WORLDGEN_SEED` を固定して S0 の比較スクリーンショットを撮る
2. 現行の不満を「同種反復」「意味差不足」「境界の硬さ」に分けて確認する
3. `SectionCut` で地形面が破綻していないかを先に確認する

### Phase 2: metadata 導入

1. `GeneratedWorldLayout` から render 用 metadata texture を組み立てる resource を追加
2. 地形 shader から world 座標でその texture を引けるようにする
3. まずは cross-type blend を入れず、tint / wetness / roughness 的な差だけ出す
4. `shore sand` と `inland sand` を見た目で分ける
5. `rock_field dirt` と通常 dirt を見た目で分ける

### Phase 3: 境界ブレンド判定

1. Phase 2 後の画面でまだ境界が硬いかを再評価する
2. 必要なら `TerrainSurfaceMaterial` を新設し、4 地形テクスチャ + `terrain_id_map` を同時参照する
3. ブレンドは cardinal 近傍中心で始め、corner の複雑化は後回しにする

### Phase 4: 後片付け

1. 旧 overlay asset を使わない方針が固まったら load を削る
2. 恒久仕様を `docs/world_layout.md` と `docs/architecture.md` に反映する

## 変更候補ファイル

- `crates/bevy_app/src/world/map/spawn.rs`
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`
- `crates/bevy_app/src/plugins/startup/asset_catalog.rs`
- `crates/bevy_app/src/systems/visual/terrain_material.rs`
- `crates/hw_visual/src/material/section_material.rs`
- `assets/shaders/section_material.wgsl`
- `crates/hw_visual/src/material/terrain_surface_material.rs`（新設する場合）
- `docs/world_layout.md`
- `docs/architecture.md`

## 検証方法

- `cargo check --workspace`
- `cargo clippy --workspace`
- 同一 seed で before / after のスクリーンショット比較
- 川沿い、内陸砂、岩場、岩撤去後 Dirt の 4 ケースを目視確認
- `TerrainChangedEvent` 発火後に見た目更新が 1 フレームで追従することを確認

## 判断メモ

- 旧案の「B: 隣接ブレンド」は今も有効だが、優先順位は一段下げる
- 今の実装で最も捨てられている情報は「隣接関係」ではなく「WFC が作った地形の意味差」
- したがって、再検討後の主軸は「overlay 復帰」でも「タイル回転」でもなく、「worldgen metadata を render path に通すこと」
