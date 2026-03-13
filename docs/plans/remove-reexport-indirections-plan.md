# 不要な再エクスポート間接層の除去

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `remove-reexport-indirections-plan-2026-03-13` |
| ステータス | `Done` |
| 作成日 | `2026-03-13` |
| 最終更新日 | `2026-03-13` |
| 作成者 | Claude |
| 関連提案 | N/A |
| 関連Issue/PR | N/A |

## 1. 目的

- 解決したい課題: `bevy_app` クレート内に存在する2つの意図が不明瞭な再エクスポートが、不要な間接層を作っている
- 到達したい状態: 各ファイルが依存元（`bevy` または `hw_core`）から直接 import する
- 成功指標: `cargo check` がエラーなしで通る

## 2. スコープ

### 対象（In Scope）

- `crates/bevy_app/src/interface/mod.rs` から `PanCamera` / `PanCameraPlugin` の再エクスポートを削除
- `crates/bevy_app/src/main.rs` から `pub mod relationships { pub use hw_core::relationships::*; }` を削除
- 上記削除に伴い影響を受ける全ファイルの import を修正

### 非対象（Out of Scope）

- `hw_ai/src/lib.rs` の Plugin 集約（正当なファサードのためそのまま）
- `interface::camera::MainCamera` の再エクスポート（20+ ファイルが使用しており妥当）

## 3. 現状とギャップ

- 現状:
  - `interface::camera` が `bevy::camera_controller::pan_camera::{PanCamera, PanCameraPlugin}` を再エクスポートしているが、使用箇所は2ファイルのみ
  - `main.rs` が `pub mod relationships { pub use hw_core::relationships::*; }` を定義し、31 ファイルが `crate::relationships::*` 経由で参照している
- 問題:
  - `PanCamera*` 再エクスポート: ファサードのメリットを得られるほどの使用箇所がなく、読む人に「なぜここで再エクスポートしているのか」という疑問を生む
  - `relationships` モジュール: バイナリクレートの `main.rs` に `pub mod` を置いても外部公開の効果がない。`bevy_app` はすでに `hw_core` に依存しているため直接 import できる
- 本計画で埋めるギャップ: 間接層を除去し、import の意図を明確化する

## 4. 実装方針（高レベル）

- 方針: 2つのマイルストーンに分割して独立して対応する。各マイルストーン後に `cargo check` で検証する
- 設計上の前提: `bevy_app` は `hw_core` と `bevy` 両方にすでに依存しているため、新たな `Cargo.toml` 変更は不要
- Bevy 0.18 API での注意点: `PanCamera` / `PanCameraPlugin` のモジュールパスが `bevy::camera_controller::pan_camera` であることを確認済み

## 5. マイルストーン

## M1: `interface::camera` から `PanCamera*` を削除

- 変更内容: `interface/mod.rs` の `pub use bevy::camera_controller::pan_camera::...` を削除し、使用している2ファイルで直接 import に変更
- 変更ファイル:
  - `crates/bevy_app/src/interface/mod.rs`
  - `crates/bevy_app/src/plugins/input.rs`（`PanCamera`, `PanCameraPlugin` を `bevy::camera_controller::pan_camera` から直接 import）
  - `crates/bevy_app/src/plugins/startup/mod.rs`（`PanCamera` を直接 import）
- 完了条件:
  - [ ] `interface/mod.rs` の `pub use bevy::camera_controller::pan_camera::...` 行が削除されている
  - [ ] `input.rs` / `startup/mod.rs` が直接 `bevy` から import している
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

## M2: `main.rs` の `relationships` モジュールを削除

- 変更内容: `main.rs` の `pub mod relationships { ... }` を削除し、31 ファイルの `use crate::relationships::*` を `use hw_core::relationships::*` に一括置換
- 変更ファイル:
  - `crates/bevy_app/src/main.rs`
  - `crate::relationships` を使用している 31 ファイル（`grep -r "use crate::relationships"` で特定）
- 完了条件:
  - [ ] `main.rs` の `pub mod relationships` ブロックが削除されている
  - [ ] `crate::relationships` の参照がコードベース内に残っていない
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `crate::relationships` の参照を見落とす | コンパイルエラー | M2 完了後に `cargo check` で即検出可能 |
| `PanCamera` のモジュールパスが変わっている | コンパイルエラー | `cargo check` で即検出可能 |

## 7. 検証計画

- 必須:
  - 各マイルストーン後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動確認シナリオ: なし（コンパイル検証のみで十分）
- パフォーマンス確認: 不要

## 8. ロールバック方針

- どの単位で戻せるか: M1 / M2 はそれぞれ独立しているため、git revert でマイルストーン単位で戻せる
- 戻す時の手順: `git diff` で対象ファイルを確認し、変更前の import に戻す

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1, M2 ともに未着手

### 次のAIが最初にやること

1. M1: `interface/mod.rs` の `pub use bevy::camera_controller::pan_camera::...` を削除
2. M1: `plugins/input.rs` と `plugins/startup/mod.rs` を直接 `bevy` import に修正
3. M1 後に `cargo check` → M2 に進む

### ブロッカー/注意点

- `crate::relationships` の参照ファイルは31個あるため、エディタの一括置換か `sed` を使うと効率的
- `main.rs` の `pub mod relationships` ブロック削除後は `use hw_core::relationships::*` に揃えること

### 参照必須ファイル

- `crates/bevy_app/src/interface/mod.rs`
- `crates/bevy_app/src/main.rs`
- `crates/bevy_app/src/plugins/input.rs`
- `crates/bevy_app/src/plugins/startup/mod.rs`

### 最終確認ログ

- 最終 `cargo check`: 未実施
- 未解決エラー: なし（着手前）

### Definition of Done

- [ ] M1, M2 が完了
- [ ] `cargo check` が成功
- [ ] `crate::relationships` の参照がコードベースに残っていない

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-13` | Claude | 初版作成 |
