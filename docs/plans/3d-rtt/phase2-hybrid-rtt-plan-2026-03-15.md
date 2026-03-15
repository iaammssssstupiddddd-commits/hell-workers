# 3D-RtT フェーズ2: ハイブリッドRtT 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `phase2-hybrid-rtt-plan-2026-03-15` |
| ステータス | `Draft` |
| 作成日 | `2026-03-15` |
| 最終更新日 | `2026-03-15` |
| 作成者 | Claude Sonnet 4.6 |
| 関連ロードマップ | `docs/plans/3d-rtt/milestone-roadmap.md` |
| 関連提案 | `docs/proposals/3d-rtt/3d-rendering-rtt-proposal-2026-03-14.md` |
| 依存完了済み | MS-Pre-A, MS-Pre-B, MS-1A〜1D |

---

## 1. 目的

- **解決したい課題**:
  - 壁の16+バリアントスプライト管理が煩雑で、新しい建築タイプ追加時にバリアント数が乗算的に増加する
  - キャラクターと壁の重なり順を `Z_CHARACTER` の手動調整で制御しており、多層階化時に破綻する
  - Camera2d(2D)のみではBuilding/キャラクターを横から見る矢視（立面図）が不可能

- **到達したい状態**:
  - 壁・主要建築物（床・ドア・設備）が Camera3d → RtT レイヤーで描画されている
  - Soul / Familiar の3Dプロキシが存在し、ハードウェアZバッファで前後関係が自動解決される
  - Camera3d の向き切替で矢視（4方向立面図）が動作する

- **成功指標**:
  - `cargo check` ゼロエラー
  - トップダウンでのハイブリッド合成が視覚的に正しい
  - 旧スプライト切替ロジック（16+バリアント、完成Building向け）が削除されている
  - キャラクター↔壁の前後関係が `Z_CHARACTER` 調整なしで成立する

---

## 2. スコープ

### 対象（In Scope）

- Phase 1 引継ぎクリーンアップ: `rtt_test_scene.rs` の削除
- MS-2A: 壁（Wall）の Sprite → 3D Cuboid 置換 + wall_connection.rs の完成Building向けバリアントロジック削除
- MS-2B: Soul / Familiar の最小3Dプロキシ化 + 毎フレーム位置同期
- MS-2C: ハイブリッド段階のZバッファ前後関係検証（手動確認）
- MS-2D: 床（Floor）・ドア（Door）・設備（Tank・MudMixer）の3D化
- MS-Elev: 矢視モード（`ElevationViewState` + Camera3d 4方向プリセット切替）

### 非対象（Out of Scope）

- 3Dモデルのアートスタイル確立（仮 Cuboid / Plane3d のまま）
- PBRライティング・シャドウ（`unlit: true` で統一）
- テレイン（地形）の3D化（Phase 3 のスコープ）
- マウスヒットテストの Raycasting 化（Phase 3 のスコープ）
- Soul / Familiar のアニメーション完全3D化（状態別スプライトは2D側で継続可）
- WFC 地形生成（並行トラックB・独立）

---

## 3. 現状とギャップ

### Phase 1 完了済みの基盤

| コンポーネント | ファイル | 状態 |
| --- | --- | --- |
| `LAYER_2D = 0` / `LAYER_3D = 1` 定数 | `hw_core/src/constants/render.rs` | ✅ |
| `Camera3dRtt` マーカー + `RttTextures` リソース | `plugins/startup/rtt_setup.rs` | ✅ |
| Camera3d（正射影, Y=100, up=NEG_Z）+ RenderTarget | `plugins/startup/mod.rs` | ✅ |
| `sync_camera3d_system`（2D↔3D 毎フレーム同期） | `systems/visual/camera_sync.rs` | ✅ |
| RtT 合成スプライト + テスト3Dキューブ | `plugins/startup/rtt_test_scene.rs` | ⚠️ 削除対象 |

### MS-Pre-B 完了済みの基盤

| コンポーネント | ファイル | 状態 |
| --- | --- | --- |
| `VisualLayerKind` (Floor/Struct/Deco/Light) | `hw_visual/src/layer/mod.rs` | ✅ |
| Building親子構造（親: ロジック, 子: VisualLayerKind + Sprite） | `building_completion/spawn.rs` | ✅ |
| Z定数（`Z_BUILDING_FLOOR=0.05` 〜 `Z_BUILDING_LIGHT=0.18`） | `hw_core/src/constants/render.rs` | ✅ |
| `wall_connection.rs` の `Children` + `VisualLayerKind` 経由更新 | `hw_visual/src/wall_connection.rs` | ✅ |

