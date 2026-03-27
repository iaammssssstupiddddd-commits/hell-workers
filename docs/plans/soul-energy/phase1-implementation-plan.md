# Soul Energy Phase 1 実装計画 — Yard 内基盤

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `soul-energy-phase1-2026-03-27` |
| ステータス | `Draft` |
| 作成日 | `2026-03-27` |
| 最終更新日 | `2026-03-27` (ECS Relationship 設計反映) |
| 作成者 | `AI (Claude)` |
| 関連提案 | N/A |
| 関連Issue/PR | N/A |

## 1. 目的

- 解決したい課題: 現在のゲームには動力・エネルギーの概念がなく、建物は配置するだけで機能する。生産チェーンの深さとマクロマネジメントの判断軸が不足している
- 到達したい状態: Soul Energy の発電→消費→停電サイクルが Yard 内で完結して動作し、Dream とのトレードオフがプレイヤーの意思決定を生む
- 成功指標:
  - Soul Spa を建設し、Soul が発電タスクを実行できる
  - 外灯が電力供給時にのみステータスボーナスを付与する
  - 発電 Soul を引き抜くと停電が発生し、外灯ボーナスが消失する
  - 発電 Soul の Dream 蓄積が目に見えて鈍化する

## 2. スコープ

### 対象（In Scope）

- Soul Energy データモデル（PowerGrid / PowerGenerator / PowerConsumer）
- ECS Relationship: `GeneratesFor` ↔ `GridGenerators`、`ConsumesFrom` ↔ `GridConsumers`
- Soul Spa（エリア型発電施設）の配置・建設・運用
- GeneratePower タスク（継続型、Dream 蓄積レート低下）
- 外灯（Outdoor Lamp）の配置・建設・電力消費・ステータスボーナス
- Yard 内電力グリッド（配電ルール1のみ: 同一 Yard 内共有）
- 停電判定と復電
- 基本的なビジュアルフィードバック

### 非対象（Out of Scope）

- Room 隣接接続・Room 間伝播（Phase 2）
- 室内照明（Phase 2）
- 送電線・バッテリー（Phase 3）
- 電気室（Phase 3）
- Room 命名・家具構成判定（Phase 4）
- Soul Energy の UI 詳細設計（最低限の電力状況表示のみ）

## 3. 現状とギャップ

- 現状: 建物は配置すれば無条件で機能する。唯一の生産チェーンは MudMixer（手動資材搬入のみ）。Dream はグローバルプールに蓄積されるが、消費先が植林のみ
- 問題: リソース管理の判断軸が少なく、中盤以降のゲームプレイに深みが不足
- 本計画で埋めるギャップ: 「労働力 vs Soul Energy vs Dream」の3つ巴トレードオフを導入し、電力管理という新しいマネジメントレイヤーを追加

## 4. 実装方針（高レベル）

- 方針: FloorConstructionSite の構造を参考にエリア型建設を実装。タスク実行は既存フレームワーク（AssignedTask + task_execution_system）に継続型バリアントを追加
- 設計上の前提:
  - Phase 2 の Room 接続を見据え、PowerGrid はエンティティベースで設計する（Yard 直結ではなく、グリッドエンティティを介在させる）
  - 発電量・消費量・Dream レート低下係数は定数化し、バランス調整を容易にする
  - Soul Spa は建設完了後の「稼働中建物」であり、Blueprint/ConstructionSite とは別のライフサイクルを持つ
- ECS Relationship 設計方針:
  - **発電/消費で Relationship を分離**: `GeneratesFor` ↔ `GridGenerators`（発電元→グリッド）と `ConsumesFrom` ↔ `GridConsumers`（消費者→グリッド）の 2 組。型レベルで分離し、グリッド再計算時のフィルタを排除
  - **接続粒度は Site レベル**: 個別の SoulSpaTile ではなく SoulSpaSite が `GeneratesFor` でグリッドに接続。Site が内部で `TaskWorkers.len()` を集計し `PowerGenerator.current_output` を更新
  - **Soul ↔ Tile は既存 Relationship で代替**: `WorkingOn(tile)` / `TaskWorkers` が「どの Soul がどのタイルで発電中か」をカバー。新 Relationship `GeneratingAt` は不要
  - **将来のバッテリー対応**: 分離された Relationship により、1 エンティティが `GeneratesFor` と `ConsumesFrom` の両方を持てる
