# MS-3-6 テレイン表面表現改善（旧 MS-3B）実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `ms-3-6-terrain-surface-plan-2026-03-31` |
| ステータス | `A/D/B 実装済み（S0 受入撮影・最終目視判定待ち）` |
| 作成日 | `2026-03-31` |
| 最終更新日 | `2026-04-05` |
| 親マイルストーン | `docs/plans/3d-rtt/milestone-roadmap.md` **MS-3-6** |
| 前提 MS | **MS-3-4**（地形＝`Mesh3d` + `Terrain3dHandles`、タイル 1 セル 1 エンティティ） / **MS-WFC-4**（`GeneratedWorldLayout` が startup 経路へ統合済み） |
| 関連提案 | [`section-material-proposal-2026-03-16.md`](../../proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md) |
| アセット計画 | [`asset-milestones-2026-03-17.md`](asset-milestones-2026-03-17.md) **MS-Asset-Terrain** |
| アート指針 | [`docs/world_lore.md`](../../world_lore.md) §6.1 パレット・**§6.2 Rough Vector Sketch**（手描き感・塗りムラ・描き込みすぎない） |
| 独立トラック | [WFC 地形生成](archived/wfc-terrain-generation-plan-2026-04-01.md) はデータ生成が主眼。本 MS はレンダリング／表現 |
| 後続 MS | **MS-3-7**（Raycast）と直列必須ではない |
| 実装サブ計画（現行アセット限定） | [`ms-3-6-ad-implementation-plan-2026-04-01.md`](ms-3-6-ad-implementation-plan-2026-04-01.md) — **A/D のコード・WGSL 手順**（新規 PNG 差し替えなし） |
| Phase 3（隣接ブレンド・`TerrainSurfaceMaterial`） | [`blueprint-terrain-surface-material.md`](blueprint-terrain-surface-material.md) — **B** 相当の詳細ブループリント。2026-04-05 実装反映済み |

---

## 1. 目的

### 解決したい課題

- MS-3-4 後、各タイルは **単一 `TerrainType` → 4 種のいずれかの `SectionMaterial`**。**隣接セルでタイプが変わると色が段差になる**ほか、**同一タイプ内でもタイリングの繰り返し**が目立つことがある。
- 旧 **90° 境界オーバーレイ**は廃止済み。**スプライト重ねに戻さず**、RtT 上で見た目を整える。

### ロードマップ完了条件

- **90° ベースの地形境界オーバーレイに依存しない**見た目が成立する（目視・MS-Asset-0 と合意した受入基準）。

### 成功指標

- [ ] 上記完了条件を満たす
- [ ] 矢視で **地形の `SectionCut`** が破綻しない（退行なし）
- [ ] `cargo check --workspace` ゼロ、`cargo clippy --workspace` 警告ゼロ

---

## 2. 推奨方針の全体像（段階採用）

**結論**: **D（アセット）→ A（metadata / macro variation）→ B（`TerrainSurfaceMaterial` + `terrain_id_map`）** の順で実装した。現在の open item は **S0 受入撮影と最終目視判定**、必要ならその後のアセット微調整だけである。**C（頂点ベイク）は本 MS のスコープ外のまま**（共有メッシュ前提と相性が悪く、地形変更時の更新コストも重い）。

**マップジェン側の境界ノイズ化は WFC に吸収される**。WFC は隣接制約を満たしながら有機的なジグザグ境界を生成するため、別途ノイズを重ねる必要はない（WFC 後にノイズで地形タイプを書き換えると禁止ペア制約が壊れる危険がある）。

