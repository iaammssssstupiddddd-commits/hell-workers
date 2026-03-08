# hw_ai crate — AI システムの crate 分離提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `hw-ai-crate-proposal-2026-03-08` |
| ステータス | `InProgress` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI` |
| 関連計画 | `docs/plans/hw-ai-crate-plan-2026-03-08.md`, `docs/plans/hw-ai-crate-phase2-2026-03-08.md` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- 現状: `src/systems/soul_ai/`（98ファイル）と `src/systems/familiar_ai/`（70ファイル）が root crate に存在し、全479ファイルの **35%** を占める。root crate 内のどのファイルを変更しても、AI コード含む全体が再コンパイル対象になる。
- 問題:
  - インクリメンタルビルドの粒度が粗い（UI変更で AI も再コンパイル）
  - AI システム内の依存関係が暗黙的（mod 境界のみで API 境界がない）
  - AI ロジックの単体テストが root crate 全体のコンパイルを要求する
- なぜ今やるか: AI ファイル数が増加傾向にあり、早期に境界を設計するほうがコストが低い。ただし前提条件（共有型の crate 化）が先に必要。

## 2. 目的（Goals）

- Soul AI と Familiar AI のコアロジックを `hw_ai` crate に分離し、root crate のコンパイル単位を縮小する
- AI システムが依存する外部型を明示的な crate 境界で可視化する
- root crate を Bevy app shell / resource adapter / visual shell に寄せる
- AI ロジックの単体テスト・ベンチマークを crate 単位で実行可能にする

## 3. 非目的（Non-Goals）

- AI アルゴリズムの変更・改善（構造変更のみ）
- Soul AI と Familiar AI の完全統合（別モジュールとして維持）
- `WorldMap` resource 本体の `hw_world` への移動
- `hw_spatial` の責務外領域（UI/asset/visual/Commands）の再設計
- 全システムの crate 分離（AI のみが対象）

## 4. 提案内容（概要）

- 一言要約: `crates/hw_ai/` を AI core crate として育て、root crate は `WorldMap` / SpatialGrid / UI / visual / asset を扱う shell / adapter として残す
- 主要な変更点:
  1. `crates/hw_ai/` crate を新設し、`hw_core`, `hw_jobs`, `hw_logistics`, `hw_world` に依存させる
  2. Shared Component / SystemSet は `hw_core` に寄せ、world/pathfinding/query の抽象は `hw_world` に寄せる
  3. `WorldMap` は root 残留、`SpatialGrid` は `hw_spatial` の concrete 7 種へ移設し、`WorldMapRead/Write` と残留 2 grid を root が受ける構成にする
- 期待される効果:
  - AI 以外のコード変更時に AI core の再コンパイルを減らせる
  - AI コードの変更が UI / visual / asset shell へ波及しにくくなる
  - 依存関係の明示化により、どこまでが core でどこからが shell か判断しやすくなる

## 5. 詳細設計

### 5.1 依存関係の課題と解決方針

AI システムが依存する外部型を分類すると以下の通り：

| 依存先 | 具体的な型 | 解決方針 |
|:--|:--|:--|
| `hw_core` | `AssignedTask`, `WorkType`, Relationships, Events, Constants, `GameSystemSet`, AI Component 群 | **直接依存** |
| `hw_jobs` | `Blueprint`, `Building`, `Designation`, `TaskSlots` | **直接依存** |
| `hw_logistics` | `ResourceItem`, `TransportRequest`, `Stockpile` | **直接依存** |
| `hw_world` | `pathfinding`, `coords`, `PathWorld` などの world helper | **直接依存** |
| root: `WorldMap` | `Resource<WorldMap>`, `WorldMapRead`, `WorldMapWrite` | **root に残留**。必要な generic capability は `hw_world` の小さい trait / helper で受ける |
| root: SpatialGrid 各種 | `Resource<*SpatialGrid>` | **resource 実体は `hw_spatial` へ移設**（`GatheringSpotSpatialGrid` / `FloorConstructionSpatialGrid` は root 残留）。共通 read API は `SpatialGridOps` で集約 |
| root: shell system | `GameAssets`, sprite spawn, speech, gizmo, UI state | **root に残留** |

補足:

- `WorldMap` は `Entity` を保持する occupancy resource であり、`BuildingType` / `DoorState` を使う更新 API を持つため、`hw_world` に移すより root shell として扱うほうが自然。
- `hw_core` に `WorldAccess` のような omnibus trait は追加しない。world/spatial concern を shared core に混ぜると責務が曖昧になるため。
- SpatialGrid の共通化は concrete 7 grid を分離したうえで進め、まず重複している `get_in_area` の read API から着手する。update shell 自体は引き続き root を残す

### 5.2 段階的アプローチ

#### Phase 0: 前提条件の整備（他の計画で実施）

- `AreaBounds` → `hw_core`
- 建設フェーズ enum → `hw_jobs`

#### Phase 1: AI が参照する Entity Component 型の crate 化

- `DamnedSoul`, `Familiar`, `Vitals` などの AI が広く読む Component を `hw_core` に移動
- `GameSystemSet`, `FamiliarAiSystemSet`, `SoulAiSystemSet` を `hw_core` に移動
- これにより AI が root の entity module に依存する箇所を削減する

#### Phase 2: root adapter 境界の確定

- `WorldMap` resource 本体は root に残す
- `WorldMapRead` / `WorldMapWrite` も root adapter として残す
- world/pathfinding/query の generic capability は `hw_world` に寄せる
  - 既存の `PathWorld` を継続利用する
  - 不足があれば consumer 近傍に小さい trait / helper を追加する
  - `hw_core::WorldAccess` は導入しない
- SpatialGrid resource と update system は root に残す
  - まず `get_in_area` の重複解消を優先する
  - `GridData` / read trait の共有化が必要な場合も、候補は `hw_world` であり `hw_core` ではない
- この Phase の目的は「resource の移設」ではなく、「root shell / adapter / `hw_ai` core の境界固定」

#### Phase 3: `hw_ai` core の段階移行

- `hw_core` 型のみで成立するシステムを `hw_ai` へ移動
- `WorldMap` / SpatialGrid 依存システムは、root wrapper から generic helper を呼ぶ形へ分解してから移動可否を判断する
- root 側には wrapper plugin・resource access・visual shell・compatibility layer を残す

### 5.3 変更対象（想定）

**新規 / 継続利用:**

- `crates/hw_ai/Cargo.toml`
- `crates/hw_ai/src/lib.rs`
- `crates/hw_ai/src/soul_ai/`
- `crates/hw_ai/src/familiar_ai/`
- `docs/plans/hw-ai-crate-phase2-2026-03-08.md`

**主な変更:**

- `Cargo.toml`（workspace members / dependency 調整）
- `crates/hw_core/src/lib.rs`（移動型の追加）
- `crates/hw_world/src/*`（必要最小限の trait / helper 追加）
- `src/world/map/access.rs`（root adapter の維持・整理）
- `src/systems/spatial/*`（重複 read API の整理）
- `src/systems/soul_ai/*`
- `src/systems/familiar_ai/*`
- `src/plugins/logic.rs`
- `docs/cargo_workspace.md`
- `docs/architecture.md`

**削除しない方針:**

- `src/systems/soul_ai/` / `src/systems/familiar_ai/` の全面削除は、段階移行中の前提にしない
- root 側には shell / wrapper / re-export が一定期間残る

### 5.4 データ / コンポーネント / API 変更

- 追加 / 継続:
  - `hw_ai::SoulAiCorePlugin`
  - `hw_ai::FamiliarAiCorePlugin`
  - root 側の `SoulAiPlugin` / `FamiliarAiPlugin`（wrapper plugin）
- 変更:
  - 一部 AI system / helper の実体が `hw_ai::` 配下へ移る
  - world/pathfinding/query の一部 helper が `hw_world::` 側へ寄る
- 明示的に行わない変更:
  - `WorldMap` の `hw_world` への移動
  - `hw_core::WorldAccess` の導入
- `hw_ai` / `hw_spatial` の依存循環を避けるための過度な責務追加

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| A: `WorldMap` を `hw_world` へ移動 | 不採用 | `Entity` を持つ occupancy resource であり、app shell 寄りの責務が強い |
| B: `hw_core` に `WorldAccess` を定義 | 不採用 | 抽象が広すぎて責務が曖昧になり、world concern を core crate に持ち込む |
| C: root shell + `hw_ai` core のハイブリッド分離 | 採用 | 既存の `PathWorld` / wrapper plugin 構造と整合し、段階移行しやすい |
| D: `hw_spatial` crate を新設 | 採用 | concrete SpatialGrid resource を分離し、`WorldMap` / shell 構造と競合しない責務境界を実現 |
| E: AI をそのまま root に残す | 保留 | 短期は成立するが、ビルド時間と境界の不明瞭さを解消できない |

## 7. 影響範囲

- ゲーム挙動: 変更なし（構造リファクタのみ）
- パフォーマンス:
  - ビルド時間の改善を狙う
  - ランタイム性能は原則変更なし
- UI/UX: 変更なし
- セーブ互換: 変更なし
- 既存ドキュメント更新:
  - `docs/architecture.md`
  - `docs/cargo_workspace.md`
  - `docs/soul_ai.md`
  - `docs/familiar_ai.md`
  - `docs/plans/hw-ai-crate-phase2-2026-03-08.md`

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `WorldMap` / SpatialGrid に依存するファイル数が多い（61 / 24） | 高 | helper 単位で分解し、resource access を root wrapper に閉じ込めてから移動する |
| broad trait を急いで作って API が肥大化する | 高 | trait は `hw_world` の consumer 近傍に小さく追加し、用途ごとに分ける |
| `GridData` を `hw_core` に寄せて責務が崩れる | 中 | spatial concern は `hw_spatial`（resource）または `hw_world`（trait）へ寄せ、core に入れない |
| UI / asset / speech shell が `hw_ai` に混入する | 中 | `GameAssets`, `Commands`, gizmo, speech bubble を使うものは root 残留ルールを固定する |
| 段階移行中の wrapper が長期残存する | 中 | Phase ごとに移動済み / 未移動を記録し、不要な re-export を M7 で削除する |

## 9. 検証計画

- `cargo check --workspace`
- 手動確認シナリオ:
  - Soul の自律行動（タスク実行、集会参加、休憩）が正常
  - Familiar のタスク割り当て・巡回・激励が正常
  - AI 状態遷移イベントが UI に正しく反映される
- 計測 / ログ確認:
  - `cargo check --workspace --timings`
  - `cargo check -p hw_ai`

## 10. ロールアウト / ロールバック

- 導入手順: Phase 0 → Phase 1 → Phase 2 → Phase 3 の段階的実施
- 段階導入の有無: あり（各 Phase が独立してマージ可能）
- 問題発生時の戻し方:
  - docs 方針の修正は提案書 / 計画書のコミット単位で revert
  - code 実装は milestone 単位で revert
  - ただし `WorldMap` / SpatialGrid を root に残す境界方針自体は維持する

## 11. 未解決事項（Open Questions）

- [ ] `WorldMap` helper の first slice は `task_execution/common.rs` と `task_finder/filter.rs` のどちらから始めるのが最も効果的か
- [ ] SpatialGrid の矩形検索共通化は、root helper で済ませるか `hw_world` に read trait を作るか
- [ ] `task_execution/*` をどこまで generic helper と root wrapper に分解できるか
- [ ] AI の Observer 登録は migrated module ごとに `hw_ai` へ寄せるか、root wrapper plugin に残すか
- [ ] familiar_ai → soul_ai の直接参照がどの範囲で `hw_ai` 内へ閉じるか

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `85%`（方針確定＋実装初期移設フェーズ完了）
- 直近で完了したこと:
  - `WorldMap` / SpatialGrid の concrete boundary policy を決定
  - proposal / plan の前提を整合させた
- 現在のブランチ / 前提: master

### 次のAIが最初にやること

1. 残存する shell の整理（`root wrapper` の最小化）を進める
2. `familiar_ai` 側の `drifting` 系の残存依存を `hw_ai` 移設可否で分類する
3. 次段階（M6以降）の移設候補を `M6` 以降計画に反映する

### ブロッカー / 注意点

- `WorldMap` は root 残留前提で確定済み。提案書の旧二択へ戻さないこと
- new trait は `hw_core` ではなく `hw_world` に寄せること
- `GameAssets`, `Commands`, speech bubble, gizmo 依存システムは root shell として残すこと

### 参照必須ファイル

- `docs/plans/hw-ai-crate-phase2-2026-03-08.md`
- `docs/cargo_workspace.md`
- `src/world/map/mod.rs`
- `src/world/map/access.rs`
- `src/world/pathfinding.rs`
- `crates/hw_world/src/pathfinding.rs`
- `src/systems/spatial/grid.rs`
- `src/systems/familiar_ai/decide/task_management/task_finder/filter.rs`

### 完了条件（Definition of Done）

- [x] 提案内容が review 可能な粒度で記述されている
- [x] `WorldMap` / SpatialGrid の方針が proposal / plan / workspace guide で一致している
- [x] 後続実装の最初の slice が `docs/plans/hw-ai-crate-phase2-2026-03-08.md` に従って着手可能である

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
| `2026-03-08` | `AI` | `WorldMap` / SpatialGrid を root shell / adapter 境界として扱う方針に更新 |
| `2026-03-08` | `AI` | M2/M3/M5 実装状況を反映し、DoD と進捗値を更新 |
