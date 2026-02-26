# Plans Index

`docs/plans` の文書ステータス一覧（更新日: 2026-02-25）。

## 新規計画書の作り方

1. テンプレートをコピーする。  
   `cp docs/plans/plan-template.md docs/plans/<topic>-plan-YYYY-MM-DD.md`
2. `メタ情報`、`目的`、`マイルストーン`、`AI引継ぎメモ` を最低限埋める。
3. 進捗に応じて `ステータス` と `更新履歴` を更新する。

## テンプレート

| Document | Status | Notes |
|---|---|---|
| [plan-template.md](plan-template.md) | テンプレート | AIが引継ぎしやすい実装計画テンプレート。 |

## 現行計画書

| Document | Status | Notes |
|---|---|---|
| [selection-placement-refactor-plan-2026-02-25.md](selection-placement-refactor-plan-2026-02-25.md) | Draft | `interface/selection` の配置処理を責務分離するリファクタ計画。 |
| [room-detection-plan-2026-02-23.md](room-detection-plan-2026-02-23.md) | Implemented | 壁・扉・床の閉領域をRoomとして検出し、オーバーレイ表示する実装計画。 |

## アーカイブ計画書一覧 (`docs/plans/archive`)

| Document | Status | Notes |
|---|---|---|
| `ai-phase-refactor-implementation-plan.md` | アーカイブ | AIフェーズリファクタ実装計画。 |
| `ai-phase-refactor.md` | アーカイブ | AIフェーズリファクタの全体設計メモ。 |
| `auto-gather-for-blueprint.md` | アーカイブ | Blueprint不足資材の自動伐採/採掘計画。 |
| `bridge-building.md` | アーカイブ | 橋（Bridge）建築物の実装計画。 |
| `bucket-return-rebuild-plan.md` | アーカイブ | バケツ返却仕様の再構築計画。 |
| `dream-bubble-physics.md` | アーカイブ | Dreamバブル物理挙動の実装計画。 |
| `dream-system.md` | アーカイブ | Dreamシステム提案。 |
| `dream-visual-update.md` | アーカイブ | Dreamビジュアル更新計画。 |
| `floor-construction.md` | アーカイブ | 床建築システムの実装計画。 |
| `global-transport-request-plan.md` | アーカイブ | 運搬系のグローバル request 化計画。 |
| `large-files-refactor-2026-02-16.md` | アーカイブ | 大規模ファイル分割リファクタ計画。 |
| `participating-in-relationship.md` | アーカイブ | Relationship参加設計に関する計画。 |
| `perf-top3-implementation-plan-2026-02-22.md` | アーカイブ | 直近Perf上位3件の最適化計画。 |
| `plant-trees-visuals-plan-2026-02-22.md` | アーカイブ | `Plant Trees` の3フェーズ演出およびドラッグ時プレビュー追加の実装計画。 |
| `refactor-roadmap-2026-02-22.md` | アーカイブ | 現行実装を前提にした全体リファクタ実行計画（回帰テスト追加はスコープ外）。 |
| `refactor-500plus-files-phase-plan-2026-02-14.md` | アーカイブ | 500行超ファイルの段階的リファクタ計画。 |
| `refactor-implementation-order-2026-02-20.md` | アーカイブ | リファクタ実装順のガイド。 |
| `refactor-phase-plan-2026-02.md` | アーカイブ | フェーズ分割リファクタ計画。 |
| `remove-instockpile-claimedby.md` | アーカイブ | `InStockpile`/`ClaimedBy` 削除統合計画。 |
| `request-unification-plan-2026-02-14.md` | アーカイブ | Request方式一本化計画。 |
| `rest-area-system.md` | アーカイブ | 休憩所（Rest Area）システム提案。 |
| `scaling-performance-bottlenecks-plan.md` | アーカイブ | スケール時ボトルネック最適化計画。 |
| `soul-spawn-despawn.md` | アーカイブ | Soul Spawn/Despawn設計計画。 |
| `task-list-left-panel-toggle.md` | アーカイブ | タスクリスト左パネル操作改善計画。 |
| `wall-construction-phase-split-plan-2026-02-19.md` | アーカイブ | 壁建築フェーズ分割計画。 |
| `wall-stasis-mud.md` | アーカイブ | 壁材（stasis mud）関連計画。 |
| `wheelbarrow-arbitration-plan.md` | アーカイブ | 猫車利用仲裁ロジックの実装計画。 |