### ギャップ（Phase 2 で解消する）

| ギャップ | 解消するマイルストーン |
| --- | --- |
| `rtt_test_scene.rs` が残存（テスト立方体が本番ビルドに混入） | Pre-2 クリーンアップ |
| 壁が2D Sprite（16+バリアント切替ロジック存在） | MS-2A |
| Soul / Familiar が2D Spriteのみ（Z値手調整） | MS-2B |
| 床・ドア・設備が2D Sprite | MS-2D |
| 矢視モードが存在しない | MS-Elev |

---

## 4. 実装方針（高レベル）

### 4.1 2D ↔ 3D 座標変換（Phase 1 確認済み）

Camera3d のセットアップ（`rtt_setup.rs`）および `camera_sync.rs` で確認済みのマッピング:

```
2D 世界座標: x = 右, y = 上
3D 世界座標: x = 右, y = 高さ（上方向）, z = 手前（Camera3d up=NEG_Z のため 2D の +y = 3D の -z）

変換式:
  3d_pos.x = 2d_pos.x
  3d_pos.y = object_height / 2.0   // 地面（y=0）からオブジェクト高さの中心
  3d_pos.z = -2d_pos.y             // 符号反転（camera_sync.rs: cam3d.z = -cam2d.y と同じルール）
```

例: グリッド (gx, gy) のワールド座標 `(gx * TILE_SIZE, gy * TILE_SIZE)` → 3D: `(gx * TILE_SIZE, height/2, -(gy * TILE_SIZE))`

### 4.2 静的建築物: 独立3Dビジュアルエンティティ方式

**課題**: Building 親エンティティは2D Transform（`(x, y, z_struct)`）を持つ。Bevy の親子階層では子の GlobalTransform が親の2D Transform を継承してしまうため、3D 描画に正しい座標を与えられない。

**解決策**: 3Dビジュアルを Building の子にせず、**トップレベルの独立エンティティ**として spawn する。

```rust
// 完成時（building_completion/spawn.rs 内）に別途 spawn
commands.spawn((
    Building3dVisual { owner: building_entity },   // 参照コンポーネント
    Mesh3d(handles.wall_mesh.clone()),
    MeshMaterial3d(handles.wall_material.clone()),
    Transform::from_xyz(world_x, TILE_SIZE / 2.0, -world_y),
    RenderLayers::layer(LAYER_3D),
));
```

Building が除去された際のクリーンアップは `RemovedComponents<Building>` を使用するシステムで対応する。

### 4.3 動的キャラクター: 3Dプロキシ + 毎フレーム同期

キャラクター（Soul / Familiar）は毎フレーム移動するため、Building と同じ「独立3Dエンティティ」方式を採用しつつ、位置を毎フレーム同期するシステムを追加する。

```rust
// Proxy コンポーネント
#[derive(Component)] pub struct SoulProxy3d { pub owner: Entity }
#[derive(Component)] pub struct FamiliarProxy3d { pub owner: Entity }

// 毎フレーム同期システム
fn sync_soul_proxies_3d(
    q_souls: Query<(Entity, &Transform), With<DamnedSoul>>,
    mut q_proxies: Query<(&SoulProxy3d, &mut Transform), Without<DamnedSoul>>,
) {
    for (proxy, mut proxy_tf) in q_proxies.iter_mut() {
        if let Ok((_, soul_tf)) = q_souls.get(proxy.owner) {
            let pos = soul_tf.translation;
            proxy_tf.translation.x = pos.x;
            proxy_tf.translation.z = -pos.y; // 2D y → 3D -z
        }
    }
}
```

### 4.4 壁バリアントロジックの廃止

Cuboid を各壁グリッドに1つ配置することで、隣接するキューブが物理的に接触して「繋がって見える」:

- 現在: 16+種テクスチャ（孤立 / 直線 / 曲がり / T字 / 十字等）を接続パターンに応じて切替
- 3D化後: 単一の `Cuboid(TILE_SIZE, TILE_SIZE, TILE_SIZE)` を配置するだけ（バリアント選択不要）

