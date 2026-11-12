# 橋以外9種の導入前性能比較

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | non-bridge-candidate-performance-plan-2026-11-12 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-12 |
| 作成者 | Codex main |
| 関連提案 | N/A |
| 関連Issue/PR | TAK-14 / 非橋9種正式導入の継続 |

## 1. 目的

正式導入前の8候補と既存Door g7の混在費用を、同一source・同一adapterの
legacy-controlと比較する。現M6は8種のReleaseApprovedを要求するため、導入前比較には使えない。
候補のauthorityを変更せず、別profile/sidecar contractを持つprofiling専用入口を追加する。
本作業は全体の正式導入の一工程であり、通常world lifecycle・GPU配分量・release承認を代替しない。

## 2. スコープ

対象はmain所有のRust profiling admission、Python matrix/oracle、拒否系テスト、実機比較。
Bridge、既存release入口の許可緩和、ゲームルール、独立承認の作成、外部公開は対象外。

## 3. 現状とギャップ

8候補にはmanifest、art/numeric approvalと制作原本provenanceが存在する。
Door g7は別loaderで維持する。現loader/presentation候補許可は単一identityであり、
複数候補比較の明示許可が必要。正式導入済み比較は引き続きReleaseApprovedだけを受理する。

## 4. 実装方針

- 明示profiling sessionでexact8 IsolatedCandidate identityとcanonical locatorを検査する。
- production、ordinary-art、art-preview、Bridge、欠損/重複kind、別scopeの混在を拒否する。
- primary assetsを変更せず、全fallback bytesと8候補の検証済みbytesから専用asset viewを作る。
- raw sidecarの別contractを独立oracleで検証し、元M6のrelease-only oracleは維持する。
- Capture→Memoryを逐次実行。N/4N、各3隣接pair、中間repeatの順序を反転、30秒warmup/60秒measure。
- 数値budgetは測定前に理由と全metricを固定する。試験結果に合わせて緩和しない。
- 既存Bevy 0.19のstartup resource/loader capture方式を使う。新規外部APIを仮定しない。
- 同目的branch `codex/tak14-non-bridge-release`、比較基点
  `38c769f2d559abf46d5c1d3deb83413e7bd74621`。公開/PRは今回操作しない。

## 5. マイルストーン

### M1: 候補比較入口

- [ ] exact8 admission / 別contract / asset viewと改変拒否を実装。
- [ ] 独立read-only review、Help実レビュー、関連テスト、check/Clippyを実施。

### M2: 実機比較

- [ ] 全candidate packaged-byte/provenanceをcodecで再照合し、測定前budgetを固定。
- [ ] 同subjectのCapture12・Memory12を登録launcherで実行・独立verify。
- [ ] 失敗を修正し新subjectで再検証。成功を正式release/art承認へ読み替えない。

### M3: 次工程へ接続

- [ ] 静止比較の結果・未検証範囲を恒久仕様へ同期。
- [ ] 通常worldの残lifecycle/性能とguard付きpromotion/install/rollback、通常起動導入を継続。

## 6. リスクと対策

候補authorityの通常経路への漏出は、profilingと明示session・全identity検査・旧入口の拒否試験で防ぐ。
静止試験の過大解釈は別contract・accepted=false・promotion_authority=falseと未検証範囲で防ぐ。
copyしたassetsの欠損/改変は開始時と独立verify時の完全inventory照合で拒否する。

## 7. 検証計画 / データ管理

primary `dev.py validation` のstorage checkを開始/終了時に実施。
同一baseからの変更別CI、check、default/profiling Clippy0、workspace test、Python回帰、Help実レビュー、
独立exact-source reviewの後にcommitしてnative sourceを凍結する。rust-analyzer診断も確認する。
batchは実行前にowner/consumer/roots/verifierを登録。既存8候補・codecのholdを維持し、
新jobは比較と独立reviewが終わるまで保持する。結果は既存仕様へ集約し、終了したconsumerをreleaseして
不要生成物を回収可能な手段で撤去し、bytesとstorage結果を記録する。原本・唯一の成果は削除しない。

## 8. ロールバック

新profiling sessionを指定しなければ旧動作。primary assets/authorityは変更しない。
実装差分は本比較のcommit単位で扱い、既存の未完成果を巻き戻さない。

## 9. AI引継ぎ

8候補のpackaged bytes/art/numeric provenanceを現行debug codecで再照合済み。
session/loader/pool、matrix/oracle、拒否系試験、測定前budgetを実装した。
初回contracts/tooling/rust CI相当gateはpass（Python800・制作229・default/profiling全workspace test・
全profiling feature check・Clippy0）。独立reviewのf32/Tank状態比較指摘は修正済み。
最後のprovenance validator hash固定を追加し、最終差分の再検証とexact review後にcommit/nativeへ進む。
既存のproduction runnerはnormal-development admission・ordinary-world capture/ACK・host instrument producerが
未実装と宣言する。旧Orcaや架空receiptで補完しない。全体受入は継続中。

### Definition of Done

- [ ] 候補比較と独立verifyが成功し、同subjectの検証/Help/reviewを記録。
- [ ] 永続仕様へ結果を移し、不要job整理とstorage確認、索引を更新。
- [ ] 未完の通常world/正式導入工程を保持して継続し、本比較を全体完了としない。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-11-12 | Codex main | 導入前比較を正式導入後M6から分離する計画 |
