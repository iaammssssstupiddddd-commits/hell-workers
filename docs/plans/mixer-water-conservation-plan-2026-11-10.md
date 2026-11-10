# Mixer給水の水量消失修正

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `mixer-water-conservation-plan-2026-11-10` |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-10 |
| 作成者 | Codex |
| 関連提案 | N/A |
| 関連Issue/PR | TAK-14実機で検出、利用者が別gameplay修正として明示許可 |

## 1. 目的

通常worldでTankTake5→空き2のMixerへ2生成→BucketEmptyとなり水3が消失した。
水入りbucketは5個分という既存表現を維持し、搬入・競合・取消・再取得で量を増減させない。

## 2. スコープ

対象はMixer給水の需要/割当、Tankからの全量取出し、投入時の容量再検査、同frameの取出し/投入競合、
実handler回帰、Helpと恒久仕様、通常worldのfresh実機。容量・bucket5・精製recipe・保存schemaは変更しない。
Bridge、全体release/アート採否、別の物流最適化は対象外。mainが全編集/検証/Gitを所有する。

## 3. 現状とギャップ

`missing_water.div_ceil(5)`が空き2でも運搬を要求し、resolverはTank水1から候補化する。
Fillingは1〜5をBoolean BucketWaterへ変換し、再取得では5へ再解釈され得る。
Pouringはmin量だけ生成して全量をEmpty化する。同frame deferred Commandsも別workerからは未反映。
従来の部分投入回帰を数量保存回帰へ変更し、旧v10のinvalidは維持する。

## 4. 実装方針

満量5の空きに限って需要/割当を成立させる。確定予約と同cycle shadowを5単位で控除する。
空bucketはTank水5があるときだけ取出し、経路Deferred/Unreachable/不足では消費しない。
投入時に全量分の空きを再検査し、競合時は水入りbucketを保ったretryable cleanupでworkerを解放する。
部分空きの前でworkerを抱え続けて精製を阻害しない。既存取消/auto-haulでfull bucketを再利用する。
system呼出し内だけの取出しID/投入量shadowでdeferred反映前の二重消費と溢れを拒否する。
Bevy 0.19の既存Query/Context/cleanupだけを利用する。
branchは同目的 `codex/tak14-non-bridge-release`、基点 `b29d7c143bdc346c52b620f7f0bc74135490b444`。
公開/push/mergeは本補正では行わない。共有・並行差分を破棄しない。

## 5. マイルストーン

### M1: 再現と数量修正

- [x] 空き0/2/5、Tank水0/1/4/5、満水bucket再取得、全量投入と拒否を実handlerで回帰。
- [ ] 後から容量増加、材料不足、取消/対象消失、予約cleanup、同frame複数投入/取出しを検証。
- [x] producer/resolverを全量単位へ揃え、workerを無期限に拘束しない。

### M2: 検証と文書

- [x] Help実影響判断と必要なprovider/coverage/snapshot、仕様/不変条件を同期。
- [x] dev check、Clippy警告0、変更別CI、rust-analyzer、exact独立read-only review。
- [ ] native Skill/primary validationによるfresh通常給水、厳密な全prefixとlossless leg検査。

### M3: 終了

- [ ] 採用結果を正本へ集約し、一時計画/診断用途を整理、索引とstorage checkを完了。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| 1workerが水を持ち続け精製できない | 部分空きでは割当しない。投入競合時はretryable cleanupで解放 |
| 同frameに2workerが同じ水/空きを使う | 呼出し内shadowと実handler複数actor回帰 |
| 少量入りbucketの再取得で水増加 | full5以外を生成しない。中断時もfull/emptyを保持 |
| 技術passを全体releaseへ拡張 | exact限定結果・全体未達を分離して記録 |

## 7. 検証・データ管理

`dev.py cargo`、`dev.py check`、変更別CI `--base b29d7c143bdc346c52b620f7f0bc74135490b444 --mode auto`。
native Skillと `dev.py validation` のplan/正規kitty/seal/finalize/checkを使い、subjectを凍結する。
v10原本181051392bytesはTAK-14所有、consumerは `M2 real partial Mixer pour quantity contract diagnosis`。
修正比較終了後にconsumerをreleaseして不要jobを整理する。候補/codec/通常Cargo cacheは継続用途を保つ。
結果正本は `docs/logistics.md`、`docs/invariants.md`、`docs/tasks.md`、`docs/building-asset-sets.md`。

実装commitは `8ba10ec6951164abeedbc1d77a0236fb1b0ec5dd`。コード/Help/文書の独立reviewと
全品質gateは通過。実handlerのDeferred維持、欠損/異種/非Mixer/部分amount拒否、予約解放、
次pass再取得、同Tank/同Mixer/同bucket異Mixerの競合を含む。
fresh v11（203984896bytes）は製品Mud10満杯でWater3のまま停止してinvalid、正式seal/finalize/storage pass。
旧5→2損失は起きなかったが、Tank由来の投入/精製は未到達なのでnative完了欄はチェックしない。
v11 consumerは `M2 ordinary output destination prerequisite comparison`。
Stockpile案はStasisMud保管禁止により実行前棄却し、試作コードも全撤去した。
正規の床/壁の需要を使う次campaignには、最低泥20（床20tileなら骨40）、実TaskArea、
採取/搬出猫車の到達・施工phaseを読取り照合してから通常入力だけで作る。未確認の成立を推測しない。

## 8. ロールバック

本修正単位。未確認の並行変更、凍結subject、旧invalidを上書きしない。

## 9. AI引継ぎ

利用者から水量不具合の修正を明示許可済み。追加の進行確認は不要。
旧Orca統括は再開しない。主agentが編集/検証/Git、既存独立review担当はread-onlyだけ。
未完の非Bridge全体受入や正式性能/production releaseを成功にしない。

### Definition of Done

- [ ] M1〜M3完了。同subject gate/Help/native/独立review/storageを確認。
- [ ] 成功・失敗・未検証範囲を正確に記録し、一時計画を撤去またはarchive。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-11-10 | Codex | 実5→2損失に対する初版 |
| 2026-11-10 | Codex | 数量保存修正・品質gate通過、v11出力満杯の未達と正規搬出前提を記録 |
