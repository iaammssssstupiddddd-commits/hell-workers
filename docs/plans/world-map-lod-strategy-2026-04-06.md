# ワールドマップ LOD 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `world-map-lod-strategy-2026-04-06` |
| ステータス | `In Progress` |
| 作成日 | `2026-04-06` |
| 最終更新日 | `2026-04-09` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |
| 関連ドキュメント | `docs/world_layout.md`, `docs/architecture.md`, `docs/plans/world-map-render-chunking-plan-2026-04-08.md`, `docs/plans/3d-rtt/ms-3-6-terrain-surface-plan-2026-03-31.md` |

## 1. 目的

- 解決したい課題:
  - 現在の地形描画は `TerrainChunk` 49 entity まで整理済みで、主要コストは entity 数ではなく terrain shader の fragment / sampling 負荷に寄っている。
  - 旧案は `LOD0=現行`, `LOD1=軽量`, `LOD2=overview` だったが、現方針では overview を採用しない。
  - 代わりに `Lod0` を将来のリッチビジュアル用予約スロットにし、現在の runtime は `Lod1` と `Lod2` の 2 段で運用する。
- 到達したい状態:
  - `Lod1` は現行フル品質の terrain shader を維持する。
  - `Lod2` は同じ chunk renderer のまま軽量 shader に切り替える。
  - LOD 判定は `tile_rtt_px` 正本、`tile_screen_px` 補助の構成で一元管理する。
- 成功指標:
  - `TerrainLodMetrics` / `TerrainLodState` によって LOD 判定と適用状態が分離されている。
  - `Terrain3dHandles { lod1, lod2 }` と `MeshMaterial3d` component 差し替えで 49 chunk を切り替えられる。
  - hysteresis が `Lod1 -> Lod2: 14px 未満`, `Lod2 -> Lod1: 16px 超` としてコードに固定されている。
  - `Lod0` は runtime では選ばれず、将来の高品質 variant 導入余地として予約されている。

## 2. スコープ

### 対象（In Scope）

- `tile_rtt_px` / `tile_screen_px` の観測基盤
- `TerrainLodMetrics` / `TerrainLodState` / `resolve_lod_level`
- chunk renderer 維持のまま行う shader LOD
- `TerrainSurfaceMaterial` と軽量 variant の 2 系統管理
- `docs/world_layout.md` / `docs/architecture.md` / 本計画書の整合

### 非対象（Out of Scope）

- overview / impostor / map 全体 1 枚化
- `TerrainChunk` 可視切り替えによる far 専用描画経路
- `WorldMap` / pathfinding / AI / logistics の責務変更
- Soul / Familiar / 建物 / 影 / RtT 合成全体の LOD

## 3. 現状

- `spawn_map` は 10,000 個の `Tile` 論理 anchor を生成するが、描画コンポーネントは持たない。
- 地形描画は `spawn_terrain_chunks` が担当し、`CHUNK_TILES = 16`、7x7 = 49 個の `TerrainChunk` entity を生成する。
- `TerrainSurfaceMaterial` は現行フル品質 shader（`terrain_surface_material.wgsl`）を使う。
- 軽量 variant は `TerrainSurfaceMaterialLod2` として分離され、`terrain_surface_material_lod2.wgsl` を使う。
- `Terrain3dHandles` は `lod1: Handle<TerrainSurfaceMaterial>` と `lod2: Handle<TerrainSurfaceMaterialLod2>` を保持する。
- `TerrainChangedEvent` は `TerrainIdMap` の部分更新だけを担い、chunk の再生成は不要である。

## 4. LOD 定義

| LOD | 現在の扱い | 内容 | 主目的 |
| --- | --- | --- | --- |
| `Lod0` | 予約 | 将来のリッチビジュアル用。現 runtime では未使用。 | 将来拡張 |
| `Lod1` | 使用中 | 現行フル品質 shader (`terrain_surface_material.wgsl`) | 品質維持 |
| `Lod2` | 使用中 | 軽量 shader (`terrain_surface_material_lod2.wgsl`) | fragment / sampling 負荷削減 |

補足:

