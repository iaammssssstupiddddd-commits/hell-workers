# visual — root visual shell + app-context visuals

## 役割

ビジュアル実装本体の大半は `hw_visual` クレートへ移っている。
このディレクトリは root 専有リソースや app context に依存する visual と、既存 import path を維持する thin shell だけを持つ。

## ファイル・ディレクトリ一覧

| ファイル/ディレクトリ | 内容 |
|---|---|
| `mod.rs` | root visual module の公開面。`hw_visual` へ移設済み領域を案内 |
| `placement_ghost.rs` | 建物配置ゴーストプレビュー。`GameAssets` / placement context 依存のため root 残留 |
| `task_area_visual.rs` | `TaskAreaMaterial` と task area 境界ビジュアル更新 |
| `floor_construction.rs` | `hw_visual::floor_construction` の thin shell |
| `wall_construction.rs` | `hw_visual::wall_construction` の thin shell |

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
