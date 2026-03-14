# hw_visual ドメイン分離：hw_jobs / hw_logistics 直接依存の解消

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `hw-visual-domain-decoupling-2026-03-14` |
| ステータス | `Draft` |
| 作成日 | `2026-03-14` |
| 最終更新日 | `2026-03-14` |
| 作成者 | Claude |
| 関連計画 | `docs/plans/hw-visual-domain-decoupling.md`（実装時に作成） |
| 関連Issue/PR | N/A |

---

## 1. 背景と問題

**現状:**
`hw_visual` は `hw_jobs` と `hw_logistics` を `Cargo.toml` で直接依存している。
`docs/crate-boundaries.md §3.2` は異なるドメイン間の連携に Pub/Sub パターンを求めており、同文書 §4.1 では `hw_visual → hw_jobs` を「移行すべき既存の直接依存」の例として名指ししている。

**使われている型（全て読み取り専用）:**

*hw_jobs 由来:*
| 型 | 使用ファイル群 | 目的 |
|---|---|---|
| `AssignedTask` | `gather/worker_indicator.rs`, `blueprint/worker_indicator.rs`, `mud_mixer.rs`, `soul/idle.rs` | タスクフェーズ（Build/Gather/Refine）に応じたビジュアル切替 |
| `Blueprint` | `blueprint/effects.rs`, `material_display.rs`, `progress_bar.rs` | 資材進捗・必要素材量の表示 |
| `FloorConstructionSite`, `WallConstructionSite` | `floor_construction.rs`, `wall_construction.rs` | タイル別の施工フェーズ・完成度の描画 |
| `GatherPhase`, `BuildPhase`, `RefinePhase` | 上記 worker_indicator 系 | フェーズ enum のパターンマッチ |
| `WorkType` | `gather/worker_indicator.rs` | 斧 / ツルハシアイコン切替 |
| `Designation`, `Tree`, `Rock` | `gather/resource_highlight.rs` | 採取対象マーカーコンポーネントのフィルタ |
| `Building`, `BuildingType`, `MudMixerStorage` | `tank.rs`, `wall_connection.rs`, `mud_mixer.rs` | 建物種別・スロット残量の表示 |

*hw_logistics 由来:*
| 型 | 使用ファイル群 | 目的 |
|---|---|---|
| `Inventory`, `ResourceItem` | `haul/carrying_item.rs` | 運搬中アイテムのアイコン表示 |
| `Wheelbarrow` | `haul/wheelbarrow_follow.rs` | 手押し車エンティティのフィルタリング |
| `Stockpile` | `tank.rs` | 備蓄量表示 |

**問題:**
- `hw_visual` が `hw_jobs` / `hw_logistics` のドメイン型に直接依存しているため、ビジネスロジック変更時にビジュアル層の再コンパイルが連鎖する。
- crate 境界規約上「推奨されない密結合」が残存し、新しい開発者が誤ったパターンを踏襲しやすい。

**なぜ今やるか:**
`hw_jobs` / `hw_logistics` の型に触れる改修が増えているタイミングであり、§4.1「既存コードの改修時に Pub/Sub 化を検討する」の基準を満たしている。

---

## 2. 目的（Goals）

- `hw_visual/Cargo.toml` から `hw_jobs` と `hw_logistics` の直接依存を削除する。
- `hw_visual` が参照するゲーム状態を `hw_core` 定義の Event / 軽量コンポーネント経由で受け取る設計にする。
- 既存のビジュアル挙動（表示内容・タイミング）を変えない。

---

## 3. 非目的（Non-Goals）

- `hw_jobs` / `hw_logistics` 内部の実装変更（イベント発行の追加のみ行う）。
- ECS ポーリングから完全なイベント駆動への移行（段階的に行う）。
- `hw_ui` や `bevy_app` 側の依存整理（スコープ外）。

---

## 4. 提案内容（概要）

**一言要約:** `hw_core` に「ビジュアル表示用の軽量 Event / Mirror Component」を追加し、`hw_jobs` / `hw_logistics` から発行、`hw_visual` はそれを購読する。

