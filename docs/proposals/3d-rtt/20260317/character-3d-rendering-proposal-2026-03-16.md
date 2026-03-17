# キャラクター 3D モデルレンダリング採用提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| 提案ID | `character-3d-rendering-proposal-2026-03-16` |
| ステータス | `Accepted` |
| 作成日 | `2026-03-16` |
| 最終更新日 | `2026-03-16` |
| 作成者 | Claude Sonnet 4.6 |
| 関連ロードマップ | `docs/plans/3d-rtt/milestone-roadmap.md` |
| 関連提案 | `docs/proposals/3d-rtt/phase2-hybrid-rtt-plan-2026-03-15.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/outline-rendering-proposal-2026-03-16.md` |
| 依存完了済み | Phase 2 全MS（MS-2A〜MS-2D, MS-Elev） |
| 実装対象フェーズ | Phase 3 |

---

## 1. 目的

### 解決したい課題

現在のキャラクター（Soul・Familiar）は手作業または手描きで作成したスプライトシートに依存している。この方式には以下の問題がある。

- スプライトシートの作成コストが高い（方向×状態の組み合わせ分だけ手作業が発生する）
- アニメーション追加のたびに再作業が必要
- スプライトシートの容量が大きくなる
- 矢視モードで横から見た表現ができない

### 到達したい状態

GLBキャラクターモデルを Camera3d でリアルタイムレンダリングすることで、スプライトシートを完全に廃止する。

```
廃止するもの
  手作業によるスプライトシート作成
  方向×状態のコマ管理
  スプライトコマ切り替えロジック

置き換えるもの
  GLBモデル 1つ + ボーンアニメーションクリップ
  → Camera3d が任意角度からリアルタイムレンダリング
  → アニメーション追加 = クリップ追加のみ
  → 矢視時も自然に横から見た姿が見える
```

### 「体積のない」外見との両立

キャラクターを体積のある存在として描くことはしない。3Dモデルとして存在しながら、アートスタイルは Unlit 変換・アウトライン・ポスタライズ処理によって2Dイラスト的な見た目に担保する。

```
3Dモデルとして存在すること（技術的事実）
  ≠
体積のある存在として見えること（アートスタイルの選択）
```

Camera3d の角度・シェーダー設計によって、3Dモデルが平面的なイラストとして見える表現を目指す。

---

## 2. スコープ

### 対象（In Scope）

- Soul・Familiar の GLB モデル採用（TRELLIS.2 または TripoSR で生成）
- Bevy `AnimationGraph` によるボーンアニメーション管理
- `CharacterMaterial` の独立定義（`SectionMaterial` とは別シェーダー）
- `section_clip.wgsl` 共通モジュールによるクリップ平面の共有
- アニメーション状態機械の設計（タスク状態との連動）

### 非対象（Out of Scope）

- スプライトシートの流用（廃止）
- ビルボード方式（廃止）
- スプライトコマ切り替えロジック（廃止）
- キャラクターの完全フォトリアル化（Unlit + アウトラインで2D的外見を維持）

---

## 3. 技術設計

### 3.1 アーキテクチャ

```
Phase 2（現在）
  SoulProxy3d + Billboard
    → 2Dスプライトと3Dプロキシの並走
    → Phase 3 への暫定対応として維持

Phase 3（本提案）
  Soul GLB モデル（SectionMaterial）
    → Camera3d が直接レンダリング
    → スプライトシート・ビルボード・プロキシは全廃
    → AnimationGraph でボーン制御
```

### 3.2 モデル構成

Soul・Familiar は幽霊・使い魔という性質上、有機的なシルエットを持つ。Decimate 後のシルエット品質差が大きいため TRELLIS.2 を優先して使用する（建築物の TripoSR との使い分けは既定方針通り）。

ボーンリグはキャラクターモデルのトポロジーに依存するため、リギングパイプラインを先行して確定する必要がある。

### 3.3 AnimationGraph 設計

Bevy 0.18 の `AnimationGraph` を使用する。状態はタスクシステム（`hw_task`）から受け取り、アニメーションクリップをブレンドする。

```rust
// アニメーション状態の定義（概念）
enum SoulAnimState {
    Idle,
    Walk,
    Work { direction: Vec2 },   // 作業方向を渡してボーンで向きを表現
    Carry,
    Fear,
    Exhausted,
}

// AnimationGraph のノード構成（概念）
// Idle ─┐
// Walk ─┤─ blend ─ output
// Work ─┘
```

