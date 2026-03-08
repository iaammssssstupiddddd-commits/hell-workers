# selection 分離提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `selection-separation-proposal-2026-03-08` |
| ステータス | `Draft` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI` |
| 関連計画 | `docs/plans/hw-ui-crate-plan-2026-03-08.md` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- 現状:
  - `hw_ui` 分離は進み、UI 本体は `hw_ui` に寄せられたが、`src/interface/selection/` は `selection` 系を root 側が全面担当している。
  - selection 系は座標変換・ワールド衝突判定・プレビュー描画・建設/移動配置の判定の大半が混在し、責務境界が `selection` 単位で閉じていない。
- 問題:
  - UI と WorldMap/配置ワークフローの依存が固定化しており、将来の分割（`selection` を crate 化、あるいは hw_ui で統一入力へ寄せる）に向けた移行コストが高い。
  - root 変更時の影響範囲が大きく、`selection` の手当・建設配置挙動を壊しやすい。
  - テスト観点で `selection` の決定ロジック（設置可否、プレビュー生成、意図解決）が `ui` と混ざっている。
- なぜ今やるか:
  - `selection` は `hw_ui` 分離後に残る最大依存層として残っており、ここを分離できれば保守性と保守テスト性が向上する。

## 2. 目的（Goals）

- `selection` の主要ロジックを「意図解決 + 判定ルール」「ゲーム状態反映」の 2 層に分離する。
- `selection` 系の world 依存を trait 化し、`hw_ui`（または新規 crate）側がテスト可能な形で配置計算を持てるようにする。
- root では `WorldMap` / camera / asset / context の副作用実行を担当し、UI 付随ロジックを薄くする。
- 既存挙動（建設・ゾーン・床/壁・移動配置）を壊さずに、段階的に移行できる形を確立する。

## 3. 非目的（Non-Goals）

- `camera.rs` の入力設計の全面再設計
- `selection` の新 UX 実装（見た目の変更）
- `soul_ai`・`tasks` 等のアルゴリズム変更
- `world` crate 自体を今回新規移植すること

## 4. 提案内容（概要）

- 一言要約:
  - `src/interface/selection/` を一度に壊さず、**「可視/入力/副作用」** と **「配置判定モデル」** を分離して `selection` を抽象化する。
- 主要な変更点:
  1. `hw_ui` 側（または中間 crate）で `selection` の中立データモデルを定義する。
  2. `SelectionPlacementApi` / `SelectionInputApi` / `SelectionTaskApi` の3系統 trait を定義し、WorldMap・カメラ・ゲーム状態変更を root が実装して注入する。
  3. `selection` の `Preview`/`Validation`/`Intent` の核心処理を model 化し、`root` は「WorldMap 参照→resource/event作成→適用」へ集約する。
  4. `src/interface/selection/*.rs` は wrapper/adapter 化を継続し、旧 API を維持しつつ移行の切れ目を作る。
- 期待される効果:
  - `selection` の主要ロジックを `hw_ui` でも再利用可能な形で記述できる。
  - world 依存を束縛し、`cargo check` 影響範囲を縮小しやすくする。
  - 将来 `selection` 単体テスト（意図生成/判定）を可能にする。

## 5. 詳細設計

### 5.1 仕様

- 振る舞い:
  - `selection` が要求する共通操作は全て "request" で表現される。
    - 例: `SelectionIntent::StartBuild`, `SelectionIntent::PlaceFloorPreview`, `SelectionIntent::CancelSelection`, `SelectionIntent::CommitPlacement` など。
  - 既存の `PlayMode / TaskMode / BuildContext / ZoneContext / MoveContext` は保持しつつ、`selection` 側は直接 mutation しない。
  - `selection` の実行順は維持する（`hover` -> `input` -> `preview` -> `commit` -> `post`）。
- 例外ケース:
  - `WorldMap` が未初期化やデータ不整合の状態で preview/commit が呼ばれた場合は、selection model は `Noop` or `InvalidPlacement` を返す。
  - プレイヤー状態遷移中に既存 `TaskMode` が変更された場合は、selection model 側の state を破棄し root が安全側へ戻す。
- 既存仕様との整合:
  - `src/interface/selection/building_move/*`, `building_place/*`, `floor_place/*` の既存 API 名（関数粒度）を当面維持。
  - door/建設/ゾーン編集の副作用は現状どおり root が実行。

### 5.2 変更対象（想定）

- `src/interface/selection/`:
  - `mod.rs` を facade 化し、adapter API を明示。
  - `input.rs`, `hit_test.rs`, `placement_common.rs`, `state.rs`, `mode.rs` の責務見直し。
  - `building_place`, `floor_place`, `building_move` は world/action trait 呼び出しへ切替。
- `crates/hw_ui/src/`（または新設 crate）:
  - `selection/` 新規モジュール（または既存 `selection` の拡張）を作成:
    - `selection/model.rs`（意図/状態）
    - `selection/placement.rs`（可能判定 + 仕様）
    - `selection/intent.rs`（intent enum/runner）
    - `selection/traits.rs`（抽象 API）
- ドキュメント:
  - `docs/architecture.md`, `docs/cargo_workspace.md` の選択依存説明を更新
  - `docs/proposals` に本提案を加えて継続する

### 5.3 データ/コンポーネント/API 変更

- 追加:
  - `hw_ui::selection::SelectionModel`
  - `hw_ui::selection::SelectionStateSnapshot`
  - `hw_ui::selection::SelectionIntent`（message / resource）
  - `hw_ui::selection::PlacementWorldApi`
  - `hw_ui::selection::InputWorldContext`
- 変更:
  - `src/interface/selection/state.rs` の公開 API（`HoveredEntity`, `SelectedEntity`, `SelectionIndicator`）は維持しつつ、内部では core model の型を参照。
  - `src/interface/selection/mod.rs` の再エクスポートは wrapper 中心に寄せる。
  - `building_place` / `floor_place` / `building_move` 系は `WorldMapWrite` 呼び出しを adapter 経由へ。
- 削除:
  - selection module 内の直接 `WorldMap::world_to_grid`/`grid_to_world` 利用点は段階的に撤去（最終的には trait 経由のみ）。
  - `selection` と UI の副作用混在を分離し、state mutation 系は root adapter へ移す。

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| 案A: `selection` をそのまま root 残留（現状維持） | 不採用 | 依存のままでは次フェーズの分割が停滞し、保守性向上が得られない |
| 案B: `selection` の判定系のみ `hw_ui`/新crateに移し、副作用は root 残留（本提案） | 採用 | 既存挙動を壊しにくく、段階移行が可能 |
| 案C: `selection` ごと新 crate 作成し、WorldMap も完全移植 | 保留 | `WorldMap`・camera・Input 系の依存が広く、移行コストが高い |

## 7. 影響範囲

- ゲーム挙動:
  - 同一入力なら既存挙動を維持（最終ゴールはビハインドが同一）。
- パフォーマンス:
  - 初期は抽象化レイヤー追加で小幅オーバーヘッド。
  - 2回目以降、配置系の再利用性・差分最適化が効き、保守性が向上。
- UI/UX:
  - 見た目や操作感に変更なしを目標。
- セーブ互換:
  - `Selection` 関連 resource の serialized state がある場合のみ、初期化順の検証が必要。
  - 既存に `SelectedEntity`/`HoveredEntity` の選択状態のみ（Entity ID）を保持する設計なので大きな互換問題は想定しにくい。
- 既存ドキュメント更新:
  - `docs/plans/hw-ui-crate-plan-...` の follow-up と本提案へのリンク更新
  - `docs/architecture.md`, `docs/cargo_workspace.md`, `docs/README.md` の選択方針追記

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| WorldMap 依存を trait 化しきれず設計が肥大化 | 複雑化しテスト価値が落ちる | まず `building_place`/`floor_place` の preview と validation のみから開始。move の副作用ロジックは最後に切替 |
| 既存モード遷移（Build/Zone/Task/Move）が壊れる | 操作不能・バグ報告 | 既存システム順と `Change` トリガを保ったまま段階実装。各フェーズで `TaskMode`/`PlayMode` 遷移シナリオを手動確認 |
| パフォーマンス劣化（毎フレーム model 化） | 配置操作時の重くなる感覚 | 既存の dirty 判定と cache を継続し、更新トリガを限定 |
| 参照漏れで root と model の二重状態になる | 選択状態の不整合 | `SelectedEntity` は single source-of-truth を root resource に固定、hw_ui 側は payload のみ扱い |

## 9. 検証計画

- `cargo check -p hw_ui`
- `CARGO_TARGET_DIR=target cargo check`
- `cargo test`（selection model 部分を追加する場合）
- 手動確認シナリオ:
  1. 建設配置: 壁/床/建物配置の開始→プレビュー→確定・キャンセル
  2. 建物移動: 移動対象選択→プレビュー→確定
  3. ゾーン編集: エリア選択中のドラッグ→確定
  4. selection overlay: Hover/指示 target のハイライトとキャンセル時復帰
  5. エンティティ選択との整合: `HoveredEntity`/`SelectedEntity` が壊れないこと
- 計測/ログ確認:
  - 主要配置経路の system 順序差分
  - `cargo check` 時の依存回帰

## 10. ロールアウト/ロールバック

- 導入手順:
  1. model 型と intent API を追加（表示/挙動は変更なし）
  2. building_place の preview/validation を adapter 経由に切替
  3. floor_place を同様に移行
  4. building_move preview/commit を最後に移行
  5. 既存 wrapper の整理と docs sync
- 段階導入の有無:
  - あり（ファイルごとに独立して移行可能）
- 問題発生時の戻し方:
  - 該当ファイル/モジュールを wrapper 化に戻し、trait 経由から直接実装へ一括 revert

## 11. 未解決事項（Open Questions）

- [ ] `selection` の抽象境界を `hw_ui` に入れるか、新規 `hw_selection` crate を作るか
- [ ] `MainCamera` 由来の cursor/world 変換をどこで正規化するか（root か middle layer か）
- [ ] `TaskArea`/`Zone` 系 context を intent 化する際の API 安定境界

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（提案段階）
- 直近で完了したこと: `hw_ui` 境界整理を反映した上で、selection follow-up 用提案を起点化
- 現在のブランチ/前提: `master` 系統

### 次のAIが最初にやること

1. `src/interface/selection/` の `WorldMap` 依存箇所を 3 つの分類（preview/validation/commit）に分解
2. まず `building_place` と `floor_place` の preview/validation から model 化して PoC を通す
3. `docs/cargo_workspace.md` / `docs/architecture.md` の selection 残留理由を更新

### ブロッカー/注意点

- `WorldMap` が root 専有 API のため、移行は抽象化なしでは進めにくい
- building_move は `unassign_task` 系の副作用が絡むため最終フェーズに回す

### 参照必須ファイル

- `docs/plans/hw-ui-crate-plan-2026-03-08.md`
- `src/interface/selection/mod.rs`
- `src/interface/selection/state.rs`
- `src/interface/selection/input.rs`
- `src/interface/selection/building_place/mod.rs`
- `src/interface/selection/floor_place/mod.rs`
- `src/interface/selection/building_move/mod.rs`
- `crates/hw_ui/src/selection`

### 完了条件（Definition of Done）

- [ ] 選択判定ロジックと副作用が明示的に分離され、依存経路が文書化されている
- [ ] `selection` 分離の実装方針とフェーズ計画が `docs/plans/...` に連動している
- [ ] `cargo check` が全工程で通る

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 提案書新規作成（selection 分離 follow-up） |
