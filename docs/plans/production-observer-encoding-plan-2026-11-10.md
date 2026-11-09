# 通常world観測履歴の再encode除去

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `production-observer-encoding-plan-2026-11-10` |
| ステータス | `In Progress` |
| 作成日 / 最終更新日 | `2026-11-10` |
| 作成者 | Codex |
| 関連提案 | N/A |
| 関連Issue/PR | TAK-14 非Bridge通常供給受入 |

## 1. 目的

供給v4の512 samplesで全履歴JSON encodeが903873us、収集9476us、write33624usだった。
増加する過去履歴の再encodeを除去し、同じ観測内容・失敗契約で通常world試験を進める。
ゲーム速度・数量や試験上限を変更して成功を作らない。

## 2. スコープ

- 対象: profiling observerの私有append-only履歴、公開時JSON組立、拒否回帰、計測、一次文書。
- 非対象: gameplay、入力、地形、採取/配送規則、アート/数値承認、正式release、性能budget変更。
- Supply専用の一時solid診断は原因切分け終了に伴い撤去する。

## 3. 現状とギャップ

各sample/eventは追記後不変だが、公開ごとに全Valueをserializeしている。
唯一の過去event書換えはevent_index全件付与で、共通appendへ移せる。
全面bytesコピーと同期writeは残るため、総公開処理が線形化したとは主張しない。

## 4. 実装方針

- 私有row/encoded bytesを持つ履歴。変更可能参照やDerefMutを公開しない。
- append時にordinalを確定し、serde_jsonで一度だけencode。extendも同経路。
- 完成JSON全体の64MiB上限、1200samples/4096events、authority/failureは不変。
- encode・上限エラーはsticky。失敗や欠損を捨てて後続成功へ変更しない。
- 排他的temporary作成・同期write・renameは既存snapshot_ioをそのまま使う。
- 追加crate/APIなし。Bevy 0.19の既存Resource/query経路を維持。
- 同目的branch `codex/tak14-non-bridge-release`、基点 `f6270ae72cede7e16e2d64333ce1781bf37f5933`。
- 編集/検証/Gitはmain所有、独立read-only review。公開/mergeは本修正では行わない。

## 5. マイルストーン

### M1: 履歴・組立

- [x] 旧JSONとの完全同値、ordinal、追記後不変、再encode非発生を回帰。
- [x] exact byte境界・加算overflow・sticky失敗・非object/header衝突を拒否（累積append上限回帰を含む）。
- [x] 既存input即時公開・件数上限・atomic writer失敗回帰を保持。

### M2: 品質・実機

- [ ] check、profiling Clippy、変更別CI、rust-analyzer、独立review。
- [ ] Help実経路No impact、storage整理。
- [ ] fresh通常供給のencode実測とactual-window結果を記録。未達は未達のまま残す。

### M3: 正本・終了

- [ ] 永久仕様へ結果/制限を同期し、一時計画を削除またはarchive、索引を再生成。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| cacheと元rowの不一致 | owned append-only型、mutable APIなし、全producer共通append |
| 件数/bytes上限を黙って切捨て | sticky失敗と既存invalid処理、境界回帰 |
| serializer計測の付替え | append encode時間と組立時間を分離記録し、合計も表示 |
| 通常world合格への誤読 | 同一source fresh job、原本/画像/独立verify、限定結果のみ記載 |

## 7. 検証計画・データ管理

`dev.py cargo`、`dev.py check`、`dev.py ci check --base f6270ae72cede7e16e2d64333ce1781bf37f5933 --mode auto`。
native Skillとprimary validation coordinatorを使用し、planからsealまでsourceを凍結する。
同じprimary/target/candidate/codecを再利用。新実機rootは準備完了後fresh v5として登録する。
旧v4は204MB規模の診断比較用途、exact bytesはstorage台帳を正本とする。
比較終了後はconsumer解除・生成物整理・finalize/check。結果正本は `docs/building-asset-sets.md`。
親track未完を理由に全jobを保持しない。

## 8. ロールバック

本差分単位。並行変更を保全し、sourceを凍結したjob中は変更しない。旧invalidは上書きしない。

## 9. AI引継ぎ

実装済み、focused66tests成功。累積append上限回帰と計測重複排除も検証済み。
旧Orca統括は再開しない。v4はRock採掘/搬送・Sand帰路まで進行してinvalid。
次はappend-only履歴の実装→exact回帰→独立review→品質→fresh実機。新たな利用者許可待ちはない。
未検証の成功・アート/数値/正式releaseを捏造しない。

### Definition of Done

- [ ] 全milestone、Help/native/変更別検証/独立review/storage完了。
- [ ] 検証対象とbase/head/sourceを記録、検証後差分なし。
- [ ] 一時診断/計画/不要成果物を整理、採用結果を正本へ集約。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-11-10 | Codex | 実測に基づく初版 |
