# Soul / Jobs システムのクレート境界リファクタリング計画

## 概要
`bevy_app` に存在する経路探索のオーケストレーションや建設完了時のワールド更新ロジックを、それぞれ `hw_soul_ai`、`hw_jobs` などの該当クレートへ移動する。

## 対象と移動先
1. パスファインディング統括 (`bevy_app/src/entities/damned_soul/movement/pathfinding/mod.rs`)
   - 移動先: `hw_soul_ai`
   - 理由: 魂の経路探索オーケストレーションは `Soul AI` のドメインに属するため。コアアルゴリズム (`hw_world::pathfinding`) を呼び出す。
2. 建設完了判定 (`bevy_app/src/systems/jobs/building_completion/world_update.rs`)
   - 移動先: `hw_jobs` (または `hw_logistics` など適切な場所)
   - 理由: 建設完了時のワールド更新は `Jobs` ドメインの責務。`hw_jobs` などの既存型のみに依存しているため移行可能。

## 実施ステップ
1. パスファインディング統括ロジックを `hw_soul_ai/src/soul_ai/execute/` などの配下に移動する。
2. `Plugin` 登録を `hw_soul_ai` 側に移譲する。
3. `building_completion` のシステムを `hw_jobs` に移動し、同様に Plugin で登録する。

## 検証方法
- `cargo check --workspace` が通ること。
- ゲーム内で魂が正しく移動できること。
- 建設が完了し、ワールドの更新（通行不可になるなど）が正しく反映されること。