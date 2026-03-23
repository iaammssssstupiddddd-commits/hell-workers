# デッドコード削除

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `dead-code-cleanup-2026-03-23` |
| ステータス | `Completed` |
| 作成日 | `2026-03-23` |
| 最終更新日 | `2026-03-23` |
| 作成者 | `Copilot` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: ワークスペース全体に散在する「未使用の`pub`関数」および「過剰な`pub`可視性の型」を除去・修正する
- 到達したい状態: 呼び出されない関数がなく、内部実装用の型が適切な最小可視性で隠蔽された状態
- 成功指標: `cargo check --workspace` がゼロエラーで通過し、削除した関数への参照が存在せず、公開API説明が実装と一致する

## 2. スコープ

### 対象（In Scope）

- 未使用のpub関数3件の削除
- `hw_logistics::transport_request::arbitration`モジュール内の9型を最小十分な可視性へ縮小
- 上記可視性変更に追従する `crates/hw_logistics/README.md` の更新

### 非対象（Out of Scope）

- ロジックの変更・リファクタリング
- テストコードの追加
- パフォーマンス最適化

## 3. 現状とギャップ

- 現状: 通常の `cargo check --workspace` では `pub` 関数の未使用は警告対象にならず、コード検索ベースでしか検出できていない
- 問題: `pub`であるために警告が出ないだけで、実際には一切呼び出されていない関数・不要に公開された内部型が存在する
- 本計画で埋めるギャップ: 手動grep調査で特定した12件の問題を修正し、コードベースを真にクリーンな状態にする

## 4. 実装方針（高レベル）

- 方針: 削除前に再度grep確認し、呼び出し元がゼロであることを確認してから削除する。可視性縮小は sibling module からの参照を壊さないことを前提に、必要なら `pub(crate)` も含めて再判定する
- 設計上の前提: `pub(super)` への変更候補は `arbitration/` 親モジュール配下だけで使われている。`super::types` / `super::{...}` 参照が壊れる場合は `pub(super)` に固定せず最小十分な可視性に留める
- Bevy 0.18 APIでの注意点: なし（Bevy APIへの変更なし）

## 5. マイルストーン

## M1: 未使用pub関数の削除（3件）

削除対象：

| # | ファイル | 関数名 |
|---|---------|--------|
| 1 | `crates/bevy_app/src/systems/command/mod.rs:22` | `to_logistics_zone_type` |
| 2 | `crates/hw_soul_ai/src/soul_ai/execute/task_execution/bucket_transport/abort.rs:38` | `drop_bucket_and_unassign` |
| 3 | `crates/hw_soul_ai/src/soul_ai/execute/task_execution/bucket_transport/guards.rs:7` | `has_bucket_in_inventory` |

- 変更ファイル:
  - `crates/bevy_app/src/systems/command/mod.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/bucket_transport/abort.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/bucket_transport/guards.rs`
- 完了条件:
  - [ ] 3関数の定義を削除
  - [ ] `cargo check --workspace` がエラーなし
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## M2: arbitration内部型の可視性縮小（9件）

変更対象（`pub` → 最小十分な可視性）：

| # | ファイル | 型名 |
|---|---------|------|
| 1 | `crates/hw_logistics/src/transport_request/arbitration/types.rs` | `BatchCandidate` |
| 2 | `crates/hw_logistics/src/transport_request/arbitration/types.rs` | `FreeItemSnapshot` |
| 3 | `crates/hw_logistics/src/transport_request/arbitration/types.rs` | `ItemBucketKey` |
| 4 | `crates/hw_logistics/src/transport_request/arbitration/types.rs` | `RequestEvalContext` |
| 5 | `crates/hw_logistics/src/transport_request/arbitration/types.rs` | `NearbyItem` |
| 6 | `crates/hw_logistics/src/transport_request/arbitration/types.rs` | `HeapEntry` |
| 7 | `crates/hw_logistics/src/transport_request/arbitration/grants.rs` | `GrantStats` |
| 8 | `crates/hw_logistics/src/transport_request/arbitration/mod.rs` | `WheelbarrowArbitrationRuntime` |
| 9 | `crates/hw_logistics/src/transport_request/arbitration/mod.rs` | `WheelbarrowArbitrationDirtyParams` |

- 変更ファイル:
  - `crates/hw_logistics/src/transport_request/arbitration/types.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/grants.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/mod.rs`
  - `crates/hw_logistics/README.md`
- 完了条件:
  - [ ] 9型の可視性を `pub(super)` または最小十分な可視性に変更
  - [ ] `crates/hw_logistics/README.md` の公開API説明が実装と一致
  - [ ] `cargo check --workspace` がエラーなし
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `drop_bucket_and_unassign` が将来使われる予定だった場合 | 機能欠落 | コードを読んで`abort_without_bucket`/`abort_with_bucket`で完全に代替可能か確認してから削除 |
| `pub(super)` への変更で sibling module からのアクセスがコンパイルエラーになる | ビルド失敗 | `arbitration/mod.rs` 配下の参照関係を確認し、`pub(super)` に固定せず `pub(crate)` を含めて最小十分な可視性を選ぶ |
| 公開API説明のREADMEを更新し忘れる | ドキュメントと実装の乖離 | `crates/hw_logistics/README.md` を M2 の変更ファイルに含め、完了条件に組み込む |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- 手動確認シナリオ: なし（ロジック変更なし）
- パフォーマンス確認: 不要

## 8. ロールバック方針

- どの単位で戻せるか: M1・M2それぞれ独立したgitコミットで管理し、個別にrevertできる
- 戻す時の手順: `git revert <commit-hash>`

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1（未使用関数削除）、M2（可視性縮小）

### 次のAIが最初にやること

1. 各対象ファイルを開き、削除・変更箇所を確認する
2. M1（3関数の削除）を実施し、`cargo check --workspace` で確認
3. M2（9型の可視性変更と README 更新）を実施し、`cargo check --workspace` で確認

### ブロッカー/注意点

- `arbitration/` サブモジュール内での相互参照がある場合、`pub(super)` ではなく `pub(crate)` が必要な場合がある
- サブモジュール構造を `crates/hw_logistics/src/transport_request/arbitration/` で確認すること

### 参照必須ファイル

- `crates/bevy_app/src/systems/command/mod.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/bucket_transport/abort.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/bucket_transport/guards.rs`
- `crates/hw_logistics/src/transport_request/arbitration/types.rs`
- `crates/hw_logistics/src/transport_request/arbitration/grants.rs`
- `crates/hw_logistics/src/transport_request/arbitration/mod.rs`
- `crates/hw_logistics/README.md`

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-03-23` / 未実施（計画作成時点ではコード検索ベースの調査のみ）
- 未解決エラー: なし

### Definition of Done

- [ ] 未使用pub関数3件が削除済み
- [ ] arbitration内部型9件が最小十分な可視性に変更済み
- [ ] `crates/hw_logistics/README.md` の公開API説明が更新済み
- [ ] `cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-23` | `Copilot` | 初版作成（デッドコード調査結果に基づく） |
| `2026-03-23` | `Codex` | レビュー指摘を反映。無効な `cargo check -W dead-code` 記述を修正し、`--workspace` 検証と `crates/hw_logistics/README.md` 更新を計画に追加。 |
