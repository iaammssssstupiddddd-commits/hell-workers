# Phase 3 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `phase3-implementation-plan-2026-03-16` |
| ステータス | `Draft` |
| 作成日 | `2026-03-16` |
| 最終更新日 | `2026-03-17` |
| 作成者 | Claude Sonnet 4.6 |
| 関連ロードマップ | `docs/plans/3d-rtt/milestone-roadmap.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260317/character-3d-rendering-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/billboard-camera-angle-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/rtt-resolution-scaling-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/outline-rendering-proposal-2026-03-16.md` |

---

## 1. 目的

### 解決したい課題

Phase 2（ハイブリッドRtT）完了後、以下のギャップが残っている。

- Camera3d が真上（90°）固定のため3Dメッシュが全て真上から見え、2.5D表現が成立しない
- RtT テクスチャが 1280×720 固定でウィンドウリサイズ非対応
- 建築物が `StandardMaterial` + Cuboid プレースホルダーのまま
- 矢視時に断面切断が実装されておらず床タイルが全列描画される
- テレインが Camera2d 依存のまま

### 到達したい状態

- 地形を含む全インゲーム要素が Camera3d → RtT に移り、Camera2d には UI だけが残る
- 矢視（セクションビュー）で GLB メッシュを任意の断面で切断できる
- ウィンドウリサイズ時に RtT テクスチャが自動追従する
- キャラクターが GLB モデル + ボーンアニメーションで Camera3d にレンダリングされる（スプライト・ビルボード廃止）

### 成功指標

- `cargo check` ゼロエラー
- Camera2d 側のインゲーム描画がゼロ
- GTX 1650 相当で 60fps 維持
- 矢視モードで切断線によるスラブクリップが動作する

---

## 2. スコープ

### 対象（In Scope）

- Phase 3 着手前の基盤整備（`WgpuFeatures::CLIP_DISTANCES` 確認・RtT リサイズ基盤・Camera3d 角度確定）
- Camera3d の斜め角度（約53°）適用
- `CharacterMaterial` 実装・`CharacterHandles` リソース定義・Soul GLB の本実装
- `SectionMaterial` 実装・`Building3dHandles` の `SectionMaterial` 移行
- AnimationGraph + `SoulAnimState` 実装・タスク状態連動（MS-3-Char-A）
- Soul の P1 クリップ接続・顔アトラス状態連動（MS-3-Char-B）
- Familiar 表示方式の再検討（MS-3-Fam-R）
- RtT テクスチャのウィンドウリサイズ追従・品質スケール係数
- テレインの3D化（MS-3A）・テレイン表面表現改善（MS-3B）
- マウスヒットテストの Raycasting 化（MS-3C）
- 2D スプライトインフラの段階的廃止（MS-3D）
- 切断線 UI（MS-Section-C）

### 非対象（Out of Scope）

- アウトライン生成の実装（Phase 3 PoC 完了・アートスタイル受入基準確定後に別計画として起票）
- Familiar の GLB 化実装本体（MS-3-Fam-R で価値を再評価するまで保留）
- Phase 4 多層階
- WFC 地形生成（独立トラック）
- HiDPI 対応（P2・Phase 3 序盤の本実装時に判断）
- アセット生成パイプライン（TRELLIS.2 / TripoSR）の詳細設計

---

## 3. 現状とギャップ

| 項目 | 現状 | ギャップ |
| --- | --- | --- |
| Camera3d 角度 | `VIEW_HEIGHT = 150.0` / `Z_OFFSET = 90.0` で斜め TopDown 適用済み | ギャップ解消済み。以後の課題はキャラクター描画本実装 |
| CharacterMaterial | Soul 用の最小 custom material を実装済み | SectionCut 共有・face atlas 状態連動・AnimationGraph 連動は未実装 |
| RtT テクスチャ | 1280×720 固定 | ウィンドウリサイズで表示崩壊 |
| マテリアル | `StandardMaterial` + Cuboid | 矢視断面切断が不可能 |
| セクションカット | 未実装 | 矢視時に床タイル全列が描画される |
| テレイン | Camera2d 依存 | フルRtT未達 |
| `CLIP_DISTANCES` | 動作未確認 | SectionMaterial 全体がブロックされる |

---

## 4. 実装方針

