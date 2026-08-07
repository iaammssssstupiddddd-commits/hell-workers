# Native Acceptance Resource Safety Plan

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `native-acceptance-resource-safety-plan-2026-08-05` |
| ステータス | `Complete` |
| 作成日 | `2026-08-05` |
| 最終更新日 | `2026-08-08` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: tmpfs上のCargo targetやtemporary artifactがRust/Bevyの成果物でRAMとswapを圧迫し、実機検証そのものを開始できなくなる。
- 到達したい状態: native acceptance、performance runner、品質ゲートは workspace の永続 target とdisk-backed temporaryだけを使い、`/tmp`は小さいlock以外に使わない。
- 成功指標: unsafe な inherited target / temporary / toolchain cache / artifact root と Hell Workers の残存 tmp target を計画時に明示して fail-closed にし、全サポート入口が安全な環境を強制する。

## 2. スコープ

### 対象（In Scope）

- native acceptance skill / helper、`scripts/perf.py`、`scripts/dev.py`のCargo target・temporary・toolchain cache・build job方針、resource preflight、自己検証。
- `/tmp/hell-workers-*-target` の非破壊検出と明確なブロック理由、およびmemory-backed artifact root拒否。
- 開発・性能ドキュメントへの永続 target / 小さい tmp artifact 契約の反映。

### 非対象（Out of Scope）

- 既存の tmp artifact を自動削除すること。
- ユーザーや他プロジェクトの process、cache、mount 設定の変更。
- native acceptance の測定条件、renderer 要件、GUI ランチャー境界の緩和。

## 3. 現状とギャップ

- 観測: `/tmp/hell-workers-p00-target` が 7.8 GiB（incremental 3.0 GiB、deps 4.6 GiB）で、tmpfs の shared memory 9.3 GiB と swap 枯渇を招いた。
- 問題: runner は RAM 下限を検知するが、Cargo target、temporary、Cargo/rustup cache、artifact child process が tmpfs に向くことを完全には防止・説明できない。
- 本計画で埋めるギャップ: target / temporary / toolchain cacheをworkspaceまたはaccountの永続storageに固定し、tmpfs、resolved symlink/mount、残存 temporary targetをsource-of-truthのplan出力へ含める。

## 4. 実装方針（高レベル）

- shared runtime helper、native helper、performance runnerは`CARGO_TARGET_DIR`をworkspace `target/`へ、`TMPDIR` / `TMP` / `TEMP`をworkspace target配下へ、unsafeな`CARGO_HOME` / `RUSTUP_HOME`をaccount既定の永続cacheへ正規化する。performance/native buildはincrementalを無効にし、全入口は最大2 Cargo jobsに制限する。Cargo compilationは`MemAvailable` 8 GiB未満では開始しない。swap使用量は診断情報として記録し、native recipeは開始時に10 GiB、実行中に8 GiBのRAM下限を守る。実行中の下限割れや監視不能ではstageのprocess group全体を停止する。
- workspace target、process temporary、toolchain cache、job root、performance artifact root、RenderDoc stagingのfilesystemがtmpfs/ramfsまたは`/tmp`なら、resolved symlink/mountを含め開始を拒否する。Tracy / csvexport / RenderDocのchild processもcontrolled temporary環境を継承する。
- Hell Workers 名のtmp targetはroot metadataまたはdebug/profiling/release lockで識別し、サイズを記録して非破壊で停止する。削除は別途明示依頼がある場合だけ行う。
- Skill本文、agent rules、性能/開発ドキュメント、plan/proposal templateは、`/tmp`に許すのは小さいlockだけであることと、Cargoを`python3 scripts/dev.py cargo -- <subcommand> ...`経由で実行することを明記する。

## 5. マイルストーン

## M1: Safety contract と全実行入口のguard

