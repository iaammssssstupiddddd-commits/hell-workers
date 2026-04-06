# 地形ゾーン境界自然化計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `terrain-zone-boundary-naturalization-2026-04-06` |
| ステータス | `M2 Complete` |
| 作成日 | `2026-04-06` |
| 最終更新日 | `2026-04-06` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  草エリアや草ゾーン寄りの領域がマンハッタン距離ベースのひし形になりやすく、境界が階段状に見えて不自然。
- 到達したい状態:
  草エリアの大域形状が「ひし形」から外れ、境界の段差が目立ちにくい自然な輪郭になる。
- 成功指標:
  代表 seed で、草ゾーン境界が明確な対角線階段ではなく、丸みまたは緩いうねりを持つ。
  `terrain_tiles` と `WorldMasks` の契約を壊さず、`cargo check --workspace` を維持する。

## 2. スコープ

### 対象（In Scope）

- `hw_world` の terrain zone 生成ロジックの見直し
- ゾーン距離場、離隔、パッチ成長アルゴリズムの見直し
- zone 形状へ影響する隣接マスク境界の確認と、必要時の限定修正
- `bevy_app` の境界メッシュ生成ロジックの補助改善
- 実装方針に応じた `docs/map_generation.md` の更新

### 非対象（Out of Scope）

- `WorldMap` の論理解像度変更
- パス探索・当たり判定・AI 到達判定のルール変更
- 川・砂浜・岩場アルゴリズム全体の再設計
- 単なる定数調整だけで問題を先送りする対応

## 3. 現状とギャップ

### 問題の根本原因（2 層構造）

**レイヤー 1: seed 選択帯がひし形**
`pick_zone_seeds()` は `compute_anchor_distance_field()` が返すマンハッタン距離を閾値に使う。
等距離帯 = マンハッタン等値線 = ひし形になるため、dirt/grass の起点がひし形帯に集中する。

**レイヤー 2: パッチ成長がひし形**
`flood_fill_zone_patches()` と `expand_mask()` が 4 近傍 BFS なので、
各起点から均等に広がると内側も外側もマンハッタン円（ひし形）になる。

### 対象関数と現状（全て `terrain_zones.rs`）

| 関数 | 現行アルゴリズム | 問題 |
|---|---|---|
| `distance_field_from_mask` (l.164) | `VecDeque`, `DIRS: 4`, コスト=1 | アンカー距離帯がひし形 |
| `compute_anchor_distance_field` (l.200) | 上の薄いラッパー | 同上 |
| `compute_zone_distance_field` (l.208) | 上の薄いラッパー | ゾーン距離帯もひし形 → C グラデーション境界もひし形 |
| `expand_mask` (l.214) | `VecDeque`, `DIRS: 4` | 離隔バッファがひし形 |
| `flood_fill_zone_patches` (l.292) | `VecDeque`, `DIRS: 4` | パッチがひし形成長 |

### 対象外関数

| 関数 / 定数 | 理由 |
|---|---|
| `compute_protection_band` (`world_masks.rs` l.253) | 主因ではないため初手の変更対象からは外す。ただし `river_protection_band` は `allowed` を削るため、M2 後もアンカー周辺に diamond artifact が残る場合は再評価対象にする |
| `generate_inland_sand_mask` (l.330) | grass zone 内の微小パッチ、制約で自然に変形される |
| `boundary.rs` | 生成側が改善された後の補助のみ（M3） |

### 現状とギャップ要約

`boundary.rs` だけでは「ひし形の大域形状」は解消できない。
描画だけ強く曲げると論理セル境界と視覚境界が乖離する。
→ **先に `terrain_zones.rs` の距離指標を自然化し、描画は補助に留める。**

補足として、`river_protection_band` などの隣接マスク境界も zone の許可領域を切るため、
主因を潰した後に局所的な diamond artifact が残る可能性はある。
その場合のみ `world_masks.rs` の保護帯形状を追加検討する。

## 4. 実装方針（高レベル）

- 方針:
  第1優先は `hw_world` 側のゾーン形状生成を自然化すること。
  第2優先で `bevy_app` 側の境界描画を補助改善し、残るセル段差を視覚的に緩和する。
  「論理形状を改善してから、見た目を磨く」順で進める。
- 設計上の前提:
  `terrain_tiles` が地形の真実であり、描画専用境界メッシュは pure visual のまま維持する。
  `WorldMasks` を使う資源配置・validate・debug 契約は壊さない。
  `fix_zone_mask_crosses()` / `enforce_no_visual_cross_2x2()` は残しつつ、入力形状そのものを改善する。