`wall_connection.rs` の完成 Building 向けスプライト更新ロジックは削除し、Blueprint（配置プレビュー）向けのスプライト更新のみ残す。

### 4.5 3Dアセットハンドルの管理

新規リソース `Building3dHandles` を `visual_handles.rs` に追加し、`init_visual_handles` で初期化する。Phase 2 では全て `unlit: true` のプロシージャルメッシュ（Bevy 組込み Primitive）を使用する。

```rust
#[derive(Resource)]
pub struct Building3dHandles {
    pub wall_mesh:           Handle<Mesh>,
    pub wall_material:       Handle<StandardMaterial>,
    pub floor_mesh:          Handle<Mesh>,
    pub floor_material:      Handle<StandardMaterial>,
    pub door_mesh:           Handle<Mesh>,
    pub door_material:       Handle<StandardMaterial>,
    pub equipment_mesh:      Handle<Mesh>,  // Tank・MudMixer（2×2グリッド）
    pub equipment_material:  Handle<StandardMaterial>,
    pub soul_mesh:           Handle<Mesh>,
    pub familiar_mesh:       Handle<Mesh>,
    pub character_material:  Handle<StandardMaterial>,
}
```

---

## 5. マイルストーン

---

### Pre-2: rtt_test_scene 削除

> **依存**: なし（Phase 2 開始前の前提クリーンアップ）

- **やること**:
  1. `crates/bevy_app/src/plugins/startup/rtt_test_scene.rs` を**ファイルごと削除**
  2. `crates/bevy_app/src/plugins/startup/mod.rs`:
     - `mod rtt_test_scene;` 行を削除
     - `PostStartup` から `rtt_test_scene::spawn_rtt_composite_sprite` を削除
     - `PostStartup` から `rtt_test_scene::spawn_test_cube_3d` を削除

- **変更ファイル**:
  - `crates/bevy_app/src/plugins/startup/rtt_test_scene.rs`（削除）
  - `crates/bevy_app/src/plugins/startup/mod.rs`（3箇所削除）

- **完了条件**: `cargo check` 通過、テスト立方体・合成スプライトが画面から消える
- **ステータス**: [ ] 未着手

---

### MS-2A: 壁セグメントの3D配置

> **依存**: Pre-2 完了、MS-Pre-B 完了（Building 親子構造が前提）

- **やること**:

  **M2A-1: Building3dHandles リソース追加**
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs` に `Building3dHandles` リソースを定義
  - `init_visual_handles` システム内で初期化（`meshes.add(...)` / `materials.add(...)`）し、`commands.insert_resource(...)` で登録する
  - ⚠️ `app.init_resource::<Building3dHandles>()` は**使用しない**。`Handle<Mesh>` / `Handle<StandardMaterial>` は `Default` で空ハンドルしか作れないため、`PostStartup` の `init_visual_handles` 内で初期化するのが正しい方式（既存の `WallVisualHandles` 等と同じパターン）

  ```rust
  // 壁（1×1グリッド）
  wall_mesh:     meshes.add(Cuboid::new(TILE_SIZE, TILE_SIZE, TILE_SIZE))
  wall_material: materials.add(StandardMaterial {
      base_color: Color::srgb(0.55, 0.45, 0.35),  // 仮: 石・泥の色調
      unlit: true,
      ..default()
  })
  ```

  **M2A-2: spawn.rs で Wall 完成時に3Dエンティティを spawn**
  - `spawn_completed_building` 関数に `Building3dHandles` を引数追加
  - `BuildingType::Wall` の場合のみ: `Building3dVisual` コンポーネントを持つ独立3Dエンティティを spawn
  - 位置: `Transform::from_xyz(world_pos.x, TILE_SIZE / 2.0, -world_pos.y)`

  **M2A-3: wall_connection.rs の完成Building向けロジック削除**
  - `q_children: Query<&Children>` と `q_visual_layers: Query<(&VisualLayerKind, &mut Sprite)>` 経由で Sprite を更新している完成Building向けブロックを削除
  - Blueprint（`q_blueprint_sprites`）向けのスプライト更新は**そのまま残す**
  - `WallVisualHandles` は Blueprint 用に引き続き必要なため削除しない

  **M2A-4: Wall3D クリーンアップシステムの追加**
  - `RemovedComponents<Building>` を監視し、対応する `Building3dVisual` エンティティを despawn するシステムを追加
  - `Building3dVisual { owner: Entity }` コンポーネントを定義
  - ⚠️ クリーンアップシステムは必ず **`bevy_app`** に置くこと。`hw_visual/src/CLAUDE.md` のcrate境界ルールにより、新規コードで `hw_jobs::Building` を直接インポートすることは禁止されている。`Building3dVisual` コンポーネント定義自体は `hw_visual/src/visual3d.rs` で可（`Entity` のみ持つため）。

- **変更ファイル**:
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs`（Building3dHandles 追加）
  - `crates/bevy_app/src/plugins/startup/mod.rs`（init_resource 追加）
  - `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`（Wall3D spawn 追加）
  - `crates/hw_visual/src/wall_connection.rs`（完成Building向けスプライト更新ブロック削除）
  - `crates/bevy_app/src/systems/visual/` （クリーンアップシステム追加・登録）※ `hw_visual` には置かない
  - 新規: `Building3dVisual` コンポーネント定義ファイル（`hw_visual/src/visual3d.rs` 等）

