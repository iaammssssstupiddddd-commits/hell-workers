# visual — root visual shell + app-context visuals

## 役割

ビジュアル実装本体の大半は `hw_visual` クレートへ移っている。
このディレクトリは root 専有リソースや app context に依存する visual と、既存 import path を維持する thin shell だけを持つ。

## ファイル・ディレクトリ一覧

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | root visual module の公開面。`hw_visual` へ移設済み領域を案内。`floor_construction` / `wall_construction` inline modules を含む |
| `placement_ghost.rs` | 建物配置ゴーストプレビュー。`GameAssets` / placement context 依存のため root 残留 |
| `task_area_visual.rs` | `TaskAreaMaterial` と task area 境界ビジュアル更新 |
| `building3d_cleanup.rs` | 3D 建物エンティティのクリーンアップ |
| `camera_sync.rs` | カメラ同期 |
| `character_proxy_3d.rs` | キャラクター 3D プロキシ |
| `elevation_view.rs` | 高度ビュー |
| `wall_orientation_aid.rs` | 壁向き補助 |

## TaskAreaMaterial

`task_area_visual.rs` で定義されるカスタムマテリアル。
タスクエリアの境界を破線アニメーションで表示する。
`pub use task_area_visual::TaskAreaMaterial` として `mod.rs` から公開されている。

---

## hw_visual との境界

| src/systems/visual/ に置くもの | `hw_visual` に置くもの |
|---|---|
| `placement_ghost.rs` など root-only context 依存 visual | floor / wall construction visual を含む visual system 本体 |
| `TaskAreaMaterial` と root `TaskContext` 依存の update | blueprint / haul / dream / gather / soul などの表示同期 |
| 互換 re-export パス | `HwVisualPlugin` による system 登録 |
