# 建築システム (Building System)

Hell-Workers における建築システムの基礎実装について説明します。

## 1. 概要

プレイヤーが設計図（Blueprint）を配置し、労働者が資材を運んで建設を完了させるシステムです。

## 2. コンポーネント

| コンポーネント | 役割 |
|:---|:---|
| `Blueprint` | 建設中の建物。`kind`, `progress`, `required_materials`, `delivered_materials` フィールドを持つ |
| `Building` | 完成した建物。`is_provisional` (仮設) フラグを持つ |
| `ProvisionalWall` | 仮設壁のアップグレード状態（`mud_delivered`）を保持 |
| `WallConstructionSite` | 壁の建設サイト（`Framing -> Coating` フェーズ、`material_center`、進捗カウンタを保持） |
| `WallTileBlueprint` | 壁1タイルの建設状態（`wood_delivered` / `mud_delivered` / `spawned_wall`）を保持 |
| `BuildingType` | 建物の種類（`Wall`, `Door`, `Floor`, `Tank`, `MudMixer`, `SandPile`, `BonePile`） |

### 資材要件

| BuildingType | 必要資材 |
|:---|:---|
| Wall | 木材 × 1 + StasisMud × 1（建築開始は木材のみで可能） |
| Door | 木材 × 1 + Bone × 1 |
| Floor | 石材 × 1 |
| Tank | 木材 × 2 |
| MudMixer | 木材 × 4 |
| SandPile | 砂 × 10 |
| BonePile | 骨 × 10 |

## 3. ワークフロー

プレイヤーが Blueprint を配置 → 資材搬入完了 → ソウルが建築作業（約3秒）→ `progress >= 1.0` で完成。全資材が揃っていれば本設、未揃いなら仮設 `Building` として完成し、追加資材搬入後に `CoatWall` で本設化。

## 4. 仮設建築 (Provisional Building)

一部の建物（例: `Wall`）は、必要最低限の資材があれば「仮設状態」として建設を完了できます。
これにより、高コストな資材（例: Stasis Mud）が不足していても、基本的な構造物としての機能（壁による通行区分など）を先行して提供できます。

### 仕組
1.  **最低要件**: `Wall` は木材1つで建設開始・完了可能です。
2.  **仮設フラグ**: `Building` コンポーネントの `is_provisional` が `true` になります。
3.  **資材搬入**: 新仕様の壁サイトでは `TransportRequestKind::DeliverToWallConstruction` がフェーズに応じて `Wood` / `StasisMud` を自動搬入します（`DeliverToProvisionalWall` は legacy 壁のみ）。
4.  **作業タスク**: `WorkType::FrameWallTile`（木材フレーミング）と `WorkType::CoatWall`（タイル塗布）で段階実行します。
5.  **視覚表現**: 仮設状態の壁は警告色オーバーレイで表示され、`CoatWall` 完了で通常見た目へ戻ります。
6.  **本設化完了**: `CoatWall` 完了時に `Building.is_provisional = false` となり、`ProvisionalWall` が削除されます。

`AssignedTask::Build` は以下の `BuildPhase` を持ちます：

1. **GoingToBlueprint**: 設計図の位置へ移動
2. **Building { progress }**: 建築作業中（約3秒で完了）
3. **Done**: 完了

## 5. 制限事項

- **TaskSlots**: 建築作業は1人ずつ（`TaskSlots::new(1)`）。※資材運搬は複数人同時並行可能。

## 6. 自動資材運搬 (Auto-Haul Logic)

`blueprint_auto_haul_system` によって、最も効率的な資材運搬が行われます。

1.  **優先度**: 建築現場への資材運搬は、**他の全てのタスク（資源採取や通常の備蓄運搬）よりも高い優先度（Priority 10）** が設定されています。
2.  **資材選定**:
    - 地上のアイテムだけでなく、**使い魔の担当エリア内にあるストックパイル（備蓄）** からも資材を調達可能です。
    - 検索範囲内の全ての有効な資材の中から、**数学的に最も近い（最短距離にある）もの** を厳密に選択します。
    - これにより、近くにストックパイルがある場合は、遠くの資源を無視して備蓄から効率的に運び出します。
