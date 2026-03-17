# hw_visual ドメイン分離：hw_jobs / hw_logistics 直接依存の解消

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `hw-visual-domain-decoupling-2026-03-14` |
| ステータス | `Completed`（残存: `mud_mixer.rs`/`tank.rs`/`wall_connection.rs` は別提案） |
| 作成日 | `2026-03-14` |
| 最終更新日 | `2026-03-15` |
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
| `AssignedTask` | `soul/mod.rs`, `gather/worker_indicator.rs`, `blueprint/worker_indicator.rs`, `mud_mixer.rs`, `soul/idle.rs` | プログレスバー表示判定・進捗値取得・タスクリンク線描画・ステータスアイコン |
| `GatherPhase`, `BuildPhase`, `RefinePhase`, `HaulPhase`, `CoatWallPhase`, `FrameWallPhase`, `PourFloorPhase`, `ReinforceFloorPhase` | `soul/mod.rs`, worker_indicator 系 | フェーズ enum のパターンマッチ、進捗値（`progress_bp`）の取得 |
| `WorkType` | `gather/worker_indicator.rs` | 斧 / ツルハシアイコン切替 |
| `Blueprint` | `blueprint/effects.rs`, `material_display.rs`, `progress_bar.rs`, `wall_connection.rs` | 資材進捗・必要素材量の表示 |
| `FloorConstructionSite`, `WallConstructionSite`, `FloorTileBlueprint`, `WallTileBlueprint` | `floor_construction.rs`, `wall_construction.rs` | タイル別の施工フェーズ・完成度の描画 |
| `FloorConstructionPhase`, `WallConstructionPhase`, `FloorTileState`, `WallTileState` | `floor_construction.rs`, `wall_construction.rs` | タイル状態のフェーズ判定 |
| `Designation`, `Tree`, `Rock` | `gather/resource_highlight.rs` | 採取対象マーカーコンポーネントのフィルタ |
| `RestArea` | `dream/particle.rs` | 休息エリアのパーティクル表示 |
| `Building`, `BuildingType`, `MudMixerStorage` | `tank.rs`, `wall_connection.rs`, `mud_mixer.rs` | 建物種別・スロット残量の表示（※本提案スコープ外）|

*hw_logistics 由来:*
| 型 | 使用ファイル群 | 目的 |
|---|---|---|
| `Inventory`, `ResourceItem` | `haul/carrying_item.rs` | 運搬中アイテムのアイコン表示 |
| `Wheelbarrow` | `haul/wheelbarrow_follow.rs` | 手押し車エンティティのフィルタリング |
| `Stockpile` | `tank.rs` | 備蓄量表示（※本提案スコープ外）|

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

- `hw_jobs` / `hw_logistics` のビジネスロジックの変更（ビジュアル同期用システム/Observer の追加のみ行う）。
- ECS ポーリングから完全なイベント駆動への移行（段階的に行う）。
- `hw_ui` や `bevy_app` 側の依存整理（スコープ外）。
- `Building`, `BuildingType`, `MudMixerStorage`, `Stockpile` の分離（別途提案予定）。

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

#### グループ A（低コスト：採取・移動マーカーのミラー化）

対象: `Designation`, `Tree`, `Rock`, `Wheelbarrow`, `RestArea`

**`Designation`/`Tree`/`Rock` を hw_core に移動しない理由:**
`Designation` は `hw_jobs` 内で `TaskSlots`/`Priority` と一体で操作されており（`model.rs:329`）、採取ロジックに強く結びついている。`crate-boundaries.md §2` パターン B に該当するドメイン特化型であるため、hw_core に移動するとドメイン境界が崩れる。

**採用アプローチ：ビジュアル専用マーカーコンポーネント（hw_core 定義）を hw_jobs 側で同期**

```rust
// hw_core に追加
#[derive(Component)]
pub struct GatherHighlightMarker;  // hw_visual が highlight 判定に使用

#[derive(Component)]
pub struct RestAreaMarker;         // hw_visual が dream particle に使用
```

`hw_jobs` の Designation 付与・削除 Observer がそれぞれ `GatherHighlightMarker` の attach/detach を行う。`hw_visual` は `Designation`/`Tree`/`Rock` を直接参照せず `GatherHighlightMarker` のみを Query する。

**`Wheelbarrow` の扱い:**
`Wheelbarrow` は hw_logistics 固有のゲームプレイ型であり hw_core への移動は不適切。`WheelbarrowMarker` を hw_core に定義し hw_logistics 側で同期するミラーパターンを採用する。

