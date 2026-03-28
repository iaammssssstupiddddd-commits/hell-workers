# Visual Test Scene

Soul GLB の 3D レンダリングをゲーム本体とは独立して検証するための Bevy example。

## 起動

```bash
cargo visual-test
```

## 検証できること

| 項目 | 概要 |
|:---|:---|
| 表情アトラス UV 切り替え | 6 表情 (Normal/Fear/Exhausted/Concentration/Happy/Sleep) の個別・一括切り替え |
| モーション | Idle / FloatingBob / Sleeping / Resting / Escaping / Dancing の Transform 操作 |
| GLB アニメーション再生 | soul.glb の 8 クリップ（Idle/Walk/Work/Carry/Fear/Exhausted/WalkLeft/WalkRight）を順番に切り替えて再生時間を確認 |
| シェーダーパラメータ調整 | 選択 Soul の ghost_alpha / rim_strength / posterize_steps をキーボードでリアルタイム調整 |
| カメラパン・ズーム | W/A/S/D パン・スクロールズーム（ゲーム本体と同じ PanCamera）|
| 矢視（Elevation View）| V キーで TopDown/North/East/South/West を切替し、側面・正面からの GLB 確認 |
| 仰角調整 | VIEW_HEIGHT / Z_OFFSET をリアルタイムに変更して TopDown 俯角を確認 |
| 複数エンティティ干渉 | Soul を最大 6 体まで追加し、矢印キーで移動させて重なり・Z-fight を確認 |
| Per-entity マテリアル | 各 Soul が独立した `Handle<CharacterMaterial>` を持ち、個別に表情・シェーダー値を変更可能 |
| RtT パイプライン | Camera3d → オフスクリーンテクスチャ → composite sprite の描画経路 |

## 操作

| キー | 操作 |
|:---|:---|
| `1`-`6` | 表情切り替え (1=Normal, 2=Fear, 3=Exhausted, 4=Concentration, 5=Happy, 6=Sleep) |
| `G` | 全表情モード — 各 Soul に異なる表情を自動割当 |
| `+` / `=` | Soul 追加 (最大 6) |
| `-` | Soul 削除 (非選択のものから優先) |
| `M` | モーション切り替え (Idle → FloatingBob → Sleeping → Resting → Escaping → Dancing) |
| `Q` | アニメーションクリップ切り替え (Idle → Walk → Work → Carry → Fear → Exhausted → WalkLeft → WalkRight) |
| `Z` / `X` | 選択 Soul の `ghost_alpha` ±0.05 |
| `C` / `F` | 選択 Soul の `rim_strength` ±0.05 |
| `B` / `N` | 選択 Soul の `posterize_steps` ±1.0 |
| `P` | シェーダーパラメータをデフォルト値にリセット |
| `W` / `A` / `S` / `D` | カメラパン |
| スクロール | カメラズーム |
| `V` | 矢視切替 (TopDown → North → East → South → West) |
| `J` / `K` | TopDown 時の `VIEW_HEIGHT` ±10 (範囲 50〜400) |
| `U` / `I` | TopDown 時の `Z_OFFSET` ±10 (範囲 0〜400) |
| `O` | VIEW_HEIGHT / Z_OFFSET をデフォルト (150/90) にリセット |
| `R` | 全 Soul の位置・回転・スケールをリセット |
| `←` `→` `↑` `↓` | 選択 Soul を移動 |
| `Tab` | Soul 選択を順番に切り替え |
| `Esc` | 終了 |

## メニューパネル

右サイドに常時表示されるデバッグパネル。現在値がリアルタイムで反映される。
`[H]` でパネルを折りたたみ（右上に `[H] メニュー表示` ヒントが出る）。

```
━━ Visual Test ━━━━━━━━━━━━━━
 Souls:3/6   Soul#0
 [H] メニューを閉じる

─ 表情  [1-6]  [G]:全体 ─────
  [1] Normal
 ►[2] Fear         ← 現在選択中
  ...

─ アニメーション  [Q]:次へ ───
  Idle
 ► Walk  (1.2s)   ← 現在再生中 + 時間
  Work
  ...

─ Transform  [M]:次へ ────────
 ► Idle
  ...

─ シェーダー  [P]:reset ──────
 ghost_alpha    1.00  [Z]/[X]
 rim_strength   0.28  [C]/[F]
 posterize      4.0   [B]/[N]

─ カメラ ────────────────────────
 [W/A/S/D]  パン
 [スクロール]  ズーム
 [V]        矢視切替
 方向:  TopDown

─ 仰角  [O]:reset ───────────
 HEIGHT   150  [J]/[K]
 OFFSET    90  [U]/[I]
 仰角    31.0°

─ Soul管理 ───────────────────
 [=]/[-]   追加 / 削除
 [Tab]     選択切替
 [R]       位置リセット
 [←→↑↓]  移動
 [Esc]     終了
```

## アーキテクチャ

ゲーム本体の `GameAssets` / `StartupPlugin` には依存せず、必要最小限のアセット（`soul.glb`, `soul_face_atlas.png`, 1x1 白テクスチャ）だけを自前で読み込む自己完結型 example。

### カメラ 3 層構造（ゲーム本体と同構成）

```
Camera3d     (LAYER_3D,      order=-1)  → RtT オフスクリーンテクスチャ
TestMainCamera Camera2d (LAYER_2D,   order= 0)  ← PanCamera（W/A/S/D パン・スクロールズーム）
Overlay Camera2d       (LAYER_OVERLAY, order= 1)  ← Composite Sprite をスクリーンに描画
```

`sync_test_camera3d` が毎フレーム TestMainCamera の `Transform.translation` と `scale` を Camera3d へ反映する（ゲーム本体の `sync_camera3d_system` 相当）。

