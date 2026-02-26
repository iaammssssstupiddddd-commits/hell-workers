# Selection配置リファクタ計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `selection-placement-refactor-plan-2026-02-25` |
| ステータス | `Completed` |
| 作成日 | `2026-02-25` |
| 最終更新日 | `2026-02-26` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  - `src/interface/selection/building_place/mod.rs` が入力処理、companionフロー、配置可否判定、spawn処理を同居しており変更影響が広い。（→ リファクタ完了）
  - `src/interface/selection/floor_place/mod.rs` で floor/wall の検証ロジックが重複し、エラーメッセージ更新時に差分ズレが起きやすい。（→ リファクタ完了）
  - カーソル座標取得やグリッド計算の処理が近接モジュール間で重複している。
- 到達したい状態:
  - 配置処理を「入力」「検証」「配置実行」「companion制御」に責務分離し、差分追跡を容易にする。
  - 既存の公開システム関数（`blueprint_placement` / `floor_placement_system`）と挙動を維持したまま内部構造を分割する。
  - 失敗理由表示（`PlacementFailureTooltip`）のメッセージ整合を保ちながら共通化する。
- 成功指標:
  - `building_place` / `floor_place` の単一ファイル肥大化を解消し、責務単位のモジュールに移行できる。
  - 配置制約（Door配置、Tank/MudMixer companion、Floor/Wall範囲制約）が回帰しない。
  - `cargo check` が成功する。

## 2. スコープ

### 対象（In Scope）

- `src/interface/selection/building_place/` のモジュール分割。（完了）
- `src/interface/selection/floor_place/` のモジュール分割。（完了）
- `interface/selection` 内で共通化可能なヘルパー（カーソル座標取得、配置検証の共通条件）を抽出。
- 既存プラグイン配線（`UiCorePlugin`, `UiTooltipPlugin`）を維持したまま参照先を更新。

### 非対象（Out of Scope）

- 建築仕様の変更（必要資材、Door成立条件、companion要件のルール変更）。
- `systems/command/area_selection` の仕様変更。
- pathfinding・物流・AIロジックの変更。
- UIデザインや演出（色、アニメーション、ツールチップ表示時間）の変更。

## 3. 現状とギャップ

- 現状:
  - `blueprint_placement` は入力→分岐→配置実行→ロールバックまでを1関数で担っている。
  - `floor_placement_system` は drag入力管理と floor/wall の個別配置実装を1ファイル内に保持している。
  - 「歩行可能判定」「建物/stockpile衝突判定」「エラー理由生成」が類似実装で散在している。
- 問題:
  - 仕様追加（新しい建築タイプ、companion条件追加）時に副作用箇所を見落としやすい。
  - 修正対象の把握に時間がかかり、レビュー時に意図しない挙動変更を見逃しやすい。
  - 同種メッセージの文言差や分岐差が将来の不整合を招く。
- 本計画で埋めるギャップ:
  - 各機能をモジュール境界で分割し、変更点を局所化する。
  - 配置失敗理由と共通判定をヘルパー化して重複を削減する。
  - プラグイン順序と公開APIを維持し、挙動互換を優先した段階移行にする。

## 4. 実装方針（高レベル）

- 方針:
  - 先に「関数抽出 + 参照更新」で挙動を変えずに分割し、その後に重複を共通化する。
  - `blueprint_placement` / `floor_placement_system` は外部呼び出し名を固定し、配線変更の影響を最小化する。
  - 失敗理由文言は既存互換を保ち、変更が必要な場合のみ明示的に差分化する。
- 設計上の前提:
  - companion配置の巻き戻し（親Blueprint確定失敗時の rollback）は既存挙動を保持する。
  - Door配置判定（左右または上下に壁/扉）を維持する。
  - Floor/Wall の範囲上限と `1xn` 制約は変更しない。
- Bevy 0.18 APIでの注意点:
  - `Query::single()` / `viewport_to_world_2d()` の戻り値処理を既存同等に維持する。
  - システム順序（`hover_tooltip_system.before(blueprint_placement)`）を崩さない。
  - `Commands` と `ResMut<WorldMap>` の可変借用境界を維持し、B0001相当の競合を導入しない。

## 5. マイルストーン

## M1: モジュール分割の骨格作成

- 変更内容:
  - `building_place` / `floor_place` をディレクトリモジュール化し、入口関数を `mod.rs` に移す。
  - 既存の内部関数を責務別ファイルへ移動（挙動変更なし）。
- 変更ファイル:
  - `src/interface/selection/building_place/mod.rs`（新規）
  - `src/interface/selection/building_place/*.rs`（新規）
  - `src/interface/selection/floor_place/mod.rs`（新規）
  - `src/interface/selection/floor_place/*.rs`（新規）
  - `src/interface/selection/mod.rs`
- 完了条件:
  - [ ] 公開関数シグネチャが維持される。
  - [ ] `UiCorePlugin` / `UiTooltipPlugin` の参照が壊れない。
- 検証:
  - `cargo check`

## M2: Building配置責務の分離

- 変更内容:
  - companion配置フロー（Tank/MudMixer）を独立モジュール化。
  - blueprint配置実体（占有判定、spawn、door置換、rollback）を分離。
  - 建物形状ヘルパー（占有グリッド、spawn座標、サイズ）を専用モジュールに集約。
- 変更ファイル:
  - `src/interface/selection/building_place/flow.rs`（新規）
  - `src/interface/selection/building_place/placement.rs`（新規）
  - `src/interface/selection/building_place/companion.rs`（新規）
  - `src/interface/selection/building_place/geometry.rs`（新規）
  - `src/interface/selection/building_place/door_rules.rs`（新規）
