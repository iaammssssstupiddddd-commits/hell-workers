# Camera3d 角度確定提案（旧：ビルボード方式採用）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 提案ID | `billboard-camera-angle-proposal-2026-03-16` |
| ステータス | `Partially Superseded` |
| 作成日 | `2026-03-16` |
| 最終更新日 | `2026-03-17` |
| 作成者 | Claude Sonnet 4.6 |
| 関連ロードマップ | `docs/plans/3d-rtt/milestone-roadmap.md` |
| 関連提案 | `docs/proposals/3d-rtt/phase2-hybrid-rtt-plan-2026-03-15.md` |
| 上書き提案 | `docs/proposals/3d-rtt/20260317/character-3d-rendering-proposal-2026-03-16.md` |
| 依存完了済み | Phase 2 全MS（MS-2A〜MS-2D, MS-Elev） |
| 実装対象フェーズ | Phase 3 着手前（Phase 2 プリミティブ段階での検証・確定が必須） |

---

> ## ⚠️ 部分的に上書き済み（2026-03-17）
>
> **有効な決定事項**（引き続き本提案書が根拠）:
> - Camera3d の斜め角度（約53°）設定（§3.1・§3.2・§5 V-1・§7）
> - 壁メッシュ構成（直線Cuboid＋コーナー/T字/十字の専用GLB）（§7）
>
> **上書きされた決定事項**（`character-3d-rendering-proposal-2026-03-16` が根拠）:
> - キャラクターのビルボード方式 → **GLB モデル + ボーンアニメーションに変更**
> - `CharacterBillboardMaterial` → **廃止。`CharacterMaterial`（AlphaMode::Blend）に置き換え**
> - `billboard_system`（全キャラクター対象）→ **廃止。`face_billboard_system`（mesh_face のみ）に置き換え**
> - 作業方向スプライトコマ切り替え → **廃止。ボーンアニメーション + AnimationGraph に置き換え**
> - `SectionCut` をキャラクターに適用しない方針 → **上書き。`CharacterMaterial` でクリップ平面を適用する**

---

## 1. 目的

### 解決したい課題

RtT（Render to Texture）移行において、トップダウン2Dビューが「成立していた」理由は、壁やキャラクターが側面から見た絵として描かれたスプライトを持っていたからである。Camera3d を正射影・真上向きに設定すると、3Dメッシュは全て真上から見た姿で描画されるため、この2.5D表現が崩壊する。

```
従来の2D（嘘をついている）
  カメラ：真上
  地面タイル：真上から見た絵 ✓
  壁スプライト：側面から見た絵（嘘）
  キャラクタースプライト：正面から見た絵（嘘）

RtT（物理的に正直）
  Camera3d：真上（OrthographicProjection）
  3Dメッシュは全て真上から見た姿で描画される
  → 壁は「板」に見える
  → キャラクターは「円盤」に見える
```

これを解決し、かつ既存の2Dスプライトアニメーション資産を流用可能な形で2.5D表現を実現する。

### 到達したい状態

- 建築物（壁・床・設備）：Camera3d を斜め角度（約53度）に設定し、側面が自然に見える
- ~~キャラクター（Soul・Familiar）：ビルボード方式でカメラに常に正面を向け、既存2Dスプライトアニメーションをそのまま流用する~~ **→ 上書き済み**。キャラクターは GLB モデル + ボーンアニメーションで Camera3d に直接レンダリングする（`character-3d-rendering-proposal` 参照）
- Camera3d の斜め角度により「体積のない存在に見える」キャラクター外見が実現されること（`character-3d-rendering-proposal` §3.6）

### なぜ Phase 3 着手前に確定が必須か

Camera3d の角度は GLB モデルの設計に直接影響する。

