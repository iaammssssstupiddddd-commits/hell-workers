# パフォーマンス計測

ランタイム最適化の比較は、`scripts/perf.py` を唯一の入口にする。スクリプトは profiling binary を計測外で一度だけbuildし、runごとに隔離したCSV・log・実行環境を保存してから、CSV契約、実GPU、ログ健全性、反復のcheckpointを検証する。raw artifact は `target/perf-runs/` 配下にだけ置き、commitしない。

再現性は二段階に分ける。通常の実時間ベンチマークは、ゲーム更新前の**初期 fixture**を必ず一致させ、warm-up/計測終端の状態は実測値として記録する。`Time<Virtual>`は実フレームのdeltaで進み、warm-up境界を越える最終frameがrunごとに異なるためである。完全に同じsimulation時刻での状態一致は、`scripts/perf.py audit` による固定stepの決定性auditとしてframe-time計測と別に扱う。auditは性能値を出力せず、通常の`summary.csv` baselineとも比較しない。

## 計測モード

| モード | runner option | 用途 | frame timeへのTracy擾乱 |
| --- | --- | --- | --- |
| Capture | `--instrumentation capture` | 標準のframe time・domain counter・CSV比較 | なし |
| Tracy | `--instrumentation tracy` | system CPU time のtrace採取 | あり。CSV baselineとは別run |
| Memory | `--instrumentation memory` | allocation/frame のtrace採取 | あり。CSV baselineとは別run |

`frames.csv` の `frame_time_ms` は Bevy 0.19 の `Time<Real>` のフレーム間隔であり、system CPU timeやGPU pass timeではない。`cpu`/`gpu` は描画構成の切替名である。system CPU timeはTracy、GPU pass/drawは固定frameのRenderDoc captureで別に採取する。

## 標準手順

最初にrunner自身の検証fixtureを実行する。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test
```

次に、比較するGPU、backend、window backendを明示し、同じseedを3回採取する。以下はIntel/Vulkanで全規模・全描画条件を採る例である。

```bash
python3 scripts/perf.py run \
  --workload gather \
  --sizes small,medium,large \
  --renders cpu,gpu \
  --repeat 3 \
  --seed 20260712 \
  --backend vulkan \
  --adapter Intel \
  --window-backend wayland \
  --present-mode novsync \
  --warmup-checksum-policy record \
  --output target/perf-runs/gather-intel-vulkan-seed-20260712
```

- runnerは `BEVY_ASSET_ROOT` をワークスペース根へ固定し、profiling binaryを直接起動する。
- perf起動ではユーザーの`settings/settings.ron`にあるpause/倍速を無視し、`Time<Virtual>`をunpause・1xへ固定する。終了時にもsettingsを書き戻さない。
- `WGPU_ADAPTER_NAME` は一致しなくてもBevyが別adapterへfallbackできる。`--adapter`と`--backend`を指定したrunでは、log上の実 `AdapterInfo` が一致しなければ失格になる。
- `--souls <n> --familiars <n>` を組にするとsize presetを上書きできる。custom populationもcase IDとsummaryへ記録される。
- `--preflight-runs 1` は本測定前に同じcaseを一回だけ温める。preflight artifactは残すが、aggregateには入れない。

短縮した経路確認には、例えば次を使う。

```bash
python3 scripts/perf.py run \
  --workload gather --sizes small --renders cpu --repeat 3 \
  --warmup-secs 3 --measure-secs 5 \
  --backend vulkan --adapter Intel --window-backend wayland \
  --output target/perf-runs/smoke-gather-intel
```

CPU/GPU切替、artifact、CSV契約だけを短時間で確認するときは、既にprofiling binaryをbuild済みである場合に限り次を使う。これは起動経路の確認であり、性能比較用のbaselineではない。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py run --skip-build \
  --workload gather --sizes small --renders cpu,gpu --repeat 1 \
  --warmup-secs 0 --measure-secs 1 \
  --backend vulkan --window-backend wayland \
  --output target/perf-runs/m0-smoke
```

CPU条件では`data/scene_roots.csv`のSoul main/mask/shadowとFamiliar rootがすべて0、GPU条件ではSoul数・Familiar数と一致しなければrunnerが失格にする。これはCPU-only条件へ対象外の3D sceneを混ぜないための契約である。

