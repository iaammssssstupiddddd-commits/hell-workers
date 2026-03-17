# SectionMaterial 採用提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| 提案ID | `section-material-proposal-2026-03-16` |
| ステータス | `Accepted` |
| 作成日 | `2026-03-16` |
| 最終更新日 | `2026-03-17` |
| 作成者 | Claude Sonnet 4.6 |
| 関連ロードマップ | `docs/plans/3d-rtt/milestone-roadmap.md` |
| 関連提案 | `docs/proposals/3d-rtt/phase2-hybrid-rtt-plan-2026-03-15.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260317/character-3d-rendering-proposal-2026-03-16.md` |
| 依存完了済み | Phase 2 全MS（MS-2A〜MS-2D, MS-Elev） |
| 実装対象フェーズ | Phase 3（GLB置換と同時） |

---

## 1. 目的

### 解決したい課題

矢視モード（`ElevationViewState`）において、Camera3d を真横に向けると建物の存在しない列の床タイルが奥まで全て描画される。これにより以下の問題が発生する。

- 視覚的問題：床タイルが画面大半を占め、建物の断面が見づらい
- パフォーマンス問題：矢視時にFrustum Cullingが深さ方向に効かず、全床タイルが描画対象になる
- 将来問題：Phase 3 で床をGLBに置き換えた後、矢視時の描画オブジェクト数が急増する

### 到達したい状態

建築CADの断面図（Section Cut）と同等の表現を実現する。

```
ワールドマップ上に切断線を引く
  ↓
切断線から奥方向に「スラブ（厚み）」を設定（例：5マス分）
  ↓
スラブ内に存在するオブジェクトのみ描画
  ↓
スラブ外の床・壁・オブジェクトはGPUレベルでクリップ
```

床描画問題はクリップ平面によって自然に解決される（`Visibility::Hidden` の一括処理が不要になる）。

### なぜ Phase 3 と同時か

断面図はマテリアルレベルの機能であり、実装タイミングによってコストが変わる。

| タイミング | 追加工数 |
| --- | --- |
| Phase 3 と同時（GLB実装時） | カスタムマテリアル設計のみ（ゼロ追加） |
| Phase 3 完了後 | 全 BuildingType のマテリアル型差し替えが発生 |

Phase 3 で全 BuildingType を GLB に置き換えるとき、`StandardMaterial` ではなく最初から `SectionMaterial` として設計することで、後からの全マテリアル差し替えコストを回避する。

---

## 2. スコープ

### 対象（In Scope）

- `SectionMaterial` カスタムマテリアルの実装（WGSLクリップ平面対応）
- `SectionCut` リソースの定義（切断線の位置・法線・スラブ厚み）
- Phase 3 の `Building3dHandles` を `StandardMaterial` から `SectionMaterial` に変更
- 矢視時の切断線UI（ワールドマップ上でのドラッグ配置）
- スラブ厚みスライダー

### 非対象（Out of Scope）

- 矢視モード自体のカメラ制御（Phase 2 MS-Elev 完了済み）
- 床・壁の GLB 化（Phase 3 本体のスコープ）
- 複数断面設定の保存・管理（初期実装は単一断面のみ）
- ズームアウト時のアウトライン無効化（別途レンダリング設計で対応）

---

## 3. 技術設計

### 3.1 クリップ平面によるスラブ実装

WebGPU の `clip_distances` を使用し、2枚のクリップ平面でスラブを定義する。

```
切断線（SectionCut.position）
  │
  │← cut_normal の方向を「奥」とする
  │
  ▼
  ┌─────────────────────┐
  │                     │ ← スラブ（thickness マス分）
  └─────────────────────┘
  
clip_distances[0] = dot(world_pos - cut_position, cut_normal)
  → 0未満 = 切断線より手前 → クリップ（非表示）

clip_distances[1] = thickness - dot(world_pos - cut_position, cut_normal)
  → 0未満 = スラブより奥 → クリップ（非表示）
```

クリップはラスタライズ段階で行われる。頂点シェーダーは実行されるが、スラブ外のピクセルシェーダーはスキップされるため、スラブ幅を狭くするほど描画コストは削減方向に働く。