#### グループ B（中コスト：タスクフェーズ表示のミラーコンポーネント化）

対象: `AssignedTask` および全フェーズ enum（`GatherPhase`, `BuildPhase`, `RefinePhase`, `HaulPhase`, `CoatWallPhase`, `FrameWallPhase`, `PourFloorPhase`, `ReinforceFloorPhase`）, `WorkType`

**イベント駆動を採用しない理由:**
`soul/mod.rs` の `task_link_system` と `update_progress_bar_fill_system` は `AssignedTask` のネスト構造から**エンティティ参照**（target, mixer, blueprint, bucket 等）と**進捗値**（`progress_bp`）を毎フレーム直接読み取っている。これを個々のイベントで表現しようとすると、`VisualTaskPhase` が実質 `AssignedTask` の複製になる。

**採用アプローチ：`SoulTaskVisualState` ミラーコンポーネント（グループ C と同じパターン）**

```rust
// hw_core に追加
#[derive(Component, Default)]
pub struct SoulTaskVisualState {
    /// タスクリンク線の描画先（None = 線を引かない）
    pub link_target: Option<Entity>,
    /// バケツ搬送中のリンク先（None の場合 link_target を使用）
    pub bucket_link: Option<Entity>,
    /// プログレスバー表示用（None = バーを非表示）
    pub progress: Option<f32>,
    /// ワーカーアイコン種別
    pub worker_icon: VisualWorkerIcon,
    /// アイドル判定（ステータスアイコン表示に使用）
    pub is_idle: bool,
}

#[derive(Default, Clone, PartialEq)]
pub enum VisualWorkerIcon {
    #[default]
    None,
    Axe,
    Pickaxe,
    Build,
    Haul,
    Refine { mixer: Entity },
}
```

`hw_jobs` 側では `AssignedTask` が変化したタイミング（`Changed<AssignedTask>`）で `SoulTaskVisualState` を同期するシステムを追加する。`hw_visual` の各システムはこのミラーコンポーネントのみを Query し、`AssignedTask` を直接参照しない。

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

`hw_jobs` 側では `Changed<Blueprint>` / `Changed<FloorConstructionSite>` を使い、変化があったタイミングのみミラーコンポーネントを同期するシステムを追加する（毎フレーム全件同期しない）。`hw_visual` はミラーコンポーネントのみを Query する。

**`hw_logistics` の `Inventory` / `Stockpile`** も同様に `CarryingItemVisual` / `StockpileVisual` ミラーコンポーネントを `hw_core` に定義し、`hw_logistics` 側で同期する。

---

### 5.2 変更対象（想定）

**hw_core:**
- `crates/hw_core/src/visual_mirror/` サブモジュール新設
  - `gather.rs`：`GatherHighlightMarker`, `RestAreaMarker`, `WheelbarrowMarker`
  - `task.rs`：`SoulTaskVisualState`, `VisualWorkerIcon`
  - `construction.rs`：`BlueprintVisualState`, `ConstructionTileVisualState`, `ConstructionPhaseVisual`
  - `logistics.rs`：`CarryingItemVisual`
- `crates/hw_core/src/lib.rs` に `pub mod visual_mirror` 追加

**hw_jobs:**
- `Designation` 付与・削除 Observer に `GatherHighlightMarker` の attach/detach を追加
- `RestArea` spawn/despawn Observer に `RestAreaMarker` の同期を追加
- `Changed<AssignedTask>` で `SoulTaskVisualState` を同期するシステム追加（`visual_sync` モジュール）
- `Changed<Blueprint>` で `BlueprintVisualState` を同期するシステム追加
- `Changed<FloorConstructionSite>` / `Changed<WallConstructionSite>` で `ConstructionTileVisualState` を同期するシステム追加

**hw_logistics:**
- `Wheelbarrow` spawn/despawn Observer に `WheelbarrowMarker` の同期を追加
- `Changed<Inventory>` で `CarryingItemVisual` を同期するシステム追加

**hw_visual:**
- `use hw_jobs::*` / `use hw_logistics::*` を `use hw_core::visual_mirror::*` へ置き換え
- 各システムのクエリをミラーコンポーネントベースに書き換え
- `Cargo.toml` から `hw_jobs` / `hw_logistics` 依存を削除

---

### 5.3 データ/コンポーネント/API 変更