- Camera3d を斜め約53°に変更し、建物の側面が自然に見える2.5D表現を実現する
- キャラクターは GLB モデル + ボーンアニメーション（Bevy AnimationGraph）で Camera3d に直接レンダリングする（`character-3d-rendering-proposal-2026-03-16` 採用）
- キャラクターには `CharacterMaterial`（`AlphaMode::Blend`）を独立定義する。Section 連動は MS-3-3 以降へ後ろ倒しし、MS-3-1 ではキャラクター単体の表示品質を優先する
- `SoulProxy3d` は Soul 本実装の間は暫定ルートとして維持し、`FamiliarProxy3d` は表示方式再検討（MS-3-Fam-R）完了まで維持する
- 全建築物は Phase 3 着手時点から `StandardMaterial` ではなく `SectionMaterial` を使う
- RtT テクスチャ管理は `create_rtt_texture` 関数を中心に一元化する
- アウトライン生成は前提条件（アートスタイル受入基準・壁メッシュ確定・断面キャップ方針）が揃うまで設計しない

### Bevy 0.18 注意点

- `clip_distances` 使用には `WgpuFeatures::CLIP_DISTANCES` のデバイス有効化が必須（`specialize()` だけでは不十分）
- `RenderTarget::Image(handle.into())` の型は `Handle<Image>` → `ImageRenderTarget` の `From` 実装で対応済み（MS-1B で確認済み）
- `mesh_face` ビルボード（`face_billboard_system`）は子エンティティのローカルトランスフォームで動作する。親（body）の `GlobalTransform` から逆回転を適用してローカル回転に変換すること（詳細は `character-3d-rendering-proposal` §3.8 参照）
- `prepass_depth` はフラグメントシェーダー専用。`CharacterMaterial` の `boundary_proximity` 計算はフラグメントシェーダー内で行う

---

## 5. マイルストーン

### 全体フロー

```
Phase 2 完了ゲート
  └─ M-Gate: MS-2C 目視確認

Phase 3 着手前（今すぐ着手可・並走可）
  M-Pre1: WgpuFeatures::CLIP_DISTANCES 動作確認          ← P0 ブロッカー確認
  M-Pre2: RtT 基盤整備（create_rtt_texture 切り出し）
  M-Pre3: Camera3d 角度 V-1（目視・数値確定）
  M-Pre4: Character GLB PoC（face atlas 表示確認）        ← M-Pre3 + MS-Asset-Char-GLB-A 後

Phase 3 序盤
  M-3-1: Soul CharacterMaterial 本実装 + 2D Sprite置換     ← M-Pre3/4 完了後
  M-3-2: RtT WindowResized + 品質スケール                 ← M-Pre2 完了後
  M-3-3: SectionMaterial 基盤（MS-Section-A）             ← M-Pre1 完了後

Phase 3 キャラクター（M-3-1 完了後・GLB アニメクリップ並走）
  M-3-Char-A: AnimationGraph + SoulAnimState 実装          ← M-3-1 + Char-GLB-B 後
  M-3-Char-B: Soul P1 クリップ + face atlas 状態連動      ← M-3-Char-A + Char-Face 後
  M-3-Fam-R: Familiar 表示方式再検討                      ← M-3-1 後

Phase 3 中盤（GLB 取込 PoC 含む）
  M-3-4: テレインの3D化（MS-3A）                          ← M-3-3 完了後
  M-3-5: Building3dHandles SectionMaterial 移行            ← M-3-3 + GLB 取込後
  M-3-6: テレイン表面表現改善（MS-3B）                    ← M-3-4 完了後
  M-3-7: マウスヒットテスト Raycasting 化（MS-3C）         ← M-3-4 完了後

Phase 3 後半
  M-3-8: 2D スプライトインフラ廃止（MS-3D）               ← M-3-7 + M-3-Char-B + M-3-Fam-R 完了後
  M-3-9: 切断線 UI（MS-Section-C）                        ← M-3-5 完了後
  M-3-10: アートスタイル受入基準確定                       ← Phase 3 PoC 後
           → アウトライン生成計画（別計画）
```

---

### M-Gate: MS-2C 目視確認

> **依存**: Phase 2 実装完了済み

**確認内容**: 壁・アイテム・キャラクターが重なる状況で前後関係が正しく描画される

**完了条件**:
- [x] 壁とキャラクターの重なりが Z 値管理コードを追加せずに成立する
- [x] `cargo check` ゼロエラー

---

### M-Pre1: `WgpuFeatures::CLIP_DISTANCES` 動作確認