### 3.2 SectionCut リソース

```rust
/// ワールドマップ上の切断線を表すリソース。
/// 矢視モード中のみ有効。
#[derive(Resource, Default)]
pub struct SectionCut {
    /// 切断線の3D位置（ワールド座標）
    pub position: Vec3,
    /// 切断方向の法線ベクトル（正規化済み）
    /// 北向き矢視なら Vec3::NEG_Z、東向きなら Vec3::NEG_X
    pub normal: Vec3,
    /// スラブの厚み（ワールド単位。TILE_SIZE の倍数を推奨）
    pub thickness: f32,
    /// 切断線が有効かどうか（矢視モード外では false）
    pub active: bool,
}
```

矢視方向が変わると `normal` を自動更新するシステムを `camera_sync.rs` の延長に実装する。

```rust
fn sync_section_cut_normal(
    elevation: Res<ElevationViewState>,
    mut cut: ResMut<SectionCut>,
) {
    cut.normal = match *elevation {
        ElevationViewState::North => Vec3::NEG_Z,
        ElevationViewState::South => Vec3::Z,
        ElevationViewState::East  => Vec3::NEG_X,
        ElevationViewState::West  => Vec3::X,
        ElevationViewState::TopDown => {
            cut.active = false;
            return;
        }
    };
    cut.active = true;
}
```

### 3.3 SectionMaterial カスタムマテリアル

```rust
/// SectionMaterial の全 uniform フィールドを packed した ShaderType 構造体。
/// WGSL 側では `@group(2) @binding(0)` の単一バッファとして受け取る。
/// ※ CurtainMaterial が使う `SectionUniforms`（クリップ平面のみ）とは別物。
///   名前衝突を避けるため、こちらは `SectionMaterialUniforms` とする。
#[derive(Clone, ShaderType)]
pub struct SectionMaterialUniforms {
    pub base_color:     LinearRgba,  // vec4<f32>
    pub cut_position:   Vec4,        // Vec3 + padding
    pub cut_normal:     Vec4,        // Vec3 + padding
    pub thickness:      f32,
    pub cut_active:     f32,
    pub build_progress: f32,         // §8.3: 0.0〜1.0。completed 層の施工進捗
    pub wall_height:    f32,         // §8.3: 壁の総高さ（ワールド単位）。0.0 のとき施工クリップ無効
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct SectionMaterial {
    /// 全 uniform フィールドを単一バインディングにまとめた packed struct
    #[uniform(0)]
    pub uniforms: SectionMaterialUniforms,

    /// テクスチャ（Phase 3 GLBのベースカラー）
    #[texture(1)]
    #[sampler(2)]
    pub base_color_texture: Option<Handle<Image>>,
}

impl Material for SectionMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/section_material.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/section_material.wgsl".into()
    }
    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // clip_distances の有効化はパイプライン側では不要。
        // デバイス機能として WgpuFeatures::CLIP_DISTANCES の有効化が必要（§5 MS-Section-A 参照）。
        Ok(())
    }
}
```

> **`WgpuFeatures::CLIP_DISTANCES` の有効化**: `@builtin(clip_distances)` を使用するには、
> App 初期化時に以下の設定が必要。これがないとシェーダーコンパイルエラーまたは無音でクリップが無視される。
>
> ```rust
> App::new()
>     .add_plugins(DefaultPlugins.set(RenderPlugin {
>         render_creation: RenderCreation::Automatic(WgpuSettings {
>             features: WgpuFeatures::CLIP_DISTANCES,
>             ..default()
>         }),
>         ..default()
>     }))
> ```
>
> Linux（Vulkan バックエンド）では `CLIP_DISTANCES` はサポートされているが、
> MS-Section-A の着手前に `cargo check` + 実機確認で動作を保証すること。
```

### 3.4 WGSLシェーダー（section_material.wgsl）

§8.3 の `build_progress` / `wall_height` ユニフォームを含む完全版。`_pad0` / `_pad1` は `build_progress` と `wall_height` に置き換えており、構造体のアライメント（16バイト境界）は維持されている。

```wgsl
#import bevy_pbr::mesh_functions::get_world_from_local
#import bevy_pbr::mesh_view_bindings::view

