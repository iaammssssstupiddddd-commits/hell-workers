# Site / Yard システム — タスクエリア刷新

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `site-yard-proposal-2026-03-03` |
| ステータス | `Review` |
| 作成日 | `2026-03-03` |
| 最終更新日 | `2026-03-03` |
| 作成者 | AI (Claude) + satotakumi |
| 関連計画 | `TBD` |
| 関連Issue/PR | N/A |

## 1. 背景と問題

- **現状**: Familiar ごとに 1 つの `TaskArea`（矩形）が全活動範囲を担っている。建築現場・設備・Stockpile すべてが同じ TaskArea 内に配置され、1 Familiar = 1 エリアの 1:1 対応。
- **問題**:
  - 建築現場（Site）と生産設備/備蓄（Yard）の区別がなく、エリア設計の柔軟性が低い
  - 設備（MudMixer, Tank 等）を複数の Familiar で共有する仕組みがない。Familiar ごとに設備を重複配置する必要がある
  - Stockpile のグルーピングが Familiar 単位のため、複数 Familiar で備蓄を効率的に共有できない
  - TaskArea が大きくなりすぎる（建築現場 + 設備 + 備蓄をすべて包含する必要がある）
- **なぜ今やるか**: ゲームの規模が拡大するにつれ、Familiar 間のリソース共有が重要になる。設備の共有なしでは拠点拡張のコストが線形に増大する。

## 2. 目的（Goals）

- 建築現場（Site）と設備・備蓄（Yard）を空間的・概念的に分離する
- 複数の Familiar が 1 つの設備・備蓄基盤を共有できるようにする
- TaskArea を Site 内のサブエリア（Familiar 担当ゾーン）に限定し、役割を明確化する
- 資材探索の流れを「TaskArea → Yard → マップ全体」に整理する

## 3. 非目的（Non-Goals）

- 複数 Site/Yard セットの導入（将来拡張として設計は考慮するが、初期実装は 1 セット固定）
- Familiar 1体に複数 TaskArea を持たせる機能（現状の 1:1 を維持）
- Yard 内の設備配置制限（どの BuildingType も Yard / Site 外にも配置可能。ただし自動運搬の対象になるのは Yard 内のもの）

## 4. 提案内容（概要）

### 一言要約

ゲーム開始時に固定配置される **Site**（建築エリア）と拡張可能な **Yard**（設備・備蓄エリア）を導入し、現在の TaskArea を Site 内のサブエリアに再定義する。

### 主要な変更点

1. **Site**: 固定サイズの矩形エリア。建築作業（Wall, Floor, Bridge, Door）の対象エリア
2. **Yard**: 拡張可能な矩形エリア。設備（Plant + Temporary）と Stockpile の配置エリア
3. **TaskArea**: Site 内のサブエリアとして再定義。各 Familiar の優先作業ゾーン
4. **資材探索**: TaskArea → Yard → マップ全体 の 3 段階探索
5. **Stockpile グルーピング**: Familiar 単位 → Yard 単位に変更

### 期待される効果

- 設備の重複配置が不要になり、拠点の効率が向上する
- 複数 Familiar が Stockpile を共有でき、物流が効率化する
- プレイヤーは「何をどこに建てるか」を明確に分離して考えられる

## 5. 詳細設計

### 5.1 エンティティ構造

```
World (singleton)
├── Site (singleton, 固定 40×20)
│   ├── TaskArea (Familiar ごとに 1 つ、Site 内サブエリア)
│   │   └── ChildOf(Site) via Relationship
│   ├── Structure buildings (Wall, Floor, Bridge, Door) ← Site 外配置禁止
│   └── Designation targets (Chop, Mine 等)
│
└── Yard (singleton, 初期 20×20, 拡張可能, Site と重複不可)
    ├── Plant buildings (Tank, MudMixer)
    ├── Temporary buildings (WheelbarrowParking, SandPile, BonePile, RestArea)
    └── Stockpile zones
```

