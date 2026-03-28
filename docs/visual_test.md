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
| 複数エンティティ干渉 | Soul を最大 6 体まで追加し、矢印キーで移動させて重なり・Z-fight を確認 |
| Per-entity マテリアル | 各 Soul が独立した `Handle<CharacterMaterial>` を持ち、個別に表情を変更可能 |
| RtT パイプライン | Camera3d → オフスクリーンテクスチャ → composite sprite の描画経路 |

## 操作

| キー | 操作 |
|:---|:---|
| `1`-`6` | 表情切り替え (1=Normal, 2=Fear, 3=Exhausted, 4=Concentration, 5=Happy, 6=Sleep) |
| `A` | 全表情モード — 各 Soul に異なる表情を自動割当 |
| `+` / `=` | Soul 追加 (最大 6) |
| `-` | Soul 削除 (非選択のものから優先) |
| `M` | モーション切り替え (Idle → FloatingBob → Sleeping → Resting → Escaping → Dancing) |
| `R` | 全 Soul の位置・回転・スケールをリセット |
| `←` `→` `↑` `↓` | 選択 Soul を移動 |
| `Tab` | Soul 選択を順番に切り替え |
| `Esc` | 終了 |

## アーキテクチャ

ゲーム本体の `GameAssets` / `StartupPlugin` には依存せず、必要最小限のアセット（`soul.glb`, `soul_face_atlas.png`, 1x1 白テクスチャ）だけを自前で読み込む自己完結型 example。

### RtT パイプライン（ゲーム本体と同構成）

```
Camera3d (LAYER_3D, order=-1) → オフスクリーンテクスチャ
    ↓
Composite Sprite (LAYER_OVERLAY) ← Camera2d (order=0) がスクリーンに描画
```

### GLB マテリアル差し替え

`SceneInstanceReady` Observer が GLB 読み込み完了後に子孫を走査し、メッシュ名で face/body を判別してマテリアルを差し替える。ゲーム本体の `apply_soul_gltf_render_layers_on_ready` と同等のロジック。

| メッシュ名 | 差し替え先 | 追加処理 |
|:---|:---|:---|
| `Soul_Face_Mesh` | `CharacterMaterial::face(...)` | `SOUL_FACE_SCALE_MULTIPLIER` でスケール補正 |
| `Soul_Mesh.010` | `CharacterMaterial::body(...)` | — |

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

### 表情更新のタイミング

`apply_faces` は毎フレーム全 Soul のマテリアル UV を上書きする（Change Detection による早期リターンなし）。

⚠️ `state.is_changed()` を使って最適化しないこと。Soul の追加/削除は Commands が遅延フラッシュされるため、変更が発生したフレームにはまだ新しいエンティティが Query に見えない。`is_changed()` を戻すと、Soul を削除した次フレームで AllDifferent モードの割り当てが更新されなくなる。最大 6 体のマテリアル更新なので性能コストは無視できる。

### モーション

`hw_visual::soul::idle` の `idle_visual_system` を参考にした time ベースの Transform 操作。

| モード | 動作 |
|:---|:---|
| Idle | 静止（回転・スケールをリセット） |
| FloatingBob | Y 方向の伸縮 + Z 軸の揺れ |
| Sleeping | 45 度傾斜 + 微小な呼吸スケール |
| Resting | 45 度傾斜 + 縮小スケール |
| Escaping | 微小な傾き + パニックパルス |
| Dancing | 大きな Z 軸揺れ + Y 方向のバウンス |

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
| `TestSoulConfig` | `CharacterHandles` (全 Soul 共通) → example は per-entity |
| `on_soul_scene_ready` | `apply_soul_gltf_render_layers_on_ready` |
| face UV 定数 | `plugins::startup::visual_handles` の `SOUL_FACE_*` 定数群 |
