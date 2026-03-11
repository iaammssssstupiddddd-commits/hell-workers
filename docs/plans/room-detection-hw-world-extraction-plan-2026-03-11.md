# Room Detection `hw_world` 抽出 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `room-detection-hw-world-extraction-plan-2026-03-11` |
| ステータス | `Draft` |
| 作成日 | `2026-03-11` |
| 最終更新日 | `2026-03-11` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/room-detection-hw-world-extraction-proposal-2026-03-11.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `src/systems/room/detection.rs` に pure な room 判定ロジックと ECS apply が同居しており、`hw_world` の責務として切り出せる部分が root shell に残っている。
- 到達したい状態: room detection の入力構築コア、flood fill、妥当性判定、候補データ型を `hw_world` に置き、root 側は Query 収集と Room entity 同期だけを担当する。
- 成功指標:
  - `detect_rooms_system` が thin adapter（入力収集 → hw_world 呼び出し → ECS apply）に縮小される。
  - `validate_rooms_system` が `room.tiles` を hw_world validator に渡すだけになる。
  - `docs/cargo_workspace.md`、`docs/architecture.md`、`docs/room_detection.md` が新しい境界を説明している。

## 2. スコープ

### 対象（In Scope）

- `hw_world` への room detection core module（`room_detection.rs`）新設
- `RoomDetectionInput`、`DetectedRoom`（旧 `RoomCandidate`）、validator など pure data / pure function の移設
- `RoomBounds` を `hw_world` へ移設（`Component` derive を含む。hw_world は既に `bevy` に依存済み）
- root 側 `detect_rooms_system` / `validate_rooms_system` の adapter 化
- 関連 docs と index の更新

### 非対象（Out of Scope）

- `Room` ECS component 自体の crate 移設
- `RoomTileLookup` resource の shared model 化
- `sync_room_overlay_tiles_system` の visual 分離
- dirty mark / cooldown / validation schedule の再設計
- room 判定ルール、save format、ゲーム挙動の変更

## 3. 現状とギャップ

### 現状コード（実装ベース）

`src/systems/room/detection.rs` の現在の関数・型一覧：

| 識別子 | 種別 | 可視性 | 抽出可否 |
| --- | --- | --- | --- |
| `RoomDetectionInput` | struct | `pub(super)` | ✅ hw_world へ移設 → `pub` に変更 |
| `RoomCandidate` | struct | private | ✅ hw_world へ移設（`DetectedRoom` に改名） |
| `build_detection_input` | fn | `pub(super)` | ✅ シグネチャ変更して移設（後述） |
| `room_is_valid_against_input` | fn | `pub(super)` | ✅ シグネチャ変更して移設（後述） |
| `detect_rooms` | fn | private | ✅ hw_world へ移設 |
| `flood_fill_room` | fn | private | ✅ hw_world へ移設 |
| `cardinal_neighbors` | fn | private | ✅ hw_world へ移設 |
| `is_in_map_bounds` | fn | private | ✅ hw_world へ移設（`hw_core` の `MAP_WIDTH`/`MAP_HEIGHT` を直接利用） |
| `detect_rooms_system` | Bevy system | `pub` | ❌ root に残す（ECS adapter に縮小） |

`src/systems/room/components.rs` の現在の型：

| 識別子 | 抽出可否 |
| --- | --- |
| `RoomBounds` | ✅ hw_world へ移設（`Component` + pure data メソッド。bevy dep があるため可） |
| `Room` | ❌ root に残す（ECS component） |
| `RoomOverlayTile` | ❌ root に残す（visual 系） |

`src/systems/room/validation.rs` の依存関係：
- `build_detection_input` と `room_is_valid_against_input` を `super::detection` から import → M2 で hw_world import に切り替える
- 変更は import パスと呼び出し引数の slice 化だけで完結する

### ギャップの根本原因

- `hw_world` の `Cargo.toml` は `bevy`・`hw_core`・`hw_jobs` を依存として持つが、room detection が住んでいない。
- `build_detection_input` が現在 `Query<(Entity, &Building, &Transform)>` を直接受けているため crate 境界が引けない（`Query` は Bevy ECS 型）。
- `room_is_valid_against_input` が `&Room`（ECS component）を受けているため、hw_world 側では `Room` 型を知る必要がある。

## 4. 実装方針

### API 設計（hw_world 側）

