# Mixer猫車搬入の隣接到着修正

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `mixer-wheelbarrow-arrival-plan-2026-11-10` |
| ステータス | `In Progress` |
| 作成日 / 最終更新日 | `2026-11-10` |
| 作成者 | Codex |
| 関連提案 | N/A |
| 関連Issue/PR | TAK-14 非Bridge通常供給 |

## 1. 目的

Supply v7で自然採取されたSand5が猫車に積載され、Mixer隣接へ帰還しても搬入しない。
同じrequestが複数workerへ再割当され、到着付近で解除・再取得が繰り返された。180秒で精製未達、invalidを維持する。
隣接経路の実終点と到着判定を一致させ、数量・疲労・予約・地形・待機上限を変えず搬入を復旧する。

## 2. スコープ

- 対象: 猫車のMixer目的地phaseだけ、実handler回帰、関連恒久仕様とfresh実機。
- 非対象: 他目的地の到着規則、pathfinding全体、数量、AI採否/疲労、アート/正式release承認。
- mainが編集・build/test・Gitを所有し、独立read-only reviewerがexact差分を監査する。

## 3. 現状とギャップ

Mixerは2×2の非歩行footprintを持つ。`navigate_to_pos`は隣接pathを選べるが中心距離だけで到着判定する。
中心がセル境界にあるため斜め終点は中心から1.8tileを越えうる。
手運びのMixer搬入はFound後のtarget-or-destination判定を既に使っている。
v7の動作原本はobserverの490samples/254eventsで上限未達、観測failureなし。

## 4. 実装方針

既存TaskExecutionContext fixtureで2×2Mixerの合法な斜め終点を再現し、修正前失敗を確認する。
Mixer branchに限りFound後に実終点も判定し、Deferred/Unreachable/消滅の既存cleanupを保持する。
Bevy 0.19の既存helper/fixtureを利用し、新APIやcrate依存は追加しない。
同目的branch `codex/tak14-non-bridge-release` を再利用、比較基点 `375158372ca1aadbcb6ba85f1d9370c68b3de743`。
公開・push/mergeは本補正で行わない。並行差分は保全する。

## 5. マイルストーン

### M1: 再現と修正

- [x] 実handlerで修正前失敗→修正後Unloading→実Sand搬入を確認。
- [x] Moving/Deferred/Unreachable/target消滅、数量・完了通知・予約cleanupを回帰。
- [x] 変更対象はMixer branchだけ。他目的地/共通helperを一括変更しない。

### M2: 品質・実機

- [ ] dev check、profiling Clippy警告0、変更別CI、rust-analyzer、独立review。
- [ ] Help実経路判断、primary storage確認。
- [ ] 同candidate/自然worldのfresh通常供給。失敗はinvalidのまま診断する。

v8では実Sand5搬入と初回RefineCommit（泥5生成）まで到達したが、追加操作を含む17回のpointer motionが
旧汎用16回上限に抵触しinvalid。取水も満水BucketWaterの正常な直接再利用でTankTake未実行だった。
次のv3シナリオでは、通常Super操作でTank Water50・専用5BucketEmptyのown storage返却を待ち、Paused後も
同一rawで維持したことを確認してからMixerを建設する。Inventory全体の受動観測で非所持も確認する。
専用contractにsetupとmotion_cap=20（宣言19移動＋任意Dev最小化1）を固定し、汎用16は維持。
6画像、1200samples/4096events/64MiB、工程180秒/全体480秒、初回精製→TankTake証明を緩めない。

### M3: 終了

- [ ] 恒久仕様へ限定結果/残条件を同期し、計画撤去またはarchiveと索引更新。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| stale destinationで遠隔搬入 | Found後だけの判定、Moving/Deferred/Unreachable回帰 |
| 中断を成功へ変更 | cancellation/完了通知/予約の実handler回帰 |
| 一供給legを全受入へ拡張解釈 | raw/画像/独立verifyの限定結果と未検証範囲を併記 |

## 7. 検証計画・データ管理

primary `dev.py cargo`、`dev.py check`、`dev.py ci check --base 375158372ca1aadbcb6ba85f1d9370c68b3de743 --mode auto`。
native Skillとprimary validation coordinatorを使用し、plan→sealまでsource凍結。
v7は200327168bytes、owner TAK-14、consumer `M2 ordinary wheelbarrow destination cancellation diagnosis`。
修正原本との比較後consumer解除・不要job整理・finalize/check。現行candidate/通常Cargo cacheは維持する。
正本は `docs/tasks.md`、`docs/soul_ai.md`、`docs/building-asset-sets.md`。親track未完を保持理由にしない。

## 8. ロールバック

本修正単位で戻す。未確認差分を破棄せず、稼働/凍結subjectは変更しない。

## 9. AI引継ぎ

実handlerで修正前失敗→修正後成功、4拒否/成功回帰とprofiling crate138tests、dev check、独立reviewは成功。
旧Orca統括は再開しない。旧invalidは成功へ読み替えない。次は残りの品質検証→fresh実機。

### Definition of Done

- [ ] M1〜M3完了、same-subject検証/Help/native/独立review/storage確認。
- [ ] 採用コード/結果を正本へ集約し、診断consumerと不要出力を整理。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-11-10 | Codex | v7実観測に基づく初版 |
