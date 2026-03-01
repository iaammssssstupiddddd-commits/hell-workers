# Zone Removal Preview 差分更新化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `zone-removal-preview-diff-plan-2026-03-01` |
| ステータス | `Draft` |
| 作成日 | `2026-03-01` |
| 最終更新日 | `2026-03-01` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Zone removal preview が毎フレーム全 stockpile の色をリセットしており、規模拡大時に不要処理が増える。
- 到達したい状態: 前フレームとの差分だけ色変更するプレビュー更新へ移行する。
- 成功指標:
  - プレビュー更新が差分反映ベースになる。
  - `FIXME: パフォーマンス最適化` を解消できる。
  - `cargo check` 成功。

## 2. スコープ

### 対象（In Scope）

- `zone_placement.rs` の preview 更新ロジック改善。
- 前回ハイライト状態を保持する `Resource`（仮称）導入。
- キャンセル/確定時のクリーンアップ整備。

### 非対象（Out of Scope）

- Zone placement 全体の仕様変更。
- 連結成分分析アルゴリズムの最適化（`identify_removal_targets` 自体）。
- 見た目の色設計変更。

## 3. 現状とギャップ

- 現状:
  - `update_removal_preview` で全 stockpile を毎回 default 色へ戻す。
- 問題:
  - stockpile 数増加時に無駄な `Query<&mut Sprite>` 更新が増える。
  - フレームごとの処理負荷がドラッグ面積に関係なく高止まりする。
- 本計画で埋めるギャップ:
  - 前回集合との差分適用（追加/削除/色変更対象のみ更新）へ移行する。

## 4. 実装方針（高レベル）

- 方針:
  - `ZoneRemovalPreviewState`（仮称）に前回の `direct/fragments` 集合を保持。
  - 現在集合との差分を計算し、必要な entity のみ色更新。
  - 確定/キャンセル時は state と色を同期リセット。
- 設計上の前提:
  - 色の意味は現状維持（default/赤/橙）。
  - `identify_removal_targets` の返り値契約は維持。
- Bevy 0.18 APIでの注意点:
  - `ResMut` と `Query<&mut Sprite>` の同時利用で借用競合を起こさないよう、差分計算と反映を段階分離する。

## 5. マイルストーン

## M1: 差分更新の状態管理導入

- 変更内容:
  - 前回プレビュー対象を保持する `Resource` 追加。
  - reset 処理を state ベースに変更。
- 変更ファイル:
  - `src/systems/command/zone_placement.rs`
  - `src/plugins/*`（resource 初期化が必要な場合）
- 完了条件:
  - [ ] 前回対象集合を保持できる
  - [ ] 全件リセットを回避できる土台がある
- 検証:
  - `cargo check`

## M2: 差分適用ロジック実装

- 変更内容:
  - 現在集合と前回集合の差分計算を追加。
  - 追加/削除/分類変更に応じた色更新を実装。
- 変更ファイル:
  - `src/systems/command/zone_placement.rs`
- 完了条件:
  - [ ] プレビュー更新が差分のみで完結
  - [ ] 色表示が既存仕様と一致
- 検証:
  - `cargo check`

## M3: クリーンアップと観測強化

- 変更内容:
  - 確定/キャンセル時の state クリアを厳密化。
  - 必要に応じて debug ログで更新件数を観測可能にする。
- 変更ファイル:
  - `src/systems/command/zone_placement.rs`
  - `docs/logistics.md`（必要時）
- 完了条件:
  - [ ] 残留ハイライトが起きない
  - [ ] `cargo check` 成功
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 差分集合の管理漏れ | ハイライト残留 | 確定/キャンセル/右クリック経路ごとに state clear を実装する |
| 分類変更（direct↔fragment）の漏れ | 色不整合 | 3集合（追加/削除/分類変更）を個別処理する |
| state と world_map の不整合 | 無効 entity 更新 | `world_map.stockpiles.get` の存在確認を維持する |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - ZoneRemoval でドラッグ範囲を連続変更した際の色更新。
  - 左クリック確定、右クリックキャンセル時の色復元。
  - 大きな stockpile 群での操作感確認。
- パフォーマンス確認（必要時）:
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`

## 8. ロールバック方針

- どの単位で戻せるか:
  - `zone_placement.rs` 単位で戻せる。
- 戻す時の手順:
  - 差分更新コミットを revert。
  - 全件リセット版に戻した上で `cargo check` を実行。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M3 未着手

### 次のAIが最初にやること

1. 現行 `update_removal_preview` の入出力を整理する。
2. `ZoneRemovalPreviewState` を追加して前回集合を保持する。
3. 差分更新へ置換し、確定/キャンセル時の reset を検証する。

### ブロッカー/注意点

- `identify_removal_targets` が返す `Vec` の順序は不定なので、集合比較は `HashSet` ベースで実装する。

### 参照必須ファイル

- `src/systems/command/zone_placement.rs`
- `docs/DEVELOPMENT.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-01` / `pass`
- 未解決エラー: なし（計画作成時点）

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-01` | `Codex` | 初版作成 |