// Rust 側の SectionMaterialUniforms に対応（§3.3 参照）。
// SectionUniforms という名前は CurtainMaterial のクリップ平面専用構造体（character-3d-rendering §8.3）と衝突するため、
// section_material.wgsl 内では SectionMaterialUniforms を使用する。
struct SectionMaterialUniforms {
    base_color:      vec4<f32>,
    cut_position:    vec4<f32>,
    cut_normal:      vec4<f32>,
    thickness:       f32,
    cut_active:      f32,
    build_progress:  f32,   // 0.0〜1.0（completed 層のみ。blueprint 層は wall_height=0.0 を渡す）
    wall_height:     f32,   // 壁の総高さ（ワールド単位）。0.0 のときは施工クリップ無効
}

@group(2) @binding(0) var<uniform> material: SectionMaterialUniforms;
@group(2) @binding(1) var base_texture: texture_2d<f32>;
@group(2) @binding(2) var base_sampler: sampler;

struct VertexOutput {
    @builtin(position)         clip_position: vec4<f32>,
    @builtin(clip_distances)   clip_distances: array<f32, 3>,
    @location(0)               world_position: vec3<f32>,
    @location(1)               uv: vec2<f32>,
}

@vertex
fn vertex(
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
) -> VertexOutput {
    var out: VertexOutput;

    let world_pos = (get_world_from_local(instance_index) * vec4<f32>(position, 1.0)).xyz;
    out.clip_position = view.clip_from_world * vec4<f32>(world_pos, 1.0);
    out.world_position = world_pos;
    out.uv = uv;

    // セクションカットクリップ（矢視モード）
    let dist = dot(world_pos - material.cut_position.xyz, material.cut_normal.xyz);
    if material.cut_active > 0.5 {
        out.clip_distances[0] = dist;                         // 手前カット
        out.clip_distances[1] = material.thickness - dist;   // 奥カット
    } else {
        out.clip_distances[0] = 1.0;  // 常に表示
        out.clip_distances[1] = 1.0;
    }

    // 施工進捗クリップ（completed 層のみ。blueprint 層は wall_height=0.0 を渡してスキップ）
    if material.wall_height > 0.0 {
        let progress_boundary = material.wall_height * material.build_progress;
        out.clip_distances[2] = progress_boundary - world_pos.y;  // y > boundary をクリップ（下から生える）
    } else {
        out.clip_distances[2] = 1.0;  // 常に表示
    }

    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(base_texture, base_sampler, in.uv);
    return tex_color * material.base_color;
}
```

### 3.5 Building3dHandles の変更（Phase 3 設計への影響）

Phase 3 の `Building3dHandles` は `StandardMaterial` ではなく `SectionMaterial` を使用する。

> **キャラクターハンドルの分離（`character-3d-rendering-proposal` 反映）**:
> キャラクターは `CharacterMaterial`（`AlphaMode::Blend`）を使用するため、`Building3dHandles` から分離した `CharacterHandles` リソースで管理する。
> クリップ平面は `section_clip.wgsl` 共通モジュール経由で同期するため、`Building3dHandles` に `character_material` フィールドは持たせない。

```rust
// Phase 3 版（変更後）
/// 建物専用ハンドル（SectionMaterial のみ）
#[derive(Resource)]
pub struct Building3dHandles {
    // メッシュ（GLB由来）
    pub wall_mesh:          Handle<Mesh>,
    pub floor_mesh:         Handle<Mesh>,
    pub door_mesh:          Handle<Mesh>,
    pub equipment_mesh:     Handle<Mesh>,

    // マテリアル（SectionMaterial）← Phase 2 の StandardMaterial から変更
    pub wall_material:      Handle<SectionMaterial>,
    pub floor_material:     Handle<SectionMaterial>,
    pub door_material:      Handle<SectionMaterial>,
    pub equipment_material: Handle<SectionMaterial>,
}