- 変更内容: workspace target / disk temporary強制、tmpfs判定、残存 tmp target inventory、全native run route・performance build/run・品質ゲートの共通env化。
- 変更ファイル:
  - `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`
  - `scripts/perf_tool/execution.py`
  - `scripts/perf_tool/fixtures.py`
  - `scripts/perf_tool/renderdoc_capture.py`
  - `scripts/perf_tool/rtt_light_bundle.py`
  - `scripts/perf_tool/model.py`
  - `scripts/cargo_runtime.py`
  - `scripts/dev.py`
  - `scripts/tests/test_cargo_runtime.py`
  - `scripts/check_agent_rules.py`
- 完了条件:
  - [x] native/perf/dev build が `/tmp` の target・temporary・Cargo/rustup cacheを継承せず、`MemAvailable` 8 GiB未満ではCargo compilationを始めない。
  - [x] unsafe workspace target / job root / artifact root / RenderDoc stagingと残存Hell Workers tmp targetがfail-closedになる。
  - [x] native job rootが`target/native-acceptance/`へ移る。
  - [x] runner が artifact を自動削除しない。
- 検証:
  - `python3 .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py self-test`
  - `python3 -m unittest scripts.tests.test_cargo_runtime`

## M2: Documentation と forward validation

- 変更内容: 永続 target / tmp artifact 契約を性能ドキュメントへ同期し、skill 構造を検証する。
- 変更ファイル:
  - `docs/performance-profiling.md`
  - `docs/DEVELOPMENT.md`
  - `.cursor/skills/hell-workers-run-native-acceptance/SKILL.md`
  - `docs/plans/README.md`
- 完了条件:
  - [x] 実機検証と性能検証の両方で RAM を重い build cache に使わない方針が読める。
  - [x] skill validation と docs index が成功する。
- 検証:
  - `python3 /home/satotakumi/.codex/skills/.system/skill-creator/scripts/quick_validate.py .codex/skills/hell-workers-run-native-acceptance`
  - `python3 scripts/dev.py docs --check`

## M3: Native recipeのlive resource guard

- 変更内容: native recipe開始はRAM 10 GiB、実行中はRAM 8 GiBを維持する。swap使用量は証跡へ記録するがRAMの余裕がある場合は開始条件にしない。下限割れではstageの専用process groupをTERM/KILLして、他の作業を巻き込まずにartifactへ理由を残す。
- 変更ファイル:
  - `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`
  - `scripts/cargo_runtime.py`
  - `scripts/tests/test_cargo_runtime.py`
  - `.cursor/skills/hell-workers-run-native-acceptance/SKILL.md`（adapter mirrorへ同期）
  - `docs/performance-profiling.md`
  - `docs/DEVELOPMENT.md`
  - `README.md`
- 完了条件:
  - [x] native helperが実行中のRAM下限割れを1秒間隔で検出し、stageだけを停止する。swapは診断値として保存する。
  - [x] Linuxの`MemAvailable`不足、監視例外、SIGTERM無視のchild process groupをfail-closedに検証する。
  - [x] helper self-testが強制的なlive guard発火後にchild process groupが残らないことを検証する。
  - [x] actual-window V1〜V5をIntel Arc / Vulkan / X11で再実行し、V1〜V5すべてPASSを確認する。
- 検証:
  - `PYTHONDONTWRITEBYTECODE=1 python3 .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py self-test`
  - `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest scripts.tests.test_cargo_runtime`
  - `plan-deconstruction` がRAM不足時にbuild前で`blocked`を返すこと、RAMに余裕があればswap空きが少なくても`ready`を返すこと

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| legacy artifact を自動削除する | 診断証跡を失う | inventory のみ。削除は別権限とする。 |
| `CARGO_TARGET_DIR` / `TMPDIR` / `CARGO_HOME` / `RUSTUP_HOME` を暗黙継承する | tmpfs 再発 | 全サポート入口がworkspace target / disk temporary / persistent toolchain cacheを明示設定する。 |
| 大きいjob/trace artifactを`/tmp`に置く | RAM / swapを圧迫する | job rootとartifact rootをtarget配下へ固定し、`/tmp`指定を拒否する。 |
| workspace 自体が tmpfs | 同じ RAM 圧迫 | mount type を計画時に検出して拒否する。 |
| RAMが閾値を下回る、またはLinuxで`MemAvailable`を取得できない | game / compiler開始後にhostを圧迫する | RAMの開始・実行下限とcounter取得をpreflightと共通Cargo入口へ追加し、native stage中も1秒間隔で再確認してprocess groupを停止する。swapは診断値として保存する。 |

