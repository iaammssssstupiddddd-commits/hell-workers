# Orca 配車・レビュー受付の修正

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | orca-dispatch-admission-plan-2026-09-24 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-09-24 |
| 作成者 | Codex coordinator |
| 関連提案 | orca-parallel-development-proposal-2026-09-20 |
| 関連Issue | TAK-12 |

## 1. 目的

TAK-12で発覚した配車前編集と、不正review JSONを送信後に検出して停止する問題を恒久修正する。
実providerで実装→差戻し→再実装→固定review→統合まで確認する。

## 2. スコープ

対象はOrca tooling、hook、review transport、停止中の通知処理と同一試験環境の回復。
ゲーム実装・ゲーム実機テスト・製品master統合は対象外。

## 3. 現状とギャップ

Cursor初期promptに実作業が含まれ、beforeSubmitPromptはauthorityなしでもcontinueを返す。
review worker_doneはJSON形式を検査せずsettleする。paused loopは未ACK通知を処理しない。
TAK-12 Aはcheckpoint済み、Bは未配車の変更を保全済み。これらを成功扱いしない。

## 4. 実装方針

同目的candidate branchを継続（比較基点7f7526c997eb1f4e8abb349bd54b097a60e14e17）。
初期promptから実作業を除き、hookでlive Dispatchとhost arm確認前のpromptを拒否する。
レビューは信頼済みticketと照合し、送信前の形式不備だけを同一Dispatchで再提出可能にする。
通信結果不明・権限不一致は従来どおり停止する。Bevy API変更なし。
primaryの既存dirty docsは保全し、公開は許可済み試験範囲に限定する。

## 5. マイルストーン

### M1 配車受付

- [x] Cursor hookとbootstrapに配車前拒否を実装。実provider受入はM3。
- [x] エラー識別子の診断と受付の回帰テストを追加する。

### M2 レビュー受付・通知

- [x] 不正形式をupstream settlement前に拒否し、正しい再提出を同一attemptで受け付ける。
- [x] pausedでも通常通知処理を行う。新規配車はしない。実TAK-12の排出は未完。

### M3 実受入

- [ ] 既存TAK-12のsource/Task/Dispatch/process証拠に基づく回復。
- [ ] 実A/B・固定reviewerで差戻しを含むループ、重複タブなし、終了通知処理を確認。

## 6. リスクと対策

hookとarmの競合は有界待機で扱う。未知の権限を推定しない。
旧driverは起動済みPython codeを保持するため、配備変更だけで更新済みとみなさない。
未配車B差分は保全し、正規実装の証拠に転用しない。

## 7. 検証計画・データ管理

focused Python tests、candidate `dev.py ci check --base 7f7526c997eb1f4e8abb349bd54b097a60e14e17 --mode auto`、
Help実レビュー、primary `dev.py validation check`、`git diff --check`。
ゲーム非変更のためRust/native game検証は対象外。
既存retentionのTAK-12 integration/A/Bとcandidateを同一用途で保持する。
ownerは統括、consumerはTAK-12 supervised review loop、終了条件は再受入と外部終了完了。
試験結果・未検証範囲は `docs/development-infra/orca-ui-extension.md` へ集約する。

## 8. ロールバック

変更commit単位で戻す。試験台帳・sourceは改変して成功にせず、実行中なら先に停止証拠を確認する。

## 9. AI引継ぎ・完了条件

M1/M2実装済み。関連102件、変更別contracts/tooling成功。M3は未完。
全体完了は実providerループの証拠が揃った時点のみ。
旧driverは旧worktreeのmoduleを保持し、candidate直接tickは作業場一致guardで拒否される。
旧試験を再開済みとみなさない。安全な配備・回復経路を実装することが次の作業。
関連scriptsはcandidate、正本文書はprimary。既存試験を新Runで隠さない。
参照: orca-development.md、orca-ui-extension.md、validation-storage-workflow.md。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-24 | Codex | TAK-12の失敗証拠に基づく修正計画 |
