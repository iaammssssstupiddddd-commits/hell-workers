# 3D-RtT 移行ロードマップ

作成日: 2026-03-15
最終更新: 2026-03-17（キャラクター3D化採用提案 反映）
ステータス: Phase 2 完了（MS-2C 目視検証待ち）/ Phase 3 着手前準備に移行

---

## ビジョン

**最終ゴール**: 地形・建築物・キャラクターをすべて3D空間に配置し、Camera3dの正射影レンダリング結果をRtT（Render-to-Texture）で2D UIと合成する「フルRtT」アーキテクチャへの移行。

**基本方針**: ロジック層（ECS・AI・パスファインディング）は一切変更しない。描画層のみを段階的にすげ替える。

### フェーズ別レンダースコープ

| フェーズ | 2Dに残すもの | 3D/RtTへ移すもの | 備考 |
|---------|-------------|------------------|------|
| Phase 1 | 地形、建築物、キャラクター、UI | テスト用3Dオブジェクトのみ | RtT配線の成立確認 |
| Phase 2 | 地形、2D UI | 壁・建築物の一部、キャラクタープロキシ | ハイブリッドRtTでZバッファ検証 |
| Phase 3 | UI、Familiar、純2Dオーバーレイ | 地形、建築物、Soul | Familiar は前面 2D 表示を維持する |
| Phase 4 | UI、純2Dオーバーレイ | Phase 3の3Dシーン + 多層階表示 | Familiar の所属階可視ルールをここで定義する |

---

## トラック構成

```
並行トラック A ──────────────────────────────────────────── 前提フェーズ
並行トラック B ──────────────────────────────────────────── WFC地形生成（独立）

メインルート:
  Phase 0 (前提)
    └─ Phase 1: RtTインフラ
         └─ Phase 2: ハイブリッドRtT（建築物+キャラクター先行3D化）
              └─ Phase 3 着手前準備（今すぐ着手可）
                   └─ Phase 3: フルRtT（地形を含むインゲーム要素の3D化）
                        └─ Phase 4: 多層階（将来構想）
```

---

## 前提フェーズ（独立実施可）

### MS-Pre-A: obstacles差分更新

> **依存**: なし

- **現況**: `world_map.register_completed_building_footprint(...)` と `clear_building_occupancy(...)` 系でセル単位の差分更新になっている
- **やること**: 全走査ベースの obstacle 再構築が再導入されていないことを確認する
- **完了条件**: `building_completion/world_update.rs` と `hw_world::WorldMap` に全走査ベースの obstacle 再構築が存在しない
- **ステータス**: [x] 現行コードで成立済み（2026-03-15確認）

---

### MS-Pre-B: Building親子構造化 + Zスロット定義

> **依存**: なし

- **やること**:
  1. `crates/hw_core/src/constants/render.rs` に Zスロット定数を追加（`Z_BUILDING_FLOOR=0.05` 〜 `Z_BUILDING_LIGHT=0.18`）
  2. `building_completion/spawn.rs` を Building親エンティティ（ロジック）+ VisualLayer子エンティティ（Sprite）に分割
  3. `VisualLayerKind` コンポーネントを `crates/hw_visual/src/` に追加
  4. 建築物ルートの `Sprite` を直接更新している既存システム（例: `hw_visual::wall_connection`, `hw_visual::tank`）を VisualLayer 子エンティティ参照へ追従させる
- **完了条件**: 既存の見た目が変わらない。建築物ルートから `Sprite` を外しても既存 visual system が破綻しない。`cargo check` 通過
- **ステータス**: [x] 完了（2026-03-15）

---

## Phase 1: RtTインフラ

> **依存**: MS-Pre-A/B は不要（独立して開始可能）

### MS-1A: Bevy `3d` フィーチャー有効化

- **やること**: `Cargo.toml` に `"3d"` フィーチャーを追加し `cargo check` を通す
- **精査済み事項**: Bevy 0.18 において `"3d"` フィーチャーは `bevy_pbr`, `bevy_core_pipeline`, `bevy_render` を包含しており、RtT に必要な 3D パイプラインがすべて有効化される
- **完了条件**: コンパイルエラーゼロ
- **ステータス**: [x] 完了（2026-03-15）

---

### MS-1B: Camera3d + RenderTarget セットアップ

> **依存**: MS-1A

- **やること**:
  - `plugins/startup/mod.rs` に Camera3d（正射影）+ `RenderTarget::Image` を追加
  - `crates/hw_core/src/constants/render.rs` に `LAYER_2D = 0`, `LAYER_3D = 1` 定数追加
  - オフスクリーンテクスチャ（`Handle<Image>`）を `assets.rs` で管理
