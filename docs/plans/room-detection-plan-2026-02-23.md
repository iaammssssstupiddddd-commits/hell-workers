# Room検出機能 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `room-detection-plan-2026-02-23` |
| ステータス | `Draft` |
| 作成日 | `2026-02-23` |
| 最終更新日 | `2026-02-23` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/room_detection.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  - 壁・扉・床で囲われた空間を、ゲーム内で「Room」として論理認識できない。
  - Room系機能（温度・モラル・部屋品質）の前提データが存在しない。
  - プレイヤーが囲いを作っても、成立/不成立の視覚フィードバックがない。
- 到達したい状態:
  - 「完成壁 + ドア1以上 + 全面完成床」で閉じた領域を自動検出できる。
  - 検出結果を `Room` エンティティで保持し、タイル単位オーバーレイで可視化できる。
  - 壁破壊や床欠損などの変化時に、Roomが自動で再判定される。
- 成功指標:
  - 提案書 `docs/proposals/room_detection.md` の手動確認シナリオが全て再現できる。
  - 検出漏れ/残留Roomが起きない（壁破壊・キャンセル後に自動回復）。
  - `cargo check` が成功する。

## 2. スコープ

### 対象（In Scope）

- `src/systems/room/` の新設（Room定義、dirty追跡、検出、検証）。
- Flood-fill ベースの Room 検出（4近傍、上限タイル数あり）。
- Roomごとのタイルオーバーレイ表示（半透明）。
- ロジック/ビジュアルプラグインへのシステム登録。
- 建築仕様ドキュメントへのRoom検出追記。

### 非対象（Out of Scope）

- Room効果（バフ/デバフ）適用。
- Roomタイプ自動分類（寝室、作業場など）。
- Room名編集UI、Room一覧UI。
- セーブデータへのRoom永続化（毎回再検出前提）。

## 3. 現状とギャップ

- 現状:
  - `Building` は個別エンティティで管理されるが、閉領域の集合概念はない。
  - `WorldMap.buildings` は壁/扉/各種建物の占有管理に使われる。
  - `FloorConstruction` 完了で生成される `BuildingType::Floor` は `WorldMap.buildings` に入らない。
  - 壁建設中サイトなど、`WorldMap.buildings` には `Building` 以外のエンティティも混在する。
- 問題:
  - `WorldMap.buildings` だけでは、Room判定に必要な「完成床」の取得が不完全になる。
  - 追加/変更だけでなく、削除（despawn）起点の再判定漏れが起きやすい。
  - プレイヤーが囲いを作っても成立可否を判断できない。
- 本計画で埋めるギャップ:
  - `Building + Transform` クエリから床/壁/扉の判定用キャッシュを毎回再構築する。
  - dirty追跡を「Changed系 + WorldMap差分」の二重化で安定化する。
  - Room成立時のみ `Room` エンティティを生成し、表示まで接続する。

## 4. 実装方針（高レベル）

- 方針:
  - 検出対象は「完成済み `Building`」のみ（Blueprint/建設中タイルは除外）。
  - dirtyタイル起点で局所再計算しつつ、定期検証で自己修復する。
  - Room境界判定は「床集合の外側が壁/扉のみ」を厳密条件とする。
- 設計上の前提:
  - Room条件:
    - 内部タイルはすべて `BuildingType::Floor`。
    - 境界は `BuildingType::Wall`（`is_provisional == false`）または `BuildingType::Door`。
    - 境界ドア数が1以上。
    - `ROOM_MAX_TILES` 以下。
  - 扉の `DoorState`（Open/Closed/Locked）はRoom境界判定に影響しない（Doorであれば境界有効）。
  - `WorldMap` の地形 (`tiles`) はRoom成立判定に使わない（Building実体優先）。
- Bevy 0.18 APIでの注意点:
  - 変更検出は `Added<T>` / `Changed<T>` と `RemovedComponents<T>` の責務を分離する。
  - 削除起点の座標回収は `RemovedComponents<Building>` 単独では困難なため、`WorldMap.buildings` 差分を併用する。
  - despawnは `try_despawn()` を使い、二重破棄時の失敗で処理を止めない。
  - `Logic` と `Visual` の順序は `GameSystemSet` の実行順（`Logic -> Actor -> Visual`）に従う。

## 5. マイルストーン

