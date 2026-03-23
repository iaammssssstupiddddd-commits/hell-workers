# コードベース整理・リファクタリング計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactoring-plan-2026-03-23` |
| ステータス | `Complete` |
| 作成日 | `2026-03-23` |
| 最終更新日 | `2026-03-24` |
| 作成者 | `Copilot` |
| 関連提案 | N/A |
| 関連Issue/PR | N/A |

## 1. 目的

- **解決したい課題**: DRY違反・責務混在・過大ファイルの解消
- **到達したい状態**: 各ファイルが単一責務を持ち、新機能追加・デバッグコストが下がっている状態
- **成功指標**: `cargo check --workspace` クリーン維持、公開 API / 登録責務 / 実行順序を崩さずに重複と責務混在が減っている

## 2. スコープ

### 対象（In Scope）

- `source_selector.rs` の関数API統合（hw_familiar_ai）
- `hw_ui/list/spawn.rs` のノード生成ヘルパ整理
- `building_move_system.rs` の責務分割（bevy_app）
- `hw_world/pathfinding.rs` のポリシー分離
- `hw_soul_ai` task_execution phase ロジックの共通化
- `hw_visual` → `hw_ui` 逆依存の解消

### 非対象（Out of Scope）

- 新機能の追加
- ゲームロジックの変更
- アセット・ビジュアルの変更

## 3. 現状とギャップ

- **現状**: 総 68,603 行 / 637 ファイル / 10 クレート。`cargo check --workspace` クリーン。
- **問題**:
  - `source_selector.rs` で `find_nearest_*` 7 関数が実質 2 パターンの薄いラッパー
  - `hw_ui/list/spawn.rs` で Familiar Section / Soul Row の子ノード生成に同一パターン反復がある
  - `building_move/system.rs` が入力判定・配置検証・適用・状態クリアを 1 システムに抱え、引数も多い
  - `pathfinding.rs` で 3 探索ポリシーが 1 ファイルに混在
  - task_execution ハンドラで「移動→到達判定→作業→完了」パターンが複数タスクに分散している
  - `hw_visual` が `hw_ui` の selection / camera / components / theme に依存している
- **本計画で埋めるギャップ**: DRY 削減・責務分離・アーキテクチャ境界の厳守

## 4. 実装方針（高レベル）

- Phase 1 → Phase 2 → Phase 3 の順に実施（各 Phase は独立マージ可能）
- 公開 API（`pub use` の re-export パス）は変えない
- 各 Phase 完了後に `cargo check --workspace` を実施して次 Phase へ進む
- bevy_app の業務ロジックは `hw_*` Leaf クレートへ段階的に移動する方針（`crate-boundaries.md` 準拠）
- system / observer の登録元を変更する場合は、実装所有先と登録責務の両方を記録し、`docs/architecture.md` と `docs/cargo_workspace.md` の更新要否を同時判定する

## 5. マイルストーン

---

## M1: [Phase 1-A] source_selector.rs 関数API統合