- **Bevy 0.18 API（実装済み確認）**:
  - `RenderTarget::Image(handle.into())` で `Handle<Image>` → `ImageRenderTarget` への変換が可能（`From` 実装あり）
  - `Image::new_target_texture(w, h, format, view_format)` を使うと `TextureUsages` を手動設定不要
  - Camera3d の向きは `looking_at(Vec3::ZERO, Vec3::NEG_Z)` が必須（`Vec3::Z` にすると画面右が World -X に反転する）
  - Camera2d の子エンティティとして合成スプライトを spawn することでパン・ズームに自動追従させられる
- **完了条件**: オフスクリーンテクスチャへのレンダリングが確認できる
- **ステータス**: [x] 完了（2026-03-15）

---

### MS-1C: Camera2d ↔ Camera3d 同期システム

> **依存**: MS-1B

- **やること**: 毎フレーム Camera2d の Transform/OrthographicProjection を Camera3d に同期するシステムを追加
  - パン: `Camera2d.Transform.translation.xy` → Camera3d の XZ 軸にマッピング
  - ズーム: `PanCamera` が更新する `transform.scale` を Camera3d の `OrthographicProjection.scale` に反映する
- **Bevy 0.18 API（実装済み確認）**:
  - `PanCamera` (0.18) は `zoom_factor` を `transform.scale = Vec3::splat(zoom_factor)` で直接反映する（`bevy_camera_controller-0.18.0/src/pan_camera.rs:236` で確認済み）
  - Camera3d 同期式: `cam3d.translation.x = cam2d.translation.x`、`cam3d.translation.z = -cam2d.translation.y`（符号反転必須）
  - Y符号反転の理由: Camera3d up=NEG_Z のため、2D の +Y が 3D の -Z に対応する
- **完了条件**: パン・ズーム操作時に既存の2Dビューが壊れない
- **ステータス**: [x] 完了（2026-03-15）

---

### MS-1D: RtTテクスチャのCamera2d合成

> **依存**: MS-1C

- **やること**:
  - MS-1Bで生成したテクスチャをフルスクリーン Sprite として Camera2d の適切なZ位置に配置
  - Camera3d の `clear_color` を `ClearColorConfig::Custom(Color::srgba(0., 0., 0., 0.))` に設定する（Bevy 0.18 に `Color::NONE` は存在しない）
  - テスト用3Dオブジェクトを Layer 1 に配置して合成確認
- **完了条件（継続可否ゲート）**:
  - テスト立方体がトップダウンビューで正しい位置に表示される
  - 建築物のない部分はテレインが透過して見える
  - ⚠️ **フルRtT継続可否判断**: パフォーマンス・合成品質・実装コストを評価し継続可否を明示的に判断する
- **ステータス**: [x] 完了（2026-03-15）

---

## Phase 2: ハイブリッドRtT（建築物+キャラクター先行3D化）

> **依存**: MS-1D完了 + MS-Pre-B完了

### MS-2A: 壁セグメントの3D配置

- **やること**:
  - 壁の VisualLayer 子エンティティを Sprite → Bevy組込みシェイプ（`Cuboid` メッシュ等）に置き換え
  - `RenderLayers::layer(1)` を付与してCamera3d側で描画
  - 16+バリアントのスプライト切替ロジックを廃止し、3Dモデルの動的配置で代替
- **完了条件**: トップダウンで壁の見た目が正しい、`cargo check` 通過、旧スプライト切替ロジック（16+バリアント）が削除されている
- **ステータス**: [x] 完了（2026-03-15）
  - `Building3dHandles` リソースを `visual_handles.rs` で初期化（wall/floor/door/equipment/character メッシュ）
  - `hw_visual/src/visual3d.rs` に `Building3dVisual` / `SoulProxy3d` / `FamiliarProxy3d` コンポーネント追加
  - `building_completion/spawn.rs`: Wall など全BuildingType（Bridge除く）を独立3Dエンティティとして spawn
  - `wall_connection.rs` の完成Building向け子エンティティ Sprite 更新ブロック削除（Blueprint向けは保持）
  - `building3d_cleanup.rs`: Building削除時クリーンアップ + 仮設→本設マテリアル遷移システム

---

### MS-2B: Zソート問題の検証 / キャラクタープロキシ

> **依存**: MS-2A

