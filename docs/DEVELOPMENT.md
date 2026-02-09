# Development Guide (for AI & Humans)

本プロジェクトを開発・保守する上での重要なガイドラインです。

## 開発サイクル

1.  **Planning**: `implementation_plan.md` を作成し、ユーザーの承認を得る。
2.  **Execution**: コードを実装し、`cargo check` で型安全性を確認する。
3.  **Verification**: 動作を確認し、`walkthrough.md` で成果を報告する。

## 開発ルール

### 1. Rust-analyzer 診断の厳守
- コンパイルエラー（赤い波線）を一つも残したまま完了報告をしてはいけない。
- `cargo check` が通ることを必ず確認する。

### 2. 死蔵コードの禁止 ([deadcode.md])
- 将来使う予定があっても、現在使われていないコードや `#[allow(dead_code)]` は残さない。

### 3. 画像生成と透過 PNG ([image-generation.md])
- アイコン等は `generate_image` で背景をマゼンタ (`#FF00FF`) にして生成する。
- `scripts/convert_to_png.py` を使用して透過 PNG に変換する。
- 変換後はバイナリ署名を確認する： `89-50-4E-47-0D-0A-1A-0A`

### 4. 型変更とメッセージ初期化の規約
型不一致や二重借用エラーが長引きやすいため、以下を必ず守る。

- 型変更の順番は `定義 -> 生成 -> 使用` を固定する
  例: `entities` の `struct/enum` を更新してから、`spawn/build` 側、最後に `systems` の `Query` を更新する。
- 変換は `From/Into` に統一し、`as` の多用を避ける
  変換地点を明確にして、型ミスの原因位置を特定しやすくする。
- `Messages<T>`/`Events<T>` は専用プラグインで集中初期化する
  `src/plugins/messages.rs` などに集約し、`build()` 冒頭で `add_message::<T>()`/`add_event::<T>()` を登録する。
- 初期化漏れに備えて `Option<Messages<T>>` か `If<Messages<T>>` を検討する
  使わないフレームでもパニックしない形にしておく。

### 5. EntityEvent Observer 登録の規約
- `EntityEvent` のオブザーバーは、原則として Plugin 側の `app.add_observer(...)` に一元登録する。
- 同じハンドラをスポーン時の `.observe(...)` と併用しない（重複実行の原因になる）。
- 例外として、特定エンティティにのみ限定した監視が必要な場合に限り `.observe(...)` を使う。

### 6. 予約（Reservation）実装の規約
物流・自動補充の競合を防ぐため、予約の責務と解除タイミングを明確にする。

- 予約責務は「発行時」か「割り当て時」のどちらか一方に統一する。
  - 自動発行時に予約を確定する場合は `ReservedForTask` を付与し、割り当て時の同種予約を重複発行しない。
- 共有ソースを消費するタスク（例: Tank からの取水）は、処理中に `ReserveSource` でロックし、成功/失敗/中断の全経路で解除する。
- フェーズ移行で不要になったロックは即時解除し、`unassign_task` 側でもフェーズに応じて解放漏れを防ぐ。
- `sync_reservations_system` の再構築条件は、実行フェーズの予約寿命と一致させる（フェーズ定義を変更したら同時更新する）。

### 7. 自動運搬 request 方式（Mixer）の規約
ポーリングの全域走査を避けるため、Mixer 向け固体運搬は「アイテム直接 Designation」ではなく「搬入先アンカー request」を使う。

- request エンティティ（`MixerHaulRequest`）に `Designation(HaulToMixer)` を付与し、`TargetMixer` と `TaskSlots` で需要を表現する。
- request 発行時にソース資材を探索しない。ソース選定は Familiar の割り当て時に遅延解決する。
- request は `mixer + resource_type` 単位で再利用し、需要 0 のときは `Designation` を外して休止する（不要増殖を避ける）。
- 実行系は request 由来タスクでも成立するようにするが、既存のアイテム直接方式のキャンセル条件（Designation除去検知）は維持する。

## 便利なコマンド

### コンパイル確認
```powershell
cargo check
```

### 画像変換
```powershell
python scripts/convert_to_png.py "source_path" "assets/textures/dest.png"
```

### PNG署名確認
```powershell
powershell -Command "[BitConverter]::ToString((Get-Content 'file_path' -Encoding Byte -TotalCount 8))"
```

### 高負荷パフォーマンス計測（500 Soul / 30 Familiar）
```powershell
cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario --perf-log-fps
```

- `--spawn-souls`: 初期 Soul 数を上書き（既定: 10）
- `--spawn-familiars`: 初期 Familiar 数を上書き（既定: 2）
- `--perf-scenario`: 収集シナリオを自動セットアップ（TaskArea / command / designation）
- `--perf-log-fps`: `PERF_FPS` ログを1秒ごとに出力
- 環境変数でも指定可: `HW_SPAWN_SOULS`, `HW_SPAWN_FAMILIARS`, `HW_PERF_SCENARIO=1`

## トラブルシューティング

### 1. Windows でのリンクエラー (too many exported symbols)
Windows の PE 形式では、一つの DLL からエクスポートできるシンボル数が 65,535 に制限されています。Bevy の `dynamic_linking` 機能を使用するとこの制限を超えやすいため、エラーが出る場合は以下の対応を行ってください。
- `Cargo.toml` の `default` features から `dynamic_linking` を削除し、静的リンクでビルドする。
- 静的リンクであってもデバッグビルドが遅い場合は、依存関係の `opt-level` を 3 に設定したままにする。

### 2. File Lock エラー
`cargo` コマンドが「Blocking waiting for file lock」で止まる場合は、別のターミナルや IDE、あるいはゲーム自体が `target/` ディレクトリを使用中（ロック中）です。それらを終了してから再度実行してください。