/// キャラクター専用ハンドル（CharacterMaterial）← Building3dHandles から分離
/// 詳細は character-3d-rendering-proposal §3.7 参照
#[derive(Resource)]
pub struct CharacterHandles {
    pub soul_mesh:          Handle<Mesh>,
    pub familiar_mesh:      Handle<Mesh>,
    pub soul_material:      Handle<CharacterMaterial>,
    pub familiar_material:  Handle<CharacterMaterial>,
}
```

`MeshMaterial3d<StandardMaterial>` を使っていた全 spawn 箇所を `MeshMaterial3d<SectionMaterial>` に置き換える。

### 3.6 SectionCut → SectionMaterial / CharacterMaterial への毎フレーム同期

`SectionCut` リソースの値を全マテリアルインスタンスに伝播するシステムを追加する。

**建物用（`SectionMaterial`）**

```rust
fn sync_section_cut_to_materials(
    cut: Res<SectionCut>,
    handles: Res<Building3dHandles>,
    mut materials: ResMut<Assets<SectionMaterial>>,
) {
    if !cut.is_changed() { return; }

    let update = |mat: &mut SectionMaterial| {
        mat.uniforms.cut_position = cut.position.extend(0.0);
        mat.uniforms.cut_normal   = cut.normal.extend(0.0);
        mat.uniforms.thickness    = cut.thickness;
        mat.uniforms.cut_active   = if cut.active { 1.0 } else { 0.0 };
    };

    for handle in [
        &handles.wall_material,
        &handles.floor_material,
        &handles.door_material,
        &handles.equipment_material,
    ] {
        if let Some(mat) = materials.get_mut(handle.id()) {
            update(mat);
        }
    }
}
```

**キャラクター用（`CharacterMaterial`）**

キャラクターへのクリップ平面伝播は `character-3d-rendering-proposal` §3.5 で定義する別システム（`sync_section_cut_to_character_materials`）が担う。`section_clip.wgsl` 共通モジュールにより `SectionMaterial` と同一の切断値が適用される。

```rust
// character_material.rs に実装（Building3dHandles とは独立）
fn sync_section_cut_to_character_materials(
    cut: Res<SectionCut>,
    handles: Res<CharacterHandles>,
    mut materials: ResMut<Assets<CharacterMaterial>>,
) {
    if !cut.is_changed() { return; }
    // cut の値を CharacterMaterial の cut_position / cut_normal / thickness / cut_active に伝播
}
```

`SectionCut` が変化したフレームのみ実行されるため、通常フレームのオーバーヘッドはゼロに近い。

---

## 4. UI設計

### 4.1 切断線の配置操作

既存の `AreaEditSession` に類似したモードとして実装する。

```
操作フロー

  矢視モードに入る
    ↓
  ワールドマップ上でクリック（切断線の起点）
    ↓
  ドラッグで切断線の長さを確定
  （矢視方向に対して垂直にスナップ）
    ↓
  マウスボタンを離す → 切断線確定・矢視に切替