- **やること**:
  - Soul / Familiar を最小3Dプロキシで RtT レイヤーへ移す
  - 少なくとも「壁の背後を歩くキャラクター」「壁際で作業するキャラクター」の2ケースを再現する
- **完了条件**: キャラクターを含む重なりがハードウェアZバッファで自然に解決される
- **ステータス**: [x] 完了（2026-03-15）
  - `SoulProxy3d` / `FamiliarProxy3d` コンポーネント定義、soul/familiar spawn に3Dプロキシ追加
  - `character_proxy_3d.rs`: 毎フレーム2D Transform → 3D XZ 同期、削除時クリーンアップ

---

### MS-2C: ハイブリッド段階の前後関係検証

> **依存**: MS-2B

- **やること**: 壁・アイテム・キャラクターが重なる状況を再現し、Zソート破綻がないか検証する
- **完了条件**: Phase 2 の対象物では Z値管理コードを追加せずに前後関係が正しく描画される
- **ステータス**: [x]　完了

---

### MS-2D: 床・ドア・家具の3D化

> **依存**: MS-2C

- **やること**: BuildingType ごとに順次 VisualLayer を3D化
  - `Floor` → 平面メッシュ（`Plane3d`）
  - `Door` → Cuboid + 開閉アニメーション準備
  - `Tank` / `MudMixer` → Cuboid 仮モデル
- **完了条件**: Phase 2 終了時点で「地形以外の主要インゲーム要素をRtTへ移す前提」が成立している
- **ステータス**: [x] 完了（2026-03-15）
  - Floor/Door/SandPile/BonePile/WheelbarrowParking/Tank/MudMixer/RestArea すべて3D化（Cuboid/Plane3dプレースホルダー）
  - Bridge のみ2Dスプライト維持（Phase 3 対象）

---

### MS-Elev: 矢視モード（4方向切替）

> **依存**: MS-2A

- **やること**: V キーで TopDown / 北 / 東 / 南 / 西 をサイクル切替
- **完了条件**: 矢視中にカメラが正しい方向を向き、パンが追従する
- **ステータス**: [x] 完了（2026-03-15）
  - `ElevationViewState` リソース + `elevation_view.rs`（Vキー入力 + Camera3d プリセット切替）
  - `camera_sync.rs` 修正: 矢視中は XZ 平行移動のみ同期、回転・Y 高度は保持

---

## Phase 3 着手前準備（今すぐ着手可・M-Gate と並走可）

> **依存**: Phase 2 実装完了（目視検証 MS-2C は並走可）
> **詳細**: `docs/plans/3d-rtt/phase3-implementation-plan-2026-03-16.md`

---

### MS-P3-Pre-A: `WgpuFeatures::CLIP_DISTANCES` 動作確認

> **依存**: なし
> **重要度**: ⚠️ P0 ブロッカー。失敗した場合は SectionMaterial の技術設計を全面再検討

- **やること**: `bevy_app` の `RenderPlugin` 設定に `WgpuFeatures::CLIP_DISTANCES` を追加し、`cargo check` + 実機起動で動作確認する
- **変更ファイル**: `crates/bevy_app/src/main.rs`（または `hw_visual/src/lib.rs`）
- **完了条件**:
  - [x] `cargo check` ゼロエラー
  - [x] ゲームが正常起動する（クラッシュしない）
- **ステータス**: [x] 完了

---

### MS-P3-Pre-B: RtT基盤整備

> **依存**: なし（MS-P3-Pre-A と並走可）

- **やること**:
  1. `rtt_setup.rs` の `create_rtt_texture` 関数を切り出す
  2. `sync_rtt_texture_size_to_window` と `sync_rtt_output_bindings` を実装・登録する
  3. `RenderTarget::Image` の受け取り型を実装前に確認する（`docsrs-mcp` / `~/.cargo/registry/src/`）
- **変更ファイル**: `plugins/startup/rtt_setup.rs`、`systems/visual/rtt_composite.rs`（新規）、`hw_core/src/constants/render.rs`
- **完了条件**:
  - [x] `cargo check` ゼロエラー
  - [x] `RttTextures.texture_3d` を手動差し替えしたとき合成スプライトのサイズが自動追従する（目視）
- **ステータス**: [x] 完了

---

### MS-P3-Pre-C: Camera3d 角度 V-1（目視確認・数値確定）

> **依存**: Phase 2 の Cuboid プリミティブが配置されていること（MS-2A 完了）
> **重要度**: P0。角度未確定のまま Phase 3 の GLB 生成パイプラインを進められない