> **依存**: なし（今すぐ着手可）
> **根拠**: `section-material-proposal` §5 MS-Section-A step 0
> **重要度**: P0 ブロッカー。失敗した場合は SectionMaterial の技術設計を再検討する

**やること**:
- `bevy_app` の `RenderPlugin` 設定に `WgpuFeatures::CLIP_DISTANCES` を追加する

```rust
// bevy_app/src/main.rs（または HwVisualPlugin の初期化箇所）
App::new()
    .add_plugins(DefaultPlugins.set(RenderPlugin {
        render_creation: RenderCreation::Automatic(WgpuSettings {
            features: WgpuFeatures::CLIP_DISTANCES,
            ..default()
        }),
        ..default()
    }))
```

**変更ファイル**:
- `crates/bevy_app/src/main.rs`（または `hw_visual/src/lib.rs` のプラグイン初期化箇所）

**完了条件**:
- [ ] `cargo check` ゼロエラー
- [ ] ゲームが正常起動する（クラッシュしない）

---

### M-Pre2: RtT 基盤整備

> **依存**: なし（今すぐ着手可・M-Pre1 と並走可）
> **根拠**: `rtt-resolution-scaling-proposal` §4 Phase 3 着手前

**やること**:
1. `rtt_setup.rs` の `create_rtt_texture` 関数を切り出す
2. `sync_rtt_composite_sprite` システムを実装・登録する
3. `RenderTarget::Image` の受け取り型を実装前に `docsrs-mcp` / `~/.cargo/registry/src/` で確認する

**変更ファイル**:
- `plugins/startup/rtt_setup.rs`（`create_rtt_texture` 切り出し）
- `systems/visual/rtt_composite.rs`（新規：`sync_rtt_composite_sprite`）
- `hw_core/src/constants/render.rs`（`Z_RTT_COMPOSITE` 定数追加）

**完了条件**:
- [ ] `cargo check` ゼロエラー
- [ ] `RttTextures.handle` を手動差し替えしたとき合成スプライトのサイズが自動追従する（目視）

---

### M-Pre3: Camera3d 角度 V-1（目視確認・数値確定）

> **依存**: Phase 2 の Cuboid プリミティブが配置されていること
> **根拠**: `billboard-camera-angle-proposal` §5 V-1
> **重要度**: P0。角度未確定のまま Phase 3 の GLB 生成パイプラインを進められない

**やること**:
- Camera3d の Y 座標・Z オフセットを調整しながら目視確認する
- `world_lore.md` §6.2 のアートスタイル基準（壁に厚みが感じられる・床と壁の境界が自然）で判断する
- 確定した数値を `hw_core/src/constants/render.rs` に記録する

**確認基準**:
- [x] 壁に厚みが感じられる
- [x] 床と壁の境界が自然に見える
- [x] キャラクタープロキシ（Cuboid または仮GLB）が「体積のない存在に見える」（`character-3d-rendering-proposal` §3.6）

**確定する値**:
- `VIEW_HEIGHT`（Camera3d の Y 座標）
- `Z_OFFSET`（Camera3d の Z オフセット）
- 仰角（度数）

---

### M-Pre4: Character GLB PoC（face atlas 表示確認）

> **依存**: M-Pre3（角度数値確定後）・MS-Asset-Char-GLB-A（Soul GLB 配置済み）
> **根拠**: `character-3d-rendering-proposal` §3.5（Camera3d 角度との関係）

**やること**:
- `GameAssets` に `soul.glb#Scene0` の `Handle<Scene>` を追加し、Soul spawn 時に `SceneRoot` ベースで RtT へ流す
- `SceneInstanceReady` で Soul GLB 子孫へ `RenderLayers::layer(LAYER_3D)` を付与し、RtT Camera3d で確実に描画する
- `CharacterHandles` リソースを定義し、`mesh_face` に `soul_face_atlas.png` の先頭セルだけを切り出す最小 `StandardMaterial` を適用する
- 斜め Camera3d で Soul GLB が建物 Cuboid と Z バッファを共有し前後関係が正しく描画されることを確認する
- `mesh_face` は GLB 既定姿勢と authoring 済み UV をそのまま使い、face atlas が視認できることを確認する

**確認基準**:
- [x] 壁の後ろに入った Soul GLB が壁に隠れる（Z バッファ共有の確認）
- [x] Soul GLB が「体積のない存在に見える」アートスタイル感が出ている
- [x] `mesh_face` に通常表情の face atlas 1 セルが十分視認できる

