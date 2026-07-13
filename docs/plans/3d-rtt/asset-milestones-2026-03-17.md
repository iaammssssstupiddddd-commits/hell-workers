# アセット作成マイルストーン

作成日: 2026-03-17
最終更新: 2026-07-13（実装・外部アセット受入状況を再照合）
ステータス: 進行中（Soul・terrain・shader 完了、建築 GLB pipeline 未着手）

---

## 概要

Phase 3 の正本（`docs/plans/3d-rtt/milestone-roadmap.md`）と連動するアセット制作のマイルストーン。統合前の詳細計画は `archived/phase3-implementation-plan-2026-03-16.md` に保存する。

**基本方針**:
- Soul は GLB モデル + ボーンアニメーション（Bevy AnimationGraph）で Camera3d に直接レンダリングする。Familiar の 3D 化は `MS-3-Fam-R` で価値を再検討する
- Soul についてはスプライトシート・ビルボード・プロキシ並走を段階的に廃止する。Phase 2 の既存スプライト（`soul.png` 等）は移行完了まで維持するが新規整備は行わない
- 建築物・地形は 3D GLB / テクスチャへ移行する（Phase 3 中盤以降）
- コードMSをアンブロックするために必要な最小アセットを先行制作し、品質向上は後続MSで行う

### 2026-07-13 棚卸し結果

| 区分 | 状態 | 次の作業 |
| --- | --- | --- |
| Soul GLB / AnimationGraph / P1 clips / face atlas | ✅ runtime 接続完了 | LOD1 ポリゴン予算を外部原本側で再確認 |
| `section_material.wgsl` / TerrainSurfaceMaterial / 3 LOD | ✅ 実装完了 | roadmap MS-3-6 の目視受入だけ継続 |
| Familiar | ✅ Phase 3 は 2D 前面表示を維持 | 3D 化しない。多層階可視ルールは Phase 4 |
| 建築 GLB pipeline / wall PoC / 全 BuildingType | ❌ 未着手 | MS-Asset-Pipeline → Build-A → Build-B の順で進める |

`assets/` のバイナリは外部同期・gitignore 運用のため、「ファイル制作」と「コード側 runtime 接続」を分けて判定する。コード側の正は HEAD、バイナリ受入は外部 asset manifest と実機読込で確認する。

---

## 現状サマリー

### キャラクタースプライト（legacy / Familiar 前面表示）

> Soul は GLB へ移行済み。Familiar は Phase 3 でも `WorldForeground2dCamera` の 2D 前面表示を維持する。

| ファイル | 状態 | 備考 |
| --- | --- | --- |
| `character/soul.png` | legacy | Soul の通常表示は GLB。新規整備不要 |
| `character/soul_move_spritesheet.png` | legacy | GLB animation に置換済み。新規整備不要 |
| `character/soul_exhausted.png` 他感情系 | legacy | face atlas / GLB animation に置換済み |
| `character/familiar/imp anime 1〜4.png` | ✅ 現役 | Phase 3 の 2D 前面表示で維持 |

### 建築テクスチャ

| ディレクトリ | 状態 | 備考 |
| --- | --- | --- |
| `buildings/wooden_wall/` | ✅ 2D完備 | 全16バリアント。Phase 3 では GLB に置換 |
| `buildings/door/` | ✅ 2D完備 | open/closed。Phase 3 では GLB に置換 |
| `buildings/tank/` | ✅ 2D完備 | empty/half/full |
| `buildings/mud_mixer/` | ✅ 2D完備 | アニメ4フレーム |
| 建築 GLBモデル | ❌ 未受入 | placeholder mesh は現役。建築 GLB は Build-A/B で制作 |

### 地形テクスチャ

| ファイル | 状態 | 備考 |
| --- | --- | --- |
| `grass.png`・`sand.png`・`dirt.png`・`river.png` | ✅ 存在 | `SectionMaterial` ベースカラーとして転用可能 |
| `terrain/grass_edge.png` 等の境界オーバーレイ | ✅ 存在 | 2D境界線。MS-3-6（表面表現改善）で代替される |

### シェーダー