- **やること**: Camera3d の Y 座標・Z オフセットを変えながら目視確認し、`world_lore.md` §6.2 のアートスタイル基準で判断する
- **確認基準**:
  - 壁に厚みが感じられる・床と壁の境界が自然
  - キャラクタープロキシ（Cuboid または仮GLB）が「体積のない存在に見える」（`character-3d-rendering-proposal` §3.6 拡張）
- **確定する値**: `VIEW_HEIGHT`（Camera3d の Y 座標）・`Z_OFFSET`（Z オフセット）・仰角（度数）
- **完了条件**:
  - [x] 数値が確定し `hw_core/src/constants/render.rs` に記録されている
  - [x] キャラクタープロキシの見え方について「体積のない存在として許容できる」判断が記録されている
- **ステータス**: [x] 完了

---

### MS-P3-Pre-D: Character GLB PoC（face atlas 表示確認）

> **依存**: MS-P3-Pre-C（角度数値確定後）・MS-Asset-Char-GLB-A（Soul GLB 配置済み）

- **やること**:
  - `GameAssets` に `soul.glb#Scene0` を追加し、Soul spawn を `SceneRoot` ベースで RtT に接続する
  - `SceneInstanceReady` で Soul GLB 子孫へ `RenderLayers::layer(LAYER_3D)` を付与する
  - `CharacterHandles` リソースを定義し Soul GLB を仮スポーンして `mesh_face` に `soul_face_atlas.png` の先頭セルだけを切り出す最小 `StandardMaterial` を適用する
  - 斜め Camera3d で Soul GLB が建物 Cuboid と正しく前後表示されることを確認する（Z バッファ共有の確認）
  - `mesh_face` は GLB 既定姿勢を維持したまま、通常表情の face atlas 1 セルが十分視認できることを確認する
- **確認基準**:
  - [x] 壁の後ろに入った Soul GLB が壁に隠れる（Z バッファが RtT に焼き込まれていることを確認）
  - [x] Soul GLB が「体積のない存在に見える」アートスタイル感が出ている
  - [x] `mesh_face` に通常表情の face atlas 1 セルが十分視認できる
- **ステータス**: [x] 完了（Soul `SceneRoot`・`mesh_face` atlas 表示・前後関係確認まで完了）

---

## Phase 3: フルRtT（地形を含むインゲーム要素の3D化）

> **依存**: Phase 2 完了（MS-2C 含む）・Phase 3 着手前準備完了
> **詳細**: `docs/plans/3d-rtt/phase3-implementation-plan-2026-03-16.md`

---

### MS-3-1: Soul CharacterMaterial 本実装 + 2D Sprite置換

> **依存**: MS-P3-Pre-C・MS-P3-Pre-D 完了

- **やること**:
  1. `hw_visual/src/material/character_material.rs` を本実装する
  2. `assets/shaders/character_material.wgsl` を新規作成し Soul `mesh_body` / `mesh_face` を custom material へ移行する
  3. `CharacterHandles` を Soul 本実装向けの構成へ整理し、PoC 用 `StandardMaterial` 差し替えを置き換える
  4. Camera2d 側の Soul Sprite を削除し、通常表示を Soul GLB 側へ一本化する
  5. Familiar は現行 billboard / 2D 経路を維持する
  6. Soul 専用 mask RtT と最終合成 Material を追加し、画面上のシルエット丸めを後段で扱えるようにする
- **変更ファイル**: `hw_visual/src/material/character_material.rs`（新規）、`hw_visual/src/material/soul_mask_material.rs`（新規）、`hw_visual/src/visual_handles.rs`、`assets/shaders/character_material.wgsl`（新規）、`assets/shaders/soul_mask_material.wgsl`（新規）、`assets/shaders/rtt_composite_material.wgsl`（新規）、`plugins/startup/rtt_setup.rs`、`plugins/startup/rtt_composite.rs`、Soul の Camera2d Sprite spawn 箇所
- **完了条件**:
  - [x] `cargo check` ゼロエラー
  - [x] 通常ビューで Soul GLB が 2.5D 的に見える
  - [x] Soul の `mesh_body` / `mesh_face` が `CharacterMaterial` 経路で描画される
  - [x] Camera2d 側に Soul の Sprite が残っていない
  - [x] Familiar の表示挙動は現状から退行していない
  - [x] Soul のシルエット丸めが通常ビューで自然に見える
- **ステータス**: [x] 完了（body/face custom material・2D Sprite 置換・Soul mask prepass・body 不透明化まで完了）