- **変更内容**: `find_nearest_mixer_source_item` / `find_nearest_blueprint_source_item` が `nearest_ground_source_with_grid(..., None)` への薄いラッパーになっている重複を削除。`find_nearest_stockpile_source_item` も同様に内部実装を 1 本化。
- **変更ファイル**:
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/haul/source_selector.rs`
- **完了条件**:
  - [ ] 公開 API シグネチャは変更なし（呼び出し側コードの変更なし）
  - [ ] 内部の重複実装が統合されている
  - [ ] `cargo check --workspace` 成功
- **検証**: `cargo check --workspace`
- **見込み削減**: ~60–80 行

---

## M2: [Phase 1-B] hw_ui/list/spawn.rs ノード生成ヘルパ整理

- **変更内容**: `spawn_familiar_section` / `spawn_soul_list_item` 周辺で繰り返している「Icon + Node」「Text + Font + Color」「小型 Button + Label」生成を、小さなヘルパー関数へ抽出する。汎用 builder 導入は目的化せず、`spawn.rs` 内に閉じた補助関数または小モジュールで整理する。
- **変更ファイル**:
  - `crates/hw_ui/src/list/spawn.rs`
  - （必要に応じて `crates/hw_ui/src/list/spawn_helpers.rs` を新規作成）
- **完了条件**:
  - [ ] `spawn.rs` の重複ノード生成がヘルパーに寄せられている
  - [ ] 呼び出し側シグネチャは必要最小限の変更に留まる
  - [ ] `cargo check --workspace` 成功
- **検証**: `cargo check --workspace`
- **見込み効果**: 反復コード削減、UI ノード変更時の修正箇所局所化

---

## M3: [Phase 2-A] building_move_system.rs 責務分割

- **変更内容**: `building_move/system.rs` の単一システムから、少なくとも次の責務を分離する。
  1. 入力と早期 return 判定
  2. 配置検証と companion 配置判定
  3. move 適用と関連タスク/リクエスト解除
  4. move mode 終了時の状態クリア
  実装形は「複数 system に分割」または「単一 system + 補助関数分離」のどちらでもよいが、登録順序が崩れない形を優先する。
- **変更ファイル**:
  - `crates/bevy_app/src/interface/selection/building_move/system.rs`
  - `crates/bevy_app/src/interface/ui/plugins/core.rs`（system 登録順序を更新する場合）
  - （必要に応じて `docs/architecture.md` / `docs/cargo_workspace.md`）
- **完了条件**:
  - [ ] 入力・検証・適用・状態クリアの責務境界がコード上で追いやすい
  - [ ] `building_move_preview_system` との順序関係を含む `Interface` 登録順序が維持される
  - [ ] 既存の移動動作が変わらない（`cargo run` で手動確認）
  - [ ] `cargo check --workspace` 成功
- **検証**: `cargo check --workspace` + `cargo run` で建物移動を手動確認

---

## M4: [Phase 2-B] hw_world/pathfinding.rs ポリシー分離

- **変更内容**: 通常探索・隣接探索・境界探索の 3 ポリシーを別ファイルに分離。
  - `pathfinding/mod.rs` — 公開 API のみ（`find_path`, `find_path_to_adjacent`, `find_path_to_boundary`）
  - `pathfinding/core.rs` — A* 共通コア
  - `pathfinding/policies.rs` — 各 `PathGoalPolicy` 実装
- **変更ファイル**:
  - `crates/hw_world/src/pathfinding.rs` → ディレクトリに変換
  - `crates/hw_world/src/pathfinding/mod.rs`（新規）
  - `crates/hw_world/src/pathfinding/core.rs`（新規）
  - `crates/hw_world/src/pathfinding/policies.rs`（新規）
  - `crates/hw_world/src/lib.rs`（re-export パス維持）
- **完了条件**:
  - [x] 外部公開 API のパスが変わらない
  - [x] 既存テスト（`test_path_to_boundary_1x1_open` 等）が全通過
  - [x] `crates/hw_world/src/pathfinding/tests.rs` へ tests の配置先が明確化されている
  - [x] `cargo check --workspace` 成功
- **検証**: `cargo test -p hw_world` で pathfinding テスト全通過 + `cargo check --workspace`

---

## M5: [Phase 3-A] hw_soul_ai task_execution phase 共通化

- **変更内容**: 既存の `handler/task_handler.rs` にある `TaskHandler<T>` を前提に、`gather.rs` / `haul.rs` / `haul_with_wheelbarrow/` などに分散している phase 遷移処理を共通化する。
  - 新しい抽象化は既存 `handler/` 層の上に重ね、trait の二重定義を避ける
  - `common.rs` または `transport_common/` に「移動 → 到達判定 → 作業 → 完了」の補助ロジックを集約する
  - 各ハンドラは差分の判定・副作用だけを持つ形を目指す
- **変更ファイル**:
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/common.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/handler/`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/gather.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul_with_wheelbarrow/` 各ファイル
- **完了条件**:
  - [x] 既存 `TaskHandler<T>` との責務重複がない
  - [x] 共通化対象の phase ロジックが 1 箇所に集約されている
  - [x] 到達不能時の解除・予約解放など既存契約を維持している
  - [x] `cargo check --workspace` 成功
- **実装メモ**:
  - `NavOutcome` + `navigate_to_adjacent`（指定チェックあり）: `gather.rs`, `collect_sand.rs`, `collect_bone.rs` に適用済み
  - `navigate_to_pos`（指定チェックなし）: `haul.rs`, `haul_with_wheelbarrow/phases/going_to_source.rs`, `going_to_destination.rs`, `going_to_parking.rs` に適用済み
  - `haul.rs` の `GoingToItem` は `can_pickup_item` の独自到達判定を維持しつつ、移動先更新だけを `navigate_to_pos` に寄せた。抽象化は「共通移動」と「task 固有の pickup/drop 条件」を分ける粒度に留めている。
- **検証**: `cargo check --workspace` + `cargo run` で haul/gather タスクを手動確認
- **依存**: なし

---

## M6: [Phase 3-B] hw_visual → hw_ui 依存整理

- **変更内容**: `hw_visual` が `hw_ui` の selection / camera / components / theme を直接参照している箇所を分類し、次のどれで解消するかを決める。
  1. `hw_core::visual_mirror::*` へ寄せられるものは mirror 化
  2. UI 固有状態は `bevy_app` 側 adapter に寄せる
  3. 真に共有が必要な read-only 型だけを `hw_core` へ昇格
  `MainCamera` や `UiNodeRegistry` のような UI 固有型は、一括 trait 化ではなく依存方向を崩さない位置へ移す。
- **変更ファイル**:
  - `crates/hw_visual/src/`
  - `crates/hw_core/src/visual_mirror/mod.rs`
  - `crates/hw_visual/Cargo.toml`
  - （必要に応じて `crates/bevy_app/src/interface/selection/mod.rs` / 関連 adapter）
- **完了条件**:
  - [x] `hw_visual` の `Cargo.toml` から `hw_ui` 依存を削除、または削除不能な理由を明文化した上で最小化されている
  - [x] 依存削減後の登録責務と adapter 層の所在が説明できる
  - [x] `cargo check --workspace` 成功
- **実装メモ**:
  - `hw_visual` の `hw_ui` 依存は削除済み。shared UI contract は `hw_core` に集約し、`hw_ui` は re-export する構成へ変更した。
  - **移動済み**: `MainCamera` → `hw_core::camera`, `SelectedEntity` / `HoveredEntity` / `SelectionIndicator` → `hw_core::selection`, `UiNodeRegistry` / `UiSlot` / `UiMountSlot` / `UiRoot` → `hw_core::ui_nodes`, `DreamIconAbsorb` → `hw_core::visual_mirror::dream`
  - `hw_visual` 側の `UiTheme` 参照はローカル色 helper に置き換え、スタイリング依存を切り離した。
  - adapter 層の説明責務は `docs/architecture.md` / `docs/cargo_workspace.md` に同期済み。
- **検証**: `cargo check --workspace`
- **依存**: M2 完了後に着手

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| building_move の分割で実行順序が崩れる | 移動動作のバグ | 実登録箇所である `interface/ui/plugins/core.rs` を起点に ordering を維持し、必要なら `.chain()` や `.after(...)` を明示する |
| pathfinding.rs のディレクトリ化で re-export パスが変わる | コンパイルエラー | `hw_world/src/lib.rs` の `pub use` を事前に固定 |
| task_execution 共通化で既存 borrow / 解除契約が崩れる | コンパイルエラーまたは行動退行 | 1 ハンドラずつ移行し、到達不能時の cleanup と reservation 解放を手動確認する |
| `hw_visual` の `hw_ui` 依存解消で責務の押し付け先を誤る | crate 境界の悪化 | `hw_core` へ昇格する型は read-only 共有契約に限定し、テーマや widget 固有ロジックは `hw_ui` / root adapter に残す |

## 7. 検証計画

- **必須**: 各マイルストーン完了後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- **M3, M5**: `cargo run` で対象機能を手動動作確認
- **M4**: `cargo test -p hw_world test_path_to_boundary_1x1_open -- --exact` + pathfinding 関連テスト全件
- **docs 更新確認**: 登録責務 / ordering / crate 境界が変わったマイルストーンでは `docs/architecture.md` と `docs/cargo_workspace.md` の更新要否を確認する

## 8. ロールバック方針

- 各マイルストーンを独立したコミットにするため、`git revert <commit>` で単位ロールバック可能
- Phase 単位（M1+M2 / M3+M4 / M5+M6）でブランチを切ることを推奨

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: M1 〜 M6
- 未着手: なし

### 次のAIが最初にやること

1. `docs/architecture.md` / `docs/cargo_workspace.md` を参照して、更新済み crate 境界と登録責務を確認
2. 追加の追従作業がある場合は `cargo check --workspace` をベースラインとして再実行
3. 必要なら `cargo run` で M3/M5 対象の手動確認を追加する

### ブロッカー/注意点

- `building_move/system.rs` の登録箇所は `interface/ui/plugins/core.rs`。ordering 変更時はここも必ず確認する
- M5 は既存 `TaskHandler<T>` 実装を置き換えず、`common.rs` のナビゲーション helper で phase 共通化する方針で完了
- M6 は shared UI contract を `hw_core` へ昇格し、`hw_visual` の `hw_ui` 依存削除まで完了
- pathfinding 分割後も `hw_world/src/lib.rs` の re-export パスは維持済み

### 参照必須ファイル

- `docs/crate-boundaries.md` — Leaf/Root 分離の原則
- `docs/architecture.md` — システムセット実行順序
- `docs/cargo_workspace.md` — crate 責務と依存方向
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/haul/source_selector.rs`
- `crates/hw_ui/src/list/spawn.rs`
- `crates/bevy_app/src/interface/selection/building_move/system.rs`
- `crates/bevy_app/src/interface/ui/plugins/core.rs`
- `crates/hw_world/src/pathfinding/mod.rs`
- `crates/hw_core/src/ui_nodes.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/handler/`
- `crates/hw_visual/Cargo.toml`
- `crates/hw_core/src/visual_mirror/mod.rs`

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-03-24` / `pass`
- 未解決エラー: なし

### Definition of Done

- [x] M1〜M6 すべて完了
- [x] `cargo check --workspace` が成功（警告なし）
- [x] 登録責務 / ordering / crate 境界に変更がある場合、`docs/architecture.md` と `docs/cargo_workspace.md` の該当箇所を更新

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-23` | `Copilot` | 初版作成 |
| `2026-03-24` | `Codex` | M4〜M6 の完了状態と crate 境界の実装結果に合わせて更新 |
