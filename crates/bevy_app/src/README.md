# src — ゲームルートクレート

## 概要

Hell Workers ゲームのルートクレート。Bevy プラグインを組み合わせてゲーム全体を構成する。
各機能は専用クレート (`hw_*`) または `src/` 直下のサブモジュールに分割されている。

## ディレクトリ構成

| パス | 内容 |
|---|---|
| `lib.rs` | 共有 Resource、公開 module、root re-export、`HellWorkersGamePlugin`、library unit testの入口 |
| `main.rs` | binary shell。process設定の解釈、window / render / backend設定、`HellWorkersGamePlugin` の追加と `run()` のみを担う |
| `plugins/` | Bevy プラグイン定義（実行順序・システム登録） |
| `entities/` | ゲームエンティティ（DamnedSoul・Familiar）のスポーンと移動 |
| `systems/` | ゲームロジック実装（AI・タスク・ロジスティクス・視覚等） |
| `interface/` | プレイヤー入力・UI インタラクション・選択システム |
| `world/` | ワールドマップ・ゾーン管理 |
| `assets.rs` | アセットカタログ（画像・フォント等のハンドル管理） |
| `events.rs` | ルートクレート固有のイベント型定義 |
| `relationships.rs` | ECS Relationship 定義（ルートクレート固有） |
| `app_contexts.rs` | アプリケーションコンテキスト型 |

## プラグイン構成（HellWorkersGamePlugin 登録順）

```
MessagesPlugin       メッセージチャネル初期化
DamnedSoulPlugin     Soul の population / movement / presentation adapter
StartupPlugin        ワールド・リソース初期化
InputPlugin          カメラ・プレイヤー入力
SpatialPlugin        空間グリッド更新
LogicPlugin          AI・タスク・ロジック（Soul AI + Familiar AI + TransportRequest）
VisualPlugin         視覚フィードバック
InterfacePlugin      UI・選択・インタラクション
SettingsPlugin       設定永続化
SavePlugin           Save/Load の Last apply phase
```

この plugin が `GameSystemSet` の実行順と game resource の初期化を一意に所有する。各 parent plugin が自身の child plugin を登録するため、shell や `HellWorkersGamePlugin` から child plugin を重ねて登録しない。

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
- `PopulationManager`, concrete `SpatialGrid`, `PathfindingContext`、および `hw_world::WorldMapRead/Write` を使った root adapter system
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
pub use hw_soul_ai::soul_ai::decide::SoulDecideOutput;

// パターン B: ラッパーモジュール
pub mod escaping_apply {
    pub use hw_soul_ai::soul_ai::execute::escaping_apply::*;
}

// パターン C: 拡張（leaf の request emitter を使い、root 側で再検証と visual spawn を追加）
pub use hw_soul_ai::soul_ai::execute::gathering_spawn::gathering_spawn_logic_system;
pub fn gathering_spawn_system(...) { ... } // src/ 独自
```

補足:
- 呼び出し側が少数で、定義元を直接 import しても root shell の責務が増えない場合は、`plugins/mod.rs` や `interface::camera` のような pass-through re-export を増やさず直接 import を選ぶ。
- thin shell を残すのは「共有される app shell 入口」または「root 側で ordering / adapter の意味がある path」に限定する。

### 用語

- thin shell: `pub use` のみを持つ互換モジュール
- root wrapper system: root-only query/resource/event を束ねて crate 実装を呼ぶ system
- root facade/helper: 公開 API や互換 helper を root が所有し、低レベル実装へ委譲する層
- root adapter: request 消費時の再検証や visual/UI/resource 依存を伴うゲーム側 system
