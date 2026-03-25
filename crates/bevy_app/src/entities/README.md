# entities — ゲームエンティティ定義

## 役割

ゲームに登場するエンティティ（DamnedSoul・Familiar）の**スポーン・移動・アニメーション・ビジュアル**を実装するディレクトリ。
AI 意思決定ロジックは `systems/soul_ai/`・`systems/familiar_ai/`（および `hw_soul_ai`・`hw_familiar_ai` クレート）に分離されている。

## ディレクトリ構成

| ディレクトリ | エンティティ | 内容 |
|---|---|---|
| `damned_soul/` | DamnedSoul（魂） | スポーン・移動・Observer |
| `familiar/` | Familiar（使い魔） | スポーン・移動・アニメーション・音声 |

## damned_soul/

| ファイル | 内容 |
|---|---|
| `mod.rs` | モジュール宣言と公開 API |
| `spawn.rs` | `DamnedSoul` エンティティのスポーン処理 |
| `observers.rs` | Observer ハンドラ（バイタル変化・タスク完了等への反応） |
| `names.rs` | ランダム名前生成 |
| `movement/` | グリッドベースの移動システム |

## familiar/

| ファイル | 内容 |
|---|---|
| `mod.rs` | モジュール宣言と公開 API |
| `spawn.rs` | `Familiar` エンティティのスポーン処理 |
| `movement.rs` | Familiar の移動システム |
| `animation.rs` | スプライトアニメーション |
| `components.rs` | Familiar 固有コンポーネント |
| `range_indicator.rs` | command_radius 表示インジケーター |
| `voice.rs` | 音声フィードバック |

## 依存関係

- コンポーネント定義: `hw_core`（`DamnedSoul`, `Familiar`）
- AI 状態: `hw_soul_ai` / `hw_familiar_ai` / `systems/soul_ai/` / `systems/familiar_ai/`
- ビジュアル: `systems/visual/`
- スポーン位置補正: `hw_world::find_nearby_walkable_grid` を各 spawn 実装から直接利用する。`entities/mod.rs` に中継用 wrapper は置かない
