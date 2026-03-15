# 3D-RtT 移行ロードマップ

作成日: 2026-03-15
最終更新: 2026-03-15
ステータス: 策定中

---

## ビジョン

**最終ゴール**: 地形・建築物・キャラクターをすべて3D空間に配置し、Camera3dの正射影レンダリング結果をRtT（Render-to-Texture）で2D UIと合成する「フルRtT」アーキテクチャへの移行。

**基本方針**: ロジック層（ECS・AI・パスファインディング）は一切変更しない。描画層のみを段階的にすげ替える。

### フェーズ別レンダースコープ

| フェーズ | 2Dに残すもの | 3D/RtTへ移すもの | 備考 |
|---------|-------------|------------------|------|
| Phase 1 | 地形、建築物、キャラクター、UI | テスト用3Dオブジェクトのみ | RtT配線の成立確認 |
| Phase 2 | 地形、2D UI | 壁・建築物の一部、キャラクタープロキシ | ハイブリッドRtTでZバッファ検証 |
| Phase 3 | UIのみ | 地形、建築物、キャラクター | ここを「フルRtT到達点」とする |
| Phase 4 | UIのみ | Phase 3の3Dシーン + 多層階表示 | ロジック座標系の拡張が入る |

---

## トラック構成

```
並行トラック A ──────────────────────────────────────────── 前提フェーズ
並行トラック B ──────────────────────────────────────────── WFC地形生成（独立）

メインルート:
  Phase 0 (前提)
    └─ Phase 1: RtTインフラ
         └─ Phase 2: ハイブリッドRtT（建築物+キャラクター先行3D化）
              └─ Phase 3: フルRtT（地形を含むインゲーム要素の3D化）
                   └─ Phase 4: 多層階（将来構想）
```

---

## 前提フェーズ（独立実施可・今すぐ）

### MS-Pre-A: obstacles差分更新

> **依存**: なし
> **元計画**: `docs/proposals/3d-rtt/related/spatial-grid-architecture-plan-2026-03-12.md` フェーズA

- **現況**: 現行コードでは `world_map.register_completed_building_footprint(...)` と `clear_building_occupancy(...)` 系を通じてセル単位の差分更新になっている
- **やること**: 追加実装ではなく、差分更新に戻っていないことを確認する
- **完了条件**: `building_completion/world_update.rs` と `hw_world::WorldMap` に全走査ベースの obstacle 再構築が再導入されていない
- **ステータス**: [x] 現行コードで成立済み（2026-03-15確認）

---

### MS-Pre-B: Building親子構造化 + Zスロット定義

> **依存**: なし
> **元計画**: `docs/proposals/3d-rtt/related/building-visual-layer-plan-2026-03-12.md`

- **やること**:
  1. `crates/hw_core/src/constants/render.rs` に Zスロット定数を追加（`Z_BUILDING_FLOOR=0.05` 〜 `Z_BUILDING_LIGHT=0.18`）
  2. `building_completion/spawn.rs` を Building親エンティティ（ロジック）+ VisualLayer子エンティティ（Sprite）に分割
  3. `VisualLayerKind` コンポーネントを `crates/hw_visual/src/` に追加
  4. 建築物ルートの `Sprite` を直接更新している既存システム（例: `hw_visual::wall_connection`, `hw_visual::tank`）を VisualLayer 子エンティティ参照へ追従させる
- **完了条件**: 既存の見た目が変わらない、建築物ルートから `Sprite` を外しても既存 visual system が破綻しない、`cargo check` 通過
- **3D化への価値**: Phase 2で `VisualLayer` 子エンティティに `RenderLayers::layer(1)` を追加するだけで3D側へ移行できる
- **ステータス**: [x] 完了（2026-03-15）

---

## Phase 1: RtTインフラ

> **依存**: MS-Pre-A/B は不要（独立して開始可能）

### MS-1A: Bevy `3d` フィーチャー有効化