## M1: Roomデータモデルと定数の追加

- 変更内容:
  - `Room` / `RoomBounds` / `RoomOverlayTile` コンポーネントを定義。
  - `RoomDetectionState` / `RoomTileLookup` リソースを定義。
  - Room検出定数（上限、クールダウン、検証間隔、オーバーレイ色）を追加。
- 変更ファイル:
  - `src/systems/room/mod.rs`（新規）
  - `src/systems/room/components.rs`（新規）
  - `src/systems/room/resources.rs`（新規）
  - `src/constants/building.rs`
  - `src/constants/render.rs`
  - `src/systems/mod.rs`
- 完了条件:
  - [ ] Room関連型がコンパイル可能な状態で定義されている。
  - [ ] `constants` 経由でRoom定数を参照できる。
- 検証:
  - `cargo check`

## M2: dirty追跡と判定入力キャッシュの実装

- 変更内容:
  - `Added/Changed<Building>`、`Added/Changed<Door>` からdirtyタイルを収集。
  - `WorldMap.buildings` 前回スナップショットとの差分で、削除/置換起点のdirtyも収集。
  - 判定入力として以下の集合を毎回構築:
    - `floor_tiles`（完成床）
    - `solid_wall_tiles`（完成壁のみ）
    - `door_tiles`（完成扉）
  - 収集したdirtyに1タイル近傍を加算し、境界変更の取りこぼしを減らす。
- 変更ファイル:
  - `src/systems/room/dirty_mark.rs`（新規）
  - `src/systems/room/detection.rs`（新規）
  - `src/plugins/logic.rs`
- 完了条件:
  - [ ] 建物追加/変更/削除でdirty集合が更新される。
  - [ ] `WorldMap.buildings` に存在しない床（FloorConstruction由来）も検出入力へ反映される。
- 検証:
  - `cargo check`

## M3: Flood-fill検出とRoomエンティティ同期

- 変更内容:
  - dirty床タイルをseedに4近傍Flood-fillを実装。
  - 不成立条件:
    - 範囲外到達
    - 外周が壁/扉以外
    - タイル数上限超過
    - ドア不足
  - 成立候補から `Room` を生成/更新し、古いRoomを整理。
  - `RoomTileLookup` を再構築して `tile -> room` を逆引き可能にする。
- 変更ファイル:
  - `src/systems/room/detection.rs`
  - `src/systems/room/components.rs`
  - `src/systems/room/resources.rs`
  - `src/plugins/logic.rs`
- 完了条件:
  - [ ] 同一dirty範囲で重複Roomが生成されない。
  - [ ] Room破壊時（壁除去、床欠損）にRoomが消える。
  - [ ] `RoomTileLookup` が最新Room状態と一致する。
- 検証:
  - `cargo check`

## M4: 既存Room検証と自己修復

- 変更内容:
  - 2秒間隔の検証システムを追加。
  - 既存Roomのタイル/境界条件を再評価し、不正Roomを破棄してdirty再投入。
  - dirtyが空でも復旧可能なよう、検証起点の再検出導線を用意。
- 変更ファイル:
  - `src/systems/room/validation.rs`（新規）
  - `src/systems/room/resources.rs`
  - `src/plugins/logic.rs`
- 完了条件:
  - [ ] 破壊/キャンセル後にRoom残留が発生しない。
  - [ ] 検証周期で自己修復できる。
- 検証:
  - `cargo check`

## M5: Roomオーバーレイ表示

- 変更内容:
  - Roomタイルごとに半透明スプライトを生成（`RoomOverlayTile` マーカー付与）。
  - Room更新時にオーバーレイの生成/破棄を同期。
  - `Z_ROOM_OVERLAY` で描画順を固定。
- 変更ファイル:
  - `src/systems/room/visual.rs`（新規）
  - `src/plugins/visual.rs`
  - `src/constants/render.rs`
- 完了条件:
  - [ ] 成立Roomだけにオーバーレイが表示される。
  - [ ] Room消滅時にオーバーレイが残留しない。
  - [ ] 壁/床/ユニットより過剰に前面化しない描画順になっている。
- 検証:
  - `cargo check`

## M6: 配線・ドキュメント更新・最終確認

- 変更内容:
  - ロジック/ビジュアルプラグインへの登録順を最終確定。
  - `docs/building.md` にRoom検出仕様を追記。
  - 提案書の関連計画を更新し、計画書との相互リンクを整備。