- 旧計画にあった overview / impostor ベースの「遠景専用 LOD2」は廃止した。
- 現在の `Lod2` は旧 `LOD1` を繰り上げた lightweight material である。
- `Lod0` は番号だけ先に確保し、runtime 判定では `Lod1` にフォールバックする。
- `Lod2` は曲線境界を完全に捨てず、`boundary_mask` の nearest region を描画上の正本にして coarse terrain の輪郭だけ維持する。
- 一方で面ディテールは `albedo` UV 量子化と簡略 shading に落とし、低解像度 texture 相当の見た目へ寄せる。

## 5. 観測と遷移契約

### 5.1 観測値

- `tile_rtt_px`
  - `Camera3dRtt.world_to_viewport(...)` で算出する RtT 上の 1 タイル見かけサイズ
  - LOD 判定の正本
- `tile_screen_px`
  - `tile_rtt_px * composite 表示倍率`
  - デバッグ補助用

### 5.2 算出方法

- 基準点 `P = Vec3::ZERO`
- 比較点:
  - `P + Vec3(TILE_SIZE, 0, 0)`
  - `P + Vec3(0, 0, TILE_SIZE)`
- 2 軸の投影差分長 `dx`, `dz` の大きい方を `tile_rtt_px` とする
- `East/West` 矢視で world X 辺が視線方向へ潰れても、2 軸の大きい方を採ることで誤判定を避ける

### 5.3 hysteresis

```rust
pub const LOD1_TO_LOD2_ENTER_PX: f32 = 14.0;
pub const LOD2_TO_LOD1_EXIT_PX: f32 = 16.0;
```

- `Lod1` 中に `tile_rtt_px < 14.0` で `Lod2`
- `Lod2` 中に `tile_rtt_px > 16.0` で `Lod1`
- `Lod0` は予約スロットのため、現在の `resolve_lod_level` では `Lod1` に寄せる

## 6. 実装方針

### 6.1 material variant

- 動的 uniform 分岐ではなく、別 shader / 別 material 型で分ける
- 現行:
  - `TerrainSurfaceMaterial`
  - `TerrainSurfaceMaterialLod2`
- 両者とも `ExtendedMaterial<StandardMaterial, ...>` のまま section clip / lighting / prepass を維持する

### 6.2 handle 管理

```rust
pub struct Terrain3dHandles {
    pub lod1: Handle<TerrainSurfaceMaterial>,
    pub lod2: Handle<TerrainSurfaceMaterialLod2>,
}
```

- startup で両 handle を初期化する
- `spawn_terrain_chunks` の初期 material は `lod1`
- `spawn_boundary_meshes` は `boundary_mask` を `lod1` / `lod2` の両方へ配る

### 6.3 chunk 切替

- `terrain_lod_switch_system` が `level != applied_level` の時だけ差し替える
- `Lod1 -> Lod2`
  - `MeshMaterial3d<TerrainSurfaceMaterial>` を remove
  - `MeshMaterial3d<TerrainSurfaceMaterialLod2>` を insert
- `Lod2 -> Lod1`
  - 上記の逆
- `Lod0`
  - 未実装のため、適用時は `Lod1` material へフォールバックする

## 7. マイルストーン

## M1: 観測基盤と LOD 契約の固定

- 状態: `完了`
- 実施内容:
  - `TerrainLodMetrics` / `TerrainLodState` を導入
  - `tile_rtt_px` / `tile_screen_px` の算出を実装
  - `resolve_lod_level` と hysteresis 定数を導入
  - `composite_logical_size(window)` を共有 helper 化
- 完了条件:
  - [x] 観測値と適用状態が分離されている
  - [x] `tile_rtt_px` が X/Z 2 軸の大きい方で算出される
  - [x] `tile_screen_px` が composite 表示倍率から導出される
  - [x] `cargo check --workspace`
  - [x] `cargo clippy --workspace`

## M2: lightweight shader の runtime 導入

- 状態: `完了`
- 実施内容:
  - `terrain_surface_material_lod2.wgsl` を追加
  - `TerrainSurfaceMaterialLod2` / `TerrainSurfaceMaterialExtLod2` を追加
  - `Terrain3dHandles { lod1, lod2 }` へ移行
  - `terrain_lod_switch_system` で 49 chunk の material component を差し替える
  - `spawn_terrain_chunks` / `spawn_boundary_meshes` を新しい handle 契約へ更新
  - `Lod2` は `boundary_mask` の nearest region を正本にし、曲線境界を残したまま 4-corner bilinear を除去
  - `Lod2` の `albedo` UV を量子化し、macro-noise / domain warp / river scroll / shoreline detail なしの coarse surface 表現へ簡略化
