# Visual Test Scene

ゲーム本体とは独立して Soul GLB レンダリングと建築物配置を検証するための独立クレート。

## 起動

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo run -p visual_test
```

## 検証できること

| 項目 | 概要 |
|:---|:---|
| 表情アトラス UV 切り替え | 6 表情 (Normal/Fear/Exhausted/Concentration/Happy/Sleep) の個別・一括切り替え |
| モーション | Idle / FloatingBob / Sleeping / Resting / Escaping / Dancing |
| GLB アニメーション再生 | soul.glb の 8 クリップを順番に切り替えて再生確認 |
| シェーダーパラメータ調整 | 選択 Soul の ghost_alpha / rim_strength / posterize_steps をリアルタイム調整 |
| カメラパン・ズーム | W/A/S/D パン・スクロールズーム（ゲーム本体と同じ PanCamera）|
| 矢視（Elevation View）| V キーで TopDown/North/East/South/West を切替して GLB を全方向から確認 |
| 仰角調整 | VIEW_HEIGHT / Z_OFFSET をリアルタイムに変更して TopDown 俯角を確認 |
| 複数 Soul 干渉 | Soul を最大 6 体まで追加し、Z-fight・マテリアル独立性を確認 |
| ワールド上での建築物配置 | ゲーム本体と同一のゴーストプレビュー + クリック配置方式 |
| 建築物 2D/3D 表示 | 2D スプライト + 3D メッシュの重ね描画を本番環境と同じ条件で確認 |
| 影・ライト | DirectionalLight + CascadeShadowConfig による影をゲーム本体と同条件で検証 |
| RtT パイプライン | Camera3d → オフスクリーンテクスチャ → composite sprite の描画経路 |

## 操作

### 共通

| キー | 操作 |
|:---|:---|
| `Space` | モード切替 (Soul ⇔ Build) |
| `H` | メニューパネル表示/非表示 |
| `W/A/S/D` | カメラパン |
| スクロール | カメラズーム（メニュー上ではパネルスクロールに切り替わる）|
| `V` | 矢視切替 (TopDown → North → East → South → West) |
| `J/K` | VIEW_HEIGHT ±10 |
| `U/I` | Z_OFFSET ±10 |
| `O` | VIEW_HEIGHT / Z_OFFSET をデフォルト値にリセット |
| `Esc` | 終了 |

### Soul モード

| キー | 操作 |
|:---|:---|
| `1`–`6` | 表情切り替え |
| `G` | 全表情モード（各 Soul に異なる表情を自動割当）|
| `=` | Soul 追加 (最大 6) |
| `-` | Soul 削除 |
| `M` | モーション切り替え |
| `Q` | アニメーションクリップ切り替え |
| `Z/X` | 選択 Soul の `ghost_alpha` ±0.05 |
| `C/F` | 選択 Soul の `rim_strength` ±0.05 |
| `B/N` | 選択 Soul の `posterize_steps` ±1.0 |
| `P` | シェーダーパラメータをデフォルトにリセット |
| `R` | 全 Soul の位置・回転・スケールをリセット |
| `←→↑↓` | 選択 Soul を移動 |
| `Tab` | Soul 選択を順番に切り替え |

### Build モード

| 操作 | 内容 |
|:---|:---|
| **マウス移動** | ゴーストプレビューが追従（緑 = 配置可能、赤 = 占有済み）|
| **左クリック** | 空きグリッドなら建築物を配置、占有済みなら削除 |
| `[` / `]` | 建築種別を前/次に切り替え |
| `Enter` | 現在のゴースト位置で配置/削除 |
| `Del` | 全建築物を削除 |

## メニューパネル

右サイドに常時表示されるデバッグパネル（`[H]` で折りたたみ）。ボタンで操作でき、現在値をリアルタイムで反映する。

### パネル構成

```
━━ Visual Test ━━━━━━━━━━━━━━
 [H] メニューを閉じる
 [SOUL]  [BUILD]          ← モード切替

─ カメラ ─────────────────────
 [TopDown]               [V]
 HEIGHT: [150-] [150+]
 OFFSET: [ 90-] [ 90+]
 [リセット]

─ ソウルセクション（Soul モード時のみ表示）─
 表情ボタン × 6 + 全表情
 アニメーションクリップボタン
 モーションボタン
 シェーダーパラメータ（ghost/rim/posterize）
 Soul 追加/削除/選択/リセット

