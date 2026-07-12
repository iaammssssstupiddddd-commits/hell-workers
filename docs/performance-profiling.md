# パフォーマンス計測

ランタイム最適化の前後比較は、同一のseed・負荷・描画条件で行う。通常の開発実行と計測実行を混在させず、計測用のCSV・Tracy capture・RenderDoc captureはそれぞれ別のrunで取得する。

## 標準ベースライン

```bash
HW_PRESENT_MODE=novsync cargo run --profile profiling -p bevy_app@0.1.0 --no-default-features --features profiling -- --perf-scenario --perf-seed 20260712 --perf-size medium --perf-workload gather --perf-render cpu
```

`--perf-scenario`は自動計測を有効化する。Warmup 30 virtual秒の後に60 virtual秒を採取し、CSVを書き出して正常終了する。比較時は同じコマンドを最低3回実行し、中央値とrun間の分散を残す。

## シナリオ設定

| オプション | 内容 |
| --- | --- |
| `--perf-seed <u64>` | worldgen・Soul配置・Familiar配置のmaster seed |
| `--perf-size small|medium|large` | `50/4`、`200/12`、`500/30` のSoul/Familiar数 |
| `--spawn-souls <n>` / `--spawn-familiars <n>` | size presetより優先する明示的な個体数 |
| `--perf-workload gather|path-door|construction|ui-gpu` | 比較対象の負荷ラベル。現時点で自動セットアップ済みなのは`gather` |
| `--perf-render cpu|gpu` | `cpu`は3D RtTと関連toggleを無効化、`gpu`は3D/mask/terrain/scene objectを基準状態で有効化 |

seedの優先順位は`--perf-seed`または`HW_PERF_SEED`、`HELL_WORKERS_WORLDGEN_SEED`、起動時乱数の順である。perf実行時のmaster seedはworldgenとSoul/Familiar配置用の独立乱数列へ分配する。通常起動は既存のworldgen環境変数とランダムseedの挙動を維持する。

`--perf-workload`の`path-door`、`construction`、`ui-gpu`は、専用の操作列と負荷構築を追加するまでCSVの比較対象に使わない。未実装workloadをGather結果として記録しない。

## 出力と確認項目

計測runは次のディレクトリへ出力する。

```text
target/perf/<workload>-<size>-<render>-seed-<seed>/
```

- `frames.csv`: frame indexとframe time（ms）
- `summary.csv`: p50/p95/p99、初期Soul/Familiar/Designation数、state checksum、Familiar delegation（実行時間・処理Familiar数・source selector走査数・reachability cache呼び出し数）の計測値

同一seedを3回実行し、worldgen seed、state checksum、初期個体数、Designation数が一致することを先に確認する。一致しないrunは性能値の比較に使わない。

`--perf-render cpu`ではmain/mask/shadowを含む3D scene objectが非表示になる。GPU比較では`--perf-render gpu`を使い、固定camera・固定DirectionalLight本数でRenderDocのpass時間とdraw/scene entity数を採取する。texture sample数はshaderの静的なsample数を記録し、利用可能な場合だけGPU vendor profilerの値を併記する。

## Tracy memory

allocation/frameを採取するときは、通常のframe time runと分けて次を使用する。

```bash
HW_PRESENT_MODE=novsync cargo run --profile profiling -p bevy_app@0.1.0 --no-default-features --features profiling-memory -- --perf-scenario --perf-seed 20260712 --perf-size medium --perf-workload gather --perf-render cpu
```

raw trace、RenderDoc capture、生成したCSVはcommitしない。比較結果の要約だけを最適化PRまたは対応する計画書へ記録する。
