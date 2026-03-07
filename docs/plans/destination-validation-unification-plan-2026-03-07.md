# 搬入先バリデーション一元化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `destination-validation-unification-plan-2026-03-07` |
| ステータス | `Draft` |
| 作成日 | `2026-03-07` |
| 最終更新日 | `2026-03-07` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/destination-validation-unification-proposal-2026-03-07.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: FloorConstruction / WallConstruction / ProvisionalWall の搬入先需要計算と実行時受入判定が、割り当て時・手運搬 dropping 時・猫車 unloading 時の 3 系統に分散し、同一ロジックを複数箇所で維持している。
- 到達したい状態: 「タイル状態から見た基礎需要」と「周辺地面資材の数え上げ」を `src/systems/logistics/` 配下の共通関数へ集約し、呼び出し側は `IncomingDeliveries` / `ReservationShadow` / `nearby_ground_resources` のどれを差し引くかだけを責務として持つ。
- 成功指標:
  - `demand.rs` / `dropping.rs` / `unloading.rs` から floor / wall / provisional 用の重複ローカル関数が除去されている。
  - 新しい建設系搬入先を追加するときの実装開始点が `src/systems/logistics/` に一本化されている。
  - 割り当て時と実行時で「基礎需要」の定義が一致し、差分が caller-specific subtraction だけに限定されている。
  - 実装完了時に `cargo check` が成功する。

## 2. スコープ

### 対象（In Scope）

- `FloorTileBlueprint` / `WallTileBlueprint` / `ProvisionalWall` の基礎需要を返す pure function の追加。
- 地面上にある資材数の共通カウント helper の追加。
- `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs` の site/provisional 需要計算を共通関数呼び出しへ置換。
- `src/systems/soul_ai/execute/task_execution/haul/dropping.rs` の site/provisional 受入判定を共通関数呼び出しへ置換。
- `src/systems/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading.rs` の site/provisional 残受入数計算を共通関数呼び出しへ置換。
- `docs/logistics.md` の実装ルール更新。

### 非対象（Out of Scope）

- Blueprint / Stockpile / Mixer の需要計算ロジック再編。
- `IncomingDeliveries` と `nearby_ground_resources` の控除方式そのものの統一。
- trait ベースの抽象化や generic destination protocol の導入。
- 過剰搬入防止の仕様変更そのもの。ここでは重複解消と責務明文化を優先する。

## 3. 現状とギャップ

- 現状:
  - `src/systems/logistics/water.rs` には pure function を集約する先例がある。
  - `demand.rs` は `compute_remaining_with_incoming` / `compute_remaining_wall_with_incoming` を内部に持ち、タイル走査と `IncomingDeliveries + ReservationShadow` 控除を同一関数に抱えている。
  - `dropping.rs` は `floor_site_can_accept` / `wall_site_can_accept` / `provisional_wall_can_accept` と、`exclude_item` 付き `count_nearby_ground_resources` をローカル定義している。
  - `unloading.rs` は `floor_site_remaining` / `wall_site_remaining` / `provisional_wall_remaining` と、別シグネチャの `count_nearby_ground_resources` をローカル定義している。
- 問題:
  - タイル状態と定数に基づく基礎需要の式が 3 箇所に分散し、将来変更時に同期漏れを起こしやすい。
  - `dropping.rs` と `unloading.rs` の地面資材カウントはほぼ同一だが、`exclude_item` の有無だけが差分として埋め込まれている。
  - 呼び出し側固有の責務と、搬入先固有の責務が混ざっているため、新規 destination 追加時のコピペ先が不明瞭。
  - `Commands` の deferred 適用により、猫車荷下ろしでは「このループですでに置いた分」をローカル変数で補正している。この補正責務を helper 化の際に壊すと回帰する。
- 本計画で埋めるギャップ:
  - 「基礎需要を返す pure function」と「呼び出し側での差し引き」の境界を明文化する。
  - 地面資材カウント helper を 1 箇所に寄せ、`exclude_item` の差分だけを引数で扱う。
  - 実装順を分離し、`dropping` / `unloading` / `demand` を段階的に置換できる計画へ落とす。

## 4. 実装方針（高レベル）

