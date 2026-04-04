# MS-WFC-0: 生成 invariant 仕様化

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms0-invariant-spec` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-03-29` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 次MS | [`wfc-ms1-anchor-data-model.md`](wfc-ms1-anchor-data-model.md) |

---

## 1. 目的

WFC 地形生成を実装する前に、生成結果が満たすべき invariant を文書として確定する。
実装・レビュー・テストの判断基準をこの MS で固め、以後のすべての MS がここを参照できる状態にする。

この MS は **コードを一切書かない**。文書のみ。

---

## 2. 成果物（完了条件）

| # | 成果物 | 内容 |
| --- | --- | --- |
| 1 | 必須資源と追加資源の区分定義 | 序盤進行に必須なリソース（水源・砂源・岩源など）と追加（装飾）リソースを別々に定義し、到達保証の要否を明記 |
| 2 | 保護帯の対象と幅定義 | `Site/Yard` 外周に設ける保護帯の距離（タイル数）と禁止要素（River・岩・高密度木）を定数名付きで定義 |
| 3 | lightweight / debug validator の責務区分 | lightweight = 起動時必須チェック、debug = 開発時のみ有効な追加診断、という責務と実行タイミングを文書化 |
| 4 | golden seeds 運用方針 | 固定 seed 集の目的・種類・追加ルール・CI での使い方を定義 |
| 5 | debug レポート仕様 | 生成レポートで出力すべき情報（地形・アンカー・保護帯・資源候補・最終配置）とフォーマット（PNG / テキスト等）を定義 |

---

## 3. 仕様化すべき invariant 一覧

### 3.0 未決定事項に対する初期提案

MS-WFC-0 では「まだ決まっていない」項目を放置せず、**初期提案値**まで置いておく。
後続 MS ではこの提案を起点に調整してよいが、未定義のまま実装へ進まない。

| 項目 | 初期提案 |
| --- | --- |
| 保護帯幅 | `PROTECTION_BAND_RIVER_WIDTH = 3`, `PROTECTION_BAND_ROCK_WIDTH = 2`, `PROTECTION_BAND_TREE_DENSE_WIDTH = 2` |
| 高密度木の定義 | 半径 2 タイル内に木が 4 本以上 |
| 木の再生エリア表現 | 離散点列ではなく、矩形または少数の zone 集合 |
| 初期木配置との関係 | 初期木は必ず `forest_regrowth_zones` 内に置く |
| 到達保証対象 | `Site↔Yard`, `Yard→初期木材`, `Yard→猫車置き場`, `Yard→水源`, `Yard→砂源`, `Yard→岩源` |
| 距離上限 | lightweight では未使用、debug validator のみで距離上限を持つ |
| マップ外周 | 特別 Border タイルを作らず通常地形のまま扱う |
| fallback 可否 | 実行時は許可、test / debug validator では失敗扱い |
| golden seeds 数 | 初版は 4 本（`STANDARD`, `WINDING_RIVER`, `TIGHT_BAND`, `RETRY`） |
| debug レポート必須項目 | `site_mask`, `yard_mask`, `protection_band`, `river_centerline`, `river_mask`, `forest_regrowth_zones`, `resource_spawn_candidates`, `used_fallback` |

### 3.0.1 外部 WFC crate 選定基準

MS-WFC-2 着手前に、候補 crate は少なくとも次を満たすこと:

- MIT / Apache 系ライセンス
- seed 指定が可能
- hard constraint または事前固定セルを扱える
- Bevy 非依存で pure ロジックとして利用できる
- 依存が過度に重くない
- adapter 層に閉じ込められる API 形状である

### 3.1 固定アンカー保護

```
- Site/Yard 内には River・Sand・木・岩を生成しない
- Site/Yard 外周に要素別保護帯を設ける
  → River: PROTECTION_BAND_RIVER_WIDTH = 3
  → 岩: PROTECTION_BAND_ROCK_WIDTH = 2
  → 高密度木: PROTECTION_BAND_TREE_DENSE_WIDTH = 2
- 初期木材・猫車置き場は Yard 内固定オフセットに配置する
```

### 3.1.1 保護帯の測り方（基準）

```
- アンカー占有領域 A = Site ∪ Yard のタイル集合とする。
- 「アンカー外周からの距離」: A に属さないセル p について、p から A のいずれかのセルへ 4 近傍で歩行して到達する最短ステップ数を d とする（マップ外は不可）。d が 1..=PROTECTION_BAND_*_WIDTH の範囲のセル集合を、種別ごとの保護帯（禁止検査対象のリング）として扱う。
  - 禁止対象（River セル・岩オブジェクトの占有セル・高密度木の判定）は、種別に応じた幅の保護帯**内**に入ってはならない（validator は上記距離で判定する）。
  - 複数種別の帯が重なる場合は、各 invariant を個別に満たすこと。
  - データモデル上は `river_protection_band` / `rock_protection_band` / `tree_dense_protection_band` の個別マスクを持ってよく、debug report 上の `protection_band` はそれらの合成結果を指す。
```