**進捗メモ**:
- [x] `assets/models/characters/soul.glb` をリポジトリへ配置済み
- [x] Soul spawn は `SceneRoot` ベースで RtT に接続済み
- [x] `CharacterHandles` を追加し、`soul_face_atlas.png` を face atlas としてロード済み
- [x] `mesh_face` の差し替え条件を `GltfMeshName` / `Name` の両方で拾うように更新済み
- [x] `mesh_face` の回転は GLB 既定姿勢を使う方針に戻し、追加の billboard 処理は外した
- [x] `CharacterMaterial` の試作経路は撤去し、PoC は `StandardMaterial` + GLB 既定 UV に整理済み
- [x] `mesh_face` は Idle セルの可視領域を元にした 1.4 倍 crop で十分な視認性を確認済み

---

### M-3-1: Soul CharacterMaterial 本実装 + 2D Sprite置換

> **依存**: M-Pre3・M-Pre4 完了
> **根拠**: `character-3d-rendering-proposal` §3

**やること**:
1. `hw_visual/src/material/character_material.rs` を新規作成し Soul 向け `CharacterMaterial` を本実装する（`AlphaMode::Blend`）
2. `assets/shaders/character_material.wgsl` を新規作成し、Soul `mesh_body` / `mesh_face` の表示を custom material へ移行する
3. `CharacterHandles` を Soul 本実装向けの構成へ整理し、PoC 用 `StandardMaterial` 差し替えを `CharacterMaterial` ベースへ置き換える
4. Soul の Camera2d Sprite を削除し、通常表示を Soul GLB 側へ一本化する
5. Familiar は現行 billboard / 2D 経路を維持し、表示差分を増やさない
6. Soul 専用 `mask` RtT と最終合成 Material を追加し、画面上のシルエット丸めを後段で扱えるようにする

**変更ファイル**:
- `hw_visual/src/material/character_material.rs`（新規）
- `hw_visual/src/material/soul_mask_material.rs`（新規）
- `assets/shaders/character_material.wgsl`（新規）
- `assets/shaders/soul_mask_material.wgsl`（新規）
- `assets/shaders/rtt_composite_material.wgsl`（新規）
- `hw_visual/src/visual_handles.rs`（`CharacterHandles` の Soul 本実装化）
- `hw_visual/src/lib.rs`
- `plugins/startup/rtt_setup.rs`
- `plugins/startup/startup_systems.rs`
- `plugins/startup/rtt_composite.rs`
- Soul の Camera2d Sprite spawn 箇所

**完了条件**:
- [x] `cargo check` ゼロエラー
- [x] 通常ビューで Soul GLB が 2.5D 的に見える
- [x] Soul の `mesh_body` / `mesh_face` が `CharacterMaterial` 経路で描画される
- [x] Camera2d 側に Soul の Sprite が残っていない
- [x] Familiar の表示挙動は現状から退行していない
- [x] Soul のシルエット丸めが通常ビューで自然に見える（目視）

**進捗メモ**:
- [x] `hw_visual::CharacterMaterial` と `assets/shaders/character_material.wgsl` を追加し、Soul 用 `AlphaMode::Blend` custom material を導入済み
- [x] `CharacterHandles` を Soul body/face 用 `Handle<CharacterMaterial>` 構成へ更新済み
- [x] `apply_soul_gltf_render_layers_on_ready` で `mesh_body` / `mesh_face` の custom material 差し替えを実装済み
- [x] `mesh_body` は `soul.png` 流用をやめ、1x1 白テクスチャ + shader 側の base/shadow 色・簡易ポスタライズ・rim 強調で 2D 寄せする方式へ切り替え済み
- [x] Soul spawn から Camera2d `Sprite` を削除し、通常表示を GLB 側へ一本化済み
- [x] `animation_system` / `idle_visual_system` は `Sprite` を optional にして、Soul 本体に 2D Sprite がなくても動くよう更新済み
- [x] 通常ビューで Soul が 2.5D 的に見えること、Familiar に退行がないことを目視確認済み
- [x] `SoulMaskProxy3d` / `SoulMaskMaterial` / `Camera3dSoulMaskRtt` を追加し、Soul 専用 mask RtT を生成する前段を実装済み
- [x] `RttCompositeMaterial` を導入し、Soul mask を近傍サンプリングして最終合成で輪郭を少し丸める経路を追加済み
- [x] Soul シルエット丸めの見え方と body 不透明化後の見え方を目視確認済み

