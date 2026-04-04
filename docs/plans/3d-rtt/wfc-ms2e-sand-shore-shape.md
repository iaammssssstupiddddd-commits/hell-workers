# MS-WFC-2e: 砂浜輪郭依存の緩和

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms2e-sand-shore-shape` |
| ステータス | `Draft` |
| 作成日 | `2026-04-04` |
| 最終更新日 | `2026-04-04` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms2d-river-driven-sand-mask.md`](wfc-ms2d-river-driven-sand-mask.md) |
| 次MS | [`wfc-ms3-procedural-resources.md`](wfc-ms3-procedural-resources.md) |
| 前提 | `WorldMasks::fill_sand_from_river_seed()` と `final_sand_mask` 反映が実装済み（MS-WFC-2d 完了） |

### サマリ

| 項目 | 内容 |
| --- | --- |
| 解決したいこと | 現在の砂浜が `river_mask` の 8 近傍 1 層候補に強く依存し、川の輪郭をそのままなぞる見た目になりやすい |
| 主変更 | `sand_candidate_mask` を「River 輪郭の 8 近傍リング」から「距離場ベースの岸帯 + seed 由来の加算的な浜の膨らみ」へ置き換える |
| 維持するもの | `final_sand_mask` の deterministic 契約、`non-sand carve`、`post_process_tiles()` / `fallback_terrain()` による後段反映、WFC 非依存の砂責務 |
| 期待効果 | 砂浜が River 輪郭のトレースから外れ、より面として見える。対角許容・連続した非砂領域との両立は維持する |
| やらないこと | retry 方針の変更、WFC への新 hard constraint 追加、資源配置ロジック変更、木/岩生成の前倒し |

---

## 1. 背景

MS-WFC-2d により、`Sand` は WFC 出力ではなく `river_mask` 由来の deterministic mask になった。これは責務分離として正しいが、現行実装の候補生成は次の性質を持つ。

- `build_sand_candidate_mask()` が `river_mask` の **8 近傍 1 層**だけを候補化する
- `build_sand_carve_mask()` は候補を **減算**するだけで、外側へ面を広げる操作を持たない
- そのため最終形が「川の輪郭 + 欠け」の見た目になりやすい

結果として、砂浜が「岸辺の帯」ではなく「River 輪郭の縁取り」に見えやすい。これは deterministic 契約そのものではなく、**candidate 生成アルゴリズムの表現力不足**が原因である。

---

## 2. 目的

- `final_sand_mask` を川輪郭トレース中心の形から、**岸帯・砂州・ふくらみを持つ面**へ寄せる
- `non-sand carve` を維持しつつ、砂浜生成に **加算ステップ**を導入する
- `Sand` を WFC 外で決める現行アーキテクチャは維持する
- 100x100 グリッド前提で、WFC コストを増やさずに見た目だけ改善する

---

## 3. 設計方針

### 3.1 基本方針

`final_sand_mask` の生成順を次へ変更する。

1. `river_mask` から **許可セル上の river distance field** を作る
2. 距離 1..=2 を **base shoreline mask** として確保する
3. seed 由来で選んだ少数の起点から、距離 3..=4 までの **sand growth** を加算する
4. その後に既存の `non-sand carve` を適用する
5. `final_sand_mask` を `post_process_tiles()` / `fallback_terrain()` で最終反映する

重要なのは、現在の `candidate - carve` 一辺倒をやめ、**`base + growth - carve`** に変えること。これにより、非砂領域実装とは競合せず、役割分担が明確になる。

### 3.2 なぜこの形か

- **距離場**を使うと、砂が「川輪郭の 1 セル外周」ではなく「川から何セル離れた岸帯か」で表現できる
- **growth** を少数の seed 起点に限定すると、全周一様に太い帯にならず、局所的な砂浜の膨らみを作れる
- **carve を最後に残す**ことで、2d の「連続した non-sand エリアで単調さを崩す」意図をそのまま生かせる

---

## 4. 提案アルゴリズム

### 4.1 river distance field

`river_mask` を多点始点にした BFS で、砂候補許可セルに対する最短距離を求める。対象セルは以下を除く。

- `river_mask`
- `anchor_mask`
- `river_protection_band`
- マップ外

距離は最大 `SAND_SHORE_MAX_DISTANCE` までで打ち切る。初期値は `4` を想定する。

### 4.2 base shoreline mask

距離場から **距離 1..=2** を `base_candidate_mask` とする。これで最低限の岸帯を確保する。

ここでは「必ず残したい砂浜の芯」を作る。現行 8 近傍リング相当の責務はこの層に吸収する。

### 4.3 sand growth mask

`base_candidate_mask` のうち River 側の frontier から deterministic に少数の起点を選び、`distance <= growth_limit` の範囲で bounded flood fill を行う。

- 起点数: 3〜8 程度
- 各起点の成長上限距離: 3〜4
- 各 growth region の面積上限: 定数化
- growth は `anchor_mask` / `river_protection_band` / 既存 `river_mask` を跨がない

これで「川に沿っただけの帯」ではなく、ところどころ広がった砂浜ができる。

### 4.4 non-sand carve の位置づけ

`non-sand carve` は **growth 後** に適用する。

順序は固定する。

1. `base_candidate_mask`
2. `sand_growth_mask`
3. `sand_candidate_mask = base_candidate_mask | sand_growth_mask`
4. `sand_carve_mask`
5. `final_sand_mask = sand_candidate_mask - sand_carve_mask`