| 段階 | 内容 | 役割 | WFC 依存 |
| --- | --- | --- | --- |
| **第 1** | **D** MS-Asset-Terrain（シームレス・トーン・ミップ・川は任意で 2 フレーム等） | **見た目の土台**（Rough Vector に合う塗りはここが主） | なし（先行可） |
| **第 2** | **A** ワールド空間 UV ＋ Stochastic UV 回転 ＋ 低周波 UV 歪み ± 明度変調 | **タイル境界シームの消去・繰り返し感の緩和** | なし（先行可） |
| **第 0** | **S0** 受入基準の固定（スクリーンショット） | 判断のブレ止め | **WFC 後に撮影** |
| **第 3（任意）** | **B** 隣接タイプの GPU 供給。**推奨サブ案**は **100×100 の ID ルックアップテクスチャ 1 枚**＋フラグメントで近傍サンプル | **異タイプ境界のソフト化**が必要なとき | **WFC 後（Dirt 領域が生まれてから）** |
| **保留** | **C** 頂点ベイク | チャンク化・メッシュ戦略の見直しと別途 | — |

**A と D の役割分担**

- **D**: タイリング・シーム・**4 種間の明度・彩度のレンジ整合**。**筆ムラ・ラフな塗り**はアルベドに載せる（`world_lore` §6.2）。
- **A**: **metadata と macro variation で意味差を返す**。最終的な cross-type の連続は **B** に任せる。

---

## 3. 方針 D — アセット（MS-Asset-Terrain）

### 3.0 現状ファイルとコード参照

| ファイル | サイズ | `asset_catalog.rs` での参照 | 状態 |
| --- | --- | --- | --- |
| `textures/grass.png` | 1024×1024 RGBA | `game_assets.grass` → `TerrainSurfaceMaterialExt.grass_albedo` | 要シームレス確認 |
| `textures/dirt.png` | 1024×1024 RGBA | `game_assets.dirt` → `TerrainSurfaceMaterialExt.dirt_albedo` | 要シームレス確認 |
| `textures/sand_terrain.png` | 1024×1024 RGBA | `game_assets.sand` → `TerrainSurfaceMaterialExt.sand_albedo` | 要シームレス確認 |
| `textures/river.png` | 1024×1024 RGBA | `game_assets.river` → `TerrainSurfaceMaterialExt.river_albedo` | 静止1枚・要シームレス確認 |
| `textures/sand.png` | 1024×1024 RGBA | **参照なし** | 旧ファイル（不使用） |

### 3.1 技術要件（4 種共通）

- **シームレス**（四辺がつながる）。ワールド空間 UV（§4.0）で境界を越えて連続するため必須。
- **解像度**: 1024×1024 のまま。`TILE_SIZE=32` でワールド UV スケール N=2 にすると 2 タイル分をカバー。
- **ミップフレンドリー**: 高周波の細かいパターンを避ける（縮小時のモアレ対策）。
- **ワークフロー**: マゼンタ背景 → `scripts/convert_to_png.py`（`docs/DEVELOPMENT.md`）。

### 3.2 アート要件（Rough Vector Sketch）

- **均一な単色塗りは避け**、**中〜低コントラストの塗りムラ・筆跡**をアルベドに含める（§6.2）。
- **4 種間で明度・彩度のレンジを近づける**と、隣接境界の色段差が目立ちにくい（完全な色連続は B の領域）。
- ベースパレットは `world_lore §6.1`（地面ベース `#2d1810`）。明るい単色は地獄の世界観と合わない。

### 3.3 種別ごとの色方針

| 種別 | 色味 | 備考 |
| --- | --- | --- |
| **Grass** | 暗い灰緑〜茶緑 | 生命感より荒廃感。ベース `#2d1810` より若干緑寄り |
| **Dirt** | 暗い赤茶〜灰褐色 | Grass と Sand の中間トーン。採掘跡・踏み固めた地面 |
| **Sand** | 暗い灰砂 | 忘却の川岸（`world_lore §2.1`）。明るいビーチ砂ではなく灰がかった砂 |
| **River** | 黒く濁った水 | `world_lore §2.1`「黒く濁った液体」。暗い青黒 |

### 3.4 川アニメーション

2 つの方式があり、**A（UV スクロール）を推奨**する：

| 方式 | 新規アセット | コード変更 |
| --- | --- | --- |
| **A: UV スクロール（推奨）** | 不要 | シェーダーに `globals.time` ベースのスクロール追加のみ |
| **B: フレーム切替** | `river_frame2.png` を新規作成 | `Terrain3dHandles` 拡張 + フレーム切替システム追加 |