| ファイル | 状態 |
| --- | --- |
| `shaders/section_material.wgsl` | ✅ 実装・wall consumer 接続済み |
| `shaders/terrain_surface_material*.wgsl` | ✅ LOD1 / Lod1Lite / Lod2 / prepass 実装済み |
| `shaders/dream_bubble.wgsl` 等 | ✅ 既存（変更不要） |

---

## アセット制作パイプライン

### 3D GLBモデル（キャラクター）

```
入力画像制作（Camera3d 確定角度に対してモデルが自然に見える正面・側面参照画像）
  ↓
TRELLIS.2 で GLB 生成（有機的シルエット重視。建築物は TripoSR との使い分け）
  ↓
Blender でポリゴン品質確認・Decimate・サブメッシュ分離
  （mesh_body / mesh_face / mesh_curtain（将来）に分離）
  ↓
リギング（Mixamo T ポーズ自動リグ → Blender でボーン調整）
  ↓
アニメーションクリップ追加（Idle / Walk / Work / Carry / Fear / Exhausted）
  ↓
assets/models/characters/ に配置
```

**アートスタイル基準** (`world_lore.md` §6.2):
- Unlit + アウトライン + ポスタライズで「2Dイラスト的な外見」を担保
- 「体積のある存在に見えない」ことを判断基準とする（MS-P3-Pre-C 検証と連動）
- 有機的シルエット重視・低ポリゴンでも印象が成立するモデル

**顔テクスチャアトラスパイプライン（mesh_face 専用）**:
```
generate_image（マゼンタ背景 #FF00FF・表情パターンを6コマ以上含む1枚構成）
  ↓
python scripts/convert_to_png.py "source" "assets/textures/character/face_atlas.png"
  ↓
PNG署名確認
```

### 3D GLBモデル（建築物）

```
入力画像制作（Camera3d 確定角度と同じ撮影角度で generate_image）
  ↓
TRELLIS.2 / TripoSR で GLB 生成（パイプライン: MS-Asset-Pipeline）
  ↓
Blender で品質確認・LOD 調整・2層構造組み込み
  ↓
assets/models/ に配置
```

> ⚠️ GLB 生成パイプラインは **MS-Asset-Pipeline** で構築する。
> Camera3d 角度（MS-P3-Pre-C）が確定するまで入力画像の撮影角度が定まらないため、Pre-C 完了後に着手する。

---

## マイルストーン

### 全体フロー

```
今すぐ着手可
  MS-Asset-Shader: section_material.wgsl 作成 ──────────────→ MS-3-3 先行作業

MS-P3-Pre-C（Camera角度確定）
  │
  ├──→ MS-Asset-0: アートスタイル受入基準確定
  │         │
  │         ├──→ MS-Asset-Char-GLB-A: キャラクター GLB PoC ──→ MS-P3-Pre-D / MS-3-1
  │         │             │
  │         │             ├──→ MS-Asset-Char-GLB-B: アニメーションクリップ整備
  │         │             └──→ MS-Asset-Char-Face: 顔テクスチャアトラス
  │         │
  │         ├──→ MS-Asset-Terrain: 地形テクスチャ整備        ──→ MS-3-4 / MS-3-6
  │         └──→ MS-Asset-Pipeline: GLB生成パイプライン構築
  │                   │
  │                   ├──→ MS-Asset-Build-A: 壁GLB PoC（4バリアント）  ──→ MS-3-5
  │                   └──→ MS-Asset-Build-B: 建築GLBフルセット         ──→ MS-3-5フル
```

---

### MS-Asset-Shader: `section_material.wgsl` 事前作成

> **依存**: なし（今すぐ着手可）
> **ブロック先**: MS-3-3（SectionMaterial 基盤）の作業コストをほぼゼロにする

**やること**:
`section-material-proposal` §3.4 の完全版 WGSL をファイルとして作成する。内容は提案書にほぼ確定しているため、配置のみで MS-3-3 の先行作業が完了する。

**ファイル**: `assets/shaders/section_material.wgsl`

**仕様**（`section-material-proposal` §3.4 より）:
- `SectionMaterialUniforms` に `build_progress`・`wall_height` を含む完全版（`_pad` は不使用）
- `clip_distances: array<f32, 3>` でセクションカット（2面）＋施工進捗（1面）を管理
- `cut_active > 0.5` で矢視クリップ有効
- `wall_height > 0.0` で施工進捗クリップ有効