- **完了条件**:
  - [ ] トップダウン視点で壁の見た目が正しく表示される
  - [ ] 完成Building向け壁バリアントのスプライト切替ロジックが削除されている（`WallVisualHandles` のバリアントテクスチャ選択ロジックが Building 向けに呼ばれていない）
  - [ ] Blueprint の壁プレビューは引き続き正常に動作する
  - [ ] `cargo check` 通過
- **ステータス**: [ ] 未着手

---

### MS-2B: Zソート問題の検証（Characterプロキシ3D化）

> **依存**: MS-2A 完了

- **やること**:

  **M2B-1: SoulProxy3d / FamiliarProxy3d コンポーネント定義**
  - `hw_visual/src/visual3d.rs`（M2A で作成済み）に追加

  **M2B-2: Building3dHandles にキャラクター用メッシュ追加**
  - Soul プロキシ: `Cuboid::new(TILE_SIZE * 0.6, TILE_SIZE * 0.8, TILE_SIZE * 0.6)` 相当の仮メッシュ
  - Familiar プロキシ: Soul より一回り大きい仮メッシュ
  - `character_material`: `StandardMaterial { base_color: Color::srgb(0.9, 0.9, 0.9), unlit: true, .. }`

  **M2B-3: Soul spawn 時に SoulProxy3d エンティティを同時 spawn**
  - `entities/damned_soul/spawn.rs` の `spawn_soul` 内で `commands.spawn(SoulProxy3d{...})` を追加
  - 既存の2D Sprite は **残したまま** spawn を続ける（検証用に2Dも表示することで視覚確認が容易）
  - ⚠️ MS-2C の検証完了後、Zバッファ動作が確認できた段階でSoulの2D Sprite を除去する

  **M2B-4: Familiar spawn 時に FamiliarProxy3d エンティティを同時 spawn**
  - `entities/familiar/spawn.rs` の `spawn_familiar` 内で同様に追加

  **M2B-5: 3Dプロキシ位置同期システムの追加**
  - `crates/bevy_app/src/systems/visual/character_proxy_3d.rs`（新規）に以下のシステムを実装:
    - `sync_soul_proxy_3d`: DamnedSoul の Transform を SoulProxy3d エンティティに毎フレーム同期
    - `sync_familiar_proxy_3d`: Familiar の Transform を FamiliarProxy3d エンティティに毎フレーム同期
  - `Visual` スケジュール（`SystemSet::Visual`）に登録する

  **M2B-6: キャラクタープロキシのクリーンアップシステムを追加**
  - `RemovedComponents<DamnedSoul>` / `RemovedComponents<Familiar>` を監視して対応プロキシを despawn

  **M2B-7: 2D Sprite の除去（MS-2C 検証完了後）**
  - Zバッファで前後関係が正しく解決されることを MS-2C で確認後に実施
  - Soul の2D Sprite を spawn コードから削除
  - Familiar の2D Sprite を spawn コードから削除
  - `Z_CHARACTER` を参照している Soul / Familiar 関連コードを整理