---

### MS-3-Char-A: AnimationGraph + SoulAnimState 実装

> **依存**: MS-3-1 完了・MS-Asset-Char-GLB-B 完了（P0 クリップ）

- **やること**:
  1. `SoulAnimationLibrary` を追加し `Gltf.named_animations` から clip handle を名前解決する
  2. `SoulAnimVisualState`（body / face 分離）を導入する
  3. `AnimationGraph` + `AnimationTransitions` で Idle / Walk / WalkLeft / WalkRight を切り替える
  4. `mesh_face` を per-instance material 化し Soul 単位で `face_uv_offset` を更新できるようにする
  5. `IdleState` / 疲労 / 会話表情イベント / タスク状態から `SoulFaceState` を更新する
- **変更ファイル**: `crates/bevy_app/src/systems/visual/soul_animation.rs`（新規）、`crates/bevy_app/src/assets.rs`、`crates/bevy_app/src/plugins/startup/asset_catalog.rs`、`crates/bevy_app/src/systems/visual/character_proxy_3d.rs`、`crates/hw_visual/src/visual3d.rs`、`crates/hw_visual/src/material/character_material.rs`
- **完了条件**:
  - [x] `cargo check` ゼロエラー
  - [x] Idle / Walk がタスク状態に連動して切り替わる（目視）
  - [x] 表情が SoulAnimState に連動して切り替わる（目視）
- **ステータス**: [x] 完了（clip registry・per-instance face material・AnimationPlayer binding・WalkLeft/WalkRight 切替・face atlas 状態連動確認まで完了）

---

### MS-3-Char-B: Soul P1 クリップ + face atlas 状態連動

> **依存**: MS-3-Char-A 完了・MS-Asset-Char-Face 完了

- **やること**:
  1. Soul の P1 クリップ（Work・Carry）をタスク状態に接続する
  2. `CharacterMaterial.face_uv_offset` に顔テクスチャアトラス（MS-Asset-Char-Face）を統合する
  3. `Fear` / `Exhausted` を含む表情切り替え規則を `SoulAnimState` と同期する
- **変更ファイル**: `crates/bevy_app/src/systems/soul_ai/` 以下、`hw_visual/src/anim/soul_anim.rs`、`hw_visual/src/material/character_material.rs`
- **完了条件**:
  - [x] `cargo check` ゼロエラー
  - [x] Soul の全 P1 クリップがタスク状態に連動する（目視）
  - [x] 顔テクスチャアトラスによる表情切り替えが動作する（目視）
- **ステータス**: [x] 完了（Work / Carry / Fear / Exhausted の写像確認、face atlas 連動確認、breakdown / exhausted の body-face 役割分離まで完了）

---

### MS-3-Fam-R: Familiar 表示方式決定

> **依存**: MS-3-1 完了

- **やること**:
  1. Familiar は Phase 3 では 2D 前面表示を本表示として維持する
  2. Familiar に shadow caster / shadow proxy は持たせず、影は仕様として扱わない
  3. 多層階での所属階可視ルールは別マイルストーンへ分離する
- **完了条件**:
  - [x] Familiar の表示方式が決定されている
  - [x] 決定内容が roadmap / implementation plan に反映されている
- **ステータス**: [x] 完了（Phase 3 は 2D 前面表示・影なし、多層階の所属階可視ルールは後段へ分離）

---

### MS-3-2: RtT WindowResized + 品質スケール

> **依存**: MS-P3-Pre-B 完了・`QualitySettings` リソース存在

- **やること**:
  1. `systems/visual/rtt_resize.rs` を新規作成（`on_window_resized`・`on_quality_changed`・共通ヘルパー `recreate_rtt`）
  2. `hw_core/src/quality.rs` に `rtt_scale()` メソッドを追加（高=1.0・中=0.75・低=0.5）
- **変更ファイル**: `systems/visual/rtt_resize.rs`（新規）、`hw_core/src/quality.rs`
- **完了条件**:
  - [ ] `cargo check` ゼロエラー
  - [ ] ウィンドウリサイズ時に建物・キャラクターの描画が追従する（目視）
  - [ ] 品質設定変更時に RtT 解像度が変わる（目視）
- **ステータス**: [ ] 未着手

---

### MS-3-3: SectionMaterial 基盤（MS-Section-A / 将来実装）

> **依存**: MS-P3-Pre-A 完了（`WgpuFeatures::CLIP_DISTANCES` 確認済みであること）

