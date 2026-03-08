# hw_ui crate — UI/Interface システムの crate 分離提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `hw-ui-crate-proposal-2026-03-08` |
| ステータス | `Draft` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI` |
| 関連計画 | `docs/plans/hw-ui-crate-plan-2026-03-08.md` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- 現状: `src/interface/`（94ファイル）が root crate に存在し、全479ファイルの **20%** を占める。UI はゲームロジックと同一コンパイル単位にあり、ロジック変更で UI が再コンパイルされる（逆も同様）。
- 問題:
  - UI コード変更のイテレーション速度がゲームロジック全体のコンパイルに依存
  - UI とゲームロジック間の依存が暗黙的（mod 境界のみ）
  - UI コンポーネントのテストに root crate 全体が必要
- なぜ今やるか: hw_ai の crate 化と合わせて実施すれば root crate を大幅に縮小できる（AI: 35% + UI: 20% = 55% が分離対象）。ただし UI の方が結合度が高く、hw_ai より難易度が高い。

## 2. 目的（Goals）

- UI/Interface システムを `hw_ui` crate に分離し、ゲームロジックとの並行コンパイルを実現する
- UI が参照するゲーム状態を Presentation Model パターンで抽象化し、依存方向を明確にする
- UI の単体テスト（レイアウト計算、ViewModel 変換）を crate 単位で実行可能にする

## 3. 非目的（Non-Goals）

- UI デザインの変更・改善（構造変更のみ）
- Bevy UI フレームワーク自体の抽象化
- Visual システム（`src/systems/visual/`）の分離（別提案で扱う）

## 4. 提案内容（概要）

- 一言要約: `crates/hw_ui/` に Interface モジュールを移動し、ゲーム状態との接点を Presentation Model に集約する
- 主要な変更点:
  1. `crates/hw_ui/` crate を新設
  2. UI が参照するゲーム状態を `EntityInspectionModel` のような ViewModel に集約（既に部分的に実施済み）
  3. root crate は `hw_ui::InterfacePlugin` を `app.add_plugins()` で登録
- 期待される効果:
  - UI とロジックの並行コンパイルによるビルド時間削減
  - UI とゲームロジック間の依存関係の明示化
  - UI テストの独立実行

## 5. 詳細設計

### 5.1 UI モジュール構成と依存分析

現在の `src/interface/` の構成：

| サブモジュール | ファイル数 | 主な外部依存 |
|:--|:--|:--|
| `ui/list/` | 18 | DamnedSoul, Familiar, FamiliarAiState, Commanding |
| `ui/panels/` | 17 | Blueprint, Building, Stockpile, AssignedTask, ResourceType |
| `selection/` | 14 | WorldMap, PlayMode, Blueprint, BuildingType |
| `ui/interaction/` | 12 | PlayMode, TaskMode, GameTime, DebugVisible |
| `ui/setup/` | 7 | UiNodeRegistry, UiMountSlot |
| `ui/plugins/` | 6 | 他 UI モジュールの Plugin 統合 |
| `ui/presentation/` | 2 | EntityInspectionModel（ViewModel パターン） |
| その他 | 18 | camera, theme, vignette, components |

### 5.2 依存関係の課題

UI のゲームロジック依存は大きく3カテゴリに分類される：

**A. 表示データの読み取り（Read-only Query）**
- Entity の Component 読み取り: DamnedSoul, Familiar, Blueprint, Stockpile, Vitals 等
- Resource の読み取り: WorldMap, GameTime, PlayMode
- → **解決策**: これらの Component/Resource 型を共有 crate に移動すれば直接参照可能

**B. ユーザー入力によるゲーム状態変更（Commands/Events）**
- 建物配置: `commands.spawn(Blueprint { ... })`
- タスク指定: `TaskMode` 変更、`DesignationRequest` 発行
- ゾーン配置: Stockpile 生成
- → **解決策**: Event/Message 経由で root に処理を委譲（既に部分的に実施済み）

**C. 空間計算（WorldMap 依存）**
- 配置プレビュー: `WorldMap::is_walkable()`, `WorldMap::has_building()`
- ヒットテスト: `WorldMap::world_to_grid()`
- → **解決策**: `selection/` モジュールは WorldMap への密結合が高く、分離が最も困難

### 5.3 段階的アプローチ

#### Phase 0: 前提条件の整備
- hw_ai 提案の Phase 1 と共通: Entity Component 型の crate 化
- `PlayMode`, `GameSystemSet` → `hw_core`（hw_ai と共有）
- `UiMountSlot`, `UiNodeRegistry` 等の UI 固有型は hw_ui 内で定義可能

#### Phase 1: UI フレームワーク層の分離
- `ui/theme.rs`, `ui/components.rs`, `ui/setup/` を `hw_ui` に移動
- テーマ定数、スロットシステム、レイアウト初期化を crate 化
- root 依存が少ないため比較的容易

#### Phase 2: Presentation Model の強化
- `EntityInspectionModel` パターンを全パネルに拡張
- `ui/list/` の ViewModel 化（Familiar/Soul リストの表示データを struct に集約）
- UI が直接 Query する Component を ViewModel 経由に変換するシステムを root に配置

#### Phase 3: パネル・リストの移動
- `ui/panels/`, `ui/list/` を `hw_ui` に移動
- ViewModel 生成システムは root に残す（Query が root Component に依存）
- UI は ViewModel Resource のみを読み取る

#### Phase 4: Selection / Interaction の移動
- `selection/` は WorldMap 依存が強く最後に対応
- 選択肢A: WorldMap が crate 化されていれば直接依存
- 選択肢B: `trait PlacementValidator` を定義し、root で WorldMap ベースの実装を提供
- `ui/interaction/` は Event 発行のみなら比較的容易

### 5.4 最終的な crate 構成

```
hw_ui/
├── src/
│   ├── lib.rs          # InterfacePlugin
│   ├── theme.rs        # テーマ定数
│   ├── components.rs   # UI Component 型
│   ├── slots.rs        # UiMountSlot, UiNodeRegistry
│   ├── list/           # Entity List UI
│   ├── panels/         # Info Panel, Task List, Tooltip
│   ├── interaction/    # UI Interaction handlers
│   ├── selection/      # Building/Zone/Floor placement
│   └── presentation/   # ViewModel builders
├── Cargo.toml
```

### 5.5 データ/コンポーネント/API 変更

- 追加: `InterfacePlugin` (Bevy Plugin), ViewModel types
- 変更: UI system 関数のパスが `hw_ui::` 配下に変更
- 削除: root 内の interface module 定義

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| A: hw_ui に全 UI を移動 | 検討中 | 最終目標だが段階実施が必要 |
| B: hw_ui_core（フレームワーク）+ root（ゲーム固有 UI） | 検討中 | 中間段階として有効。ゲーム固有パネルは root に残し、共通基盤のみ分離 |
| C: selection だけ root に残す | 検討中 | WorldMap 依存を回避しつつ大部分を分離できる。現実的な妥協案 |
| D: UI はそのまま root に残す | 保留 | 短期的には問題ないが、hw_ai と合わせた効果が大きい |

## 7. 影響範囲

- ゲーム挙動: 変更なし（構造リファクタのみ）
- パフォーマンス: ビルド時間改善（UI 変更時にロジック再コンパイル不要）、ランタイムは変更なし（ViewModel 変換のオーバーヘッドは無視可能）
- UI/UX: 変更なし
- セーブ互換: 変更なし
- 既存ドキュメント更新: `docs/architecture.md`

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| ViewModel 導入で間接参照が増えコード量が膨張 | 中 | 既存の EntityInspectionModel パターンを踏襲し、新規抽象化を最小限に |
| selection の WorldMap 依存が trait 化で複雑になる | 高 | Phase 4 を最後に回し、WorldMap の crate 化状況を見てから判断 |
| UI と Visual の境界が曖昧 | 中 | Visual（`src/systems/visual/`）は別 crate として扱い、hw_ui のスコープに含めない |
| Phase 2 の ViewModel 化が大規模リファクタになる | 高 | 段階的に1パネルずつ ViewModel 化し、一括変更を避ける |
| hw_ai と hw_ui の同時進行でマージコンフリクト多発 | 中 | hw_ai を先に完了してから hw_ui に着手する順序を推奨 |

## 9. 検証計画

- `cargo check`
- 手動確認シナリオ:
  - Info Panel: エンティティ選択時の情報表示が正常
  - Entity List: Familiar/Soul リストの表示・ドラッグ&ドロップ
  - Selection: 建物配置・ゾーン配置・床/壁配置
  - Tooltips: ホバー時のツールチップ表示
  - Keyboard shortcuts: 全ショートカットが動作
  - Speed control: 時間操作 UI が正常
- 計測/ログ確認:
  - ビルド時間の before/after 比較（`cargo build --timings`）

## 10. ロールアウト/ロールバック

- 導入手順: Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4 の段階的実施
- 段階導入の有無: あり（各 Phase が独立してマージ可能）
- 問題発生時の戻し方: Phase 単位で git revert
- 推奨順序: **hw_ai の完了後** に hw_ui に着手

## 10.5 実装反映メモ（本計画）

- `hw_ui` 側に UI コアの表示・interaction を収束し、`bevy_app` は shell（plugin 登録）とゲーム状態更新 handler を持つ形を採用。
- `selection` と `camera` / `vignette` は本実装では `bevy_app` 側に残留し、Phase 4 以降の follow-up 対象として明示。
- `src/interface/ui/{setup,plugins,panels,list}` は wrapper/adapter 化を前提として残し、実体は `hw_ui` が担う。

## 11. 未解決事項（Open Questions）

- [ ] ViewModel パターンの粒度: パネル単位か Entity 単位か
- [x] `selection/` は `WorldMap` 依存が高いため、現時点は `bevy_app` に残留。follow-up で trait 抽象化または root-surface API の再設計を検討
- [ ] hw_ui は hw_ai に依存すべきか（AI 状態の表示に必要）、それとも hw_core 経由で間接参照すべきか
- [ ] `camera.rs` は hw_ui に含めるか root に残すか（PanCamera は Input セットに属する）
- [ ] Visual システム（speech bubbles, dream particles）の一部が UI と密結合している箇所の扱い
- [ ] 案B（hw_ui_core + root）と案A（全移動）のどちらを採用するか

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（提案段階）
- 直近で完了したこと: UI モジュールの依存分析
- 現在のブランチ/前提: master

### 次のAIが最初にやること

1. `src/interface/` 全ファイルの `use crate::` 文を収集し、root 依存型の完全リストを作成
2. 既存の `EntityInspectionModel` パターンの実装を確認し、ViewModel 化の基盤を評価
3. Phase 1 の移動対象（theme, components, setup）の依存関係を詳細に調査

### ブロッカー/注意点

- hw_ai の crate 化を先に完了することを推奨（共通の前提条件あり、同時進行はコンフリクトリスク）
- `src/interface/ui/interaction/mod.rs` にグローバルキーボードショートカットが集中しており、PlayMode/TaskMode への依存が強い
- `selection/building_place/` は WorldMap, Blueprint, BuildingType を直接 Query しており、最も分離困難

### 参照必須ファイル

- `src/interface/mod.rs` — Interface モジュール構成
- `src/interface/ui/components.rs` — UI Component 型一覧
- `src/interface/ui/presentation/builders.rs` — 既存の ViewModel パターン
- `src/interface/selection/` — 配置系 UI（最も分離困難な箇所）
- `src/plugins/interface.rs` — Interface Plugin 登録
- `docs/architecture.md` — UI アーキテクチャ補足

### 完了条件（Definition of Done）

- [ ] 提案内容がレビュー可能な粒度で記述されている
- [ ] リスク・影響範囲・検証計画が埋まっている
- [ ] 実装へ進む場合の `docs/plans/...` が明記されている

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
