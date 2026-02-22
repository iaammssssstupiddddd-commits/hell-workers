# Plans Index

`docs/plans` の文書ステータス一覧（更新日: 2026-02-22）。

| Document | Status | Notes |
|---|---|---|
| `request-unification-plan-2026-02-14.md` | 提案 | 運搬AIを request 方式へ一本化する段階移行計画（manual Haul の request 化含む）。 |
| `bucket-return-rebuild-plan.md` | 提案 | バケツ置き場返却を要件起点で再実装する計画（request 1件 + 複数slots管理）。 |
| `ai-phase-refactor-implementation-plan.md` | 進行中 | M0〜M5 完了、M6（最終検証）を継続中。 |
| `global-transport-request-plan.md` | 進行中 | 運搬系を request アンカー方式へ統一するグローバル移行計画。 |
| `wheelbarrow-arbitration-plan.md` | 提案 | 猫車使用判断を request 集約後の仲裁フェーズへ移す段階導入案。 |
| `refactor-implementation-order-2026-02-20.md` | 提案 | 現行コードベース向けに、低リスクから進める段階的リファクタ実装順を定義。 |
| `auto-gather-for-blueprint.md` | 完了 | `familiar_ai` が `DeliverToBlueprint` request を起点に Wood/Rock の自動伐採・採掘Designationを段階探索で発行。 |
| `floor-construction.md` | 完了 | 床建築システム本体 + 2026-02-15 の運用修正（通行可能化・搬送安定化・配筋可視化）まで反映。 |
| `refactor-phase-plan-2026-02.md` | 提案 | TransportRequest/Task実行/Startup を対象にした段階的リファクタ計画（Phase 0〜5）。 |
| `p3_1_entity_list_diff_update_plan.md` | 完了 | 差分更新実装済み。履歴として保持。 |
| `stockpile-spatial-grid.md` | 一部完了 | グリッド導入済み。増分更新は未着手。 |
| `taskarea-and-incremental-update.md` | 未着手 | TaskArea逆引き・増分更新の構想。 |
| `bevy-best-practices-audit.md` | 監査記録 | 2026-02-04 時点のスナップショット。 |
| `task-area-ui-improvement-plan.md` | 未着手 | タスクエリアの高頻度変更に対応するUI改善計画。 |
| `large-files-refactor-2026-02-16.md` | 提案 | 450行以上の9ファイルを対象に、分割方針と周辺共通化（haul/speech/area selection/spawn/constants）を整理。 |
| `scaling-performance-bottlenecks-plan.md` | 提案 | `Soul 500 / Familiar 30` を基準に、P0〜P5 ボトルネックを段階導入で解消する実装計画。 |
| `wall-construction-phase-split-plan-2026-02-19.md` | 提案 | 壁建設を床同等のフェーズ分割へ再編する計画（養生なし・仮設利用維持）。 |
| `perf-top3-implementation-plan-2026-02-22.md` | 提案 | 直近レビューで特定した上位3件（壁材同期/Familiar委譲/Entity List UI）を優先順で最適化する計画。 |