スプライトコマで方向を表現していた部分は、ボーンの回転・IK で代替する。「壁に向かって作業する」表現は作業対象の位置をボーンシステムに渡すことで自然に成立する。

### 3.4 CharacterMaterial の独立定義

キャラクターには建物用の `SectionMaterial` ではなく、専用の `CharacterMaterial` を定義する。

**分離の理由**

建物とキャラクターは Fragment Shader に求める要件が根本的に異なる。

```
建物（SectionMaterial）
  形状：直線・平面・幾何学的
  変化：静的（施工進捗のみ）
  シルエット：メッシュの形そのもの
  関心事：クリップ平面・Unlit・テクスチャ

キャラクター（CharacterMaterial）
  形状：有機的・曲線
  変化：動的・毎フレーム
  シルエット：SDF で数式定義（将来）
  関心事：SDF・ゆらぎ・状態表現・半透明
```

同じシェーダーに両方の関心事を詰め込むと、どちらにとっても最適でないコードになる。独立して定義することで建物側はシンプルなまま保たれ、キャラクター側は自由に設計できる。

**共通モジュールによるクリップ平面の共有**

セクションビューのクリップ平面はキャラクターにも適用する必要がある。WGSL の `#import` で共通モジュールとして切り出し、両マテリアルから参照する。

```wgsl
// shaders/common/section_clip.wgsl（共通モジュール）
fn apply_section_clip(
    world_pos: vec3<f32>,
    cut_position: vec3<f32>,
    cut_normal: vec3<f32>,
    thickness: f32,
    cut_active: f32,
) -> array<f32, 2> { ... }

// shaders/section_material.wgsl
#import "common/section_clip.wgsl"

// shaders/character_material.wgsl
#import "common/section_clip.wgsl"
// + キャラクター固有処理
```

**CharacterMaterial の定義**

```rust
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct CharacterMaterial {
    // クリップ平面（section_clip.wgsl と共通）
    #[uniform(0)]
    pub cut_position: Vec4,
    #[uniform(1)]
    pub cut_normal: Vec4,
    #[uniform(2)]
    pub thickness: f32,
    #[uniform(3)]
    pub cut_active: f32,

    // キャラクター状態
    #[uniform(4)]
    pub base_color: LinearRgba,
    #[uniform(5)]
    pub ghost_alpha: f32,      // 幽霊の半透明度
    #[uniform(6)]
    pub fear_factor: f32,      // 恐怖状態（0.0〜1.0）
    #[uniform(7)]
    pub exhausted_factor: f32, // 疲弊状態（0.0〜1.0）

    // 境界面処理
    #[uniform(8)]
    pub fade_distance: f32,    // 仮想点・アウトライン強調の影響距離

    // アウトライン
    #[uniform(9)]
    pub outline_color: LinearRgba,
    #[uniform(10)]
    pub outline_width: f32,

    // テクスチャ
    #[texture(11)]
    #[sampler(12)]
    pub base_color_texture: Option<Handle<Image>>,
}

impl Material for CharacterMaterial {
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend // 半透明を有効化（建物は Opaque）
    }
}
```

**Fragment Shader の構成**

```wgsl
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // 1. セクションビューのクリップ（共通モジュール）
    // clip_distances で処理済み

    // 2. 境界への近さを計算（Depth Prepass から取得）
    let depth_diff         = prepass_depth(in.position, 0u) - in.position.z;
    let boundary_proximity = 1.0 - saturate(depth_diff / material.fade_distance);
    // 0.0 = 境界から遠い・1.0 = 境界面

    // 3. テクスチャサンプリング
    let tex = textureSample(base_texture, base_sampler, in.uv);

    // 4. 状態による色変化
    let fear_color = vec4(0.8, 0.2, 0.2, 1.0);
    let color = mix(tex * material.base_color, fear_color, material.fear_factor);

    // 5. 境界面でアウトラインを強調（手書き感補強）
    //    boundary_proximity が大きいほど線が濃く・密になる
    let enhanced_outline_color = material.outline_color
        * (1.0 + boundary_proximity * 0.4);

    // 6. 幽霊の半透明
    return vec4(color.rgb, color.a * material.ghost_alpha);
    // ※ アウトライン描画・SDF シルエット判定は将来実装時に追加
}
```

**仮想点アプローチとの関係**