**配置制約**:
- Structure 建築（Wall, Floor, Bridge, Door）は **Site 内のみ** に配置可能。Site 外での配置操作はゴーストが赤表示 + 拒否
- Plant/Temporary 設備（Tank, MudMixer, WheelbarrowParking, SandPile, BonePile, RestArea）と Stockpile は **Yard 内のみ** に配置可能。Yard 外での配置操作はゴーストが赤表示 + 拒否

### 5.2 コンポーネント設計

#### 新規コンポーネント

```rust
/// 建築エリア（固定サイズ: 40×20）
/// 初期実装は singleton だが、将来の飛び地対応で複数エンティティになる想定。
/// → システムは Query<&Site> でイテレーションし、Single<> は使わない。
#[derive(Component)]
pub struct Site {
    pub min: Vec2,
    pub max: Vec2,
}

/// 設備・備蓄エリア（拡張可能、最小 20×20、Site と重複不可）
/// Site と同様に将来は複数エンティティ。
/// → システムは Query<&Yard> でイテレーションし、Single<> は使わない。
#[derive(Component)]
pub struct Yard {
    pub min: Vec2,
    pub max: Vec2,
}

/// Site と Yard をペアリングする Relationship
/// 飛び地対応: 各 Site は 1 つの Yard と対になる
#[derive(Component)]
pub struct PairedYard(pub Entity);  // Site → Yard

#[derive(Component)]
pub struct PairedSite(pub Entity);  // Yard → Site
```

**複数セット（飛び地）対応の設計方針**:
- `Site` / `Yard` は singleton として実装するが、コード上は `Query<&Site>` / `Query<&Yard>` でイテレーションする（`Single<>` を使わない）
- `PairedYard` / `PairedSite` で Site-Yard ペアを管理。将来的に N ペアに拡張可能
- Familiar は `BelongsTo(site_entity)` で所属 Site を指定。将来は Familiar の Site 間移動も可能に
- Stockpile グルーピング・設備アクセスは Yard entity 基準のため、複数 Yard でも自然に分離される

#### 定数

```rust
pub const SITE_WIDTH: f32 = 40.0;   // タイル
pub const SITE_HEIGHT: f32 = 20.0;  // タイル
pub const YARD_MIN_WIDTH: f32 = 20.0;
pub const YARD_MIN_HEIGHT: f32 = 20.0;
pub const YARD_INITIAL_WIDTH: f32 = 20.0;
pub const YARD_INITIAL_HEIGHT: f32 = 20.0;
```

#### 既存コンポーネントの変更

```rust
/// TaskArea は Site 内のサブエリア（Familiar の担当ゾーン）として継続
/// 構造体は変更なし。意味合いのみ変更：
/// - 旧: Familiar の全活動範囲
/// - 新: Site 内の優先作業ゾーン
#[derive(Component)]
pub struct TaskArea {
    pub min: Vec2, // Site.min <= min, max <= Site.max の制約を追加
    pub max: Vec2,
}
```

### 5.3 資材探索フロー（変更後）

現在の段階的探索（TaskArea → +10 → +30 → +60 → 全域）を以下に再編:

| 段階 | 探索範囲 | 用途 |
|:---|:---|:---|
| Stage 0 | TaskArea 内 | 直近の作業対象・地面資材 |
| Stage 1 | Yard 内 | Stockpile からの調達、設備へのアクセス |
| Stage 2 | マップ全体（到達可能） | フォールバック |

**変更の影響を受けるシステム**:
- `blueprint_auto_haul_system` — ソース探索を TaskArea → Yard → 全域 に変更
- `blueprint_auto_gather_system` — 段階探索を TaskArea → Yard 周辺 → 全域 に変更
- `task_area_auto_haul_system` — Stockpile グルーピングを Yard 単位に変更
- `mud_mixer_auto_haul_system` — Mixer が Yard 内にあるため TaskArea マージン → Yard 内探索に変更

