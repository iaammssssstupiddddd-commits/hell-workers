# Plant Trees ビジュアル強化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `plant-trees-visuals-plan-2026-02-22` |
| ステータス | `Done` |
| 作成日 | `2026-02-22` |
| 最終更新日 | `2026-02-23` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/plant_trees_visuals.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  - `Plant Trees` が即時スポーンのみで、Dream消費に対する視覚フィードバックが弱い。
  - 提案書の3フェーズ演出（予兆/成長/波及）が未実装。
- 到達したい状態:
  - 各対象タイルで「魔法陣 -> 急成長 -> 周囲への生命力パーティクル」の順に短時間で再生される。
  - 既存の地形タイル（Dirt/Grass/Sand/River）は変更しない。
  - 既存のコスト/上限/候補抽選ロジックを維持したまま導入する。
- 成功指標:
  - 1回の植林実行で選ばれた全タイルに3フェーズ演出が再生される。
  - 生成本数・Dream消費量・上限制約が現行と一致する。
  - `cargo check` が成功し、手動シナリオで視覚回帰がない。

## 2. スコープ

### 対象（In Scope）

- `dream_tree_planting_system` の「即時見た目反映」部分をアニメーション駆動へ変更。
- Plant Trees専用のビジュアルコンポーネント/更新システム追加（`src/systems/visual/`配下）。
- 魔法陣・生命力パーティクル用テクスチャの追加とロード配線。
- Dream植林由来の木に `ObstaclePosition` を付与し、障害物同期の一貫性を確保。
- 演出用定数（時間、色、スケール、粒子数、半径）の追加。

### 非対象（Out of Scope）

- Dream蓄積レートや `DREAM_TREE_*` バランスの再設計。
- 伐採タスク仕様の全面改修（今回必要な同期差分のみ対象）。
- サウンド演出追加。
- UIモード遷移や入力仕様の変更（DreamPlantingドラッグ操作自体は現状維持）。

## 3. 現状とギャップ

- 現状:
  - `src/systems/dream_tree_planting.rs` で対象選定後に `Tree` を即時スポーンしている。
  - 同ファイルで `world_map.add_obstacle` を即時実行し、`-Dream` ポップアップのみ表示。
  - `src/plugins/visual.rs` に Plant Trees専用演出システムは未登録。
- 問題:
  - Dream消費体験が「数値減少 + 突然木が出る」だけになっている。
  - 提案されたフェーズ型演出がなく、プレイヤーの予兆認識が弱い。
  - Dream植林で追加した木は `ObstaclePosition` が未付与で、障害物同期の整合性リスクがある。
- 本計画で埋めるギャップ:
  - Tree実体を「アニメーション付きで出現」させる導線を追加。
  - 視覚専用一時エンティティで魔法陣と生命力パーティクルを管理。
  - 障害物コンポーネントの付与を統一し、後続の除去処理と整合させる。

## 4. 実装方針（高レベル）

- 方針:
  - ロジック側は「候補決定・コスト計算・スポーン開始トリガー」に集中する。
  - 表示進行は Visual側の状態コンポーネントでフェーズ管理する。
  - Treeはロジック側で生成しつつ、初期は不可視/縮小状態にして Visual側で成長完了まで制御する。
- 設計上の前提:
  - 現行の `DREAM_TREE_SPAWN_RATE_PER_TILE`, `DREAM_TREE_COST_PER_TREE`, `DREAM_TREE_MAX_PER_CAST`, `DREAM_TREE_GLOBAL_CAP` は維持。
  - `world_map` の地形タイル (`tiles`) は書き換えない。
  - `world_map.add_obstacle` は現行と同様に植林成立時点で実行し、演出中の通行整合性を保つ。
- Bevy 0.18 APIでの注意点:
  - `Added<T>` と `Without<T>` を使って初回セットアップを1回だけ実行する。
  - フレーム更新は `Time::delta_secs()` ベースで進行し、状態遷移は `Timer` または残り時間で管理する。
  - `Sprite` の `color` / `Transform::scale` で成長・発光を制御し、タイル自体は変更しない。
  - ブレンド方式はまず既存のアルファブレンドで導入し、必要なら `Material2d` 化を別マイルストーンに分離する。