**完了条件**:
- [x] `assets/shaders/section_material.wgsl` が存在する
- [x] 提案書 §3.4 の完全版 WGSL と一致している

**ステータス**: [x] 完了

---

### MS-Asset-0: アートスタイル受入基準確定

> **依存**: MS-P3-Pre-C（Camera3d 角度確定後、実際の見え方で判断する）
> **ブロック先**: MS-Asset-Char-A・MS-Asset-Build-A・MS-Asset-Terrain・MS-3-10（アウトライン計画）

**やること**:
`world_lore.md` §6.2 の記述をベースに、以下の未定義項目を確定して `docs/art-style-criteria.md` として文書化する。

| 未定義項目 | 選択肢 | 判断タイミング |
| --- | --- | --- |
| キャラクタースプライト方向数 | 8方向 / 左右反転のみ | MS-P3-Pre-D（V-3）の目視確認後 |
| アウトライン線幅 | 細（1px）〜太（3px） | Phase 3 GLB PoC の目視で判断 |
| アウトライン揺らぎ量 | 弱〜強 | 同上 |
| アウトライン色 | 純黒 / 暗茶 | 同上 |
| ズームアウト無効化閾値 | Camera2d scale 値 | 同上 |
| 壁ノーマルマップ | あり / なし | MS-Asset-Build-A の PoC で判断 |

**成果物**: `docs/art-style-criteria.md`（新規作成）

**完了条件**:
- [x] キャラクタースプライトの方向数が確定している（GLB/3D化により左右ミラーのみで対応。スプライト方向数は廃止）
- [x] アウトライン受入基準（線幅・揺らぎ・色・閾値）が数値または比較サンプルで記述されている（仮基準: 2px・中・暗茶 #1a0a00。PoC後に確定）

**ステータス**: [x] 完了（2026-03-21 / `docs/art-style-criteria.md` 作成済み。アウトライン詳細はPoC後に更新）

---

### MS-Asset-Char-GLB-A: キャラクター GLB PoC

> **依存**: MS-Asset-0（アートスタイル受入基準確定後）・MS-P3-Pre-C（Camera角度確定後）
> **ブロック先**: MS-P3-Pre-D（Character GLB PoC）・MS-3-1（Soul CharacterMaterial 本実装）

**やること**:
1. Soul の参照画像を制作する（正面・側面を Camera3d 確定角度に合わせる）
2. TRELLIS.2 で Soul GLB を生成し `assets/models/characters/soul.glb` に配置する
3. Blender で品質確認・Decimate（LOD1 目安: 200〜400 三角形）
4. サブメッシュ分離: `mesh_body` / `mesh_face`（`mesh_curtain` は将来）
5. Mixamo で T ポーズ自動リグを適用し Blender でエクスポート確認する
6. Bevy で GLB が読み込めること・ボーンが正しくインポートされることを確認する（コード側は仮スポーン）

**LOD 目安（PoC 段階・確定は壁2層構造 PoC 結果待ち）**:
| LOD | 三角形数 | 用途 |
| --- | --- | --- |
| LOD0 | 600〜1,200 | セクションビュー |
| LOD1 | 200〜400 | 通常プレイ |

**完了条件**:
- [x] `assets/models/characters/soul.glb` が Bevy で読み込めている
- [x] `mesh_body` / `mesh_face` サブメッシュが分離されている
- [x] ボーンリグが AnimationGraph で参照できることを確認済み
- [ ] LOD1 ポリゴン数が目安範囲内

**進捗メモ**:
- [x] `assets/models/characters/soul.glb` を `assets/models/characters/` へ配置済み
- [x] `animation_list.md` / `soul_face_atlas.png` / `soul_face_atlas_layout.md` もリポジトリへ同期済み
- [x] GLB 内に `mesh_face` / `Soul_Body` / 複数 animation clip が含まれていることを確認済み

**ステータス**: [~] runtime 接続完了・LOD1 ポリゴン予算の再確認待ち

---

