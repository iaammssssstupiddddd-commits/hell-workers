# Dream UI Particle Update 分割リファクタ実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `dream-ui-particle-update-refactor-plan-2026-03-05` |
| ステータス | `Draft` |
| 作成日 | `2026-03-05` |
| 最終更新日 | `2026-03-05` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `visual/dream/ui_particle/update.rs` の `update_standard_particle` が長大で、挙動調整時の回帰範囲が広い。
- 到達したい状態: 物理計算、見た目更新、到達判定、trail生成を独立関数へ分離し、安全に調整できる構造にする。
- 成功指標:
  - 標準更新処理が複数の明確な補助関数に分離される。
  - 粒子挙動（移動、収束、吸収、trail）が現行互換で維持される。
  - `cargo check` 成功。

## 2. スコープ

### 対象（In Scope）

- `src/systems/visual/dream/ui_particle/update.rs` の責務分割。
- 補助データ構造（必要時）の導入。
- `ui_particle.rs` の re-export 維持。

### 非対象（Out of Scope）

- 演出仕様変更（色、速度、定数値の再設計）。
- 新エフェクト追加。

## 3. 現状とギャップ

- 現状:
  - 標準更新1関数で force 計算から material 更新、到達処理までを実行。
- 問題:
  - 条件分岐が多く、小変更でも副作用範囲が見えにくい。
- 本計画で埋めるギャップ:
  - `physics` / `visual` / `arrival` / `trail` の境界をコードで明確化する。

## 4. 実装方針（高レベル）

- 方針:
  - `compute_forces`, `integrate_particle`, `update_particle_visual`, `handle_arrival`, `emit_trail`（仮称）へ段階分割。
  - 既存定数と閾値は据え置く。
- 設計上の前提:
  - merge 処理 (`update_merging_particle`) の契約は維持。
  - `DreamBubbleUiMaterial` 更新タイミングは現状維持。
- Bevy 0.18 APIでの注意点:
  - `Node` / `Transform` / `MaterialNode` 更新順を保持。
  - `Commands` child 追加順で UI 表示層が変わらないようにする。

## 5. マイルストーン

## M1: 標準更新ロジックの段階分割

- 変更内容:
  - 標準更新処理を計算段階ごとに関数化。
  - 変数の受け渡しを struct 化（必要時）。
- 変更ファイル:
  - `src/systems/visual/dream/ui_particle/update.rs`
- 完了条件:
  - [ ] `update_standard_particle` の行数・分岐密度が減少。
  - [ ] 既存パラメータ値が維持される。
- 検証:
  - `cargo check`

## M2: 到達判定と trail 生成の境界固定

- 変更内容:
  - 到達時の icon pulse 処理を専用関数へ抽出。
  - trail 生成条件を専用関数へ切り出し。
- 変更ファイル:
  - `src/systems/visual/dream/ui_particle/update.rs`
- 完了条件:
  - [ ] 到達時挙動（吸収カウント増加）が維持。
  - [ ] trail 生成頻度が既存同等。
- 検証:
  - `cargo check`

## M3: docs 整理（必要時）

- 変更内容:
  - 実装境界の変更点を `docs/dream.md` に反映（必要時）。
  - 保守向けコメントを追加（最小限）。
- 変更ファイル:
  - `docs/dream.md`（必要時）
- 完了条件:
  - [ ] 実装境界が docs と一致。
  - [ ] `cargo check` 成功。
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 分割時の更新順変更 | 粒子挙動の差異 | 既存順序をコメント化して順番固定 |
| 受け渡し変数の欠落 | 速度/サイズ計算バグ | まず純抽出（挙動不変）で段階適用 |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - Dream 増加時に粒子が出現し icon へ収束する。
  - 合体時にサイズ・フェードが崩れない。
  - trail が途切れず過剰発生もしない。
- パフォーマンス確認（必要時）:
  - 高負荷時の UI 負荷を目視確認。

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1/M2/M3 のコミット単位で戻せる。
- 戻す時の手順:
  - 問題発生段階を revert。
  - `cargo check` と Dream 粒子手動確認を再実施。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1`,`M2`,`M3`

### 次のAIが最初にやること

1. `update_standard_particle` 内を「force」「integration」「visual」「arrival」「trail」に区切って印付け。
2. M1 の純抽出のみ実施し `cargo check`。
3. M2 で到達/trail を分離。

### ブロッカー/注意点

- `DREAM_UI_*` 定数の意味を変える変更は本計画外。
- `spawn_trail_ghost` の child 付与先（DreamBubbleLayer）を維持すること。

### 参照必須ファイル

- `src/systems/visual/dream/ui_particle/update.rs`
- `src/systems/visual/dream/ui_particle.rs`
- `docs/dream.md`

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