`boundary_proximity` はキャラクター本体の SDF シルエット・アウトライン強調の両方に共通して使用する。カーテン変形（セクション8）の仮想点アプローチも同じ `boundary_proximity` を起点とするため、`CharacterMaterial` と `CurtainMaterial` でシェーダーの境界面処理の構造が統一される。

```
boundary_proximity（共通係数）
  ├─ CharacterMaterial
  │    → SDF アウトライン強調
  │    → 状態表現のブレンド
  └─ CurtainMaterial（将来）
       → 仮想点でゆらぎ評価
       → アウトライン強調
```

共通モジュール `boundary.wgsl` として切り出すことで両マテリアルから再利用できる。

```wgsl
// shaders/common/boundary.wgsl
fn calc_boundary_proximity(
    frag_position: vec4<f32>,
    fade_distance: f32,
) -> f32 {
    let depth_diff = prepass_depth(frag_position, 0u) - frag_position.z;
    return 1.0 - saturate(depth_diff / fade_distance);
}
```

**レンダリングパスとZバッファの共有**

建物（`Opaque`）とキャラクター（`Blend`）は同じ Camera3d のパスで混在して描画される。Zバッファは共有されるため、キャラクターが壁の後ろに入ると正しく隠れる。半透明オブジェクトは Bevy によって自動的に不透明オブジェクトの後のパスで描画される。

### 3.5 SectionCut との同期

`SectionMaterial` と同様に、`SectionCut` リソースの変化を `CharacterMaterial` に伝播するシステムを追加する。

```rust
fn sync_section_cut_to_character_materials(
    cut: Res<SectionCut>,
    handles: Res<CharacterHandles>,
    mut materials: ResMut<Assets<CharacterMaterial>>,
) {
    if !cut.is_changed() { return; }
    // cut の値を全 CharacterMaterial に伝播
}
```

### 3.6 Camera3d 角度との関係

キャラクターは建物と同じ Camera3d でレンダリングされる。Camera3d の角度確定（V-1：billboard-camera-angle 提案書参照）はキャラクターの見え方にも直接影響するため、角度確定の検証時にキャラクターの見え方も同時に確認する。

```
V-1 検証の拡張
  従来：Cuboid の壁・床の見え方を確認
  追加：キャラクタープロキシ（または仮GLB）の見え方を確認
        「体積のない存在に見えるか」を判断基準に加える
```

### 3.7 リソース分離：`Building3dHandles` と `CharacterHandles`

建物リソースとキャラクターリソースは責務が異なるため、`Building3dHandles` から分離して `CharacterHandles` を独立リソースとして定義する。

```rust
/// 建物用ハンドル（SectionMaterial のみ）
#[derive(Resource)]
pub struct Building3dHandles {
    pub wall_material:      Handle<SectionMaterial>,
    pub floor_material:     Handle<SectionMaterial>,
    pub door_material:      Handle<SectionMaterial>,
    pub equipment_material: Handle<SectionMaterial>,
}

/// キャラクター用ハンドル（CharacterMaterial・GLBメッシュ）
#[derive(Resource)]
pub struct CharacterHandles {
    pub soul_mesh:          Handle<Mesh>,
    pub familiar_mesh:      Handle<Mesh>,
    pub soul_material:      Handle<CharacterMaterial>,
    pub familiar_material:  Handle<CharacterMaterial>,
}
```

`sync_section_cut_to_character_materials`（3.5）は `Res<CharacterHandles>` を参照する。

### 3.8 顔の表現

**前提条件**

53度視点では顔の上半分（額・目）が主に見える。口・顎は見えにくい。Unlit のため照明による陰影が使えず、全て色とシェイプで表現する必要がある。幽霊・使い魔という性質から、細かいリアル表情より記号的・アイコン的な表現が世界観に合う。

**採用方針：テクスチャアトラス + UV オフセット**

1枚のテクスチャに複数の表情を並べ、UV オフセットで表情を切り替える。テクスチャのバインド変更なく UV オフセットの変更だけで切り替えられる。

```
テクスチャアトラスのレイアウト例

┌──────┬──────┬──────┐
│通常  │恐怖  │疲弊  │
├──────┼──────┼──────┤
│集中  │喜び  │消滅  │
└──────┴──────┴──────┘
```

**CharacterMaterial への追加**

