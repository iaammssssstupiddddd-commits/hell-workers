# RtT Pipeline Refactor Plan

## ステータス

- **完了**（2026-03-29）。`RttRuntime` 統合・`initialize_rtt_runtime`・`sync_rtt_output_bindings` の `runtime.is_changed()` ガードまでコードへ反映済み。
- 恒久仕様の参照先: `docs/architecture.md` の「RtT（Render-to-Texture）インフラ」。
- 以下の Problem / Goal / API 設計・手順は、**計画・実装時の整理用メモ**（当時の課題記述を含む）として残す。

## Problem

現在の RtT 実装は機能自体は成立しているが、責務が複数ファイルにまたがって分散している。

| ファイル | 内容 |
|---|---|
| `startup_systems.rs::setup()` | fallback viewport 算出・texture 2 枚生成・`RttTextures`/`RttViewportSize` 挿入・`Camera3dRtt`/`Camera3dSoulMaskRtt` spawn |
| `rtt_setup.rs` | `RttTextures`/`RttViewportSize` 定義・`recreate_rtt_textures()`・`sync_rtt_texture_size_to_window_and_quality()` |
| `rtt_composite.rs` | `spawn_rtt_composite_sprite()`・`sync_rtt_output_bindings()` |

この構成には次の問題がある。

- `RttTextures` と `RttViewportSize` が別 Resource として管理されており、どちらか片方だけ変わっても `is_changed()` が片方しか立たない。実際には常に同時に変わるデータ。
- `startup_systems::setup()` に RtT の viewport 計算・texture 生成・resource 挿入ロジックが直書きされており、`rtt_setup.rs` と責務が被っている。
- `sync_rtt_output_bindings` はカメラの `RenderTarget` を **毎フレーム無条件で上書き**している（変化チェックなし）。合成マテリアル側は `rtt` / `viewport_size` / `logical_size` による `continue` で更新を抑えているが、カメラ行は毎フレーム通る。
- scene RtT と soul mask RtT の対称性が `RttTextures.texture_3d` / `texture_soul_mask` というフィールド名では伝わりにくい。
- `recreate_rtt_textures()` が `setup()` では使われておらず、初期生成と再生成が別コードパスを通っている。

現状では動いているが、今後 `SectionMaterial` / 追加 pass / 品質設定拡張を入れる前に整理しておく価値がある。

## Goal

RtT を「初期化」「viewport 決定」「texture 再生成」「camera target 更新」「composite binding 更新」の一連の pipeline として扱える状態にする。

到達目標:

- `RttTextures` + `RttViewportSize` を 1 つの `RttRuntime` Resource に統合する
- `startup_systems::setup()` から RtT の実装詳細（viewport 計算・texture 生成）を除去する
- 初期化と再生成が `RttRuntime::new` / `RttRuntime::recreate` として同じコードパスを通る
- `sync_rtt_output_bindings` に `runtime.is_changed()` ガードを追加し、**カメラ `RenderTarget` とマテリアル差し替え**の無駄な毎フレーム更新を解消する（`Startup` の `setup()` でカメラに初期 `RenderTarget` が付くため、最初の `Update` で早期 return しても描画経路は破綻しにくい）

## Non-Goals

- RtT の機能追加
- 新しい texture pass の導入
- composite shader の見た目変更
- camera/light の構成変更
- 旧 `Image` ハンドルが `Assets<Image>` に残る挙動の変更（現状の `recreate_rtt_textures` と同様。参照が外れたアセットの回収は従来どおり）

---

## API 設計

### `RttRuntime` Resource（`rtt_setup.rs` に追加）

```rust
/// RtT パイプラインの runtime state を一元管理する Resource。
/// 初期化・リサイズ・品質切り替えの全経路が同じ struct を更新する。
#[derive(Resource)]
pub struct RttRuntime {
    pub viewport: RttViewportSize,
    pub scene: Handle<Image>,
    pub soul_mask: Handle<Image>,
}

impl RttRuntime {
    pub fn new(viewport: RttViewportSize, images: &mut Assets<Image>) -> Self {
        Self {
            scene: create_rtt_texture(viewport.width, viewport.height, images),
            soul_mask: create_rtt_texture(viewport.width, viewport.height, images),
            viewport,
        }
    }

    pub fn recreate(&mut self, viewport: RttViewportSize, images: &mut Assets<Image>) {
        self.viewport = viewport;
        self.scene = create_rtt_texture(viewport.width, viewport.height, images);
        self.soul_mask = create_rtt_texture(viewport.width, viewport.height, images);
    }

    pub fn pixel_size(&self) -> Vec2 {
        self.viewport.pixel_size()
    }
}
```

### `initialize_rtt_runtime()` helper（`rtt_setup.rs` に追加）

```rust
/// window 解像度と quality から RttRuntime を生成して返す。
/// window が取れない場合は fallback (1280×720) を使用する。
pub fn initialize_rtt_runtime(
    window: Option<&Window>,
    quality: QualitySettings,
    images: &mut Assets<Image>,
) -> RttRuntime {
    let viewport = window
        .map(|w| RttViewportSize::from_window(w, quality))
        .unwrap_or_else(|| RttViewportSize::from_physical_size(1280, 720, quality.rtt_scale()));
    RttRuntime::new(viewport, images)
}
```

