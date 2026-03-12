# Initial Resource Bootstrap Split Plan

既存の現行計画 `familiar-state-decision-adapter-split-plan-2026-03-12` と `refactor-worldreadapi-construction-shared` は AI 判断層と建設共通化を対象にしているため、本計画はそれらと重ならない `src/systems/logistics/initial_spawn.rs` の責務分離に限定する。

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `initial-resource-bootstrap-split-plan-2026-03-12` |
| ステータス | `Draft` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `AI (Codex)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `src/systems/logistics/initial_spawn.rs` が「固定障害物スポーン」「初期資材配置」「Site/Yard 初期化」「WheelbarrowParking 生成」「WorldMap 更新」「ログ出力」を 1 ファイルで同時に扱っており、初期レイアウト変更時の影響範囲が広い。
- 到達したい状態: 外部公開 API は `initial_resource_spawner` のまま維持しつつ、内部を「レイアウト計算」「地形/資材スポーン」「施設/ゾーン生成」「結果集計」に分け、追加要素を独立に保守できる構造へ整理する。
- 成功指標:
  - `initial_resource_spawner` が orchestration のみを担う薄い facade になる
  - Site/Yard と WheelbarrowParking の配置判定が pure helper 化される
  - 木/岩/木材ばら撒きの walkable 判定と spawn 手順の重複が縮小される
  - `cargo check --workspace` が成功する

## 2. スコープ

### 対象（In Scope）

- `src/systems/logistics/initial_spawn.rs` のモジュール分割
- 初期レイアウト計算ロジックの pure helper 抽出
- 初期スポーン結果のログ責務整理
- 関連ドキュメントの境界説明更新

### 非対象（Out of Scope）

- `GameAssets` の定義変更やアセット追加
- マップ定数（`TREE_POSITIONS`、`ROCK_POSITIONS`、`SITE_*` など）の値変更
- `hw_logistics` への crate 移設
- gameplay 上の初期配置仕様変更
- perf scenario や `StartupPlugin` 全体の再設計

## 3. 現状とギャップ

- 現状:
  - `initial_resource_spawner` が tree / rock / initial wood / wheelbarrow parking / site / yard を順に直列で生成している
  - `spawn_site_and_yard` と `spawn_initial_wheelbarrow_parking` が座標検証、ワールド座標変換、entity spawn、ログ出力をまとめて持つ
  - tree / rock 生成は `pos_to_idx` と `terrain_at_idx(...).is_walkable()` の同型分岐をそれぞれ持つ
- 問題:
  - 初期配置を 1 箇所追加するだけで `Commands` / `WorldMap` / `GameAssets` / constants が密集するファイルを同時に触る必要がある
  - 「配置が不正なため skip した」理由がレイアウト計算と spawn 実装に分散し、検証しづらい
  - startup 専用ロジックでありながら pure に確認できる部分が少なく、回帰がコードレビュー頼みになる
- 本計画で埋めるギャップ:
  - レイアウト計算と Bevy spawn を分離し、境界を明示する
  - 初期スポーン群を役割別の小モジュールへ分割し、将来の追加変更を局所化する
  - skip 理由や spawn 件数を report に集約し、確認経路を一本化する

## 4. 実装方針（高レベル）

- 方針:
  - `src/systems/logistics/initial_spawn.rs` を `src/systems/logistics/initial_spawn/` ディレクトリへ移し、`mod.rs` を facade とする
  - pure 計算と ECS spawn を分離するため、`layout.rs` と executor 系モジュールを分ける
  - 既存の公開関数名 `initial_resource_spawner` は維持し、`src/systems/logistics/mod.rs` の re-export 互換を壊さない
- 設計上の前提:
  - root 側に残る理由は `GameAssets` と `WorldMapWrite` 依存であり、crate 境界変更は行わない
  - `WorldMap::register_completed_building_footprint` や `add_grid_obstacle` の呼び順は現行仕様を維持する
  - startup 時のスポーン順序（障害物 -> loose item -> parking/site/yard -> log）も維持する
- Bevy 0.18 APIでの注意点:
  - `Commands` と `WorldMapWrite` の同時利用は facade から各 executor に順に渡し、借用期間を広げすぎない
  - child spawn を含む建物生成は `with_children` を executor 内に閉じ込め、layout helper には `EntityCommands` を持ち込まない
  - `Res<GameAssets>` は `&GameAssets` に narrowed して helper へ渡し、system param を pure helper に漏らさない

## 5. マイルストーン

## M1: 初期レイアウト計算の pure helper 抽出

- 変更内容:
  - `Site`/`Yard` と `WheelbarrowParking` の配置計算を pure helper に抽出する
  - skip 理由を enum か result 型で返せるようにし、warn 文の組み立てを facade 側へ寄せる
  - `WorldMap::grid_to_world` へ渡す前のグリッド境界検証を 1 箇所へ集約する
- 変更ファイル:
  - `src/systems/logistics/initial_spawn/mod.rs`
  - `src/systems/logistics/initial_spawn/layout.rs`
  - `src/systems/logistics/mod.rs`
- 完了条件:
  - [ ] `spawn_site_and_yard` のレイアウト計算部分が `layout.rs` へ移っている
  - [ ] `spawn_initial_wheelbarrow_parking` の占有マス計算が pure helper 化されている
  - [ ] skip 条件が spawn 本体ではなく layout result として扱える
- 検証:
  - `cargo check --workspace`

## M2: 地形/資材/施設スポーンの executor 分割

- 変更内容:
  - tree / rock / initial wood / site-yard / wheelbarrow parking を別 executor 関数へ分割する
  - tree / rock の共通 walkable 判定と sprite spawn の骨格を helper 化する
  - `initial_resource_spawner` は executor 呼び出し順の orchestration のみを担う
- 変更ファイル:
  - `src/systems/logistics/initial_spawn/mod.rs`
  - `src/systems/logistics/initial_spawn/terrain_resources.rs`
  - `src/systems/logistics/initial_spawn/facilities.rs`
  - `src/systems/logistics/initial_spawn/report.rs`
- 完了条件:
  - [ ] 地形資源系と施設系が別モジュールに分離されている
  - [ ] tree / rock spawn の重複判定ロジックが共通化されている
  - [ ] facade の主関数が 60 行前後まで縮小している
- 検証:
  - `cargo check --workspace`

## M3: startup 境界とドキュメント同期

- 変更内容:
  - `src/systems/logistics/README.md` と `docs/logistics.md` / `docs/architecture.md` に、initial spawn の責務境界を反映する
  - startup ログを `InitialSpawnReport` 経由でまとめ、追加時の確認ポイントを明文化する
- 変更ファイル:
  - `src/systems/logistics/README.md`
  - `docs/logistics.md`
  - `docs/architecture.md`
  - `src/plugins/startup/mod.rs`
- 完了条件:
  - [ ] root に残る理由が `initial_spawn` のモジュール分割後の形で説明されている
  - [ ] startup 側は `initial_resource_spawner` の入口だけを知る構造を維持している
  - [ ] docs の参照先が新しいモジュール構成と矛盾しない
- 検証:
  - `cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `WorldMap` 更新順が変わり初期障害物登録が欠落する | 初期配置が壊れ、歩行判定や建物占有がずれる | executor 分割後も `add_grid_obstacle` / `register_completed_building_footprint` 呼び出し順を固定し、M2 で差分レビューする |