- Bevy 0.18 APIでの注意点:
  - 継続型タスクは `AssignedTask` の新バリアントとして追加。完了条件を持たない点で既存タスクと異なる
  - PowerGrid の再計算頻度はパフォーマンスを考慮し、0.5〜1.0秒間隔のタイマーベースとする

## 5. マイルストーン

### M1: Soul Energy データモデル

- 変更内容:
  - `PowerGrid` コンポーネント: generation, consumption, powered フィールド
  - `PowerGenerator` コンポーネント: current_output（Site 集計値）、output_per_soul（Soul 1人あたりの発電量/秒）
  - `PowerConsumer` コンポーネント: demand（消費量/秒）
  - ECS Relationship（`hw_core/src/relationships.rs` に追加）:
    ```rust
    // 発電元 → グリッド
    #[relationship(relationship_target = GridGenerators)]
    pub struct GeneratesFor(pub Entity);  // SoulSpaSite → PowerGrid

    #[relationship_target(relationship = GeneratesFor)]
    pub struct GridGenerators(Vec<Entity>);

    // 消費者 → グリッド
    #[relationship(relationship_target = GridConsumers)]
    pub struct ConsumesFrom(pub Entity);  // OutdoorLamp 等 → PowerGrid

    #[relationship_target(relationship = ConsumesFrom)]
    pub struct GridConsumers(Vec<Entity>);
    ```
  - `Unpowered` マーカーコンポーネント（停電中の消費建物に付与）
  - `SoulEnergyConstants`: 発電レート、Dream 低下係数、外灯消費量などの定数
  - 定義場所は既存の crate 構成を調査して決定（`hw_core` or 新 crate）
- 変更ファイル:
  - `crates/hw_core/src/relationships.rs` — Relationship 2 組追加
  - `crates/hw_core/src/` 配下 — コンポーネント・定数定義
  - or 新 crate `crates/hw_energy/src/` の検討
- 完了条件:
  - [ ] データ型・Relationship が定義され `cargo check` 通過
  - [ ] 既存コードへの影響なし
- 検証:
  - `cargo check`
  - `cargo clippy --workspace`

### M2: Soul Spa 配置・建設

- 変更内容:
  - `BuildingType::SoulSpa` の追加（カテゴリ: `Plant`）
  - `SoulSpaSite` / `SoulSpaTile` エンティティ構造
    ```
    SoulSpaSite (親)
    ├─ area_bounds: TaskArea
    ├─ active_slots: u32
    ├─ tiles_total: u32
    ├─ PowerGenerator { current_output, output_per_soul }
    ├─ GeneratesFor(power_grid)  ← Relationship (M5 で接続)
    └─ children: Vec<SoulSpaTile>

    SoulSpaTile (子, ChildOf で親に接続)
    ├─ grid_pos: (i32, i32)
    ├─ state: Inactive / Active / Occupied
    ├─ Designation(GeneratePower) + TaskSlots(1)
    └─ TaskWorkers ← WorkingOn (既存 Relationship, Soul が発電開始時に自動付与)
    ```
  - **Site レベルの集計**: SoulSpaSite が Children の `TaskWorkers.len()` を定期集計し、`PowerGenerator.current_output = occupied_count * output_per_soul` を更新
  - エリア型配置 UI:
    - `TaskMode::SoulSpaPlace` の追加
    - ドラッグで矩形指定（Floor 配置と同様の UX）
    - 配置条件: Yard 内、walkable、Floor 完成済み（or 不要？要検討）
  - 建設フロー:
    - 資材: 骨 × N / タイル（具体値は要検討）
    - FloorConstructionSite の Reinforcing フェーズに類似した搬入→建設
    - 養生なし（壁と同様に即完成）
  - 稼働タイル数制御:
    - 完成した SoulSpaSite をクリック → active_slots を増減する UI
    - active_slots を超える Soul は割り当てられない
- 変更ファイル:
  - `crates/hw_jobs/src/model.rs` — BuildingType 追加
  - `crates/bevy_app/src/systems/jobs/` — Soul Spa 建設システム
  - `crates/bevy_app/src/interface/` — 配置 UI
  - `crates/bevy_app/src/plugins/` — プラグイン登録
  - `assets/textures/` — Soul Spa テクスチャ（仮素材可）
