# MudMixer Producer 段階分離リファクタ実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `mixer-producer-phase-separation-plan-2026-03-05` |
| ステータス | `Draft` |
| 作成日 | `2026-03-05` |
| 最終更新日 | `2026-03-05` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `producer/mixer.rs` が需要計算、採取タスク発行、request upsert を単一システムで実行し、保守性が低い。
- 到達したい状態: 需要計算フェーズ、CollectSand 補助フェーズ、request upsert フェーズを分割し、読みやすく安全に拡張できる構造にする。
- 成功指標:
  - `mud_mixer_auto_haul_system` の責務が明確な補助関数へ分離される。
  - request 発行/disable/despawn 条件が現行と一致。
  - `cargo check` 成功。

## 2. スコープ

### 対象（In Scope）

- `src/systems/logistics/transport_request/producer/mixer.rs` の関数分割。
- `upsert.rs` 共通ヘルパーへの置換可能箇所の統合。
- 計測ログと request profile 周辺の整理。

### 非対象（Out of Scope）

- MudMixer のゲーム仕様変更（容量、優先度、資材種）。
- CollectSand 判定ルールの仕様変更。

## 3. 現状とギャップ

- 現状:
  - 1システム内で多段分岐が続き、仕様追跡が困難。
  - `commands.spawn/try_insert` の重複が他 producer と同様に存在。
- 問題:
  - 軽微な条件変更でも差分が広範囲化する。
- 本計画で埋めるギャップ:
  - フェーズ別 API に分解し、変更点を局所化する。

## 4. 実装方針（高レベル）

- 方針:
  - `compute_mixer_desired_requests` / `issue_collect_sand_if_needed` / `upsert_mixer_requests` を中心に再構成。
  - 可能な範囲で `producer::upsert` を再利用し、spawn/insert 重複を削減。
- 設計上の前提:
  - request anchor は mixer entity を維持。
  - `TransportRequestKind` と `ResourceType` の対応契約は維持。
- Bevy 0.18 APIでの注意点:
  - Query 借用競合を避けるため、読み取り集計と Commands 更新を段階分離する。

## 5. マイルストーン

## M1: 需要計算と補助判定の分離

- 変更内容:
  - inflight 集計、active owner 解決、desired map 構築を補助関数へ抽出。
  - CollectSand の発行判定を独立関数化。
- 変更ファイル:
  - `src/systems/logistics/transport_request/producer/mixer.rs`
- 完了条件:
  - [ ] `mud_mixer_auto_haul_system` の主ループ分岐が削減される。
  - [ ] 既存 request 需要値が一致する。
- 検証:
  - `cargo check`

## M2: upsert 共通化の適用

- 変更内容:
  - 既存 request 更新/新規 request 生成を `producer/upsert.rs` ベースに寄せる。
  - 重複排除処理と disable 処理の共通パス化。
- 変更ファイル:
  - `src/systems/logistics/transport_request/producer/mixer.rs`
  - `src/systems/logistics/transport_request/producer/upsert.rs`（必要時）
- 完了条件:
  - [ ] spawn/try_insert の重複ブロックが削減される。
  - [ ] duplicate key 処理が既存同等で維持される。
- 検証:
  - `cargo check`

## M3: 整理とドキュメント同期

- 変更内容:
  - request profile/utility 関数の配置を最適化。
  - 必要に応じて logistics docs に実装境界を追記。
- 変更ファイル:
  - `src/systems/logistics/transport_request/producer/mixer.rs`
  - `docs/logistics.md`（必要時）
  - `docs/architecture.md`（必要時）
- 完了条件:
  - [ ] mixer producer の責務境界がコード上で明確。
  - [ ] `cargo check` 成功。
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 需要計算ロジックの移設ミス | request 発行漏れ/過剰発行 | 旧ロジックと key 単位で比較し段階移行 |
| upsert 共通化で kind/resource の対応崩れ | 誤 WorkType 発行 | `mixer_request_profile` 契約をテスト観点化 |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - Mixer への Sand/Water request が正常に発行される。
  - 需要 0 で disable/despawn が想定通り動く。
  - CollectSand 自動発行が過不足なく動作する。
- パフォーマンス確認（必要時）:
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1（分離）/M2（upsert置換）/M3（整理）を個別 revert 可能。
- 戻す時の手順:
  - 問題発生マイルストーンのみ revert。
  - `cargo check` と mixer シナリオを再実行。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1`,`M2`,`M3`

### 次のAIが最初にやること

1. 現行 `mud_mixer_auto_haul_system` の集計処理をフェーズ単位でコメント化。
2. desired request 構築を切り出し、挙動不変で `cargo check` を通す。
3. その後に upsert 共通化へ進む。

### ブロッカー/注意点

- `request_is_collect_sand_demand` の条件式は仕様境界なので不用意に変更しないこと。
- owner 解決 (`find_owner_for_position`) の優先ルールを維持すること。

### 参照必須ファイル

- `src/systems/logistics/transport_request/producer/mixer.rs`
- `src/systems/logistics/transport_request/producer/upsert.rs`
- `src/systems/logistics/transport_request/producer/mod.rs`
- `docs/logistics.md`

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