```rust
// crates/hw_world/src/room_detection.rs

// 入力記述子（root adapter が Query から変換して渡す）
pub struct RoomDetectionBuildingTile {
    pub grid: (i32, i32),
    pub kind: BuildingType,         // hw_jobs::BuildingType
    pub is_provisional: bool,
    pub has_building_on_top: bool,  // Floor 判定除外用（旧 world_map.has_building(grid)）
}

// 入力（RoomDetectionInput は pub に昇格）
#[derive(Default)]
pub struct RoomDetectionInput {
    pub floor_tiles: HashSet<(i32, i32)>,
    pub solid_wall_tiles: HashSet<(i32, i32)>,
    pub door_tiles: HashSet<(i32, i32)>,
}

// 候補（旧 RoomCandidate を pub に）
pub struct DetectedRoom {
    pub tiles: Vec<(i32, i32)>,
    pub wall_tiles: Vec<(i32, i32)>,
    pub door_tiles: Vec<(i32, i32)>,
    pub bounds: RoomBounds,
}

pub fn build_detection_input(tiles: &[RoomDetectionBuildingTile]) -> RoomDetectionInput
pub fn detect_rooms(input: &RoomDetectionInput) -> Vec<DetectedRoom>

// Room component ではなく tile slice を受け取る（Room 依存を排除）
pub fn room_is_valid_against_input(
    tiles: &[(i32, i32)],
    input: &RoomDetectionInput,
) -> bool
```

`RoomBounds` は hw_world に移し、`Component` derive を維持する（bevy は既存の依存）。

### root adapter の責務（移設後）

```rust
// detect_rooms_system の新構造
fn detect_rooms_system(...) {
    // 1. Query から RoomDetectionBuildingTile を収集
    let tiles: Vec<_> = q_buildings.iter().map(|(_e, b, t)| {
        let grid = WorldMap::world_to_grid(t.translation.truncate());
        RoomDetectionBuildingTile {
            grid,
            kind: b.kind,
            is_provisional: b.is_provisional,
            has_building_on_top: world_map.has_building(grid),
        }
    }).collect();

    // 2. hw_world で純計算
    let input = hw_world::room_detection::build_detection_input(&tiles);
    let detected = hw_world::room_detection::detect_rooms(&input);

    // 3. ECS apply（room entity 生成 + lookup 更新）
    ...
}
```

### Bevy 0.18 / 既存挙動の保全

- `Room` entity への `Transform::default()` 付与は継続必須。overlay の親 transform を失うと visual が壊れる。
- `Commands` / `Query` / `ResMut` は root に残す。
- `RoomTileLookup` は root resource のまま据え置く（今回の抽出対象外）。

### パフォーマンス影響

- アルゴリズム自体は変更しないため、ランタイムコストは中立。
- `Vec<RoomDetectionBuildingTile>` 収集時の一時 allocation が増えるが、room detection は cooldown ガードされており許容範囲内。
- 主効果は compile / test / review 時の変更波及範囲縮小。

## 5. マイルストーン

### M1: `hw_world` に room detection core を追加する

- 変更内容:
  - `crates/hw_world/src/room_detection.rs` を新規作成し、上記 API shape を実装する。
  - `RoomBounds` を `crates/hw_world/src/room_detection.rs` に移設し、`Component` derive を維持する。
  - `crates/hw_world/src/lib.rs` に `pub mod room_detection;` と re-export を追加する。
  - `crates/hw_world/README.md` に room detection module の記述を追加する。
  - module 内に unit test を書き、最低限の仕様回帰を固定する。
- 変更ファイル:
  - `crates/hw_world/src/room_detection.rs`（新規）
  - `crates/hw_world/src/lib.rs`
  - `crates/hw_world/README.md`