## 5. マイルストーン

## M1: Plant Trees演出のデータモデルと定数を追加

- 変更内容:
  - Plant Trees演出コンポーネント（フェーズ状態、タイマー、粒子）を定義。
  - 演出時間/色/スケール/粒子数の定数を追加。
  - 必要ならZレイヤ定数を追加して描画順を固定。
- 変更ファイル:
  - `src/constants/dream.rs`
  - `src/constants/render.rs`
  - `src/systems/visual/plant_trees/mod.rs`（新規）
  - `src/systems/visual/plant_trees/components.rs`（新規）
  - `src/systems/visual/mod.rs`
- 完了条件:
  - [ ] コンポーネント/定数の型定義が完了している。
  - [ ] 既存機能に挙動変更がない状態でコンパイルできる。
- 検証:
  - `cargo check`

## M2: 演出アセットの追加とロード配線

- 変更内容:
  - 魔法陣テクスチャと生命力粒子テクスチャを追加。
  - `GameAssets` にハンドルを追加し、`asset_catalog` でロード。
  - 画像生成時はマゼンタ背景 -> `scripts/convert_to_png.py` で透過化。
- 変更ファイル:
  - `assets/textures/ui/plant_tree_magic_circle.png`（新規）
  - `assets/textures/ui/plant_tree_life_spark.png`（新規）
  - `src/assets.rs`
  - `src/plugins/startup/asset_catalog.rs`
- 完了条件:
  - [ ] 追加アセットを参照するフィールドが `GameAssets` で解決できる。
  - [ ] アセットロード時にパス不整合がない。
- 検証:
  - `cargo check`

## M3: ロジック側を「アニメーション開始」方式へ変更

- 変更内容:
  - `dream_tree_planting_system` の即時スポーン処理を、演出コンポーネント付きスポーンへ置き換える。
  - 生成Treeに `ObstaclePosition(gx, gy)` を付与して障害物同期を統一。
  - コスト消費・上限判定・候補抽選は現行ロジックを維持。
  - `-Dream` ポップアップは既存処理を維持。
- 変更ファイル:
  - `src/systems/dream_tree_planting.rs`
  - `src/systems/jobs/mod.rs`（必要時。追加型が必要な場合のみ）
  - `src/constants/dream.rs`（必要時。M1定数の微調整）
- 完了条件:
  - [ ] 生成本数とDream消費が現行一致（同入力で同上限適用）。
  - [ ] 生成Treeに `ObstaclePosition` が付与される。
  - [ ] 木が即時フル表示されず、演出状態で開始される。
- 検証:
  - `cargo check`

## M4: フェーズ1〜3のVisualシステム実装と登録

- 変更内容:
  - フェーズ1（魔法陣）: フェードイン/拡大/フェードアウト。
  - フェーズ2（急成長）: Treeのスケール 0->1、青白い発光色 -> 白への遷移。
  - フェーズ3（波及）: 根元から粒子を円状拡散し短時間で消滅。
  - 完了時に演出コンポーネントを除去し、通常Tree表示へ遷移。
  - VisualPluginへシステムを `GameSystemSet::Visual` で登録。
- 変更ファイル:
  - `src/systems/visual/plant_trees/systems.rs`（新規）
  - `src/systems/visual/plant_trees/mod.rs`
  - `src/plugins/visual.rs`
- 完了条件:
  - [ ] 1本ごとに 予兆 -> 成長 -> 波及 の順で再生される。
  - [ ] 地形タイルの見た目/データを書き換えない。
  - [ ] 演出完了後に不要エンティティが残留しない。
- 検証:
  - `cargo check`

## M5: パラメータ調整と回帰確認