```
斜め角度が確定している場合
  → AI生成時のプロンプト・撮影角度指定が確定できる
  → 生成後の向き調整・回転修正工数が最小化される
  → セクションビューで切断される面（最高品質が求められる）が特定できる

斜め角度が未確定のまま Phase 3 に入ると
  → 角度変更のたびに全 BuildingType の見え方を再確認する必要がある
  → アセット生成パイプラインのカメラ角度指定が定まらない
  → セクションビューの切断位置に応じた品質配分が設計できない
```

なお「見えない面のポリゴンを省略できる」という直感的な最適化は、セクションビューの存在により成立しない。セクションビューは Camera3d を真横に向けるため、通常ビューで見えない底面・背面も切断面として現れる可能性がある。また、セクションビューはユーザーが「詳細確認のために使う」機能であり、スラブ内（数マス分）に限定された範囲を画面いっぱいに表示するため、LOD0（最高品質）が要求される場面である。全面を均等に作り込む前提で進める。

Phase 2 のプリミティブ（Cuboid）段階で実際に検証・数値確定してから Phase 3 に着手する。

---

## 2. スコープ

### 対象（In Scope）

- Camera3d の通常ビュー角度を真上（90度）から斜め（約53度）に変更
- `sync_camera3d_system` の座標マッピング変更（2D平面座標 → 3D斜め視点座標への変換式更新）
- ~~キャラクター（Soul・Familiar）の3DプロキシをビルボードEntityとして実装~~ **→ 上書き済み**（`character-3d-rendering-proposal` §3 参照）
- ~~`billboard_system` の実装（毎フレームカメラ回転に追従）~~ **→ 廃止**（`face_billboard_system` に置き換え）
- ~~プロキシサイズとスプライトサイズの整合検証~~ **→ 不要**（スプライトが廃止されるため）
- ~~作業方向に応じたスプライトコマ切り替えの設計確認~~ **→ 上書き済み**（ボーンアニメーションに置き換え）

### 非対象（Out of Scope）

- 矢視モードのカメラ角度（別途 MS-Elev で管理）
- ~~GLBモデルへの置き換え（Phase 3 のスコープ）~~ **→ キャラクターの GLB 化は `character-3d-rendering-proposal` で採用済み**
- ~~キャラクターアニメーションの完全3D化（ビルボードで2Dアニメを流用するため不要）~~ **→ 上書き済み**（GLB + ボーンアニメーションを採用）

---

## 3. 技術設計

### 3.1 Camera3d 角度の変更

通常ビュー（トップダウン）の Camera3d を真上から斜め角度に変更する。

```rust
// 変更前（現在）：真上
// Y = 100, looking_at(Vec3::ZERO, Vec3::NEG_Z)

// 変更後：斜め約53度
// y 成分を d の 100%、z 成分を d の 75% 程度に設定
Transform::from_xyz(target.x, d, target.z - d * 0.75)
    .looking_at(target, Vec3::Y)
// → 約53度の見下ろし角になる
```

最終的な数値は Phase 2 プリミティブ段階での目視確認で確定する。
`world_lore.md` のアートスタイル（側面が自然に見える・壁に厚みを感じる）を基準として判断する。

### 3.2 座標マッピングの更新

Camera3d を斜め角度にすると、2D 地形座標と 3D 描画座標の対応が変わる。
`camera_sync.rs` の `sync_camera3d_system` を更新する。

```rust
// 現在の変換式
// 3d_pos.x = 2d_pos.x
// 3d_pos.y = object_height / 2.0
// 3d_pos.z = -2d_pos.y

// 斜め角度での変換式（概念）
// Camera3d の視錐台中心が 2D カメラ中心に一致するよう
// カメラの向きベクトルに沿って offset を加算する
fn sync_camera3d_system(
    cam2d: Query<&Transform, With<Camera2d>>,
    mut cam3d: Query<&mut Transform, With<Camera3dRtt>>,
) {
    let cam2d_tf = cam2d.single();
    let mut cam3d_tf = cam3d.single_mut();

    // 2D カメラ中心が 3D シーンのどこに対応するかを
    // 斜め角度に合わせて計算する
    let center_x = cam2d_tf.translation.x;
    let center_y = cam2d_tf.translation.y;

    cam3d_tf.translation.x = center_x;
    cam3d_tf.translation.y = VIEW_HEIGHT;          // 定数（検証で確定）
    cam3d_tf.translation.z = -center_y + Z_OFFSET; // 斜め角度による奥行きオフセット
}
```