## 7. 検証計画

- 必須:
  - helper self-test
  - perf / dev env unit test
  - Skill Creator quick validation
  - `git diff --check`
- 計画完了時:
  - docs index check
  - resource plan が stale tmp target を明示して blocked になること
- 手動確認シナリオ: `CARGO_TARGET_DIR=/tmp/... TMPDIR=/tmp/... CARGO_HOME=/tmp/... RUSTUP_HOME=/tmp/...` を親 env に設定しても、native/perf/devがworkspace target、disk temporary、persistent toolchain cacheを使うこと。
- live guard確認: `MemAvailable` 10 GiB未満のhostではnative planが`blocked`を返してkitty/Cargoを起動しないこと。RAMが十分ならswap空きが1 GiB未満でも`ready`となることを確認する。開始後のRAM下限割れはself-testのisolated child processで検証し、実hostを意図的に圧迫しないこと。

## 8. ロールバック方針

- helper / skill / docs の小さい変更単位で戻せる。
- `target/` と `/tmp` の既存 artifacts は変更しない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `M1〜M3の実装・検証・actual-window V1〜V5を完了。`
- 完了済みマイルストーン: M1 safety guard、M2 documentation / forward validation、M3 live resource guard
- 未着手/進行中: なし。

### 次のAIが最初にやること

1. legacy tmp targetを削除せずにplanのblock理由を再確認する。
2. `MemAvailable >= 10 GiB`を確認したうえで、guarded `scripts/dev.py` gateとno-prompt native acceptanceを実行する。swapは診断値として記録する。
3. legacy tmp targetの削除が明示依頼された場合だけ、対象と容量を再確認して別作業として実施する。

### ブロッカー/注意点

- 旧`/tmp/hell-workers-p00-target`は現時点で存在しない。再発時も削除は別の明示依頼でのみ行う。
- `MemAvailable`が約9.8 GiBの状態はnative recipe開始不可だが、RAMが16 GiB以上ある状態ではswap空きが1 GiB未満でも開始可能とする。

### 参照必須ファイル

- `.codex/skills/hell-workers-run-native-acceptance/SKILL.md`
- `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`
- `docs/performance-profiling.md`

### 最終確認ログ

- helper / perf / RenderDoc self-test、Python tooling test、agent-rule、skill-sync: `2026-08-05` / pass
- M3 helper self-test、Cargo runtime unit test: `2026-08-06` / pass
- enforced probe: inherited `/tmp` target / temporary / Cargo/rustup homeは永続storageへ正規化。legacy tmp targetなしを確認し、native planは`MemAvailable 16.95 GiB`で`ready`となった（swapは`0.78 GiB`を診断記録）。
- current-source actual-window rerun: `target/native-acceptance/building-deconstruction-20260807T203553Z-56413aa7` / `MemAvailable 16.64 GiB` / swap `0.84 GiB`（診断値）/ Intel Arc・Vulkan・X11 / V1〜V5 PASS。V4はタスク行の確認状態を待ってから2回目のキャンセルを押し、操作中はシミュレーションを安定化した。

### Definition of Done

- [x] M1〜M2が完了
- [x] unsafe target の再発を helper と Skill で防止
- [x] 影響ドキュメントが更新済み
- [x] helper / Skill / docs validation が成功
- [x] RAM不足のpreflight拒否とlive process-group stopを実装・isolated検証済み
- [x] safe RAM条件下でactual-window acceptance V1〜V5を再実行

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-05` | `Codex` | tmpfs Cargo target による native acceptance resource pressure を是正する計画を作成 |
| `2026-08-06` | `Codex` | swap不足を開始前・実行中ともにfail-closedにし、native child process groupを隔離停止する契約を追加 |
| `2026-08-08` | `Codex` | RAM中心のresource guardへ修正し、swapを診断値へ変更。Intel Arc / Vulkan / X11の実機V1〜V5を全PASSで確認 |
