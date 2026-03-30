# MS-3-4 テレイン 3D 化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `ms-3-4-terrain-3d-plan-2026-03-29` |
| ステータス | `Draft` |
| 作成日 | `2026-03-29` |
| 最終更新日 | `2026-03-29` |
| 親マイルストーン | `docs/plans/3d-rtt/milestone-roadmap.md` **MS-3-4** |
| 前提 MS | **MS-3-3**（`SectionMaterial` 基盤・wall pilot 接続済み） |
| 関連提案 | `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md` |
| アセット計画 | `docs/plans/3d-rtt/asset-milestones-2026-03-17.md`（MS-Asset-Terrain：転用確認） |
| 後続 MS | **MS-3-6**（表面表現・境界ブレンド）、**MS-3-7**（Raycast ヒットテスト） |

---

## 1. 目的

### 解決したい課題

- 地形が **Camera2d + `Sprite`** と **`terrain_border` のオーバーレイ Sprite** で描画されており、**Camera3d → RtT** のワールドとレイヤーが分断している。
- 矢視時の **section clip**（`SectionCut`）と地形が同一パイプライン上にないため、Phase 3 の「インゲームは RtT 一本」に到達できない。
- `hw_world::generate_terrain_border_specs` / `terrain_border.rs` に依存した **90° 境界テクスチャ**は、MS-3-6 で廃止予定だが、MS-3-4 時点で **表現の単一化**を始める必要がある。

### 到達したい状態

- **地形メッシュ**が `LAYER_3D` で **Camera3dRtt** にのみ描画され、合成は既存 **RtT composite** 経路のまま成立する。
- **Camera2d 側にインゲーム用地形 Sprite が残らない**（UI・Familiar・オーバーレイは対象外）。
- `TerrainBorder` エンティティおよび `spawn_terrain_borders` への依存を **MS-3-4 完了時点で除去**する（見た目は平坦タイル＋単一テクスチャでよい。有機的な境界は MS-3-6）。

### 成功指標（ロードマップとの整合）

- [ ] `cargo check --workspace` ゼロエラー、`cargo clippy` ワークスペース方針に準拠
- [ ] 地形が **Camera3d → RtT のみ**で見える（TopDown・矢視の両方で退行なし）
- [ ] `terrain_border.rs` のスポーンが登録フローから外れ、`borders` スペック生成に依存しない

---

## 2. スコープ

### 対象（In Scope）

- `crates/bevy_app/src/world/map/spawn.rs`：`spawn_map` の各タイルを **`Mesh3d` + マテリアル**へ置換（`Sprite` 削除）。
- **タイル 1 セル 1 エンティティ**は維持し、`WorldMap::set_tile_entity_at_idx` の契約を壊さない（後述の既存利用者）。
- `RenderLayers::layer(LAYER_3D)`（必要なら shadow receiver 方針は建物と揃えて文書化）。
- `crates/bevy_app/src/world/map/terrain_border.rs` および `startup` の `spawn_terrain_borders_if_enabled` 呼び出しの **削除または恒久的無効化**（環境変数でのスキップだけに残さない）。
- `hw_world::terrain_visual::obstacle_cleanup_system`：**`Sprite` 前提の `Query<&mut Sprite>`** を、地形が 3D になった後の **マテリアル／テクスチャ差し替え**へ更新。
- `docs/architecture.md` の RtT / マップ関連、`docs/world_layout.md`（座標・レイヤー説明が変わる場合）の更新。

### 非対象（Out of Scope）

- **MS-3-6**：テクスチャブレンド・ノイズ・境界の有機化、`terrain/*.png` オーバーレイ相当の高品質化。
- **MS-3-7**：`viewport_to_world_2d` の Raycast 置換。MS-3-4 中は既存 2D カメラベースの入力が地形クリックで破綻する場合、**既知の制限**として計画に記録し、MS-3-7 で解消する。
- **地形用 GLB の新規大量制作**：タイルは共有 **平面メッシュ**＋既存 `grass/sand/dirt/river` テクスチャで足りる想定（`asset-milestones` の MS-Asset-Terrain は転用確認が主）。
- **WFC / 手続き的地形生成**（別トラック `wfc-terrain-generation-plan`）。

---

## 3. 現状とギャップ

### 現状（コードの事実）

