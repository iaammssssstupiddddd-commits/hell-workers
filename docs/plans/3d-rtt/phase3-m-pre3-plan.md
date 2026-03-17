# Phase 3 実装計画 (M-Pre3まで)

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `phase3-m-pre3-plan` |
| ステータス | `Draft` |
| 作成日 | `2026-03-17` |
| 関連ロードマップ | `docs/plans/3d-rtt/milestone-roadmap.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/billboard-camera-angle-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/rtt-resolution-scaling-proposal-2026-03-16.md` |

---

## 1. 目的

### 解決したい課題

Phase 3 着手前の基盤整備として、以下の課題を解決する。

- `WgpuFeatures::CLIP_DISTANCES` の動作確認（SectionMaterial のブロッカー）
- RtT テクスチャのウィンドウリサイズ非対応の解消
- Camera3d が真上（90°）固定であることの解消（角度の数値確定）

### 到達したい状態

- Phase 3 の主要実装（キャラクター3D化、セクションカット等）に進むための基盤が整っている
- Camera3d の斜め角度（約53°）の数値が確定している
- RtT テクスチャ管理が一元化されている

### 成功指標

- `cargo check` ゼロエラー
- M-Pre3 までの各完了条件を満たすこと

---

## 2. スコープ

### 対象（In Scope）

- Phase 3 着手前の基盤整備（`WgpuFeatures::CLIP_DISTANCES` 確認・RtT リサイズ基盤・Camera3d 角度確定）

### 非対象（Out of Scope）

- M-Pre4 以降の全ての実装（キャラクター GLB 対応、マテリアル本実装、テレイン3D化など）

---

## 3. 現状とギャップ

| 項目 | 現状 | ギャップ |
| --- | --- | --- |
| `CLIP_DISTANCES` | 動作未確認 | SectionMaterial 全体がブロックされる |
| RtT テクスチャ | 1280×720 固定 | ウィンドウリサイズで表示崩壊 |
| Camera3d 角度 | 真上（90°）固定 | 斜め約53°未適用。GLB 生成パイプラインの撮影角度が決められない |

---

## 4. マイルストーン

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
- [ ] 壁に厚みが感じられる
- [ ] 床と壁の境界が自然に見える
- [ ] キャラクタープロキシ（Cuboid または仮GLB）が「体積のない存在に見える」（`character-3d-rendering-proposal` §3.6）

**確定する値**:
- `VIEW_HEIGHT`（Camera3d の Y 座標）
- `Z_OFFSET`（Camera3d の Z オフセット）
- 仰角（度数）