- **検証シナリオ**（M2B-3〜5 完了後に手動確認）:
  1. 壁の背後に Soul を移動させ、壁に隠れることを確認
  2. 壁の手前を Soul が通るとき、壁より手前に表示されることを確認
  3. Familiar と Soul が重なる際、高さ順で正しく描画されることを確認

- **変更ファイル**:
  - `crates/bevy_app/src/entities/damned_soul/spawn.rs`（SoulProxy3d spawn 追加、後で Sprite 削除）
  - `crates/bevy_app/src/entities/familiar/spawn.rs`（FamiliarProxy3d spawn 追加、後で Sprite 削除）
  - `crates/hw_visual/src/visual3d.rs`（SoulProxy3d, FamiliarProxy3d コンポーネント追加）
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs`（キャラクターメッシュ追加）
  - 新規: `crates/bevy_app/src/systems/visual/character_proxy_3d.rs`
  - `crates/bevy_app/src/systems/visual/mod.rs`（システム登録追加）

- **完了条件**:
  - [ ] SoulProxy3d / FamiliarProxy3d が3Dシーンに存在し、Camera3d（RtT）で描画される
  - [ ] キャラクターを含む重なりがハードウェアZバッファで自然に解決される
  - [ ] 2D側の `Z_CHARACTER` 調整で帳尻を合わせる必要がない
  - [ ] `cargo check` 通過
- **ステータス**: [ ] 未着手

---

### MS-2C: ハイブリッド段階の前後関係検証

> **依存**: MS-2B 完了

- **やること**:
  - 以下の全検証シナリオを手動で再現し、前後関係が自然に解決されることを確認する
  - Zバッファ正常動作が確認できた場合: M2B-7（2D Sprite 除去）を実施

- **検証シナリオ**:

  | シナリオ | 期待挙動 |
  | --- | --- |
  | 壁の背後に移動したSoulが壁に隠れる | 壁のCuboidがSoulプロキシより手前に描画される |
  | 壁の手前を通るSoulが壁より手前に表示される | SoulプロキシがCuboidより手前に描画される |
  | 壁の角でSoulが作業している | Z値手調整なしで見た目が崩れない |
  | Soul と Familiar が同一グリッドに重なる | 高さ別に正しく前後が決まる（Zバッファ） |
  | 壁とアイテム（Resource）が重なる | アイテムが壁の上に正しく描画される（アイテムは2Dのまま） |

- **問題発生時の対応**:
  - 3D側（壁・プロキシ）間の深度問題 → 3D y 軸高さを調整
  - 3D（RtT合成）と2D（アイテム等）の合成上の問題 → RtT合成スプライトの Z 値や `alpha_mode` を調整

- **完了条件**: Phase 2 の対象物では Z値管理コードを追加せずに前後関係が正しく描画される
- **ステータス**: [ ] 未着手

---

### MS-2D: 床・ドア・家具の3D化

> **依存**: MS-2C 完了

- **やること**: Building3dHandles に各種メッシュ・マテリアルを追加し、`spawn_completed_building` を BuildingType ごとに拡張する。

  | BuildingType | 使用3Dプリミティブ | サイズ | 備考 |
  | --- | --- | --- | --- |
  | `Floor` | `Plane3d` (水平面) | TILE_SIZE × TILE_SIZE | y=0（地面レベル）に配置 |
  | `Door` | `Cuboid` | TILE_SIZE × TILE_SIZE × TILE_SIZE | 壁と同じサイズ（開閉アニメーション準備のため親子構造化を検討）⚠️ MS-2A 完了〜MS-2D 実装前は2D Sprite のまま残存する（許容済み） |
  | `Tank` | `Cuboid` | TILE_SIZE×2 × TILE_SIZE×1.5 × TILE_SIZE×2 | 2×2 グリッド仮モデル |
  | `MudMixer` | `Cuboid` | TILE_SIZE×2 × TILE_SIZE×1.5 × TILE_SIZE×2 | 2×2 グリッド仮モデル |
  | `RestArea` | `Plane3d` | TILE_SIZE×2 × TILE_SIZE×2 | 床扱い（仮） |
  | `Bridge` | `Plane3d` | TILE_SIZE×2 × TILE_SIZE×5 | 床扱い（仮） |
  | `SandPile`、`BonePile` | `Plane3d` | TILE_SIZE × TILE_SIZE | 地面堆積物扱い（仮） |
  | `WheelbarrowParking` | `Plane3d` | TILE_SIZE × TILE_SIZE | 床扱い（仮） |

- **Tank・MudMixer の state-dependent ビジュアル対応**:
  - 現在 `hw_visual/src/tank.rs` と `hw_visual/src/mud_mixer.rs` が `VisualLayerKind::Struct` 子エンティティの Sprite を更新している
  - Phase 2 では 3D 側の材料色（`base_color`）の変更で代替する（`MaterialHandle` の差し替えか `base_color` 書き換え）
  - または MS-2C 後の判断として3D化を延期し、2D Sprite のままにすることも可
  - **推奨**: MS-2C 完了後にスコープを確定する

- **変更ファイル**:
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs`（Building3dHandles 拡張）
  - `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`（各 BuildingType の3Dエンティティ spawn 追加）
  - `crates/hw_visual/src/tank.rs`（3D対応 or 延期）
  - `crates/hw_visual/src/mud_mixer.rs`（3D対応 or 延期）

