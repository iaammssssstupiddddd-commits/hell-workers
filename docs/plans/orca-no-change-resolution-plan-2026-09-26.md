# Orca ソース変更なし調査の終了経路

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `orca-no-change-resolution-plan-2026-09-26` |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-09-26 / 2026-09-26 |
| 作成者 | Codex |
| 関連提案 | N/A |
| 関連Issue/PR | TAK-14、PRなし |

## 1. 目的

成功した調査がソース変更不要と結論した場合も、空commitを作らず、根拠・固定review・終了照合を経て同じ案件の次工程へ進める。

## 2. スコープ

対象はhost checkpoint、review loop、統合receipt、表示文言、統括案内と回帰テスト。
ゲームソース変更、ゲーム再検証、push、案件削除は対象外。既存の調査結果を再利用する。
追補: 利用者が別sessionの追加commitを維持した再検証・固定reviewを許可したため、現在HEADの
変更別検証を対象へ追加する。ゲームソースの編集・巻戻し・pushは引き続き行わない。

## 3. 現状とギャップ

TAK-14はworker成功・release・通知ACK・同一source検証まで完了したが、空checkpoint拒否で停止した。
空差分だけを成功理由にはできない。一方でソース変更の不要な解決を表すreceiptがなく、次工程のpreflightが通らない。
過去の失敗bootstrapも通常の完了ACKを持たないため、正の中断復旧証拠で区別した終了会計が必要。

## 4. 実装方針

- 空checkpointを専用の統括判断待ちにする。統括はexact loop digest、slot、理由、観測根拠を提出する。
- clean・HEAD/assignment不変・同一source検証成功・worker成功/release/ACKを必須とする。変更ありや結果不明は拒否する。
- no-change receiptに判断を束縛し、同じ固定reviewerで妥当性を確認する。差戻しは既存の限定再実装経路へ戻す。
- 全laneがno-changeならGitを更新しない統合receiptを発行し、課題側の検証と最終固定reviewを維持する。通常変更との混在でもreview条件を弱めない。
- 中断bootstrapはexact復旧receiptで照合し、存在しないworker_done/ACKを捏造しない。
- 同目的のparallel-developmentと配備側abcd-expansion branchを再利用。文書正本はprimary。公開しない。

## 5. マイルストーン

- [x] M1: no-change判断・receipt・固定review・差戻し・統合経路。
- [x] M2: 古い中断attemptの終了会計、通知排出との接続、表示と統括prompt、回帰テスト。
- [ ] M3: 変更別検証、TAK-14の正式終了、次工程preflightの確認と統括への引継ぎ。

## 6. リスクと対策

空差分を自動承認しない。理由だけで検証・reviewを省略しない。dirty/index変更、改変receipt、未処理通知、未確定releaseはfail closed。
unknown状態の手編集、履歴削除、空commitでの迂回は禁止する。

## 7. 検証計画

no-changeの成功/idempotence、dirty・失敗検証・異なるowner・根拠不足の拒否、固定review差戻し、no-op統合と混在、次工程への継承をテストする。
各変更repoで`python3 scripts/dev.py ci check --base <開始時HEAD> --mode auto`、primary storage check、Help実レビューを行う。
Rust/ゲームnative検証は非対象。保持中の候補/cacheを維持し、追加の巨大artifactは作らない。

## 8. ロールバック方針

既存receiptを保存し、unknown時は停止する。新receipt発行後は旧helperへ無条件に戻さない。Gitの巻戻しや台帳の削除は行わない。

## 9. AI引継ぎメモ

開始時HEAD: parallel `e1a1b83e`、配備側abcd `f50a0b63`、primary `561b7e19`。
変更の採用先は上記host helper。TAK-14の凍結worker checkoutへhelperをコピーしない。
完了条件は実案件の固定review・正の終了会計・次工程preflightまで。単体テストだけで完走としない。
最終結果・Help判断・検証fingerprintは完了時に仕様書へ集約する。

実受入: 変更なし調査の個別固定reviewまで承認。統合先に別sessionの`1826355f`と`48cbcab4`が
追加され、登録base `3d3cca84`と不一致になったため正しく停止した。利用者は新HEADの再検証・reviewを許可。
exact loop/HEAD/source/reasonを照合する明示reconciliation receiptで、元baseから新HEAD全体を
新しい検証・Help判断・最終reviewへ渡す。台帳改変やcommit巻戻しで迂回しない。
M3の実案件最終承認と次工程preflightは未完。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-26 | Codex | 初版。空checkpoint停止を正式な変更なし解決へ接続 |