- **やること**:
  1. `hw_visual/src/material/section_material.rs` を新規作成（`SectionMaterial`・`SectionCut`）
  2. `assets/shaders/section_material.wgsl` を新規作成（クリップ平面 + 施工進捗クリップの完全版 WGSL）
  3. `MaterialPlugin::<SectionMaterial>` を `HwVisualPlugin` に追加
  4. `sync_section_cut_normal` システムを実装・登録
  5. `sync_section_cut_to_materials` システムを実装・登録
- **変更ファイル**: `hw_visual/src/material/section_material.rs`（新規）、`assets/shaders/section_material.wgsl`（新規）、`hw_visual/src/lib.rs`、`systems/visual/camera_sync.rs`
- **完了条件**:
  - [ ] `cargo check` ゼロエラー
  - [ ] `SectionCut.active = true` のとき Cuboid がスラブ外でクリップされる（目視）
  - [ ] `SectionCut.active = false` のとき全体が正常描画される（目視）
- **ステータス**: [ ] 将来実装

---

### MS-3-4: テレインの3D化（旧 MS-3A）

> **依存**: MS-3-3 完了

- **やること**:
  - 既存の地形タイル描画を 3D メッシュ / `SectionMaterial` ベースへ置き換える
  - `terrain_border.rs` / `borders.rs` に依存しない地形表現へ移行する
  - Phase 3 完了時点で、インゲーム要素の描画は Camera3d → RtT のみで成立させる
- **補足**: 見た目改善としてのブレンド表現は 3D マテリアル側で行う。`Material2d` への置換だけではフルRtT到達とはみなさない
- **完了条件**:
  - [ ] `cargo check` ゼロエラー
  - [ ] 地形が Camera3d → RtT のみで描画される
  - [ ] Camera2d 側にインゲーム地形描画が残っていない
- **ステータス**: [ ] 未着手

---

### MS-3-5: Building3dHandles の SectionMaterial 移行（MS-Section-B）

> **依存**: MS-3-3 完了・Phase 3 GLB 取込完了

- **やること**:
  1. `visual_handles.rs` の `Building3dHandles` を `SectionMaterial` ベースに変更
  2. `building_completion/spawn.rs` の全 `MeshMaterial3d<StandardMaterial>` を `MeshMaterial3d<SectionMaterial>` に置き換え
  3. 設備別 visual system（`tank.rs`・`mud_mixer.rs` 等）の同様置き換え
- **変更ファイル**: `hw_visual/src/visual_handles.rs`、`building_completion/spawn.rs`、`systems/visual/tank.rs`、`systems/visual/mud_mixer.rs`
- **完了条件**:
  - [ ] `cargo check` ゼロエラー
  - [ ] 矢視モードで切断線設定時に全 BuildingType のスラブ外部分がクリップされる（目視）
  - [ ] トップダウンモードで全建物が正常表示される
- **ステータス**: [ ] 未着手

---

### MS-3-6: テレイン表面表現改善（旧 MS-3B）

> **依存**: MS-3-4 完了

- **やること**: テクスチャブレンド・ノイズによる遷移境界の有機化・必要なら生成時ベイクの検証
- **完了条件**:
  - [ ] 90度ベースの地形境界オーバーレイに依存しない見た目が成立する
- **ステータス**: [ ] 未着手

---

### MS-3-7: マウスヒットテストの Raycasting 化（旧 MS-3C）

> **依存**: MS-3-4 完了

- **やること**:
  - 現在の `viewport_to_world_2d` を Camera3d からの Raycasting に全面置換する
  - `hw_ui`・`bevy_app`・`hw_visual` に散在する `viewport_to_world_2d` 利用箇所を共有ヘルパー経由の Raycast 判定へ寄せる
  - クリック・ホバー・範囲選択・配置プレビューの各入力モードを個別に検証する
- **完了条件**:
  - [ ] インゲーム入力で `viewport_to_world_2d` への依存が残らない
  - [ ] クリック・ホバー・ドラッグ操作が 3D ビューで正しく動作する
- **ステータス**: [ ] 未着手

---

### MS-3-8: 2D スプライトインフラの段階的廃止（旧 MS-3D）

> **依存**: MS-3-7 完了

- **やること**: Phase 2〜3で3D化済みのインゲーム要素から 2D Sprite コンポーネントと関連Z定数を順次削除し、Camera2d を UI 専用へ絞る
- **完了条件**:
  - [ ] Camera2d 側に残るのは UI と純2Dオーバーレイのみ
  - [ ] `cargo check` ゼロエラー
- **ステータス**: [ ] 未着手