- **完了条件**:
  - [ ] Phase 2 終了時点で「地形以外の主要インゲーム要素を RtT へ移す前提」が成立している
  - [ ] `cargo check` 通過
  - [ ] トップダウン視点で各BuildingTypeが適切に表示される
- **ステータス**: [ ] 未着手

---

### MS-Elev: 矢視（立面図）4方向切替

> **依存**: MS-2A 完了（壁の実際の3Dモデルで動作確認するため）
> **対象**: MS-2A〜MS-2D の合間に実装可能（独立性が高い）

- **やること**:

  **M-Elev-1: ElevationViewState リソース定義**

  ```rust
  // crates/hw_core/src/components/ または crates/bevy_app/src/app_contexts.rs に追加
  #[derive(Resource, Default, PartialEq, Eq, Debug, Clone, Copy)]
  pub enum ElevationViewState {
      #[default]
      TopDown,
      North,  // 南→北方向（正面）
      South,  // 北→南方向（背面）
      East,   // 西→東方向（右側面）
      West,   // 東→西方向（左側面）
  }
  ```

  **M-Elev-2: Camera3d Transform 切替システム**
  - ⚠️ `systems/visual/camera_sync.rs` の `sync_camera3d_system` は毎フレーム Camera3d 位置を Camera2d に追従させる。矢視モード中にこのシステムが動くと Camera3d の向きが上書きされる。`ElevationViewState != TopDown` のとき `sync_camera3d_system` をスキップする条件分岐を追加すること。
  - `ElevationViewState` が変わったとき `Camera3dRtt` の Transform を4プリセットに切替:

  ```
  TopDown: looking_at(target, Vec3::NEG_Z) から真上（既存の正射影俯瞰）
  North:   Transform::from_xyz(target.x, d, target.z - d).looking_at(target, Vec3::Y)
  South:   Transform::from_xyz(target.x, d, target.z + d).looking_at(target, Vec3::Y)
  East:    Transform::from_xyz(target.x + d, d, target.z).looking_at(target, Vec3::Y)
  West:    Transform::from_xyz(target.x - d, d, target.z).looking_at(target, Vec3::Y)
  ```
  - `d` は現在のズームレベルに応じた距離パラメータ

  **M-Elev-3: 矢視中のテレイン非表示**
  - `ElevationViewState != TopDown` のとき、地形タイルエンティティ（`RenderLayers::layer(LAYER_2D)` の地形スプライト）を `Visibility::Hidden` に切替
  - `TopDown` に戻ったとき `Visibility::Inherited` に戻す

  **M-Elev-4: 矢視切替UI（最低限）**
  - キーバインド（例: `Tab` または `1/2/3/4/5` キー）で `ElevationViewState` を切替
  - UI ボタン追加はオプション（Phase 2 では最低限のキーバインドで可）

- **変更ファイル**:
  - 新規: `ElevationViewState` リソース定義（`hw_core` か `bevy_app`）
  - 新規または既存: Camera3d 切替システム（`systems/visual/elevation_view.rs`）
  - `systems/visual/mod.rs`（システム登録）
  - `plugins/input/` または `plugins/startup/mod.rs`（リソース init_resource 追加）
  - 地形スプライト Visibility 制御

