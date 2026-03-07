# Cargo Workspace 移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `cargo-workspace-migration-plan-2026-03-07` |
| ステータス | `In Progress` |
| 作成日 | `2026-03-07` |
| 最終更新日 | `2026-03-08` |
| 作成者 | AI |
| 関連提案 | `docs/proposals/architecture-improvements-2026.md` |
| 関連Issue/PR | N/A |

## 1. 目的

- **解決したい課題**: `src/` 以下にすべてのコードが固まっており、コンパイル単位が1つしかないためリルドが遅い。ドメイン境界も不明確。
- **到達したい状態**: ドメインごとに独立した Cargo クレート（`hw_core`, `hw_components`, `hw_world`, ...）に分割し、変更差分のみ再コンパイルされるようにする。
- **成功指標**: `cargo check --workspace` が通り、`cargo run` で起動できる。

## 2. スコープ

### 対象（In Scope）

- `Cargo.toml` の workspace 設定
- `hw_core`, `hw_components`, `hw_world` のクレート分割（Phase 1-2）
- `bevy_app`（`src/`）側の import パスの修正

### 非対象（Out of Scope）

- `hw_visual`, `hw_ui`, `hw_systems`, `hw_ai` の分割（Phase 3-4、将来作業）
- ゲームロジックの変更

## 3. 現状とギャップ

- **Phase 1（`hw_core` 抽出）**: **完了・コミット済み**
  - `crates/hw_core/`: `constants/`, `game_state.rs`, `events.rs`, `relationships.rs` などが移動済み
  - `crates/hw_components/`: クレートは存在するが中身はほぼ空（スタブ状態）
  - `crates/hw_world/`: クレートは存在するが、`bevy_app` 側との import 整合がまだ取れていない
- **Phase 2（`hw_components`, `hw_world` の充実）**: **未完了・未コミット**
  - セッション中に大量の import 修正を試みたが、バルクスクリプトによる壊れた編集が続いたため全変更を破棄した。

## 4. 実装方針（高レベル）

**依存グラフ（下位から上位へ）:**

```
hw_core (Level 0)
  └─ hw_components (Level 1)
  └─ hw_world      (Level 1)
        └─ bevy_app (root)
```

- 各 Phase は `cargo check` が通った状態でコミットすること。
- バルクスクリプトによる機械的な import 書き換えは禁止。`cargo check` のエラーを1ファイルずつ手動修正すること。

## 5. マイルストーン

### M1: `hw_core` 抽出 ✅ 完了

- `crates/hw_core/` に以下を移動済み:
  - `src/constants/` → `crates/hw_core/src/constants/`
  - `src/game_state.rs` → `crates/hw_core/src/game_state.rs`
  - `src/events.rs` → `crates/hw_core/src/events.rs`
  - `src/relationships.rs` → `crates/hw_core/src/relationships.rs`
- 完了条件:
  - [x] `cargo check` が通る
  - [x] コミット済み

### M2: `hw_components` の充実 🔲 未完了

`bevy_app` 側の `src/systems/` に散らばっているコンポーネントを `hw_components` に移す。

移動対象（主要なもの）:

| 型 | 現在地 | 移動先 |
| --- | --- | --- |
| `ResourceType`, `ResourceItem`, `Stockpile` | `src/systems/logistics/types.rs` | `hw_components/src/logistics.rs` |
| `BelongsTo`, `ReservedForTask`, `BucketStorage` | `src/systems/logistics/types.rs` | `hw_components/src/logistics.rs` |
| `Wheelbarrow`, `WheelbarrowLease`, `WheelbarrowDestination`, `WheelbarrowPendingSince` | `src/systems/logistics/transport_request/components.rs` | `hw_components/src/logistics.rs` |
| `TransportRequest`, `TransportRequestKind`, `TransportPriority`, `TransportDemand`, `TransportPolicy`, `TransportRequestState` | `src/systems/logistics/transport_request/components.rs` | `hw_components/src/logistics.rs` |
| `Door`, `DoorState` | `src/systems/jobs/door.rs` の impl | `hw_components/src/jobs.rs` |
| `Building`, `BuildingType`, `Blueprint`, `Designation`, `WorkType`, `TaskSlots` 等 | `src/systems/jobs/` | `hw_components/src/jobs.rs` |

