# GatherWater / HaulWaterToMixer タスクの統合

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `water-transport-consolidation-proposal-2026-03-07` |
| ステータス | `Draft` |
| 作成日 | `2026-03-07` |
| 最終更新日 | `2026-03-07` |
| 作成者 | `AI (Claude)` |
| 関連計画 | `TBD` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- 現状: `GatherWater`（River → Tank）と `HaulWaterToMixer`（Tank → Mixer）は、ほぼ同一の「バケツ搬送」パターンを独立に実装している:
  - `GatherWater`: 5 フェーズ（GoingToBucket → GoingToRiver → Filling → GoingToTank → Pouring）
  - `HaulWaterToMixer`: 6 フェーズ（GoingToBucket → GoingToTank → FillingFromTank → GoingToMixer → Pouring → ReturningBucket）
  - 合計 **20ファイル**（各 `phases/` 下に 5〜6 ファイル + `mod.rs`, `guards.rs`/`transitions.rs`, `routing.rs`/`abort.rs`, `helpers.rs`）
- 問題:
  - 90% の共通ロジック（バケツ取得、移動先設定、インベントリ管理、ガード判定）が独立に書かれている
  - 例: `going_to_bucket.rs` は GatherWater 版（117行）と HaulWaterToMixer 版（115行）でほぼ同一
  - バケツ関連のバグ修正や仕様変更が 2 系統に波及する
  - `HaulWaterToMixer` に最近追加されたガード（タンク容量チェック等）が `GatherWater` にも個別に追加されている

- なぜ今やるか: 提案 001・002 でバリデーションとイテレーションの共通化が進んだ後に着手するのが最も整合性が高い。水搬送は今後の拡張（複数水源、バケツ種別等）の基盤になるため、先に統合しておくことで将来の変更コストを削減できる。

## 2. 目的（Goals）

- `GatherWater` と `HaulWaterToMixer` を **パラメータ化された単一のバケツ搬送タスク** に統合
- フェーズハンドラファイルを 20 → 10 程度に削減
- バケツ関連ロジックの変更箇所を 1 系統に集約

## 3. 非目的（Non-Goals）

- 全 16 `AssignedTask` バリアントの汎用タスクフレームワーク化（過剰抽象化。水搬送の 2 タスクに限定）
- `Haul` / `HaulWithWheelbarrow` 等の他の運搬タスクの統合（構造が大きく異なる）
- `WorkType` / `TransportRequestKind` の統合・削除（既存の producer / task_finder / score が依存しているため影響が大きい）
- `lifecycle.rs` の 250 行 match 文の解消（本提案の範囲外。統合後にバリアントが 1 つ減るため自然に簡素化される）

## 4. 提案内容（概要）

- 一言要約: `GatherWater` と `HaulWaterToMixer` を `BucketTransport` に統合し、水源（River/Tank）と搬送先（Tank/Mixer）をパラメータで切り替える
- 主要な変更点:
  1. `AssignedTask::BucketTransport { data: BucketTransportData }` バリアントを新設
  2. `BucketTransportData` にソース種別（`River` / `Tank`）とデスティネーション種別（`Tank` / `Mixer`）を持たせる
  3. 共通フェーズ: `GoingToBucket → GoingToSource → Filling → GoingToDestination → Pouring [→ ReturningBucket]`
  4. 既存の `GatherWater` / `HaulWaterToMixer` バリアントを `BucketTransport` のエイリアスまたは変換に置換
  5. フェーズハンドラを共通化し、ソース/デスティネーション依存の差分のみ分岐
- 期待される効果:
  - ~1000行の重複コード削減
  - バケツ関連バグ修正が 1 箇所で完結
  - 将来の水源追加（例: 井戸）がパラメータ追加で対応可能

## 5. 詳細設計

### 5.1 仕様

**新しいデータ構造:**

```rust
// src/systems/soul_ai/execute/task_execution/types.rs

pub enum WaterSource {
    River,
    Tank(Entity),
}

pub enum WaterDestination {
    Tank(Entity),
    Mixer(Entity),
}

pub struct BucketTransportData {
    pub bucket: Entity,
    pub source: WaterSource,
    pub destination: WaterDestination,
    pub phase: BucketTransportPhase,
}

pub enum BucketTransportPhase {
    GoingToBucket,
    GoingToSource,
    Filling { progress: f32 },
    GoingToDestination,
    Pouring { progress: f32 },
    ReturningBucket,  // Mixer 向けのみ使用
    Done,
}
```

**フェーズ遷移:**

| フェーズ | River → Tank | Tank → Mixer |
|:---|:---|:---|
| GoingToBucket | バケツ位置へ移動 | バケツ位置へ移動 |
| GoingToSource | 河川タイルへ移動 | タンク位置へ移動 |
| Filling | 河川から充填（即時） | タンクから充填（タンク水量チェック） |
| GoingToDestination | タンク位置へ移動 | ミキサー位置へ移動 |
| Pouring | タンクへ注水 | ミキサーへ注水 |
| ReturningBucket | スキップ（タンク隣にバケツ配置） | バケツをストレージへ返却 |