- **やること**: `Cargo.toml` に `"3d"` フィーチャーを追加し `cargo check` を通す
- **精査済み事項**:
  - Bevy 0.18 において `"3d"` フィーチャーは `bevy_pbr`, `bevy_core_pipeline`, `bevy_render` を包含しており、RtT に必要な 3D パイプラインがすべて有効化される
- **完了条件**: コンパイルエラーゼロ
- **ステータス**: [ ] 未着手

---

### MS-1B: Camera3d + RenderTarget セットアップ

> **依存**: MS-1A

- **やること**:
  - `plugins/startup/mod.rs` に Camera3d（正射影）+ `RenderTarget::Image` を追加
  - `crates/hw_core/src/constants/render.rs` に `LAYER_2D = 0`, `LAYER_3D = 1` 定数追加
  - オフスクリーンテクスチャ（`Handle<Image>`）を `assets.rs` で管理

- **Bevy 0.18 API 実装ガイド**:
  - `RenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 })` を使用する
  - `ImageRenderTarget` は `bevy::render::camera` に定義されている
  - `Handle<Image>` 生成時、`usage` に `TextureUsages::RENDER_ATTACHMENT` を含める必要がある

- **完了条件**: オフスクリーンテクスチャへのレンダリングが確認できる（内容は問わない）
- **ステータス**: [ ] 未着手

---

### MS-1C: Camera2d ↔ Camera3d 同期システム

> **依存**: MS-1B

- **やること**: 毎フレーム Camera2d の Transform/OrthographicProjection を Camera3d に同期するシステムを追加
  - パン: `Camera2d.Transform.translation.xy` → Camera3d の XZ 軸にマッピング
  - ズーム: `PanCamera` が更新する `transform.scale` を Camera3d に反映する
- **Bevy 0.18 API 実装ガイド**:
  - `PanCamera` (0.18) は内部の `zoom_factor` を元に `transform.scale` を直接更新する
  - Camera3d 同期時は `transform.translation` (XZ面) と `transform.scale` (一様スケーリング) を同期させれば、正射影としての見た目が一致する
  - `OrthographicProjection.scale` を操作する場合は、両カメラで同じ値を共有するように注意する

- **完了条件**: パン・ズーム操作時に既存の2Dビューが壊れない
- **ステータス**: [ ] 未着手

---

### MS-1D: RtTテクスチャのCamera2d合成

> **依存**: MS-1C

- **やること**:
  - MS-1Bで生成したテクスチャをフルスクリーン Sprite として Camera2d の適切なZ位置に配置
  - Camera3d の `clear_color` を `ClearColorConfig::Custom(Color::NONE)` に設定する
  - テスト用3Dオブジェクト（Bevy組込みの立方体等）を Layer 1 に配置して合成確認

- **完了条件（継続可否ゲート）**:
  - テスト立方体がトップダウンビューで正しい位置に表示される
  - 建築物のない部分はテレインが透過して見える
  - ⚠️ **フルRtT継続可否判断**: MS-1D 完了時点でパフォーマンス・合成品質・実装コストを評価し、Phase 2 以降の継続可否を明示的に判断する。問題が解消見込みのない場合はここで中止する
- **ステータス**: [ ] 未着手

---

## Phase 2: ハイブリッドRtT（建築物+キャラクター先行3D化）

> **依存**: MS-1D完了 + MS-Pre-B完了（親子構造が前提）

### MS-2A: 壁セグメントの3D配置

- **やること**:
  - 壁の VisualLayer 子エンティティを Sprite → Bevy組込みシェイプ（`Cuboid` メッシュ等）に置き換え
  - `RenderLayers::layer(1)` を付与してCamera3d側で描画
  - 16+バリアントのスプライト切替ロジックを廃止し、3Dモデルの動的配置で代替

- **完了条件**: トップダウンで壁の見た目が正しい、`cargo check` 通過、**旧スプライト切替ロジック（16+バリアント）が削除されている**
- **ステータス**: [ ] 未着手

---

### MS-2B: Zソート問題の検証