- **完了条件**:
  - **MS-2A 後（第1段階）**: Camera3d の Transform 切替インフラが動作する（壁 Cuboid で4方向から確認）
  - **MS-2D 後（最終完了）**: 実際の壁・床3Dモデルで4方向が正しく表示される
- **ステータス**: [ ] 未着手

---

## 6. リスクと対策

| リスク | 影響度 | 対策 |
| --- | --- | --- |
| 2D Building Transform と3D座標系の非互換 | 大（位置ズレで見た目が崩壊） | 独立3Dエンティティ方式で回避。Building エンティティの Transform は絶対に変更しない（既存ロジックが依存） |
| Building3dVisual のクリーンアップ漏れ（building 削除時） | 中（幽霊3Dオブジェクト残存） | `RemovedComponents<Building>` 監視システムを MS-2A で必ず追加すること |
| wall_connection.rs の Blueprint 向けロジック破壊 | 中（壁配置プレビューが崩れる） | M2A-3 では完成Building向けのみ削除し、`q_blueprint_sprites` 経由のロジックは必ず残す |
| tank.rs / mud_mixer.rs の VisualLayerKind 更新が3D化後に無効化 | 中（設備の視覚状態が更新されなくなる） | MS-2D スコープ確定時に対処方針を決める。3D対応を延期する場合は2D Sprite を残す |
| Familiar のアウラ3子エンティティ（Border/Outline/Pulse）の扱い | 小（アウラが3D側に映り込む可能性） | M2B-4 では Familiar 本体のプロキシのみ spawn。アウラ用エンティティは `RenderLayers::layer(LAYER_2D)` のまま維持する |
| キャラクタープロキシ同期の位相ズレ（2D Sprite と3D Meshがズレて見える） | 小（MS-2B 検証期間のみ） | 同期システムを `Visual` スケジュール末尾に配置することで最小化。MS-2C 完了後に2D Sprite を除去すれば解消 |

---

## 7. 検証計画