**主要な変更点:**
1. `hw_core` に Visual-facing イベントと Mirror コンポーネント群を追加する。
2. `hw_jobs` / `hw_logistics` 側でイベント発行 Observer / System を追加する。
3. `hw_visual` 側の Query を新しい型に置き換え、`hw_jobs` / `hw_logistics` 依存を削除する。

**期待される効果:**
- `hw_jobs` 変更時の `hw_visual` 再コンパイルが不要になる。
- crate 境界規約に完全準拠し、同パターンの悪例が消える。

---

## 5. 詳細設計

### 5.1 移行の難度分類

移行コストの観点から使用箇所を 3 グループに分ける。

#### グループ A（低コスト：マーカーコンポーネントの hw_core 移動）

対象: `Designation`, `Tree`, `Rock`, `Wheelbarrow`

これらはデータを持たない（または最小限の）マーカーコンポーネントであり、`hw_core` に移動するだけで解決する。既存の `hw_jobs` / `hw_logistics` でも `hw_core` から re-export すれば既存コードへの影響を最小化できる。

```
hw_core::jobs::Designation       (旧: hw_jobs::Designation)
hw_core::jobs::Tree              (旧: hw_jobs::Tree)
hw_core::jobs::Rock              (旧: hw_jobs::Rock)
hw_core::logistics::Wheelbarrow  (旧: hw_logistics::Wheelbarrow)
```

#### グループ B（中コスト：タスクフェーズ表示のイベント化）

対象: `AssignedTask`, `GatherPhase`, `BuildPhase`, `RefinePhase`, `WorkType`

ビジュアルシステムが「タスクのフェーズ」を毎フレームポーリングしている箇所を、タスク割り当て・フェーズ変化時のイベントに置き換える。

**hw_core に追加するイベント:**
```rust
/// hw_core::events に追加
#[derive(Event)]
pub struct OnTaskPhaseChanged {
    pub soul: Entity,
    pub phase: VisualTaskPhase,
}

/// ビジュアル層が必要とするフェーズ情報のみを持つ軽量 enum
#[derive(Clone, PartialEq, Eq)]
pub enum VisualTaskPhase {
    Idle,
    GatherAxe,
    GatherPickaxe,
    Build,
    Refine { frame_index: u8 },
}
```

`hw_jobs` 側では `AssignedTask` が変化したタイミング（既存の assign / unassign Observer）で `OnTaskPhaseChanged` を発行する。

`hw_visual` 側では `OnTaskPhaseChanged` を購読し、`VisualPhaseCache` コンポーネントとして Soul エンティティに書き込む。Worker indicator 系のシステムはこのキャッシュを読む。

#### グループ C（高コスト：建設・資材進捗の表示）

対象: `Blueprint`, `FloorConstructionSite`, `WallConstructionSite`

これらはフレームごとに進捗値が更新される可能性があり、差分イベント化よりも「ビジュアル専用ミラーコンポーネント」パターンが適している。

**hw_core に追加するコンポーネント:**
```rust
/// hw_core::construction_visual に追加
/// hw_jobs::Blueprint の表示に必要な値のミラー
#[derive(Component)]
pub struct BlueprintVisualState {
    pub progress: f32,
    pub material_counts: Vec<(ResourceType, u32, u32)>, // (type, delivered, required)
}

/// hw_jobs::FloorConstructionSite / WallConstructionSite 共通
#[derive(Component)]
pub struct ConstructionTileVisualState {
    pub phase: ConstructionPhaseVisual,
    pub completion_ratio: f32,
}

#[derive(Clone)]
pub enum ConstructionPhaseVisual { Planning, Framing, Coating, Complete }
```

`hw_jobs` 側では `Blueprint` / `FloorConstructionSite` が変化したタイミングでミラーコンポーネントを同エンティティに同期するシステムを追加する。`hw_visual` はミラーコンポーネントのみを Query する。

**`hw_logistics` の `Inventory` / `Stockpile`** も同様に `CarryingItemVisual` / `StockpileVisual` ミラーコンポーネントを `hw_core` に定義し、`hw_logistics` 側で同期する。

---

### 5.2 変更対象（想定）