- 完了条件:
  - [ ] Yard 内に Soul Spa をドラッグ配置できる
  - [ ] 資材搬入後に建設が完了する
  - [ ] 完成後に稼働タイル数を増減できる
  - [ ] `cargo check` 通過
- 検証:
  - `cargo check`
  - 手動: 配置 → 建設 → 完成 → タイル数制御の一連のフロー

### M3: GeneratePower タスク

- 変更内容:
  - `WorkType::GeneratePower` の追加
  - `AssignedTask::GeneratePower` バリアント（継続型タスク）
    ```
    GeneratePower {
        target_tile: Entity,  // SoulSpaTile
        phase: GeneratePowerPhase,
    }

    enum GeneratePowerPhase {
        GoingToTile,
        Generating,  // 完了条件なし、疲労/Familiar判断で離脱
    }
    ```
  - タスク実行ロジック:
    - `GoingToTile`: タイル位置へ移動
    - `Generating`: 継続的に発電（Site の `PowerGenerator.current_output` に反映）
    - 離脱条件: 疲労閾値（通常より高め = 長時間稼働可）、Familiar の再割り当て判断
  - **既存 Relationship の活用**:
    - タスク割り当て時: `WorkingOn(soul_spa_tile)` が Soul に設定される（既存フロー）
    - SoulSpaTile 側に `TaskWorkers` が自動付与 → Site が `TaskWorkers.len()` を集計して `PowerGenerator.current_output` を更新
    - 新規 Relationship は不要（`GeneratingAt` 等は定義しない）
  - Dream 蓄積レート低下:
    - `dream_update_system` で `AssignedTask::GeneratePower` 中の Soul の蓄積レートに係数を適用
    - 係数は `SoulEnergyConstants` から取得（初期値 0.2 = 通常の 20%）
  - Familiar タスク割り当て:
    - Active な SoulSpaTile に `Designation(GeneratePower)` を付与するシステム
    - `TaskSlots::new(1)` per tile
    - Familiar が通常のタスク発見フローで発電タスクを検出・割り当て
  - 疲労蓄積の調整:
    - GeneratePower 中の疲労蓄積レートを通常作業より低く設定（瞑想的行為）
    - 定数化して調整可能にする
- 変更ファイル:
  - `crates/hw_jobs/src/` — WorkType 追加
  - `crates/bevy_app/src/systems/soul_ai/execute/task_execution/` — GeneratePower 実行ロジック
  - `crates/hw_soul_ai/src/` — dream_update_system の修正、疲労レート調整
  - `crates/hw_familiar_ai/src/` — タスク発見・割り当てへの GeneratePower 統合
- 完了条件:
  - [ ] Familiar が Soul を Soul Spa タイルに割り当てる
  - [ ] Soul がタイルで発電を開始し、PowerGrid.generation が増加する
  - [ ] 発電中 Soul の Dream 蓄積レートが低下する
  - [ ] 疲労で自然離脱し、Familiar が次の Soul を割り当てる
  - [ ] `cargo check` 通過
- 検証:
  - `cargo check`
  - 手動: Soul Spa 建設 → Soul 割り当て → 発電確認 → Dream レート低下確認 → 疲労離脱 → 再割り当て

### M4: 外灯（Outdoor Lamp）

- 変更内容:
  - `BuildingType::OutdoorLamp` の追加（カテゴリ: `Temporary`）
  - 配置: 1x1、通常の建物配置フロー
  - 資材: 骨 × 2（仮値）
  - `PowerConsumer` コンポーネント付与（demand: 定数）
  - `ConsumesFrom(power_grid)` Relationship 付与（M5 のグリッド構築時に接続）
  - 効果: 電力供給時、周囲 N タイル以内の Soul に:
    - ストレス軽減バフ（ストレス蓄積レート × 0.8 等）
    - 疲労回復速度アップ（休憩中の回復速度 × 1.2 等）
  - 効果は `powered == true` の間のみ有効
- 変更ファイル:
  - `crates/hw_jobs/src/model.rs` — BuildingType 追加
  - `crates/bevy_app/src/systems/jobs/` — 建設・完成処理
  - `crates/hw_soul_ai/src/` — ステータスバフ適用システム
  - `assets/textures/` — 外灯テクスチャ（仮素材可）
