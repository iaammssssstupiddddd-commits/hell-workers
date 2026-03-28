# アセット作成マイルストーン

作成日: 2026-03-17
最終更新: 2026-03-28（Soul 優先・Familiar 再検討方針を反映）
ステータス: 未着手

---

## 概要

Phase 3 実装計画（`docs/plans/3d-rtt/phase3-implementation-plan-2026-03-16.md`）と連動するアセット制作のマイルストーン。

**基本方針**:
- Soul は GLB モデル + ボーンアニメーション（Bevy AnimationGraph）で Camera3d に直接レンダリングする。Familiar の 3D 化は `MS-3-Fam-R` で価値を再検討する
- Soul についてはスプライトシート・ビルボード・プロキシ並走を段階的に廃止する。Phase 2 の既存スプライト（`soul.png` 等）は移行完了まで維持するが新規整備は行わない
- 建築物・地形は 3D GLB / テクスチャへ移行する（Phase 3 中盤以降）
- コードMSをアンブロックするために必要な最小アセットを先行制作し、品質向上は後続MSで行う

---

## 現状サマリー

### キャラクタースプライト（Phase 2 プロキシ用・維持のみ）

> Phase 3 でキャラクターは GLB モデルへ完全移行する。スプライトの新規整備は行わない。
> 以下は Phase 2 プロキシが参照している現状の管理状態。

| ファイル | 状態 | 備考 |
| --- | --- | --- |
| `character/soul.png` | ✅ 実装済み | Phase 2 プロキシで使用中。Phase 3 移行後は参照なし |
| `character/soul_move_spritesheet.png` | ⚠️ ファイル存在・未接続 | Phase 3 では使用しない。整備不要 |
| `character/soul_exhausted.png` 他感情系 | ✅ 実装済み | Phase 2 プロキシで使用中。Phase 3 移行後は参照なし |
| `character/familiar/imp anime 1〜4.png` | ⚠️ 個別ファイル×4 | Phase 3 では使用しない。整備不要 |

### 建築テクスチャ

| ディレクトリ | 状態 | 備考 |
| --- | --- | --- |
| `buildings/wooden_wall/` | ✅ 2D完備 | 全16バリアント。Phase 3 では GLB に置換 |
| `buildings/door/` | ✅ 2D完備 | open/closed。Phase 3 では GLB に置換 |
| `buildings/tank/` | ✅ 2D完備 | empty/half/full |
| `buildings/mud_mixer/` | ✅ 2D完備 | アニメ4フレーム |
| GLBモデル | ❌ 存在しない | Phase 3 で全新規作成が必要 |

### 地形テクスチャ

| ファイル | 状態 | 備考 |
| --- | --- | --- |
| `grass.png`・`sand.png`・`dirt.png`・`river.png` | ✅ 存在 | `SectionMaterial` ベースカラーとして転用可能 |
| `terrain/grass_edge.png` 等の境界オーバーレイ | ✅ 存在 | 2D境界線。MS-3-6（表面表現改善）で代替される |

### シェーダー

| ファイル | 状態 |
| --- | --- |
| `shaders/section_material.wgsl` | ❌ 存在しない。MS-3-3 の前提 |
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
- [ ] `assets/models/characters/soul.glb` が Bevy で読み込めている
- [ ] `mesh_body` / `mesh_face` サブメッシュが分離されている
- [ ] ボーンリグが AnimationGraph で参照できることを確認済み
- [ ] LOD1 ポリゴン数が目安範囲内

**進捗メモ**:
- [x] `assets/models/characters/soul.glb` を `assets/models/characters/` へ配置済み
- [x] `animation_list.md` / `soul_face_atlas.png` / `soul_face_atlas_layout.md` もリポジトリへ同期済み
- [x] GLB 内に `mesh_face` / `Soul_Body` / 複数 animation clip が含まれていることを確認済み

**ステータス**: [~] 進行中

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
- [ ] P1 クリップがタスク状態と連動して切り替わる（MS-3-Char-A 完了後に確認）

**進捗メモ**:
- [x] `assets/models/characters/soul.glb` と `assets/models/characters/animation_list.md` は同期済み
- [x] 現行 GLB には `Carry / Exhausted / Fear / Idle / Walk / WalkLeft / WalkRight / Work` が含まれている
- [x] Bevy 側で `named_animations` を読む `SoulAnimationLibrary` 基盤は実装済み
- [x] AnimationGraph での Idle / Walk / WalkLeft / WalkRight 再生確認は実施済み

**ステータス**: [ ] P1 連動待ち（P0 クリップ再生確認済み）

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
- [ ] `soul_face_atlas.png` が存在する
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
| `assets/models/floor.glb` | Floor | 石畳。薄い Plane で可 |
| `assets/models/door.glb` | Door | 開閉アニメーション準備（2状態） |
| `assets/models/tank.glb` | Tank | 骨組み構造（empty/half/full は material 切替で対応） |
| `assets/models/mud_mixer.glb` | MudMixer | 中間複雑度 |
| `assets/models/stockpile.glb` | Stockpile | 紫オーラ表現 |
| `assets/models/bridge.glb` | Bridge | Phase 2 で 2D 維持中。Phase 3 で 3D 化 |
| `assets/models/rest_area.glb` | RestArea | シンプル形状 |