`Z_OFFSET` および `VIEW_HEIGHT` の具体的な数値は Phase 2 の目視確認で確定する。

### 3.3 ~~ビルボード実装~~ 【上書き済み】

> **この節は `character-3d-rendering-proposal-2026-03-16` §3 によって上書きされた。**
> キャラクターは GLB モデル + `CharacterMaterial`（AlphaMode::Blend）で実装する。
> `face_billboard_system`（`mesh_face` サブメッシュのみ）が本提案の `billboard_system`（全キャラクター対象）を代替する。
> 以下は歴史的記録として保持する。

~~キャラクターを Camera3d に常に正面を向けるビルボードEntityとして実装する。~~

```rust
/// ビルボードマーカーコンポーネント
#[derive(Component)]
pub struct Billboard;

/// 毎フレーム Camera3d の回転に追従するシステム
fn billboard_system(
    camera: Query<&Transform, With<Camera3dRtt>>,
    mut billboards: Query<&mut Transform, (With<Billboard>, Without<Camera3dRtt>)>,
) {
    let cam_tf = camera.single();
    for mut tf in billboards.iter_mut() {
        // カメラと同じ向きに回転させることでカメラ正面を向く
        tf.rotation = cam_tf.rotation;
    }
}
```

`SoulProxy3d` / `FamiliarProxy3d` はZバッファ管理用のプロキシではなく、**スプライトテクスチャを持つ実際の表示エンティティ**として実装する。Camera2d 側でキャラクタースプライトを管理する必要はなく、Camera3d → RtT で描画が完結する。

> **メッシュの向きに関する注意**: `Billboard` を適用するメッシュは **`XY` 平面**（法線が `Vec3::Z`）として定義すること。
> Bevy のデフォルトの `Plane3d` は `XZ` 平面（法線が `Vec3::Y`）であり、そのまま `cam_tf.rotation` をコピーしても
> 正面を向かない。スプライト用の縦長クワッドには `XY` 平面（例: `Mesh::from(Rectangle::new(w, h))`）を使用すること。

### 3.4 ~~作業方向に応じたコマ切り替え~~ 【上書き済み】

> **この節は `character-3d-rendering-proposal-2026-03-16` §3.3 によって上書きされた。**
> スプライトコマ切り替えは廃止され、ボーンアニメーション（`AnimationGraph` + `SoulAnimState`）で代替する。
> 作業対象の位置をボーンシステムに渡すことで「壁に向かって作業する」表現が自然に成立する。
> 以下は歴史的記録として保持する。

~~ビルボードは常にカメラを向くため、キャラクターが「壁に向かって作業している」ように見せるには、作業方向に応じてスプライトのコマを切り替える必要がある。~~

```rust
/// タスクのターゲット方向からスプライトフレームを決定する
/// work_dir は 2D ゲーム座標（Vec2）で渡す。
/// Bevy の Vec2::Y が「北」に対応するかはゲームの方向設定に依存する。
/// プロジェクト内の GridPos.y 軸方向と一致しているか実装時に確認すること。
fn direction_to_sprite_frame(work_dir: Vec2) -> SpriteFrame {
    // 8方向判定
    let angle = work_dir.y.atan2(work_dir.x);
    match (angle / (PI / 4.0)).round() as i32 {
        0 | 8  => SpriteFrame::East,
        1      => SpriteFrame::NorthEast,
        2      => SpriteFrame::North,
        3      => SpriteFrame::NorthWest,
        4 | -4 => SpriteFrame::West,
        -3     => SpriteFrame::SouthWest,
        -2     => SpriteFrame::South,
        -1     => SpriteFrame::SouthEast,
        _      => SpriteFrame::South,
    }
}
```