**hw_core:**
- `crates/hw_core/src/events.rs` に `OnTaskPhaseChanged` 追加
- `crates/hw_core/src/` に `construction_visual.rs` 新設（BlueprintVisualState 等）
- `crates/hw_core/src/logistics_visual.rs` 新設（CarryingItemVisual 等）
- `crates/hw_core/src/lib.rs` に新モジュール公開

**hw_jobs:**
- `Designation`, `Tree`, `Rock` を `hw_core` へ移動し re-export
- assign/unassign Observer に `OnTaskPhaseChanged` 発行を追加
- `Blueprint` 変化時に `BlueprintVisualState` を同期するシステム追加
- `FloorConstructionSite` / `WallConstructionSite` 変化時にミラー同期システム追加

**hw_logistics:**
- `Wheelbarrow` を `hw_core` へ移動し re-export
- `Inventory` 変化時に `CarryingItemVisual` を同期するシステム追加
- `Stockpile` 変化時に `StockpileVisual` を同期するシステム追加

**hw_visual:**
- `use hw_jobs::*` / `use hw_logistics::*` を `use hw_core::*` へ置き換え
- グループ B の worker_indicator 系システムを `VisualPhaseCache` ベースに書き換え
- グループ C の blueprint / construction 系システムをミラーコンポーネントベースに書き換え
- `Cargo.toml` から `hw_jobs` / `hw_logistics` 依存を削除

---

### 5.3 データ/コンポーネント/API 変更

| 種別 | 対象 | 内容 |
|---|---|---|
| 追加 | `hw_core::events::OnTaskPhaseChanged` | タスクフェーズ変化イベント |
| 追加 | `hw_core::VisualTaskPhase` | ビジュアル用フェーズ enum |
| 追加 | `hw_core::BlueprintVisualState` | Blueprint ミラーコンポーネント |
| 追加 | `hw_core::ConstructionTileVisualState` | 建設タイル ミラーコンポーネント |
| 追加 | `hw_core::CarryingItemVisual` | 運搬アイテム ミラーコンポーネント |
| 追加 | `hw_core::StockpileVisual` | 備蓄 ミラーコンポーネント |
| 移動 | `hw_jobs::Designation`, `Tree`, `Rock` | `hw_core` へ移動・hw_jobs から re-export |
| 移動 | `hw_logistics::Wheelbarrow` | `hw_core` へ移動・hw_logistics から re-export |
| 削除 | `hw_visual/Cargo.toml` の `hw_jobs`, `hw_logistics` | 依存削除（移行完了後） |

---

## 6. 代替案と比較

| 案 | 採否 | 理由 |
|---|---|---|
| **本提案（Mirror Component + イベント）** | 採用 | 毎フレーム更新が必要な進捗系にはキャッシュが適切。実装量は多いが hw_visual の完全分離を達成できる |
| **全面イベント駆動（差分通知のみ）** | 不採用 | Blueprint の資材カウントなど多項目の差分管理が複雑化する。over-engineering のリスクが高い |
| **hw_visual を「プレゼンテーション層」として現状維持** | 不採用 | `hw_ui` とは違い hw_visual に特例扱いの明文化がなく、今後の踏み台になる |
| **hw_core に hw_jobs / hw_logistics の基盤型をすべて移動** | 不採用 | hw_core が肥大化し、単なるダンプ先になるリスクがある |

---

## 7. 影響範囲

- **ゲーム挙動:** なし（表示タイミングが 1 フレームずれる可能性はあるが視覚的に無視可能）
- **パフォーマンス:** ミラー同期の分ごくわずかに増加するが、通常タイマー駆動の AI 処理と比べて無視可能
- **UI/UX:** なし
- **セーブ互換:** なし（ビジュアル専用コンポーネントはセーブ対象外）
- **既存ドキュメント更新:** `docs/crate-boundaries.md` §4.1 の「移行すべき直接依存」の例を削除、`hw_visual/CLAUDE.md` の依存制約テーブルを更新

---

## 8. リスクと対策

