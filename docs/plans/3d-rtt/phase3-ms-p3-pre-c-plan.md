# Phase 3 着手前基盤整備計画 (MS-2C〜MS-P3-Pre-C)

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `phase3-ms-p3-pre-c-plan` |
| ステータス | `Complete` |
| 作成日 | `2026-03-17` |
| 関連ロードマップ | `docs/plans/3d-rtt/milestone-roadmap.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/billboard-camera-angle-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/rtt-resolution-scaling-proposal-2026-03-16.md` |

---

## 1. 目的

### 解決したい課題

Phase 3 着手前の基盤整備として、以下の課題を解決する。

- **[MS-P3-Pre-A: WgpuFeatures::CLIP_DISTANCES 動作確認]** `WgpuFeatures::CLIP_DISTANCES` の動作確認（SectionMaterial のブロッカー）
- **[MS-P3-Pre-B: RtT 基盤整備]** RtT テクスチャのウィンドウリサイズ非対応の解消（RtT 基盤整備）
- **[MS-P3-Pre-C: Camera3d 角度 V-1]** Camera3d が真上（90°）固定であることの解消（Camera3d 角度 V-1）

### 到達したい状態

- Phase 3 の主要実装（キャラクター3D化、セクションカット等）に進むための基盤が整っている
- Camera3d の斜め角度（約53°）の数値が確定している
- RtT テクスチャ管理が一元化されている

### 成功指標

- `cargo check` ゼロエラー
- MS-2C、MS-P3-Pre-A、MS-P3-Pre-B、MS-P3-Pre-C の各完了条件を満たすこと

---

## 2. スコープ

### 対象（In Scope）

- **[MS-2C]** Phase 2 完了状態の目視確認（ハイブリッド段階の前後関係検証）
- **[MS-P3-Pre-A]** `WgpuFeatures::CLIP_DISTANCES` 確認
- **[MS-P3-Pre-B]** RtT リサイズ基盤整備
- **[MS-P3-Pre-C]** Camera3d 角度確定

### 非対象（Out of Scope）

- MS-P3-Pre-D 以降の全ての実装（キャラクター GLB 対応、マテリアル本実装、テレイン3D化など）

---

## 3. 現状とギャップ

| マイルストーン | 現状 | ギャップ |
| --- | --- | --- |
| **[MS-P3-Pre-A]** `CLIP_DISTANCES` 動作確認 | `WgpuFeatures::CLIP_DISTANCES` を有効化済み | 実機起動確認のみ継続監視 |
| **[MS-P3-Pre-B]** RtT 基盤整備 | `PrimaryWindow` の物理解像度追従と合成スプライト同期を実装済み | 品質スケール導入は MS-3-2 側で対応 |
| **[MS-P3-Pre-C]** Camera3d 角度 V-1 | `VIEW_HEIGHT = 150.0` / `Z_OFFSET = 90.0` を確定し、壁の見え方・配置ズレ・キャラクタープロキシ確認まで完了 | GLB PoC（MS-P3-Pre-D）へ進む |

---

## 4. マイルストーン

### MS-2C: ハイブリッド段階の前後関係検証

> **依存**: Phase 2 実装完了済み

**確認内容**: 壁・アイテム・キャラクターが重なる状況で前後関係が正しく描画される

**完了条件**:
- [x] 壁とキャラクターの重なりが Z 値管理コードを追加せずに成立する
- [x] `cargo check` ゼロエラー

---

### MS-P3-Pre-A: `WgpuFeatures::CLIP_DISTANCES` 動作確認

> **依存**: なし（今すぐ着手可）
> **根拠**: `section-material-proposal` §5 MS-Section-A step 0
> **重要度**: P0 ブロッカー。失敗した場合は SectionMaterial の技術設計を再検討する

**やること**:
- `crates/bevy_app/src/main.rs` の既存 `WgpuSettings` に `WgpuFeatures::CLIP_DISTANCES` を追加する

```rust
// crates/bevy_app/src/main.rs（RenderPlugin 設定箇所）
// 既存 import に WgpuFeatures を追加（新規行不要・destructure に追記するだけ）：
//   use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
//   → use bevy::render::settings::{Backends, RenderCreation, WgpuFeatures, WgpuSettings};
.set(RenderPlugin {
    render_creation: RenderCreation::Automatic(WgpuSettings {
        backends: Some(backends), // WSL は GL を優先（既存）
        features: WgpuFeatures::CLIP_DISTANCES, // 追加
        ..default()
    }),
    ..default()
})
```

**変更ファイル**:
- `crates/bevy_app/src/main.rs`（`WgpuSettings` に `features` フィールド追加・既存 `use bevy::render::settings::{...}` に `WgpuFeatures` を追記）

**完了条件**:
- [x] `cargo check` ゼロエラー
- [x] ゲームが正常起動する（クラッシュしない）

---

### MS-P3-Pre-B: RtT 基盤整備

> **依存**: なし（今すぐ着手可・MS-P3-Pre-A と並走可）
> **根拠**: `rtt-resolution-scaling-proposal` §4 Phase 3 着手前