`world_lore.md` のアートスタイルが「8方向アニメーション」または「左右反転のみ」かによって実装コストが変わるため、アートスタイル受入基準の確定と同時に決定する。

### 3.5 ~~CharacterBillboardMaterial~~ 【上書き済み】

> **この節は `character-3d-rendering-proposal-2026-03-16` §3.4 によって上書きされた。**
> `CharacterBillboardMaterial` は廃止。代わりに `CharacterMaterial`（`AlphaMode::Blend`・`section_clip.wgsl` 共有）を使用する。
> 顔の UV オフセット制御（`face_uv_offset` / `face_uv_scale`）は `CharacterMaterial` のフィールドとして残存する。
> セクションカットの扱いも反転した（適用しない → `section_clip.wgsl` 経由で適用する）。
> 以下は歴史的記録として保持する。

~~ビルボードクワッドはスプライトテクスチャを保持するカスタムマテリアルを使用する。~~

```rust
/// キャラクタービルボード用カスタムマテリアル
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct CharacterBillboardMaterial {
    /// スプライトテクスチャ（アトラスシートまたは単一フレーム）
    #[texture(0)]
    #[sampler(1)]
    pub sprite_texture: Handle<Image>,

    /// アトラス内の現在フレームの UV 開始位置（0.0〜1.0）
    #[uniform(2)]
    pub uv_offset: Vec2,

    /// 1フレームの UV サイズ（単一フレームなら Vec2::ONE）
    #[uniform(3)]
    pub uv_scale: Vec2,
}
```

アニメーション駆動システムは `AnimationTimer` リソースを参照し、毎フレーム `uv_offset` を更新する。`Familiar` のように個別フレームファイルを使う場合は `sprite_texture` の差し替えで対応する。

**セクションビュー中のキャラクター表示**: キャラクタービルボードにはセクションカットを適用しない（常に表示）。矢視はあくまで建物の断面確認用であり、キャラクターの半断面表示は不自然なため。`CharacterBillboardMaterial` に `SectionCut` ユニフォームは持たせない。

---

## 4. アクション成立性の確認

> **この節は `character-3d-rendering-proposal-2026-03-16` 採用により実装手段が変更された。**
> 成立性の結論（✅）は変わらないが、「必要な対処」がビルボード方式からボーンアニメーション方式に更新されている。

| アクション | 成立 | 実装手段（更新後） |
| --- | :---: | --- |
| 移動・待機アニメーション | ✅ | `AnimationGraph`（Walk / Idle クリップ） |
| 方向転換 | ✅ | ボーンの向き制御（IK / ルートボーン回転） |
| 壁への作業（Build・CoatWall 等） | ✅ | Work クリップ + 作業対象位置をボーンシステムに渡す |
| アイテム運搬（Haul タスク） | ✅ | Carry クリップ。`CarryingItemVisual` の3D表現は別途設計 |
| 川での水汲み（GatherWater タスク） | ✅ | Work クリップで代用（屈みはボーン変形で表現） |
| 建物の後ろに隠れる | ✅ | GLB メッシュが3D空間に存在するためZバッファで自然に解決（ビルボードと同様） |

キャラクター GLB が Camera3d で直接描画されるため、描画と遮蔽判定が同一エンティティで完結する点はビルボード方式と同じである。

---

## 5. 検証計画（Phase 2 プリミティブ段階）

Phase 3 着手前に以下を確認・数値確定する。

### V-1：Camera3d 斜め角度の目視確認（有効）

**確認内容：**
Cuboid の壁・床プリミティブを配置した状態で、Camera3d の角度を変えながら `world_lore.md` のアートスタイル基準に照らして判断する。

**合格基準：**
- 壁に厚みが感じられる
- 床と壁の境界が自然に見える
- キャラクタープロキシ（Cuboid または仮GLB）が「体積のない存在に見える」（`character-3d-rendering-proposal` §3.6 拡張）