| リスク | 影響 | 対策 |
|---|---|---|
| ミラー同期のタイミングずれ（1 フレーム遅延） | 低：視覚的にほぼ無視可能 | 同一 SystemSet 内で同期を先に実行するよう順序指定 |
| `hw_core` 肥大化 | 中：将来の分離コストが上がる | Visual 系型を `hw_core::visual_mirror` サブモジュールに集約し、後から `hw_visual_mirror` クレートとして分離可能な構造にする |
| グループ C の移行漏れ（タイル系の複雑さ） | 中：部分的な依存が残る | フェーズ分割を明確にし、Phase 3 完了まで `hw_jobs` 依存を残す（中間状態を文書化） |
| `hw_jobs` 側のミラー同期システムがビジュアル関心事を持ち込む | 低 | システム名・モジュールを `visual_sync` と明示してドメインロジックと分離する |

---

## 9. 検証計画

- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`（各フェーズ後）
- `hw_visual/Cargo.toml` に `hw_jobs` / `hw_logistics` が残っていないことを確認
- 手動確認シナリオ:
  - [ ] Gather ワーカーの斧 / ツルハシアイコンが正しく切り替わる
  - [ ] Blueprint 進捗バーが正しく更新される
  - [ ] 運搬中アイテムアイコンが表示される
  - [ ] 建設タイルのフェーズ色が正しく変化する

---

## 10. ロールアウト / ロールバック

**導入手順（段階的）:**

| フェーズ | 内容 | 目安工数 |
|---|---|---|
| Phase 1 | グループ A：マーカーコンポーネントを `hw_core` に移動・re-export | 小 |
| Phase 2 | グループ B：タスクフェーズを `OnTaskPhaseChanged` イベント + `VisualPhaseCache` に切替 | 中 |
| Phase 3 | グループ C：Blueprint / Construction / Inventory をミラーコンポーネントに切替 | 大 |
| 完了 | `hw_visual/Cargo.toml` から `hw_jobs` / `hw_logistics` 削除 | 小 |

**ロールバック:** 各フェーズが独立したコミットであるため、失敗したフェーズだけ revert 可能。

---

## 11. 未解決事項（Open Questions）

- [ ] `hw_core::visual_mirror` を将来独立 crate に切り出す価値があるか？（後回しで問題ない）
- [ ] `Building` / `BuildingType` / `MudMixerStorage`（`tank.rs`, `wall_connection.rs`）のミラー化は Phase 3 に含めるか、別提案とするか？
- [ ] `hw_logistics::Stockpile` の可視化は現在どの程度使われているか（`tank.rs` の精査が必要）？

---

## 12. AI 引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（提案書作成のみ、実装未着手）
- 現在のブランチ: `master`

### 次の AI が最初にやること

1. `crates/hw_visual/src/gather/resource_highlight.rs`, `haul/carrying_item.rs`, `blueprint/progress_bar.rs` を読んで具体的な Query シグネチャを把握する
2. `crates/hw_core/src/` の構造を確認し、新モジュールの配置場所を決める
3. Phase 1 から着手：`Designation`, `Tree`, `Rock`, `Wheelbarrow` を `hw_core` に移動

### ブロッカー / 注意点

- `hw_jobs` では `Designation` / `Tree` / `Rock` を内部でも使っている可能性があるため、移動後に `hw_jobs/Cargo.toml` に `hw_core` 依存が既に存在するか確認する
- ミラーコンポーネントは `hw_visual` が Plugin 内で spawn しないこと（hw_jobs/hw_logistics 側が attach する）

### 参照必須ファイル

- `docs/crate-boundaries.md`（境界ルール全文）
- `crates/hw_visual/CLAUDE.md`（Visual クレート固有ルール）
- `crates/hw_visual/src/gather/resource_highlight.rs`
- `crates/hw_visual/src/blueprint/progress_bar.rs`
- `crates/hw_visual/src/haul/carrying_item.rs`
- `crates/hw_core/src/` 全体

### 完了条件（Definition of Done）

- [ ] `hw_visual/Cargo.toml` に `hw_jobs` / `hw_logistics` の直接依存が存在しない
- [ ] `cargo check` がエラーなしで通る
- [ ] 全ビジュアル機能が手動確認シナリオで正常動作する
- [ ] `docs/crate-boundaries.md` §4.1 の直接依存の例が更新されている

---

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
|---|---|---|
| `2026-03-14` | Claude | 初版作成 |