### 5.4 Stockpile グルーピング（変更後）

**現在**: Familiar ごとに TaskArea 内の Stockpile をグループ化 → Familiar 別に `DepositToStockpile` を発行

**変更後**: Yard 内の全 Stockpile を 1 つのグループとして扱う

```
変更前:
  Familiar A の TaskArea → Stockpile群A → DepositToStockpile (issued_by: A)
  Familiar B の TaskArea → Stockpile群B → DepositToStockpile (issued_by: B)

変更後:
  Yard → 全 Stockpile → DepositToStockpile (issued_by: Yard or global)
  全 Familiar がこの request を参照
```

**競合回避**: 既存の `IncomingDeliveries.len()` による容量チェックがそのまま機能する。複数 Familiar の Soul が同時に搬入しても、容量超過は発生しない。

**Stockpile 統合 (`ConsolidateStockpile`)**: `stockpile_consolidation_producer_system` のグルーピングも Yard 単位に変更する。Yard 内の全 Stockpile を統合対象とし、同一資源タイプの分散を解消する。

### 5.5 設備アクセス

Yard 内の設備（MudMixer, Tank, WheelbarrowParking 等）へのアクセスに距離制限はない。

**現在の TaskArea マージン制限との対応**:

| 現在の挙動 | 変更後 |
|:---|:---|
| TaskArea 内 → +10 → +30 → +60 で Mixer 探索 | Yard 内の Mixer を直接参照（距離制限なし） |
| TaskArea +10 内で WheelbarrowParking 探索 | Yard 内の WheelbarrowParking を直接参照 |
| TaskArea 内で Tank/BucketStorage 探索 | Yard 内の Tank を直接参照 |

### 5.6 タスク発見とフィルタリング

`task_finder` の空間検索ロジックを更新:

**現在**: `DesignationSpatialGrid.get_in_area(task_area.min, task_area.max)` + `ManagedTasks`

**変更後**:
1. `DesignationSpatialGrid.get_in_area(task_area.min, task_area.max)` — Site 内の担当ゾーン
2. `TransportRequestSpatialGrid.get_in_area(yard.min, yard.max)` — Yard 内の運搬系タスク
3. `ManagedTasks` — 既存の管理下タスク

Mixer タスクのフィルタ免除（現在の挙動）は Yard 内探索で自然に解決される。

### 5.7 UI / ビジュアル

#### 初期配置

- ゲーム開始時に Site と Yard が自動配置される（プレイヤーの手動作成は不要）
- Site: 固定サイズ（変更不可）
- Yard: 初期サイズあり、プレイヤーが拡張可能

#### 表示

- Site: 専用の境界線シェーダー（現在の TaskArea シェーダーを流用可能）
- Yard: 別色の境界線シェーダー（設備エリアであることを視覚的に区別）
- TaskArea: 現在の色分けシェーダーを継続（Familiar ごとの色）

#### 操作

- TaskArea 編集: 現在のドラッグ操作をそのまま維持。ただし Site の範囲内に制約
- Yard 拡張: ドラッグで矩形を拡大（Stockpile の Zone 配置と同様の操作感）
- Site: 固定のため操作不要

### 5.8 `BelongsTo` / `issued_by` の変更

**現在**: 設備・Stockpile は `BelongsTo(familiar_entity)` で特定の Familiar に所属