Bevy 0.18 では `bevy_render::globals` から `globals.time` をシェーダーで参照できる。A は新規テクスチャなしで実装できるため先行して試す。

### 3.5 D だけでは限界があること

- **隣タイルが別 `TerrainType`** のときの **物理的な色のギャップ**は、テクスチャだけでは根本解決しにくい。現行は **B** まで実装済みだが、最終品質は引き続きアルベド側の出来に依存する。

---

## 4. 方針 A — シェーダ（初期段階の方針メモ）

本節は **A 段階の設計メモ**として残す。最終実装では地形専用の [`TerrainSurfaceMaterial`](blueprint-terrain-surface-material.md) へ移行し、tile 単位 90° 回転は採用していない。現行は **連続 world-space UV + macro noise / overlay + feature map / LUT** が基準線である。

### 4.0 前提：ワールド空間 UV への切り替え（最優先）

現状の `section_material.wgsl` はメッシュ UV（`in.uv`）をそのまま使用している。`Plane3d` を全タイルで共有しているため、UV は各タイルごとに **0→1 へリセット**され、テクスチャがタイル境界で必ず途切れる。

**修正**: テクスチャサンプリング UV をワールド座標から計算する。

```wgsl
// 変更前（タイルごとにリセット）
let uv = in.uv;

// 変更後（タイル境界を無視して連続）
let uv = in.world_position.xz / (TILE_SIZE * 2.0);
```

これだけで同種タイル間のシームが完全に消える。**D（テクスチャ）適用後に最初に確認すべき変更**。

### 4.1 入れるもの（優先順）

1. **世界座標 XZ 由来の低周波 UV 歪み**（マクロ）
   - タイルあたり **0.3〜1 周未満**程度を目安。**タイル内で大きく歪めない**。
2. **アルベドのごく弱い明度変調**（±数％程度）
   - **高周波ノイズを強く載せない**（デジタルグレイン化し、`world_lore` と喧嘩しやすい）。
   - 低周波（10〜20 タイルスケール）の FBM で明度にムラをつけると、タイル非依存の自然なパッチ感が出る。
3. **feature tint / palette bias**
   - `shore sand` / `inland sand` / `rock field dirt` / `grass_zone` / `dirt_zone` の意味差を、terrain kind ごとに限定して乗せる。

### 4.2 実装の選択肢

- **ハッシュ系ノイズ**（WGSL のみ）でマクロ歪みと Stochastic 回転 → **テクスチャ追加なし**で軽い。
- または **低解像度ノイズ 1 枚**で制御しやすくする。

### 4.3 `SectionCut` との順序

- **ベースアルベド（＋A の変調）の後**に **クリップ（discard）** が来る形にし、切り口が破綻しにくいようにする。
- `SectionCut` の側面では UV が XZ ベースだと引き伸びる可能性がある。矢視で切断面が目立つ場合は **Triplanar マッピング**（`abs(normal.y)` で上面/側面を自動ブレンド）への変更を検討する。

### 4.4 パラメータ

- **強度は 1〜2 パラメータ**に集約し、デフォルトは弱め。将来の品質プリセットに繋げる余地を残す。

---

## 5. 方針 B — 第 2 段階（実装済み・隣接情報）

**実装結果**: `terrain_id_map`（`R8Unorm`）+ `TerrainSurfaceMaterial` を採用し、center + cardinal 近傍の ID を使って境界をブレンドしている。

**現行仕様**:

- `TerrainChangedEvent` 後は material handle を差し替えず、`terrain_id_map` の該当ピクセルだけを更新する
- 境界ブレンドは **逐次 `mix` ではなく重み正規化した和**で計算する
- ブレンド帯は **cell edge の狭い範囲**に限定し、広い面までにじませない
- river を含むブレンドは **`river↔sand` の組み合わせだけ**を許可する
- `TerrainFeatureMap` は static bake のまま据え置き、runtime 更新は `terrain_id_map` に限定する

### 5.1 推奨サブ案：グリッド ID テクスチャ

