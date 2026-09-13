# 検証データの整理ワークフロー

2026-09-13改訂。native / performance / visual検証のjob、診断、binary copy、検証worktree / cloneを対象にする。

## 基本方針

**終了した仕事の検証データは原則として残さない。現在使う用途があるものだけ保持する。**

- 採用コード・アセット・制作原本は製品の正本へ、最終結果・未検証範囲は既存の仕様書や完了記録へ集約する。
- 全batchのmanifest・CSV・画像・capsuleを保存する義務はない。過去の結果を再検証し続けること自体も要件にしない。
- 未完の修正・比較、配布・復旧に必要なものは、具体的な用途と終了条件で保持する。
  親trackが未完、失敗した、旧文書にpathがある、将来役立つかもしれない、という理由だけでは保持しない。
- 256 GiB、7日、100 MiB等の一律上限・保存期間を設けない。実測前の蓄積量を必要容量に読み替えない。
  容量上限が必要な環境だけ、測定と実際の制約に基づいて明示設定する。通常の空き容量・RAM guardは維持する。

## フィードバックと差分ビルド

提示→応答待ち→修正→再検証→最終承認／終了を一つの作業単位とする。
同じ修正対象のcandidate worktreeとCargo targetを維持し、反復ごとに作り直さない。
途中のpass・報告やユーザーの無応答でこの作業を閉じない。最新提示画像も判断に必要な間だけ残す。
検証jobの寿命と作業用build cacheの寿命を分け、最後の利用者が終了した時点で専用環境を撤去する。

修正中の通常起動は `python3 scripts/dev.py feedback`、壁・ドア合同storyboardはnative helperの
`plan --feedback` を優先し、既存 `target/debug` と `CARGO_INCREMENTAL=1` を使う。
同じfeature/profileを継続し、軽量確認のために新しいtargetを作らない。
合同feedbackは未commit sourceを許すが、plan/run/verifyのsource・asset・driver hashは一致を要求する。
通常の正式verifyはfeedback成果を拒否する。対象外の専用gallery・正式受入・性能計測では
既存nativeの `CARGO_INCREMENTAL=0` を維持する。Cargoの未変更crate再利用は可能だが、
source / feature / profile / toolchainの変更に応じた再コンパイルは発生する。
新しいsubjectの受入は新しいjobで行い、旧passを流用しない。
現在実施する比較が元repo / asset view / binaryを要求する場合だけ、比較終了までそのviewを凍結する。
比較を終了した後も旧verifierを動かすためにworktreeを残す必要はない。

## 残す対象

| 対象 | 保持する条件 | 整理する時点 |
| --- | --- | --- |
| 修正用worktree / targetと最新提示画像 | フィードバック待ち・修正中 | 最終承認／終了後、他の利用者がいなければ撤去 |
| 比較入力・診断raw / RDC / binary | 実施する比較・原因調査が具体的にある | 比較／診断終了・打切り時に削除 |
| 採用コード・asset・制作原本 | 製品の成果物として使う | 正本へ集約。検証環境側の重複は削除 |
| 登録済みrelease・承認・復旧情報 | 現行製品の配布・復旧で参照する | 製品側の移行・廃止に合わせる。jobごとの複製は不要 |
| primary通常Cargo cache / lane | 継続する通常開発で使う | 検証job cleanupとは別の保守対象 |
| 所有不明の独自コード・原本 | 他sessionの成果を失わないための保全 | 所有・採用先確認後に正本へ引継ぐ。巨大な生成物まで一括保持しない |

保持台帳にはexact path、owner、consumer、next_action、release_when（用途が終了する条件）、
実測bytesを記録する。日数を埋めるためだけの期限は設けない。
必要な場合のreview_atは状態確認日であり、過ぎても削除・修正build停止の理由にしない。
複数の利用者がいる場合は終了した利用者だけを解除する。

## 実行と終了

primaryの `python3 scripts/dev.py validation` が台帳・起動許可・結果記録・整理確認を扱う。
台帳はprimary Git common directory内の `validation-storage/ledger.json`。
凍結した旧checkoutのhelperはprimary coordinatorの子として使い、旧subjectへ新規則をコピーしない。

1. **開始前**: 対象、owner、必要な作業環境・比較入力を確認し、保持とbatchを登録する。
   新規treeは現在の比較依存が必要とする場合に限る。
2. **実行**: planが返す既定kitty launcherを使う。nativeは既存のCapture→Memory逐次実行・原本検証を守る。
3. **結果確定**: 成功は元環境がある間に登録済み独立verifierを実行する。
   失敗・中断・打切りは理由と未検証範囲を記録し、存在しないmanifestや中止済み検証の再開を要求しない。
   `seal` はこの結果を台帳へ記録する。全jobのarchiveやファイルコピーを作らない。
4. **整理**: 利用者のないraw・job・binary copyを削除する。最新結果に必要な記述は既存文書へ集約する。
   正式releaseが必要とする承認等は正本に含まれることを確認し、必要なコピーをした場合だけ元とhash照合する。