### MS-Asset-Char-GLB-B: アニメーションクリップ整備

> **依存**: MS-Asset-Char-GLB-A 完了
> **ブロック先**: MS-3-Char-A（AnimationGraph + SoulAnimState 実装）

**やること**: Soul の基本アクションクリップを Blender で制作し GLB に含める。

| クリップ名 | 内容 | 優先度 |
| --- | --- | --- |
| `Idle` | 静止・微浮遊（2〜4秒ループ） | P0 |
| `Walk` | 移動（全方向 blend で対応） | P0 |
| `Work` | 壁への作業動作 | P1 |
| `Carry` | アイテム運搬 | P1 |
| `Fear` | 恐怖状態（震え） | P2 |
| `Exhausted` | 疲弊状態 | P2 |

Familiar は Soul の本実装と表示方式再検討（MS-3-Fam-R）後に要否を判断する。

**完了条件**:
- [x] P0 クリップ（Idle・Walk）が AnimationGraph で再生できる（コード側 MS-3-Char-A と同時検証）
- [x] P1 クリップがタスク状態と連動して切り替わる（Work / Carry / Fear / Exhausted）

**進捗メモ**:
- [x] `assets/models/characters/soul.glb` と `assets/models/characters/animation_list.md` は同期済み
- [x] 現行 GLB には `Carry / Exhausted / Fear / Idle / Walk / WalkLeft / WalkRight / Work` が含まれている
- [x] Bevy 側で `named_animations` を読む `SoulAnimationLibrary` 基盤は実装済み
- [x] AnimationGraph での Idle / Walk / WalkLeft / WalkRight 再生確認は実施済み

**ステータス**: [x] 全 P1 クリップの runtime 連動確認済み

---

### MS-Asset-Char-Face: 顔テクスチャアトラス

> **依存**: MS-Asset-0（アートスタイル受入基準確定後）
> **ブロック先**: MS-3-Char-B（Soul の face atlas 状態連動）

**やること**:
1. 顔テクスチャアトラスのレイアウトを確定する（`character-3d-rendering-proposal` §3.8 参照）
2. `generate_image` で表情を制作する（背景: マゼンタ #FF00FF）
3. 1 枚のアトラス PNG に統合する（`assets/textures/character/soul_face_atlas.png`）

**最小セット（P0）**:
| コマ | 内容 |
| --- | --- |
| (0,0) | 通常 |
| (1,0) | 恐怖 |
| (2,0) | 疲弊 |
| (0,1) | 集中（作業中） |

**完了条件**:
- [x] `soul_face_atlas.png` が存在する
- [x] `CharacterMaterial.face_uv_offset` で表情が切り替わる（MS-3-Char-A / MS-3-Char-B の前段確認）

**進捗メモ**:
- [x] `assets/textures/character/soul_face_atlas.png` は同期済み
- [x] `assets/textures/character/soul_face_atlas_layout.md` も同期済み
- [x] atlas には `通常 / 恐怖 / 疲弊 / 集中 / 喜び / 睡眠` の 6 状態が含まれている
- [x] per-instance face material と `face_uv_offset` 更新経路は実装済み
- [x] face atlas の状態切り替え目視確認は実施済み

**ステータス**: [x] 確認済み（アセット配置・コード側接続・目視確認完了）

---

### MS-Asset-Pipeline: GLB生成パイプライン構築

> **依存**: MS-P3-Pre-C（Camera3d 角度確定後。入力画像の撮影角度が定まる）
> **ブロック先**: MS-Asset-Build-A

**やること**:
1. TRELLIS.2 / TripoSR の動作環境を確認する
2. 入力画像の撮影角度を Camera3d 確定角度に統一する（角度が揃うことで生成後の向き修正工数を削減）
3. 生成 → Blender 品質確認 → LOD 調整 → `assets/models/` 配置の手順を文書化する
4. テスト用（最も単純な形状：直線壁）でパイプラインを 1 周させる

**Blender 品質ゲート（LOD1 基準）**:
- LOD1: 100 三角形以下を目安（`section-material-proposal` §8.5 より）
- LOD0: セクションビュー用（高品質・上限なし）