`carve` を先に適用すると、growth が非砂領域を埋め戻して意味を壊すため採らない。

### 4.5 smoothing は後回し

形態演算的な smoothing / closing は今回の第一段では入れない。まずは **distance field + additive growth** で輪郭依存を下げる。

理由:

- 加算ステップだけで改善量が大きい
- smoothing は `non-sand carve` の抜けを潰しやすい
- 後段で必要なら `final_sand_mask` ではなく `sand_candidate_mask` 側に限定して追加できる

---

## 5. データ構造と API 変更方針

### 5.1 `WorldMasks`

公開フィールドは増やさない。既存の

- `sand_candidate_mask`
- `sand_carve_mask`
- `final_sand_mask`

をそのまま使う。

意味だけを更新する。

- `sand_candidate_mask`: 「8 近傍リング」ではなく「base shoreline + growth を合成した候補」

### 5.2 `river.rs`

主な変更対象。内部 helper を追加する。

- `compute_river_distance_field(...)`
- `build_base_shoreline_mask(...)`
- `build_sand_growth_mask(...)`
- `merge_candidate_masks(...)` または同等処理

既存の `generate_sand_masks()` シグネチャは維持する。

---

## 6. 期待されるパフォーマンス影響

### 6.1 実行コスト

- 距離場 BFS: `O(MAP_WIDTH * MAP_HEIGHT)`
- growth flood fill: 起点数と距離上限で bounded。マップ全域に対して軽い
- 100x100 マップでは、WFC 本体に比べて十分小さい

### 6.2 実装コスト

- 主変更は `river.rs` に閉じる
- `WorldMasks` / `mapgen.rs` / `wfc_adapter.rs` の公開契約は基本維持
- `validate.rs` は任意で debug warning を 1 つ足す程度で済む

### 6.3 リスク

- growth が強すぎると `Sand` 面積が増えすぎる
- carve 定数が現行のままだと、改善後の candidate 面積に対して抜き量が不足する可能性がある

したがって **growth 定数と carve 定数の再調整**は同じ MS で扱う。

---

## 7. 実装ステップ

### Step 1: 距離場 helper を追加

- `river.rs` に river distance field 計算を追加
- `anchor_mask` / `river_protection_band` / `river_mask` を避ける制約を helper に閉じ込める

### Step 2: candidate 生成を `base + growth` に差し替え

- `build_sand_candidate_mask()` を置き換える
- まず距離 1..=2 の base shoreline を作る
- 次に bounded flood fill で growth mask を追加する

### Step 3: carve 順序を固定して再調整

- 既存の `build_sand_carve_mask()` は残す
- ただし candidate 面積の変化に合わせて seed 数・region size・ratio を見直す

### Step 4: テストを更新

- deterministic
- overlap 禁止
- `final_sand_mask` 反映
- representative seed で「距離 2 以上の `Sand` が存在する」こと

### Step 5: docs を同期

- `world_layout.md`
- `crates/hw_world/README.md`
- 親計画 / ロードマップ

---

## 8. 変更ファイル

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_world/src/river.rs` | 距離場・base shoreline・growth 追加。`generate_sand_masks()` の内部実装更新 |
| `crates/hw_world/src/world_masks.rs` | `sand_candidate_mask` の doc comment を「growth 込み候補」へ更新 |
| `crates/hw_world/src/mapgen.rs` | sand 形状の golden seed / 分布テスト追加 |
| `crates/hw_world/src/mapgen/validate.rs` | 必要なら debug warning を追加。最低限、既存 sand-mask 整合チェックが新 candidate 意味論と矛盾しないことを確認 |
| `docs/world_layout.md` | 砂浜生成説明を 8 近傍リングから distance-field + growth へ更新 |
| `crates/hw_world/README.md` | `river.rs` / `world_masks.rs` の責務説明更新 |
| `docs/plans/3d-rtt/wfc-terrain-generation-plan-2026-04-01.md` | MS-WFC-2e を追加し、2d の次段として接続 |
| `docs/plans/3d-rtt/milestone-roadmap.md` | WFC 系マイルストーン列へ MS-WFC-2e を追加 |

---

## 9. 完了条件

- [ ] 同一 seed で `sand_candidate_mask` / `sand_carve_mask` / `final_sand_mask` が deterministic
- [ ] `final_sand_mask` が `river_mask` / `anchor_mask` / `river_protection_band` と交差しない
- [ ] `final_sand_mask` 上が常に最終 `Sand` になる
- [ ] representative seed で `river` から距離 2 以上の `Sand` が存在する
- [ ] それでも `Sand` が map 全体へ拡散せず、`river` 由来の岸帯として保たれる
- [ ] `non-sand carve` が引き続き連続した非砂領域として機能する
- [ ] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が通る

---

## 10. 検証

- `cargo test -p hw_world`
- `cargo check --workspace`
- `cargo clippy --workspace`

加えて、golden seed で以下を目視確認する。

- 直線に近い川
- 強く蛇行する川
- 保護帯ぎりぎりを通る川

確認観点:

- 砂浜が「輪郭の縁取り」ではなく面として見えるか
- 砂浜の膨らみが局所的に存在するか
- `non-sand carve` により単調な全面砂になっていないか

---

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-04` | `Codex` | 初版作成。砂浜の輪郭依存を下げるため、distance field + additive growth + 既存 carve 維持の方針を整理。 |