### 3.1.2 高密度木の定義

```
- 高密度木 = 半径 2 タイル内に木が 4 本以上存在する状態
- 保護帯では「木を全面禁止」ではなく「高密度木を禁止」として扱う
  → 単木または低密度な木は許可余地あり
```

### 3.2 必須資源到達保証

用語を次のように分離する（混同しないこと）。

```
- 初期木材ストック: Yard 内の固定オフセットに置く資源スタック。配置は常に Yard 内のため「Yard から到達」は Yard 内連結で十分。
- 初期猫車置き場: 同上、Yard 内固定。
- 採取用の木（森林）: seed 依存で密度・位置が変わる木オブジェクト。必ず forest_regrowth_zones 内。序盤必達の「水源・砂・岩」とは別枠。
```

```
必須資源（マップ上の到達保証あり・各最低 1）:
  - 水源: River タイル（最低 1 セル）
  - 砂源: Sand タイル（最低 1 セル）
  - 岩源: 岩オブジェクト（最低 1 個）

追加・可変（到達保証は水源・砂・岩・導線とは独立に seed 依存）:
  - 採取用の木（森林）の本数・配置（forest_regrowth_zones 内、§3.6）
  - その他装飾的資源

到達可能性条件（hw_world::pathfinding の walkable 契約で判定）:
  - Site ↔ Yard が歩行連結
  - Yard 内の任意の歩行可能セルから、初期木材ストックのセルへ歩行経路が存在する（通常 Yard 内のみ）
  - Yard から猫車置き場（占有セルのいずれか）へ歩行経路が存在する
  - Yard から各必須資源（水源・砂源の各最低 1 セル、岩源 1 個の接地セル）へ歩行経路が存在する
```

### 3.2.1 debug validator 用の距離目安

```
- lightweight validator では距離上限を持たない
- debug validator では目安として以下を確認してよい:
  - 水源まで 40 タイル以内
  - 砂源まで 35 タイル以内
  - 岩源まで 45 タイル以内
- 数値は初期提案であり、実装後に調整可能
```

### 3.3 seed 再現性

```
- 同一 master seed → 同一最終マップ（fallback 経由も含む）
- 別 seed → River / Dirt / Sand / 木 / 岩の分布が変化する
- 再試行は master_seed + attempt_index から deterministic な sub-seed を導く
- master seed 自体は変更しない
- 最大再試行回数: MAX_WFC_RETRIES（定数、目安 64）
```

### 3.4 収束失敗時の fallback

```
- MAX_WFC_RETRIES 回失敗時、同一 master seed から deterministic に決まる安全 fallback へ落とす
- fallback では未決定セルを Grass で埋める（最小限の安全マップ）
- debug_assert / tests では fallback 到達を厳格にエラーとして扱う（正常実行では隠さない）
- GeneratedWorldLayout に `used_fallback: bool` を保持する
```

### 3.5 sandbox 境界処理

```
- マップ外周タイルは特別 Border タイルを設けず、通常地形セルとして扱う（§3.0 と同じ）。
- WFC の近傍はグリッド内にクリップする。マップ外に存在しない隣接は「その方向の候補なし」として扱い、外部 crate の API に合わせてアダプタで **仮想境界タイル**（例: 常に Grass か、川マスクで既に固定されたセル）を与えるか、**境界セルだけ許可集合を制限**する。いずれの方式も「同一 master seed では同一結果」になること。
- 角・辺のセルは 4 近傍のうち地図外に落ちる辺を持つが、pathfinding の walkable は従来どおりマップ内のセルのみを対象とする。
```

### 3.6 木の再生エリア

```
- 木の再生可能エリアは `forest_regrowth_zones` として pure データで保持する
- 形は離散固定座標ではなく、矩形または少数の zone 集合で表現する
- 初期木配置は必ず `forest_regrowth_zones` の部分集合である
- regrowth は固定座標群ではなく `forest_regrowth_zones` を参照する
```

---

## 4. golden seeds 定義

| seed 名 | 目的 |
| --- | --- |
| `GOLDEN_SEED_STANDARD` | 標準的な生成結果。通常ゲームプレイに近い状態 |
| `GOLDEN_SEED_WINDING_RIVER` | 川が大きく曲がり、砂帯が広いケース |
| `GOLDEN_SEED_TIGHT_BAND` | 保護帯ぎりぎりに地形・資源が生成されるケース |
| `GOLDEN_SEED_RETRY` | **再試行のみ**発生し、`used_fallback == false` で収束するマスタ seed。`attempt_index > 0` が一度以上起きることを期待値に含める。 |

### `GOLDEN_SEED_RETRY` の CI 安定性