**確定する数値：**
- `VIEW_HEIGHT`（Camera3d の Y 座標）
- `Z_OFFSET`（Camera3d の Z オフセット）
- 具体的な仰角（度数）

### V-2：~~ビルボードとZバッファの動作確認~~ → Character GLB PoC に置き換え 【上書き済み】

> **この検証は `character-3d-rendering-proposal` 採用により M-Pre4（Character GLB PoC）に置き換えられた。**
> `CharacterBillboardMaterial` の検証は不要。代わりに Soul GLB + `CharacterMaterial` で Z バッファ共有を確認する。

~~`Billboard` コンポーネントを付けたキャラクタープロキシが Cuboid 壁の前後に入ったとき...~~

**現行の合格基準（M-Pre4 に移管）：**
- 壁の後ろに入った Soul GLB が壁に隠れる（Z バッファ共有の確認）
- Soul GLB が「体積のない存在に見える」
- `mesh_face` がカメラを向いている（`face_billboard_system` 仮実装）

### V-3：~~作業方向コマ切り替えの確認~~ 【上書き済み・廃止】

> **この検証は `character-3d-rendering-proposal` 採用により廃止された。**
> スプライトコマ切り替えはボーンアニメーション（`SoulAnimState.Work` クリップ）に置き換えられるため、
> V-3 に相当する検証は M-3-Char-A（AnimationGraph + SoulAnimState 実装）の完了条件に統合される。

~~壁に隣接した Build タスクを実行させ、キャラクターが壁の方向を向いて見えることを目視確認する。~~

---

## 6. 影響ファイル一覧

| ファイル | 変更種別 | 内容 | 状態 |
| --- | --- | --- | --- |
| `systems/visual/camera_sync.rs` | 変更 | Camera3d 斜め角度への変換式更新・`Z_OFFSET` 定数追加 | ✅ 有効 |
| `hw_core/src/constants/render.rs` | 変更 | `VIEW_HEIGHT`・`Z_OFFSET` 定数追加 | ✅ 有効 |
| ~~`hw_visual/src/billboard.rs`~~ | ~~新規~~ | ~~`Billboard` コンポーネント・`billboard_system`~~ | ❌ 廃止（`face_billboard_system` に置き換え） |
| ~~`hw_visual/src/material/character_billboard.rs`~~ | ~~新規~~ | ~~`CharacterBillboardMaterial`~~ | ❌ 廃止（`CharacterMaterial` に置き換え） |
| `hw_visual/src/lib.rs` | 変更 | ~~`billboard_system`・`MaterialPlugin::<CharacterBillboardMaterial>` を登録~~ → `MaterialPlugin::<CharacterMaterial>` を登録 | 🔄 内容変更 |
| `building_completion/spawn.rs` | 変更 | ~~`SoulProxy3d`/`FamiliarProxy3d` に `Billboard`+`CharacterBillboardMaterial` を付与~~ → GLB ベーススポーンに置き換え・`SoulProxy3d`/`FamiliarProxy3d` 削除 | 🔄 内容変更 |
| ~~`systems/ai/task_execution.rs`~~ | ~~変更~~ | ~~作業方向 → `uv_offset` 切り替えロジック追加~~ | ❌ 廃止（ボーンアニメーション連動に置き換え） |

---

## 7. Phase 3 GLB 設計への影響

この提案で確定した Camera3d 角度は、Phase 3 の GLB モデル設計に以下の形で影響する。

### モデリング方針

セクションビューの存在により、全面を均等に作り込む前提で設計する。

```
通常ビュー（斜め53度）で見える面
  → 正面・上面・側面の一部

矢視・セクションビューで追加で見える面
  → 切断面（底面・背面を含む任意の断面）
  → ユーザーが詳細確認のために使う機能であり LOD0 が要求される

結論：全面を作り込む
  → 「見えない面は省略できる」という最適化は成立しない
  → ポリゴン予算の配分は面ごとの省略ではなく LOD 段数で制御する
```