- Bevy 0.18 APIでの注意点:
  主変更は `hw_world` の pure logic が中心で、Bevy API 依存は薄い。
  `boundary.rs` 側を変更する場合は、既存の PostStartup 登録順と `BoundarySurfaceMaterial` 契約を維持する。

## 5. 候補アプローチ比較

### A. 距離場を Chamfer 距離（8 近傍ダイクストラ）に切り替える【推奨】

- 内容:
  `distance_field_from_mask`（`expand_mask` も内部で同パターン）を、
  `VecDeque` による等コスト BFS から `BinaryHeap<Reverse<(u32,i32,i32)>>` によるダイクストラに置き換える。
  移動コスト: 直交=3、斜め=4（Chamfer 3-4 距離）。
  `flood_fill_zone_patches` も 4 近傍から **8 近傍**（コスト付き優先度付きキュー）に変更し、
  パッチが diagonal にも自然に広がるようにする。
- 利点:
  ひし形の 2 層 (seed 選択帯 / パッチ成長) を同時に潰せる。
  整数演算のみで実装でき、浮動小数点 Euclidean より計算・管理が容易。
  allowed_mask の形状が複雑なほど自然なアメーバ形状を誘発しやすい。
- 欠点:
  距離スケールが約 3 倍（直交コスト=3）に変わるため、
  `ZONE_*_DIST_*` / `ZONE_MIN_SEPARATION` / `ZONE_GRADIENT_WIDTH` 定数の再調整が必要。
  斜め方向の展開が増えることで 2×2 Visual Cross 発生数が増加しうる
  → `fix_zone_mask_crosses` + テストで早期検知する。

### B. 連続スカラー場ベースへ移行する

- 内容:
  `grass_score` / `dirt_score` のような連続場を作り、Euclidean 距離、低周波ノイズ、
  複数 seed の影響を合成してから閾値でマスク化する。
- 利点:
  最も自然な大域形状を作りやすい。
- 欠点:
  現状ロジックとの差分が大きく、実装と検証コストが高い。本計画では非推奨。

### C. 描画専用の境界場を追加する

- 内容:
  `WorldMasks` / `terrain_tiles` はそのままに、描画側だけで境界リボンを滑らかにする。
- 利点:
  ロジックへの影響が最小。
- 欠点:
  ひし形の大域形状は残る。根治ではなく補助策に留まる。

### 推奨方針

- **本命**: A を先に実施する（M1→M2）。
- **拡張**: A の後でも段差が目立つ場合のみ C を追加する（M3）。
- **非推奨**: C だけ先に強化して生成ロジックを放置する構成。

## 6. マイルストーン

## M1: 定数スケール整理と実装設計の確定

- 変更内容:
  Chamfer 3-4 距離に切り替えた場合に距離値が約 3 倍になることを踏まえ、
 影響を受ける定数すべてについて「現行値 → 初期候補値」を整理する。
  定数の参照箇所（`wfc_adapter.rs` と `pipeline.rs` も含む）を列挙し、
 変更が定数変更だけで吸収できるか・ロジック変更も必要かを確認する。
 ここでの数値は固定値ではなく、M2 後の seed 群比較で再調整する前提とする。

- 定数変更候補（距離スケール変換の初期値）:

  | 定数 | 現行値 | 新値候補 | 備考 |
  |---|---|---|---|
  | `ZONE_DIRT_DIST_MIN` | 5 | 15 | 直交 3 コスト換算の初期候補。実測で再調整する |
  | `ZONE_DIRT_DIST_MAX` | 16 | 48 | 同上 |
  | `ZONE_GRASS_DIST_MIN` | 18 | 54 | 同上 |
  | `ZONE_MIN_SEPARATION` | 3 | 9 | 直交換算の初期候補。斜め距離は一致しないため固定値扱いしない |
  | `ZONE_GRADIENT_WIDTH` | 3 | 9 | 初期候補。見た目と tile 比率を見て再調整する |

  ※ `ZONE_DIRT_REGION_AREA_MAX / ZONE_GRASS_REGION_AREA_MAX` はセル面積なので変更不要。
  ※ `×3` はあくまで直交方向の初期換算であり、斜め方向の等価性までは保証しない。

