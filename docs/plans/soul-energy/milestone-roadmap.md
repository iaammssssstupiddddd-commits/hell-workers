# Soul Energy システム — マイルストーンロードマップ

作成日: 2026-03-27
最終更新: 2026-03-27
ステータス: Phase 1 計画策定中

---

## ビジョン

**最終ゴール**: Soul が生成する Soul Energy を動力源とした電力グリッドシステムを導入し、照明・生産施設・Room ボーナスなどの新しいゲームプレイレイヤーを構築する。

**基本方針**: Dream システムとのトレードオフを核とし、「何人を発電に回すか」というマクロマネジメント判断を生み出す。

---

## コアコンセプト

### Soul Energy とは

Soul が **Soul Spa（魂スパ）** で儀式的瞑想を行うことで生成されるエネルギー。見た目はリラックス施設だが、実態は Soul の Dream 蓄積を搾取する発電所。

### Dream とのトレードオフ

- 発電タスク中の Soul は **dream 蓄積レートが大幅低下**（通常の 0.1〜0.3 倍）
- 疲労蓄積は低い（瞑想的行為）
- 発電 Soul が増える → DreamPool 成長が鈍化 + 建設/運搬の労働力が減少
- プレイヤーは「労働力 vs Soul Energy vs Dream」の3つ巴を管理する

### 電力グリッドモデル

- **蓄電なし**: リアルタイム需給バランス（将来バッテリー建物で対応）
- **停電**: consumption > generation で**グリッド内全消費建物が一斉停止**
- **グリッドローカル**: グローバルプールではなく、接続された建物群ごとのローカル容量

### 配電ルール（3つ）

1. **Yard 内共有**: 同一 Yard 内の発電エリアと電力消費建物は同一グリッド
2. **Room 隣接接続**: 電力エンティティが Room の外壁に隣接 → その Room もグリッドに参加
3. **Room 間伝播**: 壁/ドアを共有する Room 同士は同一グリッド

### ECS Relationship 設計

電力グリッドのメンバーシップは Bevy 0.18 の Relationship で管理する。発電元と消費者を型レベルで分離し、グリッド再計算時のフィルタを排除する。

**新規 Relationship（`hw_core/src/relationships.rs` に定義）**:

| Source（手動操作） | Target（Bevy自動） | 用途 |
|:---|:---|:---|
| `GeneratesFor(grid)` ← SoulSpaSite | `GridGenerators` ← PowerGrid | 発電元のグリッド所属 |
| `ConsumesFrom(grid)` ← OutdoorLamp等 | `GridConsumers` ← PowerGrid | 消費者のグリッド所属 |

**設計判断**:

- **接続粒度は Site レベル**: 個別の SoulSpaTile ではなく SoulSpaSite がグリッドに接続する。Site が内部で `TaskWorkers.len()` を集計し `PowerGenerator.current_output` を更新。GridMembers の膨張を防ぐ
- **Soul ↔ Tile は既存 Relationship で代替**: `WorkingOn(tile)` / `TaskWorkers` が「どの Soul がどのタイルで発電中か」を既にカバーしているため、`GeneratingAt` / `TileGenerators` のような新 Relationship は不要
- **発電/消費を分離する理由**: (1) フィルタなしで発電元/消費者を列挙可能 (2) 将来のバッテリー（両方持つ）に型安全に対応 (3) 停電時は `GridConsumers` だけ走査すればよい
- **Relationship にしないもの**: Room ↔ PowerGrid（トポロジ計算の結果であり Resource ベースの逆引きが適切）、Yard ↔ PowerGrid（Phase 1 では 1:1 で plain component で十分）

**データフロー**:

```
毎 0.5 秒:
1. SoulSpaSite が Children の TaskWorkers.len() を集計
   → PowerGenerator.current_output を更新

2. PowerGrid ごとに:
   GridGenerators.iter() → sum(current_output) → grid.generation
   GridConsumers.iter()  → sum(demand)          → grid.consumption
   grid.powered = generation >= consumption

3. powered 変化時:
   GridConsumers.iter() → Unpowered マーカー付与/除去
```

---

## フェーズ構成

```
Phase 1: 基盤（Yard 内完結）
  Soul Spa + GeneratePower タスク + 外灯 + Yard 内電力グリッド
    └─ Phase 2: Room 接続
         Room 隣接接続 + Room 間伝播 + 室内照明
           └─ Phase 3: 拡張
                送電線 + バッテリー + 電気室 + 新消費施設
                  └─ Phase 4: Room 命名
                       家具構成による Room タイプ判定
```

---

## Phase 1: 基盤（Yard 内完結）

> Soul Spa → GeneratePower タスク → 外灯 → Yard 内電力グリッド
> **目標**: 発電→消費→停電の基本ループと Dream トレードオフの手触りを検証する

### MS-1A: Soul Energy データモデル