3.  **過剰運搬の防止**: 「配達済み + 運搬中 + 予約済み」の合計が必要数を超えないよう、厳密に管理されます。
4.  **搬入**: Blueprint に到着すると `deliver_material()` で資材が搬入され、進捗が進みます。
5.  **不足資材の自動採取（Wood / Rock）**:
    - 地面に資材が無い場合は、`familiar_ai` の `blueprint_auto_gather_system` が `DeliverToBlueprint` request を需要起点に `Tree` / `Rock` へ `Chop` / `Mine` を自動発行します。
    - 探索は `TaskArea` 内から外側へ段階的（10 / 30 / 60 タイル -> 到達可能全域）に拡大し、近い候補から決定されます。
    - これにより、建築開始時に木や岩の手動指定を都度打たなくても、搬入チェーンを継続できます。

## 7. グリッド配置とエリア選択 (Grid Alignment & Area Selection)

すべての配置操作は、ワールドのタイルグリッドに厳密に整合するように設計されています。

### グリッドスナップ
- **エリア選択**: `Stockpile` や `TaskArea` の指定時、ドラッグ中の矩形は常にグリッドの境界線（タイルの端）にスナップします。中途半端な座標での指定はできません。
- **建築配置**: 建築物の配置位置はグリッドの中心にスナップします。

### 建築ゴースト (Placement Ghost)
建築モード（`PlayMode::BuildingPlace`）中、マウスカーソルに追従する半透明の建物（ゴースト）が表示されます。

- **視覚フィードバック**:
    - **緑色（半透明）**: 配置可能。
    - **赤色（半透明）**: 配置不可（障害物や他の建物と重複、または通行不可地形）。
- **配置失敗理由ツールチップ**:
    - 配置確定時に有効タイルが 0 の場合、`Cannot Place` ツールチップを表示し、配置できない代表理由を示します（約2秒）。
    - 主な理由: `not walkable` / `occupied by a building` / `occupied by a stockpile` / `has no completed floor` / `area too large` / `must be 1xn line`
- **サイズ対応**: 1x1（壁など）だけでなく、2x2（タンクなど）の建物も適切なオフセットで表示されます。

### Companion 配置（Tank / MudMixer）
一部の建物は、親Blueprint配置直後に companion 配置フローへ遷移します。

- **Tank**:
  - `BucketStorage`（1x2）を即時配置するまで companion モードを継続します。
  - 親の `Tank` Blueprint は companion 配置が完了するまで確定しません（未確定状態では建築予約しない）。
  - `Esc` でキャンセルした場合は、親Blueprintと未確定 companion をまとめて取り消します。
- **MudMixer**:
  - 近傍（グリッド3タイル以内）に `SandPile`（完成済み or Blueprint）がない場合、`SandPile` Blueprint 配置の companion モードに遷移します。
  - 親の `MudMixer` Blueprint は companion 配置が完了するまで確定しません。
  - 近傍外ではゴーストが赤表示になり、配置不可が明示されます。

## 8. ビジュアルフィードバック (Visual Feedback)

`visual/blueprint/` モジュールによって、設計図の状態をプレイヤーに視覚的に伝えます。

このモジュールは、汎用的なビジュアルユーティリティ（`systems/utils/`）を使用して実装されています：
- **`utils/progress_bar.rs`**: プログレスバーの生成・更新・位置同期
- **`utils/animations.rs`**: パルス・バウンスアニメーション
- **`utils/floating_text.rs`**: フローティングテキスト（ポップアップ）の表示・アニメーション

### コンポーネント

| コンポーネント | 役割 |
|:---|:---|
| `BlueprintVisual` | 設計図の視覚状態（`BlueprintState`、パルスタイマー、前回の搬入数等）を管理 |
| `ProgressBar` | 設計図下部の進捗バー |
| `MaterialIcon / Counter` | 必要資材のアイコンと「現在の搬入数/必要数」のテキスト表示 |
| `DeliveryPopup` | 資材搬入時に表示される「+1」のフローティングテキスト |
| `CompletionText` | 建築完了時に表示される「Construction Complete!」のテキスト |
| `WorkerHammerIcon` | 建築中のワーカー頭上に表示されるアニメーション付きハンマー |
| `WorkLine` | 建築中のワーカーと設計図を結ぶ視覚的な作業線 |

