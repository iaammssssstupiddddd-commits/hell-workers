# [実装計画タイトル]

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `<topic>-plan-YYYY-MM-DD` |
| ステータス | `Draft` / `In Progress` / `Blocked` / `Completed` / `Superseded` / `Archived` |
| 作成日 | `YYYY-MM-DD` |
| 最終更新日 | `YYYY-MM-DD` |
| 作成者 | `<name>` |
| 関連提案 | `docs/proposals/<proposal-file>.md`（なければ `N/A`） |
| 関連Issue/PR | `<link or N/A>` |

## 1. 目的

- 解決したい課題:
- 到達したい状態:
- 成功指標:

## 2. スコープ

### 対象（In Scope）

- 

### 非対象（Out of Scope）

- 

## 3. 現状とギャップ

- 現状:
- 問題:
- 本計画で埋めるギャップ:

## 4. 実装方針（高レベル）

- 方針:
- 設計上の前提:
- Bevy 0.19 APIでの注意点:
- 作業branch / 独立変更で新規作成か同目的branch再利用か / 比較基点:
- PRによるCI開始 / 公開の許可範囲 / 並行作業の保全:

## 5. マイルストーン

## M1: [マイルストーン名]

- 変更内容:
- 変更ファイル:
  - `crates/.../src/...`
  - `docs/...`
- 完了条件:
  - [ ] 
- 検証:
  - 同一対象の成功CI、または `python3 scripts/dev.py ci check --base <full-SHA> --mode auto`
  - Rust変更の開発中は `python3 scripts/dev.py check`、完了時のrust群はClippy警告0・workspace全testを含む。

## M2: [マイルストーン名]

- 変更内容:
- 変更ファイル:
  - `crates/.../src/...`
  - `docs/...`
- 完了条件:
  - [ ] 
- 検証:
  - 同一対象の成功CI、または `python3 scripts/dev.py ci check --base <full-SHA> --mode auto`
  - Rust変更の開発中は `python3 scripts/dev.py check`、完了時のrust群はClippy警告0・workspace全testを含む。

## M3: [マイルストーン名]

- 変更内容:
- 変更ファイル:
  - `crates/.../src/...`
  - `docs/...`
- 完了条件:
  - [ ] 
- 検証:
  - 同一対象の成功CI、または `python3 scripts/dev.py ci check --base <full-SHA> --mode auto`
  - Rust変更の開発中は `python3 scripts/dev.py check`、完了時のrust群はClippy警告0・workspace全testを含む。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
|  |  |  |

## 7. 検証計画

- 必須:
  - 同一対象の成功CI、または `python3 scripts/dev.py ci check --base <full-SHA> --mode auto`
  - Rust変更の開発中は `python3 scripts/dev.py check`、完了時のrust群はClippy警告0・workspace全testを含む。
- 計画完了時:
  - 分類が不確実な場合は `python3 scripts/dev.py ci check --base <full-SHA> --mode full` または `python3 scripts/dev.py verify`
  - `git diff --check`
- 手動確認シナリオ:
- パフォーマンス確認（必要時）:

### 検証データ管理（各バッチの開始前・報告前に更新）

- 正本: `docs/development-infra/validation-storage-workflow.md`
- primaryの `python3 scripts/dev.py validation` でretain/plan/execute、seal/finalize/checkを行う。
  台帳はGit common directoryに保持し、旧凍結subjectのhelperはprimary coordinator経由で使う。
- batch ID / 判断対象 / 責任者 / 全consumer:
- worktree・clone・branch / subject・asset view / job roots:
- 開始時bytes / 検証結果 / 採用成果物と最終結果の正本:
- 削除済みpath / 前後bytes / filesystem空き差:
- 残存path / bytes / owner / consumer / 次の作業 / release_when（終了条件）:
- 修正対応（review-active / accepted / abandoned） / 最新提示・修正日時:
- 整理状態（cleaned / review-active / 具体的な継続用途）と理由:

成功・失敗・中断のいずれもバッチ終了時に結果確定・整理する。全jobのcapsule保存は不要。
共有・親track未完を一括保持の理由にしない。固定の容量上限・日数は設けない。
フィードバック対応中のcandidate worktreeとtargetは同じ場所で保持し、不要な検証出力だけを整理する。
提示→応答待ち→修正→再確認を一つの利用単位とし、最終承認／終了前にcacheを撤去しない。
過去の記録を再検証し続けることを保持の目的にしない。

## 8. ロールバック方針

- どの単位で戻せるか:
- 戻す時の手順:

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0-100%`
- 完了済みマイルストーン:
- 未着手/進行中:

### 次のAIが最初にやること

1. 
2. 
3. 

### ブロッカー/注意点

- 

### 参照必須ファイル

- `docs/...`
- `crates/.../src/...`

### 最終確認ログ

- 最終 `python3 scripts/dev.py check`: `YYYY-MM-DD` / `pass or fail`
- 最終 `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`: `YYYY-MM-DD` / `pass or fail`
- 最終 `python3 scripts/dev.py cargo -- test --workspace`: `YYYY-MM-DD` / `pass or fail`
- CI run URL / base・head・tested SHA / mode・選択群・結果:
- ローカル代替command / 比較基点・dirty範囲・source fingerprint / 結果:
- Help実レビュー / native受入要否・結果 / primary storage確認:
- 未解決エラー:

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] 同一base/head/tested SHA・選択群の成功CIとURL、またはdirtyを含むローカル変更別検証を記録した。
- [ ] 検証後の追加差分がなく、必要群のskip/cancel/欠損を成功扱いしていない。
- [ ] Help実レビュー、必要なnative受入、storage整理をCIとは別に確認した。
- [ ] Rust対象ではcheck・Clippy警告0・workspace testが成功した（非対象なら理由を記録）。
- [ ] 各検証バッチの結果確定・不要job / binary copy / worktree / clone整理を報告前に実施した。
- [ ] 最終close時に本計画のconsumerは0。共有残存は別consumer・担当者・bytes・終了条件を引継ぎ済み。
      削除path、前後bytes / filesystem空き差と最終結果を記録し、採用成果物を正本へ集約した。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `YYYY-MM-DD` | `<name>` | 初版作成 |
