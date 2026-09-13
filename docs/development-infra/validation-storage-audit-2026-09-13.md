# 検証データの蓄積調査（2026-09-13）

以下は整理前の観測記録。現在の保持義務ではない。同日後続の整理で旧環境と不要rawを撤去した。
現行規則は[ワークフロー](validation-storage-workflow.md)、容量と残存用途は
[整理結果](validation-storage-workflow-review-2026-09-13.md)を参照する。終了済みjobの一律保存は撤廃済み。

調査対象はprimary checkout `d960da6a`、その `target/`、隣接する
`/home/satotakumi/projects/hell-workers-validation/`、保持規則と検証runner。
削除・ゲーム起動・ビルド・検証結果の再ラベルは行っていない。

## 判定

容量面で直ちに停止が必要な状態ではないが、「検証データを溜めない」運用としては不十分。
`/home` は952 GiB中400 GiB使用、549 GiB空き、使用率43%だった。
native開始時の空き15 GiB条件には余裕がある。この条件は蓄積を防ぐ上限ではなく、開始を拒否する下限。

残存は、通常のCargo cache、旧検証checkout、保存された検証job、未完trackのworktreeに分かれる。
最近のworktreeすべてを削除漏れとするのも、すべての残存を正常とするのも不適切。

## 実測

`du -sx -B1`によるallocated bytesをGiBへ換算した。包含関係のある行は加算しない。
filesystemは圧縮有効のBtrfsなので、この値は削除による物理空き容量の増加保証ではない。

| 対象 | GiB | 内容・扱い |
| --- | ---: | --- |
| primary `target/` 全体 | 350.93 | 以下のprimary配下を包含 |
| `target/debug` | 148.28 | 通常開発のCargo出力。検証jobの撤去とは別の保守対象 |
| `target/debug/incremental` | 79.38 | `debug`の内数。キャッシュ全削除は再ビルド負荷を生む |
| `target/profiling` | 25.44 | 計測用Cargo出力 |
| `target/profiling-renderdoc` | 5.42 | RenderDoc用Cargo出力 |
| `target/native-acceptance` | 53.78 | 検証job・診断・binary capsule |
| うち `renderdoc-foundation` | 43.97 | 62 directory、38 RenderDoc binary capsule、20 RDC |
| `target/perf-runs` | 10.80 | 計測結果・比較証拠 |
| `target/p00-formal-subject` | 32.32 | 8月の独立clone。primaryのworktree一覧には出ない |
| `target/p01-single-scene` | 35.40 | 上記cloneが管理するworktree。さらに内部worktreeあり |
| `target/renderdoc-live-subject.fnAQw0` | 29.58 | 8月の独立clone。tracked 5ファイルに未コミット変更あり |
| `target/renderdoc-rd0-subject-5e3fdc9a` | 0.84 | RenderDoc cloneが管理するworktree |
| `target/legacy-cargo-target-p00-20260805` | 7.77 | 旧Cargo出力 |
| 隣接 `hell-workers-validation/` 全体 | 48.11 | primaryに登録された11個のworktree |

旧checkout 4個は合計約98.13 GiB。RenderDoc内の拡張子なしファイルは約31.24 GiB、
RDCは約12.72 GiBであり、短いテキストログだけが肥大化しているわけではない。
primaryの `target/native-acceptance/*/job.json` 207件の記録上のstatusはvalid 117 / invalid 84 / running 6。
running 6件のheartbeatは8月6日〜25日のままで、現在の実行証拠にはならない。
これは保存statusの集計であり、valid 117件を現行sourceの受入として再検証した結果ではない。
`/proc`の確認時、Cargo・rustc・bevy_app・renderdoccmdの実行は見つからなかった。
他の開発sessionは存在するため、未使用・削除可能という判断には置き換えない。

## なぜ蓄積するか

1. **9月5日の変更は規則の変更で、自動撤去の実装ではない。**
   commit `1dc5aa5f`（`docs: dispose of acceptance workspaces at plan close`）は
   AGENTS、Skill、asset workflow、plan template等を変更した。
   [native Skill](../../.codex/skills/hell-workers-run-native-acceptance/SKILL.md)は
   track途中の削除を禁止し、track close時にmanifest・比較CSV・承認PNGを保存して
   worktree / branchを撤去するよう要求する。毎job終了時の削除や保持期限は定めていない。