- **依存**: MS-2A
- **やること**:
  - Soul / Familiar を最小3Dプロキシで RtT レイヤーへ移す
  - 少なくとも「壁の背後を歩くキャラクター」「壁際で作業するキャラクター」の2ケースを再現する
- **完了条件**: キャラクターを含む重なりがハードウェアZバッファで自然に解決される。2D側の `Z_CHARACTER` 調整で帳尻を合わせる必要がない
- **ステータス**: [ ] 未着手

---

### MS-2C: ハイブリッド段階の前後関係検証

- **依存**: MS-2B
- **やること**: 壁・アイテム・キャラクターが重なる状況を再現し、Phase 2 の対象範囲で Zソート破綻がないか検証する
- **完了条件**: Phase 2 の対象物では Z値管理コードを追加せずに前後関係が正しく描画される
- **ステータス**: [ ] 未着手

---

### MS-2D: 床・ドア・家具の3D化

- **依存**: MS-2C
- **やること**: BuildingType ごとに順次 VisualLayer を3D化
  - `Floor` → 平面メッシュ（`Plane3d`）
  - `Door` → Cuboid + 開閉アニメーション準備
  - `Tank` / `MudMixer` → Cuboid 仮モデル
- **完了条件**: Phase 2 終了時点で「地形以外の主要インゲーム要素をRtTへ移す前提」が成立している
- **ステータス**: [ ] 未着手

---

## Phase 3: フルRtT（地形を含むインゲーム要素の3D化）

> **依存**: Phase 2完了

### MS-3A: テレインの3D化

- **やること**:
  - 既存の地形タイル描画を 3D メッシュ / 3D マテリアルベースへ置き換える
  - `terrain_border.rs` / `borders.rs` に依存しない地形表現へ移行する
  - Phase 3 完了時点で、インゲーム要素の描画は Camera3d → RtT のみで成立させる
- **補足**: 見た目改善としてのブレンド表現は 3D マテリアル側で行う。`Material2d` への置換だけではフルRtT到達とはみなさない
- **完了条件**: 地形が 2D カメラに依存せず 3D シーン上に存在し、Camera2d 側には UI だけが残る
- **ステータス**: [ ] 未着手

---

### MS-3B: テレイン表面表現の改善

- **依存**: MS-3A
- **やること**: 3D 化した地形の表面表現を改善する
  - テクスチャブレンド
  - ノイズによる遷移境界の有機化
  - 必要なら生成時ベイクの検証
- **完了条件**: 90度ベースの地形境界オーバーレイに依存せず、3D 地形の見た目が成立する
- **ステータス**: [ ] 未着手

---

### MS-3C: マウスヒットテストのRaycasting化

- **依存**: MS-3A
- **やること**:
  - 現在の2D逆行列変換（`viewport_to_world_2d`）を Camera3d からの Raycasting に全面置換する
  - `crates/hw_ui`, `crates/bevy_app`, `crates/hw_visual` に散在する `viewport_to_world_2d` 利用箇所を、共有ヘルパー経由の Raycast 判定へ寄せる
  - クリック、ホバー、範囲選択、配置プレビューの各入力モードを個別に検証する
- **完了条件**: クリック・ホバー・ドラッグ操作の判定が 3D ビューで正しく動作し、インゲーム入力で `viewport_to_world_2d` への依存が残らない
- **ステータス**: [ ] 未着手

---

### MS-3D: 2Dスプライトインフラの段階的廃止

- **依存**: MS-3C
- **やること**: Phase 2〜3で3D化済みのインゲーム要素から 2D Sprite コンポーネントと関連Z定数を順次削除し、Camera2d を UI 専用へ絞る
- **完了条件**: Camera2d 側に残るのは UI と純2Dオーバーレイだけで、インゲーム要素の描画責務が 3D/RtT に一本化される
- **ステータス**: [ ] 未着手

---

## 矢視モード（Phase 2〜3の間に実施可能）

### MS-Elev: 矢視（立面図）4方向切替

> **依存**: MS-1D（インフラ確認）、MS-2A（意味ある3Dモデルでの検証）

