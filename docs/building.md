# 建築システム (Building System)

Hell-Workers における建築システムの基礎実装について説明します。

## 1. 概要

プレイヤーが設計図（Blueprint）を配置し、労働者が資材を運んで建設を完了させるシステムです。

## 2. コンポーネント

| コンポーネント | 役割 |
|:---|:---|
| `Blueprint` | 建設中の建物。`kind`, `progress`, `required_materials`, `delivered_materials` フィールドを持つ |
| `Building` | 完成した建物 |
| `BuildingType` | 建物の種類（`Wall`, `Floor`） |

### Blueprint フィールド

| フィールド | 型 | 説明 |
|:---|:---|:---|
| `kind` | `BuildingType` | 建物の種類 |
| `progress` | `f32` | 建築進捗 (0.0~1.0) |
| `required_materials` | `HashMap<ResourceType, u32>` | 必要資材量 |
| `delivered_materials` | `HashMap<ResourceType, u32>` | 搬入済み資材量 |

### 資材要件

| BuildingType | 必要資材 |
|:---|:---|
| Wall | 木材 × 2 |
| Floor | 石材 × 1 |

## 3. ワークフロー

```mermaid
flowchart TD
    A[プレイヤー] -->|設計図配置| B[Blueprint + Designation]
    B --> C{資材搬入済み?}
    C -->|No| D[待機/資材運搬]
    C -->|Yes| E[ソウルが建築作業]
    E -->|progress >= 1.0| F[Building 完成]
```

## 4. タスク実行フェーズ

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

## 7. ビジュアルフィードバック (Visual Feedback)

`building_visual.rs` モジュールによって、設計図の状態をプレイヤーに視覚的に伝えます。

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

## 8. 関連ファイル

- [jobs.rs](file:///f:/DevData/projects/hell-workers/src/systems/jobs.rs): `Blueprint`, `Building`, 建設完了ロジック
- [building_visual.rs](file:///f:/DevData/projects/hell-workers/src/systems/building_visual.rs): ビジュアルフィードバック（全システム）
- [utils/progress_bar.rs](file:///f:/DevData/projects/hell-workers/src/systems/utils/progress_bar.rs): 汎用プログレスバー実装
- [utils/animations.rs](file:///f:/DevData/projects/hell-workers/src/systems/utils/animations.rs): パルス・バウンスアニメーション実装
- [utils/floating_text.rs](file:///f:/DevData/projects/hell-workers/src/systems/utils/floating_text.rs): フローティングテキスト実装
- [visual.rs](file:///f:/DevData/projects/hell-workers/src/plugins/visual.rs): システム登録
- [build.rs](file:///f:/DevData/projects/hell-workers/src/systems/soul_ai/task_execution/build.rs): `handle_build_task` (進捗更新)
- [selection.rs](file:///f:/DevData/projects/hell-workers/src/interface/selection.rs): `blueprint_placement`
- [assets.rs](file:///f:/DevData/projects/hell-workers/src/assets.rs): 各種アイコンアセット