- **100×100（`MAP_WIDTH` × `MAP_HEIGHT`）相当の 1 枚**に **セルごとの `TerrainType` インデックス**を書く（R8 またはパック）。
- フラグメントで **世界座標 → UV** し、**±1 texel で四方向隣**を読み、**エッジ付近だけ 2 タイプをブレンド**する設計が取りやすい。
- **CPU**: マップ変更時（障害物除去・将来の地形変更）に **テクスチャを部分更新**する想定。**毎フレーム全タイル更新は不要**。

### 5.2 避けたい例

- **タイルエンティティごと**に隣接を `Storage` で持つ方式は、同期と帯域の面で **ID マップ 1 枚**より重くなりやすい。

---

## 6. 方針 C — 頂点ベイク（本 MS では保留）

- **全タイル同一 `tile_mesh` 共有**の現状では、**頂点カラー等のベイク**は **メッシュ分離**や **チャンク化**とセットでないと扱いにくい。
- **障害物除去 → Dirt** のたびに **近傍タイルの再ベイク**が要る可能性があり、**CPU スパイク**と **`terrain_material_sync_system` との二系統**になりやすい。
- **別マイルストーン**（地形メッシュ戦略の変更）で再評価する。

---

## 7. 現状実装とギャップ

| 箇所 | 内容 |
| --- | --- |
| `world/map/spawn.rs` | `Mesh3d` + `MeshMaterial3d<TerrainSurfaceMaterial>`。全タイルで **共有 1 ハンドル** |
| `terrain_metadata.rs` | startup で `TerrainFeatureMap`（`Rgba8Unorm`）と `TerrainIdMap`（`R8Unorm`）を生成 |
| `visual_handles.rs` | `Terrain3dHandles.surface` に共有 `TerrainSurfaceMaterial` を構築。4 アルベド、macro noise / overlay、river detail、feature LUT を bind |
| `terrain_surface_material.rs` | 地形専用 `ExtendedMaterial<StandardMaterial, ...>` と `sync_section_cut_to_terrain_surface_system` |
| `terrain_surface_material.wgsl` | world-space UV、terrain kind ごとの grade、cardinal 境界ブレンド、`river↔sand` 限定の river 境界処理 |
| `terrain_material.rs` | `TerrainChangedEvent` で **該当セルの `terrain_id_map` ピクセル更新** |

**残ギャップ**: S0 受入スクリーンショット未撮影。runtime での最終目視判定と、必要なら texture 側の微調整が残る。

---

## 8. パフォーマンス評価

### 8.1 現状の要点

| 要因 | 評価 |
| --- | --- |
| **地形マテリアル** | **共有 `TerrainSurfaceMaterial` 1 ハンドル**。4 地形アルベドと metadata を shader 側で切り替える。 |
| **ドロー** | 約 **10,000** エンティティ、**同一メッシュ + 同一マテリアル共有**。 |
| **`SectionCut` 同期** | 地形側は `sync_section_cut_to_terrain_surface_system` が `Assets<TerrainSurfaceMaterial>` を走査。現在は地形 material 数が少ないため負荷は限定的。 |
| **障害物 → Dirt** | イベント駆動・**O(1) ピクセル更新**。`terrain_id_map` の該当 byte だけ書き換える。 |

### 8.2 本 MS の各案の性能イメージ

| 案 | GPU | CPU | メモ |
| --- | --- | --- | --- |
| **D** | 帯域 **やや増**（macro noise / overlay / LUT / detail） | ほぼ ±0 | 現在の表現品質の土台 |
| **A** | **低〜中**（共通ノイズ・feature map 参照） | ±0 | world-space UV 前提で反復感と意味差を返す |
| **B（ID マップ）** | **中**（近傍サンプル＋重み付きブレンド） | **マップ変更時**のピクセル更新 | material 差し替えより局所更新に寄せられる |

### 8.3 本 MS 外だが効く改善（参照）

- **`SectionCut` の全 `SectionMaterial` 走査の縮小**（カット対象だけ更新／グローバル uniform 化）は **別タスク**で効果大。
- **地形チャンク化・LOD** は **別マイルストーン**。

---

## 9. 実装ステップ（推奨順）