```

初期実装では、矢視方向に対して切断線は常に垂直（スナップ固定）とする。これにより `SectionCut.normal` は矢視方向から自動決定され、ユーザーが法線を意識する必要がない。

### 4.2 スラブ厚みの調整

矢視モード中、画面端にスライダーを表示する。

| 設定 | 初期値 | 範囲 |
| --- | --- | --- |
| スラブ厚み | `TILE_SIZE * 5`（5マス） | 1〜20マス |

`bevy_egui` の既存UIスタックに追加する。厚みを変更すると `SectionCut.thickness` が即座に更新され、`sync_section_cut_to_materials` が次フレームで伝播する。

---

## 5. 実装計画（マイルストーン）

### MS-Section-A: SectionMaterial 基盤実装

> **依存**: Phase 3 着手（GLB取込インフラが存在すること）

**やること**:
0. `WgpuFeatures::CLIP_DISTANCES` の動作確認（**着手前 P0**）：`bevy_app` の `RenderPlugin` 設定に `WgpuFeatures::CLIP_DISTANCES` を追加し、`cargo check` + 実機起動で動作を確認する
1. `hw_visual/src/material/section_material.rs` を新規作成（`SectionMaterial`・`SectionCut`）
2. `shaders/section_material.wgsl` を新規作成（§3.4 の完全版WGSLシェーダー）
3. `MaterialPlugin::<SectionMaterial>` を `HwVisualPlugin` に追加
4. `sync_section_cut_normal` システムを実装・登録（`ElevationViewState` 変化時に `SectionCut.normal` を更新）
5. `sync_section_cut_to_materials` システムを実装・登録（`SectionCut` 変化時にマテリアル伝播）

**完了条件**:
- `cargo check` ゼロエラー
- `WgpuFeatures::CLIP_DISTANCES` が有効化された状態で起動できること
- `SectionMaterial` をアタッチした単純なCuboidが、`SectionCut.active = true` のとき指定スラブ外でクリップされること（目視確認）
- `SectionCut.active = false` のとき通常通り全体が描画されること（目視確認）

---

### MS-Section-B: Building3dHandles の SectionMaterial 移行

> **依存**: MS-Section-A 完了、Phase 3 GLB 取込完了

**やること**:
1. `visual_handles.rs` の `Building3dHandles` を `SectionMaterial` ベースに変更
2. `building_completion/spawn.rs` の全 `MeshMaterial3d<StandardMaterial>` を `MeshMaterial3d<SectionMaterial>` に置き換え
3. 設備別 visual system（`tank.rs`・`mud_mixer.rs` 等）の同様置き換え
4. Phase 3 で新規追加する全 BuildingType の spawn 時点から `SectionMaterial` を使用

**完了条件**:
- `cargo check` ゼロエラー
- 矢視モードで切断線を設定したとき、全 BuildingType のスラブ外部分がクリップされること（目視確認）
- トップダウンモード（`cut_active = false`）で全建物が正常表示されること

---

### MS-Section-C: 切断線UI実装

> **依存**: MS-Section-B 完了

**やること**:
1. `SectionCutEditSession` コンポーネントを `hw_ui` に定義
2. 矢視モード入時に切断線配置モードを自動起動する処理を `camera_sync.rs` に追加
3. ワールドマップ上のクリック・ドラッグで `SectionCut.position` を更新する入力システムを実装
4. スラブ厚みスライダーを `bevy_egui` UIに追加
5. 切断線のワールドマップ上プレビュー表示（2D Gizmo）を実装

**完了条件**:
- 矢視モードでクリック・ドラッグにより切断線を配置できること
- スラブ厚みスライダーを動かすと即座に3D描画が変化すること
- 切断線のプレビューがワールドマップ上に表示されること

---

## 6. 影響ファイル一覧

| ファイル | 変更種別 | 内容 |
| --- | --- | --- |
| `hw_visual/src/material/section_material.rs` | 新規 | `SectionMaterial`・`SectionCut` 定義 |
| `assets/shaders/section_material.wgsl` | 新規 | WGSLシェーダー（クリップ平面） |
| `hw_visual/src/lib.rs` | 変更 | `MaterialPlugin::<SectionMaterial>` 追加 |
| `hw_visual/src/visual_handles.rs` | 変更 | `Building3dHandles` のマテリアル型変更 |
| `systems/visual/camera_sync.rs` | 変更 | `sync_section_cut_normal` システム追加 |
| `building_completion/spawn.rs` | 変更 | `MeshMaterial3d` 型変更 |
| `systems/visual/tank.rs` | 変更 | `MeshMaterial3d` 型変更 |
| `systems/visual/mud_mixer.rs` | 変更 | `MeshMaterial3d` 型変更 |
| `hw_ui/src/section_cut_ui.rs` | 新規 | 切断線UI・スライダー |

---

## 7. パフォーマンス考慮

### GTX 1650 での追加コスト

| 処理 | 追加コスト |
| --- | --- |
| クリップ平面（2面） | ラスタライザ段階での判定。頂点シェーダー後に処理され追加ms は測定誤差レベル |
| `sync_section_cut_to_materials` | `SectionCut` 変化時のみ実行。通常フレームはゼロ |
| スラブ外ピクセルスキップ | 描画コストは削減方向（スラブ幅を狭くするほど軽くなる） |

矢視時の床タイル全描画問題（現状：最大100列×全幅）が解消されるため、Phase 3 以降の矢視モードは `Visibility::Hidden` 対応よりも軽くなる見込み。

### 品質設定との連携

既存の高/中/低品質設定において、スラブ幅の初期値を変更することでパフォーマンスを調整できる。

| 品質設定 | スラブ初期幅 |
| --- | --- |
| 高 | 10マス |
| 中 | 5マス（デフォルト） |
| 低 | 3マス |

---

## 8. 壁メッシュの層構造設計

### 8.1 概要

壁GLBは `blueprint`（仮設）層と `completed`（本設）層の2層を1ファイル内に物理的に持つ構造を採用する方向で検討中。

```
wall.glb の断面イメージ
  ┌─────────────────┐
  │  completed 層   │ ← 外側・build_progress でクリップ
  │  ┌───────────┐  │
  │  │blueprint層│  │ ← 内側・常に全高表示
  │  └───────────┘  │
  └─────────────────┘