- 方針:
  - `src/systems/logistics/` に建設系 destination ごとの需要モジュールを追加し、ECS world 依存を最小にした pure function を置く。
  - `count_nearby_ground_resources` は execution 専用 helper として同階層へ追加するが、site center / 半径 / `exclude_item` は呼び出し側が渡す。
  - 既存の割り当て時 API は公開関数名を維持しつつ、中で新 helper を呼び出す形へ移行する。policy 呼び出し元まで API を広げない。
  - 実行時は bool 判定と remaining 判定の両方を維持し、helper 側は「基礎需要」と「周辺資材数」を返すだけに留める。
- 設計上の前提:
  - Floor / Wall は `parent_site == anchor_entity` のタイル総和で需要を出す。
  - ProvisionalWall は「仮設壁で、まだ泥未搬入なら 1、そうでなければ 0」という現在契約を維持する。
  - `IncomingDeliveries` / `ReservationShadow` を引くのは assignment 側、地面資材を引くのは execution 側、という二層構造は維持する。
  - `unloading.rs` では deferred な drop が同フレーム query に見えないため、既存の `reserved_by_resource` によるローカル補正は維持する。
- Bevy 0.18 APIでの注意点:
  - 追加する helper は Bevy ECS の `Query` 型を直接受け取らず、既存 caller が持つ iterator / 値を受け取る形を優先する。これにより `TaskAssignmentQueries` と `TaskExecutionContext` の両方から再利用しやすくする。
  - `Entity` / `Vec2` / `Visibility` などの型は Bevy 0.18 の `prelude` を前提にする。

### 4.1 追加する共通モジュール案

- `src/systems/logistics/floor_construction.rs`
  - `floor_site_tile_demand(...) -> usize`
- `src/systems/logistics/wall_construction.rs`
  - `wall_site_tile_demand(...) -> usize`
- `src/systems/logistics/provisional_wall.rs`
  - `provisional_wall_mud_demand(...) -> usize`
- `src/systems/logistics/ground_resources.rs`
  - `count_nearby_ground_resources(...) -> usize`

### 4.2 API 契約

- 需要 helper は「現在の tile/building 状態だけから算出される基礎需要」を返す。
- 需要 helper は `IncomingDeliveries` / `ReservationShadow` / 地面資材 / request lease を差し引かない。
- `count_nearby_ground_resources` は以下だけを数える:
  - `Visibility != Hidden`
  - `LoadedIn` なし
  - `StoredIn` なし
  - 指定 resource type と一致
  - 指定半径内
- `exclude_item` は `Option<Entity>` とし、手運搬 dropping は `Some(item)`、猫車 unloading は `None` を渡す。

### 4.3 呼び出し側の責務マトリクス

| 呼び出し側 | 使う共通関数 | 呼び出し側が追加で行うこと |
| --- | --- | --- |
| `policy/haul/demand.rs` | `*_tile_demand`, `provisional_wall_mud_demand` | `IncomingDeliveries` と `ReservationShadow` を差し引く |
| `haul/dropping.rs` | `*_tile_demand`, `provisional_wall_mud_demand`, `count_nearby_ground_resources` | `exclude_item = Some(item)` で周辺地面資材を引き、`needed > nearby` で受入可否を決める |
| `haul_with_wheelbarrow/phases/unloading.rs` | `*_tile_demand`, `provisional_wall_mud_demand`, `count_nearby_ground_resources` | `exclude_item = None` で周辺地面資材を引き、さらに `reserved_by_resource` で同フレーム内の荷下ろし数を補正する |

### 4.4 実装順の原則

- 先に `logistics/` 側 helper を追加する。
- 次に `dropping.rs` を置換して `exclude_item` ありの経路を固める。
- 次に `unloading.rs` を置換し、`reserved_by_resource` 補正が維持されることを確認する。
- 最後に `demand.rs` を置換し、assignment 側の式を helper ベースへ寄せる。
- 文書更新は最後にまとめて行うが、関数契約が実装とズレるなら途中でも更新する。

## 5. マイルストーン

## M1: 共通 helper の抽出

- 変更内容:
  - `logistics/water.rs` と同じ粒度で、floor / wall / provisional 用の基礎需要 helper を追加する。
  - `ground_resources.rs` を追加し、visible / unloaded / unstored 条件を 1 箇所へ集約する。
  - `logistics/mod.rs` へ公開追加を行う。
