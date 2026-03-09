# visual — 視覚フィードバックシステム

## 役割

ゲーム内の全ビジュアルフィードバック（建設状態・運搬表示・ゴーストプレビュー・エフェクト等）を管理するシステム群。
`GameSystemSet::Visual` フェーズで実行され、`Logic`/`Actor` フェーズの結果を視覚に同期する。

## ファイル・ディレクトリ一覧

| ファイル/ディレクトリ | 内容 |
|---|---|
| `blueprint/` | ブループリット配置・建設進捗ビジュアル |
| `dream/` | Dream（夢通貨）エフェクト表示 |
| `gather/` | 採取アニメーション |
| `haul/` | 運搬アイコン・表示 |
| `plant_trees/` | 植林エフェクト |
| `speech/` | セリフバブル・吹き出し |
| `fade.rs` | フェードイン/アウトエフェクト |
| `floor_construction.rs` | 床建設ビジュアル同期 |
| `mud_mixer.rs` | 泥ミキサーアニメーション |
| `placement_ghost.rs` | 建物配置ゴーストプレビュー |
| `site_yard_visual.rs` | Site・Yard エリアのビジュアル |
| `soul.rs` | Soul ビジュアル同期（体力バー等） |
| `tank.rs` | タンク水位ビジュアル |
| `task_area_visual.rs` | タスクエリア境界ビジュアル（`TaskAreaMaterial` カスタムシェーダー） |
| `wall_connection.rs` | 壁タイル接続ビジュアル更新 |
| `wall_construction.rs` | 壁建設ビジュアル同期 |

## TaskAreaMaterial

`task_area_visual.rs` で定義されるカスタムマテリアル。
タスクエリアの境界を破線アニメーションで表示する。
`pub use task_area_visual::TaskAreaMaterial` として `mod.rs` から公開されている。