```

### 8.2 利点

**実装がシンプルになる。** `completed` 層を `build_progress` でクリップするだけでよく、`blueprint` 層はクリップ制御不要。エンティティは1つ、クリップ制御も1軸のみ。

**断面図がリッチになる。** セクションビューで壁を切断すると `completed` 層の内側に `blueprint` 層が見える。施工中の壁を切断すれば型枠・内部構造が自然に現れる。これはシェーダーで作り出すのではなくジオメトリとして存在するため、どの角度で切断しても成立する。

**アートの制御権がアーティスト側に集約される。** blueprint の見た目・completed の厚み・層の間隔は全てモデリング側で決まる。シェーダーは `build_progress` のクリップだけを担う。

### 8.3 build_progress クリップ

`build_progress` と `wall_height` は §3.3 の `SectionMaterialUniforms` に統合済み。
`SectionMaterial.uniforms.build_progress` / `.wall_height` でアクセスする。

> **§3.3 との統合理由**: Bevy の `AsBindGroup` は `#[uniform(N)]` 個別指定で各フィールドを別バインディングに展開する。
> WGSL 側は単一 packed struct（`@binding(0)`）を期待するため、全フィールドを `SectionMaterialUniforms: ShaderType` にまとめて `#[uniform(0)]` で渡す設計に統一した。

```wgsl
// completed 層にのみ適用
let progress_boundary = material.wall_height * material.build_progress;
clip_distances[2] = progress_boundary - world_pos.y;
// world_pos.y > progress_boundary の部分をクリップ → 下から生えてくる表現
```

`blueprint` 層は `clip_distances[2] = 1.0`（常に表示）。

### 8.4 状態遷移

| 状態 | blueprint 層 | completed 層 |
| --- | --- | --- |
| 仮設（着工前） | 全高表示 | progress=0（非表示） |
| 施工中 | 全高表示（上部が completed に覆われていく） | progress 0→1（下から上昇） |
| 完成 | completed に完全に覆われる | 全高表示 |

エンティティの spawn/despawn は建物の完成・撤去時のみ発生する。着工・完成といった施工イベントはマテリアルパラメータの変更のみで表現できる。

### 8.5 パフォーマンスへの影響（未解決・要PoC計測）

2層構造により壁1個あたりのポリゴン数が約2倍になる。ポリゴン予算は全オブジェクトの合計に対して設定されているため無視できない影響がある。

```
通常プレイ時の予算（LOD1中心）：30,000三角形（全オブジェクト合計）

壁を2層にした場合の試算
  壁（2層）   ：10,000〜20,000
  床・設備    ：6,000〜10,000
  キャラクター：5,000〜10,000
  合計        ：21,000〜40,000  ← 上限超過の可能性あり
```

以下の対処案を PoC で検証する。