```rust
#[uniform(13)]
pub face_uv_offset: Vec2,   // アトラス内の表情位置
#[uniform(14)]
pub face_uv_scale: Vec2,    // アトラス内の1コマのサイズ
```

```rust
// ゲーム状態から UV オフセットを決定するシステム
let offset = match soul_state {
    SoulAnimState::Idle      => vec2(0.0, 0.0),
    SoulAnimState::Fear      => vec2(1.0, 0.0),
    SoulAnimState::Exhausted => vec2(2.0, 0.0),
    SoulAnimState::Focused   => vec2(0.0, 1.0),
};
character_material.face_uv_offset = offset * face_uv_scale;
```

**表情アニメーション**

`face_uv_offset` を毎フレーム更新することで、テクスチャ貼り替えによる表情アニメーションが成立する。ボーンアニメーションと完全に独立して動作するため、歩きながらまばたきする等の組み合わせが自然に成立する。

```
まばたきの例

フレーム1: face_uv_offset = (0.0, 0.0) → 目を開いた顔
フレーム2: face_uv_offset = (1.0, 0.0) → 半目
フレーム3: face_uv_offset = (2.0, 0.0) → 目を閉じた顔
フレーム2: face_uv_offset = (1.0, 0.0) → 半目
フレーム1: face_uv_offset = (0.0, 0.0) → 目を開いた顔
```

**顔面のみビルボード化**

53度視点では顔テクスチャが3Dメッシュの曲面に貼られると歪みが生じる。顔面サブメッシュ（`mesh_face`）のみをカメラに向けることで、テクスチャが常に正面から見た状態で表示されデザイン通りの見た目になる。

```
soul.glb
  ├─ mesh_body     ← ボーンアニメーション対象
  ├─ mesh_face     ← ビルボード化対象（FaceBillboard コンポーネント）
  ├─ mesh_curtain  ← Vertex Shader 変形対象（将来）
  └─ mesh_frame    ← 固定（ドア用）
```

`mesh_face` は body の子エンティティとして GLB から読み込まれるため、その `Transform` はローカル空間（親の body 基準）になる。カメラのワールド回転をそのままローカルに代入すると、親（body）の回転が二重に乗り誤った向きになる。ワールド空間で目的の回転を決定してから親の逆回転を適用してローカル変換に戻す必要がある。

```rust
fn face_billboard_system(
    camera: Query<&GlobalTransform, With<Camera3dRtt>>,
    parents: Query<&GlobalTransform>,                     // 親の GlobalTransform
    mut faces: Query<(&mut Transform, &Parent), With<FaceBillboard>>,
) {
    let Ok(cam_tf) = camera.single() else { return };
    let cam_world_rot = cam_tf.to_scale_rotation_translation().1;

    for (mut face_local_tf, parent) in faces.iter_mut() {
        // 親のワールド回転を取得
        let Ok(parent_global_tf) = parents.get(parent.get()) else { continue };
        let parent_world_rot = parent_global_tf.to_scale_rotation_translation().1;

        // ワールド目標回転 → ローカル回転に変換
        face_local_tf.rotation = parent_world_rot.inverse() * cam_world_rot;
    }
}
```

体の向きと顔の向きが分離することで「どの方向を向いていても顔がこちらを向いている」表現が生まれる。幽霊・霊体という Soul の性質と整合する演出として活用できる。

**テクスチャアニメーション + 顔面ビルボードの組み合わせ**

```
どの角度から見ても
  テクスチャが歪まず表情が正確に見える（ビルボード）
  + まばたき・表情変化がアニメーションする（UV オフセット）
```

実装コストはいずれも低く、組み合わせによる追加コストはほぼゼロ。

**他アプローチとの比較**

| アプローチ | 実装コスト | ブレンド | アーティスト制御権 | 採否 |
| --- | --- | --- | --- | --- |
| テクスチャスワップ | 低 | ❌ | 高 | △ ドロー数増加 |
| テクスチャアトラス + UV オフセット | 低 | ❌ | 高 | ✅ 採用 |
| ブレンドシェイプ | 中 | ✅ | 中 | ❌ 低ポリゴンで効果小 |
| SDF（Fragment Shader） | 高 | ✅ | 低 | 将来検討 |

ブレンドシェイプは Bevy 0.18 でサポートされているが、53度視点・低ポリゴン・Unlit という条件では頂点変形の効果が視認しにくいため採用しない。

**将来的な SDF 移行への互換性**