### 実装履歴

1. **S1 — D**: 既存 4 地形に加え、macro noise / overlay / LUT / river detail / blend mask を整備した。
2. **S2 — A**: world-space UV・macro variation・feature tint・river flow distortion を導入した。
3. **S3 — B**: `TerrainSurfaceMaterial`・`TerrainIdMap`・cardinal 近傍ブレンドへ移行した。
4. **S4 — 調整**: ブレンド帯を狭め、river を含むブレンドを `river↔sand` に限定した。
5. **S5 — docs**: `world_layout.md` / `architecture.md` / `events.md` / `cargo_workspace.md` / `hw_visual/README.md` を同期した。

### 残作業

6. **S0**: 受入のスクリーンショット基準を固定（トップダウン・矢視）。
7. **S6**: 最終受入判定。必要なら texture 側の微調整だけ追加で実施する。

---

## 10. 完了条件チェックリスト

### WFC 非依存（先行フェーズ）
- [x] **A（ワールド空間 UV）**: タイル境界をまたいで同種テクスチャが連続する（2026-04-01 実装済み）
- [x] **A（metadata / macro variation）**: `terrain_feature_map`・macro noise / overlay・feature LUT を導入済み
- [x] **B（TerrainSurfaceMaterial）**: `terrain_id_map`・cardinal 近傍ブレンド・共有地形 material へ移行済み
- [x] **D**: 4 種アルベド + 追加 texture 群が現行 shader 経路へ接続済み
- [x] **A（歪み・変調）**: Grass / Dirt / Sand の domain warp / brightness variation、river flow distortion を導入済み
- [x] **AddressMode::Repeat**: 地形 4 テクスチャに設定済み（`asset_catalog.rs`）
- [x] **川 UV スクロール**: `uv_scroll_speed = 0.03`（U 方向・左→右の流れ）+ `river_flow_noise` / `river_normal_like`
- [x] **境界ブレンド制約**: ブレンド帯は狭い edge band、river は `river↔sand` のみ
- [ ] 矢視で地形の `SectionCut` が破綻しない（Triplanar 対応を含む）— 目視確認待ち
- [x] `cargo clippy --workspace` 警告ゼロ

### WFC 後フェーズ
- [ ] **S0**: WFC 完了後の地形で受入スクリーンショットを撮影済み
- [x] **B**: マップ変更時の更新経路（`TerrainChangedEvent` → `terrain_id_map` 部分更新）が文書化されている
- [ ] 仕様ドキュメントが実装と一致

---

## 11. 参照

| 文書 | 内容 |
| --- | --- |
| `milestone-roadmap.md` | MS-3-6 |
| `asset-milestones-2026-03-17.md` | MS-Asset-Terrain |
| `docs/world_layout.md` | 地形パイプライン |
| `crates/hw_visual/src/material/section_material.rs` | `SectionMaterial` |

---

## 12. 更新履歴

| 日付 | 内容 |
| --- | --- |
| 2026-03-31 | 初版 |
| 2026-03-31 | 対話での推奨（段階採用 D→A→任意 B、C 保留、A/D 具体策、性能 §8）に全面改稿 |
| 2026-03-31 | WFC 依存分析反映：ワールド空間 UV・Stochastic UV 回転を A に追加、S0・B を WFC 後へ移動、境界ノイズ化は WFC に吸収される旨を §2 に追記、Triplanar の検討を §4.3 に追記 |
| 2026-04-01 | §3 全面改訂：現状ファイル一覧（sand_terrain.png が実際の参照先・sand.png は未使用）、種別ごとの色方針、川アニメーション方式比較（UV スクロール推奨）を追記 |
| 2026-04-01 | メタ情報に実装サブ計画 [`ms-3-6-ad-implementation-plan-2026-04-01.md`](ms-3-6-ad-implementation-plan-2026-04-01.md) を追加 |
| 2026-04-01 | A/D 実装完了に伴い: ステータス更新、§7 ギャップ表を現行 API に合わせ修正（`make_terrain_section_material`・uniform フィールド追加）、§10 チェックリスト更新 |