**成果物**: `docs/asset-pipeline-glb.md`（手順書）

**完了条件**:
- [ ] テスト用 GLB が `assets/models/` に配置されゲーム内で読み込める
- [ ] GLB 生成の手順書が存在する

**ステータス**: [ ] 未着手

---

### MS-Asset-Build-A: 壁GLB PoC（4バリアント）

> **依存**: MS-Asset-Pipeline 完了・MS-Asset-0 完了
> **ブロック先**: MS-3-5（Building3dHandles SectionMaterial 移行）の前提となる PoC

`billboard-camera-angle-proposal` §7 の壁メッシュ構成に従い、最初の 4 バリアントを制作する。

**制作ファイル**:
| ファイル | 形状 | アウトライン検出 |
| --- | --- | --- |
| `assets/models/wall_straight.glb` | 直線 | 側面稜線あり |
| `assets/models/wall_corner.glb` | L字 | **外側コーナー稜線あり**（エッジ検出に必須） |
| `assets/models/wall_t_junction.glb` | T字 | 外側稜線あり |
| `assets/models/wall_cross.glb` | 十字 | 外側稜線あり |

**2層構造の仕様**（`section-material-proposal` §8 より）:
- `completed` 層: 外側・`build_progress` ユニフォームで Y 方向クリップ（下から生える表現）
- `blueprint` 層: 内側・常に全高表示（`wall_height=0.0` でクリップ無効）
- セクションビューで壁を切断すると内側に blueprint 層が見える

**LOD**:
- LOD0: セクションビュー用（高品質）
- LOD1: 通常プレイ用（100 三角形以下目安）
- LOD2: ズームアウト時（後回し可）

**完了条件**:
- [ ] 4 バリアントが `assets/models/` に存在する
- [ ] `SectionMaterial` を付けてスラブクリップが正しく動作する（目視）
- [ ] `build_progress` クリップで施工中アニメーションが動作する（目視）
- [ ] 通常ビュー（斜め約53°）で「黒い石積み」の見た目が成立する
- [ ] ポリゴン数が LOD1 基準（100 三角形以下）を満たす

**ステータス**: [ ] 未着手

---

### MS-Asset-Build-B: 建築GLBフルセット

> **依存**: MS-Asset-Build-A（パイプライン確立・品質ゲート定義済み）
> **ブロック先**: MS-3-5（全 BuildingType の SectionMaterial 移行完了）

| ファイル | BuildingType | 備考 |
| --- | --- | --- |
| `assets/models/wall.glb` | Wall | 接続バリアントは Build-A の4形状から開始 |
| `assets/models/door.glb` | Door | open / closed の2状態 |
| `assets/models/floor.glb` | Floor | 石畳。薄い slab で可 |
| `assets/models/tank.glb` | Tank | empty / half / full は material または子 mesh で表現 |
| `assets/models/mud_mixer.glb` | MudMixer | 稼働アニメーションを後付け可能にする |
| `assets/models/rest_area.glb` | RestArea | 1x1 temporary building |
| `assets/models/bridge.glb` | Bridge | 3D placeholder 置換対象 |
| `assets/models/sand_pile.glb` | SandPile | 1x1 temporary building |
| `assets/models/bone_pile.glb` | BonePile | 1x1 temporary building |
| `assets/models/wheelbarrow_parking.glb` | WheelbarrowParking | 1x1 temporary building |
| `assets/models/soul_spa.glb` | SoulSpa | site / tile 構造との接続を確認 |
| `assets/models/outdoor_lamp.glb` | OutdoorLamp | local light child の取付点を定義 |

**完了条件**:
- [ ] 全 BuildingType の GLB が `assets/models/` に存在する
- [ ] `SectionMaterial` 付きで矢視断面確認が可能
- [ ] LOD1 ポリゴン基準を満たす

**ステータス**: [ ] 未着手

---

### MS-Asset-Terrain: 地形テクスチャ整備

> **依存**: MS-Asset-0（受入基準確定後）
> **ブロック先**: MS-3-4（テレイン3D化）・MS-3-6（テレイン表面表現改善）