SDF アプローチに移行する場合、`face_uv_offset` を `face_expression_factor`（0.0〜1.0）に置き換えるだけで対応できる。`CharacterMaterial` の他のフィールドへの影響はない。

---

## 4. リギングパイプライン

### 4.1 候補

| ツール | 商用利用 | リグ互換性 | コスト |
| --- | --- | --- | --- |
| Mixamo | ✅ 商用可・クレジット不要・ロイヤリティなし（Adobe Community FAQ 確認済み） | T ポーズ自動リグ | 無料（CC 契約必要） |
| Rokoko | 商用可 | 高い制御性 | 有料 |
| Bevy AnimationGraph 手動 | 制約なし | 完全制御 | 工数大 |

### 4.2 未確定事項

リギングパイプラインの最終選択（Mixamo / Rokoko / 手動）は Phase 3 GLB 取込 PoC 後に確定する。Mixamo の商用利用条件は解決済み（§7 解決済み参照）。

---

## 5. ポリゴン予算

キャラクターは建物と同じポリゴン予算（全オブジェクト合計 30,000 三角形）を共有する。壁の2層構造採用によって予算が圧迫される可能性があることを踏まえ、キャラクターの LOD 設計は PoC でのポリゴン予算実測後に確定する。

暫定目安は以下の通り。

| LOD | 三角形数 | 用途 |
| --- | --- | --- |
| LOD0 | 600〜1,200 | ズームイン・セクションビュー |
| LOD1 | 200〜400 | 通常プレイ |
| LOD2 | 60〜120 | 遠景 |

壁の2層構造の PoC 結果によっては LOD1 の上限を引き下げる必要がある。

---

## 6. Phase 2 との移行関係

Phase 2 で実装した以下のコンポーネントは Phase 3 で廃止する。

| Phase 2 実装 | Phase 3 での扱い |
| --- | --- |
| `SoulProxy3d` / `FamiliarProxy3d` | 廃止・GLBモデルのエンティティに置き換え |
| `Billboard` コンポーネント | 廃止 |
| `billboard_system` | 廃止 |
| 2D Sprite との並走同期 | 廃止 |
| スプライトコマ切り替えロジック | 廃止・ボーンアニメーションに置き換え |

Phase 2 のプロキシは「Zバッファ・矢視・RtT パイプラインの技術検証用」という位置づけのままとする。Phase 2 の段階でプロキシを最適化しない。

---

## 7. 未解決事項（Pending）

### 解決済み

| 項目 | 結論 | 確認日 |
| --- | --- | --- |
| Mixamo 商用利用条件の一次情報確認 | ✅ 商用利用・無制限（営利/非営利/研究/学校すべて可）。クレジット不要・ロイヤリティなし。唯一の制限は機械学習モデルの学習データへの使用（ゲームには無関係）。Adobe Community 公式 FAQ より確認。 | 2026-03-17 |

### 未解決

| 項目 | 優先度 | タイミング |
| --- | --- | --- |
| Camera3d 角度確定時のキャラクター見え方検証（V-1 拡張） | P0 | Phase 2 プリミティブ段階 |
| `section_clip.wgsl` 共通モジュールの設計 | P0 | MS-Section-A と同時 |
| `boundary.wgsl` 共通モジュールの設計 | P1 | `CharacterMaterial` 実装時 |
| アニメーション状態機械の詳細設計 | P1 | Phase 3 GLB 取込 PoC 後 |
| ポリゴン予算の確定（壁2層構造 PoC 結果待ち） | P1 | Phase 3 PoC と同時 |
| リギングパイプラインの確定（Mixamo / Rokoko / 手動） | P1 | Phase 3 GLB 取込 PoC 後 |
| `CharacterMaterial` の Fragment Shader 詳細設計 | P1 | アートスタイル受入基準確定後 |
| `CarryingItemVisual` の3D表現方針 | P1 | アニメーション状態機械設計と同時 |
| `mesh_wall_fill` フェードアウトのトリガー実装担当の確定 | P1 | Phase 3 ドア実装時 |
| 顔テクスチャアトラスのレイアウト設計 | P1 | アートスタイル受入基準確定後 |

---

## 8. 将来用実装：カーテン変形（`CurtainMaterial`）

> **実装対象フェーズ**: Phase 3 以降・アニメーション状態機械確定後
> **ステータス**: 設計記録のみ。現時点では実装しない。