**作業手順:**
1. `hw_components/src/logistics.rs` に型定義とその `impl` ブロックを追加
2. `bevy_app` 側ファイルで元の定義を削除し、`use hw_components::logistics::...` に置き換え
3. ファイル単位で `cargo check` を確認しながら進める

- 完了条件:
  - [ ] `cargo check --workspace` が通る
  - [ ] コミット

### M3: `hw_world` の充実 🔲 未完了

| 型 | 現在地 | 移動先 |
| --- | --- | --- |
| `WorldMap`, `TerrainType`, `Tile` | `src/world/` | `hw_world/src/map/` |
| `RIVER_X_MIN` 等の定数 | `src/world/` | `hw_world/src/map/` |
| `spawn_terrain_borders` 等のシステム | `src/systems/visual/` | `hw_world/src/map/` |

- 完了条件:
  - [ ] `cargo check --workspace` が通る
  - [ ] コミット

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| バルクスクリプトによる import の二重追加・構文破壊 | 高（実際に発生した） | スクリプト禁止。ファイル単位で手動修正 |
| `E0116`（他クレートの型への impl） | 高 | `impl` ブロックは必ず型定義と同じクレートに置く |
| 重複定義（同名型が bevy_app と hw_components の両方に存在） | 高 | 型定義を hw_components に一元化し、bevy_app 側は削除する |

## 7. 検証計画

- 必須: `cargo check --workspace`（各ファイル変更後に都度実行）
- 最終: `cargo run` で起動確認

## 8. ロールバック方針

- 未コミットの変更は `git restore . && git clean -fd` で破棄できる
- 各マイルストーン = 1コミットの粒度で進めること

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: **Phase 1 完了 / Phase 2 未着手（0%）**
- 完了済みマイルストーン: M1（`hw_core` 抽出）
- 未着手/進行中: M2（`hw_components` 充実）、M3（`hw_world` 充実）

### 次のAIが最初にやること

1. `cargo check 2>&1 | head -50` を実行して現在のエラー一覧を把握する
2. エラーを**1ファイル単位**で修正する（バルクスクリプト禁止）
3. `hw_components/src/logistics.rs` に型定義が不足していれば追加し、`bevy_app` 側の元定義を削除して import を修正する

### ブロッカー/注意点

- **バルク置換スクリプト（`fix_*.py` 等）は使わない**。編集のたびに構文エラーや重複 import が発生し、ループを招いた。
- `E0116`（外部クレートへの impl）が出たら、必ず型定義側クレートに impl を移動させる。
- `src/systems/logistics/types.rs` に `ResourceType`, `ResourceItem` 等がまだ残っている可能性がある。重複定義に注意。
- `src/systems/logistics/transport_request/components.rs` に `WheelbarrowDestination`, `WheelbarrowLease` 等の定義がある。これを hw_components に移した後、元ファイルを `pub use hw_components::logistics::...` の re-export だけにする。

### 参照必須ファイル

- `docs/plans/cargo-workspace-migration-plan.md`（本ドキュメント）
- `crates/hw_components/src/logistics.rs`
- `crates/hw_components/src/jobs.rs`
- `src/systems/logistics/types.rs`
- `src/systems/logistics/transport_request/components.rs`
- `src/systems/jobs/door.rs`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-08` / **不明（変更破棄後の状態）**
- 未解決エラー: Phase 2 未着手のため不明。`cargo check` で確認が必要。

### Definition of Done

- [ ] M2: `hw_components` 充実・`cargo check` 通過・コミット
- [ ] M3: `hw_world` 充実・`cargo check` 通過・コミット
- [ ] `cargo run` で起動確認

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-07` | AI | Phase 1 実施（hw_core 抽出） |
| `2026-03-08` | AI | Phase 2 試行・失敗・変更破棄。引き継ぎドキュメント作成 |