---

### M-3-Char-A: AnimationGraph + SoulAnimState 実装

> **依存**: M-3-1 完了・MS-Asset-Char-GLB-B 完了（P0 クリップ：Idle・Walk）
> **根拠**: `character-3d-rendering-proposal` §3.3

**やること**:
1. `SoulAnimState` 列挙型（`Idle` / `Walk` / `Work` / `Carry` / `Fear` / `Exhausted`）を定義する
2. `AnimationGraph` を構築し Idle / Walk / Work / Carry を `SoulAnimState` に連動させる
3. `sync_soul_anim_state` システムを実装・登録する（タスク状態 → `SoulAnimState` → AnimationGraph ノード切替）
4. `face_billboard_system` を本実装する（`FaceBillboard` コンポーネントを `mesh_face` エンティティに付与）
5. `CharacterMaterial.face_uv_offset` をゲーム状態から更新するシステムを実装する

**変更ファイル**:
- `hw_visual/src/anim/soul_anim.rs`（新規）
- `hw_visual/src/billboard.rs`（`face_billboard_system` のみ新規）
- `crates/bevy_app/src/systems/soul_ai/` 以下（SoulAnimState 連動）

**完了条件**:
- [ ] `cargo check` ゼロエラー
- [ ] Idle / Walk がタスク状態に連動して切り替わる（目視）
- [ ] `mesh_face` がどの角度でもカメラを向いている（目視）
- [ ] 表情が `SoulAnimState` に連動して切り替わる（目視）

---

### M-3-Char-B: Soul P1 クリップ + face atlas 状態連動

> **依存**: M-3-Char-A 完了・MS-Asset-Char-Face 完了
> **根拠**: `character-3d-rendering-proposal` §3

**やること**:
1. Soul の P1 クリップ（Work・Carry）をタスク状態に接続する
2. `CharacterMaterial.face_uv_offset` に顔テクスチャアトラス（MS-Asset-Char-Face）を統合する
3. `Fear` / `Exhausted` を含む表情切り替え規則を `SoulAnimState` と同期する

**変更ファイル**:
- `hw_visual/src/anim/soul_anim.rs`
- `hw_visual/src/material/character_material.rs`
- Soul 状態連動システム群

**完了条件**:
- [ ] `cargo check` ゼロエラー
- [ ] Soul の全 P1 クリップがタスク状態に連動する（目視）
- [ ] 顔テクスチャアトラスによる表情切り替えが動作する（目視）

---

### M-3-Fam-R: Familiar 表示方式再検討

> **依存**: M-3-1 完了
> **根拠**: Familiar が Soul と異なる Z 軸表現を持ち、3D 化の価値が未確定であるため

**やること**:
1. Familiar の現行表示（billboard / 2D）と GLB 化の利点・欠点を比較する
2. Z 軸表現、被遮蔽の必要性、アニメーション要求、実装コストを評価する
3. `GLB 化する / billboard 維持 / 2D 維持` のいずれかを決定し、後続マイルストーンへ反映する

**完了条件**:
- [ ] Familiar の表示方式が決定されている
- [ ] 決定内容が roadmap / implementation plan に反映されている

---

### M-3-2: RtT WindowResized + 品質スケール

> **依存**: M-Pre2 完了・`QualitySettings` リソース存在
> **根拠**: `rtt-resolution-scaling-proposal` §4 Phase 3 序盤

**やること**:
1. `systems/visual/rtt_resize.rs` を新規作成（`on_window_resized`・`on_quality_changed`・`recreate_rtt` ヘルパー）
2. `hw_core/src/quality.rs` に `rtt_scale()` メソッドを追加（高=1.0・中=0.75・低=0.5）
3. 両システムを `Update` スケジュールに登録

**変更ファイル**:
- `systems/visual/rtt_resize.rs`（新規）
- `hw_core/src/quality.rs`

**完了条件**:
- [ ] `cargo check` ゼロエラー
- [ ] ウィンドウリサイズ時に建物・キャラクターの描画が追従する（目視）
- [ ] 品質設定変更時に RtT 解像度が変わる（目視）

---

### M-3-3: SectionMaterial 基盤（MS-Section-A）

> **依存**: M-Pre1 完了（`WgpuFeatures::CLIP_DISTANCES` 確認済みであること）・Phase 3 着手
> **根拠**: `section-material-proposal` §5 MS-Section-A