### 8.1 動機

Soul の下半身がカーテン状にゆらゆらする表現は、Vertex Shader によるリアルタイム変形が最も適している。Prerender で同等の表現を得るには1アニメーションあたり数十〜百枚以上のコマが必要になり、スプライトシート廃止という本提案の動機と矛盾する。この表現はリアルタイム3Dレンダリングを選ぶ技術的根拠の一つでもある。

### 8.2 メッシュ設計

カーテン部分を専用サブメッシュとして分離する。上半身はボーンアニメーション対象、下半身カーテンは Vertex Shader 制御対象の2層構造にする。

```
soul.glb
  ├─ mesh_body     ← ボーンアニメーション対象
  └─ mesh_curtain  ← Vertex Shader 変形対象
                     縦方向に 8〜16 分割
                     下端ほど変形量が大きい
```

### 8.3 CurtainMaterial

Rust にフィールド継承はないため、`SectionMaterial` との共通フィールドを `SectionUniforms` として切り出し、両マテリアルに埋め込む構成をとる。これによりクリップ平面などの定義を一箇所に集約する。

```rust
/// section_material.rs と curtain_material.rs で共有する共通フィールド
#[derive(Clone, ShaderType)]
pub struct SectionUniforms {
    pub cut_position: Vec4,
    pub cut_normal:   Vec4,
    pub thickness:    f32,
    pub cut_active:   f32,
}

/// カーテン変形マテリアル（mesh_curtain サブメッシュ専用）
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct CurtainMaterial {
    #[uniform(0)]
    pub section: SectionUniforms,  // SectionMaterial と共通の均一変数
    #[uniform(1)]
    pub sway_amplitude: f32,       // ゆらぎの強さ
    #[uniform(2)]
    pub sway_speed: f32,           // ゆらぎの速さ
    #[uniform(3)]
    pub curtain_height: f32,       // カーテン部分の高さ
    #[uniform(4)]
    pub fade_distance: f32,        // 境界面アウトライン強調の影響距離
}
```

### 8.4 Vertex Shader 概念コード

```wgsl
@vertex
fn vertex(...) -> VertexOutput {
    var pos = position;

    // 下に行くほど変形量が大きくなる（根元は固定）
    let curtain_factor = max(0.0, -pos.y / material.curtain_height);

    // 時間と位置でノイズを生成
    let wave = sin(pos.x * 3.0 + time * material.sway_speed)
             * cos(pos.z * 2.0 + time * material.sway_speed * 0.75)
             * curtain_factor
             * material.sway_amplitude;

    pos.x += wave;
    pos.z += wave * 0.5;

    // ...
}
```

### 8.5 ゲーム状態との連動

```
平常時   → sway_amplitude = 0.02（微細なゆらぎ）
移動時   → sway_amplitude = 0.08（移動風でなびく）
恐怖時   → sway_amplitude = 0.15（激しく震える）
消えかけ → sway_amplitude = 0.30 + alpha 減少（崩壊していく）
```

### 8.6 建物境界面の表現活用

Soul は壁をすり抜けずドアを使うため、境界面が発生するケースは限定的である。

```
発生するケース
  壁際で作業するとき    → カーテン裾が壁面に少し触れる
  壁の後ろを通り過ぎる → 53度視点のため壁との境界面が一時的に発生
```

これらは「問題として隠す」のではなく「手書き感を補強する表現として活用する」方針をとる。

**設計方針**

```
仮想点アプローチ          → ゆらぎの連続性を維持する（技術基盤）
境界面アウトライン強調    → 手書き感を補強する（表現活用）
```

**仮想点によるゆらぎの連続性維持**

境界面付近のフラグメントは実際の位置ではなく仮想位置でゆらぎを評価する。評価位置が多少ずれても sin/cos の滑らかな関数特性により波のパターンは自然に継続する。

> **実装文脈**: 以下はフラグメントシェーダーのコードである。`prepass_depth` は頂点シェーダーから呼べない。メッシュの実際の頂点変形は8.4のVertex Shader が担い、フラグメントシェーダーはその上でパターン/色評価を行う。