- 完了条件:
  - [ ] 外灯を建設できる
  - [ ] 電力供給時にステータスボーナスが適用される
  - [ ] 停電時にボーナスが消失する
  - [ ] `cargo check` 通過
- 検証:
  - `cargo check`
  - 手動: 外灯建設 → 発電 ON → ボーナス確認 → 発電 OFF → ボーナス消失確認

### M5: Yard 内電力グリッド

- 変更内容:
  - Yard ごとに **PowerGrid エンティティ** を生成・管理するシステム
  - **Relationship 接続**:
    - SoulSpaSite 建設完了時 → `GeneratesFor(yard_grid)` を付与
    - OutdoorLamp 建設完了時 → `ConsumesFrom(yard_grid)` を付与
    - 建物 despawn 時 → Relationship 自動削除により GridGenerators / GridConsumers から自動除去
  - グリッド再計算（0.5 秒間隔 + powered 状態変化時即時）:
    ```
    GridGenerators.iter() → sum(PowerGenerator.current_output) → grid.generation
    GridConsumers.iter()  → sum(PowerConsumer.demand)           → grid.consumption
    grid.powered = generation >= consumption
    ```
  - 停電処理:
    - `powered` が false に遷移 → `GridConsumers.iter()` で全消費建物に `Unpowered` マーカー付与
    - `powered` が true に復帰 → `GridConsumers.iter()` で `Unpowered` マーカー除去
  - PowerGrid と Yard の関連:
    - Phase 1: Yard ごとに 1 PowerGrid エンティティを生成（plain component `YardOwner(Entity)` で所有関係を持つ）
    - Phase 2: Room 接続時にグリッドの統合・分割ロジックを追加。`GeneratesFor` / `ConsumesFrom` のターゲットを付け替えるだけで移行可能
  - UI:
    - Yard 選択時に電力状況を表示（generation / consumption / 状態）
    - 停電時の警告表示
- 変更ファイル:
  - `crates/bevy_app/src/systems/` — 電力グリッド管理システム
  - `crates/bevy_app/src/interface/` — 電力状況 UI
  - `crates/bevy_app/src/plugins/` — プラグイン登録
- 完了条件:
  - [ ] Yard 内の発電量と消費量が Relationship 経由でリアルタイム集計される
  - [ ] consumption > generation で全消費建物が停止する
  - [ ] 発電 Soul が戻ると復電する
  - [ ] 建物 despawn 時に Relationship 自動削除でグリッドが正しく更新される
  - [ ] UI で電力状況が確認できる
  - [ ] `cargo check` 通過
- 検証:
  - `cargo check`
  - 手動シナリオ:
    1. Soul Spa 2タイル + 外灯 3個 → 1台で足りるか確認
    2. 発電 Soul を引き抜き → 停電 → 外灯消灯・ボーナス消失
    3. 発電 Soul を戻す → 復電 → 外灯点灯・ボーナス復活
    4. Soul Spa の active_slots を 0 にする → 全停電
    5. 外灯を撤去 → GridConsumers から自動削除 → consumption 減少

### M6: ビジュアル・演出

- 変更内容:
  - Soul Spa:
    - タイルの地面テクスチャ（儀式的な模様）
    - 発電中 Soul の瞑想ポーズ / アニメーション
    - Soul Energy 生成エフェクト（光の粒子が上昇するなど）
  - 外灯:
    - 点灯状態: 光源エフェクト（スプライトの輝度変更 or 光のオーバーレイ）
    - 消灯状態: 暗い外見
    - 点灯/消灯の切り替えアニメーション
  - 停電:
    - 画面全体ではなく、消費建物個別の視覚変化
    - 停電中の建物に「電力不足」アイコン表示
- 変更ファイル:
  - `crates/bevy_app/src/systems/visual/` — 各種ビジュアルシステム
  - `assets/textures/` — テクスチャ素材
- 完了条件:
  - [ ] 発電・消費・停電の各状態が視覚的に識別できる
  - [ ] `cargo check` 通過
