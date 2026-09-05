# アセット作成マイルストーン

作成日: 2026-03-17
最終更新: 2026-09-05（壁の本番アートをcanonical generation 4へ昇格）
ステータス: 進行中（建築・terrain track継続、Soul GLB runtime trackはSuperseded）

---

## 概要

単一Scene RtT移行計画と連動するアセット制作のマイルストーン。旧Phase 3の完了履歴は`docs/plans/3d-rtt/milestone-roadmap.md`と`archived/phase3-implementation-plan-2026-03-16.md`に保存する。

> **2026-08-03 方針変更:** Soul visible GLB固定、billboard廃止、全BuildingTypeのGLB化、section view用LOD0を新規作業の前提にしない。Soul GLB / animation / face atlasは完了履歴とfallback assetとして保持し、runtime表示は[`single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) M2の共有unlit billboardで再評価する。建築trackは同計画のpresentation mappingで`Structural3d`に分類された種類だけを対象に継続する。

> **2026-09-05 壁track完了:** `MS-Asset-Build-A`の壁は本番アートへ置換済みである。asset set
> `wall-production-v1` generation 4（manifest SHA-256 `7ecdfbb0…`）がcanonical generation storeへ昇格し、
> repo runtime mirrorと`assets/manifests/wall-production-v1.wallset`のrelease projectionまで同期済みである。
> 制作・受入の恒久仕様は`docs/assets_workflow.md`、`docs/art-style-criteria.md`、
> `docs/rendering-performance.md`、`docs/building.md`にある。

> **2026-08-31 壁track詳細化:** `MS-Asset-Pipeline`から`MS-Asset-Build-A`の制作、runtime接続、16接続形状、実機受入、canonical昇格は[`production-wall-art-plan-2026-08-31.md`](archived/production-wall-art-plan-2026-08-31.md)を正本とする。下記の旧PoC条件と矛盾する場合は詳細計画を優先する。

**基本方針**:
- Soul GLB / AnimationGraph / face atlasは既存成果物として保持するが、新規runtime拡張は行わない。通常表示は共有unlit billboard PoCを正本候補とし、失敗時だけvisible GLB 1系統へfallbackする
- Familiarは2D foregroundを維持し、Wall depthが必要になった場合だけ共有billboard経路へ移す
- 地形、Wall、Door、Floor、大型またはdepth /遮光が必要な建築物だけを3D asset対象にし、小型・装飾建物を一律GLB化しない
- コードMSをアンブロックするために必要な最小アセットを先行制作し、品質向上は後続MSで行う

### 2026-07-13 棚卸し結果

| 区分 | 状態 | 次の作業 |
| --- | --- | --- |
| Soul GLB / AnimationGraph / P1 clips / face atlas | ✅ 既存runtime成果物 | P02 billboardのfallback / animation sourceとしてのみ再利用を判断 |
| `section_material.wgsl` / TerrainSurfaceMaterial / 3 LOD | ✅ 既存実装 | P00 baselineとP06 Light Field接続でTopDown表示を再受入 |
| Familiar | ✅ 2D foregroundを維持 | 3D化しない。Wall depthが必要になった場合だけ別判断でbillboardへ移す |
| 建築 GLB pipeline / wall PoC / `Structural3d` BuildingType | ❌ 未着手 | [`production-wall-art-plan-2026-08-31.md`](archived/production-wall-art-plan-2026-08-31.md) M0〜M6でPipeline / Build-Aを閉じてからBuild-Bへ進む |

`assets/` のバイナリは外部同期・gitignore 運用のため、「ファイル制作」と「コード側 runtime 接続」を分けて判定する。コード側の正は HEAD、バイナリ受入は外部 asset manifest と実機読込で確認する。

---

## 現状サマリー

### キャラクター表示asset（Soul billboard再評価 / Familiar前面表示）

> Soulの現行表示はGLBだが、単一Scene RtT移行P02で共有unlit billboardへ再評価する。FamiliarはCamera2d foregroundを維持する。

| ファイル | 状態 | 備考 |
| --- | --- | --- |
| `character/soul.png` | billboard候補 | M2で足元anchor・alpha silhouette・atlas共有を評価 |
| `character/soul_move_spritesheet.png` | billboard候補 | M2でanimation sourceとして再利用可否を評価 |
| `character/soul_exhausted.png` 他感情系 | billboard候補 | state variantの共有atlas化を評価 |
| `character/familiar/imp anime 1〜4.png` | ✅ 現役 | Phase 3 の 2D 前面表示で維持 |

### 建築テクスチャ

| ディレクトリ | 状態 | 備考 |
| --- | --- | --- |
| `buildings/wooden_wall/` | ✅ 2D完備 | 全16バリアント。M2分類後もfallback / connection参照として保持 |
| `buildings/door/` | ✅ 2D完備 | open/closed。M2で3D DoorState同期とfallback用途を整理 |
| `buildings/tank/` | ✅ 2D完備 | empty/half/full |
| `buildings/mud_mixer/` | ✅ 2D完備 | アニメ4フレーム |
| 建築 GLBモデル | ❌ 未受入 | placeholder mesh は現役。建築 GLB は Build-A/B で制作 |

### 地形テクスチャ

| ファイル | 状態 | 備考 |
| --- | --- | --- |
| `grass.png`・`sand.png`・`dirt_seamless.png`・`river.png` | ✅ 存在 | `SectionMaterial` ベースカラーとして転用可能。Dirt は黒格子を含まない反復用素材を使用 |
| `terrain/grass_edge.png` 等の境界オーバーレイ | ✅ 存在 | 2D境界線。MS-3-6（表面表現改善）で代替される |

### シェーダー

| ファイル | 状態 |
| --- | --- |
| `shaders/section_material.wgsl` | ✅ 実装・wall consumer 接続済み |
| `shaders/terrain_surface_material*.wgsl` | ✅ LOD1 / Lod1Lite / Lod2 / prepass 実装済み |
| `shaders/dream_bubble.wgsl` 等 | ✅ 既存（変更不要） |

---

## アセット制作パイプライン

### 3D GLBモデル（キャラクター、完了履歴 / fallback）

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

### 現行フロー

```text
単一Scene RtT P00（契約 / baseline）
  -> P02（presentation mapping確定）
       -> MS-Asset-Pipeline
            -> MS-Asset-Build-A（Wall PoC）
            -> MS-Asset-Build-B（Structural3dのみ）
       -> Soul billboard atlas / alpha silhouette PoC
```

### 旧Phase 3フロー（完了・凍結依存の履歴）

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
  │                   ├──→ MS-Asset-Build-A: 壁GLB PoC（6バリアント）  ──→ MS-3-5
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
> **ブロック先**: MS-Asset-Char-A・MS-Asset-Build-A・MS-Asset-Terrain・単一Scene RtT移行P02（billboard silhouette）

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

**ステータス**: [x] 完了（2026-03-21 / `docs/art-style-criteria.md` 作成済み。外周受入はM2 billboard PoCへ再接続）

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
> **詳細実行計画**: [`production-wall-art-plan-2026-08-31.md`](archived/production-wall-art-plan-2026-08-31.md) M0〜M1

**やること**:
1. TRELLIS.2 / TripoSR の動作環境を確認する
2. 入力画像の撮影角度を Camera3d 確定角度に統一する（角度が揃うことで生成後の向き修正工数を削減）
3. 生成 → Blender 品質確認 → LOD 調整 → `assets/models/` 配置の手順を文書化する
4. テスト用（最も単純な形状：直線壁）でパイプラインを 1 周させる

**Blender 品質ゲート（TopDown production mesh）**:
- Wall production mesh: 150〜250 trianglesを目標、350 trianglesをhard capとする（現行`docs/rendering-performance.md`の専用予算を正とする）
- wall専用runtime LOD切替や別LOD assetは現計画の制作要件にしない

**成果物**: `docs/asset-pipeline-glb.md`（手順書）

**完了条件**:
- [ ] テスト用 GLB が外部stagingからhash固定のprimary外・clean validation worktreeへprovisionされ、ゲーム内で読み込める
- [ ] GLB 生成の手順書が存在する

**ステータス**: [ ] 未着手

---

### MS-Asset-Build-A: 壁GLB PoC（6バリアント）

> **依存**: MS-Asset-Pipeline 完了・MS-Asset-0 完了
> **ブロック先**: 建築visual quality、MS-Asset-Build-B、新PC移行計画M4の最初のcanonical asset受入（P02 / P06のruntime契約は完了済み）
> **詳細実行計画**: [`production-wall-art-plan-2026-08-31.md`](archived/production-wall-art-plan-2026-08-31.md) M2〜M6。本節は成果物一覧だけを保持する。

`billboard-camera-angle-proposal` §7 の4形状案へ孤立・端を補い、16接続maskの意味を形状で保持する6バリアントを制作する。

**制作ファイル**:
| ファイル | 形状 | アウトライン検出 |
| --- | --- | --- |
| `assets/wall_sets/<GEN>/models/wall_isolated.glb` | 孤立 | 全周silhouette |
| `assets/wall_sets/<GEN>/models/wall_end.glb` | 片端 | 接続側だけの腕と終端cap |
| `assets/wall_sets/<GEN>/models/wall_straight.glb` | 直線 | 側面稜線あり |
| `assets/wall_sets/<GEN>/models/wall_corner.glb` | L字 | **外側コーナー稜線あり**（エッジ検出に必須） |
| `assets/wall_sets/<GEN>/models/wall_t_junction.glb` | T字 | 外側稜線あり |
| `assets/wall_sets/<GEN>/models/wall_cross.glb` | 十字 | 外側稜線あり |

**TopDown構造の仕様**:
- `completed`層はWall接続形状、depth、directional shadowを成立させる。shared Light Field bindingは維持するが、現fragmentの未sample契約を壁作業だけで変更しない
- 建設中表現は現行`ConstructionMask3dVisual`とProvisionalWall material遷移を維持し、未接続の`build_progress` clippingを本PoCの要件にしない
- section cutで見せる内側層は制作要件にしない
- 見た目の公称壁厚は各armの局所横断で`9.6 wu = 0.30 tile`とし、全腕の連続visible cross-sectionも9.6 wu未満へ細らせない。石・鉄・トゲ込み局所横断外形は12.8 wu以下、全6形状の接続境界は幅9.6 wuの同一profileとする
- 論理占有は32×32 wuの1 cellを維持する。現行Door leafの5.76 wu厚はplaceholderであり、Wall厚の根拠や無隙間jambの受入値にはしない

**Mesh tier**:
- 本PoCはTopDown通常プレイ用production mesh 1段だけを作る（150〜250 triangles目標、350 triangles hard cap）
- LOD0 / LOD2 assetとwall専用runtime LOD systemは本計画の非対象

**完了条件**:
- [ ] 6 バリアントがprimary外のclean validation worktreeの`assets/wall_sets/<GEN>/models/`に存在し、Bevyで全件resident、fallback使用0である
- [ ] 6 mesh＋Y軸回転でWall / Doorを含む全16接続maskが正しく見える
- [ ] 全family / rotationで局所横断の連続壁体9.6 wu以上、装飾外形12.8 wu以下、境界port共通profileを満たす。14 px per tileでは内部色が残り、最大zoom-out 5ではHighの内部色1 px以上、Medium / Lowの最終合成silhouette連続を満たす
- [ ] completed / provisionalが同じmesh topologyと有限shared material poolを使い、完成時はmaterialだけが遷移する
- [ ] 現行TopDown斜視で「黒い石積み」の見た目が成立する
- [ ] 各production meshのポリゴン数がhard cap（350 triangles以下）を満たす
- [ ] 専用wall galleryと既存P02 actual-window回帰がfail-closedで合格し、ユーザーが見た目を承認する

**ステータス**: [ ] 未着手

---

### MS-Asset-Build-B: 構造3D建築GLBセット

> **依存**: MS-Asset-Build-A（パイプライン確立・品質ゲート定義済み）
> **ブロック先**: 単一Scene RtT移行P02 / P06（presentation mappingとLight Field receiver）

下表は制作候補であり、全件必須ではない。M2のpresentation mappingで`Structural3d`またはdepth / Room境界 / light receiverに分類された種類だけを制作対象に確定し、`Foreground2d`は既存spriteを維持する。

| ファイル | BuildingType | 備考 |
| --- | --- | --- |
| `assets/wall_sets/<GEN>/models/wall_*.glb` | Wall | 接続バリアントは Build-A の6形状を使用。active `.wallset` projectionから参照 |
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
| `assets/models/outdoor_lamp.glb` | OutdoorLamp | 3D分類された場合のみ。発光原点・mount sideはGLB childではなくlogical emitter契約を正本にする |

**完了条件**:
- [ ] `Structural3d`に分類された全BuildingTypeのGLBまたは受入済みplaceholderが存在する
- [ ] TopDownでdepth、Door状態、Light Field receiverの表示が成立する
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

**ステータス**: [x] 実装完了（TopDown最終目視とLight Field接続は移行計画P00 / P06）

---

## 旧Phase 3コードMSとの依存関係（履歴）

| アセットMS | アンブロックするコードMS | 備考 |
| --- | --- | --- |
| MS-Asset-Shader | MS-3-3 | シェーダーファイルが先に存在すると MS-3-3 の実装コストがほぼゼロ |
| MS-Asset-Char-GLB-A | MS-P3-Pre-D・MS-3-1 | Character GLB がないと Soul 本実装の目視確認ができない |
| MS-Asset-Char-GLB-B | MS-3-Char-A | AnimationGraph 実装の視覚確認に必要 |
| MS-Asset-Char-Face | MS-3-Char-B | Soul の face atlas 状態連動の目視確認に必要 |
| MS-Asset-Terrain | MS-3-4・MS-3-6 | 実装済み。MS-3-6 の最終目視受入だけ継続 |
| MS-Asset-Pipeline | MS-Asset-Build-A | 建築 GLB の品質向上を開始する前提。MS-3-5 の material 契約は placeholder で先行可能 |
| MS-Asset-Build-A | 建築 visual quality | 壁 6 バリアントの品質ゲート。MS-3-5 の material 契約は placeholder で先行可能 |
| MS-Asset-Build-B | 建築 visual quality | `Structural3d`に分類された種類だけを最終GLBへ置換する |
| MS-Asset-0 | MS-3-10 | 仮基準は完了。outline と壁ノーマルの PoC 受入が残る |

---

## 優先度ガイド

| 優先 | MS | 理由 |
| --- | --- | --- |
| P0 | MS-Asset-Pipeline | 建築 GLB の生成・Blender 品質確認・外部同期手順を確立する |
| P1 | MS-Asset-Build-A | isolated・end・straight・corner・T・cross の wall PoC で品質ゲートを固定する |
| P2 | MS-Asset-Build-B | M2で`Structural3d`に分類された建物だけを順次GLBへ置換する |
| P2 | MS-Asset-0 residual | billboard alpha silhouetteと壁ノーマルのPoC受入値を確定する |
| 完了 / fallback | Shader / Soul GLB / clips / face / terrain | 既存runtime接続済み。Soul系は新規本線にせず履歴とfallbackとして保持 |

---

## 関連ドキュメント

| ドキュメント | 内容 |
| --- | --- |
| `docs/plans/3d-rtt/single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md` | 現行のpresentation分類・Soul billboard・構造3D asset正本 |
| `docs/plans/3d-rtt/archived/phase3-implementation-plan-2026-03-16.md` | 統合前の Phase 3 コード実装計画（履歴） |
| `docs/plans/3d-rtt/milestone-roadmap.md` | Phase 全体の依存グラフ |
| `docs/proposals/3d-rtt/archived/billboard-camera-angle-proposal-2026-03-16.md` | Camera3d / 壁メッシュ採用判断の履歴 |
| `docs/proposals/3d-rtt/archived/section-material-proposal-2026-03-16.md` | 壁 2 層構造・build_progress・WGSL 採用判断の履歴 |
| `docs/proposals/soul-outline-mask-ring-proposal-2026-04-16.md` | Soul mask outlineのSuperseded比較履歴 |
| `docs/proposals/3d-rtt/archived/character-3d-rendering-proposal-2026-03-16.md` | CharacterMaterial・AnimationGraph・顔 atlas 採用判断の履歴 |
| `docs/world_lore.md` §6.2〜6.3・§8 | アートスタイル仕様・アセットリスト |
| `docs/DEVELOPMENT.md` | 2D スプライト制作パイプライン（generate_image → convert_to_png.py） |