---

### MS-3-9: 切断線 UI（MS-Section-C）

> **依存**: MS-3-5 完了

- **やること**:
  1. `SectionCutEditSession` コンポーネントを `hw_ui` に定義
  2. 矢視モード入時に切断線配置モードを自動起動する処理を `camera_sync.rs` に追加
  3. ワールドマップ上のクリック・ドラッグで `SectionCut.position` を更新する入力システムを実装
  4. スラブ厚みスライダーを `bevy_egui` UI に追加
  5. 切断線のワールドマップ上プレビュー（2D Gizmo）を実装
- **変更ファイル**: `hw_ui/src/section_cut_ui.rs`（新規）、`systems/visual/camera_sync.rs`
- **完了条件**:
  - [ ] `cargo check` ゼロエラー
  - [ ] クリック・ドラッグで切断線を配置できる
  - [ ] スラブ厚みスライダーを動かすと即座に 3D 描画が変化する
- **ステータス**: [ ] 未着手

---

### MS-3-10: アートスタイル受入基準確定（→ アウトライン生成計画へ）

> **依存**: Phase 3 GLB 取込 PoC 完了

- **やること**:
  - アウトラインの受入基準を文書化する（線幅・揺らぎ量・色・ズームアウト無効化閾値）
  - 壁メッシュ方法A（コーナー専用メッシュ）の GLB 仕様を確定する
  - 断面キャップの実装方針（A / B / C）を確定する
  - アウトライン生成計画を別計画として起票する
- **完了条件**:
  - [ ] アートスタイル受入基準がドキュメント化されている
  - [ ] アウトライン生成の実装計画が別計画として起票されている
- **ステータス**: [ ] 未着手

---

## 並行トラックB: WFC地形生成

> **依存**: なし（`hw_world/src/mapgen.rs` のみ影響）
> **元計画**: `docs/plans/3d-rtt/related/wfc-terrain-generation-plan-2026-03-12.md`

| MS | 内容 | ステータス |
|----|------|-----------|
| MS-WFC-1 | `TerrainType::can_be_adjacent()` / `can_be_diagonal()` 実装 | [ ] 未着手 |
| MS-WFC-2 | `bevy_procedural_tilemaps` 導入 + 基本WFC生成（**要: Bevy 0.18 対応確認**） | [ ] 未着手 |
| MS-WFC-3 | 川・砂バッファゾーンの固定制約統合 | [ ] 未着手 |
| MS-WFC-3.5 | コーナー制約検証システム（`#[cfg(debug_assertions)]`） | [ ] 未着手 |

---

## Phase 4: 多層階（将来構想）

> **依存**: Phase 3完了

- `TileIndex(x, y, z)` への座標型拡張（ロジック層）
- パスファインディングの多層階ネットワーク（階段ポータル）対応
- Floor ID ごとに3Dメッシュを表示/非表示切替

*詳細設計は Phase 3完了後に策定する。*

---

## 依存グラフ

```
MS-Pre-A ─────────────────────────────────────────────── (成立済み)
MS-Pre-B ──────────────────────────────────┐
                                           │
MS-1A → MS-1B → MS-1C → MS-1D ────────────┤
                                     MS-2A─┘→ MS-2B → MS-2C → MS-2D
                                     MS-Elev (MS-2A完了で最終完了)
                                                          │
                              ┌──────────────────────────┘
                              │
                Phase 3 着手前準備（Phase 2 実装完了後・並走可）
                  MS-P3-Pre-A (CLIP_DISTANCES)  ─────────────────────────┐
                  MS-P3-Pre-B (RtT基盤整備)     ──────────────────────┐  │
                  MS-P3-Pre-C (Camera角度V-1)   ──┐                  │  │
                  MS-P3-Pre-D (Character GLB PoC) ←─Pre-C+Char-GLB-A │  │
                              │                                      │  │
                              ↓ Phase 3 本実装                       │  │
                  MS-3-1 (Soul CharacterMaterial) ←─────Pre-C,D       │  │
                       │                                             │  │
                  MS-3-Char-A (AnimationGraph+SoulAnimState) ←─3-1+GLB-B│
                  MS-3-Char-B (Soul P1+face atlas) ←────── 3-Char-A+Face│
                  MS-3-Fam-R (Familiar表示決定)  ←─────── 3-1          │
                  MS-3-2 (RtT WindowResized)   ←─Pre-B              │  │
                  MS-3-3 (SectionMaterial基盤) ←─Pre-A              ↓  │
                       │                                            │  │
                  MS-3-4 (テレイン3D化)  ←──────────────────────── ┘  │
                  MS-3-5 (SectionMaterial移行) ←── MS-3-3 + GLB         │
                       │                                               │
                  MS-3-6 (テレイン表面) ←── MS-3-4                     │
                  MS-3-7 (Raycasting)  ←── MS-3-4                     │
                  MS-3-8 (2D廃止)      ←── MS-3-7 + MS-3-Char-B + Fam-R│
                  MS-3-9 (切断線UI)    ←── MS-3-5                     │
                  MS-3-10 (アウトライン計画) ←── GLB PoC               │
                              │                                       │
                         Phase 4 ──────────────────────────────────┘

MS-WFC-1 → MS-WFC-2 → MS-WFC-3 → MS-WFC-3.5  (独立)
```

