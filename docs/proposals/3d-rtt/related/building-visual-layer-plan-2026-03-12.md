# 建築物ビジュアル多層レイヤー（2D互換・3D準備）詳細設計書

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `building-visual-layer-plan-2026-03-12` |
| ステータス | `Draft` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-15` |
| 作成者 | `Gemini CLI` |
| 関連提案 | `docs/proposals/3d-rtt/3d-rendering-rtt-proposal-2026-03-14.md`<br>`docs/proposals/3d-rtt/related/spatial-grid-architecture-plan-2026-03-12.md` |

---

## 1. 目的と設計思想

### 1-1. 解決したい課題
- **単一スプライトの限界**: `Building` エンティティが `Sprite` を1枚しか持てないため、床の上に壁や配線を重ねる表現ができない。
- **暗黙的な Z 管理**: `spawn.rs` で `Z_AURA` 等を直接指定しており、重なり順の制御が分散している。
- **3D 移行のコスト**: 今のまま 3D-RTT を導入すると、既存の 2D コードの多くを書き換える必要がある。

### 1-2. 到達目標（2D+α 構成）
- **ECS 親子構造の導入**: `Building` エンティティを「論理（データ）」のコンテナとし、描画実体（Sprite）を子エンティティとして分割する。
- **Z-スロット方式**: 建築物内の要素（床、壁、配線、家具、照明）に専用の Z 帯域を割り当てる。
- **3D 透明性**: ロジック層からは「見た目が 2D か 3D か」を意識させず、プラグイン構成の変更だけで描画方式を切り替えられるようにする。

---

## 2. 詳細設計：Z スロット定義（render.rs の拡張）

既存の `Z_MAP (0.0)` から `Z_AURA (0.2)` までの空間を 0.01 単位で精密に定義し、重なりを制御する。

| レイヤー | 2D Z 値 | 位置関係（既存定数） | 用途・具体例 |
|---------|---------|-------------------|------------|
| **Z_BUILDING_FLOOR** | `0.05` | `Z_MAP` < **これ** < `Z_ROOM_OVERLAY` | 床タイル（MudFloor） |
| **Z_BUILDING_PIPE** | `0.06` | 床の直上 | 配管（将来追加される空間データ） |
| **Z_BUILDING_WIRE** | `0.07` | 配管の直上 | 配線（電力網） |
| **Z_BUILDING_DECAL** | `0.09` | `Z_ROOM_OVERLAY` の上 | 汚れ、血痕、サイン |
| **Z_BUILDING_STRUCT** | `0.12` | `Z_ITEM` (0.1) < **これ** < `Z_AURA` (0.2) | 壁（Wall）、ドア（Door） |
| **Z_BUILDING_FURNITURE**| `0.15` | 構造体の上 | 設備（Tank, MudMixer）、家具 |
| **Z_BUILDING_LIGHT** | `0.18` | 家具の上 | 照明の光円、エフェクト |

---

## 3. ECS 表現：VisualLayer システム

### 3-1. `VisualLayerKind` と `BuildingType` のマッピング

既存の `BuildingType` を以下のレイヤーカテゴリに分類する。

| カテゴリ | `BuildingType` | デフォルト Z スロット |
|---------|----------------|-------------------|
| **Structure** | `Wall`, `Door`, `Bridge` | `Z_BUILDING_STRUCT` |
| **Base** | `Floor` | `Z_BUILDING_FLOOR` |
| **Plant** | `Tank`, `MudMixer` | `Z_BUILDING_FURNITURE` |
| **Temporary** | `SandPile`, `BonePile`, `RestArea` | `Z_BUILDING_FURNITURE` |

### 3-2. コンポーネント定義

```rust
// crates/hw_visual/src/layer/mod.rs

#[derive(Component)]
pub struct VisualLayer {
    pub kind: VisualLayerKind,
}

pub enum VisualLayerKind {
    StaticSprite,  // 通常のスプライト
    Connectable,   // 隣接接続が必要なもの（壁・配線）
    Animated,      // アニメーションが必要なもの（ドア・混合機）
    Effect,        // 半透明エフェクト（照明）
}
```

---

## 4. 実装詳細：`spawn_completed_building` の刷新

既存の `spawn.rs` を以下のようにリファクタリングし、多層化を可能にする。

```rust
// 移行後の spawn.rs イメージ
pub(super) fn spawn_completed_building(
    commands: &mut Commands,
    bp: &Blueprint,
    transform: &Transform,
    game_assets: &GameAssets,
) -> Entity {
    // 1. 親エンティティ（ロジック層）の生成
    let building_entity = commands.spawn((
        Building { kind: bp.kind, .. },
        SpatialBundle::from_transform(*transform),
        Name::new(format!("Building ({:?})", bp.kind)),
    )).id();

    // 2. 基本ビジュアルレイヤーの追加
    commands.entity(building_entity).with_children(|parent| {
        // 全ての建築物は少なくとも1つのビジュアルレイヤーを持つ
        spawn_main_layer(parent, bp, game_assets);

        // 将来的に追加されるサブレイヤー（例：フロア、配線）
        if needs_floor_overlay(bp.kind) {
            spawn_sub_layer(parent, BuildingLayerType::Floor, game_assets);
        }
    });

    building_entity
}

fn spawn_main_layer(parent: &mut ChildBuilder, bp: &Blueprint, assets: &GameAssets) {
    let (image, z_offset) = match bp.kind {
        BuildingType::Wall => (assets.wall_isolated.clone(), Z_BUILDING_STRUCT),
        BuildingType::Floor => (assets.mud_floor.clone(), Z_BUILDING_FLOOR),
        // ... 他の BuildingType も Z 定数に従いマッピング
    };

    parent.spawn((
        VisualLayer { kind: VisualLayerKind::StaticSprite },
        Sprite { image, ..default() },
        Transform::from_xyz(0.0, 0.0, z_offset), // 親からの相対 Z
    ));
}
```

---

## 5. 将来の 3D-RTT 移行へのブリッジ

本設計がなぜ 3D 移行を容易にするのか、その具体的な技術的根拠を提示する。

1.  **RenderLayers の一括付与**:
    `VisualLayer` を持つ子エンティティすべてに `RenderLayers::layer(1)` を追加するだけで、描画パスを 3D 側へ切り離せる。
2.  **Transform の一貫性**:
    子エンティティの `Transform.translation.z` は相対座標であるため、3D 化の際はこれを `translation.y` (高さ) にマッピングし直すだけで論理的な高さ関係が維持される。
3.  **モデルの差し替え**:
    `VisualLayer` エンティティから `Sprite` コンポーネントを remove し、`SceneBundle` を insert するシステムを書くだけで、スポーンロジック本体（`spawn.rs`）を汚さずにビジュアルを刷新できる。

---

## 6. AI引継ぎメモ

### 現在地
- **進捗**: 3D-RTT のビジョンを維持しつつ、即座に実装可能な 2D 多層レイヤー設計として詳細化完了。

### 次のAIが最初にやること
1. `crates/hw_core/src/constants/render.rs` に設計した Z 定数群を追加。
2. `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs` を読み、親子構造へのリファクタリングを1つの `BuildingType` (例: Floor) から順次実施。

### 参照必須ファイル
- `crates/hw_core/src/constants/render.rs` (Z値の競合確認)
- `crates/hw_jobs/src/model.rs` (`BuildingType` 一覧)
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs` (リファクタリング対象)

---

## 7. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `Claude` | 初版作成 |
| `2026-03-15` | `Gemini CLI` | 2D多層化・3D準備段階への再定義。Zスロットの精密化、ECS階層構造の詳細化。 |