**変更後**: Yard 所属の設備・Stockpile は `BelongsTo(yard_entity)` に統一
- `issued_by` も `yard_entity` を使用
- Familiar 間の所有権競合がなくなる
- Tank → Bucket → BucketStorage の所有チェーンも Yard 単位に統一

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| A: TaskArea を Site にリネームし Yard を追加 | 不採用 | TaskArea のサブエリア機能（Familiar 担当ゾーン）が失われる |
| B: Yard なし、設備を全 Familiar で共有可能にする | 不採用 | 空間的な整理がなく、配置の意図が不明確になる |
| C: Site/Yard を物理エリアではなく論理グループ（タグ）にする | 不採用 | 空間的な探索最適化（SpatialGrid）が活用できない |
| **D: Site + Yard の独立矩形エリア（本提案）** | **採用** | 空間的分離が明確で、既存の SpatialGrid 最適化と相性が良い |

## 7. 影響範囲

### ゲーム挙動

- 資材探索の段階が変更（TaskArea マージン → Yard → 全域）
- Stockpile の DepositToStockpile が Yard 単位に統合
- 設備アクセスの距離制限撤廃（Yard 経由）
- **互換性**: 既存セーブデータからの移行ロジックが必要（TaskArea 内の設備/Stockpile を Yard に再割り当て）

### パフォーマンス

- Stockpile グルーピングが N Familiar × M Stockpile → 1 グループに簡略化（改善）
- Yard 内の SpatialGrid 探索は既存のグリッドをそのまま利用可能
- TaskArea 外周のマージン計算が不要になる（改善）

### UI/UX

- Site/Yard の境界線表示（新規シェーダーまたは既存流用）
- Yard 拡張操作の追加
- TaskArea 編集に Site 内制約の追加

### セーブ互換

- 既存セーブデータに Site/Yard エンティティが存在しないため、マイグレーションが必要
- TaskArea 位置から Site/Yard を推定生成するか、デフォルト値で初期化

### 既存ドキュメント更新

- `docs/tasks.md` — 資材探索フロー、TaskArea の役割説明
- `docs/familiar_ai.md` — Familiar と Site/Yard の関係
- `docs/logistics.md` — Stockpile グルーピング、DepositToStockpile の変更
- `docs/building.md` — 建築物の配置エリア（Site/Yard）
- `docs/architecture.md` — Site/Yard の概要追加

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Yard 単位のグルーピングで複数 Familiar の Soul が同時搬入し、効率低下 | 中 | `IncomingDeliveries` による容量制御は既存。距離ベースのスコアリングで最寄り Soul が優先される |
| TaskArea の Site 内制約で既存プレイヤーの配置が無効になる | 高 | マイグレーション時に Site を既存 TaskArea を包含するサイズで初期化 |
| `BelongsTo` の変更で Tank → Bucket チェーンの所有チェックが壊れる | 高 | 段階的移行: まず Yard リソースを `BelongsTo(yard)` に、次に所有チェックを Yard 対応に |
| 1 Yard の Stockpile に全 Familiar の Soul が殺到して渋滞 | 中 | 距離スコアリングが自然に分散する。必要なら Stockpile の TaskSlots を増加 |

## 9. 検証計画

- `cargo check`
- 手動確認シナリオ:
  1. ゲーム開始 → Site / Yard が表示される
  2. Familiar の TaskArea が Site 内に制約される
  3. Yard に Stockpile を配置 → 全 Familiar の Soul が搬入できる
  4. Yard に MudMixer を配置 → 全 Familiar が利用できる
  5. TaskArea 内の木を伐採 → Yard の Stockpile に自動搬入される
  6. Blueprint 配置 → Yard の Stockpile から資材が調達される
  7. Yard を拡張 → 新しい領域に設備を配置できる
- 計測/ログ確認:
  - `TransportRequestMetrics` で DepositToStockpile の発行数が Familiar 数に比例しないことを確認
  - 搬入効率（Soul の移動距離/完了タスク数）が悪化していないことを確認

## 10. ロールアウト/ロールバック

### 導入手順（段階的）

1. **Phase 1: データモデル追加**
   - `Site`, `Yard` コンポーネント追加
   - 初期配置システム実装
   - TaskArea に Site 内制約を追加
   - 既存の挙動は維持（TaskArea ベースの探索はそのまま）

