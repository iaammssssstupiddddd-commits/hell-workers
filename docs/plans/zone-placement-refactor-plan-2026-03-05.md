# Zone Placement/Removal 分割リファクタ実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `zone-placement-refactor-plan-2026-03-05` |
| ステータス | `Done` |
| 作成日 | `2026-03-05` |
| 最終更新日 | `2026-03-05` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `zone_placement.rs` が配置・削除・プレビュー・連結判定を1ファイルで抱え、変更衝突と見通し悪化を招いている。
- 到達したい状態: Zone 配置系と削除系を責務単位で分割し、機能追加時の変更範囲を局所化する。
- 成功指標:
  - 配置/削除の主要ロジックが別モジュールに分離される。
  - 既存挙動（配置条件、削除時フラグメント判定、プレビュー色）が維持される。
  - `cargo check` 成功。

## 2. スコープ

### 対象（In Scope）

- `src/systems/command/zone_placement.rs` の責務分割。
- Zone removal preview と removal target 判定ロジックの抽出。
- 必要に応じた `mod.rs` / plugin 側参照の更新。

### 非対象（Out of Scope）

- Zone仕様変更（判定条件や UI 操作の仕様変更）。
- Shift 継続配置の新機能追加（既存 TODO の解消は別タスク）。

## 3. 現状とギャップ

- 現状:
  - 配置 (`zone_placement_system`) と削除 (`zone_removal_system`) が同居。
  - `identify_removal_targets` が preview と確定削除の両方から呼ばれる。
- 問題:
  - 変更箇所特定に時間がかかり、回帰リスクが高い。
- 本計画で埋めるギャップ:
  - `placement` / `removal` / `removal_preview` / `geometry` などに分離し、責務境界を明確化する。

## 4. 実装方針（高レベル）

- 方針:
  - まずファイル分割のみ行い、挙動を変えない。
  - 共通関数（`world_cursor_pos` など）は shared util に寄せる。
- 設計上の前提:
  - `TaskMode::ZonePlacement/ZoneRemoval` と `TaskContext` の契約は維持する。
  - `AreaBounds` / `WorldMap` の計算方式は現状踏襲。
- Bevy 0.18 APIでの注意点:
  - `Query`/`ResMut` の借用競合を分割後も発生させない。
  - system 登録順は現行を維持する。

## 5. マイルストーン

## M1: ファイル分割の骨格作成

- 変更内容:
  - `zone_placement/` ディレクトリ化し `mod.rs` へ移行。
  - 配置系と削除系の public API を維持。
- 変更ファイル:
  - `src/systems/command/zone_placement.rs`（削除 or 縮小）
  - `src/systems/command/zone_placement/mod.rs`
  - `src/systems/command/zone_placement/placement.rs`
  - `src/systems/command/zone_placement/removal.rs`
- 完了条件:
  - [ ] 既存 import が壊れずビルドできる。
  - [ ] 旧ファイル依存が除去される。
- 検証:
  - `cargo check`

## M2: removal preview / connectivity 判定の抽出

- 変更内容:
  - `identify_removal_targets` と preview 更新を専用モジュールへ分離。
  - `ZoneRemovalPreviewState` の責務を preview 管理に限定。
- 変更ファイル:
  - `src/systems/command/zone_placement/removal.rs`
  - `src/systems/command/zone_placement/removal_preview.rs`
  - `src/systems/command/zone_placement/connectivity.rs`
- 完了条件:
  - [ ] preview 表示と確定削除の結果が一致する。
  - [ ] フラグメント削除判定が既存通り動作する。
- 検証:
  - `cargo check`

## M3: 参照整理とドキュメント更新

- 変更内容:
  - plugin/system 登録側の参照パスを整理。
  - 必要なら docs の参照先を更新。
- 変更ファイル:
  - `src/systems/command/mod.rs`
  - `docs/architecture.md`（必要時）
  - `docs/tasks.md`（必要時）
- 完了条件:
  - [ ] 参照パスが新構成に一致。
  - [ ] `cargo check` 成功。
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 分割時の import/公開範囲ミス | ビルド失敗 | `pub(crate)` 範囲を段階的に調整し M1 で固定 |
| preview と確定削除ロジックの乖離 | 見た目と実挙動の不一致 | 判定関数を単一化して両経路で再利用 |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - Stockpile 配置（Yard 内/外）。
  - Zone 削除ドラッグ時の direct/fragment 色分け。
  - 削除確定で最大クラスタのみ残ること。
- パフォーマンス確認（必要時）:
  - 不要（挙動互換の構造変更が目的）。

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1/M2/M3 のコミット単位で revert 可能。
- 戻す時の手順:
  - 直近マイルストーンのみ `git revert`。
  - `cargo check` と手動削除シナリオを再確認。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1`,`M2`,`M3`

### 次のAIが最初にやること

1. `zone_placement.rs` の public 関数境界を洗い出す。
2. `placement` と `removal` を先に分割して `cargo check` を通す。
3. `identify_removal_targets` を preview/確定の共通 API に固定する。

### ブロッカー/注意点

- `TaskMode` 遷移（右クリックキャンセル）挙動を変えないこと。
- preview 色の定数値を不用意に変更しないこと。

### 参照必須ファイル

- `src/systems/command/zone_placement.rs`
- `src/systems/command/mod.rs`
- `docs/architecture.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-05` / `not run`（計画書作成のみ）
- 未解決エラー: `N/A`

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-05` | `Codex` | 初版作成 |