### 状態別表示

設計図は「青写真」をイメージした青みがかった配色になります。

| 状態 | 透明度 | オーバーレイ色(RGBA) |
|:---|:---|:---|
| `NeedsMaterials` | 25% | (0.8, 0.4, 0.4, 0.4) - 警告赤 |
| `Preparing` | 25~50% | (0.8, 0.8, 0.4, 0.4) - 準備中黄 |
| `ReadyToBuild` | 50% | (0.4, 0.8, 0.6, 0.4) - 待機緑 |
| `Building` | 50~100% + パルス | (0.4, 0.6, 1.0, 0.5) - 建築中青 |

### アニメーション・エフェクト

- **透明度**: `opacity = 0.25 + 0.25 * material_ratio + 0.5 * build_progress`
- **パルス**: 建築作業中、設計図の透明度とスケールが脈動します。
- **バウンス**: 建物が完成した瞬間、実体化した建物が一度ピョンと跳ねる（スケールアップ・ダウン）演出が入ります。
- **フローティングテキスト**:
  - 資材搬入時: 「+1」のテキストがふわっと浮き上がりながらフェードアウトします。
  - 建設完了時: 「Construction Complete!」のテキストが強調表示されます。
- **ワーカー表示**:
  - 建築に従事している間、ワーカーの頭上でハンマーが上下に動きます。
  - ワーカーの位置と建設箇所が半透明の線（作業線）で結ばれます。

### プログレスバー

- 設計図の下部に幅24px、高さ4pxのバーを表示。
- 左詰め（Left-aligned）で増加し、視覚的な直感性を高めています。
- 資材搬入中は橙色（Haul/Prepare）、建築中は緑色（Building）に変化します。

### 壁の自動接続 (Wall Connections)

壁（`BuildingType::Wall`）は、隣接する他の壁や壁の設計図を検知して自動的に形状を変更します。

- **4方向接続**: 上下左右の隣接状況に応じてスプライトを切り替えます。
- **バリエーション**: 直線、コーナー、T字、十字など全15種類（+孤立状態）。
- **設計図連携**: 完成した壁だけでなく、建設中の壁（設計図）とも視覚的に接続します。
- **扉連携**: `BuildingType::Door` も接続対象として扱われるため、`壁-扉-壁` の並びでも隣接壁が接続形状になります（扉自身は専用スプライト）。

### 扉 (Door)

- 扉は 1x1 の建物で、`Open / Closed / Locked` の状態を持ちます。
- 配置条件: 左右が壁/扉、または上下が壁/扉のどちらかを満たす必要があります。
- 閉扉は通行可能ですが追加コスト（開扉待機コスト）が発生し、ロック中は通行不可です。
- 魂は扉タイルへ進入する前に短時間待機し、通過後は一定時間で自動的に閉じます。
- コンテキストメニューからロック/アンロックを切り替えられます。

### Room 検出 (Room Detection)

壁・扉・床で囲まれた空間は、`Room` エンティティとして自動検出されます。

- **成立条件**:
  - 内部タイルがすべて完成 `BuildingType::Floor`
  - 外周が完成 `BuildingType::Wall`（`is_provisional == false`）または `BuildingType::Door`
  - 境界にドアが1つ以上存在
  - タイル数が `ROOM_MAX_TILES`（400）以下
- **検出方式**: 4近傍 Flood-fill
- **再判定トリガー**:
  - `Building` / `Door` の追加・変更
  - `WorldMap.buildings` 差分（削除・置換）
- **自己修復**: 2秒ごとの検証で不正な Room を破棄し、dirty 再検出に戻す
- **可視化**: 成立 Room の床タイルに半透明オーバーレイを表示