### 必須（全マイルストーン共通）

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
```

### 手動確認シナリオ

| マイルストーン | 確認項目 |
| --- | --- |
| Pre-2 | テスト立方体・合成スプライトが画面から消えている |
| MS-2A | 壁がトップダウンで表示され、隣接する壁が接触している / Blueprint プレビューが正常 |
| MS-2B（段階1） | 2D Sprite と3Dプロキシが同位置に重なって表示される（確認用） |
| MS-2B（M2B-7後） | 壁背後に入ったキャラクターが隠れ、手前では表示される |
| MS-2C | 上記の全検証シナリオが Z 値追加コードなしでパスする |
| MS-2D | 各BuildingTypeが正しい形状・位置でトップダウン表示される |
| MS-Elev | `Tab`/キーで矢視切替 → 4方向から壁・建築物が正しく見える |

---

## 8. ロールバック方針

- **単位**: 各マイルストーン単位で `git revert` 可能
- **手順**:
  - MS-2A のロールバック: `Building3dVisual` spawn を削除し、`wall_connection.rs` の完成Building向けスプライト更新ブロックを復元
  - MS-2B のロールバック: `SoulProxy3d` / `FamiliarProxy3d` spawn を削除し、2D Sprite を復元
  - 2D スプライトは BuildingType/キャラクター別に独立して復元可能

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（本計画書作成完了、実装未着手）
- 前提完了済み: MS-Pre-A, MS-Pre-B, MS-1A〜1D（Phase 1）
- 残存削除対象: `rtt_test_scene.rs`（Pre-2 で最初に削除すること）

### 最初にやること

1. `rtt_test_scene.rs` を削除し、`startup/mod.rs` の参照3箇所を削除する（Pre-2）
2. `cargo check` でコンパイル確認
3. MS-2A の M2A-1（Building3dHandles）から実装を開始する

### 絶対に守るべき制約

- **Building エンティティの2D Transform を変更してはいけない**: `wall_connection.rs` 等、多くのシステムが `transform.translation.truncate()` で2Dグリッド座標を取得している
- **3D エンティティは必ず `RenderLayers::layer(LAYER_3D)` を付与すること**: ないと Camera2d にも映り込む
- **`wall_connection.rs` の Blueprint 向けロジックは残す**: `q_blueprint_sprites`（`Without<VisualLayerKind>`）経由の更新は削除禁止
- **`unlit: true` で統一**: Phase 2 では照明・シャドウは不使用
- **全 Bevy API は 0.18 系で確認**: `Mesh3d`, `MeshMaterial3d`, `Plane3d`, `Cuboid` は 0.18 での正しい書き方を `docsrs-mcp` または `~/.cargo/registry/src/` で確認すること

### Bevy 0.18 API 確認済み事項

- `Cuboid::new(x, y, z)` → `meshes.add(Cuboid::new(...))` → `Handle<Mesh>`
- `Plane3d` は `Plane3d::new(normal)` か `Plane3d::default()` で作成、サイズは `meshes.add(Plane3d::default().mesh().size(w, h))` で指定
- 3Dオブジェクト spawn: `(Mesh3d(handle), MeshMaterial3d(mat_handle), Transform::..., RenderLayers::layer(1))`（`PbrBundle` は 0.18 で廃止済み）
- `RenderLayers` のパス: `bevy::camera::visibility::RenderLayers`（prelude 外）
- Camera3d の向き: `looking_at(Vec3::ZERO, Vec3::NEG_Z)` が俯瞰用（up=NEG_Z が必須）

### 参照必須ファイル

| ファイル | 参照目的 |
| --- | --- |
| `crates/bevy_app/src/plugins/startup/mod.rs` | Camera3d セットアップ確認・システム登録 |
| `crates/bevy_app/src/plugins/startup/rtt_setup.rs` | RttTextures, Camera3dRtt 定義 |
| `crates/bevy_app/src/plugins/startup/visual_handles.rs` | Building3dHandles 追加場所 |
| `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs` | 3D エンティティ spawn 追加場所 |
| `crates/hw_visual/src/wall_connection.rs` | バリアントロジック削除箇所の特定 |
| `crates/hw_visual/src/layer/mod.rs` | VisualLayerKind 定義 |
| `crates/hw_visual/src/tank.rs`, `mud_mixer.rs` | MS-2D 対応 or 延期の判断 |
| `crates/bevy_app/src/entities/damned_soul/spawn.rs` | SoulProxy3d spawn 追加場所 |
| `crates/bevy_app/src/entities/familiar/spawn.rs` | FamiliarProxy3d spawn 追加場所 |
| `crates/bevy_app/src/systems/visual/camera_sync.rs` | 2D↔3D 座標マッピングのリファレンス |
| `crates/hw_core/src/constants/render.rs` | LAYER_2D / LAYER_3D / Z定数 |
| `crates/hw_core/src/constants/world.rs` | TILE_SIZE = 32.0 |

### 座標変換クイックリファレンス

```rust
// 2D ワールド座標 (x, y) → 3D ワールド座標
// Camera3d は up=NEG_Z で俯瞰しているため 2D y = 3D -z
fn pos_2d_to_3d(x: f32, y: f32, height_y: f32) -> Vec3 {
    Vec3::new(x, height_y, -y)
}

// 壁の場合（1×1グリッド, 地面y=0 から高さ TILE_SIZE の中心）
Transform::from_xyz(world_x, TILE_SIZE / 2.0, -world_y)

// 床の場合（地面 y=0）
Transform::from_xyz(world_x, 0.0, -world_y)

// キャラクターの場合（2D transform から毎フレーム同期）
let pos_2d = soul_tf.translation;  // (pos.x, pos.y, Z_CHARACTER)
proxy_tf.translation = Vec3::new(pos_2d.x, TILE_SIZE * 0.4, -pos_2d.y);
```

### Definition of Done（Phase 2 全体）

- [ ] Pre-2: rtt_test_scene.rs 削除完了
- [ ] MS-2A: 壁の3D表示・旧バリアントロジック削除完了
- [ ] MS-2B: キャラクター3Dプロキシ + 同期システム実装、2D Sprite 除去完了
- [ ] MS-2C: 全検証シナリオ（Zバッファ前後関係）パス
- [ ] MS-2D: 床・ドア・設備の3D化完了
- [ ] MS-Elev: 矢視4方向切替動作確認
- [ ] `cargo check` ゼロエラー・ゼロ警告（可能な限り）
- [ ] `docs/plans/3d-rtt/milestone-roadmap.md` のステータス更新

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-15` | Claude Sonnet 4.6 | 初版作成。コードベース調査（phase1引継ぎメモ・Pre-B実装・各エンティティspawn確認）に基づき策定。 |