- **やること**:
  - `ElevationViewState` リソースを追加（`TopDown` / `North` / `South` / `East` / `West`）
  - Camera3d の Transform を4プリセットに切替
  - 矢視中はテレイン（Layer 0の2Dスプライト）を非表示
  - 矢視UI（方向切替ボタン/キーバインド）追加

- **完了条件（2段階）**:
  - **MS-1D後**: Camera3d の Transform 切替インフラが動作する（テスト立方体で確認）
  - **MS-2A後**: 実際の壁3Dモデルで4方向が正しく表示される（こちらを最終完了条件とする）
- **ステータス**: [ ] 未着手

---

## 並行トラックB: WFC地形生成

> **依存**: なし（`hw_world/src/mapgen.rs` のみ影響）
> **元計画**: `docs/plans/3d-rtt/related/wfc-terrain-generation-plan-2026-03-12.md`

| MS | 内容 | ステータス |
|----|------|-----------|
| MS-WFC-1 | `TerrainType::can_be_adjacent()` / `can_be_diagonal()` 実装 | [ ] 未着手 |
| MS-WFC-2 | `bevy_procedural_tilemaps` 導入 + 基本WFC生成（**要: Bevy 0.18 対応確認、非対応時は自前実装へ切替**） | [ ] 未着手 |
| MS-WFC-3 | 川・砂バッファゾーンの固定制約統合 | [ ] 未着手 |
| MS-WFC-3.5 | コーナー制約検証システム（`#[cfg(debug_assertions)]`） | [ ] 未着手 |

---

## Phase 4: 多層階（将来構想）

> **依存**: Phase 3完了

- `TileIndex(x, y, z)` への座標型拡張（ロジック層）
- パスファインディングの多層階ネットワーク（階段ポータル）対応
- Floor ID ごとに3Dメッシュを表示/非表示切替

*詳細設計は Phase 3完了後に策定する。*

---

## 依存グラフ

```
MS-Pre-A ────────────────────────────────────────────── (成立済み)
MS-Pre-B ─────────────────────────────────┐
                                          │
MS-1A → MS-1B → MS-1C → MS-1D ──────────┤──→ MS-Elev
                                          │
                                    MS-2A─┘→ MS-2B → MS-2C → MS-2D
                                                                  │
                                              MS-3A → MS-3B ──────┤
                                                   └→ MS-3C → MS-3D┘
                                                                  │
                                                       Phase 4 ───┘

MS-WFC-1 → MS-WFC-2 → MS-WFC-3 → MS-WFC-3.5  (独立)
```

---

## 優先度ガイド

| 優先 | 理由 |
|------|------|
| MS-Pre-A | 現行コードで成立済み。以後は退行監視のみ |
| MS-Pre-B | Phase 2の前提。建築物spawner の2D/3D分離ポイント |
| MS-1A〜1D | RtT全体の基盤。ここが通らないと何も進まない |
| MS-2B〜2C | Character を含めた Zバッファ検証。ここを通さないとハイブリッドRtTの価値が未検証のままになる |
| MS-3A〜3D | フルRtT定義の本体。地形3D化と入力刷新をここで揃える必要がある |
| MS-WFC-1〜3 | メインルートとは独立。地形改善を先行させることも可 |

---

## 関連ドキュメント

| ドキュメント | 内容 |
|------------|------|
| `docs/proposals/3d-rtt/3d-rendering-rtt-proposal-2026-03-14.md` | ハイブリッドRtT提案（Phase 1〜4詳細） |
| `docs/proposals/3d-rtt/3d-rendering-rtt-proposal-phase2-2026-03-14.md` | フルRtT・多層階アーキテクチャ方針 |
| `docs/proposals/3d-rtt/related/building-visual-layer-plan-2026-03-12.md` | MS-Pre-B詳細設計 |
| `docs/proposals/3d-rtt/related/spatial-grid-architecture-plan-2026-03-12.md` | MS-Pre-A詳細設計 |
| `docs/proposals/3d-rtt/related/wfc-terrain-generation-plan-2026-03-12.md` | WFCトラック詳細設計 |