### MudMixer と Stasis Mud
- **MudMixer**: 2x2 の生産施設。
    - **建設**: 木材 × 4 で建設。
    - **機能**: 砂(1) + 水(1) + 岩(1) = Stasis Mud(5) を精製。
    - **要件**: 稼働には `Tank` からの水供給（`HaulWaterToMixer`）が必要です。
    - **SandPile**: 建設完了時の自動生成は行いません。必要時は companion フローで `SandPile` Blueprint を配置します。
    - **運搬制約**: `Sand` 搬入は猫車運搬のみ（徒歩運搬なし）。

- **Stasis Mud**: 高度な建築（完全な壁など）に必要な強化建材。
    - **運搬制約**: `StasisMud` の運搬ルールは [logistics.md](logistics.md) に準拠（原則猫車必須、近接ピックドロップ完結時は徒歩許可）。

- **SandPile**:
    - 建物として配置された砂置き場は、砂タイルと同様に**無限の砂ソース**として扱われます。
    - 砂回収は即時で、採取・猫車直採取のどちらでもソース自体は消費されません。

- **WheelbarrowParking（初期配布）**:
    - ゲーム開始時に1棟が初期配置され、猫車不足による初期停滞を防止します。


### 9. FloorConstructionSite 仕様

従来の Blueprint システム（単一タイル配置）とは異なる、**エリア指定型の床建設システム**です。ドラッグ操作で矩形エリアを指定し、複数タイルを一括で建設します。

### 9.1 基本仕様

- **配置方法**: ドラッグ&ドロップで矩形エリアを指定（最大 10×10 タイル）
- **建設フェーズ**: 3段階の建設プロセス
  1. **Reinforcing Phase**: 骨（2個/タイル）を使って補強
  2. **Pouring Phase**: 泥（1個/タイル）を注ぐ
  3. **Curing Phase**: 打設後に一定時間養生（立ち入り禁止）
- **資材コスト**: 骨 × 2 + Stasis Mud × 1 per tile
- **通行性**: 建築中の `FloorTileBlueprint` は通行可能（障害物として扱わない）。ただし `Curing` 中は立ち入り禁止（障害物扱い）
- **キャンセル**: エリア全体を一括キャンセル（部分キャンセル不可）

### 9.2 エンティティ構造

```
FloorConstructionSite (親エンティティ)
  ├─ phase: FloorConstructionPhase (Reinforcing | Pouring)
  ├─ area_bounds: TaskArea (エリア範囲)
  ├─ material_center: Vec2 (資材配送の集約地点)
  ├─ tiles_total, tiles_reinforced, tiles_poured
  └─ children: Vec<FloorTileBlueprint>

FloorTileBlueprint (子エンティティ、タイルごと)
  ├─ parent_site: Entity
  ├─ grid_pos: (i32, i32)
  ├─ state: FloorTileState
  └─ bones_delivered, mud_delivered
```

### 9.3 フェーズフロー

エリア作成 → **Reinforcing**: 骨を `material_center` へ配送 → ワーカーが各タイル補強 → 全完了で **Pouring**: 泥を配送 → ワーカーが各タイル注ぎ → 全完了で **Curing**: Soul退避・立ち入り禁止 → 時間経過で **Completion**: Floor Building 生成・Site/Tile despawn。

### 9.4 タイル状態 (FloorTileState)

`WaitingBones` → `ReinforcingReady` → `Reinforcing { progress }` → `ReinforcedComplete` → `WaitingMud` → `PouringReady` → `Pouring { progress }` → `Complete`

### 9.5 資材配送システム

**TransportRequest による自動配送**:
- `floor_construction_auto_haul_system` が Site ごとに必要資材を計算
- TransportRequest エンティティを生成（`TransportRequestKind::DeliverToFloorConstruction`）
- 資材は `material_center` 位置に集約配送される
- Priority: 10 (Blueprint と同等の高優先度)

**Phase に応じた資材**:
- **Reinforcing Phase**: 骨（Bone）を配送
  - 地面Boneがあれば通常 `Haul` で搬送
  - 地面Boneが無い場合は `BonePile` / River からの猫車直採取にフォールバック