**やること**:
1. `plugins/startup/mod.rs` の `setup()` にインライン記述されている RTT テクスチャ生成処理を `create_rtt_texture` 関数として `rtt_setup.rs` に切り出す
2. `plugins/startup/rtt_composite.rs` に `sync_rtt_composite_sprite` システムを追加・登録する
   > ⚠️ `rtt-resolution-scaling-proposal` §3.3 のサンプルコードは `rtt.handle` を参照しているが、実際の `RttTextures` フィールドは `texture_3d`。実装時は `rtt.texture_3d` を使用すること
3. `hw_core/src/constants/render.rs` に `Z_RTT_COMPOSITE` 定数を追加する
4. `RenderTarget::Image` の受け取り型を実装前に `docsrs-mcp` / `~/.cargo/registry/src/` で確認する
5. `PrimaryWindow` の物理解像度に追従して RtT を再生成し、合成スプライトは logical size + TopDown 縦補正で同期する

**変更ファイル**:
- `crates/bevy_app/src/plugins/startup/rtt_setup.rs`（`create_rtt_texture` 関数追加）
- `crates/bevy_app/src/plugins/startup/mod.rs`（`setup()` のインライン生成処理を `create_rtt_texture` 呼び出しに置換）
- `crates/bevy_app/src/plugins/startup/rtt_composite.rs`（`sync_rtt_output_bindings` システム追加）
- `crates/hw_core/src/constants/render.rs`（`Z_RTT_COMPOSITE` 定数追加）

**完了条件**:
- [x] `cargo check` ゼロエラー
- [x] `RttTextures.texture_3d` を手動差し替えしたとき合成スプライトのサイズが自動追従する（目視）

---

### MS-P3-Pre-C: Camera3d 角度 V-1（目視確認・数値確定）

> **依存**: Phase 2 の Cuboid プリミティブが配置されていること
> **根拠**: `billboard-camera-angle-proposal` §5 V-1
> **重要度**: P0。角度未確定のまま Phase 3 の GLB 生成パイプラインを進められない

**やること**:
- Camera3d の Y 座標・Z オフセットを調整しながら目視確認する
- `world_lore.md` §6.2 のアートスタイル基準（壁に厚みが感じられる・床と壁の境界が自然）で判断する
- 確定した数値を `hw_core/src/constants/render.rs` に定数として追加し、`camera_sync.rs` を以下の変更で差し替える：

  **① translation の更新（既存の `y = 100.0` を置換）**

  ```rust
  // TopDown モード（変更後）
  cam3d.translation.x = cam2d.translation.x;
  cam3d.translation.y = VIEW_HEIGHT;              // 100.0 → 定数に
  cam3d.translation.z = scene_z + Z_OFFSET;       // 2D 中心 + 固定奥行きオフセット
  ```

  **② rotation の更新（新規追加）**

  現状の `camera_sync.rs` は translation と projection scale のみ更新しており rotation には触れていない。Camera3d 初期 rotation は `mod.rs` の spawn 時に斜め俯瞰用の固定回転として設定し、TopDown 時はその回転を毎フレーム維持する。

  ```rust
  // TopDown モード translation 更新直後に追加
  cam3d.rotation = ElevationDirection::TopDown.camera_rotation();
  ```

  `camera_sync.rs` が毎フレーム rotation を上書きするため、`mod.rs` 側の Camera3d 初期 spawn も同じ固定回転を使用する。

**変更ファイル**:
- `crates/hw_core/src/constants/render.rs`（`VIEW_HEIGHT`・`Z_OFFSET` 定数追加）
- `crates/bevy_app/src/systems/visual/camera_sync.rs`（TopDown ブロックで translation と Projection.scale を更新）

**確認基準**:
- [x] 壁に厚みが感じられる
- [x] 床と壁の境界が自然に見える
- [x] キャラクタープロキシ（Cuboid または仮GLB）が「体積のない存在に見える」（`character-3d-rendering-proposal` §3.6）
- [x] 壁の上面と側面を見分けられる補助表示が確認できる（検証用）

**確定する値**:
- `VIEW_HEIGHT`（Camera3d の Y 座標）
- `Z_OFFSET`（Camera3d の Z オフセット）
- 仰角（度数）

**進め方（3 ステップ）**:

#### Step 1: コード変更（初期値で実装）

1. `render.rs` に初期値を追加：
   ```rust
  pub const VIEW_HEIGHT: f32 = 150.0;
  pub const Z_OFFSET: f32 = 90.0;  // 仰角 ≈ 59°（arctan(150/90) ≈ 59.0°）
   ```
2. `camera_sync.rs` の TopDown ブロックを ①②（上記コード）に差し替える

#### Step 2: 目視確認（目視で調整）

ゲームを起動し、以下で判断する：

| 状態 | 対処 |
| --- | --- |
| 壁が薄すぎる（面だけに見える） | `Z_OFFSET` を小さくする（仰角を上げる） |
| 壁が厚すぎる（奥行き感が強い） | `Z_OFFSET` を大きくする（仰角を下げる） |
| 床と壁の境界が不自然 | `VIEW_HEIGHT` を微調整する |

#### Step 3: 数値確定

確認後に確定値を報告 → `VIEW_HEIGHT`・`Z_OFFSET` を最終値に更新してタスク完了