- 完了条件:
  - [ ] `hw_world::room_detection::build_detection_input` / `detect_rooms` / `room_is_valid_against_input` が公開 API として存在する
  - [ ] `hw_world::room_detection::RoomBounds`・`RoomDetectionInput`・`DetectedRoom` が pub に揃っている
  - [ ] 以下のケースを pure test で表現できる:
    - `ROOM_MAX_TILES` 超過 → 検出されない
    - マップ外と接続する領域 → 検出されない
    - door が 0 個 → 検出されない
    - 仮設壁（`is_provisional=true`）は `solid_wall_tiles` に入らない
    - 正常な閉鎖領域 → 1 room が返る
  - [ ] root の `RoomBounds` 定義が hw_world import に差し替えられているか、二重定義が解消されている
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world room_detection`

### M2: root room systems を thin adapter に縮小する

- 変更内容:
  - `src/systems/room/detection.rs`:
    - `detect_rooms`・`flood_fill_room`・`cardinal_neighbors`・`is_in_map_bounds`・`RoomDetectionInput`・`RoomCandidate` を削除する。
    - `build_detection_input` を root helper（`collect_building_tiles`）に置き換え、hw_world API を呼ぶ形にする。
    - `room_is_valid_against_input` の root 定義を削除する。
  - `src/systems/room/components.rs`:
    - `RoomBounds` の定義を削除し、`hw_world::room_detection::RoomBounds` を re-export する。
  - `src/systems/room/validation.rs`:
    - import を `super::detection` から `hw_world::room_detection` に切り替える。
    - `room_is_valid_against_input(room, &input)` → `hw_world::room_detection::room_is_valid_against_input(&room.tiles, &input)` に修正する。
  - `src/systems/room/mod.rs`: re-export を整理する（`RoomBounds` の出自が変わるため）。
- 変更ファイル:
  - `src/systems/room/detection.rs`
  - `src/systems/room/components.rs`
  - `src/systems/room/validation.rs`
  - `src/systems/room/mod.rs`
- 完了条件:
  - [ ] root 側から `detect_rooms` / `flood_fill_room` / `RoomCandidate` / `is_in_map_bounds` / `cardinal_neighbors` が消えている
  - [ ] `detect_rooms_system` が「Query → `RoomDetectionBuildingTile` 収集 → hw_world 呼び出し → despawn → spawn → lookup 更新」の構造になっている
  - [ ] `validate_rooms_system` が `hw_world::room_detection::room_is_valid_against_input(&room.tiles, &input)` を呼ぶだけになっている
  - [ ] `RoomBounds` の二重定義が解消されている
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world room_detection`

### M3: docs と運用上の境界を同期する

- 変更内容:
  - `docs/cargo_workspace.md`: room detection core が `hw_world` 所有であることを追記する。
  - `docs/architecture.md`: room detection の「crate core + root shell」境界を追記する。
  - `docs/room_detection.md` と `src/systems/room/README.md`: algorithm の所在（`hw_world::room_detection`）と root で残る責務（入力収集・ECS apply・dirty scheduling）を明記する。
  - proposal の `関連計画` が本ファイルを指すことを確認・修正する。
  - `python scripts/update_docs_index.py` で index を再生成する。
- 変更ファイル:
  - `docs/cargo_workspace.md`
  - `docs/architecture.md`
  - `docs/room_detection.md`
  - `src/systems/room/README.md`
  - `docs/proposals/room-detection-hw-world-extraction-proposal-2026-03-11.md`
  - `docs/plans/README.md`（存在する場合）
  - `docs/proposals/README.md`（存在する場合）
- 完了条件:
  - [ ] docs 上で room detection core の所在と root shell の責務が矛盾なく説明されている
  - [ ] 新規 plan が docs index に載っている
  - [ ] proposal の `関連計画` リンクが本ファイルを正しく指している
- 検証:
  - `python scripts/update_docs_index.py`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `build_detection_input` が `Query` / `WorldMapRead` を直接受ける形のまま移設される | `hw_world` が Bevy ECS に直接依存し crate 境界が崩れる | `RoomDetectionBuildingTile` スライスを受ける形に変換し、Query 反復は root adapter に留める |
| `RoomBounds` を root と hw_world で二重定義のまま残す | 境界が曖昧になり次の refactor で混乱する | hw_world を唯一の定義元とし、root は `pub use hw_world::room_detection::RoomBounds` に変える |
| `room_is_valid_against_input` が `&Room` を受け続ける | hw_world が `Room` 型を知らねばならずコンパイルエラーか循環依存 | `tiles: &[(i32, i32)]` だけを受ける形に slim 化し、`Room` 知識を root 側で吸収する |
| `has_building_on_top` フラグの意味が不明確になる | `Floor` タイル除外ロジックが hw_world 側に正しく反映されない | `RoomDetectionBuildingTile::has_building_on_top` のコメントに「`world_map.has_building(grid)` の結果」と明記する |
| `Transform::default()` が Room entity spawn で省略される | overlay visual が壊れる（親 transform が消える） | M2 の完了条件チェックリストに明示し、`sync_room_overlay_tiles_system` の動作確認を手動検証に含める |
| docs 更新漏れで crate 境界の説明が古いままになる | 次の refactor で誤配置が起きる | M3 を DoD に含め、index 更新スクリプトまで実行する |