- 完了条件:
  - [x] `Lod1 / Lod2` の切替が動作する
  - [x] `TerrainChangedEvent -> TerrainIdMap` 更新が両 material で反映される
  - [x] `Lod2` でも曲線境界の coarse silhouette が維持される
  - [x] `Lod2` の面テクスチャは低解像度 texture 相当の見た目へ落ちている
  - [x] `cargo check --workspace`
  - [x] `cargo clippy --workspace`

## M3: 将来の `Lod0` 導入準備

- 状態: `未着手`
- 内容:
  - `Lod0` を「現行より高品質なリッチビジュアル」へ割り当てる場合の設計を別途詰める
  - `Lod1` を baseline quality として維持し、`Lod0` だけ近景限定で使う
- 非目標:
  - overview / impostor の復活は本計画では扱わない

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `tile_screen_px` を正本に使って RtT 品質差を見落とす | 高 | LOD 判定は `tile_rtt_px` のみを正本にする |
| `East/West` 矢視で視線方向の潰れに引っ張られる | 高 | world X/Z の 2 軸投影差分の大きい方を採用する |
| metric 更新だけで material 差し替えが毎フレーム走る | 高 | `TerrainLodMetrics` と `TerrainLodState` を分離し、`level != applied_level` の時だけ切り替える |
| 軽量 shader の意味と LOD 番号がずれる | 高 | docs と型名を `Lod1=現行`, `Lod2=軽量`, `Lod0=予約` へ統一する |
| 将来 `Lod0` を追加するときに既存 runtime と衝突する | 中 | 現在の `resolve_lod_level` では `Lod0` を常に `Lod1` に寄せ、予約スロットとして固定する |

## 9. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace`
- 手動確認:
  - TopDown で zoom in / out を往復し、`Lod1 <-> Lod2` が `14 / 16` の hysteresis で切り替わることを確認する
  - `North / East / South / West` 矢視でも同じ hysteresis で安定することを確認する
  - obstacle cleanup 後に near / far を往復して、両 material で見た目が更新されることを確認する
  - dev panel の LOD 表示が runtime レベルと一致することを確認する

## 10. ロールバック方針

- `TerrainLodState` を固定で `Lod1` にする
- `terrain_lod_switch_system` の登録を外す
- `Terrain3dHandles.lod2` と軽量 material plugin を外す

## 11. AI 引継ぎメモ

### 現在地

- M1 完了
- M2 完了
- docs は `world_layout.md` / `architecture.md` / 本計画書を新番号付けへ同期済み
- `Lod2` の説明は「曲線境界維持 + 面ディテール簡略化」で docs と実装を同期済み

### 次にやること

1. `Lod0` を本当に使うなら、近景専用の高品質 variant の要件を先に定義する
2. `Lod1` を baseline quality として維持するか、`Lod0` 側へ一部表現を移すかを決める
3. 旧 overview 案を再利用したくなっても、まず別計画として切り出す

### 注意点

- 旧 `LOD2=overview` 案は廃止済み
- 旧 `TerrainSurfaceMaterialLod1` 系の名前は使わない
- `tile_screen_px` はあくまで補助観測値
- `Lod0` は予約であり、runtime の通常遷移先に含めない

## 12. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-06` | `Codex` | 初版作成 |
| `2026-04-09` | `Codex` | chunk renderer・`boundary_mask` baked 前提へ再整理 |
| `2026-04-09` | `Codex` | `tile_rtt_px` 正本化、矢視 2 軸測定、`clippy` 検証を追記 |
| `2026-04-09` | `Codex` | `Lod1=現行`, `Lod2=軽量`, `Lod0=予約` の番号付けへ全面更新。旧 overview `LOD2` 案を本計画から除外 |
| `2026-04-09` | `Codex` | `Lod2` の実装済み仕様を反映。`boundary_mask` nearest による曲線境界維持と albedo UV 量子化による coarse surface を明記 |
