# src — ゲームルートクレート

## 概要

Hell Workers ゲームのルートクレート。Bevy プラグインを組み合わせてゲーム全体を構成する。
各機能は専用クレート (`hw_*`) または `src/` 直下のサブモジュールに分割されている。

## ディレクトリ構成

| パス | 内容 |
|---|---|
| `main.rs` | エントリポイント。Bevy App にプラグインを登録する |
| `plugins/` | Bevy プラグイン定義（実行順序・システム登録） |
| `entities/` | ゲームエンティティ（DamnedSoul・Familiar）のスポーンと移動 |
| `systems/` | ゲームロジック実装（AI・タスク・ロジスティクス・視覚等） |
| `interface/` | プレイヤー入力・UI インタラクション・選択システム |
| `world/` | ワールドマップ・ゾーン管理 |
| `assets.rs` | アセットカタログ（画像・フォント等のハンドル管理） |
| `events.rs` | ルートクレート固有のイベント型定義 |
| `relationships.rs` | ECS Relationship 定義（ルートクレート固有） |
| `app_contexts.rs` | アプリケーションコンテキスト型 |

## プラグイン構成（main.rs 登録順）

```
MessagesPlugin       メッセージチャネル初期化
StartupPlugin        ワールド・リソース初期化
InputPlugin          カメラ・プレイヤー入力
SpatialPlugin        空間グリッド更新
LogicPlugin          AI・タスク・ロジック（Soul AI + Familiar AI + TransportRequest）
VisualPlugin         視覚フィードバック
InterfacePlugin      UI・選択・インタラクション
```

## フレーム実行順序

```
Input → Spatial → Logic → Actor → Visual → Interface
```

`Logic` フェーズ内の AI サイクル（Familiar → Soul の順）:

```
Perceive → ApplyDeferred → Update → ApplyDeferred → Decide → ApplyDeferred → Execute
```

---

## クレート境界の原則

`crates/hw_*` と `src/` の分割ルールを以下に示す。

### hw_* クレートに置くもの
- 純粋な型定義・定数・アルゴリズム（Bevy エンティティへの依存なし）
- ゲームエンティティ非依存のシステム関数（汎用 Query のみ）
- 複数システムから共有されるコンポーネント型

### src/ に置くもの
- `DamnedSoul`, `Destination`, `Path`, `Familiar` など Root 定義エンティティへのアクセス
- `WorldMap`・`Visibility`・`Transform` を変更するシステム
- ECS Relationship を生成・削除する処理
- タスク実行ハンドラ（ゲーム状態全体に依存）
- `SystemParam` ラッパー（`WorldMapRead` 等）

### 判断フロー

```
ゲームエンティティ (DamnedSoul / Destination / Path) に触れる?
  YES → src/ に置く

WorldMap を変更する、または Visibility / Transform を操作する?
  YES → src/ に置く

ECS Relationship を生成・削除する?
  YES → src/ に置く

それ以外（純粋ロジック・型定義）?
  → 対応する hw_* クレートに置く
```

### re-export パターン

src/ 側で hw_* の実装を公開する方法は 3 種類ある:

```rust
// パターン A: 単純 re-export
pub use hw_ai::soul_ai::decide::SoulDecideOutput;

// パターン B: ラッパーモジュール
pub mod escaping_apply {
    pub use hw_ai::soul_ai::execute::escaping_apply::*;
}

// パターン C: 拡張（純粋関数を re-export し、副作用関数を追加）
pub use hw_ai::soul_ai::helpers::work::is_soul_available_for_work; // hw_ai から
pub fn unassign_task(..., world_map: &WorldMap) { ... }            // src/ 独自
```