**現状**: `grass.png`・`sand.png`・`dirt.png`・`river.png` は `TerrainSurfaceMaterial` に接続済み。境界は `terrain_id_map`、macro / feature は `terrain_feature_map`、川は shader の scroll / distortion で表現し、旧 2D 境界オーバーレイには依存しない。

**やること**:
1. 既存テクスチャを `TerrainSurfaceMaterial` で受入確認する
2. WFC terrain mask と狭い cell-edge blend の見た目を受入確認する
3. river の shader scroll / distortion を受入確認する
4. 壁ノーマルマップの要否は MS-3-10 / Build-A の PoC へ分離する

**完了条件**:
- [x] 4 種（草・砂・土・川）が `TerrainSurfaceMaterial` に接続されている
- [x] 境界ブレンドと 3 LOD shader が実装されている

**ステータス**: [x] 実装完了（最終目視受入は roadmap MS-3-6）

---

## Phase 3 コードMSとの依存関係

| アセットMS | アンブロックするコードMS | 備考 |
| --- | --- | --- |
| MS-Asset-Shader | MS-3-3 | シェーダーファイルが先に存在すると MS-3-3 の実装コストがほぼゼロ |
| MS-Asset-Char-GLB-A | MS-P3-Pre-D・MS-3-1 | Character GLB がないと Soul 本実装の目視確認ができない |
| MS-Asset-Char-GLB-B | MS-3-Char-A | AnimationGraph 実装の視覚確認に必要 |
| MS-Asset-Char-Face | MS-3-Char-B | Soul の face atlas 状態連動の目視確認に必要 |
| MS-Asset-Terrain | MS-3-4・MS-3-6 | 実装済み。MS-3-6 の最終目視受入だけ継続 |
| MS-Asset-Pipeline | MS-Asset-Build-A | 建築 GLB の品質向上を開始する前提。MS-3-5 の material 契約は placeholder で先行可能 |
| MS-Asset-Build-A | 建築 visual quality | 壁 4 バリアントの品質ゲート。MS-3-5 の material 契約は placeholder で先行可能 |
| MS-Asset-Build-B | 建築 visual quality | 全 BuildingType の placeholder を最終 GLB に置換する |
| MS-Asset-0 | MS-3-10 | 仮基準は完了。outline と壁ノーマルの PoC 受入が残る |

---

## 優先度ガイド

| 優先 | MS | 理由 |
| --- | --- | --- |
| P0 | MS-Asset-Pipeline | 建築 GLB の生成・Blender 品質確認・外部同期手順を確立する |
| P1 | MS-Asset-Build-A | 直線・corner・T・cross の wall PoC で品質ゲートを固定する |
| P2 | MS-Asset-Build-B | 現行 12 `BuildingType` の placeholder を順次 GLB へ置換する |
| P2 | MS-Asset-0 residual | Soul outline と壁ノーマルの PoC 受入値を確定する |
| 完了 | Shader / Soul GLB / clips / face / terrain | runtime 接続済み。履歴は各マイルストーンに残す |

---

## 関連ドキュメント

| ドキュメント | 内容 |
| --- | --- |
| `docs/plans/3d-rtt/archived/phase3-implementation-plan-2026-03-16.md` | 統合前の Phase 3 コード実装計画（履歴） |
| `docs/plans/3d-rtt/milestone-roadmap.md` | Phase 全体の依存グラフ |
| `docs/proposals/3d-rtt/archived/billboard-camera-angle-proposal-2026-03-16.md` | Camera3d / 壁メッシュ採用判断の履歴 |
| `docs/proposals/3d-rtt/archived/section-material-proposal-2026-03-16.md` | 壁 2 層構造・build_progress・WGSL 採用判断の履歴 |
| `docs/proposals/soul-outline-mask-ring-proposal-2026-04-16.md` | Soul outline の現行提案 |
| `docs/proposals/3d-rtt/archived/character-3d-rendering-proposal-2026-03-16.md` | CharacterMaterial・AnimationGraph・顔 atlas 採用判断の履歴 |
| `docs/world_lore.md` §6.2〜6.3・§8 | アートスタイル仕様・アセットリスト |
| `docs/DEVELOPMENT.md` | 2D スプライト制作パイプライン（generate_image → convert_to_png.py） |