| pure helper へ寄せる過程で仕様変更が混入する | レイアウトが意図せず変わる | constants の値は一切触らず、helper は現式をそのまま移送してから整形する |
| logging 集約で skip 理由が失われる | startup 不具合の調査性が落ちる | `InitialSpawnReport` に spawn 数だけでなく skip 理由も保持する |

## 7. 検証計画

- 必須:
  - `cargo check --workspace`
- 手動確認シナリオ:
  - `cargo run` で初期木・岩・木材・Site/Yard・WheelbarrowParking が従来どおり出現すること
  - startup ログに spawn 件数と skip 理由が期待どおり出ること
- パフォーマンス確認（必要時）:
  - Startup/PostStartup のみが対象なので、ランタイム perf 改善は主目的にしない
  - 期待効果は「将来の変更時に不要な全体見直しが減る」ことで、実行時性能は概ね中立

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1 の pure helper 抽出
  - M2 の executor 分割
  - M3 の docs / report 整理
- 戻す時の手順:
  - モジュール分割だけを元に戻す場合は `initial_spawn/mod.rs` facade を残して内部関数を再集約する
  - 挙動差分が出た場合は M2 だけを revert し、M1 の layout helper までは維持して原因を切り分ける

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン:
- 未着手/進行中:
  - M1〜M3 全て未着手

### 次のAIが最初にやること

1. `src/systems/logistics/initial_spawn.rs` を `mod.rs` 化する前に、tree / rock / wood / site-yard / parking の責務境界をコメントで整理する
2. `layout.rs` に site-yard と parking の pure 計算を先に移し、spawn 本体はまだ触らず `cargo check --workspace` を通す
3. その後 executor 分割と report 集約を進め、最後に docs を同期する

### ブロッカー/注意点

- `src/plugins/startup/mod.rs` は PostStartup の chain 順序を持っているため、入口関数名や呼び出し順は変えない
- `initial_spawn` は `GameAssets` の texture handle を直接使うため、`hw_logistics` へ寄せない
- `INITIAL_WHEELBARROW_PARKING_GRID` と `occupied` 2x2 配置は挙動依存があるため、先にレイアウト式を固定化してから分割する

### 参照必須ファイル

- `docs/DEVELOPMENT.md`
- `docs/logistics.md`
- `docs/architecture.md`
- `src/systems/logistics/README.md`
- `src/systems/logistics/initial_spawn.rs`
- `src/plugins/startup/mod.rs`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-12` / `not run (plan only)`
- 未解決エラー:
  - `N/A`

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `AI (Codex)` | 初版作成 |