- CI で固定する値は **`used_fallback == false` かつ再試行が 1 回以上**という条件を満たす seed を **オフライン探索で 1 本確定**し、定数としてコミットする。
- そのような seed が見つからない場合は、本 golden を **スキップ**し、issue に「RETRY 用 seed 未確定」と残す（曖昧な「もし再現可能なら」は採用しない）。

### 運用ルール

- `cargo test -p hw_world` で全 golden seeds が validator を通る
- 新 invariant 追加時は影響する seed の期待値を更新する
- CI では毎回すべての golden seed をチェックする

---

## 5. lightweight / debug validator 区分

### lightweight validator（起動時必須）

実行タイミング: startup 生成直後、ゲーム開始前  
失敗時動作: panic（不正マップで起動を継続しない）。**早期失敗・実装バグの即検知**を優先し、リリースでメニューへ戻す等の代替は別 MS で検討可。

| チェック項目 |
| --- |
| Site/Yard 内に River・Sand がない |
| Site ↔ Yard が歩行連結 |
| Yard から初期木材ストックのセルへ到達可能（Yard 内） |
| Yard から猫車置き場へ到達可能 |
| Yard から各必須資源の最低 1 セルへ到達可能 |
| 初期木材・猫車置き場の配置が Yard 内の定義どおりである |

### debug validator（`debug_assertions` / テストのみ）

実行タイミング: `#[cfg(debug_assertions)]` または `#[cfg(test)]`  
失敗時動作: panic または assert_eq でテスト失敗

| チェック項目 |
| --- |
| 保護帯内に River・岩が存在しない |
| 保護帯内に高密度木が存在しない |
| Site/Yard の全セルが Grass または Dirt のみ |
| 木・岩が exclusion zone に存在しない |
| 初期木配置が `forest_regrowth_zones` 内に含まれる |
| 砂タイルの 8 割以上が川に辺接している（目安）|
| 川タイルの総数が seed から決まる目標値に近い |
| fallback に到達していない（再試行 OK だが fallback は NG）|
| debug validator のみ、必須資源までの距離目安を超えていない |

---

## 6. debug レポート仕様

### 出力形式

- **地形レイヤー ASCII**: `println!` で端末出力。地形タイプのみ 1 文字（G=Grass, D=Dirt, R=River, S=Sand）。木・岩は地形グリッドと別レイヤのため、**別ブロックで座標一覧**または行を分けて出力する（1 文字に T/K を詰め込むと地形とオブジェクトが混線する）。
- **PNG（開発時・推奨）**: 色分けで地形・アンカー・保護帯・木・岩・資源候補を**重ね合わせ**て可視化（合成の正は PNG 側）。
  - 出力先: `target/debug_reports/wfc_<master_seed>.png`
  - 色分け例: Grass=緑, Dirt=茶, River=青, Sand=黄, Site=赤枠, Yard=橙枠, 保護帯=半透明緑
- `GeneratedWorldLayout` の pure データから再現可能にする（レポート自体にゲーム状態の副作用を持たせない）

### 必須出力情報

- `site_mask`
- `yard_mask`
- `protection_band`（要素別保護帯の合成表示）
- `river_centerline`
- `river_mask`
- `forest_regrowth_zones`
- `resource_spawn_candidates`
- `used_fallback`

### 出力トリガー

- `--debug-worldgen` コマンドライン引数（または環境変数）で有効化
- `cargo test -p hw_world` のデバッグテスト関数でも生成

---

## 7. 変更ファイル

- `docs/plans/3d-rtt/wfc-ms0-invariant-spec.md`（本ファイル）
- `docs/world_layout.md`: MS-WFC-0 では**短いメモ追記のみ**可。恒久仕様の本更新は親計画の **MS-WFC-4.5**（または同等）で行う。

---

## 8. 完了条件チェックリスト

- [ ] 必須資源と追加資源の区分が本計画書に定義されている
- [ ] 保護帯の対象と幅が定数名とともに定義されている
- [ ] lightweight / debug validator の責務と実行タイミングが分かれている
- [ ] golden seeds の種類と運用ルールが定義されている
- [ ] debug レポートで出力すべき情報と形式が定義されている
- [ ] 木の再生可能エリアの invariant が定義されている
- [ ] 未決定事項に対する初期提案が置かれている
- [ ] 保護帯の測り方（§3.1.1）とマップ境界の WFC 扱い（§3.5）が本文に含まれている

---

## 9. 検証

- 文書レビューのみ（コード変更なし）

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md から分割・詳細化 |
| `2026-04-02` | `Codex` | 未決定事項への初期提案、木の再生エリア invariant、到達保証対象、保護帯詳細、debug レポート必須項目を追記 |
| `2026-03-29` | — | レビュー反映: §3.1.1 保護帯測定、§3.2 用語分離、§3.5 境界 WFC、§4 RETRY 定義、§5/§6/§7 整理、完了条件追記 |