---

## 各ファイルの変更詳細

### Step 1: `rtt_setup.rs` — `RttRuntime` 追加・既存 Resource 削除

**追加:**
- `RttRuntime` struct + impl（上記 API 設計のとおり）
- `initialize_rtt_runtime()` helper

**削除:**
- `RttTextures` struct（`RttRuntime` に吸収）
- standalone Resource としての `RttViewportSize`（`RttRuntime.viewport` フィールドとして保持）
  - 型定義と impl は残す（`RttRuntime.viewport` のフィールド型として使うため）
- `recreate_rtt_textures()` 関数（`RttRuntime::recreate` に統合）

**変更:**
- `sync_rtt_texture_size_to_window_and_quality` のシグネチャを更新:

```rust
// before
pub fn sync_rtt_texture_size_to_window_and_quality(
    q_window: Query<Ref<Window>, With<PrimaryWindow>>,
    quality: Res<QualitySettings>,
    mut viewport_size: ResMut<RttViewportSize>,
    mut rtt: ResMut<RttTextures>,
    mut images: ResMut<Assets<Image>>,
)

// after
pub fn sync_rtt_texture_size_to_window_and_quality(
    q_window: Query<Ref<Window>, With<PrimaryWindow>>,
    quality: Res<QualitySettings>,
    mut runtime: ResMut<RttRuntime>,
    mut images: ResMut<Assets<Image>>,
)
```

関数本体も `runtime.recreate(next_size, &mut images)` の 1 行呼び出しに変わる。

### Step 2: `startup_systems.rs` — RtT 初期化コードの置換

**変更前（現在の `setup()` 内 51–65 行目）:**

```rust
let fallback_viewport =
    rtt_setup::RttViewportSize::from_physical_size(1280, 720, quality.rtt_scale());
let viewport_size = q_window
    .single()
    .map(|window| rtt_setup::RttViewportSize::from_window(window, *quality))
    .unwrap_or(fallback_viewport);
let rtt_handle =
    rtt_setup::create_rtt_texture(viewport_size.width, viewport_size.height, &mut images);
let soul_mask_handle =
    rtt_setup::create_rtt_texture(viewport_size.width, viewport_size.height, &mut images);
commands.insert_resource(RttTextures {
    texture_3d: rtt_handle.clone(),
    texture_soul_mask: soul_mask_handle.clone(),
});
commands.insert_resource(viewport_size);
```

**変更後:**

```rust
let runtime =
    rtt_setup::initialize_rtt_runtime(q_window.single().ok(), *quality, &mut images);
let rtt_handle = runtime.scene.clone();
let soul_mask_handle = runtime.soul_mask.clone();
commands.insert_resource(runtime);
```

`use super::rtt_setup::{self, Camera3dRtt, Camera3dSoulMaskRtt, RttTextures}` から `RttTextures` を除去する。

### Step 3: `rtt_composite.rs` — `RttRuntime` への切り替えと early-exit 最適化

**import 変更:**

```rust
// before
use crate::plugins::startup::{Camera3dRtt, Camera3dSoulMaskRtt, RttTextures, RttViewportSize};

// after
use crate::plugins::startup::{Camera3dRtt, Camera3dSoulMaskRtt, RttRuntime};
```

**`spawn_rtt_composite_sprite` シグネチャ変更:**

```rust
// before
pub fn spawn_rtt_composite_sprite(
    ...
    rtt: Res<RttTextures>,
    viewport_size: Res<RttViewportSize>,
    ...
)

// after
pub fn spawn_rtt_composite_sprite(
    ...
    runtime: Res<RttRuntime>,
    ...
)
```

内部の `rtt.texture_3d` → `runtime.scene`、`rtt.texture_soul_mask` → `runtime.soul_mask`、`viewport_size.pixel_size()` → `runtime.pixel_size()` に差し替え。

**`sync_rtt_output_bindings` のシグネチャ変更と早期リターン追加:**

現在のコードはカメラの `RenderTarget` を毎フレーム無条件上書きしている（マテリアル更新ループは条件付き `continue` あり）。変更後は `runtime.is_changed()` でカメラ・マテリアル更新ブロック全体を囲み、変化がないフレームではそこをスキップする。