- 変更ファイル:
  - `src/systems/logistics/floor_construction.rs`
  - `src/systems/logistics/wall_construction.rs`
  - `src/systems/logistics/provisional_wall.rs`
  - `src/systems/logistics/ground_resources.rs`
  - `src/systems/logistics/mod.rs`
- 完了条件:
  - [ ] floor / wall / provisional の基礎需要関数が追加されている
  - [ ] 地面資材カウント helper が `Option<Entity>` で除外対象を受け取れる
  - [ ] 各 helper の doc comment に「何を差し引かないか」が明記されている
- 検証:
  - `cargo check`

## M2: 手運搬 dropping 経路の置換

- 変更内容:
  - `dropping.rs` のローカル `count_nearby_ground_resources` / `floor_site_can_accept` / `wall_site_can_accept` / `provisional_wall_can_accept` を共通 helper 呼び出しへ置換する。
  - 受入半径と site center / wall position の取得は caller 側に残し、現在の判定半径を維持する。
  - cancel 分岐と reservation 解放の現行契約は変更しない。
- 変更ファイル:
  - `src/systems/soul_ai/execute/task_execution/haul/dropping.rs`
  - `src/systems/logistics/ground_resources.rs`
  - `src/systems/logistics/floor_construction.rs`
  - `src/systems/logistics/wall_construction.rs`
  - `src/systems/logistics/provisional_wall.rs`
- 完了条件:
  - [ ] `dropping.rs` に建設系 destination 専用の需要計算ローカル関数が残っていない
  - [ ] `exclude_item = Some(item)` の扱いが helper 経由になっている
  - [ ] floor / wall / provisional の cancel 判定結果が現行仕様と等価である
- 検証:
  - `cargo check`

## M3: 猫車 unloading 経路の置換

- 変更内容:
  - `unloading.rs` のローカル `count_nearby_ground_resources` / `floor_site_remaining` / `wall_site_remaining` / `provisional_wall_remaining` を共通 helper 呼び出しへ置換する。
  - `reserved_by_resource` による「このループで置いた分」の補正は残し、query にまだ反映されない分をローカルで抑える。
  - stockpile / blueprint / mixer 分岐は対象外とし、site/provisional 分岐だけを共通 helper 化する。
- 変更ファイル:
  - `src/systems/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading.rs`
  - `src/systems/logistics/ground_resources.rs`
  - `src/systems/logistics/floor_construction.rs`
  - `src/systems/logistics/wall_construction.rs`
  - `src/systems/logistics/provisional_wall.rs`
- 完了条件:
  - [ ] `unloading.rs` から site/provisional 用 remaining 関数が削除されている
  - [ ] `reserved_by_resource` 補正が helper 化の後も維持されている
  - [ ] 同一 destination への複数個荷下ろしで過剰 drop を出さない契約が残っている
- 検証:
  - `cargo check`

## M4: assignment 側の需要計算置換と文書同期