```wgsl
// @fragment fn fragment(...) 内
let depth_diff         = prepass_depth(in.position, 0u) - in.position.z;
let boundary_proximity = 1.0 - saturate(depth_diff / material.fade_distance);

// ゆらぎパターンの評価位置を仮想点にずらす
let virtual_pos = world_pos + view_forward * fade_distance * boundary_proximity;
let eval_pos    = mix(world_pos, virtual_pos, boundary_proximity);

let wave = sin(eval_pos.x * 3.0 + time * sway_speed)
         * cos(eval_pos.z * 2.0 + time * sway_speed * 0.75)
         * curtain_factor
         * sway_amplitude;
```

**境界面でのアウトライン強調**

手描きアニメーションでは物体の境界・重なりの部分に線が集中し、密度が上がる。`boundary_proximity` でアウトライン幅と濃度を制御することで、シェーダーが自動的にこの手描きの性質を再現する。

```wgsl
// 通常：距離ゼロの1本線がアウトライン
let outline = dist > -outline_width && dist < 0.0;

// 境界面付近：等値線を追加して線の密度を上げる（クロスハッチング的効果）
let extra_lines = sin(dist * 20.0 * boundary_proximity) * 0.5 + 0.5;
let enhanced = outline || (boundary_proximity > 0.3 && extra_lines > 0.8);

let outline_color = mix(
    material.outline_color,
    material.outline_color * 1.4, // 境界面で線を濃く
    boundary_proximity
);
```

壁の後ろを通り過ぎる瞬間にシルエットの境界が強調されることで、キャラクターの「奥行きの変化」が視覚的に読み取りやすくなる。単に壁に隠れるより「境界面を通過した感」が明確になる。

**SDF との相性**

SDF でシルエットを定義している場合、境界面付近で SDF の等値線を複数描画することで線の密度が自然に増す。頂点数に依存せず解像度独立で機能する。

**実装タイミング**

アートスタイル受入基準の確定後、`CurtainMaterial` の実装と同時に行う。`fade_distance`・アウトライン強調係数はアートスタイル PoC で調整する。

---

## 9. ドアとキャラクターの設計方針

### 9.1 ドア通過の基本仕様

ドアアニメーション完了後にキャラクターが通過するのではなく、ドアのアニメーションフレーム数に対応したキャラクターアニメーションクリップを用意し同期再生する。

```
ドア開始イベント発火
  ↓ 同時に
ドアアニメーション再生（フレーム数: N）
キャラクターの「ドアを開ける」クリップ再生（同じフレーム数: N）
  ↓
両方終了 → キャラクターが通過開始
```

追加実装はアニメーションクリップの追加とドアの向きに応じた向き制御のみ。IK・リアルタイム同期・速度調整は不要。

### 9.2 ドア GLB のメッシュ構成

ドアのGLBに開口部を塞ぐ壁面メッシュ（`mesh_wall_fill`）を含める。

```
door.glb
  ├─ mesh_door_panel  ← 扉本体（回転する）
  ├─ mesh_frame       ← ドア枠（固定）
  └─ mesh_wall_fill   ← 開口部を塞ぐ壁面
                         ドア閉時のみ表示
                         SectionMaterial を適用
```

`mesh_wall_fill` により壁とドアの継ぎ目が自然になり、セクションビューで切断したときも壁面の断面が連続して見える。表示・非表示はドアの開閉状態に連動して切り替える。

**マテリアル構成**

```
mesh_door_panel → DoorMaterial（将来定義・回転アニメーション対応）
mesh_frame      → SectionMaterial
mesh_wall_fill  → SectionMaterial（壁と同じ）
```

### 9.3 排他制御

ドア自体はタスクではないが、複数キャラクターが同時に使用しようとした場合の制御が必要。

```
状態管理
  ドアが閉・予約なし  → 予約取得・開けてから通過
  ドアが開・予約なし  → そのまま通過
  予約あり           → 解放まで待機
```

既存のタスクリソース予約ロジックを借用して実装する。ドア固有の新規実装は予約ロジックとドア開閉状態の連動部分のみ。

### 9.4 仮設状態

ゲーム仕様上、ドアに仮設状態はない。壁の2層構造（blueprint + completed）をドアに適用しない。GLBは `completed` 層のみの単一構造で設計する。

### 9.6 mesh_wall_fill とキャラクターシェーダーの整合性

**キャラクター上半身が開口部から覗く表現**

53度視点でキャラクターがドアを通過するとき、キャラクターの上半身が `mesh_wall_fill` の上端より高い位置に出る場合がある。これは物理的に正しい表現であり、問題として扱わない。

