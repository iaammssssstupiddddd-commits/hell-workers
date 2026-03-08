# hw_ai crate — AI システムの crate 分離提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `hw-ai-crate-proposal-2026-03-08` |
| ステータス | `InProgress` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI` |
| 関連計画 | `docs/plans/hw-ai-crate-plan-2026-03-08.md` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- 現状: `src/systems/soul_ai/`（98ファイル）と `src/systems/familiar_ai/`（70ファイル）が root crate に存在し、全479ファイルの **35%** を占める。root crate 内のどのファイルを変更しても、AI コード含む全体が再コンパイル対象になる。
- 問題:
  - インクリメンタルビルドの粒度が粗い（UI変更で AI も再コンパイル）
  - AI システム内の依存関係が暗黙的（mod 境界のみで API 境界がない）
  - AI ロジックの単体テストが root crate 全体のコンパイルを要求する
- なぜ今やるか: AI ファイル数が増加傾向にあり、早期に境界を設計するほうがコストが低い。ただし前提条件（共有型の crate 化）が先に必要。

## 2. 目的（Goals）

- Soul AI と Familiar AI のシステムロジックを `hw_ai` crate に分離し、root crate のコンパイル単位を縮小する
- AI システムが依存する外部型を明示的な crate 境界で可視化する
- AI ロジックの単体テスト・ベンチマークを crate 単位で実行可能にする

## 3. 非目的（Non-Goals）

- AI アルゴリズムの変更・改善（構造変更のみ）
- Soul AI と Familiar AI の統合（別々のモジュールとして維持）
- 全システムの crate 分離（AI のみが対象）

## 4. 提案内容（概要）

- 一言要約: `crates/hw_ai/` に Soul AI・Familiar AI のシステムロジックを移動し、Bevy Plugin として root crate に登録する
- 主要な変更点:
  1. `crates/hw_ai/` crate を新設（`hw_core`, `hw_jobs`, `hw_logistics`, `hw_world` に依存）
  2. AI が参照するゲーム固有型（WorldMap, SpatialGrid, Entity Components）をトレイト境界または共有型で抽象化
  3. root crate は `hw_ai::SoulAiPlugin` / `hw_ai::FamiliarAiPlugin` を `app.add_plugins()` で登録
- 期待される効果:
  - AI 以外のコード変更時にAIの再コンパイルが不要に
  - AI コードの変更が root crate の再コンパイルをトリガーしない
  - 依存関係の明示化によるアーキテクチャ理解の向上

## 5. 詳細設計

### 5.1 依存関係の課題と解決方針

AI システムが依存する外部型を分類すると以下の通り：

| 依存先 | 具体的な型 | 解決方針 |
|:--|:--|:--|
| hw_core | AssignedTask, WorkType, Relationships, Events, Constants | **直接依存**（既に crate） |
| hw_jobs | Blueprint, Building, Designation, TaskSlots | **直接依存**（既に crate） |
| hw_logistics | ResourceItem, TransportRequest, Stockpile | **直接依存**（既に crate） |
| hw_world | pathfinding, coords | **直接依存**（既に crate） |
| root: WorldMap | Resource<WorldMap> | **案A: hw_world に移動** / **案B: トレイト抽象化** |
| root: SpatialGrid 各種 | Resource<*SpatialGrid> | **案A: hw_spatial crate** / **案B: トレイト抽象化** |
| root: Entity Components | DamnedSoul, Familiar, Vitals 等 | **hw_core に移動** or **hw_entities crate 新設** |
| root: GameSystemSet | SystemSet enum | **hw_core に移動** |

### 5.2 段階的アプローチ

#### Phase 0: 前提条件の整備（他の計画で実施）
- `AreaBounds` → `hw_core`
- 建設フェーズ enum → `hw_jobs`

#### Phase 1: AI が参照する Entity Component 型の crate 化
- `DamnedSoul`, `Familiar`, `Vitals` 等のマーカー/データ Component を `hw_core` に移動
- `GameSystemSet`, `AiSystemSet` を `hw_core` に移動
- これにより AI が root の型に依存する箇所を削減

#### Phase 2: WorldMap / SpatialGrid の抽象化
- 選択肢A: `WorldMap` を `hw_world` に移動（hw_world が hw_jobs に依存する必要あり）
- 選択肢B: `trait WorldAccess` を `hw_core` に定義し、AI は trait 経由でアクセス
- SpatialGrid は `GridData` + `SpatialGridOps` を `hw_core` に移動し、具体 Grid は root に残す

#### Phase 3: hw_ai crate 作成
- `crates/hw_ai/src/soul_ai/` — Soul AI systems を移動
- `crates/hw_ai/src/familiar_ai/` — Familiar AI systems を移動
- `crates/hw_ai/src/lib.rs` — `SoulAiPlugin`, `FamiliarAiPlugin` を export
- root は `app.add_plugins((SoulAiPlugin, FamiliarAiPlugin))` で登録

### 5.3 変更対象（想定）

**新規:**
- `crates/hw_ai/Cargo.toml`
- `crates/hw_ai/src/lib.rs`
- `crates/hw_ai/src/soul_ai/` (98 files moved)
- `crates/hw_ai/src/familiar_ai/` (70 files moved)

**変更:**
- `Cargo.toml` (workspace members)
- `src/main.rs` (plugin 登録)
- `crates/hw_core/src/lib.rs` (移動型の追加)

**削除:**
- `src/systems/soul_ai/` (移動元)
- `src/systems/familiar_ai/` (移動元)

### 5.4 データ/コンポーネント/API 変更

- 追加: `SoulAiPlugin`, `FamiliarAiPlugin` (Bevy Plugin)
- 変更: 既存の AI system 関数のパスが `hw_ai::` 配下に変更
- 削除: root 内の AI module 定義

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| A: hw_ai に soul_ai + familiar_ai を統合 | 検討中 | 依存関係が共通のため1 crate にまとめるのが自然 |
| B: hw_soul_ai + hw_familiar_ai に分離 | 不採用 | familiar_ai が soul_ai の型を直接参照しており、2 crate に分けると循環依存のリスク |
| C: AI はそのまま root に残す | 保留 | 短期的には問題ないが、ファイル数増加で長期的にビルド時間が悪化 |
| D: trait 抽象化なしで直接依存 | 検討中 | Phase 1 で Entity Component が crate 化できれば、trait 不要で直接依存できる可能性 |

## 7. 影響範囲

- ゲーム挙動: 変更なし（構造リファクタのみ）
- パフォーマンス: ビルド時間改善（AI 変更時に root 再コンパイル不要）、ランタイムは変更なし
- UI/UX: 変更なし
- セーブ互換: 変更なし
- 既存ドキュメント更新: `docs/architecture.md`, `docs/soul_ai.md`, `docs/familiar_ai.md`

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Entity Component の crate 化が大規模になる | 高 | Phase 1 で最小限の型のみ移動し、残りは trait で抽象化 |
| WorldMap の移動で hw_world → hw_jobs 依存が発生 | 中 | trait 抽象化（案B）で依存方向を逆転させる |
| AI system の Query 型が root の Component に依存 | 高 | SystemParam の trait 化または Component の段階的移動 |
| 移動作業中の長期間のマージコンフリクト | 高 | Phase ごとにマージし、一括移動は避ける |

## 9. 検証計画

- `cargo check`
- 手動確認シナリオ:
  - Soul の自律行動（タスク実行、集会参加、休憩）が正常
  - Familiar のタスク割り当て・巡回・激励が正常
  - AI 状態遷移イベントが UI に正しく反映される
- 計測/ログ確認:
  - ビルド時間の before/after 比較（`cargo build --timings`）

## 10. ロールアウト/ロールバック

- 導入手順: Phase 0 → Phase 1 → Phase 2 → Phase 3 の段階的実施
- 段階導入の有無: あり（各 Phase が独立してマージ可能）
- 問題発生時の戻し方: Phase 単位で git revert

## 11. 未解決事項（Open Questions）

- [ ] WorldMap は hw_world に移動すべきか、trait 抽象化すべきか
- [ ] Entity Component（DamnedSoul, Familiar, Vitals）の移動先は hw_core か新規 hw_entities か
- [ ] GameSystemSet / AiSystemSet は hw_core に置くのが妥当か
- [ ] familiar_ai → soul_ai の直接参照がどの程度あるか（循環リスクの定量評価）
- [ ] AI の Observer 登録は root で行うか hw_ai 内で行うか

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（提案段階）
- 直近で完了したこと: 依存関係の調査と分類
- 現在のブランチ/前提: master

### 次のAIが最初にやること

1. `src/systems/soul_ai/` と `src/systems/familiar_ai/` の `use` 文を全収集し、root 依存の型リストを作成
2. 各依存型を「crate 化済み / crate 移動可能 / trait 抽象化必要 / root 残留」に分類
3. Phase 1 の具体的な移動対象を決定

### ブロッカー/注意点

- Phase 0（AreaBounds, 建設フェーズ enum の移動）が前提条件
- familiar_ai が 70 ファイルと巨大で、移動時のマージコンフリクトリスクが高い
- AI の一部 Observer は `src/entities/` で登録されている可能性がある

### 参照必須ファイル

- `src/systems/soul_ai/mod.rs` — Soul AI モジュール構成
- `src/systems/familiar_ai/mod.rs` — Familiar AI モジュール構成
- `src/plugins/logic.rs` — AI system の登録箇所
- `docs/soul_ai.md` — Soul AI 仕様
- `docs/familiar_ai.md` — Familiar AI 仕様

### 完了条件（Definition of Done）

- [ ] 提案内容がレビュー可能な粒度で記述されている
- [ ] リスク・影響範囲・検証計画が埋まっている
- [ ] 実装へ進む場合の `docs/plans/...` が明記されている

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