- 検証:
  - `cargo check`
  - 目視確認

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 継続型タスクの実装複雑度 | task_execution_system の大幅変更 | M3 で早期着手。既存 Refine タスク（単発だが参考になる）の構造を分析 |
| FloorConstructionSite との重複 | 類似だが微妙に異なるエリア型建設ロジックが並存 | 共通化は Phase 1 では追求せず、動くことを優先。Phase 2 以降でリファクタ検討 |
| Dream バランス崩壊 | 発電の Dream コストが高すぎ/低すぎ | 定数を `SoulEnergyConstants` に集約。テストプレイで反復調整 |
| Familiar AI の発電タスク優先度 | 建設中に全員発電に回される等 | 発電タスクの priority を調整可能にし、建設系より低めに設定 |
| Phase 2 移行時の設計変更 | Yard 直結グリッドから Room 接続への移行が困難 | M1 で Relationship ベースのグリッドメンバーシップを導入。Phase 2 では `GeneratesFor` / `ConsumesFrom` のターゲットを付け替えるだけ |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace`
- 手動確認シナリオ:
  1. **基本フロー**: Soul Spa 建設 → 発電開始 → 外灯建設 → ボーナス確認
  2. **停電**: 発電 Soul 離脱 → 停電 → ボーナス消失 → 視覚フィードバック
  3. **復電**: 発電再開 → 復電 → ボーナス復活
  4. **Dream トレードオフ**: 発電 Soul vs 非発電 Soul の Dream 蓄積速度を比較
  5. **スロット制御**: active_slots 変更 → 割り当て可能数の変化
- パフォーマンス確認:
  - PowerGrid 再計算が大量の Soul Spa タイル（50+）で問題ないか

## 8. ロールバック方針

- どの単位で戻せるか: マイルストーン単位。M1→M2→...の順で積み上げるため、途中で止めても既存機能に影響しない
- 戻す時の手順: 該当マイルストーンの変更を revert。データモデル（M1）を戻せば全体が無効化される

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1 から開始

### 次のAIが最初にやること

1. このドキュメントと `milestone-roadmap.md` を読む
2. 既存の FloorConstructionSite / WallConstructionSite の実装を調査し、エリア型建設の共通パターンを把握する
3. 既存のタスク実行フロー（特に Refine タスク）を調査し、継続型タスクの設計方針を固める
4. M1 のデータモデル定義から着手する

### ブロッカー/注意点

- 継続型タスクは前例がない。`task_execution_system` のメインループが「完了」を前提としている可能性がある
- エリア型配置は FloorConstructionSite と WallConstructionSite に前例があるが、「建設完了後も稼働し続ける」点が異なる
- Soul Spa は建設後に「稼働中施設」としてのライフサイクルを持つ（BuildingType としては MudMixer に近い）
- Soul ↔ Tile の在席管理は既存の `WorkingOn` / `TaskWorkers` Relationship で代替する。`GeneratingAt` のような新 Relationship は定義しない
- `GeneratesFor` / `ConsumesFrom` は `hw_core/src/relationships.rs` に定義。既存の Relationship パターン（`Default` impl with `Entity::PLACEHOLDER`、`Vec<Entity>` ベースの Target）に揃えること

### 参照必須ファイル

- `docs/plans/soul-energy/milestone-roadmap.md` — 全体ロードマップ（ECS Relationship 設計セクション必読）
- `docs/tasks.md` — タスクシステム仕様（§2 コンポーネント接続マップ）
- `docs/building.md` — 建築システム仕様（FloorConstructionSite §9）
- `docs/logistics.md` — 物流仕様（TransportRequest）
- `docs/dream.md` — Dream システム（トレードオフの対象）
- `docs/familiar_ai.md` — Familiar AI（タスク割り当て）
- `crates/hw_core/src/relationships.rs` — 既存 Relationship 定義（パターンの参照元）
- `crates/bevy_app/src/systems/jobs/floor_construction/` — エリア型建設の参考実装
- `crates/bevy_app/src/systems/soul_ai/execute/task_execution/` — タスク実行フレームワーク

### 最終確認ログ

- 最終 `cargo check`: N/A
- 未解決エラー: N/A

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了（M1〜M6）
- [ ] 影響ドキュメントが更新済み（tasks.md, building.md, architecture.md）
- [ ] `cargo check` が成功
- [ ] `cargo clippy --workspace` が 0 warnings
- [ ] 手動確認シナリオが全て通過

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-03-27 | AI (Claude) | 初版作成（ブレスト結果をもとに） |
| 2026-03-27 | AI (Claude) | ECS Relationship 設計反映: `GeneratesFor`/`ConsumesFrom` 2組、Site レベル接続、`WorkingOn`/`TaskWorkers` 再利用 |