─ 建築セクション（Build モード時のみ表示）─
 建築種別ボタン × 11
 配置位置 (x, y)         ← マウス追従
 [配置/削除 [Enter]]
 [全削除 [Del]]
```

パネル上でスクロールするとパネルがスクロールされ、ワールドのズームは無効化される。

## アーキテクチャ

独立した `visual_test` クレート (`crates/visual_test/`)。ゲーム本体の `GameAssets` / `StartupPlugin` には一切依存しない。

### モジュール構成

| ファイル | 責務 |
|:---|:---|
| `main.rs` | App 構築・プラグイン登録・システム順序定義 |
| `types.rs` | 全共有型・定数・リソース (`TestState`, `TestElev`, enums) |
| `setup.rs` | カメラ・ライト・RtT・UIパネルの初期化 |
| `building.rs` | 建築物のアセット定義・スポーン/デスポーン・ゴーストシステム・ワールドマップ生成 |
| `soul.rs` | Soul GLB スポーン・SceneInstanceReady Observer |
| `systems.rs` | ボタンインタラクション・カメラ同期・Soul 描画システム群 |
| `hud.rs` | パネル表示制御・ボタン状態更新・動的テキスト更新 |
| `input.rs` | キーボード入力ハンドラ（Soul モード / Build モード）|

### カメラ 3 層構造（ゲーム本体と同構成）

```
Camera3d          (LAYER_3D,      order=-1)  → RtT オフスクリーンテクスチャ
TestMainCamera    (LAYER_2D,      order= 0)  ← PanCamera（パン・ズーム）
Overlay Camera2d  (LAYER_OVERLAY, order= 1)  ← composite sprite をスクリーンに描画
```

`sync_test_camera3d` が毎フレーム TestMainCamera の Transform/scale を Camera3d へ反映する。

### ゴーストプレビュー（建築物配置）

マウス座標 → `Camera::viewport_to_world_2d` → `world_to_grid` → グリッドスナップ の流れで毎フレーム更新。

| 状態 | ゴースト色 |
|:---|:---|
| 空きグリッド | `Color::srgba(0.5, 1.0, 0.5, 0.5)` 緑・50% |
| 占有済みグリッド | `Color::srgba(1.0, 0.2, 0.2, 0.5)` 赤・50% |

ゴーストスプライトには実際の建築テクスチャが表示される。メニューパネル上ではマウス入力が無効化される。

`update_building_cursor` システム（`building.rs`）が座標変換・占有チェック・左クリック配置を一括担当する。

### UI パネルボタン

`VisualTestAction` コンポーネントで各ボタンアクションを識別。`Changed<Interaction>` フィルタで `Interaction::Pressed` を検知し `TestState` を更新する。ボタン色は `update_button_states`（毎フレーム）が管理する。

| 色 | 状態 |
|:---|:---|
| `BTN_DEF` (暗グレー) | 通常 |
| `BTN_HOVER` (紫) | ホバー |
| `BTN_PRESS` (オレンジ暗) | 押下中 |
| `BTN_ACT` (オレンジ明) | 選択中 |
| `BTN_ACT_H` (オレンジ明+ホバー) | 選択中かつホバー |

### ゲーム本体との対応

| visual_test 内 | ゲーム本体対応 |
|:---|:---|
| `TestMainCamera` + `PanCamera` | `hw_ui::camera::MainCamera` + `PanCamera` |
| `sync_test_camera3d` | `systems::visual::camera_sync::sync_camera3d_system` |
| `TestElevDir` / `TestElev` | `ElevationDirection` / `ElevationViewState` |
| `update_building_cursor` ゴースト | `systems::visual::placement_ghost::placement_ghost_system` |
| `SoulAnimHandle` | `SoulAnimationPlayer3d` + `SoulAnimationLibrary` |
| `on_soul_scene_ready` | `apply_soul_gltf_render_layers_on_ready` |

## ファイルパス

| ファイル | 内容 |
|:---|:---|
| `crates/visual_test/` | クレートルート |
| `crates/visual_test/src/` | Rust ソース（上記モジュール）|
| `crates/visual_test/Cargo.toml` | 依存クレート定義 |

