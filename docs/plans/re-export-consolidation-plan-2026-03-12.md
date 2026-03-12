# re-export 削減・統合計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `re-export-consolidation-plan-2026-03-12` |
| ステータス | `Completed` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `AI (Codex)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  - `src/` 配下に `pub use` が 255 行あり、そのうち 143 行が `hw_*` crate の直接中継、61 行がワイルドカード再公開になっている。
  - root shell の互換層として必要な facade と、単なる多段中継が混在しており、正規 import path が不明確になっている。
  - 同じ型や helper が複数経路から公開されており、削除や移設時の影響範囲を読みづらい。
- 到達したい状態:
  - 「1シンボルにつき正規 public path は 1 つ」を原則にする。
  - root crate に残す re-export は app shell として意味のある集約点だけに絞る。
  - leaf module の純粋な passthrough re-export を減らし、公開面を `mod.rs` / facade へ寄せる。
- 成功指標:
  - `pub use` 総数を段階的に削減し、特に `hw_*` 直接中継と `*` 再公開を縮小できる。
  - 同一シンボルの並列 export を排除し、主要ドメインごとに正規入口が文書化される。
  - `cargo check --workspace` が通り、plugin wiring や import path に回帰がない。

## 2. スコープ

### 対象（In Scope）

- facade 候補と passthrough 候補の棚卸し
- `interface/ui`, `systems/{spatial,command,logistics,soul_ai,familiar_ai}`, `world/map`, `entities/{damned_soul,familiar}` の public API 整理
- 並列 re-export / 多段 re-export / wildcard re-export の削減方針策定
- 実装後に必要になる `docs/architecture.md`, `docs/cargo_workspace.md`, 関連 README の更新範囲整理

### 非対象（Out of Scope）

- `hw_*` crate 間の責務移動そのもの
- gameplay 挙動や Bevy schedule の変更
- crate 名や主要ドメイン型の rename
- 外部公開 API の完全廃止を一度に行う大規模 breaking change

## 3. 現状とギャップ

### 主な配置パターン

1. facade 集約:
   - `src/interface/ui/mod.rs`
   - `src/systems/spatial/mod.rs`
   - `src/systems/command/mod.rs`
   - `src/systems/logistics/mod.rs`
   - `src/entities/familiar/mod.rs`
2. leaf passthrough:
   - `src/interface/ui/panels/info_panel/mod.rs`
   - `src/world/map/access.rs`
   - `src/world/map/layout.rs`
   - `src/systems/familiar_ai/perceive/state_detection.rs`
   - `src/systems/soul_ai/execute/gathering_apply.rs`
3. facade 配下の重複再公開:
   - `src/systems/command/mod.rs` と `src/systems/command/area_selection/geometry.rs` の `wall_line_area`
   - `src/interface/ui/mod.rs` と `src/interface/ui/panels/mod.rs` と `src/interface/ui/panels/info_panel/mod.rs`
   - `src/systems/soul_ai/execute/task_execution/context/{access,queries,mod.rs}`
4. wildcard 再公開:
   - `src/systems/logistics/mod.rs`
   - `src/interface/ui/mod.rs`
   - `src/entities/familiar/mod.rs` の `components::*`
   - `src/systems/jobs/mod.rs`

### 明らかな多段チェーン

| シンボル例 | 現状の公開経路 | 問題 |
| --- | --- | --- |
| `InfoPanelPinState` | `hw_ui::panels::info_panel` -> `interface/ui/panels/info_panel/mod.rs` -> `interface/ui/panels/mod.rs` -> `interface/ui/mod.rs` | 同一型に 3 段の root 経由 alias がある |
| `EntityListNodeIndex` | `hw_ui::list` -> `interface/ui/list/mod.rs` -> `interface/ui/mod.rs` | `ui` facade と `ui::list` facade の両方が入口になる |
| `TransportRequest` | `hw_logistics::transport_request::components` -> `systems/logistics/transport_request/mod.rs` -> 呼び出し側 | transport_request 集約自体は妥当だが `components::*` が広すぎる |
| `TaskAssignmentQueries` | `hw_ai::...::context::queries` -> `context/queries.rs` -> `context/mod.rs` -> 呼び出し側 | `queries.rs` / `access.rs` が mirror export になっている |
| `WorldMapRead` | `hw_world` -> `world/map/access.rs` -> `world/map/mod.rs` | `access.rs` の存在意義が薄く、入口が二重化している |
| `wall_line_area` | `hw_core::area` -> `command/area_selection/geometry.rs` と `command/mod.rs` | 正規入口が二つある |

### 残すべき集約ポイントの候補

- `crate::plugins`
  - plugin 型の app-shell 入口として妥当
- `crate::entities::damned_soul` / `crate::entities::familiar`
  - app 側が直接使う ECS 型・plugin・spawn API の入口として維持価値がある
- `crate::systems::spatial`
  - grid resource / update system の単一入口として維持価値が高い
- `crate::systems::logistics::transport_request`
  - request domain の公開面はここに寄せる価値がある
- `crate::systems::command`
  - UI / input / mode 系が依存するため facade 維持。ただし helper の二重 export は減らす
- `crate::world::map`
  - world map 読み書きと座標 helper の入口として維持

### ギャップ

- facade と leaf passthrough の基準が未定義
- wildcard export が多く、公開シンボル境界が読みにくい
- 既存 import が facade 経由か leaf 経由か統一されていない
- 実コードでも混在が顕在化している
  - `src/plugins/spatial.rs` / `src/plugins/startup/mod.rs` は `hw_spatial::*` と `crate::systems::spatial::*` を併用している
  - `src/interface/ui/plugins/*.rs` は `crate::interface::ui::*` facade を使う箇所と `hw_ui::*` 直参照が混在している
  - `src/systems/familiar_ai/*` は `crate::systems::soul_ai::*` 互換 path と `hw_ai::*` 直参照が同居している

## 4. 実装方針（高レベル）

- 方針:
  - 「正規 public path を 1 つ決め、他は段階的に compat shim 化して削除する」
  - leaf module は原則として型定義・実装・adapter を持つ場合だけ public re-export を残す
  - facade に残す re-export は「複数呼び出し側が共有する app shell 入口」に限定する
  - wildcard export は原則廃止し、必要な公開シンボルを明示列挙する
- 設計上の前提:
  - `bevy_app` は app shell であり、一定の facade は残してよい
  - ただし facade は 1 ドメイン 1 箇所に寄せ、`mod.rs` と leaf module の二重公開を避ける
  - `hw_*` crate の owned type を root から再公開する場合も、公開場所は最小限に絞る
- 期待される効果:
  - runtime 性能影響は基本的にない
  - compile error の発生箇所と依存方向が読みやすくなり、crate 抽出・型移動の追従コストを下げられる
  - docs 上で「どこを import すべきか」を明確化できる
- Bevy 0.18 API での注意点:
  - API 変更は伴わないが、plugin 登録や `register_type::<T>()` が暗黙 import に依存している箇所は壊しやすい
  - `Component` / `Resource` / `Message` の参照先を変える際は path 解決のみに留め、所有クレートは変えない

## 5. マイルストーン

## M1: 公開面の分類表を作る

- 変更内容:
  - `pub use` を `Keep Facade` / `Collapse To Facade` / `Drop Passthrough` / `Replace Wildcard` に分類する
  - 正規入口をドメイン単位で決定する
  - 互換 shim が一時的に必要な箇所を洗い出す
- 変更ファイル:
  - `src/interface/ui/mod.rs`
  - `src/interface/ui/list/mod.rs`
  - `src/interface/ui/panels/mod.rs`
  - `src/systems/command/mod.rs`
  - `src/systems/spatial/mod.rs`
  - `src/systems/logistics/mod.rs`
  - `src/systems/logistics/transport_request/mod.rs`
  - `src/world/map/mod.rs`
  - `src/entities/damned_soul/mod.rs`
  - `src/entities/familiar/mod.rs`
- 完了条件:
  - [ ] ドメインごとの正規 public path が決まっている
  - [ ] 削減対象の leaf passthrough が一覧化されている
  - [ ] 並列 export の削除順序が決まっている
- 検証:
  - `rg -n --glob '*.rs' '^\\s*pub use ' src`
  - `cargo check --workspace`

## M2: leaf passthrough と並列 export を潰す

- 変更内容:
  - facade のみを公開入口にし、leaf 側の mirror export を削除または `pub(crate)` へ縮小する
  - helper の二重公開を 1 経路へ統一する
  - import を正規入口へ寄せる
- 変更ファイル:
  - `src/interface/ui/panels/info_panel/mod.rs`
  - `src/interface/ui/panels/info_panel/state.rs`
  - `src/interface/ui/panels/info_panel/layout.rs`
  - `src/interface/ui/panels/info_panel/update.rs`
  - `src/interface/ui/list/dirty.rs`
  - `src/interface/ui/list/selection_focus.rs`
  - `src/interface/ui/list/interaction.rs`
  - `src/interface/ui/list/interaction/visual.rs`
  - `src/world/map/access.rs`
  - `src/world/map/layout.rs`
  - `src/systems/soul_ai/execute/mod.rs`
  - `src/systems/soul_ai/execute/task_execution/context/access.rs`
  - `src/systems/soul_ai/execute/task_execution/context/queries.rs`
  - `src/systems/familiar_ai/perceive/state_detection.rs`
  - `src/systems/command/area_selection/geometry.rs`
- 完了条件:
  - [ ] `InfoPanelPinState`, `TaskExecutionContext`, `WorldMapRead`, `wall_line_area` などの正規入口が 1 つに統一されている
  - [ ] leaf module 直 import が必要最小限になっている
  - [ ] compat shim を残す場合は削除予定先がコメントまたは計画に明記されている
- 検証:
  - `rg -n 'crate::world::map::access::|crate::systems::command::area_selection::wall_line_area|crate::interface::ui::panels::info_panel::' src`
  - `cargo check --workspace`

## M3: wildcard re-export を縮小し facade を明示化する

- 変更内容:
  - `pub use ...::*` を必要シンボルの列挙へ置き換える
  - facade の責務外シンボルを submodule 参照に戻す
  - import 側も `crate::systems::logistics::{...}` の過剰集約を見直す
- 変更ファイル:
  - `src/interface/ui/mod.rs`
  - `src/interface/ui/list/mod.rs`
  - `src/systems/logistics/mod.rs`
  - `src/systems/logistics/transport_request/mod.rs`
  - `src/systems/jobs/mod.rs`
  - `src/entities/familiar/mod.rs`
  - `src/entities/familiar/components.rs`
  - `src/systems/soul_ai/decide/idle_behavior/mod.rs`
  - `src/systems/familiar_ai/decide/task_management/mod.rs`
  - `src/world/pathfinding.rs`
  - `src/world/river.rs`
- 完了条件:
  - [ ] facade から見える公開シンボルが明示列挙されている
  - [ ] wildcard 依存の import 崩れが `cargo check` で解消されている
  - [ ] 主要ドメインの公開境界を docs へ反映できる状態になっている
- 検証:
  - `rg --glob '*.rs' '^\\s*pub use [^;]*\\*' src`
  - `cargo check --workspace`

## M4: docs 同期と暫定互換層の整理

- 変更内容:
  - facade 方針を `docs/architecture.md` と `docs/cargo_workspace.md` に反映する
  - root shell に残す re-export の基準を文章化する
  - 一時 compat shim を残した場合は削除条件を TODO ではなく docs に記録する
- 変更ファイル:
  - `docs/architecture.md`
  - `docs/cargo_workspace.md`
  - `docs/README.md`
  - `src/interface/ui/README.md` または関連 README
  - `src/systems/soul_ai/README.md`
- 完了条件:
  - [ ] どの facade を残すか docs に書かれている
  - [ ] root shell の re-export 方針が他の crate extraction 計画と矛盾しない
  - [ ] 実装者が import path を推測せず選べる
- 検証:
  - `cargo check --workspace`
  - `python scripts/update_docs_index.py`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| facade を潰しすぎて app shell の利点まで失う | import path が逆に散らばる | `Keep Facade` の基準を先に決め、 `plugins`, `entities`, `spatial`, `transport_request`, `world::map` は原則維持する |
| wildcard 依存で見えていたシンボルを落とす | `cargo check` で広範囲に import error が出る | M3 で一括変更せず、M2 で正規入口へ import を寄せてから wildcard を縮小する |
| crate extraction 計画と逆行する path 整理をしてしまう | 今後の移設時に再度 import churn が出る | `docs/cargo_workspace.md` の root shell 方針に合わせ、 facade の残留理由を明記する |
| leaf module を private 化できず `pub(crate)` にも落とせないケースがある | 計画が途中で止まる | compat shim を短期的に残し、削除対象を M4 で明文化する |
| docs が import 実態に追従しない | 次回以降の実装で再び中継が増える | 実装完了時に architecture / workspace docs を同時更新する |

## 7. 検証計画

- 必須:
  - `cargo check --workspace`
  - `rg -n --glob '*.rs' '^\\s*pub use ' src`
  - `rg --glob '*.rs' '^\\s*pub use [^;]*\\*' src`
- 手動確認シナリオ:
  - `plugins/logic.rs`, `plugins/spatial.rs`, `plugins/visual.rs`, `plugins/startup/mod.rs` の import が新しい正規入口だけで読めることを確認する
  - `interface/ui/interaction/intent_handler.rs` と `familiar_ai/decide/*` のような多依存ファイルで import path が短く一貫していることを確認する
- パフォーマンス確認（必要時）:
  - 不要。runtime 挙動は変えない前提

## 8. ロールバック方針

- どの単位で戻せるか:
  - M2 と M3 を別コミットに分け、 facade 決定と wildcard 縮小を分離して戻せるようにする
- 戻す時の手順:
  - public path 変更だけを revert し、所有クレートや system 登録には触れない
  - compat shim を消しすぎた場合は、まず `mod.rs` 側だけ一時復旧して `cargo check` を通す

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン:
  - M1: 公開面の分類（実装中に並行実施）
  - M2: leaf passthrough と並列 export の削除
  - M3: wildcard re-export の明示化
  - M4: docs 同期
- 未着手/進行中:
  - なし

### 次のAIが最初にやること

1. `src/interface/ui/mod.rs`, `src/systems/logistics/mod.rs`, `src/systems/spatial/mod.rs` を開いて facade と passthrough を分類する
2. `InfoPanelPinState`, `WorldMapRead`, `TaskAssignmentQueries`, `wall_line_area` の正規入口を決める
3. M2 では leaf passthrough を削る前に、呼び出し側 import を facade へ寄せる

### ブロッカー/注意点

- 既存の crate extraction 計画が進行中なので、「全部 private にする」方向は危険
- `transport_request` と `spatial` は facade を残したほうが plugin wiring が読みやすい
- `interface/ui` は `hw_ui` crate の shell でもあるため、 top-level `ui` facade を消すより公開シンボルを明示化するほうが安全
- `src/systems/spatial` には one-line shell だけでなく `Blueprint` のような root 型を generic system へ流す wrapper もあるため、一括削除ではなく file ごとの分類が必要

### 参照必須ファイル

- `docs/DEVELOPMENT.md`
- `docs/cargo_workspace.md`
- `src/interface/ui/mod.rs`
- `src/systems/logistics/mod.rs`
- `src/systems/spatial/mod.rs`
- `src/systems/command/mod.rs`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-12` / `pass`
- 未解決エラー:
  - `N/A`

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `AI (Codex)` | 初版作成 |