```rust
pub fn sync_rtt_output_bindings(
    runtime: Res<RttRuntime>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut main_camera_targets: Query<...>,
    mut soul_mask_targets: Query<...>,
    mut quads: Query<(&MeshMaterial2d<RttCompositeMaterial>, &mut Transform), With<RttCompositeSprite>>,
    mut materials: ResMut<Assets<RttCompositeMaterial>>,
) {
    let logical_size = q_window.single().ok().map(logical_composite_size);

    // メッシュスケールはウィンドウリサイズで常時追従（RttRuntime 変化とは独立）
    for (_, mut tf) in quads.iter_mut() {
        if let Some(size) = logical_size {
            tf.scale = size.extend(1.0);
        }
        tf.translation.z = Z_RTT_COMPOSITE;
    }

    // テクスチャ参照の差し替えは RttRuntime が変化したときだけ行う
    if !runtime.is_changed() {
        return;
    }

    if let Ok(mut target) = main_camera_targets.single_mut() {
        *target = RenderTarget::Image(runtime.scene.clone().into());
    }
    if let Ok(mut target) = soul_mask_targets.single_mut() {
        *target = RenderTarget::Image(runtime.soul_mask.clone().into());
    }
    for (material_handle, _) in quads.iter() {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.scene_texture = runtime.scene.clone();
            material.soul_mask_texture = runtime.soul_mask.clone();
            material.params.pixel_size = runtime.pixel_size();
        }
    }
}
```

### Step 4: `mod.rs` — re-export 更新

```rust
// before
pub use rtt_setup::{Camera3dRtt, Camera3dSoulMaskRtt, RttTextures, RttViewportSize};

// after
pub use rtt_setup::{Camera3dRtt, Camera3dSoulMaskRtt, RttRuntime, RttViewportSize};
// RttTextures は削除（RttRuntime に統合）
// RttViewportSize は RttRuntime.viewport フィールドへのアクセスで必要なので残す
```

### Step 5 (Optional): Camera spawn helper の切り出し

`startup_systems.rs::setup()` の Camera3dRtt / Camera3dSoulMaskRtt spawn ブロックを
`rtt_setup.rs` の `spawn_rtt_cameras(commands: &mut Commands, runtime: &RttRuntime)` に移動する。

**判断基準:**
- `rtt_setup.rs` への追加 import が増える（`LAYER_3D`, `LAYER_3D_SOUL_MASK`, `VIEW_HEIGHT`, `Z_OFFSET`, `ElevationDirection` など）
- Step 1–4 完了後に改めて `setup()` の見通しを評価してから実施するかどうか決める

### Step 6: docs 更新

`docs/architecture.md` の startup / RtT セクションを新しい責務分割に合わせて更新する。

---

## 実施単位（ビルドが通る単位）

`RttTextures` / `RttViewportSize` を消して `RttRuntime` のみにすると、同じ変更セットで `rtt_setup.rs`・`startup_systems.rs`・`rtt_composite.rs`・`mod.rs` をまとめて直さないと `cargo check` が通らない。上記 Step 1〜4（Rust 側）は **1 コミット相当でまとめて適用**する想定でよい。ドキュメント（Step 6 / `docs/architecture.md`）はその後でもよい。

## Proposed Steps（実施順）

1. `rtt_setup.rs` に `RttRuntime` 追加・`initialize_rtt_runtime()` 追加
2. `rtt_setup.rs` の `sync_rtt_texture_size_to_window_and_quality` を `RttRuntime` を受けるように変更
3. `rtt_setup.rs` から `RttTextures` 削除・`recreate_rtt_textures()` 削除
4. `startup_systems.rs` の RtT 初期化ブロックを `initialize_rtt_runtime()` 呼び出しへ置換
5. `rtt_composite.rs` を `RttRuntime` 対応に更新（import, spawn, sync 各関数）
6. `mod.rs` の re-export を更新
7. `cargo check --workspace` + `cargo clippy --workspace` でクリーン確認（コマンドは Verification と同一）
8. `docs/architecture.md` 更新
9. (Optional) camera spawn helper の切り出し評価

## Files To Modify

| ファイル | 変更内容 |
|---|---|
| `crates/bevy_app/src/plugins/startup/rtt_setup.rs` | `RttRuntime` 追加・`RttTextures` 削除・helper 追加 |
| `crates/bevy_app/src/plugins/startup/rtt_composite.rs` | `RttRuntime` 使用へ切り替え・early-exit 追加 |
| `crates/bevy_app/src/plugins/startup/startup_systems.rs` | RtT 初期化ブロック置換 |
| `crates/bevy_app/src/plugins/startup/mod.rs` | re-export 更新 |
| `docs/architecture.md` | startup / RtT セクション更新 |

## Verification

```bash
# コンパイルチェック（ワークスペース全体）
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace

# Clippy ゼロ警告
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated
```

実装時メモ: `Res::<RttRuntime>::is_changed()` がフレーム境界で期待どおり立つかは Bevy 0.18 の挙動に合わせて一度確認する（挿入直後・`ResMut` 更新直後の検知）。

動作確認チェックリスト:
- [ ] 起動時に RtT が正常初期化される
- [ ] window resize で scene / soul mask の両方が追従する
- [ ] `F4` の品質切り替えで RtT 解像度が変わる
- [ ] composite 表示と Familiar 2D 前面表示に退行がない
- [ ] `sync_rtt_output_bindings` が変化なし時に早期リターンする（デバッグログで確認）

## Asset Check

このリファクタに追加アセットは不要。

- 既存 shader / texture / GLB をそのまま使う
- 必要なのは runtime resource と startup wiring の整理のみ