**やること**:
1. `hw_visual/src/material/section_material.rs` を新規作成（`SectionMaterial`・`SectionCut`）
2. `assets/shaders/section_material.wgsl` を新規作成（`section-material-proposal` §3.4 の完全版 WGSL）
3. `MaterialPlugin::<SectionMaterial>` を `HwVisualPlugin` に追加
4. `sync_section_cut_normal` システムを実装・登録（`ElevationViewState` 変化時に `SectionCut.normal` を更新）
5. `sync_section_cut_to_materials` システムを実装・登録（`SectionCut` 変化時にマテリアル伝播）

**変更ファイル**:
- `hw_visual/src/material/section_material.rs`（新規）
- `assets/shaders/section_material.wgsl`（新規）
- `hw_visual/src/lib.rs`
- `systems/visual/camera_sync.rs`

**完了条件**:
- [ ] `cargo check` ゼロエラー
- [ ] `SectionCut.active = true` のとき Cuboid がスラブ外でクリップされる（目視）
- [ ] `SectionCut.active = false` のとき全体が正常描画される（目視）

---

### M-3-4: テレインの3D化（MS-3A）

> **依存**: M-3-3 完了
> **根拠**: `docs/plans/3d-rtt/milestone-roadmap.md` MS-3A

**やること**:
- 既存の地形タイル描画を 3D メッシュ / `SectionMaterial` ベースへ置き換える
- Camera2d 側のテレイン描画を廃止する
- `terrain_border.rs` / `borders.rs` に依存しない地形表現へ移行する

**完了条件**:
- [ ] `cargo check` ゼロエラー
- [ ] 地形が Camera3d → RtT のみで描画される
- [ ] Camera2d 側にインゲーム地形描画が残っていない

---

### M-3-5: Building3dHandles の SectionMaterial 移行（MS-Section-B）

> **依存**: M-3-3 完了・Phase 3 GLB 取込完了
> **根拠**: `section-material-proposal` §5 MS-Section-B

**やること**:
1. `visual_handles.rs` の `Building3dHandles` を `SectionMaterial` ベースに変更
2. `building_completion/spawn.rs` の全 `MeshMaterial3d<StandardMaterial>` を `MeshMaterial3d<SectionMaterial>` に置き換え
3. 設備別 visual system（`tank.rs`・`mud_mixer.rs` 等）の同様置き換え

**変更ファイル**:
- `hw_visual/src/visual_handles.rs`
- `building_completion/spawn.rs`
- `systems/visual/tank.rs`
- `systems/visual/mud_mixer.rs`

**完了条件**:
- [ ] `cargo check` ゼロエラー
- [ ] 矢視モードで切断線設定時に全 BuildingType のスラブ外部分がクリップされる（目視）
- [ ] トップダウンモードで全建物が正常表示される

---

### M-3-6: テレイン表面表現改善（MS-3B）

> **依存**: M-3-4 完了
> **根拠**: `docs/plans/3d-rtt/milestone-roadmap.md` MS-3B

**やること**:
- テクスチャブレンド
- ノイズによる遷移境界の有機化
- 必要なら生成時ベイクの検証

**完了条件**:
- [ ] 90度ベースの地形境界オーバーレイに依存しない見た目が成立する

---

### M-3-7: マウスヒットテスト Raycasting 化（MS-3C）

> **依存**: M-3-4 完了
> **根拠**: `docs/plans/3d-rtt/milestone-roadmap.md` MS-3C

**やること**:
- `hw_ui`・`bevy_app`・`hw_visual` に散在する `viewport_to_world_2d` 利用箇所を Camera3d からの Raycasting に全面置換する
- クリック・ホバー・範囲選択・配置プレビューを個別検証する

**完了条件**:
- [ ] インゲーム入力で `viewport_to_world_2d` への依存が残らない
- [ ] クリック・ホバー・ドラッグ操作が 3D ビューで正しく動作する

---

### M-3-8: 2D スプライトインフラの段階的廃止（MS-3D）

> **依存**: M-3-7 完了・M-3-Char-B 完了・M-3-Fam-R 完了
> **根拠**: `docs/plans/3d-rtt/milestone-roadmap.md` MS-3D

**やること**:
- 3D化済みインゲーム要素から `Sprite` コンポーネントと関連 Z 定数を順次削除する
- Camera2d を UI 専用へ絞る

**完了条件**:
- [ ] Camera2d 側に UI と純 2D オーバーレイだけが残る
- [ ] `cargo check` ゼロエラー

