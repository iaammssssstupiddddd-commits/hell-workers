# 伐採・運搬ビジュアルシステム (Gather & Haul Visual)

Hell-Workers における伐採・運搬作業のビジュアルフィードバックについて説明します。

## 1. 概要

プレイヤーがゲーム内の作業状態を視覚的に把握できるよう、以下のビジュアルフィードバックを提供します：

- ワーカーの作業状態（アイコン表示）
- 対象リソースのハイライト
- 運搬中アイテムの表示
- タスクリンク（ワーカーと目標の接続線）

## 2. モジュール構成

```
systems/visual/
├── gather/                 # 伐採・採掘ビジュアル
│   ├── mod.rs              # 定数・re-exports
│   ├── components.rs       # コンポーネント定義
│   ├── worker_indicator.rs # 斧/ツルハシアイコン
│   └── resource_highlight.rs # リソースハイライト
├── haul/                   # 運搬ビジュアル
│   ├── mod.rs              # 定数・re-exports
│   ├── components.rs       # コンポーネント定義
│   ├── carrying_item.rs    # 運搬中アイテム表示
│   └── effects.rs          # ドロップエフェクト
├── speech/                 # セリフ吹き出しシステム (NEW)
│   ├── mod.rs              # システム登録
│   ├── components.rs       # 感情・アニメーション定義
│   ├── spawn.rs            # 生成ロジック
│   ├── animation.rs        # アニメーション制御
│   └── typewriter.rs       # タイプライター効果
└── soul.rs                 # タスクリンク
```

## 3. コンポーネント

### 伐採関連

| コンポーネント | 役割 |
|:---|:---|
| `WorkerGatherIcon` | ワーカー頭上の斧/ツルハシアイコンへの参照 |
| `HasGatherIndicator` | インジケータ付与済みマーカー |
| `ResourceVisual` | リソースのビジュアル状態（パルス、透明度） |
| `ResourceHighlightState` | `Normal`, `Designated`, `Working` の3状態 |

### 運搬関連

| コンポーネント | 役割 |
|:---|:---|
| `CarryingItemVisual` | 運搬中のミニアイコンへの参照 |
| `HasCarryingIndicator` | インジケータ付与済みマーカー |
| `DropPopup` | ドロップ時のポップアップ |

## 4. ビジュアル表示

### ワーカーインジケータ

| タスク | アイコン | 色 |
|:---|:---|:---|
| Chop（伐採） | 斧 | 緑 (0.4, 0.9, 0.3) |
| Mine（採掘） | ツルハシ | 灰 (0.7, 0.7, 0.8) |

アイコンはワーカーの頭上に表示され、上下に揺れる（bob）アニメーションが適用されます。

### リソースハイライト

| 状態 | 表示 |
|:---|:---|
| Designated | シアンティント + パルスアニメーション |
| Working | 固定ティント |

### 運搬中アイテム

`Holding` リレーションシップを持つワーカーの頭上に、運搬中のリソースタイプに応じたミニアイコンが表示されます。

### タスクリンク

| タスク | 線の色 |
|:---|:---|
| Gather（採取） | 緑 (0.0, 1.0, 0.0, 0.4) |
| Haul（運搬） | 黄 (1.0, 1.0, 0.0, 0.4) |
| Build（建築） | 白 (1.0, 1.0, 1.0, 0.5) |
| HaulToBlueprint | 薄黄 (1.0, 1.0, 0.5, 0.4) |

目標地点には半径4pxのマーカー円が表示されます。

### セリフ吹き出し (Speech Bubbles)

ワーカーの感情や使い魔の命令を視覚化します。詳細は [speech_system.md](speech_system.md) を参照してください。

## 5. 関連ファイル

- [visual/gather/mod.rs](file:///home/iaamm/projects/hell-workers/src/systems/visual/gather/mod.rs)
- [visual/haul/mod.rs](file:///home/iaamm/projects/hell-workers/src/systems/visual/haul/mod.rs)
- [visual/soul.rs](file:///home/iaamm/projects/hell-workers/src/systems/visual/soul.rs)
- [utils/worker_icon.rs](file:///home/iaamm/projects/hell-workers/src/systems/utils/worker_icon.rs)
- [visual.rs](file:///home/iaamm/projects/hell-workers/src/plugins/visual.rs)