| 箇所 | 内容 |
| --- | --- |
| `world/map/spawn.rs` | 各タイルに `Tile` + `Sprite`（`GameAssets` の地形 `Image`）+ `Transform`（`Z_MAP` 系の 2D Z） |
| `world/map/terrain_border.rs` | `TerrainBorder` + `Sprite` で `grass_edge` 等を重ねる |
| `hw_world/src/terrain.rs` | `TerrainType::z_layer()` で 2D Z を分離（Grass > Dirt > Sand > River） |
| `hw_world/src/terrain_visual.rs` | 障害物削除時に `tile_entity` の **`Sprite.image`** を `dirt` に差し替え |
| `hw_familiar_ai/.../direct_collect.rs` | **`tile_entity_at_idx`** でタイル Entity を取得し、`TaskState`（Designation 等）を参照して採取対象を選ぶ |

### ギャップ

- 3D ワールドでは建物と同様 **XZ 平面＋ Y 上向き**が主で、`spawn_map` の 2D `Transform` は RtT カメラと一致しない。**`grid_to_world` の X/Y を 3D の X / -Z（および適切な Y）に写像**する必要がある（既存 `building_completion` の 3D spawn パターンに合わせる）。
- `Tile` Entity は **ゲームロジックの錨**なので削除できない。変えるのは **表現コンポーネント**（`Sprite` → `Mesh3d` + `MeshMaterial3d<…>`）。
- `SectionMaterial` を地形に載せるか：**矢視で地形もスラブクリップに含める**なら `SectionMaterial`（または建物と同じ `ExtendedMaterial` 経路）が一貫。初期実装は **壁と同じ `SectionMaterial` + `SectionCut` 同期**を前提にする（MS-3-3 済みの `sync_section_cut_to_materials` が地形も拾うようクエリ拡張が必要になる可能性が高い）。

---

## 4. 実装方針（高レベル）

### 4.1 メッシュ戦略

- **推奨（初期）**：タイルごとに **薄い `Plane3d` / `Quad` メッシュ**（または共有 `Mesh` ハンドル 1 つを全タイルが参照）を使用し、`TILE_SIZE` にスケール。
- **代替（後続最適化）**：チャンク結合メッシュでドローコール削減。MS-3-4 ではスコープ外とし、必要なら別タスク化。

### 4.2 マテリアル

- **方針 A（推奨）**：`MeshMaterial3d<SectionMaterial>`（建物 wall pilot と同型）にし、`StandardMaterial` 側にベースカラー／アルベドとして既存 `Handle<Image>` を設定。`SectionCut` を terrain マテリアルにも伝播するよう **section 同期システムのクエリ**を拡張。
- **方針 B（切り戻し用）**：最初の PR でだけ `StandardMaterial` を使い、次 PR で `SectionMaterial` に寄せる二段階も可。ただしロードマップ完了条件「フル RtT」と section 一貫性を満たすには **方針 A が最終形**。

### 4.3 深度・レイヤー

- 2D 時代の `TerrainType::z_layer()` は **3D では無意味または Y オフセット微調整に再解釈**する。川・砂・土・草の **見た目の重なり**は、3D では **同一 Y + 描画順／depth bias** または **微小 Y オフセット**で再現するか、MS-3-6 まで単純平坦に寄せるかを実装時に決定し、`docs/world_layout.md` に記録。

### 4.4 `TerrainBorder` の廃止

- `spawn_terrain_borders` を呼ばない。既存 `TerrainBorder` エンティティを despawn するマイグレーションは **起動時に一度**でよい（開発ビルド想定）。
- `hw_world/borders.rs` の **削除は必須ではない**（他用途がなければ後続で dead code 整理）。MS-3-4 の完了条件は **「表現がオーバーレイに依存しない」**こと。

### 4.5 Bevy 0.18 での注意

- `Mesh3d` / `MeshMaterial3d` / `RenderLayers` の付け方は **既存 `building_completion/spawn.rs` の 3D 建物**に合わせる。
- `SectionMaterial` の prepass / shadow の挙動は wall pilot と同じく **ライトの `render_layers` とカメラの交差**を維持する（`architecture.md` の DirectionalLight 記述参照）。

---

## 5. 実装ステップ（推奨順序）

### M1: 3D タイルスポーン（境界なし）