---

### M-3-9: 切断線 UI（MS-Section-C）

> **依存**: M-3-5 完了
> **根拠**: `section-material-proposal` §5 MS-Section-C

**やること**:
1. `SectionCutEditSession` コンポーネントを `hw_ui` に定義
2. 矢視モード入時に切断線配置モードを自動起動する処理を `camera_sync.rs` に追加
3. ワールドマップ上のクリック・ドラッグで `SectionCut.position` を更新する入力システムを実装
4. スラブ厚みスライダーを `bevy_egui` UI に追加
5. 切断線のワールドマップ上プレビュー（2D Gizmo）を実装

**変更ファイル**:
- `hw_ui/src/section_cut_ui.rs`（新規）
- `systems/visual/camera_sync.rs`（矢視モード入時の自動起動）

**完了条件**:
- [ ] `cargo check` ゼロエラー
- [ ] クリック・ドラッグで切断線を配置できる
- [ ] スラブ厚みスライダーを動かすと即座に 3D 描画が変化する

---

### M-3-10: アートスタイル受入基準確定（アウトライン生成への引き渡し）

> **依存**: Phase 3 GLB 取込 PoC 完了
> **根拠**: `outline-rendering-proposal` §3 前提条件1〜3

**やること**:
- `world_lore.md` §6.2 をベースに以下を文書化する
  - アウトライン：線幅・揺らぎ量・色・ズームアウト無効化閾値
  - 壁メッシュ構成：方法A（コーナー専用メッシュ）の GLB 仕様
  - 断面キャップ：方針 A / B / C の選択
- アウトライン生成の実装計画を別計画として起票できる状態にする

**完了条件**:
- [ ] アートスタイル受入基準がドキュメント化されている
- [ ] 壁メッシュ方法A の GLB 仕様が確定している
- [ ] 断面キャップ方針が確定している

---

## 6. リスクと対策

| リスク | 影響度 | 対策 |
| --- | --- | --- |
| `WgpuFeatures::CLIP_DISTANCES` が未サポート | 高（SectionMaterial 全体がブロック） | M-Pre1 で最初に確認。非対応なら方針Aのステンシルバッファ方式に切り替え |
| Camera3d 斜め角度変更で既存ゲームプレイが破綻 | 高（全建物の見た目が崩れる） | V-1 で段階的に確認。Phase 2 Cuboid 段階で数値確定してから本適用 |
| テレイン 3D 化でパフォーマンスが大幅悪化 | 中（60fps 維持不可） | `SectionMaterial` クリップでスラブ外描画を削減。LOD 設計で補う |
| AnimationGraph と Bevy 0.18 ボーン API の互換性 | 中（ボーンのインポート形式・ノード構成が想定と異なる可能性） | M-Pre4（Character GLB PoC）で最小実装を先行確認し API を確定してから M-3-Char-A に進む |
| `mesh_face` のローカルトランスフォーム計算 | 低（親の GlobalTransform 取得が必要） | `face_billboard_system` は親の `GlobalTransform` を Query し逆回転を適用する（`character-3d-rendering-proposal` §3.8 参照） |
| GLB 生成品質が受入基準未達 | 中（M-3-4 以降がブロック） | 手動モデリングを並走オプションとして維持 |
| `WindowResized` 毎フレーム発火でテクスチャ再生成 | 低（フレームドロップ） | `events.read().last()` でスロットリング（M-3-2 で実装済み） |

---

## 7. 検証計画

### 各マイルストーン共通

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

### Phase 3 PoC ゲート（M-3-3 完了後）

- Cuboid に `SectionMaterial` を付けてスラブクリップが動作すること
- `SectionCut.active = false` で全体が正常描画されること
- `WgpuFeatures::CLIP_DISTANCES` 有効・無効それぞれで動作を比較

### フルRtT到達ゲート（M-3-8 完了後）

- Camera2d 側にインゲーム要素が残っていない
- 全インゲーム入力が Raycasting で処理されている
- GTX 1650 相当で 60fps を維持している

---

## 8. ロールバック方針

- 各マイルストーンは独立 PR として実装し、git revert で単体で戻せる粒度にする
- Phase 3 着手前の基盤整備（M-Pre1〜M-Pre4）は副作用が少ないため低リスク
- `SectionMaterial` 移行（M-3-5）は `StandardMaterial` が動いている状態で着手し、`cargo check` が通るまで切り替えない

---

