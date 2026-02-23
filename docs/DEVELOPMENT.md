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

### 7. TransportRequest の規約（M3〜M7 完了）
運搬系は全て **Anchor Request パターン** に統一済み。request エンティティをアンカー位置（Blueprint/Mixer/Stockpile）に生成し、割り当て時にソースを遅延解決する。

- **request 化済み**: `DepositToStockpile`, `DeliverToBlueprint`, `DeliverToFloorConstruction`, `DeliverToWallConstruction`, `DeliverToProvisionalWall`, `DeliverToMixerSolid`, `DeliverWaterToMixer`, `GatherWaterToTank`, `ReturnBucket`, `ReturnWheelbarrow`, `BatchWheelbarrow`, `ConsolidateStockpile`
- `task_finder` は `DesignationSpatialGrid` と `TransportRequestSpatialGrid` の両方から候補を収集。
- 運搬系 WorkType（`Haul`, `HaulToMixer`, `GatherWater`, `HaulWaterToMixer`, `WheelbarrowHaul`）は request 付き候補のみを扱う。
- request は需要 0 のとき `Designation` を外して休止、または despawn。
- アンカー消失時は `transport_request_anchor_cleanup_system` で request を close。

### 8. 割り当て・搬送・UIの実装境界

- Familiar の割り当て発行は `src/systems/familiar_ai/decide/task_management/builders/mod.rs` の `submit_assignment(...)` を必ず経由する（`ReservationShadow` 反映を保証するため）。
- `TaskAssignmentQueries` は `TaskAssignmentReadAccess` を内包する構成になっている。Familiar 側の型参照は `task_management::FamiliarTaskAssignmentQueries` を優先し、`soul_ai` 実装詳細への直接依存を増やさない。
- `apply_task_assignment_requests_system` を拡張する場合は、既存の責務分離ヘルパー（受理判定 / idle正規化 / 予約反映 / DeliveringTo / イベント）へ追記し、単一関数へ責務を戻さない。
- `pathfinding_system` の変更は補助関数（再利用判定・再探索・休憩フォールバック・失敗時処理）単位で行い、分岐をインラインで肥大化させない。
- floor/wall の搬入同期変更は `src/systems/logistics/transport_request/producer/mod.rs` の共通ヘルパー（`group_tiles_by_site`, `consume_waiting_tile_resources`）を再利用して重複実装を避ける。
- UI/Visual の更新は `src/interface/ui/interaction/status_display/` と `src/systems/visual/dream/ui_particle/` の責務分割単位で行い、再び単一巨大ファイルに戻さない。

### 9. docs 直下ドキュメントの記述ルール

- `docs/*.md`（`plans/` と `proposals/` を除く）は、作業報告ではなく仕様・設計・運用ルールの説明を目的とする。
- 「対応済み」「実装完了」「今回の変更」など、時点依存の進捗/報告表現は書かない。
- 実施ログ・進捗・作業メモは PR 説明、Issue、または `docs/plans/` / `docs/proposals/` に記載する。
- 挙動変更を伴う実装時は、関連する仕様文書と `docs/README.md` の参照関係を同時に更新する。

### 10. MCP（rust-analyzer-mcp / docsrs-mcp）活用フロー

- 目的:
  - `rust-analyzer-mcp`: ローカルコードの型・参照・定義を正確に把握する。
  - `docsrs-mcp`: 外部 crate API（特に Bevy 0.18）のシグネチャと仕様を一次情報で確認する。
- 実装前:
  - 変更対象シンボルは `rust-analyzer-mcp` で定義・参照・関連型を確認する。
  - 外部 API を使う箇所は `docsrs-mcp` で対象バージョンの関数シグネチャを確認する。
- 実装中:
  - ローカル依存関係の追跡は `rust-analyzer-mcp` を優先する。
  - API 仕様確認は `docsrs-mcp` を優先し、推測でメソッド名や引数を書かない。
  - Bevy API は必ず 0.18 系のドキュメント/シグネチャで確認する。
- 実装後:
  - rust-analyzer 診断を確認し、`cargo check` を必ず実行する。
  - MCP の結果と実コードが不一致の場合は、`~/.cargo/registry/src/` の実ソースを確認して整合を取る。
- MCP が使えない場合の代替:
  - `~/.cargo/registry/src/` のクレートソースと `docs.rs` の一次情報で確認する。

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
cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario
```

- `--spawn-souls`: 初期 Soul 数を上書き（既定: 10）
- `--spawn-familiars`: 初期 Familiar 数を上書き（既定: 2）
- `--perf-scenario`: 収集シナリオを自動セットアップ（TaskArea / command / designation）
- 環境変数でも指定可: `HW_SPAWN_SOULS`, `HW_SPAWN_FAMILIARS`, `HW_PERF_SCENARIO=1`

## トラブルシューティング

### 1. Windows でのリンクエラー (too many exported symbols)
Windows の PE 形式では、一つの DLL からエクスポートできるシンボル数が 65,535 に制限されています。Bevy の `dynamic_linking` 機能を使用するとこの制限を超えやすいため、エラーが出る場合は以下の対応を行ってください。
- `Cargo.toml` の `default` features から `dynamic_linking` を削除し、静的リンクでビルドする。
- 静的リンクであってもデバッグビルドが遅い場合は、依存関係の `opt-level` を 3 に設定したままにする。

### 2. File Lock エラー
`cargo` コマンドが「Blocking waiting for file lock」で止まる場合は、別のターミナルや IDE、あるいはゲーム自体が `target/` ディレクトリを使用中（ロック中）です。それらを終了してから再度実行してください。

### 3. Bevy ECS `error[B0001]`（Query 競合パニック）
`cargo run` で `error[B0001]` が出る場合、同一システム内で Query のアクセス競合（例: `&mut T` と別 Query の `&T`）が発生しています。

- 原因調査: `cargo run --features bevy/debug` で実行し、衝突した system/query 名を表示して特定する。
- 修正方針: `Without<T>` で Query を排他的に分離するか、`ParamSet` に統合して同時借用を避ける。
- 既存共通クエリ（`TaskQueries` など）がある箇所では、同種コンポーネントへの重複 Query を新設しない。