---

## 優先度ガイド

| 優先 | MS | 理由 |
|------|------|------|
| ⚠️ P0（ブロッカー確認） | MS-P3-Pre-A | `CLIP_DISTANCES` 非対応なら SectionMaterial 設計を全面再検討 |
| P0（データ確定） | MS-P3-Pre-C | Camera3d 角度未確定のまま GLB 生成パイプラインを進められない |
| P1（基盤整備） | MS-P3-Pre-B | Phase 3 参照箇所が増える前に一元化しておく必要がある |
| P1（PoC） | MS-P3-Pre-D | Character GLB + face atlas 表示確認。MS-3-1 の前提 |
| P1（本実装） | MS-3-2 | RtT 解像度と品質スケール整備。現行 Phase 3 の次タスク |
| P4（将来実装） | MS-3-3 | SectionMaterial / section clip は将来フェーズへ延期 |
| P2（キャラクター） | MS-3-Char-A | M-3-1 完了後の次タスク |
| P2（キャラクター） | MS-3-Char-A | AnimationGraph + タスク連動。MS-3-1 完了後すぐ着手 |
| P2（キャラクター） | MS-3-Char-B | Soul の P1 クリップ接続 + face atlas 状態連動。MS-3-Char-A 完了後 |
| P3（方針確定） | MS-3-Fam-R | Familiar を Phase 3 では 2D 前面表示・影なしで扱い、多層階の可視ルールを後段へ送る |
| P2（本実装） | MS-3-4〜MS-3-9 | Phase 3 中盤〜後半の順次実装 |
| 独立 | MS-WFC-1〜3 | メインルートとは独立。地形改善を先行させることも可 |

---

## 関連ドキュメント

| ドキュメント | 内容 |
|------------|------|
| `docs/plans/3d-rtt/phase3-implementation-plan-2026-03-16.md` | Phase 3 詳細実装計画（各MSの変更ファイル・完了条件） |
| `docs/plans/3d-rtt/asset-milestones-2026-03-17.md` | アセット制作マイルストーン（スプライト・GLB・シェーダー・テクスチャ） |
| `docs/proposals/3d-rtt/20260317/character-3d-rendering-proposal-2026-03-16.md` | キャラクター3D化採用提案（CharacterMaterial・AnimationGraph・顔アトラス・CurtainMaterial） |
| `docs/proposals/3d-rtt/20260316/billboard-camera-angle-proposal-2026-03-16.md` | Camera3d 斜め角度採用提案（ビルボード記述はキャラクター提案書で上書き） |
| `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md` | SectionMaterial・セクションカット採用提案 |
| `docs/proposals/3d-rtt/20260316/rtt-resolution-scaling-proposal-2026-03-16.md` | RtT 解像度スケーリング設計提案 |
| `docs/proposals/3d-rtt/20260316/outline-rendering-proposal-2026-03-16.md` | アウトライン生成設計方針（実装保留・前提条件整理） |
| `docs/proposals/3d-rtt/3d-rendering-rtt-proposal-2026-03-14.md` | ハイブリッドRtT提案（Phase 1〜4詳細） |
| `docs/proposals/3d-rtt/3d-rendering-rtt-proposal-phase2-2026-03-14.md` | フルRtT・多層階アーキテクチャ方針 |
| `docs/proposals/3d-rtt/related/building-visual-layer-plan-2026-03-12.md` | MS-Pre-B詳細設計 |
| `docs/proposals/3d-rtt/related/spatial-grid-architecture-plan-2026-03-12.md` | MS-Pre-A詳細設計 |
| `docs/proposals/3d-rtt/related/wfc-terrain-generation-plan-2026-03-12.md` | WFCトラック詳細設計 |