2. **runnerの成功とtrackの整理完了が接続されていない。**
   例えば `wall_door_joint_acceptance.py` の `run` は新規job directoryを作り、
   manifest検証後にstatusをvalidにして終了する。capsuleの外部保存やworktree撤去はしない。
   `renderdoc_foundation.py::copy_binary_capsule` はjobごとに実行binaryをコピーする。
   `renderdoc_capture.py::RetainedFailureDirectory` が成功時に消すのはscratchで、
   失敗診断は保持する。検査したrunner / dev gateには、trackの所有者・保持期限・容量上限を
   管理してclose時の残存を拒否する共通処理がない。

3. **未完trackと共有するworktreeは、子タスクを閉じても残る。**
   [型枠計画](../plans/3d-rtt/provisional-wall-formwork-plan-2026-09-05.md)と
   [扉計画](../plans/3d-rtt/production-door-art-plan-2026-09-05.md)は
   通常release受入の残件と全trackのcapsule / worktree整理を未完としている。
   [壁修正のclose記録](../plans/archive/wall-surface-uv-plan-2026-09-12.md)も
   `wall-door-joint-83ad3f85`は両track所有として保持し、回収0と明記した。
   11個のworktreeはいずれもtracked差分はなかったが、過去の失敗jobもcloseまでは保持する契約。
   この調査は未検証範囲の再開や他trackのcloseを指示するものではない。

4. **primaryのworktree一覧だけでは古い検証領域を把握できない。**
   `p00-formal-subject` と `renderdoc-live-subject.fnAQw0` は `.git/` を持つ独立clone。
   前者の `git worktree list` には `p01-single-scene` とその内部 `source-e4a`、
   後者には `renderdoc-rd0-subject-5e3fdc9a` が登録されていた。
   primaryからの一覧だけをclose確認にしても、この残存は検出できない。

5. **通常Cargo cacheは別方針で保持される。**
   [開発規則](../DEVELOPMENT.md)と[scripts説明](../../scripts/README.md)は通常check/buildの
   暗黙cleanupを行わない。`post-build-cleanup.sh`も通常debug領域は容量警告のみで、
   削除対象は大きなWindows cross compile領域に限られる。現行wrapperからの自動呼出しもない。

## 現状の保全と改善対象

- 最近の11 worktreeは、未完trackの証拠としての保持理由が文書上存在する。
  ただし過去subjectを新subjectの受入へ流用できず、保持期限のない全量保存は蓄積を継続させる。
- 旧checkout群とlegacy Cargo出力は優先整理候補。ただし削除可能とはまだ認定していない。
  特にRenderDoc cloneの未コミット5ファイルは差分の所有者・必要性を確認して保全する必要がある。
  今回は差分本文の廃棄レビューや、全旧jobと保存capsuleの対応確認までは実施していない。
- 最新の壁修正については、外部release-evidenceのJ1 `manifest.json` と `joint-loaded.png` を
  実際にSHA-256検算し、[asset workflow](../assets_workflow.md)の記録と一致した。
  全jobの自己完結した証拠保存が完了したという判定ではない。
- 再発防止には、文書上のclose義務に加え、checkout / clone / jobの所有trackと寿命の台帳、
  capsuleの保存・hash検証・不要領域撤去を一つの明示close処理にまとめる仕組みが必要。
  共有領域は所有trackを列挙し、最後の所有者がcloseする際の整理を検査する。
  Cargo cache保守と失敗診断の保持期限は別に定義する。これらは提案であり今回未実装。

## 調査・検証範囲

`git status`、primary / clone別の `git worktree list`、Git履歴、`du`、`df`、
`findmnt`、bounded JSON集計、process inventory、外部capsule 2ファイルのhash照合を実施した。
容量調査のために実機acceptanceやRustビルドを再実行する必要はなく、実行していない。
文書追加・索引リンクのみ変更。Helpは静的Rust providerから構築され、今回の調査文書を
runtimeで読み込む経路はない。操作・成立条件・表示・runtime assetは不変（Help: No impact）。
`python3 scripts/dev.py docs --check`、Help impact gate、`git diff --check`はpassした。