固定step auditはsimulation状態の診断専用である。frame-timeを採取せず、`summary.csv`も生成しない。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit \
  --sizes small --renders cpu --repeat 3 \
  --fixed-hz 64 --warmup-ticks 1920 --audit-ticks 128 \
  --backend vulkan --window-backend wayland \
  --output target/perf-runs/gather-fixed-audit
```

audit artifactの`data/determinism.csv`はcheckpointごとの状態checksum、`data/determinism_records.csv`は差分調査用のactor単位recordである。失敗時はそのaudit sessionを失格にするが、実時間baselineの`summary.csv`を置き換えたり、frame-time比較に混ぜたりしない。

Tracyやallocationは標準baselineと混ぜない。

```bash
python3 scripts/perf.py run --instrumentation tracy --sizes medium --renders cpu \
  --backend vulkan --adapter Intel --output target/perf-runs/tracy-medium

python3 scripts/perf.py run --instrumentation memory --sizes medium --renders cpu \
  --backend vulkan --adapter Intel --output target/perf-runs/memory-medium
```

## 反復・有効性の契約

各runは次をすべて満たしたときだけ有効である。

1. processが成功終了し、`PERF_CAPTURE: wrote`、空でない`frames.csv`、schema version一致の`summary.csv`がある。
2. `seed`、workload、size、render、初期entity/task checksumが要求caseと一致する。
3. 指定した場合、logの実adapter/backendが要求値と一致する。
4. capture完了前に、allowlist外の`WARN`、`ERROR`、Bevy command errorがない。
5. 同じcaseの全反復で、ゲーム更新前に採った`initial_state_checksum`（Soul/Familiar/Designation数と位置を含む）が一致する。これは常に必須である。

`--warmup-checksum-policy record` が既定であり、実時間ベンチマークの標準条件である。warm-up終端checksumの差と実際のvirtual/real秒数をartifactへ残し、負荷の位相ずれを確認できる。`require` は、同じwarm-up状態が成立することを診断したい場合だけ使う。現在の可変delta実行では、同じseedでも境界を越えるframeが異なるため、`require`で失格になることは期待される挙動である。

計測完了後のwarning/errorは有効性を失わせないが、`validation.json`の`teardown_warning_lines`、`aggregate.csv`の`post_capture_teardown_warning_counts`、`report.md`へ必ず記録される。現在確認されている`CommandQueue has un-applied commands`は、speech/conversationの`Commands::delayed()`が次の`PreUpdate`より前に`AppExit`で破棄されるteardown由来であり、強制flushして計測状態を変えてはならない。完了マーカー前の同種warningは従来どおり失格である。

scenario driverは `Warmup → Measure → Flush → AppExit` を自動遷移する。各checkpointのinitial、warm-up終端、measure終端のentity数・Designation数・state checksum、実際のvirtual/real秒数、p50/p95/p99/maxは`summary.csv`に入る。既存`gather`以外の`path-door`、`construction`、`ui-gpu`は専用セットアップを実装するまでrunnerで失敗するため、Gather結果として記録しない。

## Artifact形式と集約

```text
target/perf-runs/<session>/
  manifest.json             # git/binary hash、host、要求環境、実adapter、session status
  matrix.json               # seed、規模、描画、反復、時間、checksum policy
  aggregate.csv             # valid runだけのrunごとquantileの中央値/MAD
  report.md                 # valid/invalidと失格理由
  cases/<workload-size-render-seed>/
    run-001/
      command.txt
      requested-environment.json
      run.log
      validation.json
      run-metadata.json
      data/frames.csv
      data/summary.csv
      data/scene_roots.csv