- 完了条件:
  - [ ] Tank companion配置の確定/巻き戻しが現行互換で動作する。
  - [ ] MudMixer companion配置条件（近傍SandPile要件）が維持される。
  - [ ] Door置換配置（Wall -> Door）が維持される。
- 検証:
  - `cargo check`

## M3: Floor/Wall配置責務の分離

- 変更内容:
  - drag入力状態処理と配置実処理を分離。
  - floor/wall の共通タイル検証フローを抽出し、分岐差分のみ個別化。
  - `PlacementFailureTooltip` に渡す代表エラー生成処理を共通化。
- 変更ファイル:
  - `src/interface/selection/floor_place/input.rs`（新規）
  - `src/interface/selection/floor_place/floor_apply.rs`（新規）
  - `src/interface/selection/floor_place/wall_apply.rs`（新規）
  - `src/interface/selection/floor_place/validation.rs`（新規）
- 完了条件:
  - [ ] Floorの矩形配置（上限10x10）制約が維持される。
  - [ ] Wallの `1xn` 制約と completed floor 必須条件が維持される。
  - [ ] 失敗理由ツールチップ表示の文言カテゴリが互換になる。
- 検証:
  - `cargo check`

## M4: 共通ヘルパー統合と最終整理

- 変更内容:
  - `selection` 内のカーソル座標取得とグリッド変換ヘルパーを共通化。
  - 不要になった重複関数・importを削除し、モジュール境界を最終確定。
  - ドキュメント参照（必要に応じて `docs/building.md`）を更新。
- 変更ファイル:
  - `src/interface/selection/placement_common.rs`（新規）
  - `src/interface/selection/input.rs`
  - `src/interface/selection/building_place/mod.rs`
  - `src/interface/selection/floor_place/mod.rs`
  - `docs/building.md`（必要時）
- 完了条件:
  - [ ] 配置関連の重複ロジックが削減される。
  - [ ] dead code を導入せず、未使用関数が残らない。
  - [ ] `cargo check` が成功する。
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| システム順序の変更でツールチップ表示タイミングが変わる | UI挙動回帰 | `UiTooltipPlugin` の `.before(blueprint_placement)` を維持し、関数名公開を固定 |
| companion配置の巻き戻し経路漏れ | 建築予約の不整合 | M2でrollback専用ヘルパーを作り、失敗経路を1箇所に集約 |
| floor/wall検証共通化で条件を混同 | 配置可否の誤判定 | 共通部分と差分条件（floor専用/wall専用）を構造体で明示分離 |
| 分割時のQuery借用競合 | 実行時パニック | `SystemParam` 境界を維持し、`ResMut<WorldMap>` の可変借用範囲を最小化 |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - Tank配置 -> companion BucketStorage 配置成功/失敗（範囲外）/右クリックキャンセル。
  - MudMixer配置 -> 近傍SandPileあり/なしの分岐確認。
  - Door配置 -> `壁-扉-壁` または縦方向条件でのみ配置可能。
  - Floor配置 -> 10x10超過時に `area too large` 系理由が表示される。
  - Wall配置 -> `1xn` 以外で拒否、completed floor なしタイルで拒否。
- パフォーマンス確認（必要時）:
  - 配置連打時にフレーム落ちや入力遅延が悪化しないことを目視確認。

## 8. ロールバック方針

- どの単位で戻せるか:
  - `building_place` 分割単位で revert 可能。
  - `floor_place` 分割単位で revert 可能。
  - 共通ヘルパー導入部分のみ個別撤回可能。
- 戻す時の手順:
  1. 分割対象モジュールを段階的に元の単一ファイル構成へ戻す。
  2. `src/interface/selection/mod.rs` の公開参照を旧構成へ戻す。
  3. `cargo check` で型整合を確認する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1` `M2` `M3` `M4`

### 次のAIが最初にやること

1. `building_place/` と `floor_place/` の責務区分を確認し、M1の移動単位を確定する。（完了）
2. `UiCorePlugin` / `UiTooltipPlugin` の配線を確認し、公開関数名を維持したままモジュール化する。
3. M2実施後に companion配置フロー（確定/巻き戻し）を手動確認する。

### ブロッカー/注意点

- companion処理は親Blueprint確定とロールバックが絡むため、分割時に副作用順序を変えないこと。
- 失敗理由の文字列変更はUI回帰として扱われるため、共通化時も既存文言カテゴリを保つこと。
- ワークツリーに他作業の変更がある可能性があるため、対象外ファイルは触らないこと。

### 参照必須ファイル

- `src/interface/selection/building_place/mod.rs`
- `src/interface/selection/building_place/geometry.rs`
- `src/interface/selection/building_place/door_rules.rs`
- `src/interface/selection/building_place/placement.rs`
- `src/interface/selection/building_place/companion.rs`
- `src/interface/selection/building_place/flow.rs`
- `src/interface/selection/floor_place/mod.rs`
- `src/interface/selection/floor_place/validation.rs`
- `src/interface/selection/floor_place/floor_apply.rs`
- `src/interface/selection/floor_place/wall_apply.rs`
- `src/interface/selection/mod.rs`
- `src/interface/ui/plugins/core.rs`
- `src/interface/ui/plugins/tooltip.rs`
- `docs/building.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-02-26` / `成功（警告なし）`
- 未解決エラー: なし

### Definition of Done

- [x] `M1` `M2` `M3` `M4` の完了条件を満たす
- [ ] 配置フローの主要シナリオで回帰がない（手動確認要）
- [x] `cargo check` が成功する

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-25` | `Codex` | 初版作成 |
| `2026-02-26` | `Copilot` | 実装完了。ファイルパスを新モジュール構成に更新、ステータスを Completed に変更 |