2. **Phase 2: Stockpile グルーピング変更**
   - `task_area_auto_haul_system` を Yard 単位に変更
   - `BelongsTo` を Yard 対応に移行
   - `issued_by` を Yard entity に統一

3. **Phase 3: 資材探索フロー変更**
   - blueprint_auto_haul / auto_gather の段階探索を TaskArea → Yard → 全域に変更
   - mud_mixer_auto_haul のマージン探索を Yard 内探索に変更

4. **Phase 4: UI/ビジュアル**
   - Site/Yard の境界線表示
   - Yard 拡張 UI
   - TaskArea 編集の Site 内制約の UI フィードバック

### 問題発生時の戻し方

- 各 Phase は独立して revert 可能
- Phase 1 は既存挙動を壊さない（追加のみ）
- Phase 2/3 は feature flag で切り替え可能にする

## 11. 未解決事項（Open Questions）

### 解決済み

- [x] **Site の初期サイズ**: 固定値 **縦 20 × 横 40** タイル
- [x] **Yard の初期・最大サイズ**: 初期 **20 × 20** タイル。最大サイズ制限なし。最小サイズ 20 × 20。Site と重複不可
- [x] **Site 外への建築**: **不許可**。Wall, Floor, Bridge, Door は Site 内のみ
- [x] **RestArea の配置先**: **Yard** に配置（Soul の休憩動線は Yard 経由）

- [x] **Yard 外の設備配置**: **禁止**。Plant/Temporary 設備および Stockpile は Yard 内のみ配置可能
- [x] **複数 Site/Yard セット**: 最終的には複数セット（飛び地）が成り立つよう設計する。初期実装は 1 セット
- [x] **Stockpile 統合 (`consolidation`)**: **Yard 単位**に変更する

### 未解決

（なし — 全項目解決済み）

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%` — 提案書ドラフト作成完了。実装未着手。
- 直近で完了したこと: ヒアリングに基づく提案書作成
- 現在のブランチ/前提: `master`

### 次のAIが最初にやること

1. この提案書のレビュー結果を確認する
2. 未解決事項（§11）の回答を確認する
3. `docs/plans/site-yard-system.md` に実装計画を作成する

### ブロッカー/注意点

- `BelongsTo` の変更は影響範囲が広い（Tank/Bucket/BucketStorage の所有チェーン全体に波及）
- `task_area_auto_haul_system` は現在 Familiar 単位のイテレーションで構成されている。Yard 単位への変更はループ構造自体の見直しが必要
- 段階導入の Phase 2/3 の境界で一時的に不整合が発生しないよう注意

### 参照必須ファイル

- `docs/tasks.md` — タスクシステム全体
- `docs/logistics.md` — 物流・Stockpile・TransportRequest
- `docs/familiar_ai.md` — Familiar AI と TaskArea の関係
- `docs/building.md` — 建築システム
- `src/systems/command/mod.rs` — TaskArea コンポーネント定義
- `src/systems/familiar_ai/decide/task_management/` — タスク探索・委譲
- `src/systems/transport_request/producer/` — TransportRequest 生産者群

### 完了条件（Definition of Done）

- [ ] 提案内容がレビュー可能な粒度で記述されている
- [ ] リスク・影響範囲・検証計画が埋まっている
- [ ] 実装へ進む場合の `docs/plans/...` が明記されている

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-03-03 | AI (Claude) | 初版作成 — ヒアリング結果に基づくドラフト |
| 2026-03-03 | AI (Claude) | §11 の 4 項目を解決済みに移行。Site サイズ(40×20)、Yard 最小サイズ(20×20)、Site 外建築禁止、RestArea→Yard を反映 |
| 2026-03-03 | AI (Claude) | §11 残り 3 項目を解決。Yard 外設備配置禁止、複数セット飛び地対応設計、Consolidation→Yard 単位。ステータスを Review に変更 |