- `ZONE_GRADIENT_WIDTH` 参照箇所（定数変更で自動更新される）:
  - `wfc_adapter.rs` l.607–608（`dirt_dist <= ZONE_GRADIENT_WIDTH` 比較）
  - `pipeline.rs` l.293（`dd <= ZONE_GRADIENT_WIDTH` 比較）

- 変更ファイル:
  - `crates/hw_world/src/terrain_zones.rs`（定数候補整理）
  - `crates/hw_world/src/mapgen/pipeline.rs`（診断しきい値の参照箇所確認）
  - `docs/map_generation.md`

- 参照ファイル（変更対象外・設計時に確認すること）:
  - `crates/hw_world/src/mapgen/wfc_adapter.rs`（`fix_zone_mask_crosses` / `enforce_no_visual_cross_2x2` の実装先, `ZONE_GRADIENT_WIDTH` 比較箇所）
  - `crates/hw_world/src/mapgen/pipeline.rs`（`ZONE_GRADIENT_WIDTH` / `compute_anchor_distance_field` 利用箇所）

- 完了条件:
  - [ ] 定数の現行値・初期候補値・理由が一覧化されている
  - [ ] `wfc_adapter.rs` / `pipeline.rs` の参照箇所が定数変更で吸収できると確認できている
  - [ ] `compute_protection_band` を追加で触る必要があるか否かの判断条件が明文化されている

- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

## M2: ゾーン形状の自然化実装

- 変更内容（実装 step 順）:

  **Step 1: `distance_field_from_mask` を Chamfer 8 近傍ダイクストラに置換**
  ```
  // 変更前
  use std::collections::VecDeque;
  const DIRS: [(i32,i32);4] = [(0,1),(0,-1),(1,0),(-1,0)];
  // → VecDeque BFS, コスト 1

  // 変更後
  use std::collections::BinaryHeap;
  use std::cmp::Reverse;
  const DIRS_8: [((i32,i32),u32);8] = [
      ((0,1),3),((0,-1),3),((1,0),3),((-1,0),3),  // 直交=3
      ((1,1),4),((1,-1),4),((-1,1),4),((-1,-1),4), // 斜め=4
  ];
  // → BinaryHeap<Reverse<(u32,i32,i32)>> によるダイクストラ
  ```
  これにより `compute_anchor_distance_field` / `compute_zone_distance_field` が
  自動的に Chamfer 距離を返すようになる。

  **Step 2: `expand_mask` を同パターンに置換**
  `radius` の意味が「マンハッタン距離」から「chamfer コスト閾値」に変わる。
  M1 で整理した `ZONE_MIN_SEPARATION` 初期候補値を使い、seed 群比較で離隔を再調整する。

  **Step 3: `flood_fill_zone_patches` を 8 近傍コスト付き BFS に置換**
  `DIRS: 4` → `DIRS_8: 8` に拡張する（最小限）。
  コスト付きにする場合は `BinaryHeap` を用いる（より円形に近いパッチ）。
  `area_max` はセル数基準なので変更不要。

  **Step 4: 定数をM1で整理した新値に更新**
  `ZONE_DIRT_DIST_MIN/MAX`, `ZONE_GRASS_DIST_MIN`, `ZONE_MIN_SEPARATION`, `ZONE_GRADIENT_WIDTH` を
  M1 の初期候補値で更新し、その後の比較で必要なら再調整する。

  **Step 5: 代表 seed で目視確認 + テスト実行**
  ひし形傾向が弱まっているか確認。Visual Cross テストが通るか確認。

- 変更ファイル:
  - `crates/hw_world/src/terrain_zones.rs`（Step 1–4 の主変更）
  - `crates/hw_world/src/mapgen/wfc_adapter.rs`（`fix_zone_mask_crosses` が chamfer 距離場を扱えるか確認、必要なら調整）
  - `crates/hw_world/src/mapgen/pipeline.rs`（距離帯別診断の `bands` 配列や比較基準を新スケールへ更新）
  - `crates/hw_world/src/world_masks.rs`（M2 後もアンカー周辺に diamond artifact が残る場合のみ `compute_protection_band` を再評価）
  - `docs/map_generation.md`

- 完了条件:
  - [ ] 代表 seed でひし形傾向が弱まっている（目視）
  - [ ] 既存 invariant（river/sand/anchor/rock の禁止条件）を維持している
  - [ ] Visual Cross テストが通っている
  - [ ] アンカー周辺や保護帯際に残る局所 diamond artifact の有無が確認されている

- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world generated_layouts_have_no_visual_cross -- --nocapture`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world test_zone_masks_deterministic -- --nocapture`

## M3: 境界描画の補助改善（条件付き）

- 前提: M2 完了後に残る段差が視覚的に問題となる場合のみ実施する。
- 変更内容:
  `boundary.rs` のコーナー簡略化・短い鋸歯のマージ・描画専用の緩い補間強化。
  論理セル境界との乖離を増やさない範囲で調整する。
- 変更ファイル:
  - `crates/bevy_app/src/world/map/boundary.rs`
  - `docs/map_generation.md`
- 完了条件:
  - [ ] 論理境界との乖離を増やさずに視覚段差が軽減している
  - [ ] pure visual 契約が維持されている
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo run` で代表 seed の目視確認

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 定数スケール変換が不正確で zone 面積・分布が大きく変わる | 木・岩・砂の配置バランスが崩れる | M1 では固定値ではなく初期候補として整理する。M2 完了後に seed 群で面積統計（`pipeline.rs` の診断コード）と目視比較を行い、定数を微調整する |
| 斜め展開増加で 2×2 Visual Cross が増える | `generated_layouts_have_no_visual_cross` テスト失敗 | `fix_zone_mask_crosses` (`wfc_adapter.rs` l.266) はゾーンマスク自体を修正するため基本的に対応可能。テスト失敗が続く場合は `MAX_PASSES` (現行 64) の増加かパスロジック強化を検討する |
| `compute_zone_distance_field` の出力変化で C グラデーション帯が変わる | dirt/grass 境界付近のタイル比率が変わる | `ZONE_GRADIENT_WIDTH` を ×3 してから目視確認。必要なら `ZONE_GRADIENT_*_BIAS_PERCENT` を微調整する |
| `pipeline.rs` の診断テスト（距離帯別集計）が新距離スケールで比較できなくなる | 診断結果の解釈が困難になる | `pipeline.rs` 内の `bands` 配列（l.339-346）も新スケールに合わせて更新する |
| `river_protection_band` など 4 近傍保護帯の形が局所 diamond artifact を残す | アンカー周辺だけ不自然さが残る | M2 完了後の目視確認にアンカー周辺を含める。残る場合のみ `world_masks.rs::compute_protection_band` を追加検討する |
| smoothing / open-close を追加した場合に細い通路や禁止帯に食い込む | validate 失敗や見た目破綻 | smoothing は最終 mask 限定・少パスに留め、blocked cell を常に再マスクする |
| 描画補助（M3）が強すぎて論理境界と見た目がずれる | プレイヤーの認知負荷が増える | `boundary.rs` は補助のみに留め、リボン幅・変位量・コーナー処理を現行契約内で調整する |

## 8. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- 推奨テスト:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world generated_layouts_have_no_visual_cross -- --nocapture`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world test_zone_masks_deterministic -- --nocapture`
- 手動確認シナリオ:
  - 固定 seed を複数選び、grass zone の外周が明確なひし形から外れているか確認する
  - 境界が不自然な 1 マスごとの鋸歯列ではなく、2〜4 セル単位でまとまった輪郭になっているか確認する
  - アンカー周辺と `river_protection_band` 際に局所的な diamond artifact が残っていないか確認する
  - 川・砂浜・岩場との境界に新しい破綻が入っていないか確認する
- パフォーマンス確認（必要時）:
  - zone 生成の計算量が大きく増えないかを startup 時間で確認する

## 9. ロールバック方針

- どの単位で戻せるか:
  M1/M2 の `terrain_zones.rs` 系変更と、M3 の `boundary.rs` 変更を分離して戻せるようにする。
- 戻す時の手順:
  まず描画補助（M3）だけを切り戻せる構成にする。
  生成側（M2）を戻す場合は、`terrain_zones.rs` と `docs/map_generation.md` を同時に戻して契約を一致させる。

## 10. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100% (M1/M2 完了、M3 はオプション)`
- 完了済みマイルストーン: M1, M2
- 未着手/進行中: M3（M2 後の目視確認で段差が残る場合のみ実施）

### 次のAIが最初にやること

1. **M1: 定数スケール整理**（コード変更なし、整理のみ）
   - `crates/hw_world/src/terrain_zones.rs` を開き、定数ブロック（l.17-58）を読む
   - セクション 6 M1 の「定数変更候補」テーブルを照合し、現行値が合っているか確認する
   - 候補値は固定値ではなく初期値であることを前提に、代表 seed 比較で再調整する前提を維持する
   - `wfc_adapter.rs` l.607-608 と `pipeline.rs` l.293 を読み、定数変更だけで吸収できると確認する