- `spawn_map` で `Sprite` をやめ、`Mesh3d` + `MeshMaterial3d`（仮で `StandardMaterial` でも可）を付与。
- `Transform` を 3D 座標系に変換し、`LAYER_3D` を付与。
- `cargo check`、起動して RtT 上に地形が出ることを確認。

### M2: SectionMaterial 化 + Section 同期

- 地形用 `SectionMaterial` の生成パターンを wall と揃える（`visual_handles` またはローカル `Assets` 登録）。
- `systems/visual/section_cut.rs`（および関連）で **地形エンティティ**もマテリアル更新対象に含める。
- 矢視 + `SectionCut.active` で地形がクリップされることを目視。

### M3: 境界オーバーレイ撤去

- `spawn_terrain_borders_if_enabled` を `PostStartup` から外す。`terrain_border.rs` を削除するか、プラン完了までに空実装へ。

### M4: `terrain_visual` / 採取・Designation 連携の検証

- `obstacle_cleanup_system` を **3D マテリアル更新**に変更（`TerrainVisualHandles` は `dirt` テクスチャのまま利用可能）。
- `direct_collect` 等 **`tile_entity_at_idx` 利用箇所**の回帰テスト（手動：岩撤去後タイルが土に見える、採取 Designation が付く）。

### M5: ドキュメント

- `docs/architecture.md`：地形が RtT パイプライン側であること。
- `docs/world_layout.md`：2D Z 定数から 3D への移行メモ。
- `milestone-roadmap.md` の MS-3-4 チェックボックス更新。

---

## 6. 変更ファイル（想定）

| ファイル | 変更内容 |
| --- | --- |
| `crates/bevy_app/src/world/map/spawn.rs` | Sprite → 3D mesh + material、Transform 写像 |
| `crates/bevy_app/src/world/map/terrain_border.rs` | 削除または未使用化 |
| `crates/bevy_app/src/plugins/startup/mod.rs` / `startup_systems.rs` | `spawn_terrain_borders` 呼び出し削除 |
| `crates/bevy_app/src/plugins/startup/visual_handles.rs`（該当すれば） | 地形用 `SectionMaterial` ハンドル共有 |
| `crates/bevy_app/src/systems/visual/section_cut.rs` | 地形エンティティを section 同期対象に追加 |
| `crates/hw_world/src/terrain_visual.rs` | `Sprite` 依存の除去、3D 用更新 |
| `crates/hw_world/src/terrain.rs` | `z_layer` コメント／3D 方針の整理（破壊的変更に注意） |
| `docs/architecture.md` / `docs/world_layout.md` | 仕様追従 |

**注**: 追加システムが分かれる場合は `section_cut` 近傍にモジュール分割してよい。

---

## 7. 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace
```

### 手動チェックリスト

- [ ] TopDown：地形・建物・Soul の前後関係が従来と大きく変わらない（許容範囲内）
- [ ] 矢視：`SectionCut` on/off で地形も含め破綻しない
- [ ] ウィンドウリサイズ・F4 品質：`RttRuntime` 連鎖で地形テクスチャが追従（既存同期のまま）
- [ ] 岩（障害物）撤去後：該当タイルが土表示に戻る
- [ ] Familiar / Soul の採取 Designation がタイル Entity に付く既存フローが動く

---

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| タイル数 × メッシュでドローコール増 | GPU 負荷 | 初回は共有 `Mesh`、将来チャンク化 |
| `SectionMaterial` 化で全タイルが section システムの対象になりコスト増 | CPU | クエリを `With<Tile>` に限定、バッチ更新を検討 |
| MS-3-7 前はクリックが 2D カメラ基準のまま | 地形クリックのズレ | 既知制限として文書化。優先度の高い UI は別途調整 |
| `TerrainType::z_layer` 廃止で微妙な 2D オーバーレイ順が失われる | 見た目変化 | MS-3-6 まで単色境界を許容。必要なら微小 Y オフセット |

---

## 9. 完了の定義（この計画書）

- ロードマップ **MS-3-4** の完了条件（`milestone-roadmap.md`）をすべて満たす。
- 本計画の **§1 成功指標**および **§7 手動チェックリスト**を満たす。
- ステータスを `Completed` に更新し、親ロードマップの MS-3-4 チェックを更新する。

---

## 10. 更新履歴

| 日付 | 内容 |
| --- | --- |
| 2026-03-29 | 初版（Draft） |
