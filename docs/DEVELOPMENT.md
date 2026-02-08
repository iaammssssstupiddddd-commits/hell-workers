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