5. **確認**: `finalize` はjobの不存在、またはjob自体の具体的な保持用途を確認する。
   candidateのreview holdだけでは全旧jobの保持を認めない。`check` を通して報告する。
6. **仕事のclose**: その仕事のconsumerをreleaseし、残る利用者がなければ専用worktree / branchを撤去する。
   共有treeを別の作業に引継いでも、終了した仕事の旧job一式は残さない。

`dev.py verify` もstorage checkを実行する。未登録出力、未整理batch、用途のない残存、未分類legacy、
稼働中依存の欠損を拒否する。終了済みcapsuleの消失・変更は検査しない。
台帳には対象・結果・整理経過のmetadataだけが残り、過去成果物を保存させる依存を作らない。
coordinatorはファイルを自動削除しない。担当者が下記の確認をして削除する。

## コマンド例

初期化は一度だけ。旧領域があれば未分類として棚卸しし、存在するだけで保持を認めない。

```bash
python3 scripts/dev.py validation init
python3 scripts/dev.py validation status
python3 scripts/dev.py validation retain --spec /absolute/active-use.json
python3 scripts/dev.py validation plan --spec /absolute/batch.json -- <既存helperのplan argv>
```

active-use.jsonの例。review以外は `comparison` / `diagnostic` / `evidence` を用途に応じて使う。

```json
{
  "id": "wall-candidate", "path": "/absolute/candidate",
  "kind": "review", "owner": "wall-formwork",
  "consumers": ["formwork-release-matrix"],
  "review_item_id": "wall-formwork",
  "latest_feedback_at": "2026-09-13T00:00:00+00:00",
  "next_action": "現在のreleaseで未実施の混在matrixを確認する",
  "release_when": "当該受入とフィードバック対応が終了する"
}
```

batch.jsonには `id` / `repo` / `owner` / `consumers` / `verify_command` を指定する。
planはhelper出力のjob_root等を登録し、verify argvの `@job_root`、`@state_root`、
`@output_root`（子path指定可）を解決する。perfの直接実行は `register --spec ... -- <argv>` →
`execute --batch ...` とし、specにexact `roots` も指定する。予約bytesは実測に基づく任意項目。

```json
{
  "result": "pass", "reason": "登録した独立verifyがpass。未検証範囲は完了記録へ記載",
  "verify_command": ["python3", "/absolute/helper.py", "verify", "--root", "/absolute/job"]
}
```

```bash
python3 scripts/dev.py validation seal --batch <id> --spec /absolute/result.json
# ここで用途のないjobを、以下の確認を経て削除する。
python3 scripts/dev.py validation finalize --batch <id>
python3 scripts/dev.py validation release --hold <id> --consumer <consumer> --reason '<終了理由>'
# 最後のconsumerを解除した専用環境を撤去する。
python3 scripts/dev.py validation reconcile
python3 scripts/dev.py validation check
```

失敗時はresultを `invalid` / `interrupted` / `abandoned` とし、理由を記録する。
coordinatorが停止した場合は子processも終了したことを確認して `recover --batch ... --reason ...` から
seal / finalizeへ進む。稼働中のhelperがある場合は回復・削除を拒否する。
任意の上限は `budget --gib <値> --reason '<実測と環境制約>'`、
撤廃は `budget --none --reason '<理由>'`。既定上限はない。

## 既存領域の整理

- primaryと各独立cloneのworktree一覧、対象の `git log --oneline -5` と `git diff HEAD`、
  未追跡・ignoredファイルを確認する。独自commitや未コミット変更を廃棄しない。
- processのcwd・open file・実行binary、作業sessionの所有を確認する。使用中は削除しない。
- asset / 原本 / save / release / 復旧情報を生成物と分け、唯一の成果物は正本または明示した保全先へ引継ぐ。
  Git管理情報と少量の独自コードの保全のために、数十GiBのtargetを残さない。
- 不要生成物をexact pathで削除後、子worktreeから `git worktree remove` で撤去する。
  `--force`、広範囲の `git clean`、primary `target/` の一括削除を使わない。
- `reconcile` は削除済み・用途を登録済みのlegacy entriesだけを台帳から外す。
  新たに増えた未登録pathをlegacyへ追加するコマンドではない。
- 削除path、前後のdu / df、保持path・用途と独自成果物の引継先を報告する。
  Btrfsの圧縮・共有extentによりdu減少と実際の空き増加は一致しない場合がある。

旧計画の成功・失敗・未検証という結果は履歴のまま維持する。元データの撤去後は履歴として読み、
古いpathやhashの記載を現在実行できる検証・保持義務と解釈しない。

## 適用結果

制定時の調査は[容量調査](validation-storage-audit-2026-09-13.md)、
初期設計の問題と本改訂の検証・整理結果は[レビュー記録](validation-storage-workflow-review-2026-09-13.md)を参照する。