## 7. 検証計画

### 自動検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world room_detection
```

### 追加すべき unit test（M1 で実装）

| テスト名 | 検証内容 |
| --- | --- |
| `test_closed_room_with_door` | 床 × N + 壁全周 + ドア 1 → room 1 件検出 |
| `test_open_region_is_not_a_room` | 壁が欠けた領域 → 検出されない |
| `test_map_boundary_contact_is_not_a_room` | マップ外タイルに接する領域 → 検出されない |
| `test_room_max_tiles_exceeded` | `ROOM_MAX_TILES + 1` 床 → 検出されない |
| `test_provisional_wall_not_solid` | `is_provisional=true` の壁は `solid_wall_tiles` に入らない |
| `test_no_door_is_not_a_room` | ドアのない閉鎖領域 → 検出されない |
| `test_valid_room_passes_validator` | 検出済み room tiles が validator を通過する |
| `test_invalid_room_fails_validator` | 床が足りない tiles が validator で false になる |

### 手動確認シナリオ

- 壁・床・ドアで囲まれた部屋が従来どおり生成される
- 壁欠けやマップ外接続のある領域が room にならない
- 仮設壁のままでは room が成立しない
- door を取り除くと validation か再検出で room が消える
- overlay が room 位置に追従し、原点へ崩れない（`Transform::default()` 保全確認）

### パフォーマンス確認（必要時）

既存セーブまたは高密度建築シナリオで room 再検出時のフレーム落ちに有意差がないことを目視確認する。

## 8. ロールバック方針

- M1 は `hw_world` module 追加だけなので単独で巻き戻せる。
- M2 は root adapter の import 先を旧実装へ戻せば切り戻し可能。
- M3 は docs のみなので機能ロールバックとは独立して戻せる。

戻す時の手順:
1. root `detection.rs` に一時的に旧 pure helper を戻す、または抽出前コミットへ戻す。
2. `components.rs` で `RoomBounds` の定義元を切り戻す。
3. `hw_world` export と docs 参照を削除し、index を再生成する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1-M3 すべて未着手

### 作業開始前に確認すべきこと

1. `crates/hw_world/Cargo.toml` — `bevy`, `hw_core`, `hw_jobs` が依存に含まれていることを再確認。
2. `src/systems/room/detection.rs` — `CARDINAL_OFFSETS` 定数と `is_in_map_bounds` が `hw_core::constants` を使っていることを確認。hw_world も hw_core を依存するので、同じ定数を参照可能。
3. `crates/hw_world/src/lib.rs` に現在 room 関連の mod が存在しないことを確認。

### 実装上の最重要注意

- **`Transform::default()` の保全**: Room entity spawn 時に必須。省略すると `sync_room_overlay_tiles_system` で overlay が原点に崩れる。
- **`RoomDetectionBuildingTile::has_building_on_top`**: `world_map.has_building(grid)` の結果を渡す。`BuildingType::Floor` タイルで別建物が重なっている場合に floor_tiles から除外するためのフラグ。
- **`room_is_valid_against_input` の引数変更**: root 側の呼び出し元は `validation.rs` の 1 箇所のみ。`room_is_valid_against_input(room, &input)` → `hw_world::room_detection::room_is_valid_against_input(&room.tiles, &input)` に変更するだけで完結する。
- **`RoomTileLookup`**: root resource のまま据え置き。今回の抽出対象に含めない。

### 参照必須ファイル

- `docs/proposals/room-detection-hw-world-extraction-proposal-2026-03-11.md`
- `docs/cargo_workspace.md`
- `docs/architecture.md`
- `docs/room_detection.md`
- `crates/hw_world/src/lib.rs`
- `crates/hw_world/Cargo.toml`
- `src/systems/room/detection.rs`
- `src/systems/room/components.rs`
- `src/systems/room/validation.rs`

### 最終確認ログ

- 最終 `cargo check`: `未実行`（コード変更なし / docs only）
- 未解決エラー: なし

### Definition of Done

- [ ] M1-M3 が完了している
- [ ] room detection core と root shell の境界が code / docs の両方で一致している
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功している
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world room_detection` が成功している

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-11` | `Codex` | 初版作成 |
| `2026-03-11` | `Copilot` | コード調査に基づきブラッシュアップ（API 設計具体化・移設対象一覧追加・test ケース列挙・CARGO_HOME 修正） |
