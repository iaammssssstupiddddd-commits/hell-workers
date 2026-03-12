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
- shared crate 型 (`hw_core` / `hw_jobs` / `hw_logistics` / `hw_world` / `hw_spatial`) と Bevy 汎用 API だけで閉じる型定義・定数・アルゴリズム
- root-only resource / wrapper / relationship 契約の最終確定を持たないシステム関数
- 複数システムから共有されるコンポーネント・resource・marker 型
- UI の場合: ゲームエンティティクエリを持たないシステム、`Res<GameAssets>` を引数に取らないシステム

### src/ に置くもの
- `DamnedSoul`, `Destination`, `Path`, `Familiar`, `relationships.rs`, `events.rs` など root 所有型の契約を最終確定する処理
- `WorldMapRead/Write`, `PopulationManager`, concrete `SpatialGrid`, `PathfindingContext` など root 固有 resource / wrapper を前提にする system
- request 消費時に stale 再検証を行い、relationship/event/visual spawn を確定する adapter
- `Res<GameAssets>` を引数に取るシステム（Bevy は `Res<dyn Trait>` 不可）
- plugin wiring、互換 thin shell、root facade / wrapper system

### 判断フロー

```
root-only resource / wrapper / 契約最終確定が必要か？
  YES → src/ に置く

互換 import path のための thin shell / facade / wrapper を残す必要があるか？
  YES → src/ に置く

shared crate 型と Bevy 汎用 API だけで閉じるか？
  YES → 対応する hw_* クレートに置く
  NO  → src/ に置く
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

### 用語

- thin shell: `pub use` のみを持つ互換モジュール
- root wrapper system: root-only query/resource/event を束ねて crate 実装を呼ぶ system
- root facade/helper: 公開 API や互換 helper を root が所有し、低レベル実装へ委譲する層
- root adapter: request 消費時の再検証や visual/UI/resource 依存を伴うゲーム側 system