ただし AI 生成パイプラインにとって角度確定の意義は残る。入力画像の撮影角度を統一することで、生成後のメッシュの向き・品質のばらつきを抑えられる。

### 壁メッシュ構成

メッシュ共通（全壁を同一Cuboid）では外側コーナーの稜線がジオメトリ上に存在しないため、アウトラインのエッジ検出でコーナーが拾えずチープに見える。この問題を解決するため、壁メッシュは以下の構成を採用する。

```
直線部分  → 共通 Cuboid（全バリアントで共有・インスタンシング有効）
コーナー  → wall_corner.glb（L字形状・外側稜線を持つ）
T字      → wall_t_junction.glb
十字     → wall_cross.glb
```

隣接検出は Phase 2 で保持した `wall_connection.rs` の層A（隣接検出ロジック）を Phase 3 で再利用する。フルバリアント（16種）には戻らず、形状が変わる箇所（3〜4種）のみ別GLBとする。

### AI生成パイプラインへの影響

TRELLIS.2 / TripoSR での生成時に、入力画像の撮影角度を確定した Camera3d 角度に合わせることで、生成後の向き調整・回転修正工数を削減できる。生成ツールは入力画像の視点からメッシュを構築するため、撮影角度を統一することで出力品質のばらつきも抑えられる。

---

## 8. 未解決事項（Pending）

| 項目 | 優先度 | タイミング | 状態 |
| --- | --- | --- | --- |
| Camera3d 角度の数値確定（V-1 実施） | P0 | Phase 2 プリミティブ段階 | 未着手 |
| ~~8方向 vs 左右反転のみ の選択~~ | ~~P1~~ | ~~アートスタイル受入基準確定後~~ | ❌ 廃止（ボーンアニメーションで代替。方向数の選択は不要） |
| ~~UV アトラスアニメーションの実装方式確定~~ | ~~P1~~ | ~~MS-Asset-Char-A 確定後~~ | ❌ 廃止（`CharacterMaterial.face_uv_offset` で顔のみ管理。全身コマは不要） |
| ~~`CarryingItemVisual` のビルボード実装~~ | ~~P2~~ | ~~MS-3-1 以降~~ | ❌ 廃止（`character-3d-rendering-proposal` §7 `CarryingItemVisual` の3D表現方針に移管） |

---

## 9. 決定事項サマリ

| 決定内容 | 日付 | 状態 |
| --- | --- | --- |
| 建築物は Camera3d を斜め角度（約53度）に設定して側面を見せる | 2026-03-16 | ✅ 有効 |
| 具体的な角度数値は Phase 2 プリミティブ段階で目視確認して確定する | 2026-03-16 | ✅ 有効 |
| 壁メッシュは直線共通Cuboid＋コーナー/T字/十字の専用GLB（3〜4種）構成とする | 2026-03-16 | ✅ 有効 |
| フルバリアント（16種）には戻らず、`wall_connection.rs` 層Aを Phase 3 で再利用する | 2026-03-16 | ✅ 有効 |
| ~~キャラクターはビルボード方式で既存2Dスプライトアニメーションを流用する~~ | 2026-03-16 | ❌ 上書き済み（`character-3d-rendering-proposal` 2026-03-17） |
| ~~プロキシサイズとスプライトサイズの整合は MS-2B で実測する~~ | 2026-03-16 | ❌ 廃止（スプライトが廃止されるため不要） |
| ~~作業方向コマ切り替えはアートスタイル受入基準（8方向 vs 左右反転）と連動して確定する~~ | 2026-03-16 | ❌ 廃止（ボーンアニメーションに置き換え） |
| ~~キャラクターは `CharacterBillboardMaterial` を持つ3Dビルボードクワッドとして Camera3d で描画する~~ | 2026-03-17 | ❌ 上書き済み（`CharacterMaterial` + GLB に置き換え） |
| ~~キャラクタービルボードにはセクションカットを適用しない（常に表示）~~ | 2026-03-17 | ❌ 上書き済み（`section_clip.wgsl` 経由でクリップを適用する） |