| 対処案 | 内容 | トレードオフ |
| --- | --- | --- |
| LOD0のみ2層 | LOD1・LOD2は `completed` 層のみ | ズームアウト時に blueprint 層が消える |
| LOD1のポリゴン削減 | 壁LOD1を100→50三角形に削減 | Blender品質ゲートの閾値変更が必要 |

**現時点では採用方向で検討中・PoC でポリゴン予算への影響を実測してから確定する。**

---

## 9. 未解決事項（Pending）

| 項目 | 優先度 | タイミング |
| --- | --- | --- |
| `WgpuFeatures::CLIP_DISTANCES` の有効化と Bevy 0.18 + wgpu Vulkan バックエンドでの動作確認 | P0 | MS-Section-A 着手前に検証（§3.3 の App 設定例を参照） |
| 壁メッシュ2層構造のポリゴン予算影響の実測 | P0 | Phase 3 GLB 取込 PoC と同時 |
| 断面キャップの実装方針決定（下記参照） | P1 | Phase 3 着手前に決定 |
| ~~キャラクターへのクリップ適用方法~~ | ~~P1~~ | **解消（方針変更あり）**：`character-3d-rendering-proposal` 採用によりキャラクターは GLB + `CharacterMaterial` で実装する。`section_clip.wgsl` 共通モジュール経由でクリップ平面を適用する（「常に表示」方針は上書き済み）。詳細は `character-3d-rendering-proposal` §3.4・§3.5 参照。 |
| 複数断面設定の保存（方針B）の採用検討 | P2 | MS-Section-C 完了後に判断 |

### 断面キャップの実装方針

GLBメッシュは表面ポリゴンのみで構成されており、内部は空洞である。クリップ平面で切断すると切断面に「穴」が開く。この穴をどう扱うかはシェーダー・レンダリング側（Bevy）の責務であり、以下の3方針から選択する。

なお壁メッシュが2層構造を採用した場合、断面には `completed` 層の内側に `blueprint` 層が見えるためジオメトリとして内部表現が生まれる。方針Cでも空洞感が軽減される可能性がある。

| 方針 | 内容 | 実装コスト | 見た目 |
| --- | --- | --- | --- |
| A：断面キャップ描画 | クリップ平面の位置にステンシルバッファを使い単色または断面テクスチャを塗りつぶす | 中 | 詰まって見える（建築CAD標準） |
| B：動的キャップ生成 | クリップ平面上にポリゴンを動的生成して貼る | 高 | 最もリッチ・断面テクスチャを貼れる |
| C：何もしない | 切断面は穴が開いたまま。内側の面が見える | 低 | 2層構造のジオメトリが見える |

方針AはステンシルバッファをBevyのレンダーパスに統合する実装で、`SectionMaterial` のフラグメントシェーダーとは別に専用のパスが必要になる。方針Bはさらに複雑でGeometry Shaderが使えないwgpuでは実現が難しい。

**アートスタイル受入基準の確定と同時に方針を決定する（P1）。** 壁メッシュ2層構造のPoC結果を見てから判断する。

---

## 10. 決定事項サマリ

| 決定内容 | 日付 |
| --- | --- |
| SectionMaterial を Phase 3 の全 BuildingType に採用する | 2026-03-16 |
| StandardMaterial は Phase 2 仮実装（Cuboid）のみで使用し、Phase 3 GLB 実装時に SectionMaterial に移行する | 2026-03-16 |
| 切断線は矢視方向に対して垂直スナップ固定（初期実装） | 2026-03-16 |
| スラブ厚みの初期値は 5マス（調整可能） | 2026-03-16 |
| 断面キャップの実装方針（A/B/C） | 未決定（アートスタイル受入基準・PoC結果確定後に決定） |
| 壁GLBは blueprint 層と completed 層の2層構造を採用する方向で検討中 | 2026-03-16 |
| 2層構造の採用確定はPoC でのポリゴン予算実測後とする | 2026-03-16 |
| 施工進捗は `build_progress` ユニフォームによる縦方向クリップで表現する | 2026-03-16 |