```

fixed-step auditでは`frames.csv`と`summary.csv`の代わりに、`data/determinism.csv`と`data/determinism_records.csv`を出力する。

`summary.csv` schema v4には、frame-timeに加えcapture期間全体の`task_execution_souls_queried`、`task_execution_idle_skips`、`task_execution_handler_runs`、`reservation_sync_full_rebuilds`、`reservation_sync_pending_tasks_scanned`、`reservation_sync_assigned_tasks_scanned`を入れる。`aggregate.csv`には各counterの中央値/MADと、run内で割り算してから集約したidle skip比率・handler到達比率を併記する。これらはframeあたりの値ではないため、比較時は同じmeasure秒数でのみ用いる。別々のcounterを独立に中央値化した値どうしを引き算して比率を作ってはならない。

`aggregate.csv`はframe sampleをrun間で混ぜず、各runのp50/p95/p99/maxを先に出し、その値の中央値とMADをcaseごとに出す。initial fixture checksum、warm-up checksum群、post-capture teardown warning件数も併記する。invalid runを黙って除外せず、session全体をinvalidにする。schema v2の既存artifactにはtask execution counterがなく、schema v3以前のartifactにはreservation sync counterがない。frame-time比較は可能だが、存在しないcounterを0としてM1以降と比較してはならない。

既存artifactの再集約と、互換なsession同士の比較には次を使う。

```bash
python3 scripts/perf.py summarize target/perf-runs/gather-intel-vulkan-seed-20260712

python3 scripts/perf.py compare \
  --baseline target/perf-runs/baseline \
  --candidate target/perf-runs/candidate \
  --metric p50 \
  --max-regression-pct 5
```

`summarize --warmup-checksum-policy record|require`は、既存artifactのCSV/log検証結果を保ったまま、以前に適用したwarm-up policyだけを再評価する。たとえば調査時の`require`失格を、標準の`record`へ戻して再集約できる。

`compare`はmatrixと実adapterが異なるsession、または各caseに3 valid runがないsessionを比較しない。異なるマシンの値は参考値として扱う。

正式matrixの一部caseだけを再測定する場合は、明示的に`--allow-case-subset`を付ける。この場合もworkload、seed、反復数、warm-up/measure秒数、checksum policy、custom population、計測mode、要求環境、実adapterは一致し、candidateのsize/renderがbaselineの部分集合でなければ失格にする。

```bash
python3 scripts/perf.py compare \
  --baseline target/perf-runs/full-baseline \
  --candidate target/perf-runs/large-cpu-candidate \
  --allow-case-subset \
  --metric p50
```

## 新しい workload への展開

別の最適化対象でも、同じrunnerとartifact契約を使う。新しいworkloadは、手操作や既存saveへ依存させず、次の順に追加・採取する。

1. `PerfWorkload`とscenario setupに名前・決定的な操作列・必要entity数を追加する。初期fixture checkpointより前に配置を完了し、master seedから専用substreamを分ける。
2. `--workload <name> --sizes small,medium,large --renders cpu,gpu`の短縮runを3反復し、initial fixture、実adapter/backend、marker前logが全て有効であることを確認する。失格artifactは削除せず残すが、比較値にはしない。
3. 標準の30秒warm-up / 60秒measure matrixを3反復する。frame-timeはCaptureだけ、system CPU timeはTracy、allocationはMemory、draw/passはRenderDocへ分ける。
4. 最適化前後は同じseed、population、window/backend、adapter、present mode、runner versionを使い、`compare`でcaseごとに比較する。workloadの意味やfixtureが変わった場合は新しいbaselineとして扱う。

marker前のwarning/errorは、allowlistへ追加して通すのではなく、発火したsystem・deferred command順・target/sourceの存続条件を特定してから修正する。特にBevy Relationship警告は「存在しないtargetへのinsert」を示すため、targetのdespawn処理だけでなく、同じmessage/deferred command batch内の後続insertも監査する。

## 直接実行のデバッグ

runnerを使わない調査時も、asset rootと出力先は必ず固定する。

```bash
cargo build --profile profiling -p bevy_app@0.1.0 --no-default-features --features profiling

BEVY_ASSET_ROOT="$PWD" \
WGPU_BACKEND=vulkan WGPU_ADAPTER_NAME=Intel \
HW_WINDOW_BACKEND=wayland HW_PRESENT_MODE=novsync \
target/profiling/bevy_app \
  --perf-scenario --perf-seed 20260712 --perf-size medium \
  --perf-workload gather --perf-render cpu \
  --perf-output-dir "$PWD/target/perf-runs/manual-debug/data"
```

直接実行はartifact manifest、adapter検証、反復集約を作らないため、最終比較には使用しない。