**分岐ポイント:**
- `Filling`: `WaterSource::River` は即時完了、`WaterSource::Tank` はタンク水量チェック + 減算
- `Pouring`: `WaterDestination::Tank` はタンク水量加算、`WaterDestination::Mixer` はミキサーへ注水
- `ReturningBucket`: `WaterDestination::Tank` の場合はスキップ（バケツをタンク隣に配置して終了）、`WaterDestination::Mixer` の場合はバケツストレージへ返却

**ガード（タスク開始前チェック）:**
- 共通: バケツ存在チェック、バケツ位置の歩行可能チェック
- Tank 向け追加: タンク容量チェック（`tank_can_accept_new_bucket` — `logistics/water.rs`）
- Mixer 向け追加: ミキサー存在チェック、水容量チェック

**AssignedTask の移行:**
- `AssignedTask::GatherWater(data)` → `AssignedTask::BucketTransport(BucketTransportData { source: River, destination: Tank(tank), ... })`
- `AssignedTask::HaulWaterToMixer(data)` → `AssignedTask::BucketTransport(BucketTransportData { source: Tank(tank), destination: Mixer(mixer), ... })`
- `WorkType::GatherWater` / `WorkType::HaulWaterToMixer` は **変更しない**（producer / task_finder / score が依存）
- builders（`builders/water.rs`）で `WorkType` → `BucketTransportData` の変換を行う

### 5.2 変更対象（想定）

**新規作成:**
- `src/systems/soul_ai/execute/task_execution/bucket_transport/` ディレクトリ
  - `mod.rs` — ハンドラ entry point
  - `guards.rs` — 共通ガード
  - `phases/going_to_bucket.rs`
  - `phases/going_to_source.rs` — River / Tank 分岐
  - `phases/filling.rs` — River / Tank 分岐
  - `phases/going_to_destination.rs` — Tank / Mixer 分岐
  - `phases/pouring.rs` — Tank / Mixer 分岐
  - `phases/returning_bucket.rs` — Mixer のみ

**変更:**
- `src/systems/soul_ai/execute/task_execution/types.rs` — `BucketTransportData` / `BucketTransportPhase` 追加、旧型の削除
- `src/systems/soul_ai/execute/task_execution/mod.rs` — ディスパッチの切り替え
- `src/systems/soul_ai/execute/task_execution/handler/dispatch.rs` — `BucketTransport` ハンドラ登録
- `src/systems/familiar_ai/decide/task_management/builders/water.rs` — `BucketTransportData` を構築するように変更
- `src/systems/soul_ai/execute/task_execution/transport_common/lifecycle.rs` — `BucketTransport` の予約操作マッピング追加（旧2バリアント分を統合）

**削除:**
- `src/systems/soul_ai/execute/task_execution/gather_water/` ディレクトリ全体（10ファイル）
- `src/systems/soul_ai/execute/task_execution/haul_water_to_mixer/` ディレクトリ全体（10ファイル）

### 5.3 データ/コンポーネント/API 変更

- 追加: `BucketTransportData`, `BucketTransportPhase`, `WaterSource`, `WaterDestination`
- 変更: `AssignedTask` enum（2バリアント削除、1バリアント追加）
- 削除: `GatherWaterData`, `GatherWaterPhase`, `HaulWaterToMixerData`, `HaulWaterToMixerPhase`

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| A: パラメータ化 `BucketTransport`（本提案） | 採用 | 2タスクの差分が明確で、パラメータ化に最適。~10ファイル削減 |
| B: 汎用 `PhaseMachine` trait で全タスクを統一 | 不採用 | 16タスクの差異が大きく、trait ボイラープレートが増大。ROI が低い |
| C: 共通ヘルパーの抽出のみ（構造は維持） | 不採用 | フェーズファイルの重複は残る。根本的な削減にならない |
| D: `HaulWaterToMixer` を廃止し `GatherWater` + 自動 Haul に分解 | 不採用 | ゲームプレイ上、バケツの連続搬送（Tank→Mixer の直通）が重要。分解すると効率が低下 |

## 7. 影響範囲

- ゲーム挙動: 変更なし（フェーズ遷移と計算ロジックは同一）
- パフォーマンス: 変更なし（フェーズハンドラの呼び出しコストは同一）
- UI/UX: タスクリスト表示で `GatherWater` / `HaulWaterToMixer` のラベルは `WorkType` ベースなので変更不要
- セーブ互換: `AssignedTask` がセーブ対象でない限り影響なし（確認必要）
- 既存ドキュメント更新:
  - `docs/tasks.md` §4.3 — `GatherWater` / `HaulWaterToMixer` のフェーズ説明を `BucketTransport` に統合
  - `docs/soul_ai.md` §3 — タスク実行ロジックの記述更新
  - `docs/logistics.md` §4.4, §4.7 — 参照先モジュール名の更新

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `lifecycle.rs` の予約マッピングの移行漏れ | 予約リーク（解放されないソース予約） | 旧2バリアントの予約操作を列挙し、新バリアントの match arm で網羅確認 |
| `task_finder/score.rs` の `WorkType` スコアへの影響 | スコア計算が変わる | `WorkType` は変更しないため影響なし |
| `unassign_task` のアイテムドロップロジック | バケツが正しくドロップ/返却されない | 既存の `HaulWaterToMixer` の abort ロジックを `BucketTransport` に統合。返却フェーズは `WaterDestination::Mixer` のみ |
| 変更量が大きく、一括マージのリスク | コンフリクトや regression | 段階導入（下記）で対応 |