- **Pouring Phase**: 泥（StasisMud）を配送（猫車必須）

### 9.6 タスク割り当て

**Designation の自動付与**:
- `floor_tile_designation_system` が各タイルの state を監視
- `ReinforcingReady` → `WorkType::ReinforceFloorTile` の Designation を付与
- `PouringReady` → `WorkType::PourFloorTile` の Designation を付与
- TaskSlots: 1（1タイルに1ワーカー）

**タスク実行**:
- **ReinforceFloorTile**: `reinforce_floor.rs`
  1. `GoingToMaterialCenter`: Site の material_center へ移動
  2. `PickingUpBones`: タイルが `ReinforcingReady` であることを確認
  3. `GoingToTile`: タイル位置へ移動
  4. `Reinforcing`: 作業実行（約3秒）
  5. `Done`: タスク完了、Designation 解放

- **PourFloorTile**: `pour_floor.rs`
  1. `GoingToMaterialCenter`: Site の material_center へ移動
  2. `PickingUpMud`: タイルが `PouringReady` であることを確認
  3. `GoingToTile`: タイル位置へ移動
  4. `Pouring`: 作業実行（約2秒）
  5. `Done`: タスク完了、Designation 解放

### 9.7 Phase Transition System

**Reinforcing → Pouring の移行**:
- `floor_construction_phase_transition_system` が実行
- 条件: `site.tiles_reinforced == site.tiles_total` かつ全タイルが `ReinforcedComplete`
- 処理:
  1. `site.phase` を `Pouring` に更新
  2. 全タイルの state を `WaitingMud` に更新
  3. 既存の Designation を削除（泥配送後に再付与される）

### 9.8 Completion System

**建設完了処理**:
- `floor_construction_completion_system` が実行
- 条件: 全タイルが `Complete` 状態
- 処理:
  1. （初回）`Curing` フェーズへ移行
  2. 対象タイルを障害物化し、範囲内の Soul を退避
  3. 一定時間（`FLOOR_CURING_DURATION_SECS`）待機
  4. 養生完了後、各タイルに `Floor` Building をスパウン
  5. 養生中は Site 中央に進捗バーを表示し、残り時間を可視化
  6. 完成床生成時にバウンスアニメーションを再生
  7. 建築中タイルを完成床へ置換（床として通行可能）
  8. FloorTileBlueprint エンティティを despawn
  9. FloorConstructionSite エンティティを despawn



### 9.10 壁建設フェーズ分割（Framing -> Coating, 養生なし）

- 壁のドラッグ配置は `Blueprint` 直建てではなく `WallConstructionSite` + `WallTileBlueprint` を生成する。
- 壁配置は `TaskMode::WallPlace` で行い、選択領域は **1 x n の直線**（水平または垂直）に制限される。
- 壁タイル候補は以下をすべて満たす必要がある:
  - `world_map.is_walkable == true`
  - `world_map.buildings` / `world_map.stockpiles` に未占有
  - 該当グリッドに完成済み `BuildingType::Floor` が存在する
- フェーズは 2 段階のみ:
  1. `Framing`: 木材搬入 (`WALL_WOOD_PER_TILE`) -> `FrameWallTile` 実行
  2. `Coating`: 泥搬入 (`WALL_MUD_PER_TILE`) -> `CoatWall` 実行
- `Framing` 完了タイルは即時に `Building { kind: Wall, is_provisional: true }` を生成し、通路分離・壁接続判定に参加する。
- `Coating` 完了時に `Building.is_provisional = false` へ更新し、`ProvisionalWall` を除去する。
- `Curing` 相当フェーズは持たず、全タイル `Complete` 到達で site / tile / request を即時 cleanup する。
- キャンセルは site 単位で処理され、搬入済み `Wood` / `StasisMud` を返却し、関連 request / 作業割り当てを解除する。
- すべての候補が無効な場合は site を生成せず、`Cannot Place` ツールチップで最初に検出した無効理由を表示する。