| 種別 | 対象 | 内容 |
|---|---|---|
| 追加 | `hw_core::visual_mirror::GatherHighlightMarker` | 採取対象ハイライト用マーカー |
| 追加 | `hw_core::visual_mirror::RestAreaMarker` | 休息エリアパーティクル用マーカー |
| 追加 | `hw_core::visual_mirror::WheelbarrowMarker` | 手押し車フィルタ用マーカー |
| 追加 | `hw_core::visual_mirror::SoulTaskVisualState` | AssignedTask ミラーコンポーネント |
| 追加 | `hw_core::visual_mirror::VisualWorkerIcon` | ワーカーアイコン種別 enum |
| 追加 | `hw_core::visual_mirror::BlueprintVisualState` | Blueprint ミラーコンポーネント |
| 追加 | `hw_core::visual_mirror::ConstructionTileVisualState` | 建設タイル ミラーコンポーネント |
| 追加 | `hw_core::visual_mirror::CarryingItemVisual` | 運搬アイテム ミラーコンポーネント |
| 削除 | `hw_visual/Cargo.toml` の `hw_jobs`, `hw_logistics` | 依存削除（移行完了後） |

---

## 6. 代替案と比較

| 案 | 採否 | 理由 |
|---|---|---|
| **本提案（Mirror Component で全グループ統一）** | 採用 | `soul/mod.rs` のようなネスト構造の深い参照も一貫したパターンで扱える。毎フレーム更新の進捗系にも適切 |
| **イベント駆動（グループ B をイベント化）** | 不採用 | `task_link_system` が AssignedTask から多数のエンティティ参照を取得しており、イベントが AssignedTask の複製になる |
| **グループ A でマーカー型を hw_core に移動** | 不採用 | `Designation`/`Tree`/`Rock`/`Wheelbarrow` は hw_jobs/hw_logistics ドメインの型（§2 パターン B）。hw_core に移動するとドメイン境界が崩れる |
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
| Phase 1 | グループ A：`hw_core::visual_mirror` に `GatherHighlightMarker`/`RestAreaMarker`/`WheelbarrowMarker` 追加、hw_jobs/hw_logistics 側で同期 Observer 追加、hw_visual 側クエリ書き換え | 小 |
| Phase 2 | グループ B：`SoulTaskVisualState` ミラーコンポーネント追加、hw_jobs 側で `Changed<AssignedTask>` 同期システム追加、hw_visual 側の全 AssignedTask クエリを書き換え | 中 |
| Phase 3 | グループ C：Blueprint / Construction / Inventory ミラーコンポーネント追加、同期システム追加、hw_visual 側クエリ書き換え | 大 |
| 完了 | `hw_visual/Cargo.toml` から `hw_jobs` / `hw_logistics` 削除 | 小 |

**ロールバック:** 各フェーズが独立したコミットであるため、失敗したフェーズだけ revert 可能。

---

## 11. 未解決事項（Open Questions）

- [ ] `hw_core::visual_mirror` を将来独立 crate に切り出す価値があるか？（後回しで問題ない）
- [x] `Building` / `BuildingType` / `MudMixerStorage` / `Stockpile` のミラー化は別提案として立てるか、本提案の Phase 4 として追加するか？ → **本提案に組み込み完了（2026-03-17）**
- [ ] `SoulTaskVisualState` の `link_target`/`bucket_link` フィールドはデバッグ用 Gizmos のためのものだが、リリースビルドで無効化する仕組みが必要か？

---

## 12. AI 引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`（全フェーズ実装完了 / 2026-03-17）
- `hw_visual/Cargo.toml` から `hw_jobs` / `hw_logistics` の直接依存を削除済み
- `docs/crate-boundaries.md` §4.1 更新済み

### 完了条件（Definition of Done）

- [x] `hw_visual/Cargo.toml` に `hw_jobs` / `hw_logistics` の直接依存が存在しない
- [x] `cargo check` がエラーなしで通る
- [x] 全ビジュアル機能が手動確認シナリオで正常動作する
- [x] `docs/crate-boundaries.md` §4.1 の直接依存の例が更新されている

---

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
|---|---|---|
| `2026-03-14` | Claude | 初版作成 |
| `2026-03-17` | Copilot | Phase 4（Building/MudMixer/Stockpile ミラー化）実装完了。`hw_visual/Cargo.toml` から `hw_jobs`/`hw_logistics` 削除。AI引継ぎメモを完了状態に更新。Open Questions の Building 系ミラー化項目をクローズ。 |