## 9. 検証計画

- `cargo check`
- 手動確認シナリオ:
  - `GatherWater`: Soul がバケツを持って河川→タンクへ水を運べること
  - `HaulWaterToMixer`: Soul がバケツを持ってタンク→ミキサーへ水を運べること
  - タスク中断: バケツが正しくドロップされること
  - タンク満杯: 水充填がガードされること
  - ミキサー破壊: abort が正しく動作すること
  - バケツ返却: Mixer 搬送後にバケツがストレージに戻ること
- 計測/ログ確認: `TASK_EXEC:` ログが従来と同等の情報を出力すること

## 10. ロールアウト/ロールバック

- 導入手順:
  1. `BucketTransportData` / `BucketTransportPhase` を `types.rs` に追加（旧型は残す）
  2. `bucket_transport/` ディレクトリを作成し、共通フェーズハンドラを実装
  3. `builders/water.rs` を `BucketTransportData` 構築に切り替え
  4. ディスパッチを切り替え、旧ハンドラへの呼び出しを削除
  5. 旧 `gather_water/` / `haul_water_to_mixer/` ディレクトリを削除
  6. `lifecycle.rs` の予約マッピングを更新
- 段階導入の有無: あり。Step 1-2 で新旧並存が可能。Step 3 で切り替え、Step 5 で cleanup。
- 問題発生時の戻し方: Step 3 の前なら旧ハンドラが残っているため、ディスパッチを元に戻すだけで復帰可能。

## 11. 未解決事項（Open Questions）

- [ ] `AssignedTask` はセーブデータに含まれるか？含まれる場合、旧バリアントとの互換性をどう扱うか。
- [ ] `ReturningBucket` フェーズは `GatherWater`（River→Tank）でもオプションとして用意すべきか？（現在は Tank 隣にバケツを置くだけだが、将来的にバケツストレージへの返却が必要になる可能性）
- [ ] `WorkType::GatherWater` / `WorkType::HaulWaterToMixer` を `WorkType::BucketTransport` に統合するか？producer / score への波及が大きいため、初期実装では変更しない方針だが、長期的にはどうするか。

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 直近で完了したこと: 提案書の作成
- 現在のブランチ/前提: `master`

### 次のAIが最初にやること

1. 提案 001・002 の完了を確認（依存はないが、同時変更の衝突を避けるため）
2. `gather_water/` と `haul_water_to_mixer/` の全ファイルを精読し、差分一覧を作成
3. `BucketTransportData` / `BucketTransportPhase` を `types.rs` に追加
4. 共通フェーズから順に実装（`GoingToBucket` → `Filling` → `Pouring`）
5. 各段階で `cargo check`

### ブロッカー/注意点

- 変更量が大きい（20ファイル削除 + 10ファイル新規）。他の変更セットとの衝突に注意。
- `lifecycle.rs` の `collect_active_reservation_ops` 内の `GatherWater` / `HaulWaterToMixer` match arm を `BucketTransport` に統合する際、予約操作の網羅性を慎重に確認すること。
- `abort.rs`（`HaulWaterToMixer`）のバケツクリーンアップロジックは `GatherWater` の `helpers.rs` と異なる。統合時に両方のロジックを正しくマージすること。

### 参照必須ファイル

- `src/systems/soul_ai/execute/task_execution/gather_water/` — 全ファイル
- `src/systems/soul_ai/execute/task_execution/haul_water_to_mixer/` — 全ファイル
- `src/systems/soul_ai/execute/task_execution/types.rs` — `AssignedTask` enum 定義
- `src/systems/soul_ai/execute/task_execution/transport_common/lifecycle.rs` — 予約操作マッピング
- `src/systems/familiar_ai/decide/task_management/builders/water.rs` — タスク構築
- `src/systems/logistics/water.rs` — タンク容量チェック共通関数

### 完了条件（Definition of Done）

- [ ] `AssignedTask::BucketTransport` バリアントが存在する
- [ ] `gather_water/` と `haul_water_to_mixer/` ディレクトリが削除されている
- [ ] `bucket_transport/` ディレクトリに統合フェーズハンドラが存在する
- [ ] `lifecycle.rs` の予約マッピングが `BucketTransport` に対応している
- [ ] `cargo check` がエラーなしで通過する
- [ ] River→Tank、Tank→Mixer の両方の水搬送が正しく動作すること（手動テスト）
- [ ] タスク中断時にバケツが正しくドロップ/返却されること

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-07` | `AI (Claude)` | 初版作成 |
