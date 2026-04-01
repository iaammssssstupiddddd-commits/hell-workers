# MS-3-5 Building3dHandles の SectionMaterial 移行（MS-Section-B）実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `ms-3-5-building-section-material-plan-2026-03-31` |
| ステータス | `Draft` |
| 作成日 | `2026-03-31` |
| 親マイルストーン | `docs/plans/3d-rtt/milestone-roadmap.md` **MS-3-5** |
| 前提 MS | **MS-3-3**（`SectionMaterial` 基盤・`sync_section_cut_to_materials_system`）、**MS-3-4**（地形が同一 RtT・`SectionMaterial` パイプライン上） |
| 関連提案 | [`section-material-proposal-2026-03-16.md`](../../proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md) |
| アセット計画 | [`asset-milestones-2026-03-17.md`](asset-milestones-2026-03-17.md)（MS-Asset-Build-A/B：本 MS は **プレースホルダ Cuboid でもコード完了可能**。GLB 本番は別ゲート） |
| 後続 MS | **MS-3-9**（切断線 UI・`MS-3-5` 依存）、見た目の本番品質は **MS-Asset-Build-B** |

---

## 1. 目的

### 解決したい課題

- 建物 3D ビジュアルのうち **床・扉・1x1/2x2 設備**がまだ `StandardMaterial` のため、**矢視モードの `SectionCut`（スラブ切断）**の対象外となり、壁だけがクリップされる状態が続いている。
- `Building3dHandles` 内のマテリアル型が **Wall＝`SectionMaterial`**、それ以外＝**`StandardMaterial`** と混在しており、保守とクエリ（例: `building3d_cleanup`）の前提が分岐している。

### 到達したい状態

- **インゲーム建物 3D メッシュ**（`Building3dVisual` 経由で spawn されるもの）が、少なくとも **論理上すべて `MeshMaterial3d<SectionMaterial>`** となる。
- `hw_visual::material::sync_section_cut_to_materials_system` により、**切断面が全 BuildingType で一貫して適用**される（プレースホルダメッシュでも検証可能）。
- **トップダウン**では現状と同等の見え方を維持（色・テクスチャの意図した退化がないこと）。

### 成功指標（ロードマップとの整合）

- [ ] `cargo check --workspace` ゼロエラー、`cargo clippy --workspace` 警告ゼロ（リポジトリ方針）
- [ ] 矢視モードで切断線を動かしたとき、**対象となる全 BuildingType** の 3D メッシュでスラブ外がクリップされる（目視）
- [ ] トップダウンで全建物タイプが破綻なく表示される（目視）

---

## 2. スコープ

### 対象（In Scope）

| 領域 | 内容 |
| --- | --- |
| **リソース** | `crates/bevy_app/src/plugins/startup/visual_handles.rs` の `Building3dHandles` フィールド `floor_material` / `door_material` / `equipment_material` を **`Handle<SectionMaterial>`** に変更し、`init_visual_handles` で `make_section_material` / `make_section_material_textured`（`GameAssets` の画像）で生成する。 |
| **スポーン** | `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs` の `spawn_building_3d_visual`：既に `MeshMaterial3d(handles_3d.*)` なので **ハンドル型が `SectionMaterial` に変われば追従**。 |
| **床建設完了** | `crates/bevy_app/src/systems/jobs/floor_construction/completion.rs`：`floor_material` 参照の型整合。 |
| **Soul Spa 配置など** | `crates/bevy_app/src/interface/selection/soul_spa_place/spawn.rs`：`equipment_material` 参照の型整合。 |
| **仮設→本設** | `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`：`sync_provisional_wall_material_system` は既に `MeshMaterial3d<SectionMaterial>` 前提。床・扉・設備に **動的マテリアル差し替え**が将来増えた場合は同パターンで拡張可能なことを確認。 |

### 非対象（Out of Scope）

| 項目 | 理由 |
| --- | --- |
| **Familiar 3D の `familiar_material`（`StandardMaterial`）** | BuildingType ではない。キャラ系は `CharacterMaterial` / 既存キャラ MS の範囲。本 MS では **`Building3dHandles` に残してもよい**が、型が混在するので **コメントで「建物以外」**と明記するか、後続で `CharacterHandles` 側へ移す検討は別タスク。 |
| **壁向き補助（`wall_orientation_aid_material`）** | 現状 `StandardMaterial`（発光・unlit）。断面クリップの対象にするかは **任意**。対象にするなら `SectionMaterial` 化、不要なら **補助専用として Standard のまま**でもよい（`Building3dVisual` のクリップ要件から外す旨をコメント）。 |
| **GLB 差し替え・MS-Asset-Build 完了** | プレースホルダ **Cuboid/Plane** のままでも MS-3-5 のコード完了条件は満たせる。見た目の受入は **MS-Asset-Build-B** が追跡。 |
| **`hw_visual/src/visual_handles.rs`** | リポジトリ現状では **`Building3dHandles` は `bevy_app` の `plugins/startup/visual_handles.rs`** にある。旧計画表記との差異に注意。 |

---

## 3. 現状とギャップ（2026-03-31 時点のコード）

### `Building3dHandles`（`bevy_app` / `visual_handles.rs`）

| フィールド | 型 | 用途 |
| --- | --- | --- |
| `wall_material` / `wall_provisional_material` | `SectionMaterial` | 壁・仮設壁（**移行済み**） |
| `wall_orientation_aid_material` | `StandardMaterial` | 壁向き補助（上記 Out of Scope または任意対応） |
| `floor_material` | `StandardMaterial` | 床 |
| `door_material` | `StandardMaterial` | 扉 |
| `equipment_material` | `StandardMaterial` | Tank / MudMixer / RestArea / SoulSpa / 1x1 設備等 |
| `familiar_material` | `StandardMaterial` | Familiar 板ポリ（非 BuildingType） |

