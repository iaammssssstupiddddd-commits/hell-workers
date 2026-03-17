# MS-Asset-Shader 実装計画：section_material.wgsl 事前作成

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `ms-asset-shader-plan` |
| ステータス | `Done` |
| 作成日 | `2026-03-18` |
| 関連アセットMS | `asset-milestones-2026-03-17.md` MS-Asset-Shader |
| 関連コードMS | `phase3-implementation-plan-2026-03-16.md` M-3-3 |
| 関連提案 | `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260317/character-3d-rendering-proposal-2026-03-16.md` |
| 依存 | なし（今すぐ着手可） |

---

## 目的

`section-material-proposal` §3.4 で確定した WGSL を `assets/shaders/section_material.wgsl` として先行配置する。

M-3-3（SectionMaterial 基盤実装）では Rust 側（`section_material.rs`）の実装が主作業になるが、シェーダーファイルが存在しないと `cargo check` すら通らない。先に配置することで M-3-3 の実装コストをほぼゼロにする。

---

## 作成するファイル

| ファイル | 種別 |
| --- | --- |
| `assets/shaders/section_material.wgsl` | 新規作成 |

**今回のスコープ外**（M-3-1 以降で作成）:
- `assets/shaders/common/section_clip.wgsl`（CharacterMaterial と共有するクリップ平面モジュール）
- `assets/shaders/character_material.wgsl`

---

## 実装ステップ

### Step 1: ファイル作成

`assets/shaders/section_material.wgsl` を以下の内容で作成する。

```wgsl
#import bevy_pbr::mesh_functions::get_world_from_local
#import bevy_pbr::mesh_view_bindings::view

// Rust 側の SectionMaterialUniforms に対応（section-material-proposal §3.3 参照）。
// SectionUniforms という名前は CurtainMaterial のクリップ平面専用構造体
// （character-3d-rendering §8.3）と衝突するため、
// section_material.wgsl 内では SectionMaterialUniforms を使用する。
struct SectionMaterialUniforms {
    base_color:      vec4<f32>,   // LinearRgba → vec4<f32>
    cut_position:    vec4<f32>,   // Vec3 + padding
    cut_normal:      vec4<f32>,   // Vec3 + padding
    thickness:       f32,
    cut_active:      f32,
    build_progress:  f32,         // 0.0〜1.0（completed 層のみ。blueprint 層は wall_height=0.0 を渡す）
    wall_height:     f32,         // 壁の総高さ（ワールド単位）。0.0 のとき施工クリップ無効
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

### Step 2: 検証

WGSL ファイル単体では `cargo check` での検証ができないため、以下を目視で確認する。

| チェック項目 | 根拠 |
| --- | --- |
| `SectionMaterialUniforms` のフィールド順が Rust `SectionMaterialUniforms` と一致する | GPU バッファのオフセットが一致しないと値が化ける（M-3-3 で作成する `section_material.rs` §3.3 を参照） |
| `@binding(0)`=uniforms、`@binding(1)`=texture、`@binding(2)`=sampler | Rust `AsBindGroup` の `#[uniform(0)]`・`#[texture(1)]`・`#[sampler(2)]` と対応 |
| `clip_distances: array<f32, 3>` | セクションカット×2 + 施工進捗×1 の合計3面 |
| `@builtin(clip_distances)` が `VertexOutput` に含まれている | 頂点シェーダーで書き込む。フラグメントシェーダーでは不要 |
| `SectionMaterialUniforms` 構造体名を使用している（`SectionUniforms` ではない） | `SectionUniforms` は `CurtainMaterial` 専用（`section-material-proposal §3.3`・`character-3d-rendering §8.3`）。名前衝突を避けるため |

### Step 3: M-3-3 着手前の最終確認

M-3-3 で `section_material.rs` を実装するとき、以下の対応が崩れていないか再確認する。

```
Rust struct field          WGSL struct field          GPU binding
─────────────────────────────────────────────────────────────────
uniforms: SectionMaterialUniforms  →  SectionMaterialUniforms  @binding(0)
  .base_color:     LinearRgba      →  .base_color:   vec4<f32>
  .cut_position:   Vec4            →  .cut_position: vec4<f32>
  .cut_normal:     Vec4            →  .cut_normal:   vec4<f32>
  .thickness:      f32             →  .thickness:    f32
  .cut_active:     f32             →  .cut_active:   f32
  .build_progress: f32             →  .build_progress: f32
  .wall_height:    f32             →  .wall_height:  f32
base_color_texture: Handle<Image>  →  base_texture   @binding(1)
                                   →  base_sampler   @binding(2)
```

---

## 注意点

### アライメント

`SectionMaterialUniforms` は以下の構成で 16 バイト境界を満たしている（`section-material-proposal §3.4` より）。

| フィールド | サイズ | 累積 |
| --- | --- | --- |
| `base_color` (vec4) | 16 B | 16 B |
| `cut_position` (vec4) | 16 B | 32 B |
| `cut_normal` (vec4) | 16 B | 48 B |
| `thickness` + `cut_active` + `build_progress` + `wall_height` (f32×4) | 16 B | 64 B |

合計 64 B。16 バイト境界維持・パディング不要。

### `@builtin(clip_distances)` の使用条件

シェーダーで `clip_distances` を書き込むには、Rust 側で `WgpuFeatures::CLIP_DISTANCES` を有効化している必要がある（M-Pre1 の担当）。このファイル単体では有効化できないため、実際のクリップ動作確認は M-3-3 の完了条件チェック時に行う（`phase3-m-pre3-plan` M-Pre1 参照）。

### `section_clip.wgsl` との関係

`section_material.wgsl` はクリップ平面ロジックをインラインで持つ。M-3-1 で `CharacterMaterial` を実装するとき、クリップ部分を `assets/shaders/common/section_clip.wgsl` に切り出して `SectionMaterial` と `CharacterMaterial` の両方で `#import` する（`section-material-proposal` §3.4 / `character-3d-rendering-proposal` §3.4 参照）。今回はインライン実装のままで問題ない。

---

## 完了条件

- [x] `assets/shaders/section_material.wgsl` が存在する
- [x] `section-material-proposal` §3.4 の内容と一致している（フィールド順・バインディング番号・コメント文言）
- [x] Step 2 の目視チェック項目がすべて通っている

---

## 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-18` | Claude Sonnet 4.6 | 初版作成 |
| `2026-03-18` | Claude Sonnet 4.6 | `assets/shaders/section_material.wgsl` 作成・完了 |
