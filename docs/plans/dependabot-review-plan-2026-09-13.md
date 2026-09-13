# Dependabot初回4件の採否レビュー

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `dependabot-review-plan-2026-09-13` |
| ステータス | `In Progress` |
| 作成日 / 最終更新日 | 2026-09-13 |
| 作成者 | Codex |
| 関連提案 | `docs/proposals/library-tooling-evaluation-proposal-2026-09-13.md` |
| 関連PR | #13、#14、#15、#16 |

## 1. 目的

初回Dependabot PRを実際の利用経路・一次ソース・検証から評価し、採用できる更新をマージする。
現行の契約を壊す更新は理由と再検討条件をPRへ記録してクローズする。
成功条件は4件すべての処置が確定し、採用後の品質ゲートが成功すること。

## 2. スコープ

- 対象: checkout、engine-render、worldgen、other-cargoの4件と採否記録。
- 非対象: Bevyの系列移行、WFCの置換、地形生成仕様変更、新しい機能。

## 3. 現状とギャップ

- 開始時master: `7e88ba341efeae0fc2de571299b60d6ff7237291`、作業ツリーはclean。
- #13 head: `2565843221e7bdd42d7979044013c35b7e9d2521`。
- #14 head: `6334332972507cb6bda7da5c5b27808dbbd2e0ee`。
- #15 head: `dc3a61f68f818e76fe28600e6037d4f78d1398a3`。
- #16 head: `e2eabba417f3957e311c09b19c22f2241d85c4b1`。
- #13–15の初回CIは古いPillow環境、#16は未レビューのHelp影響で停止している。
  依存更新自体の成功・失敗を示すものとは扱わない。

## 4. 実装方針

- upstreamの変更と現在のAPI境界を照合し、majorという理由だけで採否を決めない。
- 採用候補は最新masterを取り込み、更新されたheadで検証する。
- Helpは実際のproducer/consumerから判定し、Cargo変更を覆う子孫commitへ理由を記録する。
- Bevyのwgpuと直接依存のwgpu、WFCのrand/directionと直接依存の型の同一性を確認する。

## 5. マイルストーン

### M1: 4件の一次レビュー

- [ ] checkoutの修正内容・pin・workflow利用方法を確認。
- [ ] engine-renderの型境界を確認。
- [ ] worldgenの型境界・乱数互換性を確認。
- [ ] other-cargoのhash・serialization・library loading経路を確認。

### M2: 採否と検証

- [ ] 不採用のPRへ具体的理由と再検討条件を記録してclose。
- [ ] 採用候補に必要な修正・Helpレビューを反映。
- [ ] 採用候補の品質ゲートを確認してmerge。

### M3: 完了処理

- [ ] 採否と最終検証結果を既存の開発ドキュメントへ記録。
- [ ] 4件のremote状態とmasterへの反映を確認。
- [ ] 計画を終了し索引・storageを確認。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| グループ更新に非互換majorが含まれる | 実利用の型・API契約まで確認し、移行作業が必要なら今回のPRは閉じる |
| 古いCI失敗を依存の障害と誤認する | 現行masterを取り込んでから判定する |
| save/hash形式やnative診断の変更 | 既存の互換性テストとfeature checkを実施し、実機が必要な差分にはnative skillを使う |

## 7. 検証計画

- upstreamのmanifest/source/change logと全PR差分のレビュー。
- 採用するCargo変更: focused test、`dev.py check`、`dev.py verify`。
  full gateにworkspace test、profiling系check、Clippy `-D warnings`を含む。
- Actionのみ: pin照合、actionlint、正確なPR headのGitHub quality gate。
- `git diff --check`、Help影響レビュー、docs index、`dev.py validation check`。

### 検証データ管理

- 正本: `docs/development-infra/validation-storage-workflow.md`を開始時に再読。
- primaryの通常Cargo targetを再利用。新しいworktree・native job・binary copyは現時点で不要。
- 開始時storage checkはpass、既存2batchのallocated bytesは4,881,321,984。
- 既存consumerとキャッシュを維持し、この作業専用の生成物は必要時だけ登録・整理する。
- remote CIはGitHub管理。ローカルで全ログやbinaryの複製を保存しない。

## 8. ロールバック方針

採用前はPR単位で見送り可能。採用後に障害が確認された場合は該当mergeを対象とする
revert PRで戻す。履歴や他sessionの差分を破壊しない。

## 9. AI引継ぎメモ

- 現在地: 一次レビュー中。PR操作・採用変更は未実施。
- 次の作業: #13を現行masterへ更新、#14/#15の不一致を確定、#16の互換性確認。
- 必読: 本計画、DEVELOPMENT、Help skill、validation-storage-workflow。
- 最終確認: storage pass。依存変更後のlocal/remote gateは未実施。
- DoD: M1–M3完了、4件resolved、採用subjectのgate成功、不要な専用資源なし。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-13 | Codex | ユーザーのmerge/close依頼に基づき開始 |