```
53度視点・ドア通過中

  カメラ（斜め）
    ↓
  [キャラクター上半身] ← mesh_wall_fill の上端より高い → 正しく描画
  ════════════════════  ← mesh_wall_fill の上端
  [mesh_wall_fill   ]
  [    開口部        ]
```

**深度バッファの共有による前後関係の保証**

`SectionMaterial`（Opaque）と `CharacterMaterial`（Blend）はシェーダーが異なるが、同じ Camera3d のレンダリングパスで同じ深度バッファを参照する。

```
描画順序
  1. SectionMaterial（Opaque）
     → mesh_wall_fill が深度バッファに書き込まれる

  2. CharacterMaterial（Blend）
     → 深度バッファを参照して前後判定
     → mesh_wall_fill より前のフラグメント → 正しく描画
     → mesh_wall_fill より後のフラグメント → 正しく遮蔽
```

シェーダーが異なることによる前後関係の破綻は発生しない。

**SectionCut クリップ平面の整合**

`mesh_wall_fill`（SectionMaterial）とキャラクター（CharacterMaterial）は `section_clip.wgsl` 共通モジュールを通じて同じ `SectionCut` リソースの値でクリップされる。

```
SectionCut が有効なとき
  mesh_wall_fill → スラブ外でクリップ
  キャラクター   → 同じスラブ外でクリップ

→ 両者が同じ基準でクリップされるため
  壁とキャラクターの表示・非表示が常に一致する
```

**boundary_proximity の誤発火への対処**

通過中にキャラクターが `mesh_wall_fill` の手前面と奥面に挟まれる状態になるため、`boundary_proximity` が高い値で発動し続ける可能性がある。`fade_distance` を壁の厚みより小さく設定することで開口部中央付近での誤発火を抑制できる。

```
壁の厚み = TILE_SIZE × 0.3（例）
fade_distance = TILE_SIZE × 0.15

→ 開口部の中央付近では両面から遠いため
  boundary_proximity がゼロに近い値に保たれる
```

`fade_distance` はアートスタイル PoC で調整する。

### 9.7 LOD（将来検討）

ドアは回転する可動部品であるため、LOD切り替えのタイミングでメッシュが差し替わると回転中にポップが視覚的に目立ちやすい。LOD段数の削減・切り替え距離閾値の調整等を Phase 3 以降に検討する。

---

## 10. 旧提案書との関係billboard 提案書は Camera3d 角度確定・壁メッシュ構成の決定事項として有効だが、キャラクターに関する以下の記述は本提案書の内容で上書きされる。

| billboard 提案書の記述 | 本提案書での扱い |
| --- | --- |
| ビルボード方式の採用 | 廃止 |
| 既存2Dスプライトの流用 | 廃止 |
| プロキシサイズとスプライトサイズの整合 | 不要（スプライトがなくなるため） |
| 作業方向のスプライトコマ切り替え | ボーンアニメーションに置き換え |
| SectionMaterial クリップの別途対応（P1） | CharacterMaterial + 共通モジュールで解消 |

---

## 11. 決定事項サマリ

| 決定内容 | 日付 |
| --- | --- |
| キャラクターは GLB モデル + ボーンアニメーションで Camera3d レンダリングに移行する | 2026-03-16 |
| スプライトシート・ビルボード・プロキシ並走は Phase 3 で廃止する | 2026-03-16 |
| Phase 2 のプロキシは技術検証用の暫定実装として維持し最適化しない | 2026-03-16 |
| キャラクターには `SectionMaterial` ではなく `CharacterMaterial` を独立定義する | 2026-03-16 |
| クリップ平面は `section_clip.wgsl` 共通モジュールで `SectionMaterial` と共有する | 2026-03-16 |
| キャラクターは `AlphaMode::Blend` で半透明を有効化する（建物は Opaque） | 2026-03-16 |
| キャラクターモデルは TRELLIS.2 を優先して生成する | 2026-03-16 |
| ポリゴン予算・アニメーション状態機械は壁2層構造 PoC 後に確定する | 2026-03-16 |
| 顔の表現はテクスチャアトラス + UV オフセット方式を採用する | 2026-03-16 |
| 顔面サブメッシュ（mesh_face）のみビルボード化し53度視点でのテクスチャ歪みを防ぐ | 2026-03-16 |
| ブレンドシェイプは53度視点・低ポリゴン・Unlit の条件では効果が小さいため採用しない | 2026-03-16 |