## 9. AI 引継ぎメモ

### 現在地

- 進捗: `33%`（M-Pre1〜M-Pre4・M-3-1 完了、次は M-3-Char-A）
- 完了済みマイルストーン: M-Pre1・M-Pre2・M-Pre3・M-Pre4・M-3-1
- 今すぐ着手可: M-3-Char-A・M-3-2・M-3-3

### 次の AI が最初にやること

1. `docs/proposals/3d-rtt/20260317/character-3d-rendering-proposal-2026-03-16.md` を読む（キャラクター3D化の採用提案・本計画の主要な変更元）
2. `docs/proposals/3d-rtt/20260316/` の 4 提案書を読む
3. M-3-Char-A（AnimationGraph + SoulAnimState 実装）へ着手する
4. Section 連動は後回しにし、Soul 単体の表示品質を先に固める
5. Familiar は M-3-Fam-R まで現行経路を維持し、3D 化を前提にしない

### ブロッカー/注意点

- MS-3-3 に着手するまで `section_clip` の共有化は行わない。CharacterMaterial は単体で成立させること
- Camera3d の `RenderTarget::Image` 型は `Handle<Image>` → `ImageRenderTarget` の `From` 実装で対応済み（MS-1B で確認済み）
- `SoulProxy3d` は Soul 本実装中の暫定ルートとして扱う。`FamiliarProxy3d` の扱いは M-3-Fam-R で決定する
- `mesh_face` の `face_billboard_system` は子エンティティのローカルトランスフォームに書き込む。親の `GlobalTransform` から逆回転を適用すること（`character-3d-rendering-proposal` §3.8 参照）
- `prepass_depth` はフラグメントシェーダー専用。`boundary_proximity` 計算をVertex Shaderに書かないこと
- Soul のシルエット丸めは `RttCompositeMaterial` 側の 2D 後段処理として扱う。`CharacterMaterial` 側で全体輪郭の再構成をしようとしないこと
- Familiar の GLB 化は前提にしない。M-3-Fam-R で価値評価が終わるまで後続マイルストーンへ混ぜない

### 参照必須ファイル

- `docs/plans/3d-rtt/milestone-roadmap.md`（Phase 2 完了状況・依存グラフ全体）
- `docs/plans/3d-rtt/asset-milestones-2026-03-17.md`（キャラクター GLB・アニメクリップ・顔アトラスの制作フロー）
- `docs/proposals/3d-rtt/20260317/character-3d-rendering-proposal-2026-03-16.md`（CharacterMaterial・AnimationGraph・face_billboard_system 設計）
- `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md`（WGSL 完全版・WgpuFeatures 設定例）
- `docs/proposals/3d-rtt/20260316/billboard-camera-angle-proposal-2026-03-16.md`（Camera3d 角度確定方法）
- `docs/proposals/3d-rtt/20260316/rtt-resolution-scaling-proposal-2026-03-16.md`（RtT リサイズ・品質スケール）
- `hw_visual/src/visual3d.rs`（`SoulProxy3d` / `FamiliarProxy3d` 定義・Soul/Familiar 表示経路の判断材料）
- `plugins/startup/rtt_setup.rs`（現在の RtT 初期化・`create_rtt_texture` 切り出し対象）
- `systems/visual/camera_sync.rs`（Camera3d 同期システム・SectionCut 更新の追加先）

### 最終確認ログ

- 最終 `cargo check`: `2026-03-28` / pass（Soul mask prepass + RtT composite 更新後）
- 未解決エラー: なし

### Definition of Done

- [ ] M-Gate〜M-3-9・M-3-Char-A・M-3-Char-B が全て完了
- [ ] Camera2d 側のインゲーム描画がゼロ
- [ ] `cargo check` ゼロエラー
- [ ] GTX 1650 相当で 60fps を維持
- [ ] `docs/architecture.md` / 各 `docs/*.md` が Phase 3 完了状態に更新済み
- [ ] M-3-10 が完了しアウトライン生成計画が別計画として起票されている

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-16` | Claude Sonnet 4.6 | 初版作成（Phase 3 計画ドラフト） |
| `2026-03-17` | Claude Sonnet 4.6 | キャラクター3D化採用提案（`character-3d-rendering-proposal-2026-03-16`）を反映。ビルボード方式を廃止し CharacterMaterial + AnimationGraph に変更。M-Pre4・M-3-1 書き換え、M-3-Char-A・M-3-Char-B 追加 |
