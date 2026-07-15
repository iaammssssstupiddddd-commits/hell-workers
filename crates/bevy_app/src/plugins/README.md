# plugins — Bevy プラグイン定義

## 役割

ゲームの各フェーズに対応する Bevy `Plugin` をまとめるディレクトリ。
各プラグインはシステムの登録・リソース初期化・実行順序の配線のみを担い、ロジックは `systems/` に実装する。

## プラグイン一覧

| ファイル | プラグイン | フェーズ | 内容 |
|---|---|---|---|
| `game.rs` | `HellWorkersGamePlugin` | production 構成 | game resource / state / system set と parent game plugin の一意な登録 |
| `messages.rs` | `MessagesPlugin` | 初期化 | メッセージチャネル・Observer 登録 |
| `startup/` | `StartupPlugin` | Startup | マップ生成・リソース初期化・初期スポーン |
| `input.rs` | `InputPlugin` | Input | カメラ操作・プレイヤー入力 |
| `spatial.rs` | `SpatialPlugin` | Spatial | 全空間グリッドの毎フレーム更新 |
| `logic.rs` | `LogicPlugin` | Logic | Soul AI・Familiar AI・タスク・建設・ロジスティクス |
| `visual.rs` | `VisualPlugin` | Visual | 視覚フィードバック・アニメーション同期 |
| `interface.rs` | `InterfacePlugin` | Interface | UI・選択・インタラクション |
| `interface_debug.rs` | (デバッグ用) | Interface | デバッグ UI 補助 |

`main.rs` は process の `PerfScenarioConfig` 解釈と Window / Log / Render backend の設定だけを行い、構成済みの config を `HellWorkersGamePlugin::new(...)` へ渡す。`game.rs` は `MessagesPlugin`、`DamnedSoulPlugin`、`StartupPlugin`、`InputPlugin`、`SpatialPlugin`、`LogicPlugin`、`VisualPlugin`、`InterfacePlugin`、`SettingsPlugin`、`SavePlugin` をこの順で登録する。`SettingsPlugin` は `SavePlugin` より前に置き、Save/Load の `Last` apply phase が settings persistence の後に実行される契約を満たす。

`LogicPlugin` は Soul/Familiar AI、transport、logistics と root の ordering facade を、`DamnedSoulPlugin` は population / movement と root asset 依存の spawn・presentation adapter を所有する。両 plugin の child plugin や system を外側から重ねて登録しない。

## startup/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `mod.rs` | `StartupPlugin` と Startup/PostStartup 配線を持つ root shell |
| `asset_catalog.rs` | アセットハンドルの一括ロードと `AssetCatalog` リソース登録 |
| `perf_scenario.rs` | `--perf-scenario` フラグ時の高負荷テスト用スポーン |
| `startup_systems.rs` | camera/resource 初期化、初期スポーン、地形境界生成の実装本体 |

## MessagesPlugin について

新しい `Message` 型を実装した際は必ず `messages.rs` に登録すること。
登録漏れがあるとメッセージが配信されない。

## 設計方針

- プラグインファイルにはシステム登録のみを記述する
- `in_state(...)` / `run_if(...)` による条件分岐はここで設定する
- `ApplyDeferred` の挿入位置もここで管理する
- `mod.rs` はモジュール宣言にとどめ、1 段だけの `pub use` 集約層にしない。呼び出し側は `crate::plugins::input::InputPlugin` のように定義モジュールを直接参照する
