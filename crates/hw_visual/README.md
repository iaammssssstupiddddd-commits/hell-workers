# hw_visual — ビジュアルシステム集約クレート

## 役割

描画・アニメーション・UI 以外のゲーム状態を読み取り、見た目へ反映する visual system を集約するクレート。
`GameAssets` 自体は持たず、必要なハンドルは root startup が Resource として注入する。

## 主要モジュール

| ファイル/ディレクトリ | 内容 |
|---|---|
| `lib.rs` | `HwVisualPlugin` と visual system の登録 |
| `handles.rs` | `WallVisualHandles`, `SpeechHandles` などの handle resource |
| `soul/` | Soul の progress bar, status, task link, idle/gathering/vitals visual |
| `speech/` | 吹き出しと observer ベースの発話演出 |
| `blueprint/` | 設計図 visual, progress bar, delivery popup |
| `gather/` | 採取インジケータ、resource highlight |
| `haul/` | 運搬 visual、手押し車追従 |
| `dream/` | Dream bubble, particle, popup |
| `plant_trees/` | 植樹演出 |
| `site_yard_visual.rs` | site / yard 境界表示 |
| `task_area_visual.rs` | `TaskAreaMaterial`, `TaskAreaVisual` 型定義 |

## soul/ の責務

`soul/` は Soul の見た目専用モジュールで、次を担当する。

- progress bar の spawn / update / follow
- status icon 表示
- task link gizmo 表示
- `idle_visual_system`
- `gathering_visual_update_system`
- `gathering_debug_visualization_system`
- `familiar_hover_visualization_system`

`idle/gathering/vitals` は root `src/systems/soul_ai/visual/*` から移設済みで、root 側は互換 re-export のみを持つ。

## Plugin 登録ルール

- この crate に実装本体がある visual system は `HwVisualPlugin` を唯一の登録元にする。
- root 側の `pub use` / thin shell は互換パス維持や run condition 付与のために残してよいが、同じ system function を再登録しない。
- `DebugVisible` や `GameAssets` のような root-only resource を使う条件付け・startup 注入だけを root 側が担当する。

## 依存クレート

- `hw_core`, `hw_jobs`, `hw_logistics`, `hw_spatial`, `hw_world`, `hw_ui`, `bevy`

## src/ との境界

| hw_visual に置くもの | src/ に置くもの |
|---|---|
| `GameAssets` 非依存の visual system 本体 | `GameAssets` のロードと handle resource 注入 |
| observer ベースの演出表示 | `DebugVisible` や `PlayMode` による run condition |
| Soul/Familiar/Blueprint/Haul の見た目更新 | `placement_ghost`, `floor_construction`, `wall_construction` など root-only app context 依存 visual |
| `TaskAreaMaterial`, `TaskAreaVisual` 型 | `update_task_area_material_system` のような root `TaskContext` 依存 system |