- 変更内容:
  - `demand.rs` の `compute_remaining_with_incoming` / `compute_remaining_wall_with_incoming` を基礎需要 helper 呼び出しへ置換する。
  - `compute_remaining_floor_*` / `compute_remaining_wall_*` / `compute_remaining_provisional_wall_mud` の公開 API 名は維持し、呼び出し元影響を局所化する。
  - `docs/logistics.md` に「建設系 destination の基礎需要は `logistics/` helper に置き、assignment / execution は caller-specific subtraction のみ持つ」旨を追記する。
  - 必要なら proposal 側の `関連計画` や備考を同期する。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs`
  - `docs/logistics.md`
  - `docs/proposals/destination-validation-unification-proposal-2026-03-07.md`
- 完了条件:
  - [ ] `demand.rs` から floor / wall ごとの内部重複ループが削除されている
  - [ ] assignment 側の控除責務が `IncomingDeliveries + ReservationShadow` のみとして読み取れる
  - [ ] `docs/logistics.md` の実装ルールが新しい置き場を説明している
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 基礎需要 helper に控除ロジックまで入れてしまう | assignment と execution で必要な差分が隠れ、再び分岐が増える | helper 契約を「控除なし」に固定し、doc comment と計画書に明記する |
| `exclude_item` の扱いを誤る | 手運搬 dropping で自分が持っている item を誤カウントし、搬入が止まる | `Option<Entity>` に統一し、dropping のみ `Some(item)` を渡す方針にする |
| `unloading.rs` の deferred 反映補正を落とす | 同一フレーム内の複数荷下ろしで再び過剰搬入する | `reserved_by_resource` を削らず、helper は stateless に保つ |
| helper のシグネチャが `TaskAssignmentQueries` / `TaskExecutionContext` に寄りすぎる | 呼び出し側ごとに別 helper が必要になり、統合効果が落ちる | iterator / 値ベース引数を優先し、ECS query 依存を caller 側へ残す |
| 進行中の overdelivery 修正と衝突する | マージ時に plan がすぐ古くなる | `dropping.rs` / `unloading.rs` / `demand.rs` の最新状態を実装着手前に再確認し、M2-M4 を独立 revert 可能な粒度に保つ |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - FloorConstruction へ Bone / StasisMud を複数 Soul で同時搬入し、必要量到達で停止すること。
  - WallConstruction へ Wood / StasisMud を複数 Soul で同時搬入し、site 周辺に余剰 drop が増えないこと。
  - ProvisionalWall へ StasisMud を搬入し、1 個で停止すること。
  - 猫車で floor / wall / provisional へ複数個荷下ろししても、同フレーム内で余剰 drop が出ないこと。
  - 既存 Blueprint / Stockpile / Mixer の挙動が変わらないこと。
- パフォーマンス確認（必要時）:
  - helper 化は pure function 抽出が中心のため必須ではないが、過剰搬入修正と同時着手する場合は `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario` を任意で再確認する。

## 8. ロールバック方針

- どの単位で戻せるか:
  - `M2 dropping`、`M3 unloading`、`M4 demand/docs` は独立して revert 可能にする。
  - `M1` の helper 追加は単独で残っても動作影響が小さいため、まず caller 置換側だけ戻して切り分けられる。
- 戻す時の手順:
  - まず `unloading.rs` 側だけ戻し、同フレーム多重荷下ろしの回帰有無を確認する。
  - 次に `dropping.rs` 側を戻し、手運搬 cancel 判定の差分を確認する。
  - 最後に `demand.rs` 側を戻し、assignment の stale task 差分を確認する。
  - helper 群そのものに問題があると判明した場合のみ `logistics/` の追加モジュールを削除する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1` / `M2` / `M3` / `M4`

### 次のAIが最初にやること

1. `src/systems/logistics/water.rs` を参照し、建設系 destination helper 4 ファイルの最小 API を確定する。
2. `dropping.rs` を先に置換し、`exclude_item` の扱いを固めて `cargo check` を実行する。
3. `unloading.rs` を置換し、`reserved_by_resource` 補正を維持したまま `cargo check` を実行する。
4. 最後に `demand.rs` と `docs/logistics.md` を更新する。

### ブロッカー/注意点

- `docs/proposals/destination-validation-unification-proposal-2026-03-07.md` は proposal であり、ここに書かれたシグネチャ案は最終決定ではない。実装前に最新コードへ合わせて微修正してよい。
- `dropping.rs` / `unloading.rs` は直近の overdelivery 修正と同じ領域なので、着手前に未コミット差分と衝突していないか確認する。
- `unloading.rs` の site/provisional 判定は `reserved_by_resource` を通じて loop 内で補正している。このローカル state を helper に押し込めない。
- `ground_resources` helper は execution 専用だが、`TaskExecutionContext` 全体を受け取る設計にはしない。将来別 caller からも再利用できるよう、iterator / 値ベースを優先する。

### 参照必須ファイル

- `docs/proposals/destination-validation-unification-proposal-2026-03-07.md`
- `docs/plans/transport-overdelivery-fix-plan-2026-03-07.md`
- `docs/logistics.md`
- `src/systems/logistics/water.rs`
- `src/systems/logistics/mod.rs`
- `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs`
- `src/systems/soul_ai/execute/task_execution/haul/dropping.rs`
- `src/systems/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading.rs`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-07` / `not run`
- 未解決エラー: 未確認（計画書作成のみ）

### Definition of Done

- [ ] `src/systems/logistics/` に建設系 destination helper が追加されている
- [ ] `demand.rs` / `dropping.rs` / `unloading.rs` の重複ローカル関数が削除されている
- [ ] `docs/logistics.md` が新しい責務境界を説明している
- [ ] `cargo check` が成功する

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-07` | `Codex` | 初版作成 |