- 変更ファイル:
  - `src/plugins/logic.rs`
  - `src/plugins/visual.rs`
  - `docs/building.md`
  - `docs/proposals/room_detection.md`
  - `docs/plans/README.md`
- 完了条件:
  - [ ] Room機能がプラグインから有効化される。
  - [ ] 関連ドキュメントに参照欠落がない。
  - [ ] `cargo check` が成功する。
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `WorldMap.buildings` だけ参照して床を見逃す | Room未検出 | `BuildingType::Floor` を `Query<(&Building, &Transform)>` から直接収集 |
| 削除起点でdirty漏れ（RemovedComponentsに座標がない） | Room残留 | `WorldMap.buildings` 差分追跡を併用 |
| dirty範囲の増加で再検出コスト上昇 | フレーム低下 | クールダウン、最大タイル上限、dirty近傍展開を1タイルに限定 |
| provisional壁を境界に含める誤判定 | 建設途中でRoom成立 | `BuildingType::Wall && !is_provisional` のみ境界有効 |
| オーバーレイ残留/重複 | 視覚破綻 | `RoomOverlayTile` と親Roomエンティティで一意管理し、同期時に不要分を破棄 |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - 5x5を壁+ドア1+全面床で囲む -> Room成立、オーバーレイ表示。
  - 壁1枚を破壊 -> Room消滅。
  - ドア無し完全閉鎖 -> Room不成立。
  - 仮設壁のみで囲む -> Room不成立。
  - 床1タイル欠損 -> Room不成立。
  - L字・不規則形状 -> Room成立（境界条件を満たす場合）。
  - 共有壁で2部屋 -> 別Roomで検出。
  - FloorConstruction完了直後（`WorldMap.buildings` 非登録床）でもRoom成立。
- パフォーマンス確認（必要時）:
  - 壁・扉を連続編集した際の検出ログ（処理件数/room数）を確認し、必要ならクールダウン調整。

## 8. ロールバック方針

- どの単位で戻せるか:
  - ロジックのみ停止: `src/plugins/logic.rs` のRoomシステム登録を外す。
  - 表示のみ停止: `src/plugins/visual.rs` のRoomオーバーレイ同期を外す。
  - 完全撤回: `src/systems/room/` とRoom定数を削除し、参照を戻す。
- 戻す時の手順:
  1. `src/plugins/logic.rs` からRoom検出/検証システムを削除。
  2. `src/plugins/visual.rs` からRoomオーバーレイ同期を削除。
  3. 未使用のRoom関連型・定数を整理し、`cargo check` で確認。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1` 以降すべて未着手

### 次のAIが最初にやること

1. `M1` として `src/systems/room/` の型定義と定数を追加する。
2. `M2` で dirty 収集（Changed系 + `WorldMap.buildings` 差分）を実装する。
3. `M3` で Flood-fill 実装と `Room` エンティティ同期を接続する。

### ブロッカー/注意点

- `FloorConstruction` 由来の床は `WorldMap.buildings` に入らないため、床判定は `Building + Transform` クエリ必須。
- `WorldMap.buildings` には建設中siteエンティティが入るため、`Building` 成分の有無でフィルタすること。
- Door置換や壁キャンセルで短時間に map 差分が大きく変わるため、dirty収集は削除系に強い実装を優先すること。

### 参照必須ファイル

- `docs/proposals/room_detection.md`
- `src/world/map/mod.rs`
- `src/systems/jobs/mod.rs`
- `src/systems/jobs/door.rs`
- `src/systems/jobs/floor_construction/completion.rs`
- `src/systems/jobs/wall_construction/phase_transition.rs`
- `src/plugins/logic.rs`
- `src/plugins/visual.rs`
- `src/constants/building.rs`
- `src/constants/render.rs`

### 最終確認ログ

- 最終 `cargo check`: `未実行`（計画書作成のみ）
- 未解決エラー: `N/A`

### Definition of Done

- [ ] Room検出ロジック（dirty追跡 + Flood-fill + 検証）が動作する
- [ ] Roomオーバーレイ表示が同期される
- [ ] 関連ドキュメント（提案/建築仕様/計画一覧）が更新される
- [ ] `cargo check` が成功する

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-23` | `Codex` | 初版作成 |