### マテリアル生成

- 壁は `make_section_material` / `with_alpha_mode` を使用済み。
- 床・扉・設備は `StandardMaterial { base_color: Color::srgb(...), ... }` の **単色**。
- 地形（`Terrain3dHandles`）は `make_section_material_textured` 済み。**建物もテクスチャを載せる場合**は、対応する `GameAssets` の `Image` ハンドルを `make_section_material_textured` に渡す（`docs/world_lore`・アート方針に合わせる）。

### ギャップ

- `sync_section_cut_to_materials_system` は `Assets<SectionMaterial>` を更新する。**`StandardMaterial` のメッシュは切断に反応しない**。
- 床・扉・設備を `SectionMaterial` に揃えると、`InitVisualHandlesParams` の `materials: ResMut<Assets<StandardMaterial>>` 依存は **建物用が減り**、`familiar_material` と `wall_orientation_aid_material` などに限定される（リファクタ時に整理）。

---

## 4. 実装方針

### 4.1 マテリアル定義

1. **床**: `GameAssets::mud_floor` 等、2D スプライトと同系のテクスチャを `make_section_material_textured` に渡すか、当面は **単色 `make_section_material(LinearRgba::...)`** で壁 pilot と同様に開始し、目視で問題なければテクスチャ化する。
2. **扉**: `door_closed` / `door_open` の画像を載せるかはアセット次第。最低限 **単色 SectionMaterial** でクリップ検証可能。
3. **設備**: 現状は単色 1 本の `equipment_material` を全 2x2/1x1 で共有。**タイプ別ハンドル**（Tank / MudMixer …）は本 MS では必須としない。見た目差分は後続（アセット・`BuildingAnimHandles` 連動）で拡張可能。

### 4.2 `AlphaMode`・透過

- 仮設壁と同様、必要なら `with_alpha_mode(..., AlphaMode::Blend)` を使用。`SectionMaterial` の拡張フィールドは `hw_visual::material::section_material` の既存 API に従う。

### 4.3 システム・クエリの整合

- `MeshMaterial3d<SectionMaterial>` に統一した後、**`MeshMaterial3d<StandardMaterial>` を前提にした建物系クエリ**が残っていないか `rg` で確認する。
- `character_proxy_3d.rs` の `StandardMaterial` は **キャラ用**のため除外してよい。

### 4.4 設備別 `tank.rs` / `mud_mixer.rs`

- ロードマップ記載の **旧パス**。現リポジトリでは該当ファイルは **`systems/visual/` 直下に存在しない**（統合・削除済みの可能性）。実装時は **`MeshMaterial3d` / `Building3dHandles` を参照する全 `systems/visual`・`systems/jobs`** を `rg` で再確認する。

---

## 5. 実装ステップ（推奨順）

1. **M1**: `visual_handles.rs` — `floor_material` / `door_material` / `equipment_material` を `Handle<SectionMaterial>` に変更し、`init_visual_handles` で生成。`Building3dHandles` 構造体と `InitVisualHandlesParams` の整合。
2. **M2**: `cargo check` で `building_completion/spawn.rs`、`floor_construction/completion.rs`、`soul_spa_place/spawn.rs` などのコンパイルエラーを潰す。
3. **M3**: ゲーム内で **各 BuildingType** を 1 つずつ配置し、**矢視 + SectionCut** とトップダウンを目視。退行があれば色・テクスチャ・`alpha_mode` を調整。
4. **M4**: `wall_orientation_aid` の方針決定（Standard のまま / Section 化）。ドキュメント化。
5. **M5**: `docs/architecture.md` の RtT・建物関連、`docs/events.md` への変更が不要ならスキップ。変更があれば **hell-workers-update-docs** 手順で更新。

---

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| `SectionCut` 変更時の `Assets<SectionMaterial>` 全走査コスト | 建物マテリアル数は地形より少ない想定。問題が出たら計測し、`section_material.rs` の最適化は **別タスク**（ロードマップ上は既知のパターン）。 |
| テクスチャ付き `SectionMaterial` の見た目が 2D スプライトと乖離 | 本 MS は **クリップ整合が主目的**。見た目の本番は MS-Asset-Build / テクスチャ整備と明記。 |
| Familiar が `Building3dHandles` に残り型が混在 | コメントまたは小さなリネーム（例: `placeholder_familiar_material`）で誤用を防ぐ。 |

---

## 7. 完了条件チェックリスト

- [ ] `Building3dHandles` の建物用 `floor` / `door` / `equipment` がすべて `SectionMaterial`
- [ ] 上記を参照する spawn / completion / cleanup がコンパイルし、実行時にパニックしない
- [ ] 矢視：全 BuildingType でスラブクリップが目視確認できる
- [ ] トップダウン：退行なし
- [ ] `cargo clippy --workspace` 警告ゼロ

---

## 8. 参照

| 文書 | 内容 |
| --- | --- |
| `milestone-roadmap.md` | MS-3-5 依存・後続 MS-3-9 |
| `asset-milestones-2026-03-17.md` | MS-Asset-Build-A/B とコード MS の対応 |
| `docs/architecture.md` | Camera3d・RtT・`SectionCut` |
| `hw_visual/src/material/section_material.rs` | `make_section_material` / `sync_section_cut_to_materials_system` |

---

## 9. 更新履歴

| 日付 | 内容 |
| --- | --- |
| 2026-03-31 | 初版（コードベースパス・現状表を実装に合わせて記載） |