2. **M2 Step 1: `distance_field_from_mask` の置換（`terrain_zones.rs` l.164-195）**
   ```rust
   // 変更前 import
   use std::collections::VecDeque;
   
   // 変更後 import（追加）
   use std::collections::BinaryHeap;
   use std::cmp::Reverse;
   
   // 変更後 関数本体パターン
   const DIRS_8: [((i32,i32),u32);8] = [
       ((0,1),3),((0,-1),3),((1,0),3),((-1,0),3),
       ((1,1),4),((1,-1),4),((-1,1),4),((-1,-1),4),
   ];
   fn distance_field_from_mask(mask: &BitGrid) -> Vec<u32> {
       let mut dist = vec![u32::MAX; (MAP_WIDTH * MAP_HEIGHT) as usize];
       let mut heap: BinaryHeap<Reverse<(u32, i32, i32)>> = BinaryHeap::new();
       // mask の true セルを距離 0 でキューに積む
       // ダイクストラで伝播
       dist
   }
   ```
   `compute_anchor_distance_field` と `compute_zone_distance_field` はラッパーなので変更不要。

3. **M2 Step 2: `expand_mask` を同パターンに置換**（`terrain_zones.rs` l.214-253）
   `radius` パラメータの意味が chamfer コスト閾値に変わる。
   呼び出し元 `generate_terrain_zone_masks` l.119 で渡している `ZONE_MIN_SEPARATION` が
   M1 の初期候補値に更新済みであることを確認してから変更する。

4. **M2 Step 3: `flood_fill_zone_patches` を 8 近傍に拡張**（`terrain_zones.rs` l.292-324）
   最低限: `DIRS: 4` → `DIRS_8: 8` に置換するだけで diagonal 方向にも展開可能になる。
   よりコスト付きにする場合は `BinaryHeap` を使う（丸みが増す）。

5. **Step 4: 定数更新**（`terrain_zones.rs` l.18-58）
   セクション 6 M1 の表に従って初期候補値へ更新し、seed 比較後に必要なら再調整する。

6. **検証**: `cargo test -p hw_world generated_layouts_have_no_visual_cross` + 目視確認
   - とくにアンカー周辺と保護帯際に局所 diamond artifact が残るか確認する

### ブロッカー/注意点

- **`compute_protection_band` (`world_masks.rs`) は初手では変更しない**。ただし `river_protection_band` は `allowed` を削るため、
  M2 後にアンカー周辺の局所 diamond artifact が残る場合は再評価対象になる。
- `distance_field_from_mask` は `terrain_zones.rs` 専用の private 関数。同名関数が `world_masks.rs` には存在しないが、`compute_protection_band` も 4 近傍 BFS を持つ（独立した実装）。混同しないこと。
- `fix_zone_mask_crosses` は `wfc_adapter.rs` l.266 にある。`terrain_zones.rs` にはない。
- `pipeline.rs` l.339-346 に距離帯別統計診断コード (`bands` 配列) がある。新スケールで数値が変わるため、M2 の変更対象として更新する。

### 参照必須ファイル

- `crates/hw_world/src/terrain_zones.rs`（主変更先、定数と3関数）
- `crates/hw_world/src/mapgen/wfc_adapter.rs`（`fix_zone_mask_crosses` / `ZONE_GRADIENT_WIDTH` 比較箇所）
- `crates/hw_world/src/mapgen/pipeline.rs`（`ZONE_GRADIENT_WIDTH` 比較箇所 / 診断コード）
- `docs/map_generation.md`
- `crates/bevy_app/src/world/map/boundary.rs`（M3 のみ）

### 最終確認ログ

- 最終 `cargo check`: `2026-04-06` / `passed (M2 完了後)`
- 未解決エラー: なし（全 51 テスト通過）

### Definition of Done

- [ ] M1〜M2 が完了し、目的のマイルストーンが全て達成されている
- [ ] `docs/map_generation.md` が新アルゴリズムを反映して更新済み
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功
- [ ] `generated_layouts_have_no_visual_cross` テストが全 seed で通っている

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-06` | `Codex` | 初版作成 |

| `2026-04-06` | `Copilot` | M1/M2 実装完了（Chamfer 3-4 ダイクストラ置換、定数更新、全テスト通過） |