**完了条件**:
- [ ] 全 BuildingType の GLB が `assets/models/` に存在する
- [ ] `SectionMaterial` 付きで矢視断面確認が可能
- [ ] LOD1 ポリゴン基準を満たす

**ステータス**: [ ] 未着手

---

### MS-Asset-Terrain: 地形テクスチャ整備

> **依存**: MS-Asset-0（受入基準確定後）
> **ブロック先**: MS-3-4（テレイン3D化）・MS-3-6（テレイン表面表現改善）

**現状**: `grass.png`・`sand.png`・`dirt.png`・`river.png` が存在し `SectionMaterial` のベースカラーとして転用可能。`terrain/` 配下の境界オーバーレイ系は 2D 用のため MS-3-6 で代替される。

**やること**:
1. 既存テクスチャ（`grass.png`・`sand.png`・`dirt.png`）が `SectionMaterial` に設定して違和感がないか確認する
2. 問題があれば 3D 向けにタイリング最適化した版を再生成する
3. `river.png` のアニメーション対応（最低 2 フレーム）を検討する
4. MS-Asset-0 で「壁ノーマルマップあり」と決定した場合に限り法線マップを追加する

**完了条件**:
- [ ] 4 種（草・砂・土・川）が `SectionMaterial` に設定して目視確認できる
- [ ] 境界ブレンドが MS-3-6 で実装できる品質のテクスチャが揃っている

**ステータス**: [ ] 未着手

---

## Phase 3 コードMSとの依存関係

| アセットMS | アンブロックするコードMS | 備考 |
| --- | --- | --- |
| MS-Asset-Shader | MS-3-3 | シェーダーファイルが先に存在すると MS-3-3 の実装コストがほぼゼロ |
| MS-Asset-Char-GLB-A | MS-P3-Pre-D・MS-3-1 | Character GLB がないと Soul 本実装の目視確認ができない |
| MS-Asset-Char-GLB-B | MS-3-Char-A | AnimationGraph 実装の視覚確認に必要 |
| MS-Asset-Char-Face | MS-3-Char-B | Soul の face atlas 状態連動の目視確認に必要 |
| MS-Asset-Terrain | MS-3-4・MS-3-6 | ベーステクスチャなしではテレイン3D化後の見た目確認ができない |
| MS-Asset-Pipeline | MS-Asset-Build-A | GLB なしでは MS-3-5 の本実装ができない |
| MS-Asset-Build-A | MS-3-5 PoC | 壁 4 バリアントが揃ってから Building3dHandles を移行する |
| MS-Asset-Build-B | MS-3-5 完了 | 全 BuildingType の GLB が揃ってから完了とする |
| MS-Asset-0 | MS-3-10 | アウトライン設計計画の前提条件 P0 |

---

## 優先度ガイド

| 優先 | MS | 理由 |
| --- | --- | --- |
| P0 | MS-Asset-Shader | 今すぐ着手可。依存なし。MS-3-3 の先行作業 |
| P0 | MS-Asset-0 | 全アセット制作の品質基準が未定。これがないと作り直しリスク |
| P1 | MS-Asset-Char-GLB-A | Camera角度確定後に最優先。CharacterMaterial PoC の前提 |
| P1 | MS-Asset-Pipeline | GLB 生成の手順が確立しないと壁 GLB が作れない |
| P1 | MS-Asset-Terrain | 既存テクスチャの転用可否確認のみなら低コスト。MS-3-4 の前提 |
| P2 | MS-Asset-Char-GLB-B | Char-GLB-A 完了後。AnimationGraph 実装と並走 |
| P2 | MS-Asset-Char-Face | アートスタイル受入基準確定後。MS-3-Char-B の前提 |
| P2 | MS-Asset-Build-A | MS-3-5 の前提。Phase 3 中盤のゲートになる |
| P3 | MS-Asset-Build-B | MS-Asset-Build-A のパイプライン確立後に連続制作 |

---

## 関連ドキュメント

| ドキュメント | 内容 |
| --- | --- |
| `docs/plans/3d-rtt/phase3-implementation-plan-2026-03-16.md` | Phase 3 コード実装計画 |
| `docs/plans/3d-rtt/milestone-roadmap.md` | Phase 全体の依存グラフ |
| `docs/proposals/3d-rtt/20260316/billboard-camera-angle-proposal-2026-03-16.md` | 壁メッシュ構成・GLB 設計方針・ビルボード仕様 |
| `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md` | 壁 2 層構造・build_progress・WGSL 完全版 |
| `docs/proposals/3d-rtt/20260316/outline-rendering-proposal-2026-03-16.md` | コーナー専用メッシュがアウトライン検出の前提 |
| `docs/proposals/3d-rtt/20260317/character-3d-rendering-proposal-2026-03-16.md` | キャラクター3D化採用提案（CharacterMaterial・AnimationGraph・顔アトラス設計） |
| `docs/world_lore.md` §6.2〜6.3・§8 | アートスタイル仕様・アセットリスト |
| `docs/DEVELOPMENT.md` | 2D スプライト制作パイプライン（generate_image → convert_to_png.py） |