- `PowerGrid` コンポーネント（generation / consumption / powered）
- `PowerGenerator` / `PowerConsumer` コンポーネント
- ECS Relationship: `GeneratesFor` ↔ `GridGenerators`、`ConsumesFrom` ↔ `GridConsumers`
- `SoulEnergyConstants`: 発電レート、Dream 低下係数、消費量などの定数
- **完了条件**: `cargo check` 通過、データモデルのみ（動作なし）

### MS-1B: Soul Spa 配置・建設

- `BuildingType::SoulSpa` の追加
- エリア型配置 UI（Floor と同様のドラッグ配置）
- `SoulSpaSite` / `SoulSpaTile` エンティティ構造
- 建設フロー（資材搬入 → 建設 → 完成）
- 稼働タイル数のクリック制御 UI
- **完了条件**: Yard 内に Soul Spa を建設・配置でき、タイル数を調整できる

### MS-1C: GeneratePower タスク

- `WorkType::GeneratePower` の追加
- 継続型タスク実行（完了なし、疲労/Familiar 判断で離脱）
- Dream 蓄積レート低下の実装
- Familiar による発電タスク割り当て
- 発電中の Soul Energy 生成（グリッドへの加算）
- **完了条件**: Soul が Soul Spa で発電タスクを実行し、Soul Energy が生成される。Dream 蓄積が低下する

### MS-1D: 外灯（Outdoor Lamp）

- `BuildingType::OutdoorLamp` の追加（`Temporary` カテゴリ）
- 配置・建設（1x1、資材: 木材 or 骨）
- `PowerConsumer` コンポーネント付与
- 効果: 周囲 Soul のストレス軽減 / 疲労回復速度アップ（電力供給時のみ）
- **完了条件**: 外灯を配置でき、電力供給時にステータスボーナスが適用される

### MS-1E: Yard 内電力グリッド

- Yard 単位のグリッド構築（同一 Yard 内 = 1 グリッド）
- generation / consumption のリアルタイム計算
- 停電判定（consumption > generation → 全消費建物停止）
- 停電時の外灯ボーナス消失
- UI: 電力状況の表示（発電量 / 消費量 / 状態）
- **完了条件**: 発電→消費→停電→復電のサイクルが正しく動作する

### MS-1F: ビジュアル・演出

- Soul Spa の儀式的ビジュアル（Soul の瞑想アニメーション）
- Soul Energy 生成エフェクト
- 外灯の点灯/消灯ビジュアル
- 停電時の視覚フィードバック
- **完了条件**: 発電・消費・停電の各状態が視覚的に明確

---

## Phase 2: Room 接続（計画未詳細化）

### MS-2A: Room 隣接接続
- 発電エリア/消費建物が Room 外壁に隣接 → グリッド接続
- Union-Find によるグリッドトポロジ管理
- Room 検出（2秒クールダウン）と同期した再計算

### MS-2B: Room 間伝播
- 壁/ドアを共有する Room 同士の自動接続
- 連結成分の再計算

### MS-2C: 室内照明（Room Light）
- Room 内設置の照明建物
- Room ボーナスの構成要素としての基盤

---

## Phase 3: 拡張（構想段階）

- 送電線（離れた Room への明示的接続）
- バッテリー建物（蓄電機能）
- 電気室（建設サイト向け電力供給）
- 新しい電力消費施設（上位生産設備など）

---

## Phase 4: Room 命名（構想段階）

- Room 内の家具・設備構成による自動命名
- 特定構成 = 特定 Room タイプ（発電室、精製工房、宿舎など）
- Room タイプに応じたボーナス・アンロック

---

## 既存システムとの接続点

| 既存システム | 接続方法 |
|:---|:---|
| Dream システム | GeneratePower タスク中の dream 蓄積レート低下 |
| タスクシステム | `WorkType::GeneratePower`（継続型）、`Designation`、`TaskSlots` |
| ECS Relationship | `WorkingOn`/`TaskWorkers`（Soul↔Tile 在席管理）、`GeneratesFor`/`ConsumesFrom`（グリッドメンバーシップ） |
| Familiar AI | 発電タスクへの Soul 割り当て判断 |
| 建築システム | Soul Spa のエリア型建設（FloorConstructionSite 類似） |
| Room 検出 | Phase 2 で電力グリッドのトポロジに利用 |
| 物流 | Soul Spa 建設時の資材搬入（既存 TransportRequest 基盤） |

---

## リスクと対策

| リスク | 影響 | 対策 |
|:---|:---|:---|
| 継続型タスクの前例がない | タスク実行系の大幅変更が必要になる可能性 | MS-1C で早期に検証。既存 Refine タスクの構造を参考に |
| Dream トレードオフのバランス | 発電が強すぎ/弱すぎでジレンマが成立しない | 定数調整可能な設計。テストプレイで反復調整 |
| Yard 内グリッドの単純さ | Phase 2 の Room 接続で設計を大きく変える必要 | MS-1A のデータモデルを Room 接続を見据えて設計 |
| エリア型配置の複雑さ | FloorConstructionSite と似て非なる仕組みが増える | 共通基盤の抽出を検討（ただし過度な抽象化は避ける） |