- 変更内容:
  - フェード速度、粒子数、拡散半径、色の最終調整。
  - 多数本同時生成時の負荷を確認し、必要なら粒子上限を導入。
  - 仕様との差分を最終確認し、必要なドキュメント参照を追記。
- 変更ファイル:
  - `src/constants/dream.rs`
  - `docs/plans/plant-trees-visuals-plan-2026-02-22.md`
- 完了条件:
  - [ ] 提案書の3フェーズ要件を満たす。
  - [ ] 大面積キャスト時に演出が破綻しない。
  - [ ] `cargo check` が通る。
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 同時20本生成で粒子が多くなりフレーム低下 | 演出時のみ体感重くなる | 粒子寿命短縮・1本あたり粒子数上限・必要なら距離カリング |
| 演出フェーズ遷移の不整合（完了しない） | 木が不可視のまま残る | フェーズをenum化し、最終フェーズで強制的に通常表示へフォールバック |
| 障害物同期漏れ | 通行判定が壊れる | Dream植林Treeに `ObstaclePosition` を付与し既存同期経路を使用 |
| 画像アセット欠落/パスミス | 実行時に表示されない | `asset_catalog` と `GameAssets` の同時更新、`cargo check` + 実行確認 |
| 描画順競合（木やUIに埋もれる） | 演出が見えない | 専用Z定数を導入し `Z_ITEM_OBSTACLE` 周辺で順序固定 |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - 4タイル未満の範囲でDream植林し、従来通り「不成立で消費なし」を確認。
  - 十分なDreamで複数タイル植林し、各タイルで3フェーズ演出が再生されることを確認。
  - 大面積ドラッグ時に `DREAM_TREE_MAX_PER_CAST` とDream残高で本数が制限されることを確認。
  - 成長完了後のTreeを伐採し、通行可能状態へ戻る（障害物同期）ことを確認。
  - 地形（Dirt/Sand/Grass）テクスチャが一切置換されないことを確認。
- パフォーマンス確認（必要時）:
  - 連続キャスト時の体感フレーム低下を比較し、必要なら粒子数を再調整。

## 8. ロールバック方針

- どの単位で戻せるか:
  - M3以前: `dream_tree_planting_system` を即時スポーン実装へ戻すことで機能復旧可能。
  - M4以降: `src/systems/visual/plant_trees/` の登録を外せば演出のみ無効化可能。
- 戻す時の手順:
  1. `src/plugins/visual.rs` から Plant Trees演出システム登録を削除。
  2. `src/systems/dream_tree_planting.rs` を即時スポーン分岐へ戻す。
  3. 未使用定数/アセット参照を削除し `cargo check` で確認。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1`〜`M5`
- 未着手/進行中: なし

### 次のAIが最初にやること

1. `M1` として `plant_trees` 演出コンポーネントと定数を先に定義する。
2. `M3` で `dream_tree_planting_system` を演出トリガー型へ変換する（コスト計算ロジックは維持）。
3. `M4` のVisualシステムを追加し、`plugins/visual.rs` へ順序付きで登録する。

### ブロッカー/注意点

- Bevy 0.18 の `Sprite` で加算合成を厳密に行う場合は追加実装（Material）が必要。
- 演出導入時も `world_map` 地形タイルを書き換えないこと。
- `ObstaclePosition` を付与しないと障害物同期が崩れるため省略しないこと。

### 参照必須ファイル

- `docs/proposals/plant_trees_visuals.md`
- `src/systems/dream_tree_planting.rs`
- `src/plugins/visual.rs`
- `src/systems/visual/dream/particle.rs`
- `src/systems/logistics/initial_spawn.rs`

### 最終確認ログ

- 最終 `cargo check`: `成功`（2026-02-22）
- 未解決エラー: `N/A`

### Definition of Done

- [x] 目的に対応するマイルストーンが全て完了
- [x] 影響ドキュメントが更新済み
- [x] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-22` | `Codex` | 初版作成 |
| `2026-02-23` | `Copilot` | 実装完了に合わせてステータス・DoD・最終確認ログを更新 |