### GLB マテリアル差し替え

`SceneInstanceReady` Observer が GLB 読み込み完了後に子孫を走査し、メッシュ名で face/body を判別してマテリアルを差し替える。ゲーム本体の `apply_soul_gltf_render_layers_on_ready` と同等のロジック。

| メッシュ名 | 差し替え先 | 追加処理 |
|:---|:---|:---|
| `Soul_Face_Mesh` | `CharacterMaterial::face(...)` | `SOUL_FACE_SCALE_MULTIPLIER` でスケール補正 |
| `Soul_Mesh.010` | `CharacterMaterial::body(...)` | — |

### GLB アニメーション設定（on_soul_scene_ready）

1. 子孫から `AnimationPlayer` エンティティを特定
2. `Assets<Gltf>.named_animations` から `ANIM_CLIP_NAMES` 順で `AnimationGraph` を構築
3. `AnimationGraphHandle` + `AnimationTransitions` を AnimationPlayer エンティティに挿入
4. `SoulAnimHandle` コンポーネントを Soul ルートエンティティに追加

`apply_animation` システムが毎フレーム `state.anim_clip_idx` と `SoulAnimHandle.current_playing` を比較し、変わっていれば `AnimationTransitions::play(..., Duration::ZERO)` で即切り替え。

### シェーダーパラメータ調整

`apply_shader_params` が毎フレーム選択 Soul の `body_mat` に `state.ghost_alpha / rim_strength / posterize_steps` を適用する。per-entity マテリアルのため他の Soul には影響しない。

⚠️ Tab で選択を変えると、新しい選択 Soul に現在の state 値が即時適用される（意図的な動作）。

### 矢視（Elevation View）

`TestElevDir`（TopDown/North/East/South/West）を `TestElev` リソースで管理。V キーで循環切替。

`sync_test_camera3d` が各方向に応じた Camera3d 位置と回転を毎フレーム設定する：

| 方向 | Camera3d 位置 | 用途 |
|:---|:---|:---|
| TopDown | `(x, VIEW_HEIGHT, scene_z + Z_OFFSET)` | 通常の斜め俯瞰 |
| North | `(x, SOUL_MID_Y, scene_z + ELEV_DISTANCE)` | 南向き側面視 |
| East | `(x + ELEV_DISTANCE, SOUL_MID_Y, scene_z)` | 西向き側面視 |
| South | `(x, SOUL_MID_Y, scene_z - ELEV_DISTANCE)` | 北向き側面視 |
| West | `(x - ELEV_DISTANCE, SOUL_MID_Y, scene_z)` | 東向き側面視 |

### 仰角（TopDown）調整

J/K で `state.view_height`、U/I で `state.z_offset` を変更。`sync_test_camera3d` が毎フレーム反映するため、`apply_camera` のような変更検知は不要。

`apply_composite_sprite` が TopDown 時のみ composite sprite のサイズ補正係数を計算する：`comp = view_height.hypot(z_offset) / view_height`。矢視時は `comp = 1.0`。

### 表情更新のタイミング

`apply_faces` は毎フレーム全 Soul のマテリアル UV を上書きする（Change Detection による早期リターンなし）。

⚠️ `state.is_changed()` を使って最適化しないこと。Soul の追加/削除は Commands が遅延フラッシュされるため、変更が発生したフレームにはまだ新しいエンティティが Query に見えない。`is_changed()` を戻すと、Soul を削除した次フレームで AllDifferent モードの割り当てが更新されなくなる。最大 6 体のマテリアル更新なので性能コストは無視できる。

### モーション

`hw_visual::soul::idle` の `idle_visual_system` を参考にした time ベースの Transform 操作。GLB アニメーション（[Q]）と Transform モーション（[M]）は独立して動作する。

### 表情アトラス UV 計算

`soul_face_atlas.png` (768x512, 3x2 グリッド) から crop + magnification 補正付きで UV を算出。`visual_handles.rs` と同じ定数・計算式を使用。

| キー | (col,row) | 表情 |
|:---|:---|:---|
| `1` | (0,0) | Normal |
| `2` | (1,0) | Fear |
| `3` | (2,0) | Exhausted |
| `4` | (0,1) | Concentration |
| `5` | (1,1) | Happy |
| `6` | (2,1) | Sleep |

## ファイル

| ファイル | 内容 |
|:---|:---|
| `crates/bevy_app/examples/visual_test.rs` | テストシーン本体 |
| `crates/bevy_app/Cargo.toml` | `[[example]]` 登録 |
| `.cargo/config.toml` | `cargo visual-test` エイリアス |

## ゲーム本体との対応

| example 内の型 | ゲーム本体の対応 |
|:---|:---|
| `Camera3dRtt` (ローカル定義) | `plugins::startup::Camera3dRtt` |
| `TestMainCamera` + `PanCamera` | `hw_ui::camera::MainCamera` + `PanCamera` |
| `sync_test_camera3d` | `systems::visual::camera_sync::sync_camera3d_system` |
| `TestElevDir` / `TestElev` | `systems::visual::elevation_view::ElevationDirection` / `ElevationViewState` |
| `apply_composite_sprite` | 同等のロジックはゲーム本体では startup 時の固定サイズ設定 |
| `TestSoulConfig` | `CharacterHandles` (全 Soul 共通) → example は per-entity |
| `SoulAnimHandle` | `SoulAnimationPlayer3d` + `SoulAnimationLibrary` |
| `on_soul_scene_ready` | `apply_soul_gltf_render_layers_on_ready` |
| face UV 定数 | `plugins::startup::visual_handles` の `SOUL_FACE_*` 定数群 |
